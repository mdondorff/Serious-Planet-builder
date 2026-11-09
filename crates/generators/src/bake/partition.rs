//! The partitioned route (GEN-143, A3 sections 6.5 and 6.6): the region's routing under the round rules computed tile by tile, equal bit for bit to the resident route (`route_rules`, G-PART).
//!
//! - **Filled surface.** The canonical flood's level is the unique fixed point of `F(c) = max(z(c), min over neighbours q of F(q) + epsilon)` (an outlet is `z`): a shortest path under a monotone
//!   step. Each tile is flooded from its outlets and from the values its neighbours offer across the tile border; a tile whose inputs were seen before is not flooded again (the memo key holds every input of the flood, so a hit cannot change a bit short of a 128-bit hash collision). Tiles are swept until no input changes; the values only fall, so the sweeps end.
//! - **Receivers** read a cell and its neighbours on the filled surface: per tile, no border problem.
//! - **Area.** The resident accumulation adds into a cell, in order of the donors' (level descending, id), what each donor passes on. A cell's area is therefore its own plus its donors' shares added in that
//!   order, whatever order the cells are finished in: the cells are finished by a topological sweep (a cell when all its donors are), tile by tile, with the shares crossing tile borders.
//!
//! The arrays are resident (this change gates the equality, not the memory); the tiles bound what each step touches.
//!
//! A compact cube grid (GEN-149) is tiled in the layout of its six faces stacked (`Grid::layout`): a tile is a square of a face, holds the active cells inside it, and a cell outside the landmass is skipped.

#![allow(clippy::needless_range_loop)]

use super::route::{catchment_roots, receiver_of, topological_order, MfdClip, RoundRules, Routing, MFD_EXPONENT, NONE};
use super::{assert_finite, Grid};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

/// The opposite of each neighbour direction (`NEIGHBOURS`) on a plane grid.
const OPPOSITE: [usize; 8] = [1, 0, 3, 2, 7, 6, 5, 4];

/// What the partitioned route did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartStats {
    pub tiles: usize,
    pub sweeps: usize,
    /// Tile floods run (a memo miss).
    pub floods: usize,
    /// Tile evaluations answered by the memo.
    pub memo_hits: usize,
    /// Against the previous call of the same cache: tiles whose filled surface, receivers and area differ.
    pub tiles_filled_changed: usize,
    pub tiles_receiver_changed: usize,
    pub tiles_area_changed: usize,
}

/// The partitioned route's result: the routing and the filled surface it came from.
pub struct PartRoute {
    pub routing: Routing,
    pub filled: Vec<f64>,
    pub stats: PartStats,
}

impl PartCache {
    /// A cache that keeps nothing: the memo of floods and the previous result are not stored (a solve that is not edited again needs neither; GEN-149).
    pub fn without_memory() -> PartCache {
        PartCache { forget: true, ..PartCache::default() }
    }

    /// Compares the next results with the last of slot `k`: a series of routings of one surface in rounds uses a slot a round, so that "the tiles changed" reads against the same round of the previous solve.
    pub fn use_slot(&mut self, k: usize) {
        self.slot = k;
    }
}

/// What a series of calls keeps: the memo of tile floods by their inputs and the previous result (for the change counts).
#[derive(Default)]
pub struct PartCache {
    /// Set by [`PartCache::without_memory`]: nothing is kept between calls.
    forget: bool,
    floods: HashMap<(u64, u64), Arc<Vec<f64>>>,
    /// The last result of each slot (slot 0 unless [`PartCache::use_slot`] says otherwise).
    previous: Vec<Option<Previous>>,
    slot: usize,
}

/// The last result of a cache: the tile size, the filled surface, the receivers and the area.
struct Previous {
    tile: usize,
    size: (usize, usize),
    cube: bool,
    filled: Vec<f64>,
    receiver: Vec<u32>,
    area: Vec<f64>,
}

#[derive(Clone, Copy)]
struct Rect {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
}

/// A 128-bit hash of a word stream (two independent 64-bit mixes): the memo key.
#[derive(Clone, Copy)]
pub(super) struct Hash(u64, u64);

impl Hash {
    pub(super) fn new() -> Hash {
        Hash(0xcbf2_9ce4_8422_2325, 0x9e37_79b9_7f4a_7c15)
    }
    pub(super) fn word(&mut self, v: u64) {
        self.0 = (self.0 ^ v).wrapping_mul(0x100_0000_01b3);
        self.1 = (self.1.rotate_left(23) ^ v.wrapping_mul(0xff51_afd7_ed55_8ccd)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    }
    pub(super) fn key(self) -> (u64, u64) {
        (self.0, self.1)
    }
}

fn rects(grid: &Grid, tile: usize) -> (usize, usize, Vec<Rect>) {
    let (lw, lh) = grid.layout();
    let (tx, ty) = (lw.div_ceil(tile), lh.div_ceil(tile));
    let mut v = Vec::with_capacity(tx * ty);
    for j in 0..ty {
        for i in 0..tx {
            let (x0, y0) = (i * tile, j * tile);
            v.push(Rect { x0, y0, w: tile.min(lw - x0), h: tile.min(lh - y0) });
        }
    }
    (tx, ty, v)
}

fn in_rect(grid: &Grid, r: Rect, q: usize) -> bool {
    let (x, y) = grid.xy(q);
    x >= r.x0 && x < r.x0 + r.w && y >= r.y0 && y < r.y0 + r.h
}

/// One tile's flood: from its outlets and from the values offered across its border (`ext`: cell, the lowest value of a neighbour outside the tile). The filled value of the tile's cells (`f64::INFINITY`
/// for a cell nothing reaches yet), row-major in the tile.
fn flood_tile(grid: &Grid, r: Rect, z: &[f64], outlet: &[bool], ext: &[(usize, f64)], epsilon: f64) -> Vec<f64> {
    let local = |q: usize| {
        let (x, y) = grid.xy(q);
        (y - r.y0) * r.w + (x - r.x0)
    };
    // The cell of a local slot: `None` for a slot off a compact grid's landmass.
    let global = |l: usize| grid.at_xy(r.x0 + l % r.w, r.y0 + l / r.w);
    let key = |v: f64| {
        let b = v.to_bits();
        if b & (1 << 63) != 0 {
            !b
        } else {
            b | (1 << 63)
        }
    };
    let mut dist = vec![f64::INFINITY; r.w * r.h];
    let mut done = vec![false; r.w * r.h];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    for l in 0..dist.len() {
        let Some(p) = global(l) else { continue };
        if outlet[p] && grid.is_active(p) {
            dist[l] = z[p];
            heap.push(Reverse((key(z[p]), l as u32)));
        }
    }
    for &(p, e) in ext {
        let l = local(p);
        if outlet[p] {
            continue;
        }
        let cand = z[p].max(e + epsilon);
        if cand < dist[l] {
            dist[l] = cand;
            heap.push(Reverse((key(cand), l as u32)));
        }
    }
    while let Some(Reverse((_, l))) = heap.pop() {
        let l = l as usize;
        if done[l] {
            continue;
        }
        done[l] = true;
        let p = global(l).expect("a reached slot is a cell");
        assert!(dist[l] + epsilon > dist[l], "the epsilon {epsilon} is below the spacing of the heights near {}", dist[l]);
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if !in_rect(grid, r, q) {
                continue;
            }
            let lq = local(q);
            if done[lq] || outlet[q] {
                continue;
            }
            let cand = z[q].max(dist[l] + epsilon);
            if cand < dist[lq] {
                dist[lq] = cand;
                heap.push(Reverse((key(cand), lq as u32)));
            }
        }
    }
    dist
}

/// The lowest value a neighbour outside the tile offers each border cell of the tile (cells with no finite offer are left out), in cell order.
fn offers(grid: &Grid, r: Rect, filled: &[f64]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    for y in r.y0..r.y0 + r.h {
        for x in r.x0..r.x0 + r.w {
            if x != r.x0 && x != r.x0 + r.w - 1 && y != r.y0 && y != r.y0 + r.h - 1 {
                continue;
            }
            let Some(p) = grid.at_xy(x, y) else { continue };
            let mut best = f64::INFINITY;
            for d in 0..8 {
                if let Some(q) = grid.neighbour(p, d) {
                    if !in_rect(grid, r, q) && filled[q] < best {
                        best = filled[q];
                    }
                }
            }
            if best.is_finite() {
                out.push((p, best));
            }
        }
    }
    out
}

/// Routes `z` tile by tile (`tile` cells a side) under `rules` (the canonical flood is required; the clip may be off or local) and returns the routing of `route_rules` bit for bit.
/// `cache` keeps the memo of tile floods and the previous result across calls (an edited surface reuses the floods of the tiles it did not touch).
#[allow(clippy::too_many_arguments)]
pub fn partitioned_route(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    tile: usize,
    rules: RoundRules,
    cache: &mut PartCache,
) -> PartRoute {
    partitioned_route_weighted(grid, z, outlet, epsilon, seed, tile, rules, None, cache)
}

/// [`partitioned_route`] with the runoff weight of each cell (GEN-129, GEN-146): every cell starts with the area `w dA`, as in `route_rules` with weights. The weights enter the area only, so
/// the tile floods and their memo do not read them.
#[allow(clippy::too_many_arguments)]
pub fn partitioned_route_weighted(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    epsilon: f64,
    seed: u64,
    tile: usize,
    rules: RoundRules,
    weights: Option<&[f32]>,
    cache: &mut PartCache,
) -> PartRoute {
    let n = grid.len();
    if let Some(w) = weights {
        assert_eq!(w.len(), n, "one runoff weight per cell");
        assert!(w.iter().all(|v| v.is_finite() && *v > 0.0), "the runoff weights are finite and positive");
    }
    assert!(
        rules.canonical_flood && rules.clip_mfd != MfdClip::ToReceiver,
        "the partitioned route runs the round rules (canonical flood; clip off or local)"
    );
    assert!(tile >= 3, "a tile is at least 3 cells a side");
    assert!(epsilon > 0.0 && epsilon.is_finite(), "the epsilon is finite and positive");
    assert_eq!(z.len(), n, "one height per cell");
    assert_eq!(outlet.len(), n, "one outlet flag per cell");
    assert_finite("the heights", z);
    assert!((0..n).any(|p| outlet[p] && grid.is_active(p)), "a flood needs an outlet on the grid");
    assert!(!grid.is_cube() || grid.layout().0.is_multiple_of(tile), "the tiles of a cube grid divide a face edge");
    let (_, _, tiles) = rects(grid, tile);
    let mut stats = PartStats { tiles: tiles.len(), ..PartStats::default() };

    // ---- the filled surface
    let mut z_hash = Vec::with_capacity(tiles.len());
    for r in &tiles {
        let mut h = Hash::new();
        for y in r.y0..r.y0 + r.h {
            for x in r.x0..r.x0 + r.w {
                let Some(p) = grid.at_xy(x, y) else {
                    h.word(u64::MAX);
                    continue;
                };
                h.word(z[p].to_bits());
                h.word(u64::from(outlet[p]) | (u64::from(grid.is_active(p)) << 1));
            }
        }
        z_hash.push(h);
    }
    let mut filled = vec![f64::INFINITY; n];
    for p in 0..n {
        if outlet[p] && grid.is_active(p) {
            filled[p] = z[p];
        }
    }
    let mut seen: Vec<Option<(u64, u64)>> = vec![None; tiles.len()];
    loop {
        stats.sweeps += 1;
        let mut changed = false;
        for (t, &r) in tiles.iter().enumerate() {
            let ext = offers(grid, r, &filled);
            let has_outlet = (r.y0..r.y0 + r.h)
                .any(|y| (r.x0..r.x0 + r.w).any(|x| grid.at_xy(x, y).is_some_and(|p| outlet[p] && grid.is_active(p))));
            if ext.is_empty() && !has_outlet {
                continue;
            }
            let mut h = z_hash[t];
            h.word(u64::from(grid.is_cube()) | (u64::from(grid.is_compact()) << 1));
            h.word(grid.cell_area(0).to_bits());
            h.word(epsilon.to_bits());
            for v in [grid.layout().0, grid.layout().1, r.x0, r.y0, r.w, r.h] {
                h.word(v as u64);
            }
            for &(p, e) in &ext {
                h.word(p as u64);
                h.word(e.to_bits());
            }
            let k = h.key();
            if seen[t] == Some(k) {
                continue;
            }
            seen[t] = Some(k);
            changed = true;
            let result = match cache.floods.get(&k) {
                Some(v) => {
                    stats.memo_hits += 1;
                    Arc::clone(v)
                }
                None => {
                    stats.floods += 1;
                    let v = Arc::new(flood_tile(grid, r, z, outlet, &ext, epsilon));
                    if !cache.forget {
                        cache.floods.insert(k, Arc::clone(&v));
                    }
                    v
                }
            };
            for y in 0..r.h {
                for x in 0..r.w {
                    if let Some(p) = grid.at_xy(r.x0 + x, r.y0 + y) {
                        filled[p] = result[y * r.w + x];
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    assert!((0..n).all(|p| !grid.is_active(p) || filled[p].is_finite()), "a cell is not reached by the flood: no outlet on its component");

    // The cells off the grid hold what the resident flood leaves there: their height if flagged as outlets, 0.0 otherwise.
    for p in 0..n {
        if !grid.is_active(p) {
            filled[p] = if outlet[p] { z[p] } else { 0.0 };
        }
    }

    // ---- receivers, per tile
    let mut receiver = vec![NONE; n];
    for r in &tiles {
        for y in r.y0..r.y0 + r.h {
            for x in r.x0..r.x0 + r.w {
                let Some(p) = grid.at_xy(x, y) else { continue };
                if !outlet[p] && grid.is_active(p) {
                    receiver[p] = receiver_of(grid, &filled, p, seed);
                }
            }
        }
    }
    let order = topological_order(&receiver);
    let clip = rules.clip_mfd == MfdClip::Local;
    let root = if clip { catchment_roots(&receiver, &order) } else { Vec::new() };

    // ---- area: the weights per cell, then the topological sweep over the tiles
    let passes = |p: usize, d: usize, q: usize| -> Option<f64> {
        let slope = (filled[p] - filled[q]) / grid.distance_at(p, d);
        (slope > 0.0 && !(clip && root[q] != root[p])).then(|| libm::pow(slope, MFD_EXPONENT))
    };
    // The direction in which `p` reaches `q`, given that `q` reaches `p` in `d`: the opposite one on a plane; across a cube edge the faces are turned, so it is looked up.
    let cube = grid.is_cube();
    let slot = |p: usize, q: usize, d: usize| -> usize {
        if cube && grid.xy(p).1 / grid.layout().0 != grid.xy(q).1 / grid.layout().0 {
            (0..8).find(|&e| grid.neighbour(p, e) == Some(q)).expect("a neighbour reaches back")
        } else {
            OPPOSITE[d]
        }
    };
    let mut sum = vec![0.0f64; n];
    for p in 0..n {
        if outlet[p] {
            continue;
        }
        let mut s = 0.0;
        for d in 0..8 {
            if let Some(q) = grid.neighbour(p, d) {
                if let Some(w) = passes(p, d, q) {
                    s += w;
                }
            }
        }
        sum[p] = s;
    }
    let mut pending = vec![0u8; n];
    for q in 0..n {
        for d in 0..8 {
            if let Some(p) = grid.neighbour(q, d) {
                if !outlet[p] && sum[p] > 0.0 && passes(p, slot(p, q, d), q).is_some() {
                    pending[q] += 1;
                }
            }
        }
    }
    let tile_of = |p: usize| {
        let (x, y) = grid.xy(p);
        (y / tile) * grid.layout().0.div_ceil(tile) + x / tile
    };
    let mut ready: Vec<Vec<u32>> = vec![Vec::new(); tiles.len()];
    for p in 0..n {
        if pending[p] == 0 {
            ready[tile_of(p)].push(p as u32);
        }
    }
    let mut area = vec![0.0f64; n];
    let mut finished = 0usize;
    while finished < n {
        let before = finished;
        for t in 0..tiles.len() {
            while let Some(q) = ready[t].pop() {
                let q = q as usize;
                // The donors in the order of the resident accumulation: level descending, then cell id.
                let mut donors: Vec<(usize, f64)> = Vec::new();
                for d in 0..8 {
                    if let Some(p) = grid.neighbour(q, d) {
                        if !outlet[p] && sum[p] > 0.0 {
                            if let Some(w) = passes(p, slot(p, q, d), q) {
                                donors.push((p, w));
                            }
                        }
                    }
                }
                donors.sort_unstable_by(|a, b| filled[b.0].total_cmp(&filled[a.0]).then(a.0.cmp(&b.0)));
                let mut a = match weights {
                    Some(w) => grid.cell_area(q) * f64::from(w[q]),
                    None => grid.cell_area(q),
                };
                for &(p, w) in &donors {
                    a += area[p] * w / sum[p];
                }
                area[q] = a;
                finished += 1;
                if !outlet[q] && sum[q] > 0.0 {
                    for d in 0..8 {
                        if let Some(c) = grid.neighbour(q, d) {
                            if passes(q, d, c).is_some() {
                                pending[c] -= 1;
                                if pending[c] == 0 {
                                    ready[tile_of(c)].push(c as u32);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(finished > before, "the accumulation stalls: a cell waits on itself");
    }

    // ---- what changed against the previous call
    if let Some(Some(Previous { tile: tl, size, cube, filled: pf, receiver: pr, area: pa })) = cache.previous.get(cache.slot) {
        if *tl == tile && *size == grid.layout() && *cube == grid.is_cube() {
            for r in &tiles {
                let (mut f, mut rc, mut ar) = (false, false, false);
                for y in r.y0..r.y0 + r.h {
                    for x in r.x0..r.x0 + r.w {
                        let Some(p) = grid.at_xy(x, y) else { continue };
                        f |= filled[p].to_bits() != pf[p].to_bits();
                        rc |= receiver[p] != pr[p];
                        ar |= area[p].to_bits() != pa[p].to_bits();
                    }
                }
                stats.tiles_filled_changed += usize::from(f);
                stats.tiles_receiver_changed += usize::from(rc);
                stats.tiles_area_changed += usize::from(ar);
            }
        }
    }
    if cache.forget {
        return PartRoute { routing: Routing { receiver, order, area }, filled, stats };
    }
    let last = Some(Previous {
        tile,
        size: grid.layout(),
        cube: grid.is_cube(),
        filled: filled.clone(),
        receiver: receiver.clone(),
        area: area.clone(),
    });
    if cache.previous.len() <= cache.slot {
        cache.previous.resize_with(cache.slot + 1, || None);
    }
    cache.previous[cache.slot] = last;
    PartRoute { routing: Routing { receiver, order, area }, filled, stats }
}
