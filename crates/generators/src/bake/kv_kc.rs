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
}

impl KvParams {
    pub fn plan(cell_m: f64) -> KvParams {
        KvParams { s_wall: 0.20, w_floor_factor: 3.0, d_buf_m: 200.0, cell_m, reach_m: 3000.0, seed: 1 }
    }
}

/// Which stages run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    KvOnly,
    KcOnly,
    Both,
}

/// What a run made.
pub struct KvKc {
    pub kv: Vec<f64>,
    pub kc: Vec<f64>,
    pub kv_wanted: Vec<f64>,
    pub residuals: Vec<Residual>,
    pub divide: Vec<bool>,
    pub near_divide: Vec<bool>,
    /// KC's patches: (cells, volume m3).
    pub patches: Vec<(usize, f64)>,
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
    let near_divide: Vec<bool> = {
        let div_src: Vec<(usize, usize)> = (0..n).filter(|&c| divide[c]).map(|c| (c, c)).collect();
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
        let v = bed_m(r.attrs.bed[k]) + p.s_wall * (dist[c] - half).max(0.0);
        let want = (s0[c] - v).max(0.0);
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
    if switch != Switch::KvOnly {
        // the pits from the depression hierarchy (flats are no depressions): a cell flooded in a depression, not protected, fills to its tree's spill height
        let hy = super::basins_rg::hierarchy(grid, &kv, outlet);
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
        let filled = &level;
        let pit = |c: usize| filled[c] > kv[c];
        let mut seen = vec![false; n];
        for c0 in 0..n {
            if seen[c0] || protected[c0] || !pit(c0) {
                continue;
            }
            // one patch: the connected cells the flood raises, outside protected basins
            let (mut cells, mut vol) = (0usize, 0.0f64);
            let mut stack = vec![c0];
            seen[c0] = true;
            while let Some(c) = stack.pop() {
                cells += 1;
                let need = filled[c] - kv[c];
                let fill = need.min(budget[c].max(0.0));
                kc[c] = kv[c] + fill;
                vol += fill * grid.cell_area(c);
                if need > fill + BUDGET_EPS_M {
                    let feature = feature_of(c);
                    residuals.push(Residual { cell: c, stage: "kc", needed_m: need, available_m: budget[c].max(0.0), feature });
                }
                for q in (0..8).filter_map(|k| grid.neighbour(c, k)) {
                    if !seen[q] && !protected[q] && pit(q) {
                        seen[q] = true;
                        stack.push(q);
                    }
                }
            }
            patches.push((cells, vol));
        }
    }
    KvKc { kv, kc, kv_wanted, residuals, divide, near_divide, patches, nearest }
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
