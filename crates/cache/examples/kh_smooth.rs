#![allow(dead_code, unused_imports, unused_variables)]
//! spikes #138 (spike/kh-stage, never merged): kh_smooth, SPIKE-136's handover.rs with h2 (the triangulation), h3 (the skeleton) and h4 (Clough-Tocher on h2's split)
//! from planet_generators::bake::kh, measured on SPIKE-135's ladder; `--crop-at cx,cy,crop` writes #137's render crops.
//! SPIKE-136 (lead #136; spike/handover, never merged): two drainage-preserving hand-overs of z_s at L (h1: SPIKE-116's 4-neighbour breach, bilinear; h2: a flow-diagonal
//! triangulation without lowering), measured with SPIKE-135's ladder; `main` below the SPIKE-136 line. SPIKE-135's ladder helpers (`rung`, `dilate`) above it.
//! SPIKE-135 (lead #135; spike/repair-debt, never merged): the repair-debt ladder of S0 by origin. The helpers above it are kg_trunk.rs's
//! (spike/deep-basins f6395a8f), kept whole.
//! SPIKE-129 (G1c, lead #132; spike branch only, never merged): KG-trunk, the L images of the H1 trunk graph (G0 contracts, GEN-189 types), without an L erosion round.
//! H1 base: K6' on the sills of S0 (SPIKE-127; DEC spikes-3 default) or today's K6 (`--base today`). KS0 = C1(z_s) + min(r, C1(HAND)); L anchors by contract §2.1 on S0;
//! centrelines by BUILD-057's corridor route on KS0; beds by §3 with the §6 budget remainder (rise_over_budget beyond it); rasterisation by §5.
//! The helpers below are SPIKE-127's and SPIKE-131's (g1a_sills.rs, k5p_sills.rs).
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::Commit;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, solve_region, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{commit_differences, lay_store_ground, LWindow, WindowIdentity};
use planet_cache::river_suite::corridor_bar_m;
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::channel_graph::{q_value, ChannelGraph};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, id_hash, Hydro, HydroParams};
use planet_generators::bake::k7::RegionField;
use planet_generators::bake::river_graph::{
    diff, node_id, rasterise, reach_id, validate, validate_raster, Attrs, Checks, Domain, Exception as GException, Graph, Level, Marker,
    Node, NodeKind, Reach as GReach, Reason, Violation,
};
use planet_generators::bake::route::{catchment_roots, jitter, route_rules, RoundRules, Routing, MFD_EXPONENT, NONE};
use planet_generators::bake::{Grid, NEIGHBOURS};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
const L: usize = LOCAL_PER_REGIONAL;
const EDGE_Q: f64 = 1.0;
const K_C: f64 = 2.0;

struct Lattice {
    face: Face,
    i0: usize,
    j0: usize,
    w: usize,
    h: usize,
    z: Vec<f32>,
}

impl LatticeSource for Lattice {
    fn res(&self) -> u32 {
        LATTICE as u32
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let (i, j) = (i as usize, j as usize);
        if face != self.face || i < self.i0 || j < self.j0 || i >= self.i0 + self.w || j >= self.j0 + self.h {
            return Err(ReadError::Source(format!("node ({}, {i}, {j}) is outside the sampled rectangle", face.0)));
        }
        Ok(GlobalSample {
            continent: 0.0,
            elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)],
            temperature_c: 0.0,
            precipitation_mm: 0.0,
        })
    }
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).map(|k| args.get(k + 1).map(String::as_str).unwrap_or(""))
}

fn par_fill(out: &mut [f64], w: usize, threads: usize, f: &(dyn Fn(usize) -> f64 + Sync)) {
    let rows = (out.len() / w).div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    *v = f(k * rows * w + c);
                }
            });
        }
    });
}

/// A uniform number in [-1, 1) from a seed and a key (SplitMix64 steps).
fn noise(seed: u64, key: u64) -> f64 {
    let mut x = seed ^ key.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

// ---------------------------------------------------------------- nodes and sills

/// The forward edges of a cell: E, S, SE, SW (block offsets).
const FWD: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (-1, 1)];

struct Sills {
    /// Node height (lowest L cell of S0) and its L index, per H1 cell.
    h: Vec<f64>,
    low: Vec<u32>,
    /// Per H1 cell the sills of its forward edges (NaN off the grid).
    fwd: Vec<[f64; 4]>,
}

impl Sills {
    /// The sill between `p` and its neighbour in NEIGHBOURS direction `d` (`q` that neighbour).
    fn at(&self, p: usize, d: usize, q: usize) -> f64 {
        let (dx, dy) = NEIGHBOURS[d];
        if let Some(k) = FWD.iter().position(|&f| f == (dx, dy)) {
            self.fwd[p][k]
        } else {
            let k = FWD.iter().position(|&f| f == (-dx, -dy)).expect("every direction is forward or backward");
            self.fwd[q][k]
        }
    }
}

/// One block's L cells sorted by (height, L index): (height, L index).
fn sorted_block(s0: &(dyn Fn(usize) -> f64 + Sync), lw: usize, bx: usize, by: usize) -> Vec<(f64, u32)> {
    let mut v: Vec<(f64, u32)> = Vec::with_capacity(L * L);
    for y in by * L..(by + 1) * L {
        for x in bx * L..(bx + 1) * L {
            let c = y * lw + x;
            v.push((s0(c), c as u32));
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    v
}

fn find(parent: &mut [u8], mut a: usize) -> usize {
    while parent[a] as usize != a {
        parent[a] = parent[parent[a] as usize];
        a = parent[a] as usize;
    }
    a
}

/// The minimax height of 8-connected L paths between the lowest cells of block A (at (ax, ay)) and block B (A offset by `off` blocks), inside the two blocks.
fn sill_of(a: &[(f64, u32)], b: &[(f64, u32)], lw: usize, ax: usize, ay: usize, off: (i64, i64)) -> f64 {
    let n = L * L;
    // slot of an L cell given its frame coordinates relative to A's origin
    let (ox, oy) = (off.0 * L as i64, off.1 * L as i64);
    let slot = |fx: i64, fy: i64| -> Option<usize> {
        if fx >= 0 && fy >= 0 && fx < L as i64 && fy < L as i64 {
            Some((fy * L as i64 + fx) as usize)
        } else if fx >= ox && fy >= oy && fx < ox + L as i64 && fy < oy + L as i64 {
            Some(n + ((fy - oy) * L as i64 + (fx - ox)) as usize)
        } else {
            None
        }
    };
    let (gx0, gy0) = ((ax * L) as i64, (ay * L) as i64);
    let frame = |c: u32| ((c as usize % lw) as i64 - gx0, (c as usize / lw) as i64 - gy0);
    let mut parent: Vec<u8> = (0..2 * n).map(|k| k as u8).collect();
    let mut added = vec![false; 2 * n];
    let (fa, fb) = (frame(a[0].1), frame(b[0].1));
    let (sa, sb) = (slot(fa.0, fa.1).expect("A's lowest is in A"), slot(fb.0, fb.1).expect("B's lowest is in B"));
    let (mut i, mut j) = (0usize, 0usize);
    loop {
        // the next cell of the merged order (height, L index)
        let take_a = j >= b.len() || (i < a.len() && (a[i].0.total_cmp(&b[j].0).then(a[i].1.cmp(&b[j].1))).is_lt());
        let (v, c) = if take_a {
            i += 1;
            a[i - 1]
        } else {
            j += 1;
            b[j - 1]
        };
        let (fx, fy) = frame(c);
        let s = slot(fx, fy).expect("a cell of the two blocks");
        added[s] = true;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(t) = slot(fx + dx, fy + dy) {
                    if added[t] {
                        let (ra, rb) = (find(&mut parent, s), find(&mut parent, t));
                        if ra != rb {
                            parent[ra.max(rb)] = ra.min(rb) as u8;
                        }
                    }
                }
            }
        }
        if added[sa] && added[sb] && find(&mut parent, sa) == find(&mut parent, sb) {
            return v;
        }
        assert!(i < a.len() || j < b.len(), "the two blocks are connected");
    }
}

/// Node heights and forward sills over the H1 grid `rw` x `rh` (L window `rw L` x `rh L`), S0 read by `s0(L index)`; `sill_noise` adds a uniform +-amplitude
/// per edge (then lifts the sill to its nodes).
fn nodes_and_sills(s0: &(dyn Fn(usize) -> f64 + Sync), rw: usize, rh: usize, threads: usize, sill_noise: Option<(u64, f64)>) -> Sills {
    nodes_and_sills_rank(s0, rw, rh, threads, sill_noise, 0)
}

/// [`nodes_and_sills`] with the node height the `rank`-th lowest L cell of its block (0: the lowest; 32: the median of 64); the sills stay the minimax between
/// the blocks' lowest cells, lifted to the higher of the two node heights.
fn nodes_and_sills_rank(
    s0: &(dyn Fn(usize) -> f64 + Sync),
    rw: usize,
    rh: usize,
    threads: usize,
    sill_noise: Option<(u64, f64)>,
    rank: usize,
) -> Sills {
    let lw = rw * L;
    let n = rw * rh;
    let mut h = vec![0f64; n];
    let mut low = vec![0u32; n];
    let mut fwd = vec![[f64::NAN; 4]; n];
    let rows = rh.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (band, ((hc, lc), fc)) in h.chunks_mut(rows * rw).zip(low.chunks_mut(rows * rw)).zip(fwd.chunks_mut(rows * rw)).enumerate() {
            let j0 = band * rows;
            s.spawn(move || {
                let j1 = (j0 + rows).min(rh);
                let row = |j: usize| (0..rw).map(|i| sorted_block(s0, lw, i, j)).collect::<Vec<_>>();
                let mut cur = row(j0);
                for j in j0..j1 {
                    let next = if j + 1 < rh { row(j + 1) } else { Vec::new() };
                    for i in 0..rw {
                        let c = (j - j0) * rw + i;
                        hc[c] = cur[i][rank].0;
                        lc[c] = cur[i][rank].1;
                        for (k, &(ox, oy)) in FWD.iter().enumerate() {
                            let (ni, nj) = (i as i64 + ox, j as i64 + oy);
                            if ni < 0 || ni >= rw as i64 || nj >= rh as i64 {
                                continue;
                            }
                            let other = if oy == 0 { &cur[ni as usize] } else { &next[ni as usize] };
                            let mut v = sill_of(&cur[i], other, lw, i, j, (ox, oy)).max(cur[i][rank].0).max(other[rank].0);
                            if let Some((seed, amp)) = sill_noise {
                                let id = ((j * rw + i) as u64) * 4 + k as u64;
                                v = (v + amp * noise(seed, id)).max(cur[i][0].0).max(other[0].0);
                            }
                            fc[c][k] = v;
                        }
                    }
                    cur = next;
                }
            });
        }
    });
    Sills { h, low, fwd }
}

// ---------------------------------------------------------------- K6' routing

#[derive(PartialEq)]
struct Key(f64, u32);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    // min-heap by (level, id)
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// The filled levels: outlets at their node height, every other cell the minimax over paths to an outlet of `max(sill, W + eps)` (Dijkstra by (level, id)).
fn flood(grid: &Grid, h: &[f64], sill: &dyn Fn(usize, usize, usize) -> f64, outlet: &[bool], eps: f64) -> Vec<f64> {
    let n = grid.len();
    let mut w = vec![f64::INFINITY; n];
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    for p in 0..n {
        if outlet[p] {
            w[p] = h[p];
            heap.push(Key(h[p], p as u32));
        }
    }
    while let Some(Key(l, p)) = heap.pop() {
        let p = p as usize;
        if done[p] || l > w[p] {
            continue;
        }
        done[p] = true;
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            if done[q] || outlet[q] {
                continue;
            }
            let v = sill(p, d, q).max(w[p] + eps).max(h[q]);
            if v < w[q] {
                w[q] = v;
                heap.push(Key(v, q as u32));
            }
        }
    }
    assert!(w.iter().all(|v| v.is_finite()), "every cell reaches an outlet");
    w
}

fn topological_order(receiver: &[u32]) -> Vec<u32> {
    let n = receiver.len();
    let mut start = vec![0u32; n + 1];
    for &r in receiver {
        if r != NONE {
            start[r as usize + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut next = start.clone();
    let mut donors = vec![0u32; n];
    for (p, &r) in receiver.iter().enumerate() {
        if r != NONE {
            donors[next[r as usize] as usize] = p as u32;
            next[r as usize] += 1;
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut stack: Vec<u32> = (0..n as u32).rev().filter(|&p| receiver[p as usize] == NONE).collect();
    while let Some(p) = stack.pop() {
        order.push(p);
        let (a, b) = (start[p as usize] as usize, start[p as usize + 1] as usize);
        stack.extend(donors[a..b].iter().rev());
    }
    assert_eq!(order.len(), n, "the order reaches every cell");
    order
}

/// K6's routing on the filled levels with the sill condition: a neighbour is open when it is lower and the cell's level reaches the sill between them.
fn route_sills(
    grid: &Grid,
    w: &[f64],
    sill: &dyn Fn(usize, usize, usize) -> f64,
    outlet: &[bool],
    seed: u64,
    weights: &[f32],
    mfd_open: bool,
) -> Routing {
    let n = grid.len();
    let open = |p: usize, d: usize| -> Option<usize> {
        let q = grid.neighbour(p, d)?;
        (w[q] < w[p] && sill(p, d, q) <= w[p]).then_some(q)
    };
    let mut receiver = vec![NONE; n];
    for p in 0..n {
        if outlet[p] || !grid.is_active(p) {
            continue;
        }
        let mut best = (0.0f64, NONE);
        for d in 0..8 {
            let Some(q) = open(p, d) else { continue };
            let mut s = (w[p] - w[q]) / grid.distance_at(p, d);
            s *= 1.0 + 0.5 * jitter(seed, grid.global_id(p), d);
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        assert!(best.1 != NONE, "cell {p} has no open lower neighbour");
        receiver[p] = best.1;
    }
    let order = topological_order(&receiver);
    let root = catchment_roots(&receiver, &order);
    // the clipped (Local) MFD area over the open neighbours, highest first, ties by index
    let mut by_height: Vec<u32> = (0..n as u32).collect();
    by_height.sort_unstable_by(|&a, &b| w[b as usize].total_cmp(&w[a as usize]).then(a.cmp(&b)));
    let mut area: Vec<f64> = (0..n).map(|p| grid.cell_area(p) * f64::from(weights[p])).collect();
    for &p in &by_height {
        let p = p as usize;
        if outlet[p] {
            continue;
        }
        let mut wt = [0.0f64; 8];
        let mut sum = 0.0;
        for (d, x) in wt.iter_mut().enumerate() {
            let q = if mfd_open { open(p, d) } else { grid.neighbour(p, d).filter(|&q| w[q] < w[p]) };
            if let Some(q) = q {
                if root[q] != root[p] {
                    continue;
                }
                *x = libm::pow((w[p] - w[q]) / grid.distance_at(p, d), MFD_EXPONENT);
                sum += *x;
            }
        }
        if sum > 0.0 {
            let held = area[p];
            for (d, x) in wt.iter().enumerate() {
                if *x > 0.0 {
                    area[grid.neighbour(p, d).expect("weighted")] += held * x / sum;
                }
            }
        }
    }
    Routing { receiver, order, area }
}

/// A routed H1: routing, filled levels, hydrology and graph, with the trunk reaches (q >= 1 m3/s, 2 cells or more) as segment indices.
struct H1 {
    routing: Routing,
    hydro: Hydro,
    graph: ChannelGraph,
    trunk_seg: Vec<usize>,
    trunk_cells: Vec<Vec<u32>>,
    /// The D8 drainage area (true area, unweighted) of each cell along the receivers, m2.
    d8: Vec<f64>,
}

fn h1_from(grid: &Grid, z: &[f64], filled: &[f64], routing: Routing, outlet: &[bool], runoff: &[f32], evap: &[f32]) -> H1 {
    let hydro = hydrology(grid, z, filled, &routing, outlet, Some(runoff), Some(evap), &HydroParams::default());
    let graph = ChannelGraph::from_hydro(grid, &routing, &hydro, z);
    let (mut trunk_seg, mut trunk_cells) = (Vec::new(), Vec::new());
    graph.for_each(|s| {
        let cells = s.cells(grid);
        let q_max = s.q.iter().map(|&q| q_value(q)).fold(0.0, f64::max);
        if cells.len() >= 2 && q_max >= EDGE_Q {
            trunk_seg.push(s.index);
            trunk_cells.push(cells);
        }
    });
    let mut d8: Vec<f64> = (0..grid.len()).map(|p| grid.cell_area(p)).collect();
    for &c in routing.order.iter().rev() {
        let r = routing.receiver[c as usize];
        if r != NONE {
            d8[r as usize] += d8[c as usize];
        }
    }
    H1 { routing, hydro, graph, trunk_seg, trunk_cells, d8 }
}

// ---------------------------------------------------------------- the corridor measure (land-only snap)

#[derive(Clone, Debug)]
struct Reach {
    failed: bool,
    /// The minimax rise (failed) or the largest rise of the route (passed), m.
    rise: f64,
    /// Where (window L cell) the rise is largest.
    at: (usize, usize),
    ended_at_sea: bool,
}

struct Measure<'a> {
    w: usize,
    h: usize,
    water: &'a [f64],
    inside: &'a [bool],
    region: &'a [bool],
    sea: f64,
    radius: f64,
    bar: f64,
    /// Reading 12: the connected sea; when given, a cell is land unless it is in it.
    ocean: Option<&'a [bool]>,
}

impl Measure<'_> {
    fn land(&self, c: usize) -> bool {
        match self.ocean {
            Some(o) => !o[c],
            None => self.water[c] > self.sea,
        }
    }

    fn lowest_in_disc(&self, p: (f64, f64), keep: &dyn Fn(usize) -> bool) -> Option<usize> {
        let (w, h) = (self.w as i64, self.h as i64);
        let r = self.radius / K_C; // the node radius: one H1 cell
        let (x0, x1) = (((p.0 - r).floor() as i64).max(0), ((p.0 + r).ceil() as i64).min(w - 1));
        let (y0, y1) = (((p.1 - r).floor() as i64).max(0), ((p.1 + r).ceil() as i64).min(h - 1));
        let mut best: Option<usize> = None;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let c = (y * w + x) as usize;
                if libm::hypot(x as f64 - p.0, y as f64 - p.1) > r || !keep(c) {
                    continue;
                }
                if best.is_none_or(|b| self.water[c] < self.water[b] || (self.water[c] == self.water[b] && c < b)) {
                    best = Some(c);
                }
            }
        }
        best
    }

    /// The land-first snap: a cell `keep` and land admits, else any `keep` admits.
    fn snap(&self, p: (f64, f64), land_only: bool, keep: &dyn Fn(usize) -> bool) -> Option<usize> {
        if land_only {
            if let Some(c) = self.lowest_in_disc(p, &|c| keep(c) && self.land(c)) {
                return Some(c);
            }
        }
        self.lowest_in_disc(p, keep)
    }

    fn corridor(&self, line: &[(f64, f64)]) -> Option<(HashMap<usize, usize>, Vec<usize>)> {
        let (w, h, radius) = (self.w, self.h, self.radius);
        let inside_grid = |p: (f64, f64)| p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= (w - 1) as f64 && p.1 <= (h - 1) as f64;
        if line.len() < 2 || !inside_grid(line[0]) || !inside_grid(line[line.len() - 1]) {
            return None;
        }
        let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for &(x, y) in line {
            (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        }
        let clamp = |v: f64, n: usize| (v.max(0.0) as usize).min(n - 1);
        let (bx0, bx1, by0, by1) = (
            clamp((x0 - radius).floor(), w),
            clamp((x1 + radius).ceil(), w),
            clamp((y0 - radius).floor(), h),
            clamp((y1 + radius).ceil(), h),
        );
        let seg_dist = |px: f64, py: f64| {
            line.windows(2)
                .map(|s| {
                    let ((ax, ay), (bx, by)) = (s[0], s[1]);
                    let (dx, dy) = (bx - ax, by - ay);
                    let l2 = dx * dx + dy * dy;
                    let t = if l2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                    libm::hypot(px - ax - t * dx, py - ay - t * dy)
                })
                .fold(f64::INFINITY, f64::min)
        };
        let (mut index, mut cells) = (HashMap::new(), Vec::new());
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                if self.inside[y * w + x] && seg_dist(x as f64, y as f64) <= radius {
                    index.insert(y * w + x, cells.len());
                    cells.push(y * w + x);
                }
            }
        }
        Some((index, cells))
    }

    /// The maximin route (river_suite's corridor_route) with the land rule: steps only from land cells; a sea cell reached ends the route (returned as its end).
    #[allow(clippy::too_many_arguments)]
    fn route(
        &self,
        index: &HashMap<usize, usize>,
        cells: &[usize],
        from: usize,
        to: usize,
        d: f64,
        start_label: f64,
    ) -> Option<(Vec<usize>, f64)> {
        let (w, z) = (self.w, self.water);
        if z[from] - start_label > d {
            return None;
        }
        let mut label = vec![f64::NEG_INFINITY; cells.len()];
        let mut parent = vec![usize::MAX; cells.len()];
        let mut done = vec![false; cells.len()];
        let mut heap = BinaryHeap::new();
        let l0 = start_label.min(z[from]);
        label[index[&from]] = l0;
        heap.push(MaxKey(l0, from));
        while let Some(MaxKey(l, c)) = heap.pop() {
            let ci = index[&c];
            if done[ci] || l < label[ci] {
                continue;
            }
            done[ci] = true;
            if c == to || (c != from && !self.land(c)) {
                let mut route = vec![c];
                let mut k = ci;
                while parent[k] != usize::MAX {
                    k = parent[k];
                    route.push(cells[k]);
                }
                route.reverse();
                return Some((route, l));
            }
            if c != from && !self.land(c) {
                continue;
            }
            let (cx, cy) = ((c % w) as i64, (c / w) as i64);
            for (dx, dy) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= self.h as i64 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                let Some(&ni) = index.get(&n) else { continue };
                if done[ni] || z[n] - l > d {
                    continue;
                }
                let nl = l.min(z[n]);
                if nl > label[ni] {
                    label[ni] = nl;
                    parent[ni] = ci;
                    heap.push(MaxKey(nl, n));
                }
            }
        }
        None
    }

    /// Leg after leg; a leg that ends at the sea ends the route.
    fn waypoints(&self, index: &HashMap<usize, usize>, cells: &[usize], points: &[usize], d: f64) -> Option<(Vec<usize>, bool)> {
        let (mut route, mut label) = (vec![points[0]], f64::INFINITY);
        for leg in points.windows(2) {
            let (part, l) = self.route(index, cells, leg[0], leg[1], d, label)?;
            let end = *part.last().expect("a route has cells");
            route.extend_from_slice(&part[1..]);
            label = l;
            if end != leg[1] {
                return Some((route, true));
            }
        }
        Some((route, false))
    }

    /// One reach (window L polyline, upstream first); `mouth` lets its last node snap to the sea. `None`: skipped as trunk_corridor skips it.
    fn reach(&self, line: &[(f64, f64)], mouth: bool) -> Option<Reach> {
        let (w, h) = (self.w, self.h);
        let cell_of = |p: (f64, f64)| (p.1.round() as usize).min(h - 1) * w + (p.0.round() as usize).min(w - 1);
        let (index, cells) = self.corridor(line)?;
        if !line.iter().any(|&p| self.inside[cell_of(p)]) {
            return None;
        }
        let region = |c: usize| self.region[c];
        let edge = |c: usize| {
            let (x, y) = (c % w, c / w);
            region(c)
                && [(x > 0).then(|| c - 1), (x + 1 < w).then(|| c + 1), (y > 0).then(|| c - w), (y + 1 < h).then(|| c + w)]
                    .into_iter()
                    .flatten()
                    .any(|n| !region(n))
        };
        let mut points = vec![self.snap(line[0], true, &|_| true)?];
        for pair in line.windows(2) {
            if region(cell_of(pair[0])) != region(cell_of(pair[1])) {
                let mid = (0.5 * (pair[0].0 + pair[1].0), 0.5 * (pair[0].1 + pair[1].1));
                if let Some(x) = self.snap(mid, true, &edge) {
                    points.push(x);
                }
            }
        }
        points.push(self.snap(line[line.len() - 1], !mouth, &|_| true)?);
        points.dedup();
        if points.len() < 2 || points.iter().any(|p| !index.contains_key(p)) {
            return None;
        }
        let top = cells.iter().map(|&c| self.water[c]).fold(f64::NEG_INFINITY, f64::max)
            - cells.iter().map(|&c| self.water[c]).fold(f64::INFINITY, f64::min);
        let mut failed = false;
        let (route, sea) = match self.waypoints(&index, &cells, &points, self.bar) {
            Some(r) => r,
            None => {
                failed = true;
                let (mut lo, mut hi) = (self.bar, top.max(self.bar));
                let mut best = self.waypoints(&index, &cells, &points, hi)?;
                while hi - lo > 0.1 {
                    let mid = 0.5 * (lo + hi);
                    match self.waypoints(&index, &cells, &points, mid) {
                        Some(r) => (hi, best) = (mid, r),
                        None => lo = mid,
                    }
                }
                best
            }
        };
        let (mut bed, mut deepest) = (f64::INFINITY, (0.0f64, route[0]));
        for &c in &route {
            bed = bed.min(self.water[c]);
            if self.water[c] - bed > deepest.0 {
                deepest = (self.water[c] - bed, c);
            }
        }
        Some(Reach { failed, rise: deepest.0, at: (deepest.1 % w, deepest.1 / w), ended_at_sea: sea })
    }
}

#[derive(PartialEq)]
struct MaxKey(f64, usize);
impl Eq for MaxKey {}
impl PartialOrd for MaxKey {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for MaxKey {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0).then(o.1.cmp(&self.1))
    }
}

/// The reaches of a set in parallel (`threads`), in input order.
fn measure_all(m: &Measure, lines: &[Vec<(f64, f64)>], mouth: &[bool], threads: usize) -> Vec<Option<Reach>> {
    let mut out: Vec<Option<Reach>> = vec![None; lines.len()];
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= lines.len() {
                    break;
                }
                let r = m.reach(&lines[k], mouth[k]);
                results.lock().expect("lock").push((k, r));
            });
        }
    });
    for (k, r) in results.into_inner().expect("lock") {
        out[k] = r;
    }
    out
}

// ---------------------------------------------------------------- comparisons

struct Topo {
    reaches: HashSet<Vec<u32>>,
    junctions: HashSet<u32>,
    mouths: HashSet<u32>,
    cells: HashSet<u32>,
}

fn topo(h1: &H1, in_box: &dyn Fn(u32) -> bool) -> Topo {
    let mut t = Topo { reaches: HashSet::new(), junctions: HashSet::new(), mouths: HashSet::new(), cells: HashSet::new() };
    let mut ups: HashMap<u32, usize> = HashMap::new();
    for (k, cells) in h1.trunk_cells.iter().enumerate() {
        if !cells.iter().any(|&c| in_box(c)) {
            continue;
        }
        t.reaches.insert(cells.clone());
        t.cells.extend(cells.iter().copied().filter(|&c| in_box(c)));
        let sg = &h1.hydro.segments[h1.trunk_seg[k]];
        if sg.downstream == NONE {
            if in_box(sg.down) {
                t.mouths.insert(sg.down);
            }
        } else {
            *ups.entry(sg.down).or_default() += 1;
        }
    }
    t.junctions = ups.into_iter().filter(|&(c, n)| n >= 2 && in_box(c)).map(|(c, _)| c).collect();
    t
}

fn near(set: &HashSet<u32>, c: u32, rw: usize, r: i64) -> bool {
    let (x, y) = ((c as usize % rw) as i64, (c as usize / rw) as i64);
    (-r..=r).any(|dy| {
        (-r..=r).any(|dx| x + dx >= 0 && x + dx < rw as i64 && y + dy >= 0 && set.contains(&(((y + dy) * rw as i64 + x + dx) as u32)))
    })
}

fn diff_line(name: &str, a: &Topo, b: &Topo, rw: usize) -> String {
    let changed = a.reaches.iter().filter(|r| !b.reaches.contains(*r)).count();
    let new = b.reaches.iter().filter(|r| !a.reaches.contains(*r)).count();
    // a reach moved: one of its cells lies more than 1 cell from every trunk cell of the other graph
    let moved1 = a.reaches.iter().filter(|r| r.iter().any(|&c| a.cells.contains(&c) && !near(&b.cells, c, rw, 1))).count();
    let moved1b = b.reaches.iter().filter(|r| r.iter().any(|&c| b.cells.contains(&c) && !near(&a.cells, c, rw, 1))).count();
    let j_lost = a.junctions.iter().filter(|&&c| !b.junctions.contains(&c)).count();
    let j_lost1 = a.junctions.iter().filter(|&&c| !near(&b.junctions, c, rw, 1)).count();
    let o_lost = a.mouths.iter().filter(|&&c| !b.mouths.contains(&c)).count();
    let o_lost1 = a.mouths.iter().filter(|&&c| !near(&b.mouths, c, rw, 1)).count();
    let cells_off = b.cells.iter().filter(|&&c| !a.cells.contains(&c)).count();
    let cells_off1 = b.cells.iter().filter(|&&c| !near(&a.cells, c, rw, 1)).count();
    format!(
        "{name}: reaches {} -> {} ({changed} of the first not in the second, {:.2} %; {new} new; {moved1} of the first and {moved1b} of the second with a cell more than 1 cell off the other's trunks); junctions {} -> {} ({j_lost} moved, {j_lost1} by more than 1 cell); outlets {} -> {} ({o_lost} moved, {o_lost1} by more than 1 cell); trunk cells {} -> {} ({cells_off} of the second off the first's cells, {:.2} %; {cells_off1} more than 1 cell off, {:.2} %)",
        a.reaches.len(),
        b.reaches.len(),
        100.0 * changed as f64 / a.reaches.len().max(1) as f64,
        a.junctions.len(),
        b.junctions.len(),
        a.mouths.len(),
        b.mouths.len(),
        a.cells.len(),
        b.cells.len(),
        100.0 * cells_off as f64 / b.cells.len().max(1) as f64,
        100.0 * cells_off1 as f64 / b.cells.len().max(1) as f64,
    )
}

/// The outlet each cell drains to.
fn roots(r: &Routing) -> Vec<u32> {
    catchment_roots(&r.receiver, &r.order)
}

/// Area (km2) of the land cells in the box whose outlet differs, and the count whose receiver differs.
fn catchment_diff(grid: &Grid, a: &H1, b: &H1, land_box: &[bool], tol: i64) -> (f64, usize, f64) {
    let (ra, rb) = (roots(&a.routing), roots(&b.routing));
    let rw = grid.w();
    let far = |x: u32, y: u32| {
        let (ax, ay, bx, by) = ((x as usize % rw) as i64, (x as usize / rw) as i64, (y as usize % rw) as i64, (y as usize / rw) as i64);
        (ax - bx).abs().max((ay - by).abs()) > tol
    };
    let (mut area, mut recv, mut total) = (0.0, 0usize, 0.0);
    for c in 0..grid.len() {
        if !land_box[c] {
            continue;
        }
        total += grid.cell_area(c);
        if far(ra[c], rb[c]) {
            area += grid.cell_area(c);
        }
        recv += usize::from(a.routing.receiver[c] != b.routing.receiver[c]);
    }
    (area / 1e6, recv, total / 1e6)
}

/// The 10 largest rivers of `a` by mouth discharge (mouths in or out of the box, a cell in the box): per river, whether `b` keeps its mouth (within 1 cell) and the
/// share of its main stem (largest-Q upstream chain) within 1 cell of `b`'s trunk cells.
fn major(a: &H1, b: &H1, in_box: &dyn Fn(u32) -> bool, rw: usize) -> Vec<(u32, f64, bool, f64, f64, i64, usize)> {
    let segs = &a.hydro.segments;
    let trunk: HashSet<usize> = a.trunk_seg.iter().copied().collect();
    let mut ups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, s) in segs.iter().enumerate() {
        if s.downstream != NONE && trunk.contains(&k) {
            ups.entry(s.downstream as usize).or_default().push(k);
        }
    }
    let cells_of = |h1: &H1, k: usize| -> Vec<u32> {
        let s = &h1.hydro.segments[k];
        let mut v = vec![s.up];
        let mut c = s.up;
        while c != s.down {
            c = h1.routing.receiver[c as usize];
            v.push(c);
        }
        v
    };
    let b_cells: HashSet<u32> = b.trunk_cells.iter().flatten().copied().collect();
    // the rivers leaving the box: trunk segments whose end cell is in the box and whose downstream segment ends outside it (or none: a mouth)
    let exits_of = |h1: &H1| -> Vec<usize> {
        let sg = &h1.hydro.segments;
        let mut v: Vec<usize> = h1
            .trunk_seg
            .iter()
            .copied()
            .filter(|&k| in_box(sg[k].down) && (sg[k].downstream == NONE || !in_box(sg[sg[k].downstream as usize].down)))
            .collect();
        v.sort_by(|&x, &y| sg[y].q_down.total_cmp(&sg[x].q_down).then(x.cmp(&y)));
        let mut seen = HashSet::new();
        v.retain(|&k| seen.insert(sg[k].down));
        v
    };
    let mouths = exits_of(a);
    let b_exit: Vec<(u32, f64)> = exits_of(b).iter().map(|&k| (b.hydro.segments[k].down, b.hydro.segments[k].q_down)).collect();
    let b_mouths: HashSet<u32> = b_exit.iter().map(|e| e.0).collect();
    let b_q: HashMap<u32, f64> = b_exit.into_iter().collect();
    let mut out = Vec::new();
    for &m in mouths.iter() {
        if out.len() == 10 {
            break;
        }
        let mut stem = Vec::new();
        let mut k = m;
        loop {
            stem.extend(cells_of(a, k));
            // upstream segments end at this segment's up cell
            let Some(u) = ups.get(&k) else { break };
            k = *u.iter().max_by(|&&x, &&y| segs[x].q_down.total_cmp(&segs[y].q_down).then(y.cmp(&x))).expect("non-empty");
        }
        let stem_in: Vec<u32> = stem.into_iter().filter(|&c| in_box(c)).collect();
        // a major river runs at least 20 cells (16 km) of its main stem in the box
        if stem_in.len() < 20 {
            continue;
        }
        let kept = stem_in.iter().filter(|&&c| near(&b_cells, c, rw, 1)).count();
        let mouth = segs[m].down;
        let mouth_kept = near(&b_mouths, mouth, rw, 1);
        // the b mouth of the largest Q within 10 cells (Chebyshev), and its distance
        let (mx, my) = ((mouth as usize % rw) as i64, (mouth as usize / rw) as i64);
        let mut qb = (0.0f64, -1i64);
        for (&c, &q) in &b_q {
            let (x, y) = ((c as usize % rw) as i64, (c as usize / rw) as i64);
            let dd = (x - mx).abs().max((y - my).abs());
            if dd <= 10 && q > qb.0 {
                qb = (q, dd);
            }
        }
        out.push((mouth, segs[m].q_down, mouth_kept, 100.0 * kept as f64 / stem_in.len().max(1) as f64, qb.0, qb.1, stem_in.len()));
    }
    out
}

/// Upstream trunk segments of each trunk segment.
fn ups_of(h1: &H1) -> HashMap<usize, Vec<usize>> {
    let trunk: HashSet<usize> = h1.trunk_seg.iter().copied().collect();
    let mut ups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, s) in h1.hydro.segments.iter().enumerate() {
        if s.downstream != NONE && trunk.contains(&k) {
            ups.entry(s.downstream as usize).or_default().push(k);
        }
    }
    ups
}

/// The main stem upstream of segment `k` (the upstream segment of the largest D8 area at each confluence): its cells, and its major confluences in the box
/// (a tributary whose D8 area above the confluence is at least 75 km2 and 10 % of the stem's D8 area at the confluence) as (cell, tributary D8 area km2).
/// D8 area, not the MFD Q: the MFD splits a trunk's Q over parallel D8 lines that re-join, and those are no tributaries.
fn main_stem(h1: &H1, ups: &HashMap<usize, Vec<usize>>, k: usize, in_box: &dyn Fn(u32) -> bool) -> (Vec<u32>, Vec<(u32, f64)>) {
    let segs = &h1.hydro.segments;
    let (mut cells, mut conf) = (Vec::new(), Vec::new());
    let mut k = k;
    loop {
        let s = &segs[k];
        let mut c = s.up;
        cells.push(c);
        while c != s.down {
            c = h1.routing.receiver[c as usize];
            cells.push(c);
        }
        let Some(u) = ups.get(&k) else { break };
        // a tributary's D8 area just above the confluence: its cell whose receiver is the confluence
        let above = |t: usize| -> f64 {
            let st = &segs[t];
            let mut c = st.up;
            while h1.routing.receiver[c as usize] != st.down && c != st.down {
                c = h1.routing.receiver[c as usize];
            }
            h1.d8[c as usize]
        };
        let main = *u.iter().max_by(|&&x, &&y| above(x).total_cmp(&above(y)).then(y.cmp(&x))).expect("non-empty");
        if in_box(s.up) {
            for &t in u {
                let a = above(t);
                if t != main && a >= 75.0e6 && a >= 0.1 * h1.d8[s.up as usize] {
                    conf.push((s.up, a / 1e6));
                }
            }
        }
        k = main;
    }
    (cells, conf)
}

/// Every trunk confluence of a graph: its cell and the D8 areas (km2) of all segments joining there.
fn confluences(h1: &H1, ups: &HashMap<usize, Vec<usize>>) -> Vec<(u32, Vec<f64>)> {
    let segs = &h1.hydro.segments;
    let above = |t: usize| -> f64 {
        let st = &segs[t];
        let mut c = st.up;
        while h1.routing.receiver[c as usize] != st.down && c != st.down {
            c = h1.routing.receiver[c as usize];
        }
        h1.d8[c as usize] / 1e6
    };
    let mut v: Vec<(u32, Vec<f64>)> =
        ups.iter().filter(|(_, u)| u.len() >= 2).map(|(&k, u)| (segs[k].up, u.iter().map(|&t| above(t)).collect())).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Whether `set` holds a confluence within 2 cells of `c` that a segment of D8 area within a factor 2 of `a` (km2) joins.
fn has_confluence(set: &[(u32, Vec<f64>)], c: u32, a: f64, rw: usize) -> bool {
    set.iter().any(|(x, areas)| cheb(*x, c, rw) <= 2 && areas.iter().any(|&t| t >= 0.5 * a && t <= 2.0 * a))
}

fn cheb(a: u32, b: u32, rw: usize) -> i64 {
    let (ax, ay, bx, by) = ((a as usize % rw) as i64, (a as usize / rw) as i64, (b as usize % rw) as i64, (b as usize / rw) as i64);
    (ax - bx).abs().max((ay - by).abs())
}

/// The addendum's stop test (lead #131a): for the 10 largest rivers of `a` leaving the box (20 main-stem cells or more in it), whether the water at `a`'s exit cell
/// reaches an outlet of `b` more than 10 cells (8 km) from `a`'s outlet, and the major confluences of its main stem that `b` lacks (no confluence of `b` within 2
/// cells joined by a segment of 0.5 to 2 times the tributary's D8 area), and those of `b`'s main stem (from its trunk cell of the largest D8 area within 2 cells
/// of the exit) that `a` lacks. Matching against every confluence, not only the other main stem's, keeps a switch of the main-stem branch out of the count.
fn stop_test(a: &H1, b: &H1, in_box: &dyn Fn(u32) -> bool, rw: usize) -> Vec<String> {
    let (ua, ub) = (ups_of(a), ups_of(b));
    let (all_a, all_b) = (confluences(a, &ua), confluences(b, &ub));
    let segs = &a.hydro.segments;
    let mut exits: Vec<usize> = a
        .trunk_seg
        .iter()
        .copied()
        .filter(|&k| in_box(segs[k].down) && (segs[k].downstream == NONE || !in_box(segs[segs[k].downstream as usize].down)))
        .collect();
    exits.sort_by(|&x, &y| segs[y].q_down.total_cmp(&segs[x].q_down).then(x.cmp(&y)));
    let mut seen = HashSet::new();
    exits.retain(|&k| seen.insert(segs[k].down));
    let (ra, rb) = (roots(&a.routing), roots(&b.routing));
    // b's segment of every trunk cell
    let mut seg_of_b: HashMap<u32, usize> = HashMap::new();
    for &k in &b.trunk_seg {
        let s = &b.hydro.segments[k];
        let mut c = s.up;
        seg_of_b.insert(c, k);
        while c != s.down {
            c = b.routing.receiver[c as usize];
            seg_of_b.entry(c).or_insert(k);
        }
    }
    let mut out = Vec::new();
    for &m in &exits {
        if out.len() == 10 {
            break;
        }
        let (stem, conf_a) = main_stem(a, &ua, m, in_box);
        if stem.iter().filter(|&&c| in_box(c)).count() < 20 {
            continue;
        }
        let exit = segs[m].down;
        let od = cheb(ra[exit as usize], rb[exit as usize], rw);
        // b's counterpart: its trunk cell of the largest D8 area within 2 cells of the exit
        let mut best: Option<(f64, u32)> = None;
        let (ex, ey) = ((exit as usize % rw) as i64, (exit as usize / rw) as i64);
        for dy in -2..=2i64 {
            for dx in -2..=2i64 {
                let c = ((ey + dy) * rw as i64 + ex + dx) as u32;
                if seg_of_b.contains_key(&c) && best.is_none_or(|(q, bc)| b.d8[c as usize] > q || (b.d8[c as usize] == q && c < bc)) {
                    best = Some((b.d8[c as usize], c));
                }
            }
        }
        let (stem_b, conf_b) = best.map(|(_, c)| main_stem(b, &ub, seg_of_b[&c], in_box)).unwrap_or_default();
        let stem_b_in = stem_b.iter().filter(|&&c| in_box(c)).count();
        let stem_a_in = stem.iter().filter(|&&c| in_box(c)).count();
        let d8b = best.map_or(0.0, |(a, _)| a);
        // lost: a major confluence of a's main stem with no confluence of b (anywhere in b's trunk graph) within 2 cells joined by a segment of a similar D8 area;
        // gained: the same the other way round, for b's main stem against all of a's confluences
        let lost: Vec<&(u32, f64)> = conf_a.iter().filter(|x| !has_confluence(&all_b, x.0, x.1, rw)).collect();
        let gained: Vec<&(u32, f64)> = conf_b.iter().filter(|y| !has_confluence(&all_a, y.0, y.1, rw)).collect();
        let fmt = |v: &[&(u32, f64)]| v.iter().map(|x| format!("{}({:.0})", x.0, x.1)).collect::<Vec<_>>().join("/");
        out.push(format!(
            "exit {exit} Q {:.0} D8 {:.0} -> {:.0} km2, stem {stem_a_in} -> {stem_b_in} cells in the box: outlet {} cells off{}; {} major confluences (D8 km2), lost {} [{}], gained {} [{}]{}",
            segs[m].q_down,
            a.d8[exit as usize] / 1e6,
            d8b / 1e6,
            od,
            if od > 10 { " (CHANGED)" } else { "" },
            conf_a.len(),
            lost.len(),
            fmt(&lost),
            gained.len(),
            fmt(&gained),
            if best.is_none() { "; no trunk of the second within 2 cells of the exit" } else { "" }
        ));
    }
    out
}

/// Trunk heads (trunk segments without an upstream trunk segment, head cell in the box), trunk length in the box (km, cell to receiver) and trunk cells in the box.
fn trunk_stats(h1: &H1, grid: &Grid, in_box: &dyn Fn(u32) -> bool) -> (usize, usize, f64, usize) {
    let ups = ups_of(h1);
    let head_cells: Vec<u32> = h1
        .trunk_seg
        .iter()
        .filter(|&&k| !ups.contains_key(&k) && in_box(h1.hydro.segments[k].up))
        .map(|&k| h1.hydro.segments[k].up)
        .collect();
    let heads = head_cells.len();
    let side = head_cells.iter().filter(|&&c| h1.d8[c as usize] < 75.0e6).count();
    let mut cells: HashSet<u32> = HashSet::new();
    for cs in &h1.trunk_cells {
        cells.extend(cs.iter().copied().filter(|&c| in_box(c)));
    }
    let mut len = 0.0;
    for &c in &cells {
        let r = h1.routing.receiver[c as usize];
        if r != NONE {
            let d = (0..8).find(|&d| grid.neighbour(c as usize, d) == Some(r as usize)).expect("a receiver is a neighbour");
            len += grid.distance_at(c as usize, d);
        }
    }
    (heads, side, len / 1000.0, cells.len())
}

// ---------------------------------------------------------------- KG-trunk (lead #132, G1c)

const BODY: u64 = 0x6731_C000_0000_0001;
/// L cells of a tile (K7's canonical tile grid, GEN-182).
const TILE: u64 = 768;

/// A global cell id: the row in the high half, the column in the low half (Grid::global_id's form).
fn gid(gx: u64, gy: u64) -> u64 {
    (gy << 32) | gx
}

fn gxy(id: u64) -> (u64, u64) {
    (id & 0xFFFF_FFFF, id >> 32)
}

/// The maximin corridor route through fixed waypoints on `z` (land-only steps, a sea cell ends it), as river_suite's corridor measure; the minimax rise found to
/// 0.1 m by bisection when no route keeps within `bar`. Returns the route (window cells), whether it failed the bar, its largest rise.
fn route_through(m: &Measure, line: &[(f64, f64)], points: &[usize]) -> Option<(Vec<usize>, bool, f64)> {
    let (index, cells) = m.corridor(line)?;
    if points.iter().any(|p| !index.contains_key(p)) {
        return None;
    }
    let top = cells.iter().map(|&c| m.water[c]).fold(f64::NEG_INFINITY, f64::max)
        - cells.iter().map(|&c| m.water[c]).fold(f64::INFINITY, f64::min);
    let (route, failed) = match m.waypoints(&index, &cells, points, m.bar) {
        Some((r, _)) => (r, false),
        None => {
            let (mut lo, mut hi) = (m.bar, top.max(m.bar));
            let (mut best, _) = m.waypoints(&index, &cells, points, hi)?;
            while hi - lo > 0.1 {
                let mid = 0.5 * (lo + hi);
                match m.waypoints(&index, &cells, points, mid) {
                    Some((r, _)) => (hi, best) = (mid, r),
                    None => lo = mid,
                }
            }
            (best, true)
        }
    };
    let (mut bed, mut rise) = (f64::INFINITY, 0.0f64);
    for &c in &route {
        bed = bed.min(m.water[c]);
        rise = rise.max(m.water[c] - bed);
    }
    Some((route, failed, rise))
}

/// The lowest-path centreline: Dijkstra inside the corridor from `points[0]` through each waypoint, the cost of a step its length times `1 + (z - z_min) / 10 m`
/// (`z_min` the corridor's lowest cell), steps from land only, a sea cell ends the route; ties by cell id. Returns the route, whether its largest rise above its
/// running minimum exceeds `bar`, and that rise.
fn route_low(m: &Measure, line: &[(f64, f64)], points: &[usize]) -> Option<(Vec<usize>, bool, f64)> {
    let (index, cells) = m.corridor(line)?;
    if points.iter().any(|p| !index.contains_key(p)) {
        return None;
    }
    let zmin = cells.iter().map(|&c| m.water[c]).fold(f64::INFINITY, f64::min);
    let w = m.w;
    let mut route = vec![points[0]];
    for leg in points.windows(2) {
        let (from, to) = (leg[0], leg[1]);
        let mut dist = vec![f64::INFINITY; cells.len()];
        let mut parent = vec![usize::MAX; cells.len()];
        let mut heap = BinaryHeap::new();
        dist[index[&from]] = 0.0;
        heap.push(Key(0.0, from as u32));
        let mut end = None;
        while let Some(Key(d, c)) = heap.pop() {
            let c = c as usize;
            let ci = index[&c];
            if d > dist[ci] {
                continue;
            }
            if c == to || (c != from && !m.land(c)) {
                end = Some(c);
                break;
            }
            let (cx, cy) = ((c % w) as i64, (c / w) as i64);
            for (dx, dy) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= m.h as i64 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                let Some(&ni) = index.get(&n) else { continue };
                let len = if dx != 0 && dy != 0 { std::f64::consts::SQRT_2 } else { 1.0 };
                let nd = d + len * (1.0 + (m.water[n] - zmin).max(0.0) / 10.0);
                if nd < dist[ni] {
                    dist[ni] = nd;
                    parent[ni] = ci;
                    heap.push(Key(nd, n as u32));
                }
            }
        }
        let e = end?;
        let mut part = vec![e];
        let mut k = index[&e];
        while parent[k] != usize::MAX {
            k = parent[k];
            part.push(cells[k]);
        }
        part.reverse();
        route.extend_from_slice(&part[1..]);
        if e != to {
            break;
        }
    }
    let (mut bed, mut rise) = (f64::INFINITY, 0.0f64);
    for &c in &route {
        bed = bed.min(m.water[c]);
        rise = rise.max(m.water[c] - bed);
    }
    Some((route, rise > m.bar, rise))
}

/// An H1 reach of the trunk graph before its L image: the H1 cells (up first), its up and down node kinds, its source segment.
struct H1Reach {
    cells: Vec<u32>,
    up_kind: NodeKind,
    down_kind: NodeKind,
    seg: usize,
    /// The kept basin it enters (down node a lake inlet) or leaves (up node a lake spill).
    basin_in: Option<usize>,
    basin_out: Option<usize>,
}

/// The H1 trunk reaches: every channel segment (>= 75 km2), cut where it enters a kept basin (a `lake_inlet`) and restarted at the basin's spill cell (a `lake_spill`);
/// a segment ending at a sea outlet ends at an `outlet_sea`; one ending on the window's edge also at an `outlet_sea` node on the edge cell (a window is not a domain:
/// the landmass graph continues there; counted apart).
fn h1_reaches(h1: &H1, grid: &Grid, sea_outlet: &dyn Fn(u32) -> bool, budget: &[f64]) -> (Vec<H1Reach>, usize) {
    let segs = &h1.hydro.segments;
    let mut inflows = vec![0usize; segs.len()];
    for s in segs {
        if s.downstream != NONE {
            inflows[s.downstream as usize] += 1;
        }
    }
    // reading 13: a basin cuts the trunk only when its depth (spill - floor) is within the budget at its floor; a deeper one is crossed
    let kept = |c: u32| -> Option<usize> {
        let b = h1.hydro.label[c as usize];
        if b == NONE {
            return None;
        }
        let bs = &h1.hydro.basins[b as usize];
        (bs.spill_m - bs.floor_m <= budget[bs.floor_cell as usize]).then_some(b as usize)
    };
    let mut out = Vec::new();
    let mut edge_ends = 0usize;
    for (k, s) in segs.iter().enumerate() {
        let mut cells = vec![s.up];
        let mut c = s.up;
        while c != s.down {
            c = h1.routing.receiver[c as usize];
            cells.push(c);
        }
        let up_kind = if inflows[k] >= 2 { NodeKind::Confluence } else { NodeKind::Source };
        let down_kind = if s.downstream != NONE {
            NodeKind::Confluence
        } else {
            if !sea_outlet(s.down) {
                edge_ends += 1;
            }
            NodeKind::OutletSea
        };
        // cut at kept basins
        let mut start = 0usize;
        let mut kind = up_kind;
        let mut from_basin: Option<usize> = None;
        let mut i = 0usize;
        while i < cells.len() {
            if let Some(b) = kept(cells[i]) {
                if i > start || (i == start && from_basin.is_none() && i > 0) {
                    out.push(H1Reach {
                        cells: cells[start..=i].to_vec(),
                        up_kind: kind,
                        down_kind: NodeKind::LakeInlet,
                        seg: k,
                        basin_in: Some(b),
                        basin_out: from_basin,
                    });
                }
                // skip the basin's cells; restart at its spill cell when the chain leaves through it
                let spill = h1.hydro.basins[b].spill_cell;
                let mut j = i;
                while j < cells.len() && kept(cells[j]) == Some(b) {
                    j += 1;
                }
                // the spill cell on the chain: the first cell after the basin, or the spill cell itself when it is on the chain
                let at = cells.iter().position(|&x| x == spill).filter(|&p| p >= i).unwrap_or(j.min(cells.len() - 1));
                start = at;
                kind = NodeKind::LakeSpill;
                from_basin = Some(b);
                i = at.max(j);
                if start >= cells.len() - 1 {
                    break;
                }
                continue;
            }
            i += 1;
        }
        if start < cells.len() - 1 {
            out.push(H1Reach { cells: cells[start..].to_vec(), up_kind: kind, down_kind, seg: k, basin_in: None, basin_out: from_basin });
        }
        let _ = grid;
    }
    (out, edge_ends)
}

/// Strahler order of the H1 segments (sources 1; at a confluence the largest, plus one when two or more inflows share it).
fn strahler(h1: &H1) -> Vec<u8> {
    let segs = &h1.hydro.segments;
    let mut ups: Vec<Vec<usize>> = vec![Vec::new(); segs.len()];
    for (k, s) in segs.iter().enumerate() {
        if s.downstream != NONE {
            ups[s.downstream as usize].push(k);
        }
    }
    let mut order = vec![0u8; segs.len()];
    fn go(k: usize, ups: &[Vec<usize>], order: &mut [u8]) -> u8 {
        if order[k] != 0 {
            return order[k];
        }
        let v: Vec<u8> = ups[k].clone().into_iter().map(|u| go(u, ups, order)).collect();
        let o = match v.iter().max() {
            None => 1,
            Some(&m) => {
                if v.iter().filter(|&&x| x == m).count() >= 2 {
                    m + 1
                } else {
                    m
                }
            }
        };
        order[k] = o;
        o
    }
    for k in 0..segs.len() {
        go(k, &ups, &mut order);
    }
    order
}

/// The budget field of contract §6 at H1: `relief_1km` (the range of `s0` over the L cells within 5 cells of the cell's centre L cell), its p50 over the H1
/// cells within 25 km (evaluated on every `stride`-th cell and read from the nearest such cell), `budget = min(50, max(10, 0.5 relief10))`.
fn budget_field(s0: &[f64], lw: usize, rw: usize, rh: usize, dx_h1: f64, stride: usize, threads: usize) -> Vec<f64> {
    let mut r1 = vec![0f64; rw * rh];
    par_fill(&mut r1, rw, threads, &|c| {
        let (cx, cy) = ((c % rw) * L + L / 2, (c / rw) * L + L / 2);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for y in cy.saturating_sub(5)..(cy + 6).min(rh * L) {
            for x in cx.saturating_sub(5)..(cx + 6).min(lw) {
                let v = s0[y * lw + x];
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        hi - lo
    });
    let rad = (25_000.0 / dx_h1).floor() as i64;
    let (sw, sh) = (rw.div_ceil(stride), rh.div_ceil(stride));
    let mut coarse = vec![0f64; sw * sh];
    par_fill(&mut coarse, sw, threads, &|k| {
        let (cx, cy) = (((k % sw) * stride) as i64, ((k / sw) * stride) as i64);
        let mut v: Vec<f64> = Vec::new();
        for dy in -rad..=rad {
            for dx in -rad..=rad {
                if dx * dx + dy * dy > rad * rad {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as usize) < rw && (y as usize) < rh {
                    v.push(r1[y as usize * rw + x as usize]);
                }
            }
        }
        v.sort_by(f64::total_cmp);
        (0.5 * v[v.len() / 2]).clamp(10.0, 50.0)
    });
    (0..rw * rh).map(|c| coarse[((c / rw) / stride).min(sh - 1) * sw + ((c % rw) / stride).min(sw - 1)]).collect()
}

/// The L trunk graph of the window: H1 trunk graph (level H1) and its L images (level L), all owned by the landmass.
#[allow(clippy::too_many_arguments)]
fn build_graph(
    h1: &H1,
    reaches: &[H1Reach],
    grid: &Grid,
    s0: &[f64],
    ks0: &[f64],
    budget_h1: &[f64],
    window: (u64, u64, usize, usize, usize),
    sea: f64,
    bar: f64,
    order: &[u8],
    threads: usize,
    low_centre: bool,
    ocean: &[bool],
) -> (Graph, Vec<String>) {
    let (x0, y0, w, h, rw) = window;
    let h1_gid = |c: u32| grid.global_id(c as usize);
    let l_gid = |c: usize| gid(x0 + (c % w) as u64, y0 + (c / w) as u64);
    let to_local = |c: u32| {
        let c = c as usize;
        ((L * (c % rw)) as f64 + (L as f64 - 1.0) / 2.0, (L * (c / rw)) as f64 + (L as f64 - 1.0) / 2.0)
    };
    let parent_h1 = |c: usize| ((c / w) / L) * rw + (c % w) / L;
    let mut notes = Vec::new();
    let mut g = Graph { body: BODY, nodes: Vec::new(), reaches: Vec::new(), basins: Vec::new() };
    // ---- H1 level
    let mut h1_node: HashMap<(u32, NodeKind), u64> = HashMap::new();
    let node_h1 = |g: &mut Graph, h1_node: &mut HashMap<(u32, NodeKind), u64>, c: u32, kind: NodeKind| -> u64 {
        *h1_node.entry((c, kind)).or_insert_with(|| {
            let id = node_id(BODY, kind, Level::H1, h1_gid(c));
            g.nodes.push(Node { id, kind, level: Level::H1, anchor: h1_gid(c), parent: None, owner: Domain::Landmass, marker: None });
            id
        })
    };
    let mut h1_reach_ids = Vec::with_capacity(reaches.len());
    for r in reaches {
        let up = node_h1(&mut g, &mut h1_node, r.cells[0], r.up_kind);
        let down = node_h1(&mut g, &mut h1_node, *r.cells.last().expect("cells"), r.down_kind);
        let id = reach_id(BODY, Level::H1, up, down);
        h1_reach_ids.push(id);
        let n = r.cells.len();
        let attrs = Attrs {
            a_geo: r.cells.iter().map(|&c| h1.d8[c as usize]).collect(),
            a_ext: vec![0.0; n],
            a_run: r.cells.iter().map(|&c| h1.hydro.q[c as usize]).collect(),
            a_run_no_ext: r.cells.iter().map(|&c| h1.hydro.q[c as usize]).collect(),
            q: r.cells.iter().map(|&c| h1.hydro.q[c as usize]).collect(),
            q_w: r.cells.iter().map(|&c| h1.hydro.qw[c as usize]).collect(),
            strahler: vec![order[r.seg]; n],
            bed: vec![0; n],
        };
        g.reaches.push(GReach {
            id,
            level: Level::H1,
            up,
            down,
            vertices: r.cells.iter().map(|&c| h1_gid(c)).collect(),
            parent: None,
            owner: Domain::Landmass,
            attrs,
            exceptions: Vec::new(),
        });
    }
    // ---- L anchors (contract §2.1: the lowest L cell of S0 within one H1 cell, ties by global cell id; land only for a non-outlet node, reading 2 for outlets)
    let all_inside = vec![true; w * h];
    // reading 12: a cell of the connected sea counts at the sea level; any other cell keeps its height (an inland cell below the sea level is land)
    let ks_water: Vec<f64> = (0..ks0.len()).map(|c| if ocean[c] { ks0[c].max(sea) } else { ks0[c] }).collect();
    let mks =
        Measure { w, h, water: &ks_water, inside: &all_inside, region: &all_inside, sea, radius: K_C * L as f64, bar, ocean: Some(ocean) };
    let anchor_of = |c: u32, outlet: bool, taken: &HashSet<usize>| -> Option<usize> {
        let p = to_local(c);
        let disc = |keep: &dyn Fn(usize) -> bool| -> Option<usize> {
            let r = L as f64;
            let (x0_, x1_) = (((p.0 - r).floor() as i64).max(0), ((p.0 + r).ceil() as i64).min(w as i64 - 1));
            let (y0_, y1_) = (((p.1 - r).floor() as i64).max(0), ((p.1 + r).ceil() as i64).min(h as i64 - 1));
            let mut best: Option<usize> = None;
            for y in y0_..=y1_ {
                for x in x0_..=x1_ {
                    let cc = (y as usize) * w + x as usize;
                    if libm::hypot(x as f64 - p.0, y as f64 - p.1) > r || !keep(cc) {
                        continue;
                    }
                    if best.is_none_or(|b| s0[cc] < s0[b] || (s0[cc] == s0[b] && l_gid(cc) < l_gid(b))) {
                        best = Some(cc);
                    }
                }
            }
            best
        };
        if outlet {
            disc(&|cc| !taken.contains(&cc))
        } else {
            disc(&|cc| !ocean[cc] && !taken.contains(&cc)).or_else(|| disc(&|cc| !taken.contains(&cc)))
        }
    };
    // reading 7: the H1 nodes snap in order of descending q_w, ties by H1 node id; a cell already taken is excluded
    let mut h1_nodes: Vec<(u32, NodeKind)> =
        reaches.iter().flat_map(|r| [(r.cells[0], r.up_kind), (*r.cells.last().expect("cells"), r.down_kind)]).collect();
    h1_nodes.sort_by_key(|&(c, k)| (c, k));
    h1_nodes.dedup();
    h1_nodes.sort_by(|a, b| {
        h1.hydro.qw[b.0 as usize].total_cmp(&h1.hydro.qw[a.0 as usize]).then(node_id(BODY, a.1, Level::H1, h1_gid(a.0)).cmp(&node_id(
            BODY,
            b.1,
            Level::H1,
            h1_gid(b.0),
        )))
    });
    let mut taken: HashSet<usize> = HashSet::new();
    let mut anchor: HashMap<(u32, NodeKind), usize> = HashMap::new();
    for &(c, kind) in &h1_nodes {
        if let Some(a) = anchor_of(c, kind == NodeKind::OutletSea, &taken) {
            taken.insert(a);
            anchor.insert((c, kind), a);
        }
    }
    // routes in parallel
    let jobs: Vec<(usize, Vec<(f64, f64)>, usize, usize)> = reaches
        .iter()
        .enumerate()
        .filter_map(|(k, r)| {
            let up = *anchor.get(&(r.cells[0], r.up_kind))?;
            let down = *anchor.get(&(*r.cells.last().expect("cells"), r.down_kind))?;
            Some((k, r.cells.iter().map(|&c| to_local(c)).collect(), up, down))
        })
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let q = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if q >= jobs.len() {
                    break;
                }
                let (k, line, up, down) = &jobs[q];
                let pts = if up == down { vec![*up] } else { vec![*up, *down] };
                let r = if pts.len() < 2 {
                    Some((pts.clone(), false, 0.0))
                } else if low_centre {
                    route_low(&mks, line, &pts)
                } else {
                    route_through(&mks, line, &pts)
                };
                done.lock().expect("lock").push((*k, *up, *down, r));
            });
        }
    });
    let mut routed: Vec<(usize, usize, usize, Option<(Vec<usize>, bool, f64)>)> = done.into_inner().expect("lock");
    routed.sort_by_key(|x| x.0);
    // ---- L nodes and reaches, beds in topological order of the H1 reaches (upstream first: by the segments' order along the routing)
    let mut l_node: HashMap<(usize, NodeKind), u64> = HashMap::new();
    let mut l_parent: HashMap<u64, u32> = HashMap::new();
    let mut merged: BTreeSet<u64> = BTreeSet::new();
    let mut anchor_owner: HashMap<usize, u64> = HashMap::new();
    let mut collisions = 0usize;
    let mut skipped = 0usize;
    let mut node_bed: HashMap<u64, f64> = HashMap::new();
    // topological order of reaches: by the routing order position of their first H1 cell, downstream first reversed
    let pos: HashMap<u32, usize> = h1.routing.order.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let mut idx: Vec<usize> = (0..routed.len()).collect();
    idx.sort_by_key(|&i| std::cmp::Reverse(pos[&reaches[routed[i].0].cells[0]]));
    let mut exceptions = 0usize;
    let mut sea_ends = 0usize;
    let mut sea_inland = 0usize;
    let mut l_reaches: Vec<GReach> = Vec::new();
    let mut markers: Vec<Node> = Vec::new();
    for &i in &idx {
        let (k, up_c, down_c, ref r) = routed[i];
        let Some((route, failed, rise)) = r else {
            skipped += 1;
            continue;
        };
        let hr = &reaches[k];
        let mut route = route.clone();
        let mut down_c = down_c;
        let mut down_kind = hr.down_kind;
        if *route.last().expect("route") != down_c {
            // the route met the sea before the down node: the image ends at an outlet there
            down_c = *route.last().expect("route");
            down_kind = NodeKind::OutletSea;
            if hr.down_kind != NodeKind::OutletSea {
                sea_ends += 1;
                sea_inland += usize::from(!ocean[down_c]);
            }
        }
        if down_kind == NodeKind::OutletSea {
            // reading 2: the outlet image is the first sea cell of the chain
            if let Some(p) = route.iter().position(|&c| ocean[c]) {
                route.truncate(p + 1);
                down_c = route[p];
            }
        }
        if route.len() < 2 {
            skipped += 1;
            continue;
        }
        let mut mk = |c: usize, kind: NodeKind, parent_c: u32| -> u64 {
            let id = node_id(BODY, kind, Level::L, l_gid(c));
            if let Some(&pc) = l_parent.get(&id) {
                if pc != parent_c {
                    merged.insert(id);
                }
            } else {
                l_parent.insert(id, parent_c);
            }
            *l_node.entry((c, kind)).or_insert_with(|| {
                let id = node_id(BODY, kind, Level::L, l_gid(c));
                if let Some(&o) = anchor_owner.get(&c) {
                    if o != id {
                        collisions += 1;
                    }
                } else {
                    anchor_owner.insert(c, id);
                }
                g.nodes.push(Node {
                    id,
                    kind,
                    level: Level::L,
                    anchor: l_gid(c),
                    parent: Some(node_id(BODY, kind, Level::H1, h1_gid(parent_c))),
                    owner: Domain::Landmass,
                    marker: None,
                });
                id
            })
        };
        let up = mk(up_c, hr.up_kind, hr.cells[0]);
        let down = mk(down_c, down_kind, *hr.cells.last().expect("cells"));
        let _ = up_c;
        // the bed: running minimum of KS0 from the up node's bed, the cut at most budget - KS_used; beyond it the bed follows the surface (rise_over_budget)
        let n = route.len();
        let mut bed = vec![0f64; n];
        let mut run = node_bed.get(&up).copied().unwrap_or(f64::INFINITY).min(ks_water[route[0]]);
        let mut over = 0.0f64;
        let mut rise_at: Vec<usize> = Vec::new();
        for (v, &c) in route.iter().enumerate() {
            let z = ks_water[c];
            let ks_used = (s0[c] - ks0[c]).max(0.0);
            let room = (budget_h1[parent_h1(c)] - ks_used).max(0.0);
            if z - run > room {
                over = over.max(z - run - room);
                if z - room > run {
                    rise_at.push(v);
                }
                run = z - room;
            } else {
                run = run.min(z);
            }
            bed[v] = run;
        }
        // the confluence's bed is shared: the lowest inflow's
        let end_bed = bed[n - 1];
        let e = node_bed.entry(down).or_insert(end_bed);
        *e = e.min(end_bed);
        let id = reach_id(BODY, Level::L, up, down);
        let status = if over > 0.0 {
            exceptions += 1;
            for &v in &rise_at {
                // reading 8: hash(body, kind, level, anchor, on_reach) (GEN-189 follows in a main PR: main's validator still reports these ids as IdMismatch)
                let mid = id_hash(node_id(BODY, NodeKind::Exception, Level::L, l_gid(route[v])), id, 0x8A2E_0008);
                markers.push(Node {
                    id: mid,
                    kind: NodeKind::Exception,
                    level: Level::L,
                    anchor: l_gid(route[v]),
                    parent: None,
                    owner: Domain::Landmass,
                    marker: Some(Marker { on_reach: id, at_vertex: v as u32 }),
                });
            }
            vec![GException { reason: Reason::RiseOverBudget, value: over }]
        } else {
            Vec::new()
        };
        let _ = (failed, rise);
        // attributes: the H1 reach's vertex at the same share of the length
        let m = hr.cells.len();
        let at = |v: usize| hr.cells[if v + 1 == n { m - 1 } else { (v * (m - 1)) / (n - 1).max(1) }.min(m - 1)] as usize;
        let attrs = Attrs {
            a_geo: (0..n).map(|v| h1.d8[at(v)]).collect(),
            a_ext: vec![0.0; n],
            a_run: (0..n).map(|v| h1.hydro.q[at(v)]).collect(),
            a_run_no_ext: (0..n).map(|v| h1.hydro.q[at(v)]).collect(),
            q: (0..n).map(|v| h1.hydro.q[at(v)]).collect(),
            q_w: (0..n).map(|v| h1.hydro.qw[at(v)]).collect(),
            strahler: vec![order[hr.seg]; n],
            bed: bed.iter().map(|&b| (b / 0.5).floor() as i32).collect(),
        };
        l_reaches.push(GReach {
            id,
            level: Level::L,
            up,
            down,
            vertices: route.iter().map(|&c| l_gid(c)).collect(),
            parent: Some(h1_reach_ids[k]),
            owner: Domain::Landmass,
            attrs,
            exceptions: status,
        });
    }
    // shared node beds: each reach's last vertex carries its down node's bed
    for r in &mut l_reaches {
        if let Some(&b) = node_bed.get(&r.down) {
            let code = (b / 0.5).floor() as i32;
            let last = r.attrs.bed.len() - 1;
            r.attrs.bed[last] = r.attrs.bed[last].min(code);
        }
    }
    g.reaches.extend(l_reaches);
    g.nodes.extend(markers);
    notes.push(format!(
        "L reaches skipped (no route or a one-cell chain) {skipped}; routes meeting the sea before their down node (ended at an outlet_sea there) {sea_ends}, of them on an inland cell {sea_inland}; anchor collisions (two kinds on one L cell) {collisions}; rise_over_budget reaches {exceptions}"
    ));
    notes.push(format!("MERGED {}", merged.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")));
    g.sort();
    (g, notes)
}

/// L routing of a window surface (flood with epsilon, D8 steepest receivers, D8 area): the river cells over `a_min` m2 (land only).
fn l_rivers(z: &[f64], w: usize, h: usize, dx: f64, sea: f64, a_min: f64) -> Vec<bool> {
    let grid = Grid::new(w, h, dx);
    let outlet = grid.outlets(z, sea);
    let filled = priority_flood_with(&grid, z, &outlet, 1e-6, false);
    let n = w * h;
    let mut recv = vec![u32::MAX; n];
    for p in 0..n {
        if outlet[p] {
            continue;
        }
        let mut best = (0.0f64, u32::MAX);
        for d in 0..8 {
            let Some(q) = grid.neighbour(p, d) else { continue };
            let s = (filled[p] - filled[q]) / grid.distance_at(p, d);
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        recv[p] = best.1;
    }
    let mut by: Vec<u32> = (0..n as u32).collect();
    by.sort_unstable_by(|&a, &b| filled[b as usize].total_cmp(&filled[a as usize]).then(a.cmp(&b)));
    drop(filled);
    let mut area = vec![(dx * dx) as f32; n];
    for &p in &by {
        let r = recv[p as usize];
        if r != u32::MAX {
            area[r as usize] += area[p as usize];
        }
    }
    let riv: Vec<bool> = (0..n).map(|p| f64::from(area[p]) >= a_min && z[p] > sea).collect();
    let broken = (0..n)
        .filter(|&p| riv[p] && recv[p] != u32::MAX && !riv[recv[p] as usize] && !outlet[recv[p] as usize] && z[recv[p] as usize] > sea)
        .count();
    let fell = (0..n).filter(|&p| recv[p] != u32::MAX && area[recv[p] as usize] < area[p]).count();
    println!("KG l_rivers check: {broken} river cells whose receiver is land, not an outlet and not a river cell; {fell} cells whose receiver holds less area");
    riv
}

#[allow(clippy::type_complexity)]
// ================================================================ SPIKE-135 (lead #135): the repair-debt ladder

const ROUTE_EPS: f64 = 1.0e-3;
const BASIN_EPS: f64 = 1.0e-7;
const WET: f64 = 1.0e-3;
/// A breach or fill counts on a cell from this depth (m); the defect presence for the attribution from `PRESENT`.
const COUNT_M: f64 = 0.01;
const PRESENT: f64 = 1.0;

#[derive(PartialEq, PartialOrd)]
struct BKey(f64);
impl Eq for BKey {}
impl Ord for BKey {
    fn cmp(&self, o: &BKey) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}
const NB8: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
const NB4: [(i64, i64); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// SPIKE-116's complete breach over a neighbourhood (drain_base.rs, spike/e3-def): priority flood keyed by height without raising; `floor` keeps land above it.
fn breach_nb(w: usize, h: usize, z: &mut [f64], outlet: &[bool], eps: f64, nbs: &[(i64, i64)], floor: f64) -> Vec<f64> {
    let z0 = z.to_vec();
    let n = w * h;
    let mut parent = vec![NONE; n];
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    for c in 0..n {
        if outlet[c] {
            done[c] = true;
            heap.push(std::cmp::Reverse((BKey(z[c]), c)));
        }
    }
    while let Some(std::cmp::Reverse((_, c))) = heap.pop() {
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        for &(dx, dy) in nbs {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let q = ny as usize * w + nx as usize;
            if done[q] {
                continue;
            }
            done[q] = true;
            parent[q] = c as u32;
            if z[q] <= z[c] {
                let mut lvl = z[q] - eps;
                let mut a = c;
                loop {
                    if outlet[a] || z[a] < lvl || (z0[a] > floor && z[a] <= floor) {
                        break;
                    }
                    z[a] = if z0[a] > floor { lvl.max(floor) } else { lvl };
                    lvl -= eps;
                    if parent[a] == NONE {
                        break;
                    }
                    a = parent[a] as usize;
                }
            }
            heap.push(std::cmp::Reverse((BKey(z[q]), q)));
        }
    }
    z.iter().zip(&z0).map(|(a, b)| b - a).collect()
}

fn par_rows<T: Send + Copy>(out: &mut [T], w: usize, threads: usize, f: &(dyn Fn(usize) -> T + Sync)) {
    let rows = (out.len() / w).div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    *v = f(k * rows * w + c);
                }
            });
        }
    });
}

fn q_of(v: &mut [f64], f: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let k = ((v.len() as f64 * f) as usize).min(v.len() - 1);
    *v.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1
}

/// One rung's fields kept for the attribution.
struct Kept {
    breach: Vec<f32>,
    ch75: Vec<bool>,
}

/// Routes `z` on `g` with one rule for every rung (outlets: the grid's edge and every cell at or below the sea; the canonical priority flood with a 1e-3 m grade; D8 steepest descent on
/// it, ties by the lower cell id; D8 drainage area), finds the closed depressions (a 1e-7 m flood standing more than 1 mm above a cell; 8-connected components), the complete breach along
/// the routing (each cell lowered to the lowest height upstream of it: the least lowering that makes every flow path non-increasing on this routing) and the fill, over the land cells
/// `inside` admits. Returns the CSV row and, with `keep`, the breach and the 75 km2 channels.
#[allow(clippy::too_many_arguments)]
fn rung(
    window: &str,
    name: &str,
    g: &Grid,
    z: &[f64],
    inside: &(dyn Fn(usize) -> bool + Sync),
    sea_seed: &(dyn Fn(usize) -> bool + Sync),
    keep: bool,
    threads: usize,
    extra: &str,
) -> (String, Option<Kept>) {
    let t = Instant::now();
    let n = g.len();
    let w = g.w();
    let hgt = n / w;
    // the outlets (G0 contracts reading 12): the grid's edge and the sea, the cells at or below 0 m connected (8 neighbours, through such cells) to the edge or to a sea
    // cell of K5's solve (`sea_seed`); inland cells at or below 0 m are land
    let mut outlet: Vec<bool> = (0..n).map(|p| g.is_edge(p)).collect();
    let mut stack: Vec<usize> = (0..n).filter(|&p| z[p] <= 0.0 && (g.is_edge(p) || sea_seed(p))).collect();
    for &p in &stack {
        outlet[p] = true;
    }
    while let Some(c) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = g.neighbour(c, d) {
                if !outlet[q] && z[q] <= 0.0 {
                    outlet[q] = true;
                    stack.push(q);
                }
            }
        }
    }
    let inland = (0..n).filter(|&p| inside(p) && !outlet[p] && z[p] <= 0.0).count();
    let filled = priority_flood_with(g, z, &outlet, ROUTE_EPS, true);
    let mut rd = vec![u8::MAX; n];
    par_rows(&mut rd, w, threads, &|p| {
        if outlet[p] {
            return u8::MAX;
        }
        let (mut best, mut bd, mut bq) = (0.0f64, u8::MAX, usize::MAX);
        for d in 0..8 {
            if let Some(q) = g.neighbour(p, d) {
                let s = (filled[q] - filled[p]) / g.distance_at(p, d);
                if s < best || (s == best && bd != u8::MAX && q < bq) {
                    (best, bd, bq) = (s, d as u8, q);
                }
            }
        }
        bd
    });
    drop(filled);
    let recv: Vec<u32> =
        (0..n).map(|p| if rd[p] == u8::MAX { NONE } else { g.neighbour(p, rd[p] as usize).expect("a neighbour") as u32 }).collect();
    let stuck = (0..n).filter(|&p| !outlet[p] && recv[p] == NONE).count();
    let order = topological_order(&recv);
    let mut area: Vec<f64> = (0..n).map(|p| g.cell_area(p)).collect();
    let mut lvl = z.to_vec();
    for &c in order.iter().rev() {
        let c = c as usize;
        let r = recv[c];
        if r != NONE {
            area[r as usize] += area[c];
            if lvl[c] < lvl[r as usize] {
                lvl[r as usize] = lvl[c];
            }
        }
    }
    drop(order);
    let breach: Vec<f64> = (0..n).map(|p| z[p] - lvl[p]).collect();
    drop(lvl);
    let fb = priority_flood_with(g, z, &outlet, BASIN_EPS, false);
    let depth: Vec<f64> = (0..n).map(|p| if outlet[p] { 0.0 } else { fb[p] - z[p] }).collect();
    drop(fb);
    // the depressions: 8-connected components of wet cells, their depth (largest fill) and size
    let mut label = vec![u32::MAX; n];
    let mut comps: Vec<(f64, usize, bool)> = Vec::new();
    for s in 0..n {
        if depth[s] <= WET || label[s] != u32::MAX {
            continue;
        }
        let id = comps.len() as u32;
        let (mut dmax, mut k, mut ins) = (0.0f64, 0usize, false);
        label[s] = id;
        stack.push(s);
        while let Some(c) = stack.pop() {
            dmax = dmax.max(depth[c]);
            k += 1;
            ins |= inside(c);
            for d in 0..8 {
                if let Some(q) = g.neighbour(c, d) {
                    if label[q] == u32::MAX && depth[q] > WET {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
        comps.push((dmax, k, ins));
    }
    let land = |p: usize| inside(p) && !outlet[p];
    let step = |p: usize| if rd[p] == u8::MAX { g.dx() } else { g.distance_at(p, rd[p] as usize) };
    let ridge = |p: usize| {
        let (x, y) = ((p % w) as i64, (p / w) as i64);
        [(1i64, 0i64), (0, 1), (1, 1), (1, -1)].iter().any(|&(dx, dy)| {
            let (a, b) = ((x - 2 * dx, y - 2 * dy), (x + 2 * dx, y + 2 * dy));
            let ok = |q: (i64, i64)| q.0 >= 0 && q.1 >= 0 && q.0 < w as i64 && q.1 < hgt as i64;
            ok(a) && ok(b) && z[p] >= z[a.1 as usize * w + a.0 as usize] + 2.0 && z[p] >= z[b.1 as usize * w + b.0 as usize] + 2.0
        })
    };
    let (mut land_n, mut l75, mut l75_pit, mut l75_pit5, mut l1, mut l1_pit, mut l1_pit5) = (0usize, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut fill, mut fill_vol, mut br, mut br_vol, mut rbr) = (Vec::new(), 0.0, Vec::new(), 0.0, Vec::new());
    let (mut ridge1, mut ridge10, mut ridge_n) = (0usize, 0usize, 0usize);
    for p in 0..n {
        if !land(p) {
            continue;
        }
        land_n += 1;
        let pit = label[p] != u32::MAX;
        let pit5 = pit && comps[label[p] as usize].0 > 5.0;
        let ln = step(p);
        if area[p] >= 75.0e6 {
            l75 += ln;
            l75_pit += if pit { ln } else { 0.0 };
            l75_pit5 += if pit5 { ln } else { 0.0 };
            if breach[p] > COUNT_M {
                rbr.push(breach[p]);
            }
        }
        if area[p] >= 1.0e6 {
            l1 += ln;
            l1_pit += if pit { ln } else { 0.0 };
            l1_pit5 += if pit5 { ln } else { 0.0 };
        }
        if depth[p] > WET {
            fill.push(depth[p]);
            fill_vol += depth[p] * g.cell_area(p);
        }
        if breach[p] > COUNT_M {
            br.push(breach[p]);
            br_vol += breach[p] * g.cell_area(p);
        }
        if ridge(p) {
            ridge_n += 1;
            ridge1 += usize::from(breach[p] > 1.0);
            ridge10 += usize::from(breach[p] > 10.0);
        }
    }
    let over = |v: &[f64], m: f64| v.iter().filter(|&&x| x > m).count();
    let pits_in: Vec<&(f64, usize, bool)> = comps.iter().filter(|c| c.2).collect();
    let pits5 = pits_in.iter().filter(|c| c.0 > 5.0).count();
    let gate_basins = pits_in.iter().filter(|c| c.0 >= 0.5 && c.1 >= 16).count();
    let pct = |a: f64, b: f64| 100.0 * a / b.max(1e-9);
    let (f10, f50, b10, b50, r10, r50) =
        (over(&fill, 10.0), over(&fill, 50.0), over(&br, 10.0), over(&br, 50.0), over(&rbr, 10.0), over(&rbr, 50.0));
    let (nf, nb, nr) = (fill.len(), br.len(), rbr.len());
    let row = format!(
        "{window},{name},{:.1},{land_n},{:.1},{:.2},{:.2},{:.1},{:.2},{:.2},{},{pits5},{gate_basins},{nf},{:.2},{:.2},{:.1},{f10},{f50},{:.4},{nb},{:.2},{:.2},{:.1},{b10},{b50},{:.4},{nr},{:.2},{:.1},{r10},{r50},{ridge_n},{ridge1},{ridge10},{extra},{inland},{stuck},{:.0}",
        g.dx(),
        l75 / 1000.0,
        pct(l75_pit, l75),
        pct(l75_pit5, l75),
        l1 / 1000.0,
        pct(l1_pit, l1),
        pct(l1_pit5, l1),
        pits_in.len(),
        q_of(&mut fill, 0.5),
        q_of(&mut fill, 0.95),
        fill.iter().copied().fold(0.0, f64::max),
        fill_vol / 1.0e9,
        q_of(&mut br, 0.5),
        q_of(&mut br, 0.95),
        br.iter().copied().fold(0.0, f64::max),
        br_vol / 1.0e9,
        q_of(&mut rbr, 0.95),
        rbr.iter().copied().fold(0.0, f64::max),
        t.elapsed().as_secs_f64()
    );
    println!(
        "RD {window} {name} ({:.1} m): river 75 km2 {:.0} km, on closed depressions {:.2} % (over 5 m deep {:.2} %); 1 km2 {:.0} km, {:.2} % ({:.2} %); depressions {} (over 5 m {pits5}, gate basins {gate_basins}); fill {nf} cells ({:.2} % of land), over 10 m {f10}, over 50 m {f50}, {:.3} km3; breach {nb} cells ({:.2} %), over 10 m {b10} ({:.3} %), over 50 m {b50} ({:.3} %), {:.3} km3; on 75 km2 rivers {nr} cells, over 10 m {r10}, over 50 m {r50}; ridge cells {ridge_n}, breached over 1 m {ridge1}, over 10 m {ridge10}; inland land at or below 0 m {inland}; {stuck} stuck; {:.0} s",
        g.dx(),
        l75 / 1000.0,
        pct(l75_pit, l75),
        pct(l75_pit5, l75),
        l1 / 1000.0,
        pct(l1_pit, l1),
        pct(l1_pit5, l1),
        pits_in.len(),
        pct(nf as f64, land_n as f64),
        fill_vol / 1.0e9,
        pct(nb as f64, land_n as f64),
        pct(b10 as f64, land_n as f64),
        pct(b50 as f64, land_n as f64),
        br_vol / 1.0e9,
        t.elapsed().as_secs_f64()
    );
    let kept = keep
        .then(|| Kept { breach: breach.iter().map(|&v| v as f32).collect(), ch75: (0..n).map(|p| area[p] >= 75.0e6 && land(p)).collect() });
    (row, kept)
}

/// Cells within `r` cells (Chebyshev) of a cell where `f` holds (separable max).
fn dilate(f: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut a = vec![false; w * h];
    for y in 0..h {
        let mut last: i64 = -1_000_000;
        for x in 0..w {
            if f[y * w + x] {
                last = x as i64;
            }
            a[y * w + x] = x as i64 - last <= r as i64;
        }
        let mut next: i64 = 1_000_000;
        for x in (0..w).rev() {
            if f[y * w + x] {
                next = x as i64;
            }
            a[y * w + x] |= next - x as i64 <= r as i64;
        }
    }
    let mut b = vec![false; w * h];
    for x in 0..w {
        let mut last: i64 = -1_000_000;
        for y in 0..h {
            if a[y * w + x] {
                last = y as i64;
            }
            b[y * w + x] = y as i64 - last <= r as i64;
        }
        let mut next: i64 = 1_000_000;
        for y in (0..h).rev() {
            if a[y * w + x] {
                next = y as i64;
            }
            b[y * w + x] |= next - y as i64 <= r as i64;
        }
    }
    b
}

fn read_f32(path: &str, n: usize) -> Result<Vec<f32>, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() != n * 4 {
        return Err(format!("{path}: {} bytes, expected {}", b.len(), n * 4));
    }
    Ok(b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

fn read_f64(path: &str, n: usize) -> Result<Vec<f64>, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if b.len() != n * 8 {
        return Err(format!("{path}: {} bytes, expected {}", b.len(), n * 8));
    }
    Ok(b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().expect("8 bytes"))).collect())
}

const HEADER: &str = "window,rung,cell_m,land_cells,riv75_km,riv75_pit_pct,riv75_pit5_pct,riv1_km,riv1_pit_pct,riv1_pit5_pct,depressions,depr5,gate_basins,fill_cells,fill_p50_m,fill_p95_m,fill_max_m,fill_over10,fill_over50,fill_km3,breach_cells,breach_p50_m,breach_p95_m,breach_max_m,breach_over10,breach_over50,breach_km3,riv_breach_cells,riv_breach_p95_m,riv_breach_max_m,riv_breach_over10,riv_breach_over50,ridge_cells,ridge_breach1,ridge_breach10,d2_measured,d2_failed,inland_cells,stuck,seconds";

// ================================================================ SPIKE-136 (lead #136): the hand-over at the source

/// The relief kept (SPIKE-116): the std of the 200 m-1 km band (box means over 3 and 11 cells) and the 1 km block range (10 x 10 cells) p50 / p95, over land of the box.
fn relief_stats(z: &[f64], w: usize, h: usize, inside: &dyn Fn(usize) -> bool) -> (f64, f64, f64) {
    // integral image
    let mut ii = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0.0;
        for x in 0..w {
            row += z[y * w + x];
            ii[(y + 1) * (w + 1) + x + 1] = ii[y * (w + 1) + x + 1] + row;
        }
    }
    let boxm = |x: usize, y: usize, r: usize| {
        let (x0, y0, x1, y1) = (x.saturating_sub(r), y.saturating_sub(r), (x + r + 1).min(w), (y + r + 1).min(h));
        (ii[y1 * (w + 1) + x1] - ii[y0 * (w + 1) + x1] - ii[y1 * (w + 1) + x0] + ii[y0 * (w + 1) + x0]) / ((x1 - x0) * (y1 - y0)) as f64
    };
    let (mut s, mut s2, mut n) = (0.0, 0.0, 0usize);
    for p in 0..w * h {
        if !inside(p) || z[p] <= 0.0 {
            continue;
        }
        let (x, y) = (p % w, p / w);
        let b = boxm(x, y, 1) - boxm(x, y, 5);
        s += b;
        s2 += b * b;
        n += 1;
    }
    let mean = s / n.max(1) as f64;
    let std = (s2 / n.max(1) as f64 - mean * mean).max(0.0).sqrt();
    let mut r: Vec<f64> = Vec::new();
    for by in (0..h - 9).step_by(10) {
        for bx in (0..w - 9).step_by(10) {
            let (mut lo, mut hi, mut all) = (f64::INFINITY, f64::NEG_INFINITY, true);
            for y in by..by + 10 {
                for x in bx..bx + 10 {
                    let c = y * w + x;
                    all &= inside(c);
                    lo = lo.min(z[c]);
                    hi = hi.max(z[c]);
                }
            }
            if all && lo > 0.0 {
                r.push(hi - lo);
            }
        }
    }
    (std, q_of(&mut r, 0.5), q_of(&mut r, 0.95))
}

/// G0 contracts section 9's ridge cells of `z` (at least 2 m above both cells 2 out along one of 4 lines).
fn ridges(z: &[f64], w: usize, h: usize, inside: &(dyn Fn(usize) -> bool + Sync), threads: usize) -> Vec<bool> {
    let mut out = vec![false; w * h];
    par_rows(&mut out, w, threads, &|p| {
        if !inside(p) || z[p] <= 0.0 {
            return false;
        }
        let (x, y) = ((p % w) as i64, (p / w) as i64);
        [(1i64, 0i64), (0, 1), (1, 1), (1, -1)].iter().any(|&(dx, dy)| {
            let (a, b) = ((x - 2 * dx, y - 2 * dy), (x + 2 * dx, y + 2 * dy));
            let ok = |q: (i64, i64)| q.0 >= 0 && q.1 >= 0 && q.0 < w as i64 && q.1 < h as i64;
            ok(a) && ok(b) && z[p] >= z[a.1 as usize * w + a.0 as usize] + 2.0 && z[p] >= z[b.1 as usize * w + b.0 as usize] + 2.0
        })
    });
    out
}

const HEADER2: &str = "window,surface,band_std_m,relief1km_p50_m,relief1km_p95_m,band_std_kept_pct,p50_kept_pct,p95_kept_pct,lowered_vs_ref_over10,lowered_vs_ref_over50,s0_ridges_lowered10,s0_ridges_lowered50,raised_vs_ref_over10,breach_cells,breach_km3,on_none_cells,on_none_km3,on_base_km3,on_k5_km3,on_interp_km3,on_band_km3,fixed_base_pct,fixed_k5_pct,fixed_interp_pct,fixed_band_pct,fixed_all_pct";

fn main() -> Result<(), String> {
    let t_all = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let win: Vec<usize> = value(&args, "--window")
        .ok_or("--window face,i0,i1,j0,j1,seed")?
        .split(',')
        .map(|x| x.parse::<usize>().map_err(|_| "--window numbers".to_string()))
        .collect::<Result<_, _>>()?;
    let margin: usize = value(&args, "--margin").map_or(Ok(200), str::parse).map_err(|_| "--margin")?;
    let threads: usize = value(&args, "--threads").map_or(Ok(8), str::parse).map_err(|_| "--threads")?;
    let out_dir = PathBuf::from(value(&args, "--out").unwrap_or("target/ho"));
    let tag = value(&args, "--tag").unwrap_or("w").to_string();
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let (face, seed) = (Face(win[0] as u8), win[5] as u64);
    let region = (win[1], win[2], win[3], win[4]);
    let def = WorldDefinition::single_planet(R, seed);
    let inputs = class_inputs(&def, seed, threads);
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f64; nw * nh];
    par_fill(&mut nodes, nw, threads, &|p| inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)));
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes.iter().map(|&v| v as f32).collect() };
    drop(nodes);
    let base = |d: Vec3| -> f64 { f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle")) };
    let orography = inputs.orography();
    let climate = |d: Vec3| {
        let (pm, tc) = (orography.precipitation_mm(d), orography.temperature_c(d));
        let ro = runoff_mm(pm, tc);
        (ro / 1000.0 / SECONDS_PER_YEAR, (pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR, discharge_weight(ro))
    };
    let spec = RegionSpec {
        seed,
        face,
        radius_m: R,
        regional: REGIONAL,
        region,
        margin,
        tile: 768,
        halo: 64,
        rounds: 4,
        threads,
        res: LATTICE as u32,
        base_id: 0x6c61_7474_6963_6531 ^ (LATTICE as u64),
        inputs_id: class_inputs_key(&def, seed),
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    };
    let front = region_front(&spec, &base, &climate)?;
    let solve = &front.solve;
    let (sri0, srj0, rw, rh) = (solve.ri0, solve.rj0, solve.rw, solve.rh);
    let (x0, y0, w, h) = ((sri0 * L) as i64, (srj0 * L) as i64, rw * L, rh * L);
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
    let grid = &solve.grid;
    let outlet = &solve.outlet;
    let cells = rw * rh;
    let sea = 0.0;
    let nl = w * h;
    println!("HO {tag}: window {:?}, {rw} x {rh} cells at 814 m, {w} x {h} at L; setup {:.0} s", win, t_all.elapsed().as_secs_f64());
    let mut csv = format!("{HEADER}\n");
    let mut csv2 = format!("{HEADER2}\n");
    let flush = |csv: &str, csv2: &str| -> Result<(), String> {
        std::fs::write(out_dir.join(format!("{tag}-ladder.csv")), csv).map_err(|e| e.to_string())?;
        std::fs::write(out_dir.join(format!("{tag}-handover.csv")), csv2).map_err(|e| e.to_string())
    };
    let in_core_h1 = |c: usize| {
        let (i, j) = (sri0 + c % rw, srj0 + c / rw);
        i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3
    };
    let (bx0, bx1, by0, by1) = ((region.0 - sri0) * L, (region.1 + 1 - sri0) * L, (region.2 - srj0) * L, (region.3 + 1 - srj0) * L);
    let in_core_l = |p: usize| {
        let (x, y) = (p % w, p / w);
        x >= bx0 && x < bx1 && y >= by0 && y < by1
    };
    let to_h1 = |p: usize| (p / w / L) * rw + (p % w) / L;
    let sea_h1 = |c: usize| outlet[c] && !grid.is_edge(c) && solve.zs[c] <= 0.0;
    let sea_l = |p: usize| sea_h1(to_h1(p));

    // ---- L0, L1 at 814 m (for S0's classes)
    let (row, k0) = rung(&tag, "L0 z_b 814 m", grid, &solve.zb, &in_core_h1, &sea_h1, true, threads, "-,-");
    csv += &format!("{row}\n");
    let (row, k1) = rung(&tag, "L1 z_s 814 m", grid, &solve.zs, &in_core_h1, &sea_h1, true, threads, "-,-");
    csv += &format!("{row}\n");
    let (k0, k1) = (k0.expect("kept"), k1.expect("kept"));

    // ---- today's H1 (K6): the trunk lines for the D-2 count
    let dir_h1 = |c: usize| solve.face_spec.halo_direction((sri0 + c % rw) as i64, (srj0 + c / rw) as i64);
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; cells], vec![0.0f32; cells], vec![0.0f32; cells]);
    for c in (0..cells).filter(|&c| solve.zs[c] > 0.0) {
        let (ro, ev, wt) = climate(dir_h1(c));
        runoff[c] = ro as f32;
        evap[c] = ev as f32;
        weight[c] = wt as f32;
    }
    let eps = solve.precision.epsilon_w;
    let h1 = {
        let r0 = route_rules(grid, &solve.zs, outlet, eps, solve.stream, 0.0, Some(&weight), RoundRules::ROUND);
        let f0 = priority_flood_with(grid, &solve.zs, outlet, eps, true);
        h1_from(grid, &solve.zs, &f0, r0, outlet, &runoff, &evap)
    };
    let to_local = |c: u32| {
        let c = c as usize;
        ((L * (c % rw)) as f64 + (L as f64 - 1.0) / 2.0, (L * (c / rw)) as f64 + (L as f64 - 1.0) / 2.0)
    };
    let lines: Vec<Vec<(f64, f64)>> = h1.trunk_cells.iter().map(|cs| cs.iter().map(|&c| to_local(c)).collect()).collect();
    let mouth: Vec<bool> = h1.trunk_seg.iter().map(|&k| h1.hydro.segments[k].downstream == NONE).collect();
    drop(h1);
    let in_box: Vec<bool> = (0..nl).map(in_core_l).collect();

    // ---- S0 and r at L
    let mut bl = vec![0f64; nl];
    par_fill(&mut bl, w, threads, &|p| base(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    let zb_field = RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, solve.zb.iter().map(|&v| v as f32).collect());
    let zs_field = RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, solve.zs.iter().map(|&v| v as f32).collect());
    let mut rl = vec![0f64; nl];
    par_fill(&mut rl, w, threads, &|p| bl[p] - zb_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    let mut start = vec![0f64; nl];
    par_fill(&mut start, w, threads, &|p| bl[p] + front.regional_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    drop(bl);
    let bar = {
        let mut r: Vec<f64> = Vec::new();
        for by in (0..h - 9).step_by(10) {
            for bx in (0..w - 9).step_by(10) {
                let (mut lo, mut hi, mut all) = (f64::INFINITY, f64::NEG_INFINITY, true);
                for y in by..by + 10 {
                    for x in bx..bx + 10 {
                        let c = y * w + x;
                        all &= in_box[c];
                        lo = lo.min(start[c]);
                        hi = hi.max(start[c]);
                    }
                }
                if all && lo > sea {
                    r.push(hi - lo);
                }
            }
        }
        r.sort_by(f64::total_cmp);
        corridor_bar_m(r[r.len() / 2])
    };
    let lg = Grid::new(w, h, grid.dx() / L as f64);
    let d2 = |z: &[f64]| -> String {
        let t = Instant::now();
        let water: Vec<f64> = z.iter().map(|&v| v.max(sea)).collect();
        let m = Measure { w, h, water: &water, inside: &in_box, region: &in_box, sea, radius: K_C * L as f64, bar, ocean: None };
        let r = measure_all(&m, &lines, &mouth, threads);
        let measured = r.iter().flatten().count();
        let failed = r.iter().flatten().filter(|x| x.failed).count();
        println!("HO {tag} D-2 (k_c 2 snapped, land-only snap, bar {bar:.1} m): {failed} of {measured} H1 trunk reaches fail; {:.0} s", t.elapsed().as_secs_f64());
        format!("{measured},{failed}")
    };

    // ---- L2s and L4 (S0) for the classes
    let k2 = {
        let mut z = vec![0f64; nl];
        par_fill(&mut z, w, threads, &|p| zs_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
        let (row, k) = rung(&tag, "L2s C1(z_s) at L", &lg, &z, &in_core_l, &sea_l, true, threads, "-,-");
        csv += &format!("{row}\n");
        k.expect("kept")
    };
    let k4 = {
        let e = d2(&start);
        let (row, k) = rung(&tag, "L4 S0 = C1(z_s) + r", &lg, &start, &in_core_l, &sea_l, true, threads, &e);
        csv += &format!("{row}\n");
        k.expect("kept")
    };
    flush(&csv, &csv2)?;
    // S0's classes (SPIKE-135): 1 base, 2 K5, 3 interpolation, 4 band; 0 no S0 breach over PRESENT m
    let class: Vec<u8> = {
        let pres814 = |b: &[f32]| -> Vec<bool> { dilate(&b.iter().map(|&v| v > PRESENT as f32).collect::<Vec<_>>(), rw, rh, 1) };
        let (p0, p1) = (pres814(&k0.breach), pres814(&k1.breach));
        let p2 = dilate(&k2.breach.iter().map(|&v| v > PRESENT as f32).collect::<Vec<_>>(), w, h, L / 2);
        (0..nl)
            .map(|p| {
                if !in_box[p] || start[p] <= sea || f64::from(k4.breach[p]) <= PRESENT {
                    return 0;
                }
                let c = to_h1(p);
                if p1[c] && p0[c] {
                    1
                } else if p1[c] {
                    2
                } else if p2[p] {
                    3
                } else {
                    4
                }
            })
            .collect()
    };
    drop(k2);
    let (s0_std, s0_p50, s0_p95) = relief_stats(&start, w, h, &in_core_l);
    let s0_ridge = ridges(&start, w, h, &in_core_l, threads);
    println!(
        "HO {tag} S0: band std {s0_std:.2} m, 1 km block range p50 {s0_p50:.1} p95 {s0_p95:.1} m; ridge cells {}; S0 breach cells over {PRESENT} m by class: {:?}",
        s0_ridge.iter().filter(|&&b| b).count(),
        (0..5).map(|k| class.iter().filter(|&&c| c == k).count()).collect::<Vec<_>>()
    );
    csv2 += &format!("{tag},S0,{s0_std:.3},{s0_p50:.2},{s0_p95:.2},100,100,100,0,0,0,0,0,-,-,-,-,-,-,-,-,-,-,-,-,-\n");

    // ---- spikes #138: h2 (the triangulation), h3 (the skeleton: C1 lowered along each D8 diagonal step), h4 (Clough-Tocher on h2's split), from the kh module
    use planet_generators::bake::kh;
    let mut ol: Vec<bool> = (0..cells).map(|c| grid.is_edge(c)).collect();
    let mut stack: Vec<usize> = (0..cells).filter(|&c| solve.zs[c] <= 0.0 && (grid.is_edge(c) || outlet[c])).collect();
    for &c in &stack {
        ol[c] = true;
    }
    while let Some(c) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = grid.neighbour(c, d) {
                if !ol[q] && solve.zs[q] <= 0.0 {
                    ol[q] = true;
                    stack.push(q);
                }
            }
        }
    }
    let (recv, area) = kh::routing(grid, &solve.zs, &ol);
    let rcv = |c: usize| (recv[c] != NONE).then(|| recv[c] as usize);
    let (split, st) = kh::split(&solve.zs, rw, rh, &rcv, &|c| area[c]);
    println!("HO {tag} h2 quads: split along a D8 diagonal step {} (both diagonals stepped {}), along the higher pair {}", st.by_step, st.both, st.by_pair);
    let grad = kh::gradients(&solve.zs, rw, rh);
    let step = |p: usize, q: usize| recv[p] == q as u32 || recv[q] == p as u32;
    let diag = |q: usize| -> Option<(usize, usize)> {
        let (i, j) = (q % rw, q / rw);
        if i + 1 >= rw || j + 1 >= rh {
            return None;
        }
        let (a, b, c, d) = (q, q + 1, q + rw, q + rw + 1);
        if rcv(a) == Some(d) {
            Some((a, d))
        } else if rcv(d) == Some(a) {
            Some((d, a))
        } else if rcv(b) == Some(c) {
            Some((b, c))
        } else if rcv(c) == Some(b) {
            Some((c, b))
        } else {
            None
        }
    };
    let lr = L as f64;
    let fxy = |c: usize| (((c % w) as f64 - (lr - 1.0) / 2.0) / lr, ((c / w) as f64 - (lr - 1.0) / 2.0) / lr);
    let mut h2z = vec![0f64; nl];
    par_fill(&mut h2z, w, threads, &|c| {
        let (fx, fy) = fxy(c);
        kh::read(&solve.zs, rw, rh, &split, fx, fy)
    });
    let mut c1 = vec![0f64; nl];
    par_fill(&mut c1, w, threads, &|p| zs_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    let mut h3z = vec![0f64; nl];
    par_fill(&mut h3z, w, threads, &|c| {
        let (fx, fy) = fxy(c);
        kh::read_skeleton(&solve.zs, rw, rh, &diag, c1[c], fx, fy, lr, 1.0, 2.0)
    });
    drop(c1);
    let mut h4z = vec![0f64; nl];
    par_fill(&mut h4z, w, threads, &|c| {
        let (fx, fy) = fxy(c);
        kh::read_ct(&solve.zs, rw, rh, &split, &grad, &step, fx, fy).0
    });
    // h4's overshoot beyond the corners of its macro triangle, over land of the box
    {
        let (mut n_land, mut over, mut under, mut omax, mut umax) = (0usize, 0usize, 0usize, 0f64, 0f64);
        for p in (0..nl).filter(|&p| in_core_l(p) && start[p] > 0.0) {
            n_land += 1;
            let (fx, fy) = fxy(p);
            let (v, lo, hi) = kh::read_ct(&solve.zs, rw, rh, &split, &grad, &step, fx, fy);
            if v > hi + 0.01 {
                over += 1;
                omax = omax.max(v - hi);
            }
            if v < lo - 0.01 {
                under += 1;
                umax = umax.max(lo - v);
            }
        }
        // h4 along every D8 step of the box's land cells: 32 samples from the cell centre to its receiver's, any rise over 1 mm
        let (mut steps, mut rising, mut rmax) = (0usize, 0usize, 0f64);
        for p in 0..cells {
            let Some(q) = rcv(p) else { continue };
            let (i, j) = (sri0 + p % rw, srj0 + p / rw);
            if i < region.0 || i > region.1 || j < region.2 || j > region.3 || solve.zs[p] <= 0.0 {
                continue;
            }
            let (ax, ay, bx, by) = ((p % rw) as f64, (p / rw) as f64, (q % rw) as f64, (q / rw) as f64);
            steps += 1;
            let (mut prev, mut worst) = (f64::INFINITY, 0f64);
            for k in 0..=32 {
                let t = k as f64 / 32.0;
                let v = kh::read_ct(&solve.zs, rw, rh, &split, &grad, &step, ax + t * (bx - ax), ay + t * (by - ay)).0;
                worst = worst.max(v - prev);
                prev = v;
            }
            if worst > 1e-3 {
                rising += 1;
                rmax = rmax.max(worst);
            }
        }
        println!(
            "HO {tag} h4 overshoot over {n_land} land cells of the box: above the macro triangle's corners by over 1 cm {over} ({:.3} %, max {omax:.2} m), below {under} ({:.3} %, max {umax:.2} m); D8 steps of the box's land {steps}: rising over 1 mm along the step {rising} (max rise {rmax:.3} m)",
            100.0 * over as f64 / n_land.max(1) as f64,
            100.0 * under as f64 / n_land.max(1) as f64
        );
        csv += &format!("#h4,{tag},{n_land},{over},{omax:.3},{under},{umax:.3},{steps},{rising},{rmax:.4}\n");
    }
    // the render crop (#137's: the box's 512 x 512 block of the largest p50 1 km relief), every L cell: S0 and the three hand-overs with the band
    let crop_at: Option<(usize, usize, usize)> = value(&args, "--crop-at").map(|v| {
        let x: Vec<usize> = v.split(',').map(|t| t.parse().expect("--crop-at cx,cy,crop")).collect();
        (x[0], x[1], x[2])
    });
    if let Some((cx, cy, cr)) = crop_at {
        for (name, f) in [("s0", &start), ("h2r", &h2z), ("h3r", &h3z), ("h4r", &h4z)] {
            let band = name != "s0";
            let mut b: Vec<u8> = Vec::with_capacity(cr * cr * 4);
            for y in cy..cy + cr {
                for x in cx..cx + cr {
                    let p = y * w + x;
                    b.extend_from_slice(&((f[p] + if band { rl[p] } else { 0.0 }) as f32).to_le_bytes());
                }
            }
            std::fs::write(out_dir.join(format!("{tag}-crop-{name}-{cr}.f32")), b).map_err(|e| e.to_string())?;
        }
    }

    // ---- the hand-over surfaces (spikes #138)
    let near_s0 = dilate(&k4.breach.iter().map(|&v| v > PRESENT as f32).collect::<Vec<_>>(), w, h, L / 2);
    let ca = lg.dx() * lg.dx() / 1.0e9;
    for (name, key) in [("h2 + r", "h2r"), ("h3 skeleton", "h3"), ("h3 + r", "h3r"), ("h4 Clough-Tocher", "h4"), ("h4 + r", "h4r")] {
        let src = match &key[..2] {
            "h2" => &h2z,
            "h3" => &h3z,
            _ => &h4z,
        };
        let z: Vec<f64> = if key.ends_with('r') { (0..nl).map(|p| src[p] + rl[p]).collect() } else { src.clone() };
        let e = d2(&z);
        let (row, k) = rung(&tag, name, &lg, &z, &in_core_l, &sea_l, true, threads, &e);
        csv += &format!("{row}\n");
        let k = k.expect("kept");
        let (sd, p50, p95) = relief_stats(&z, w, h, &in_core_l);
        // the reference: S0 for a surface with the band, C1(z_s) = S0 - r without it (so the lowering is the hand-over's own)
        let with_band = key.ends_with('r');
        let reff = |p: usize| if with_band { start[p] } else { start[p] - rl[p] };
        let (mut l10, mut l50, mut r10, mut r50, mut up10) = (0usize, 0usize, 0usize, 0usize, 0usize);
        for p in 0..nl {
            if !in_box[p] || start[p] <= sea {
                continue;
            }
            let dz = reff(p) - z[p];
            up10 += usize::from(dz < -10.0);
            l10 += usize::from(dz > 10.0);
            l50 += usize::from(dz > 50.0);
            r10 += usize::from(s0_ridge[p] && dz > 10.0);
            r50 += usize::from(s0_ridge[p] && dz > 50.0);
        }
        // the surface's breach cells (over PRESENT m) by S0's class at the cell; S0's class cells with no breach of the surface within 4 L cells
        let near_x = dilate(&k.breach.iter().map(|&v| v > PRESENT as f32).collect::<Vec<_>>(), w, h, L / 2);
        let (mut vol, mut cnt) = ([0f64; 5], [0usize; 5]);
        let (mut cls_n, mut fixed) = ([0usize; 5], [0usize; 5]);
        for p in 0..nl {
            if !in_box[p] || start[p] <= sea {
                continue;
            }
            let c = class[p] as usize;
            if c > 0 {
                cls_n[c] += 1;
                fixed[c] += usize::from(!near_x[p]);
            }
            let b = f64::from(k.breach[p]);
            if b > PRESENT {
                vol[c] += b;
                cnt[c] += 1;
            }
        }
        let nb: usize = cnt.iter().sum();
        let vb: f64 = vol.iter().sum();
        let new_far = (0..nl).filter(|&p| in_box[p] && start[p] > sea && f64::from(k.breach[p]) > PRESENT && !near_s0[p]).count();
        let pc = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
        let fixed_all = pc(fixed[1..].iter().sum(), cls_n[1..].iter().sum());
        println!(
            "HO {tag} {name}: relief band std {sd:.2} m ({:.0} % of S0), 1 km p50 {p50:.1} ({:.0} %) p95 {p95:.1} m ({:.0} %); lowered under its reference ({}) over 10 m {l10} cells, over 50 m {l50}; S0 ridge cells lowered over 10 m {r10}, over 50 m {r50}; raised over 10 m {up10}; breach cells over {PRESENT} m {nb}, {:.3} km3: on S0's none {} cells ({:.3} km3; {new_far} not within 4 cells of an S0 breach), base {:.3}, K5 {:.3}, interpolation {:.3}, band {:.3} km3; S0's class cells with none within 4 cells: base {:.0} %, K5 {:.0} %, interpolation {:.0} %, band {:.0} %, all {fixed_all:.0} %",
            if with_band { "S0" } else { "C1(z_s)" },
            100.0 * sd / s0_std,
            100.0 * p50 / s0_p50,
            100.0 * p95 / s0_p95,
            vb * ca,
            cnt[0],
            vol[0] * ca,
            vol[1] * ca,
            vol[2] * ca,
            vol[3] * ca,
            vol[4] * ca,
            pc(fixed[1], cls_n[1]),
            pc(fixed[2], cls_n[2]),
            pc(fixed[3], cls_n[3]),
            pc(fixed[4], cls_n[4])
        );
        csv2 += &format!(
            "{tag},{key},{sd:.3},{p50:.2},{p95:.2},{:.1},{:.1},{:.1},{l10},{l50},{r10},{r50},{up10},{nb},{:.4},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.1},{:.1},{:.1},{:.1},{fixed_all:.1}\n",
            100.0 * sd / s0_std,
            100.0 * p50 / s0_p50,
            100.0 * p95 / s0_p95,
            vb * ca,
            cnt[0],
            vol[0] * ca,
            vol[1] * ca,
            vol[2] * ca,
            vol[3] * ca,
            vol[4] * ca,
            pc(fixed[1], cls_n[1]),
            pc(fixed[2], cls_n[2]),
            pc(fixed[3], cls_n[3]),
            pc(fixed[4], cls_n[4])
        );
        flush(&csv, &csv2)?;
        // the sheet's inputs for the surfaces with the band: per 4 x 4 block the S0 class of the block's deepest breach cell of the surface (5: on a cell S0 does not breach),
        // and the largest lowering under S0
        if key.ends_with('r') {
            let s = 4;
            let (cw, ch) = ((bx1 - bx0) / s, (by1 - by0) / s);
            let mut cls = vec![0u8; cw * ch];
            let mut lowm = vec![0f32; cw * ch];
            for j in 0..ch {
                for i in 0..cw {
                    let (mut best, mut kk, mut lm) = (PRESENT as f32, 0u8, 0f32);
                    for y in 0..s {
                        for x in 0..s {
                            let p = (by0 + j * s + y) * w + bx0 + i * s + x;
                            if k.breach[p] > best {
                                best = k.breach[p];
                                kk = if class[p] == 0 { 5 } else { class[p] };
                            }
                            lm = lm.max((reff(p) - z[p]) as f32);
                        }
                    }
                    cls[j * cw + i] = kk;
                    lowm[j * cw + i] = lm;
                }
            }
            std::fs::write(out_dir.join(format!("{tag}-{key}-class-{cw}x{ch}.u8")), &cls).map_err(|e| e.to_string())?;
            let b: Vec<u8> = lowm.iter().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-{key}-low-{cw}x{ch}.f32")), b).map_err(|e| e.to_string())?;
            if key == "h2r" {
                let mut zz = vec![0f32; cw * ch];
                for j in 0..ch {
                    for i in 0..cw {
                        let mut sum = 0.0;
                        for y in 0..s {
                            for x in 0..s {
                                sum += start[(by0 + j * s + y) * w + bx0 + i * s + x];
                            }
                        }
                        zz[j * cw + i] = (sum / (s * s) as f64) as f32;
                    }
                }
                let b: Vec<u8> = zz.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(out_dir.join(format!("{tag}-s0-{cw}x{ch}.f32")), b).map_err(|e| e.to_string())?;
            }
        }
    }
    flush(&csv, &csv2)?;
    println!("HO {tag} done: {:.0} s", t_all.elapsed().as_secs_f64());
    Ok(())
}
