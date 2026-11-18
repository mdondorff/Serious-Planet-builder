//! SPIKE-132 (spike branch spike/kv-kc, never merged): KV and KC of the river-graph plan (G1f, early) on given graph beds, switchable (KV only, KC only, both), inside the terrain-change
//! budget of the G0 contracts (section 6). Prototype on synthetic graph inputs (addendum section 6): it reads the graph, never redefines its beds.
//!
//! - **KV** (plan sections 3.2 and 3.4): around every L reach vertex a cross-section: a floor of `W_floor = 3 x` GEN-165's width of `q_w` at the bed, walls rising `S_wall` = 0.20 (m per m)
//!   from its edge: `V(x) = bed + S_wall max(0, d(x) - W_floor/2)`, `d` the distance to the nearest vertex (8-neighbour chamfer, ties by the vertex's cell id). KV lowers to `V`, never raises,
//!   with weight zero within `d_buf` = 200 m of a divide (a catchment boundary: cells whose D8 paths reach different reaches, or a reach and the sea) and on protected basins' cells. The
//!   lowering is held at the budget left (`budget - used`); a clamp writes a residual with the nearest reach as its feature.
//! - **KC**: the pits of KV's output that are not protected basins (cells flooded in a depression of the hierarchy), filled to their tree's spill height within `budget` (a fill stage); a deeper fill is clamped at `budget` with a
//!   residual. Patches (connected filled cells) are counted with their volume.
//!
//! Single-threaded, fixed orders (CON-14).

use super::k7::WidthLaw;
use super::river_graph::*;
use super::route::{route_rules, RoundRules, NONE};
use super::Grid;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

/// The plan's KV parameters (section 3.4).
#[derive(Clone, Copy, Debug)]
pub struct KvParams {
    pub s_wall: f64,
    pub w_floor_factor: f64,
    pub d_buf_m: f64,
    /// The cell edge (m) the chamfer distance counts in.
    pub cell_m: f64,
    /// No lowering beyond this distance from a vertex (m): far cells are untouched.
    pub reach_m: f64,
    /// The routing seed of the graph's build (its D8 paths decide the catchments).
    pub seed: u64,
    /// Reading 10 (G0 contracts section 12, lead #165): a divide cell within `W_floor(q_w)/2 + d_buf` of a graph line (the nearest vertex's `q_w`) is valley floor, not a divide, so
    /// it puts no buffer around it; `false`: every divide cell does (SPIKE-132's form).
    pub exempt_near_lines: bool,
    /// Lead #166 (M1: KV cut every side steeper than `s_wall` within `reach_m` to the budget): outside the floor a cell's target is the `s_wall` cone measured as if the cell stood no higher
    /// than its nearest vertex's cell (`want = min(s0[c], s0[v]) - v_target`), so a side is lowered only where the line's own height above the bed, carried up an `s_wall` wall, reaches it;
    /// the floor (`dist <= W_floor/2`) is cut as before. On KS's output the line stands near its bed, so KV leaves the sides; `false`: the cone alone (SPIKE-132's form).
    pub cap_at_incision: bool,
    /// Lead #166 (source fix 5, graph offset misses): the valley floor rises from the line toward its edge at this grade (m per m), so the routing on the floor runs to the line; 0: a flat
    /// floor (SPIKE-132's form). 1e-3 is 0.1 m a cell, 100 times the gate's flood grade, and 0.5 m (one bed code) at the edge of a 500 m half floor.
    pub s_floor: f64,
}

impl KvParams {
    pub fn plan(cell_m: f64) -> KvParams {
        KvParams {
            s_wall: 0.20,
            w_floor_factor: 3.0,
            d_buf_m: 200.0,
            cell_m,
            reach_m: 3000.0,
            seed: 1,
            exempt_near_lines: true,
            cap_at_incision: true,
            s_floor: 1.0e-3,
        }
    }
}

/// Which stages run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    KvOnly,
    KcOnly,
    Both,
}

/// The bound of KC's breach search (reading 22): settled cells per depression.
const BREACH_CAP: usize = 20_000;

/// What a run made.
pub struct KvKc {
    pub kv: Vec<f64>,
    pub kc: Vec<f64>,
    pub kv_wanted: Vec<f64>,
    pub residuals: Vec<Residual>,
    pub divide: Vec<bool>,
    /// The divide cells reading 10 exempts (near a graph line), and the cells within `d_buf` of the divide cells that remain.
    pub divide_exempt: Vec<bool>,
    pub near_divide: Vec<bool>,
    /// KC's patches: (cells, volume m3).
    pub patches: Vec<(usize, f64)>,
    /// KC's breaches (reading 22): the depressions drained by a bounded least-cost breach instead of a fill, and their cut volume (m3).
    pub breaches: usize,
    pub breach_volume: f64,
    /// KC's wanted fill per cell (reading 9: a fill held at the budget is a fill clamp the validator reads).
    pub kc_wanted: Vec<f64>,
    /// The nearest L reach of each cell KV reached, as an index into the graph's L reaches sorted by id (`u32::MAX`: none; order #161's memory cut).
    pub nearest: Vec<u32>,
}

/// The bed level (m) of a code (GEN-170: 0.5 m codes).
fn bed_m(code: i32) -> f64 {
    f64::from(code) * 0.5
}

/// The chamfer distance (m) from the given sources, with each cell's nearest source; ties by the source's cell id.
fn chamfer(grid: &Grid, sources: &[(usize, usize)], cell_m: f64, limit_m: f64) -> (Vec<f64>, Vec<u32>) {
    let n = grid.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut src = vec![u32::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u64, usize, usize)>> = BinaryHeap::new();
    for &(c, tag) in sources {
        if dist[c] > 0.0 || (tag as u32) < src[c] {
            dist[c] = 0.0;
            src[c] = tag as u32;
            heap.push(Reverse((0, tag, c)));
        }
    }
    // distances as integer millimetres in the heap key: fixed order
    while let Some(Reverse((dmm, tag, c))) = heap.pop() {
        let d = dmm as f64 / 1000.0;
        if d > dist[c] || (d == dist[c] && tag as u32 != src[c]) {
            continue;
        }
        for k in 0..8 {
            let Some(q) = grid.neighbour(c, k) else { continue };
            let step = if k < 4 { cell_m } else { cell_m * core::f64::consts::SQRT_2 };
            let nd = libm::round((d + step) * 1000.0) / 1000.0;
            if nd > limit_m {
                continue;
            }
            if nd < dist[q] || (nd == dist[q] && (tag as u32) < src[q]) {
                dist[q] = nd;
                src[q] = tag as u32;
                heap.push(Reverse(((nd * 1000.0) as u64, tag, q)));
            }
        }
    }
    (dist, src)
}

/// Runs KV and KC on `s0` (the stage input; KS off in the prototype) around the L reaches of `g`, within `budget` per cell. `protected` marks the protected basins' cells; `outlet` the sea.
pub fn run(grid: &Grid, s0: &[f64], outlet: &[bool], g: &Graph, protected: &[bool], budget: &[f64], p: &KvParams, switch: Switch) -> KvKc {
    run_with(grid, s0, outlet, g, protected, budget, None, p, switch)
}

/// [`run`] with the budget's reference (reading 21, owner #161): `reference` is `S_KH`, and the lowering budget left at a cell is `budget - max(0, S_KH - s0)` (what KS used before
/// KV and KC). `None`: the budget is the budget left already.
#[allow(clippy::too_many_arguments)]
pub fn run_with(
    grid: &Grid,
    s0: &[f64],
    outlet: &[bool],
    g: &Graph,
    protected: &[bool],
    budget: &[f64],
    reference: Option<&[f64]>,
    p: &KvParams,
    switch: Switch,
) -> KvKc {
    let n = s0.len();
    // the lowering budget left before KV (reading 21)
    let left_at = |c: usize| (budget[c] - reference.map_or(0.0, |r| (r[c] - s0[c]).max(0.0))).max(0.0);
    let law = WidthLaw::STANDARD;
    let mut reaches: Vec<&Reach> = g.reaches.iter().filter(|r| r.level == Level::L).collect();
    reaches.sort_by_key(|r| r.id);
    // vertices: (cell, reach index, vertex index); the first claim of a cell by reach id order
    let mut vtx: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for (ri, r) in reaches.iter().enumerate() {
        for (k, &c) in r.vertices.iter().enumerate() {
            vtx.entry(c as usize).or_insert((ri, k));
        }
    }
    let sources: Vec<(usize, usize)> = vtx.keys().map(|&c| (c, c)).collect();
    let (dist, src) = chamfer(grid, &sources, p.cell_m, p.reach_m);
    // catchments: the river chain a cell's D8 path (round rules on s0, the graph's seed) reaches first, within 1.5 cells of a vertex; a chain is the reaches joined across tile
    // crossings (one tributary from its source to its junction, confluence or outlet); the sea is its own catchment
    let node_kind: BTreeMap<u64, NodeKind> = g.nodes.iter().map(|n| (n.id, n.kind)).collect();
    let from: BTreeMap<u64, usize> = reaches.iter().enumerate().map(|(i, r)| (r.up, i)).collect();
    let chain_of = |ri: usize| {
        let mut x = ri;
        let mut steps = 0;
        while node_kind.get(&reaches[x].down) == Some(&NodeKind::TileCrossing) && steps < reaches.len() {
            match from.get(&reaches[x].down) {
                Some(&next) => x = next,
                None => break,
            }
            steps += 1;
        }
        reaches[x].id
    };
    // chains as dense u32 indices (order #161's memory cut: catch holds 4 bytes a cell)
    let chain_ids: Vec<u64> = (0..reaches.len()).map(chain_of).collect();
    let mut dense: BTreeMap<u64, u32> = BTreeMap::new();
    for &id in &chain_ids {
        let next = dense.len() as u32;
        dense.entry(id).or_insert(next);
    }
    let chain: Vec<u32> = chain_ids.iter().map(|id| dense[id]).collect();
    let routing = route_rules(grid, s0, outlet, 1e-3, p.seed, 0.0, None, RoundRules::ROUND);
    let reached = 1.5 * p.cell_m;
    let mut catch = vec![u32::MAX - 1; n];
    for c in 0..n {
        let mut x = c;
        let mut steps = 0;
        loop {
            if dist[x] <= reached && src[x] != u32::MAX {
                catch[c] = chain[vtx[&(src[x] as usize)].0];
                break;
            }
            let to = routing.receiver[x];
            if to == NONE || steps > n {
                break;
            }
            x = to as usize;
            steps += 1;
        }
    }
    drop(routing);
    let divide: Vec<bool> =
        (0..n).map(|c| !outlet[c] && (0..8).filter_map(|k| grid.neighbour(c, k)).any(|q| !outlet[q] && catch[q] != catch[c])).collect();
    drop(catch);
    // reading 10: a divide cell near a graph line (within half the valley floor plus d_buf of its nearest vertex) is valley floor, not a divide
    let divide_exempt: Vec<bool> = (0..n)
        .map(|c| {
            divide[c] && p.exempt_near_lines && src[c] != u32::MAX && {
                let (ri, k) = vtx[&(src[c] as usize)];
                dist[c] <= 0.5 * p.w_floor_factor * law.width_m(reaches[ri].attrs.q_w[k]) + p.d_buf_m
            }
        })
        .collect();
    let near_divide: Vec<bool> = {
        let div_src: Vec<(usize, usize)> = (0..n).filter(|&c| divide[c] && !divide_exempt[c]).map(|c| (c, c)).collect();
        let (ddiv, _) = chamfer(grid, &div_src, p.cell_m, p.d_buf_m + p.cell_m);
        (0..n).map(|c| ddiv[c] < p.d_buf_m).collect()
    };
    // KV
    let mut kv = s0.to_vec();
    let mut kv_wanted = vec![0.0; n];
    let mut residuals = Vec::new();
    let mut nearest = vec![u32::MAX; n];
    let kv_on = switch != Switch::KcOnly;
    for c in 0..n {
        if src[c] == u32::MAX {
            continue;
        }
        let (ri, k) = vtx[&(src[c] as usize)];
        let r = reaches[ri];
        nearest[c] = ri as u32;
        if !kv_on || outlet[c] || protected[c] || near_divide[c] {
            continue;
        }
        let half = 0.5 * p.w_floor_factor * law.width_m(r.attrs.q_w[k]);
        let bed = bed_m(r.attrs.bed[k]);
        let v = bed + p.s_floor * dist[c].min(half) + p.s_wall * (dist[c] - half).max(0.0);
        let top = if p.cap_at_incision && dist[c] > half { s0[c].min(s0[src[c] as usize]) } else { s0[c] };
        let want = (top - v).max(0.0);
        if want <= 0.0 {
            continue;
        }
        kv_wanted[c] = want;
        let left = left_at(c);
        let low = want.min(left);
        kv[c] = s0[c] - low;
        if want > low + BUDGET_EPS_M {
            residuals.push(Residual { cell: c, stage: "kv", needed_m: want, available_m: left, feature: r.id });
        }
    }
    drop(dist);
    drop(src);
    let feature_of = |c: usize| if nearest[c] == u32::MAX { u64::MAX } else { reaches[nearest[c] as usize].id };
    // KC: the pits of KV's output outside protected basins, filled within the budget
    let mut kc = kv.clone();
    let mut patches = Vec::new();
    let (mut breaches, mut breach_volume) = (0usize, 0.0f64);
    let mut kc_wanted = vec![0.0; n];
    if switch != Switch::KvOnly {
        // reading 22 (owner, #161): first a bounded least-cost breach for each depression of KV's output where it fits the lowering budget left (after KS and KV) and cuts less
        // than its fill would raise; then the remaining pits filled within the budget, as before
        let patches_of = |z: &[f64]| -> (Vec<f64>, Vec<u32>, Vec<Vec<usize>>) {
            // each cell's fill level (its tree's spill; -inf for a drained or protected cell) and the connected patches the fill raises
            let hy = super::basins_rg::hierarchy(grid, z, outlet);
            let level: Vec<f64> = (0..n)
                .map(|c| {
                    let mut k = hy.node_of[c];
                    if k == super::basins_rg::DRAINED || protected[c] {
                        return f64::NEG_INFINITY;
                    }
                    while hy.nodes[k as usize].parent != super::basins_rg::DRAINED {
                        k = hy.nodes[k as usize].parent;
                    }
                    hy.nodes[k as usize].spill.map_or(f64::NEG_INFINITY, |sp| sp.1)
                })
                .collect();
            drop(hy);
            let mut patch_of = vec![u32::MAX; n];
            let mut patches: Vec<Vec<usize>> = Vec::new();
            for c0 in 0..n {
                if patch_of[c0] != u32::MAX || protected[c0] || level[c0] <= z[c0] {
                    continue;
                }
                let id = patches.len() as u32;
                let mut cells = Vec::new();
                let mut stack = vec![c0];
                patch_of[c0] = id;
                while let Some(c) = stack.pop() {
                    cells.push(c);
                    for q in (0..8).filter_map(|k| grid.neighbour(c, k)) {
                        if patch_of[q] == u32::MAX && !protected[q] && level[q] > z[q] {
                            patch_of[q] = id;
                            stack.push(q);
                        }
                    }
                }
                patches.push(cells);
            }
            (level, patch_of, patches)
        };
        let remaining: Vec<f64> = (0..n).map(|c| (left_at(c) - (s0[c] - kv[c]).max(0.0)).max(0.0)).collect();
        {
            let (level, patch_of, ps) = patches_of(&kv);
            for (id, cells) in ps.iter().enumerate() {
                let fill_ok = cells.iter().all(|&c| level[c] - kv[c] <= budget[c].max(0.0) + BUDGET_EPS_M);
                let fill_vol: f64 = cells.iter().map(|&c| (level[c] - kv[c]) * grid.cell_area(c)).sum();
                let pit = *cells.iter().min_by(|&&a, &&b| kv[a].total_cmp(&kv[b]).then(a.cmp(&b))).unwrap();
                let floor = kv[pit];
                let found = super::basins_rg::breach_path(
                    grid,
                    &kv,
                    outlet,
                    pit,
                    floor,
                    &remaining,
                    &|c| patch_of[c] == id as u32,
                    &|c| protected[c],
                    BREACH_CAP,
                );
                if let Some((vol, path)) = found {
                    if vol < fill_vol || !fill_ok {
                        for &c in &path {
                            if kc[c] > floor && !outlet[c] {
                                kc[c] = floor;
                            }
                        }
                        breaches += 1;
                        breach_volume += vol;
                    }
                }
            }
        }
        let (level, patch_of, ps) = patches_of(&kc);
        drop(patch_of);
        let base = kc.clone();
        for cells in ps {
            for &c in &cells {
                kc_wanted[c] = level[c] - base[c];
            }
            let mut vol = 0.0f64;
            for &c in &cells {
                let need = level[c] - base[c];
                let fill = need.min(budget[c].max(0.0));
                kc[c] = base[c] + fill;
                vol += fill * grid.cell_area(c);
                if need > fill + BUDGET_EPS_M {
                    let feature = feature_of(c);
                    residuals.push(Residual { cell: c, stage: "kc", needed_m: need, available_m: budget[c].max(0.0), feature });
                }
            }
            patches.push((cells.len(), vol));
        }
    }
    KvKc { kv, kc, kv_wanted, residuals, divide, divide_exempt, near_divide, patches, breaches, breach_volume, kc_wanted, nearest }
}

/// What [`flag_residuals`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Flagged {
    /// Reaches that now carry `budget_residual`.
    pub reaches: usize,
    /// Of them, reaches that were already exceptional for another reason (reading 9: both kept in the list).
    pub with_other: usize,
    /// Residuals whose feature is no reach of the graph (`u64::MAX`: no reach within KV's reach).
    pub no_feature: usize,
}

/// Flags every residual's feature `exception(budget_residual)` in `g` (contract section 6, reading 9; lead #159): the value is the largest excess `needed - available` of its residuals. A
/// reach already carrying `budget_residual` keeps the larger value. One pass over the residuals; reaches looked up by id.
pub fn flag_residuals(g: &mut Graph, residuals: &[Residual]) -> Flagged {
    let at: std::collections::HashMap<u64, usize> = g.reaches.iter().enumerate().map(|(i, r)| (r.id, i)).collect();
    let mut excess: BTreeMap<usize, f64> = BTreeMap::new();
    let mut out = Flagged::default();
    for r in residuals {
        match at.get(&r.feature) {
            Some(&i) => {
                let e = excess.entry(i).or_insert(f64::NEG_INFINITY);
                *e = e.max(r.needed_m - r.available_m);
            }
            None => out.no_feature += 1,
        }
    }
    for (i, need) in excess {
        let x = &mut g.reaches[i];
        if !x.has_exception(Reason::BudgetResidual) && !x.exceptions.is_empty() {
            out.with_other += 1;
        }
        match x.exceptions.iter_mut().find(|e| e.reason == Reason::BudgetResidual) {
            Some(e) => e.value = e.value.max(need),
            None => x.exceptions.push(Exception { reason: Reason::BudgetResidual, value: need }),
        }
        out.reaches += 1;
    }
    out
}
