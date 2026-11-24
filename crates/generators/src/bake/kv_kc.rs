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
// spike only (never merged): kept as run, so the runs keep their bits; main gets its own form through an OpenSpec change
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use super::k7::WidthLaw;
use super::river_graph::*;
use super::route::{route_rules, RoundRules, NONE};
use super::Grid;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

/// Lead #184a (width form 1 with A2; spike only, off by default): per cell a cap on KV's wall grade, S0's side slope there, so no carve makes a cell's 10 m cross-section
/// narrower than S0's (a gentler wall lowers more of the side, never less); a field of another length than the grid is ignored. `None`: `s_wall` alone.
pub static WALL_CAP: std::sync::RwLock<Option<std::sync::Arc<Vec<f64>>>> = std::sync::RwLock::new(None);

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
    /// Lead #166 (M1: KV cut every side steeper than `s_wall` within `reach_m` to the budget): outside the floor a cell is lowered at most by the larger of the cone measured as if the cell
    /// stood no higher than its nearest vertex's cell (`min(s0[c], s0[v]) - v_target`) and the floor's own lowering carried outward, falling `s_wall` a metre; the floor
    /// (`dist <= W_floor/2`) is cut as before. A side then follows the line's height above its bed and the floor's edge, never a cone to `reach_m`; `false`: the cone alone (SPIKE-132's form).
    pub cap_at_incision: bool,
    /// Lead #166 (source fix 5, graph offset misses): the valley floor rises from the line toward its edge at this grade (m per m), so the routing on the floor runs to the line; 0: a flat
    /// floor (SPIKE-132's form). 1e-3 is 0.1 m a cell, 100 times the gate's flood grade, and 0.5 m (one bed code) at the edge of a 500 m half floor.
    pub s_floor: f64,
    /// Lead #166 (source fix 3, KC breach misses): KC's breach first searches a path within one cell of a graph vertex (or inside the depression), so the drained depression's outlet runs
    /// along the graph's line; only without one the free least-cost breach of reading 22; `false`: the free breach alone.
    pub breach_along_line: bool,
    /// Lead #169 item 3 (R3a: on flat floors the routing runs parallel to a lower line): after the lowering, every floor cell stands below each floor neighbour farther from the line by at
    /// least this grade (m per m) times the distance gained (at most `s_floor`: a larger grade lowers the whole floor below its cone, numerics review), so the floor falls toward the line from any channel on it; lowering only, never below the nearest vertex's bed, within the budget
    /// left. 0: off (the cone alone).
    pub cross_grade: f64,
    /// DEC crusader-19 (lead, 2026-11-19; a switch, default off): the cross rule also holds on side cells within this distance (m) of a vertex that stand at most
    /// `cross_side_grade` a metre above the vertex (a flat side, where the tributaries' floor is under a cell wide); 0: the floor alone.
    pub cross_side_m: f64,
    pub cross_side_grade: f64,
    /// Lead #173 (spikes #152, R3a: rises of one bed code on coastal flats): KV never cuts a vertex's target below this height (m; the sea level with --kv-sea-floor), so a
    /// bed coded under the sea makes no pit beside it that KC fills back; `-inf`: the bed as coded.
    pub bed_floor_m: f64,
    /// Lead #173 (R1h: KC refills the cells KV cut beside a kept lake up to its tree's spill): KC's depressions take the protected cells as outlets, so a cut that drains into
    /// a kept lake is no pit; `false`: the protected cells only stay unfilled.
    pub kc_lake_outlets: bool,
    /// Lead #177 (reading 38, spikes' c4c in KS): on each diagonal step of an L reach, one of the two cells sharing an edge with both vertices is lowered to the step's mean bed
    /// (the lower of the two on KV's input, ties by cell), within the budget left and above a rim floor, so the line's water passes through an edge and the mesh draws no corner
    /// link; `false`: the vertices alone.
    pub connect4: bool,
    /// Lead #178 item 1 (reading 17 in KV and KC, as KS): no land cell (not an outlet) is lowered to this height or below, by KV's cut, its 4-connected rule, the cross pass or
    /// KC's breach (the sea level + 0.5 m); a cell already there is not lowered. A KV cut the clamp stops is a residual; a breach that would cross such a cell is not taken. `-inf`: off.
    pub sea_clamp_m: f64,
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
            breach_along_line: true,
            cross_grade: 0.0,
            cross_side_m: 0.0,
            cross_side_grade: 0.0,
            bed_floor_m: f64::NEG_INFINITY,
            kc_lake_outlets: false,
            connect4: false,
            sea_clamp_m: f64::NEG_INFINITY,
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
    /// The cross-slope pass (lead #169 item 3): cells it lowered, and its volume (m3).
    pub cross_cells: usize,
    pub cross_volume: f64,
    /// The cells where the budget left stopped the pass above its target, and their shortfall summed (m): the parallel channel may stay there (numerics review of 8630f2a8).
    pub cross_short: usize,
    pub cross_short_m: f64,
    /// The pass's cut per cell (m; empty when the pass is off), for the sheet of DEC crusader-19.
    pub cross_cut: Vec<f32>,
    /// Lead #173 (spikes #152: trunk-image vertices left uncut mid-reach): every L vertex cell above its bed by more than 0.25 m before KV, by why KV did or did not cut it:
    /// (label, trunk-image vertices, other vertices). A cell is counted once, under its first claiming reach.
    pub census: Vec<(&'static str, usize, usize)>,
    /// Lead #175: the census per vertex cell: (cell, class index into `census`, trunk image, bed m, height before KV, after KV).
    pub vertex_class: Vec<(usize, u8, bool, f64, f64, f64)>,
    /// Lead #177: the edge cells the 4-connected rule lowered.
    pub connect_cells: usize,
    /// Lead #178 item 1: KV's cuts reading 17's clamp stopped (each a residual).
    pub sea_clamped: usize,
}

/// The least fall (m per m) the cross-slope pass keeps toward the line where it reaches the vertex's height: the gate's flood grade (1e-5; `s_floor` is 100 times it).
const CROSS_MIN_GRADE: f64 = 1.0e-5;

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

/// Values carried outward from `seeds` (cell, value m) through the cells `pass` admits, falling `grade` a metre of chamfer step (`cell_m`, diagonals sqrt 2); each cell holds the largest
/// arrival, 0 where none arrives. Values as integer millimetres in the heap key: a fixed order. `grade * cell_m` must exceed a millimetre, or a value would spread undiminished.
pub fn carry_down(grid: &Grid, seeds: &[(usize, f64)], grade: f64, cell_m: f64, pass: &dyn Fn(usize) -> bool) -> Vec<f64> {
    assert!(grade * cell_m >= 1.0e-3, "carry_down: a step must fall at least a millimetre");
    let mut val = vec![0u64; grid.len()];
    let mut heap: BinaryHeap<(u64, Reverse<usize>)> = BinaryHeap::new();
    for &(c, v) in seeds {
        let mm = libm::round(v.max(0.0) * 1000.0) as u64;
        if mm > val[c] {
            val[c] = mm;
            heap.push((mm, Reverse(c)));
        }
    }
    while let Some((mm, Reverse(c))) = heap.pop() {
        if mm < val[c] {
            continue;
        }
        for k in 0..8 {
            let Some(q) = grid.neighbour(c, k).filter(|&q| pass(q)) else { continue };
            let step = if k < 4 { cell_m } else { cell_m * core::f64::consts::SQRT_2 };
            let nv = mm as f64 / 1000.0 - grade * step;
            if nv <= 0.0 {
                continue;
            }
            let nmm = libm::round(nv * 1000.0) as u64;
            if nmm > val[q] {
                val[q] = nmm;
                heap.push((nmm, Reverse(q)));
            }
        }
    }
    val.into_iter().map(|mm| mm as f64 / 1000.0).collect()
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
    run_with_rims(grid, s0, outlet, g, protected, budget, reference, p, switch, None)
}

/// [`run_with`] with a rim floor (lead #176a, the owner's condition on option C): no cell is lowered below `rim_floor` (by KV's cut or KC's breach), so a kept basin's rim stays at its
/// recorded spill; `-inf` (or `None`) where no rim is.
#[allow(clippy::too_many_arguments)]
pub fn run_with_rims(
    grid: &Grid,
    s0: &[f64],
    outlet: &[bool],
    g: &Graph,
    protected: &[bool],
    budget: &[f64],
    reference: Option<&[f64]>,
    p: &KvParams,
    switch: Switch,
    rim_floor: Option<&[f64]>,
) -> KvKc {
    let rim = |c: usize| rim_floor.map_or(f64::NEG_INFINITY, |f| f[c]);
    // lead #178 item 1: reading 17's floor of a land cell at height z (the clamp, or z itself where z is at or under it); -inf on an outlet
    debug_assert!(!p.sea_clamp_m.is_nan(), "sea_clamp_m is NaN");
    let sea_lo = |c: usize, z: f64| if outlet[c] { f64::NEG_INFINITY } else { p.sea_clamp_m.min(z) };
    let mut sea_clamped = 0usize;
    let n = s0.len();
    // the lowering budget left before KV (reading 21)
    let left_at = |c: usize| (budget[c] - reference.map_or(0.0, |r| (r[c] - s0[c]).max(0.0))).max(0.0);
    let law = WidthLaw::STANDARD;
    let wall_cap = WALL_CAP.read().ok().and_then(|g| g.clone()).filter(|c| c.len() == n);
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
    // an eligible cell's reach, whether it lies on the floor, the cone's want, and outside the floor the cone measured from no higher than its nearest vertex's cell (lead #166)
    let wants = |c: usize| -> Option<(usize, bool, f64, f64)> {
        if src[c] == u32::MAX || !kv_on || outlet[c] || protected[c] || near_divide[c] {
            return None;
        }
        let (ri, k) = vtx[&(src[c] as usize)];
        let r = reaches[ri];
        let half = 0.5 * p.w_floor_factor * law.width_m(r.attrs.q_w[k]);
        let bed = bed_m(r.attrs.bed[k]).max(p.bed_floor_m);
        let s_wall = wall_cap.as_ref().map_or(p.s_wall, |cap| p.s_wall.min(cap[c]));
        let v = bed + p.s_floor * dist[c].min(half) + s_wall * (dist[c] - half).max(0.0);
        let cone = (s0[c] - v).max(0.0);
        let floor = dist[c] <= half;
        let capped = if floor { cone } else { (s0[c].min(s0[src[c] as usize]) - v).max(0.0) };
        Some((ri, floor, cone, capped))
    };
    // the floor's lowering carried outward, falling s_wall a metre, so a side next to a lowered floor follows it down (no cliff at the floor's edge; lead #166)
    let carry: Vec<f64> = if p.cap_at_incision {
        // seeded with the floor's cut within the budget left, entering only cells KV acts on (no divide buffer, protected basin or sea; within reach_m of a vertex)
        let seeds: Vec<(usize, f64)> =
            (0..n).filter_map(|c| wants(c).filter(|w| w.1 && w.2 > 0.0).map(|w| (c, w.2.min(left_at(c))))).collect();
        carry_down(grid, &seeds, p.s_wall, p.cell_m, &|q| wants(q).is_some())
    } else {
        Vec::new()
    };
    for c in 0..n {
        if src[c] != u32::MAX {
            nearest[c] = vtx[&(src[c] as usize)].0 as u32;
        }
        let Some((ri, floor, cone, capped)) = wants(c) else { continue };
        let r = reaches[ri];
        let want = if !p.cap_at_incision || floor { cone } else { cone.min(capped.max(carry[c])) };
        if want <= 0.0 {
            continue;
        }
        // lead #176a: a kept basin's rim is not lowered below its spill (the want itself stops there, so only the budget makes a residual)
        let want = want.min((s0[c] - rim(c)).max(0.0));
        kv_wanted[c] = want;
        let left = left_at(c);
        // lead #178 item 1: the clamp, a residual where it binds
        let avail = left.min(s0[c] - sea_lo(c, s0[c]));
        sea_clamped += usize::from(want > avail + BUDGET_EPS_M && avail < left);
        let low = want.min(avail);
        // exact at the clamp (numerics review: s0 - (s0 - clamp) can land an ulp under it)
        kv[c] = (s0[c] - low).max(sea_lo(c, s0[c]));
        if want > low + BUDGET_EPS_M {
            residuals.push(Residual { cell: c, stage: "kv", needed_m: want, available_m: avail, feature: r.id });
        }
    }
    drop(carry);
    // lead #177: the 4-connected rule on KV's diagonal steps; the claims are taken from KV's input and the lowest one per cell applied, so the order changes nothing
    let mut connect_cells = 0usize;
    if p.connect4 && kv_on {
        let orth = |c: usize| -> Vec<usize> { (0..4).filter_map(|k| grid.neighbour(c, k)).collect() };
        let mut claim: BTreeMap<usize, (f64, u64)> = BTreeMap::new();
        for r in &reaches {
            for k in 0..r.vertices.len().saturating_sub(1) {
                let (a, b) = (r.vertices[k] as usize, r.vertices[k + 1] as usize);
                let oa = orth(a);
                if oa.contains(&b) || !(4..8).any(|d| grid.neighbour(a, d) == Some(b)) {
                    continue;
                }
                let ob = orth(b);
                let mut shared: Vec<usize> = oa.into_iter().filter(|c| ob.contains(c)).collect();
                if shared.is_empty() {
                    continue;
                }
                shared.sort_by(|&x, &y| s0[x].total_cmp(&s0[y]).then(x.cmp(&y)));
                let c = shared[0];
                if protected[c] || outlet[c] {
                    continue;
                }
                let mean = 0.5 * (bed_m(r.attrs.bed[k]) + bed_m(r.attrs.bed[k + 1]));
                let e = claim.entry(c).or_insert((f64::INFINITY, r.id));
                if mean < e.0 || (mean == e.0 && r.id < e.1) {
                    *e = (mean, r.id);
                }
            }
        }
        for (&c, &(mean, feature)) in &claim {
            let target = mean.max(rim(c));
            let want = s0[c] - target;
            if want <= 0.0 || kv[c] <= target {
                continue;
            }
            let left = left_at(c);
            let avail = left.min(s0[c] - sea_lo(c, s0[c]));
            let low = want.min(avail);
            let to = (s0[c] - low).max(sea_lo(c, s0[c]));
            if to < kv[c] {
                kv[c] = to;
                kv_wanted[c] = kv_wanted[c].max(want);
                connect_cells += 1;
                if want > low + BUDGET_EPS_M {
                    sea_clamped += usize::from(avail < left);
                    residuals.push(Residual { cell: c, stage: "kv", needed_m: want, available_m: avail, feature });
                }
            }
        }
    }
    // lead #173: the census of the vertex cells KV should cut to their bed
    let census = {
        let labels = [
            "cut to the bed (within 0.25 m)",
            "cut short by the budget left (a residual)",
            "sea or window outlet",
            "protected (a kept basin or H1 lake cell)",
            "divide buffer",
            "no budget left after KS",
            "claimed by a reach with a higher bed (another reach's bed is lower here by > 0.25 m)",
            "other",
        ];
        let mut low_bed: BTreeMap<usize, f64> = BTreeMap::new();
        for r in &reaches {
            for (k, &c) in r.vertices.iter().enumerate() {
                let e = low_bed.entry(c as usize).or_insert(f64::INFINITY);
                *e = e.min(bed_m(r.attrs.bed[k]));
            }
        }
        let mut count = vec![(0usize, 0usize); labels.len()];
        let mut per: Vec<(usize, u8, bool, f64, f64, f64)> = Vec::new();
        for (&c, &(ri, k)) in &vtx {
            let bed = bed_m(reaches[ri].attrs.bed[k]);
            if s0[c] - bed <= 0.25 {
                continue;
            }
            let class = if kv[c] - bed <= 0.25 {
                0
            } else if outlet[c] {
                2
            } else if protected[c] {
                3
            } else if near_divide[c] {
                4
            } else if left_at(c) <= 1e-9 {
                5
            } else if kv_wanted[c] > s0[c] - kv[c] + BUDGET_EPS_M {
                1
            } else if low_bed[&c] < bed - 0.25 {
                6
            } else {
                7
            };
            per.push((c, class as u8, reaches[ri].parent.is_some(), bed, s0[c], kv[c]));
            if reaches[ri].parent.is_some() {
                count[class].0 += 1;
            } else {
                count[class].1 += 1;
            }
        }
        (labels.iter().zip(count).map(|(&l, (a, b))| (l, a, b)).collect::<Vec<_>>(), per)
    };
    let (census, vertex_class) = census;
    // lead #169 item 3: the floor falls toward the line; floor cells farthest first (ties by cell), each at most its farther floor neighbours' height less the grade's drop
    let (mut cross_cells, mut cross_volume, mut cross_short, mut cross_short_m) = (0usize, 0.0f64, 0usize, 0.0f64);
    let mut cross_cut: Vec<f32> = Vec::new();
    if p.cross_grade > 0.0 && kv_on {
        let mut in_floor = vec![false; n];
        let mut order: Vec<u32> = Vec::new();
        // the floor, and with DEC crusader-19 the flat side near a vertex (its height above the vertex's measured on KV's output, before this pass)
        let side = |c: usize| {
            p.cross_side_m > 0.0 && dist[c] <= p.cross_side_m && {
                let v = src[c] as usize;
                let (ri, k) = vtx[&v];
                kv[c] - bed_m(reaches[ri].attrs.bed[k]).max(kv[v]) <= p.cross_side_grade * dist[c]
            }
        };
        for c in 0..n {
            if matches!(wants(c), Some(w) if w.1 || side(c)) {
                in_floor[c] = true;
                order.push(c as u32);
            }
        }
        cross_cut = vec![0.0f32; n];
        // the keys are unique (cell last), so an unstable sort gives the same order
        order.sort_unstable_by(|&a, &b| dist[b as usize].total_cmp(&dist[a as usize]).then(a.cmp(&b)));
        for &c in &order {
            let c = c as usize;
            let mut z = kv[c];
            for k in 0..8 {
                let Some(q) = grid.neighbour(c, k) else { continue };
                // only the same vertex's farther cells: across the line's own strip, never along the valley (a farther cell of the next vertex downstream stands lower by the
                // valley's own fall, and following it would cut the whole side down; found by the side rule's test)
                // and only a farther cell that stands lower (a channel or a cell already lowered for one): a flat or rising side is never cut, so the pass makes no strip
                // beside a line where no channel runs (DEC crusader-19: no visible strips)
                if !in_floor[q] || dist[q] <= dist[c] || src[q] != src[c] || kv[q] >= kv[c] {
                    continue;
                }
                // the drop counts the distance gained toward the line, not the step (a diagonal step gains less than its length; a cone at the grade is left as it is)
                z = z.min(kv[q] - p.cross_grade * (dist[q] - dist[c]));
            }
            // the floor never sinks to or below its vertex (numerics review of 8630f2a8): the higher of the vertex's bed and the vertex cell's own height (held above the bed where
            // its budget ran out), plus the flood grade per metre, so the inner floor still falls to the line; the vertex itself is never lowered by this pass
            let v = src[c] as usize;
            let (ri, k) = vtx[&v];
            let lo = bed_m(reaches[ri].attrs.bed[k]).max(kv[v]) + CROSS_MIN_GRADE * dist[c];
            let lo_budget = s0[c] - left_at(c);
            if z < lo_budget && lo_budget > lo {
                cross_short += 1;
                cross_short_m += lo_budget - z.max(lo);
            }
            let z = z.max(lo).max(lo_budget).max(sea_lo(c, kv[c]));
            if z < kv[c] {
                cross_cells += 1;
                cross_volume += (kv[c] - z) * p.cell_m * p.cell_m;
                cross_cut[c] = (kv[c] - z) as f32;
                kv_wanted[c] = kv_wanted[c].max(s0[c] - z);
                kv[c] = z;
            }
        }
    }
    // the cells within one cell of a graph vertex (KC's breach along the line, lead #166)
    let near_line: Vec<bool> = if p.breach_along_line && switch != Switch::KvOnly {
        let mut m = vec![false; n];
        for &c in vtx.keys() {
            m[c] = true;
            for k in 0..8 {
                if let Some(q) = grid.neighbour(c, k) {
                    m[q] = true;
                }
            }
        }
        m
    } else {
        Vec::new()
    };
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
            // lead #173: with kc_lake_outlets the kept lakes are outlets too (a cut draining into a lake is no pit)
            let lake_outlet: Vec<bool>;
            let outs: &[bool] = if p.kc_lake_outlets {
                lake_outlet = (0..n).map(|c| outlet[c] || protected[c]).collect();
                &lake_outlet
            } else {
                outlet
            };
            let hy = super::basins_rg::hierarchy(grid, z, outs);
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
                let inside = |c: usize| patch_of[c] == id as u32;
                let breach = |blocked: &dyn Fn(usize) -> bool| {
                    super::basins_rg::breach_path(grid, &kv, outlet, pit, floor, &remaining, &inside, blocked, BREACH_CAP)
                };
                // lead #178 item 1: a breach to a floor under the clamp crosses no land cell it would lower
                let sea_block = |c: usize| floor < p.sea_clamp_m && !outlet[c] && kv[c] > floor;
                let along = || breach(&|c| protected[c] || rim(c) > floor || sea_block(c) || !(near_line[c] || inside(c)));
                // the along path is taken when it beats the fill (or the fill does not fit); otherwise the free breach of reading 22 decides
                let found = if p.breach_along_line {
                    along()
                        .filter(|(vol, _)| *vol < fill_vol || !fill_ok)
                        .or_else(|| breach(&|c| protected[c] || rim(c) > floor || sea_block(c)))
                } else {
                    breach(&|c| protected[c] || rim(c) > floor || sea_block(c))
                };
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
    KvKc {
        kv,
        kc,
        kv_wanted,
        residuals,
        divide,
        divide_exempt,
        near_divide,
        patches,
        breaches,
        breach_volume,
        kc_wanted,
        nearest,
        cross_cells,
        cross_volume,
        cross_short,
        cross_short_m,
        cross_cut,
        census,
        vertex_class,
        connect_cells,
        sea_clamped,
    }
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
