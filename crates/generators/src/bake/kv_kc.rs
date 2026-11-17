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
    /// The nearest L reach of each cell KV reached (`u64::MAX`: none).
    pub nearest: Vec<u64>,
}

/// The bed level (m) of a code (GEN-170: 0.5 m codes).
fn bed_m(code: i32) -> f64 {
    f64::from(code) * 0.5
}

/// The chamfer distance (m) from the given sources, with each cell's nearest source; ties by the source's cell id.
fn chamfer(grid: &Grid, sources: &[(usize, usize)], cell_m: f64, limit_m: f64) -> (Vec<f64>, Vec<usize>) {
    let n = grid.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut src = vec![usize::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u64, usize, usize)>> = BinaryHeap::new();
    for &(c, tag) in sources {
        if dist[c] > 0.0 || tag < src[c] {
            dist[c] = 0.0;
            src[c] = tag;
            heap.push(Reverse((0, tag, c)));
        }
    }
    // distances as integer millimetres in the heap key: fixed order
    while let Some(Reverse((dmm, tag, c))) = heap.pop() {
        let d = dmm as f64 / 1000.0;
        if d > dist[c] || (d == dist[c] && tag != src[c]) {
            continue;
        }
        for k in 0..8 {
            let Some(q) = grid.neighbour(c, k) else { continue };
            let step = if k < 4 { cell_m } else { cell_m * core::f64::consts::SQRT_2 };
            let nd = libm::round((d + step) * 1000.0) / 1000.0;
            if nd > limit_m {
                continue;
            }
            if nd < dist[q] || (nd == dist[q] && tag < src[q]) {
                dist[q] = nd;
                src[q] = tag;
                heap.push(Reverse(((nd * 1000.0) as u64, tag, q)));
            }
        }
    }
    (dist, src)
}

/// Runs KV and KC on `s0` (the stage input; KS off in the prototype) around the L reaches of `g`, within `budget` per cell. `protected` marks the protected basins' cells; `outlet` the sea.
pub fn run(grid: &Grid, s0: &[f64], outlet: &[bool], g: &Graph, protected: &[bool], budget: &[f64], p: &KvParams, switch: Switch) -> KvKc {
    let n = s0.len();
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
    let chain: Vec<u64> = (0..reaches.len()).map(chain_of).collect();
    let routing = route_rules(grid, s0, outlet, 1e-3, p.seed, 0.0, None, RoundRules::ROUND);
    let reached = 1.5 * p.cell_m;
    let mut catch = vec![u64::MAX - 1; n];
    for c in 0..n {
        let mut x = c;
        let mut steps = 0;
        loop {
            if dist[x] <= reached && src[x] != usize::MAX {
                catch[c] = chain[vtx[&src[x]].0];
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
    let divide: Vec<bool> =
        (0..n).map(|c| !outlet[c] && (0..8).filter_map(|k| grid.neighbour(c, k)).any(|q| !outlet[q] && catch[q] != catch[c])).collect();
    let div_src: Vec<(usize, usize)> = (0..n).filter(|&c| divide[c]).map(|c| (c, c)).collect();
    let (ddiv, _) = chamfer(grid, &div_src, p.cell_m, p.d_buf_m + p.cell_m);
    let near_divide: Vec<bool> = (0..n).map(|c| ddiv[c] < p.d_buf_m).collect();
    // KV
    let mut kv = s0.to_vec();
    let mut kv_wanted = vec![0.0; n];
    let mut residuals = Vec::new();
    let mut nearest = vec![u64::MAX; n];
    let kv_on = switch != Switch::KcOnly;
    for c in 0..n {
        if src[c] == usize::MAX {
            continue;
        }
        let (ri, k) = vtx[&src[c]];
        let r = reaches[ri];
        nearest[c] = r.id;
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
        let left = budget[c].max(0.0);
        let low = want.min(left);
        kv[c] = s0[c] - low;
        if want > low + BUDGET_EPS_M {
            residuals.push(Residual { cell: c, stage: "kv", needed_m: want, available_m: left, feature: r.id });
        }
    }
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
                    let feature = nearest[c];
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
