//! SPIKE-127 (G1a, lead #131; spike branch only, never merged): H1 (K6's flood and routing) re-routed on the node heights and sill edges of S0.
//!
//! `g1a_sills --window face,i0,i1,j0,j1,seed [--margin 200] [--threads 8] [--store <B's commit dir> --no-key-check] [--d2 <SPIKE-126 final log>] [--out <dir>] [--tag <name>]`
//!
//! - S0 = B + C1(z_s - z_b) at L: the river gate's start surface (the E3 start surface).
//! - Node height of an 814 m cell: the lowest L cell of S0 inside it (ties by L index).
//! - Sill of an 8-neighbour edge: the minimax height of S0 over 8-connected L paths between the two cells' lowest L cells, inside the two cells.
//! - K6': K6's canonical flood (Dijkstra on the edge levels `max(sill, W(p) + eps)`), D8 receivers by the steepest jittered slope of the filled level among the
//!   neighbours whose sill the water at the cell's level crosses, the clipped MFD area over the same neighbours, then K6's hydrology and channel graph unchanged
//!   (initiation 75 km2) on the node heights. K5, the outlets and the runoff are today's. No flat regrade (GEN-173): a control run of this code on the point
//!   heights `z_s` (sills `max(z_s)`) measures what that alone changes.
//! - The corridor measure: river_suite's `trunk_corridor` at k_c 2 snapped, with explorer #163's land-only snap applied locally (#163 not merged): non-mouth nodes
//!   and edge crossings snap to land cells only, a route steps only over land, and a route that meets the sea ends there.
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::Commit;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{commit_differences, lay_store_ground, LWindow, WindowIdentity};
use planet_cache::river_suite::corridor_bar_m;
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::channel_graph::{q_value, ChannelGraph};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, Hydro, HydroParams};
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
        Ok(GlobalSample { continent: 0.0, elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)], temperature_c: 0.0, precipitation_mm: 0.0 })
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
                        hc[c] = cur[i][0].0;
                        lc[c] = cur[i][0].1;
                        for (k, &(ox, oy)) in FWD.iter().enumerate() {
                            let (ni, nj) = (i as i64 + ox, j as i64 + oy);
                            if ni < 0 || ni >= rw as i64 || nj >= rh as i64 {
                                continue;
                            }
                            let other = if oy == 0 { &cur[ni as usize] } else { &next[ni as usize] };
                            let mut v = sill_of(&cur[i], other, lw, i, j, (ox, oy));
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
fn route_sills(grid: &Grid, w: &[f64], sill: &dyn Fn(usize, usize, usize) -> f64, outlet: &[bool], seed: u64, weights: &[f32], mfd_open: bool) -> Routing {
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
}

impl Measure<'_> {
    fn land(&self, c: usize) -> bool {
        self.water[c] > self.sea
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
        let (bx0, bx1, by0, by1) =
            (clamp((x0 - radius).floor(), w), clamp((x1 + radius).ceil(), w), clamp((y0 - radius).floor(), h), clamp((y1 + radius).ceil(), h));
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
    fn route(&self, index: &HashMap<usize, usize>, cells: &[usize], from: usize, to: usize, d: f64, start_label: f64) -> Option<(Vec<usize>, f64)> {
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
        let top = cells.iter().map(|&c| self.water[c]).fold(f64::NEG_INFINITY, f64::max) - cells.iter().map(|&c| self.water[c]).fold(f64::INFINITY, f64::min);
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
    (-r..=r).any(|dy| (-r..=r).any(|dx| x + dx >= 0 && x + dx < rw as i64 && y + dy >= 0 && set.contains(&(((y + dy) * rw as i64 + x + dx) as u32))))
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

fn cheb(a: u32, b: u32, rw: usize) -> i64 {
    let (ax, ay, bx, by) = ((a as usize % rw) as i64, (a as usize / rw) as i64, (b as usize % rw) as i64, (b as usize / rw) as i64);
    (ax - bx).abs().max((ay - by).abs())
}

/// The addendum's stop test (lead #131a): for the 10 largest rivers of `a` leaving the box (20 main-stem cells or more in it), whether the water at `a`'s exit cell
/// reaches an outlet of `b` more than 10 cells (8 km) from `a`'s outlet, and the major confluences of the main stem lost or gained (no counterpart within 2 cells)
/// on `b`'s main stem from its trunk cell of the largest Q within 2 cells of the exit.
fn stop_test(a: &H1, b: &H1, in_box: &dyn Fn(u32) -> bool, rw: usize) -> Vec<String> {
    let (ua, ub) = (ups_of(a), ups_of(b));
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
        // b's counterpart: its trunk cell of the largest Q within 2 cells of the exit
        let mut best: Option<(f64, u32)> = None;
        let (ex, ey) = ((exit as usize % rw) as i64, (exit as usize / rw) as i64);
        for dy in -2..=2i64 {
            for dx in -2..=2i64 {
                let c = ((ey + dy) * rw as i64 + ex + dx) as u32;
                if seg_of_b.contains_key(&c) && best.is_none_or(|(q, bc)| b.hydro.qw[c as usize] > q || (b.hydro.qw[c as usize] == q && c < bc)) {
                    best = Some((b.hydro.qw[c as usize], c));
                }
            }
        }
        let conf_b = best.map(|(_, c)| main_stem(b, &ub, seg_of_b[&c], in_box).1).unwrap_or_default();
        let lost: Vec<&(u32, f64)> = conf_a.iter().filter(|x| !conf_b.iter().any(|y| cheb(x.0, y.0, rw) <= 2)).collect();
        let gained: Vec<&(u32, f64)> = conf_b.iter().filter(|y| !conf_a.iter().any(|x| cheb(x.0, y.0, rw) <= 2)).collect();
        let fmt = |v: &[&(u32, f64)]| v.iter().map(|x| format!("{}({:.0})", x.0, x.1)).collect::<Vec<_>>().join("/");
        out.push(format!(
            "exit {exit} Q {:.0}: outlet {} cells off{}; {} major confluences (D8 km2), lost {} [{}], gained {} [{}]{}",
            segs[m].q_down,
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
    let head_cells: Vec<u32> = h1.trunk_seg.iter().filter(|&&k| !ups.contains_key(&k) && in_box(h1.hydro.segments[k].up)).map(|&k| h1.hydro.segments[k].up).collect();
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


// ---------------------------------------------------------------- main

#[allow(clippy::type_complexity)]
fn main() -> Result<(), String> {
    let t_all = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_key_check = args.iter().any(|a| a == "--no-key-check");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--no-key-check").collect();
    let win: Vec<usize> = value(&args, "--window")
        .ok_or("--window face,i0,i1,j0,j1,seed")?
        .split(',')
        .map(|x| x.parse::<usize>().map_err(|_| "--window numbers".to_string()))
        .collect::<Result<_, _>>()?;
    let margin: usize = value(&args, "--margin").map_or(Ok(200), str::parse).map_err(|_| "--margin")?;
    let threads: usize = value(&args, "--threads").map_or(Ok(8), str::parse).map_err(|_| "--threads")?;
    let store = value(&args, "--store").map(Path::new);
    let d2_log = value(&args, "--d2").map(Path::new);
    let out_dir = PathBuf::from(value(&args, "--out").unwrap_or("target/g1a"));
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
    let t = Instant::now();
    let front = region_front(&spec, &base, &climate)?;
    println!("G1A front (K5 + today's K6): {:.1} s", t.elapsed().as_secs_f64());
    let solve = &front.solve;
    let (sri0, srj0, rw, rh) = (solve.ri0, solve.rj0, solve.rw, solve.rh);
    let (x0, y0, w, h) = ((sri0 * L) as i64, (srj0 * L) as i64, rw * L, rh * L);
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
    let t = Instant::now();
    let mut start = vec![0f64; w * h];
    par_fill(&mut start, w, threads, &|p| {
        let d = dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
        base(d) + front.regional_field.at(d)
    });
    let s0_s = t.elapsed().as_secs_f64();
    println!("G1A S0: {w} x {h} L cells in {s0_s:.1} s ({:.3} us a cell, {threads} threads)", 1e6 * s0_s / (w * h) as f64);
    let (bi0, bi1, bj0, bj1) = ((region.0 * L) as i64, ((region.1 + 1) * L) as i64, (region.2 * L) as i64, ((region.3 + 1) * L) as i64);
    let in_box: Vec<bool> = (0..w * h)
        .map(|c| {
            let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
        })
        .collect();
    // the store's ground (B's variant) and its tiles
    let mut stored: Option<(Vec<f64>, Vec<bool>, f64)> = None;
    if let Some(dir) = store {
        let commit = Commit::read(dir)?.ok_or_else(|| format!("{}: no commit record", dir.display()))?;
        let world = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
        let tiles = TileStore::open(dir, world.params()).map_err(|e| format!("{}: {e}", dir.display()))?;
        let cs = &commit.spec;
        let window = WindowIdentity { cells: spec.local() as u32, radius_m: R, seed, base_id: spec.base_id, inputs_id: spec.effective_inputs_id() };
        let differ = commit_differences(cs, &window, !no_key_check);
        if !differ.is_empty() {
            return Err(format!("{}: not baked on this window: {}", dir.display(), differ.join(", ")));
        }
        let mut surface = start.clone();
        let mut inside = vec![false; w * h];
        let (read, _) = lay_store_ground(&tiles, &world, &commit, !no_key_check, LWindow { face, x0, y0, w, h }, &|x, y| base(dir_of(x, y)), threads, &mut surface, &mut inside)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        println!("G1A store {}: {read} tiles, sea level {} m", dir.display(), cs.sea_level_m);
        stored = Some((surface, inside, cs.sea_level_m));
    }
    let sea = stored.as_ref().map_or(0.0, |s| s.2);

    // ---- today's K6, recomputed (the same bits as region_front)
    let grid = &solve.grid;
    let outlet = &solve.outlet;
    let cells = rw * rh;
    let dir_h1 = |c: usize| solve.face_spec.halo_direction((sri0 + c % rw) as i64, (srj0 + c / rw) as i64);
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; cells], vec![0.0f32; cells], vec![0.0f32; cells]);
    for c in (0..cells).filter(|&c| solve.zs[c] > 0.0) {
        let (ro, ev, wt) = climate(dir_h1(c));
        runoff[c] = ro as f32;
        evap[c] = ev as f32;
        weight[c] = wt as f32;
    }
    let eps = solve.precision.epsilon_w;
    let stream = solve.stream;
    let r0 = route_rules(grid, &solve.zs, outlet, eps, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let f0 = priority_flood_with(grid, &solve.zs, outlet, eps, true);
    let t0 = h1_from(grid, &solve.zs, &f0, r0, outlet, &runoff, &evap);
    if t0.graph.segments() != front.graph.segments() || t0.graph.vertices() != front.graph.vertices() {
        return Err("the recomputed K6 differs from region_front's".into());
    }
    // the control: this code on the point heights (no regrade)
    let zs = &solve.zs;
    let point_sill = |p: usize, _d: usize, q: usize| zs[p].max(zs[q]);
    let fc = flood(grid, zs, &point_sill, outlet, eps);
    let rc = route_sills(grid, &fc, &point_sill, outlet, stream, &weight, true);
    let tc = h1_from(grid, zs, &fc, rc, outlet, &runoff, &evap);

    // ---- sills of S0
    let s0_read = |c: usize| start[c];
    let t = Instant::now();
    let sills = nodes_and_sills(&s0_read, rw, rh, threads, None);
    let sill_s = t.elapsed().as_secs_f64();
    let edges = sills.fwd.iter().flatten().filter(|v| v.is_finite()).count();
    let above: Vec<f64> = (0..cells)
        .flat_map(|c| {
            let s = &sills;
            (0..4).filter(move |&k| s.fwd[c][k].is_finite()).map(move |k| {
                let (ox, oy) = FWD[k];
                let q = ((c / rw) as i64 + oy) as usize * rw + ((c % rw) as i64 + ox) as usize;
                s.fwd[c][k] - s.h[c].max(s.h[q])
            })
        })
        .collect();
    let mut sorted = above.clone();
    sorted.sort_by(f64::total_cmp);
    let q = |f: f64| sorted[((sorted.len() - 1) as f64 * f) as usize];
    println!(
        "G1A sills: {cells} H1 cells, {edges} edges in {sill_s:.1} s ({threads} threads; {:.2} us an edge); sill above the higher node: zero on {:.1} %, p50 {:.2} p90 {:.2} p99 {:.2} max {:.2} m",
        1e6 * sill_s / edges as f64,
        100.0 * sorted.iter().filter(|&&v| v == 0.0).count() as f64 / sorted.len() as f64,
        q(0.5),
        q(0.9),
        q(0.99),
        q(1.0)
    );
    let land_box_h1: Vec<bool> = (0..cells)
        .map(|c| {
            let (i, j) = (sri0 + c % rw, srj0 + c / rw);
            i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3 && !outlet[c]
        })
        .collect();
    let hz: Vec<f64> = (0..cells).filter(|&c| land_box_h1[c]).map(|c| solve.zs[c] - sills.h[c]).collect();
    let mut hz_s = hz.clone();
    hz_s.sort_by(f64::total_cmp);
    println!(
        "G1A node heights against z_s (land cells of the box): z_s - h p50 {:.1} p90 {:.1} p99 {:.1} m; h at or below the sea on {} land cells",
        hz_s[hz_s.len() / 2],
        hz_s[hz_s.len() * 9 / 10],
        hz_s[hz_s.len() * 99 / 100],
        (0..cells).filter(|&c| land_box_h1[c] && sills.h[c] <= 0.0).count()
    );

    let k6_sills = |s: &Sills| -> H1 {
        let sill = |p: usize, d: usize, q: usize| s.at(p, d, q);
        let f = flood(grid, &s.h, &sill, outlet, eps);
        let r = route_sills(grid, &f, &sill, outlet, stream, &weight, true);
        h1_from(grid, &s.h, &f, r, outlet, &runoff, &evap)
    };
    let k6_sills_mfd_all = |s: &Sills| -> H1 {
        let sill = |p: usize, d: usize, q: usize| s.at(p, d, q);
        let f = flood(grid, &s.h, &sill, outlet, eps);
        let r = route_sills(grid, &f, &sill, outlet, stream, &weight, false);
        h1_from(grid, &s.h, &f, r, outlet, &runoff, &evap)
    };
    let t = Instant::now();
    let t1 = k6_sills(&sills);
    println!("G1A K6' routing: {:.1} s", t.elapsed().as_secs_f64());
    let t1_mfd = k6_sills_mfd_all(&sills);
    // perturbed runs
    let t = Instant::now();
    let s0_noise = |c: usize| {
        let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
        start[c] + 0.5 * noise(0x6731_A5_0001, ((gy as u64) << 32) | gx as u64)
    };
    let sills_p0 = nodes_and_sills(&s0_noise, rw, rh, threads, None);
    let t1_p0 = k6_sills(&sills_p0);
    drop(sills_p0);
    let sills_ps = nodes_and_sills(&s0_read, rw, rh, threads, Some((0x6731_A5_0002, 1.0)));
    let t1_ps = k6_sills(&sills_ps);
    drop(sills_ps);
    let zs_p: Vec<f64> = (0..cells).map(|c| solve.zs[c] + 0.5 * noise(0x6731_A5_0003, grid.global_id(c))).collect();
    let r0p = route_rules(grid, &zs_p, outlet, eps, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let f0p = priority_flood_with(grid, &zs_p, outlet, eps, true);
    let t0_p = h1_from(grid, &zs_p, &f0p, r0p, outlet, &runoff, &evap);
    println!("G1A perturbed runs: {:.1} s", t.elapsed().as_secs_f64());

    // ---- topology
    let in_box_h1 = |c: u32| {
        let (i, j) = (sri0 + c as usize % rw, srj0 + c as usize / rw);
        i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3
    };
    let (g0, gc, g1, gp0, gps) = (topo(&t0, &in_box_h1), topo(&tc, &in_box_h1), topo(&t1, &in_box_h1), topo(&t1_p0, &in_box_h1), topo(&t1_ps, &in_box_h1));
    println!("G1A basins (kept, H1): today {}, control {}, K6' {}", t0.hydro.basins.len(), tc.hydro.basins.len(), t1.hydro.basins.len());
    println!("G1A topology {}", diff_line("today -> control (this code on z_s, no regrade)", &g0, &gc, rw));
    println!("G1A topology {}", diff_line("today -> K6' (sills of S0)", &g0, &g1, rw));
    println!("G1A topology {}", diff_line("control -> K6'", &gc, &g1, rw));
    println!("G1A topology {}", diff_line("today -> K6' with the MFD over all lower neighbours", &g0, &topo(&t1_mfd, &in_box_h1), rw));
    let g0p = topo(&t0_p, &in_box_h1);
    println!("G1A sensitivity {}", diff_line("today -> today with z_s +-0.5 m (baseline)", &g0, &g0p, rw));
    println!("G1A sensitivity {}", diff_line("K6' -> K6' with S0 +-0.5 m", &g1, &gp0, rw));
    println!("G1A sensitivity {}", diff_line("K6' -> K6' with sills +-1 m", &g1, &gps, rw));
    for (name, a, b) in [("today -> control", &t0, &tc), ("today -> K6'", &t0, &t1), ("today -> z_s +-0.5 m", &t0, &t0_p), ("K6' -> S0 +-0.5 m", &t1, &t1_p0), ("K6' -> sills +-1 m", &t1, &t1_ps)] {
        let (area, recv, total) = catchment_diff(grid, a, b, &land_box_h1, 2);
        let (area10, _, _) = catchment_diff(grid, a, b, &land_box_h1, 10);
        println!(
            "G1A catchments {name}: of {total:.0} km2 of the box's land, {area:.0} km2 ({:.2} %) drain to an outlet more than 2 cells (1.6 km) from the first's, {area10:.0} km2 ({:.2} %) more than 10 cells (8 km); {recv} cells change receiver",
            100.0 * area / total,
            100.0 * area10 / total
        );
    }
    // K5's channel cells: today's H1 channel cells (A_d >= 75 km2 on z_s), and K5's carve (z_b - z_s)
    let k5ch: HashSet<u32> = (0..cells as u32).filter(|&c| t0.hydro.channel[c as usize]).collect();
    for (name, g) in [("today", &g0), ("control", &gc), ("K6'", &g1)] {
        let off = g.cells.iter().filter(|&&c| !k5ch.contains(&c)).count();
        let off1 = g.cells.iter().filter(|&&c| !near(&k5ch, c, rw, 1)).count();
        let mut carve_on: Vec<f64> = g.cells.iter().filter(|&&c| k5ch.contains(&c)).map(|&c| solve.zb[c as usize] - solve.zs[c as usize]).collect();
        let mut carve_off: Vec<f64> = g.cells.iter().filter(|&&c| !k5ch.contains(&c)).map(|&c| solve.zb[c as usize] - solve.zs[c as usize]).collect();
        carve_on.sort_by(f64::total_cmp);
        carve_off.sort_by(f64::total_cmp);
        let p50 = |v: &[f64]| v.get(v.len() / 2).copied().unwrap_or(f64::NAN);
        println!(
            "G1A K5 channels {name}: {off} of {} trunk cells in the box off K5's channel cells ({:.2} %), {off1} more than 1 cell off ({:.2} %); K5's carve z_b - z_s p50 on {:.1} m, off {:.1} m",
            g.cells.len(),
            100.0 * off as f64 / g.cells.len().max(1) as f64,
            100.0 * off1 as f64 / g.cells.len().max(1) as f64,
            p50(&carve_on),
            p50(&carve_off)
        );
    }
    for (name, a, b) in [("today -> K6'", &t0, &t1), ("today -> control", &t0, &tc), ("today -> z_s +-0.5 m", &t0, &t0_p), ("K6' -> S0 +-0.5 m", &t1, &t1_p0), ("K6' -> sills +-1 m", &t1, &t1_ps)] {
        let m = major(a, b, &in_box_h1, rw);
        let changed = m.iter().filter(|r| !r.2 || r.3 < 95.0).count();
        let rows: Vec<String> =
            m.iter().map(|r| format!("{}:{:.0}->{:.0}m3/s@{},{},stem{}:{:.1}%", r.0, r.1, r.4, r.5, if r.2 { "mouth" } else { "MOVED" }, r.6, r.3)).collect();
        println!("G1A major {name}: {changed} of {} changed (the box exit moved more than 1 cell or under 95 % of the main stem in the box within 1 cell; the 10 largest rivers leaving the box with 20 cells or more of main stem in it): {}", m.len(), rows.join(" "));
    }

    for (name, h1) in [("today", &t0), ("control", &tc), ("K6'", &t1), ("K6' MFD over all lower neighbours", &t1_mfd)] {
        let (heads, side, km, n) = trunk_stats(h1, grid, &in_box_h1);
        println!(
            "G1A trunk stats {name}: {heads} heads in the box ({side} with a D8 area under 75 km2: fed by the MFD split, not by their own catchment), {km:.0} km of trunk, {n} cells ({:.3} km a cell)",
            km / n.max(1) as f64
        );
    }
    for (name, a, b) in [("today -> K6'", &t0, &t1), ("today -> control", &t0, &tc), ("today -> z_s +-0.5 m", &t0, &t0_p), ("K6' -> S0 +-0.5 m", &t1, &t1_p0), ("K6' -> sills +-1 m", &t1, &t1_ps)] {
        let rows = stop_test(a, b, &in_box_h1, rw);
        let changed = rows.iter().filter(|r| r.contains("CHANGED") || !r.contains("lost 0 [") || !r.contains("gained 0 [")).count();
        println!("G1A stop test {name}: {changed} of {} top rivers change outlet (> 8 km) or lose or gain a major confluence", rows.len());
        for r in rows {
            println!("G1A stop test {name}:   {r}");
        }
    }

    // ---- corridors
    let to_local = |c: u32| {
        let c = c as usize;
        ((L * (c % rw)) as f64 + (L as f64 - 1.0) / 2.0, (L * (c / rw)) as f64 + (L as f64 - 1.0) / 2.0)
    };
    let lines = |h1: &H1| -> (Vec<Vec<(f64, f64)>>, Vec<bool>) {
        (
            h1.trunk_cells.iter().map(|cs| cs.iter().map(|&c| to_local(c)).collect()).collect(),
            h1.trunk_seg.iter().map(|&k| h1.hydro.segments[k].downstream == NONE).collect(),
        )
    };
    // the bar: the start's p50 relief over 10 x 10 cells over the region's box
    let relief10 = {
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
        r[r.len() / 2]
    };
    let bar = corridor_bar_m(relief10);
    let radius = K_C * L as f64;
    let water_s0: Vec<f64> = start.iter().map(|&z| z.max(sea)).collect();
    let m_s0 = Measure { w, h, water: &water_s0, inside: &in_box, region: &in_box, sea, radius, bar };
    let water_b: Option<Vec<f64>> = stored.as_ref().map(|s| s.0.iter().map(|&z| z.max(sea)).collect());
    let m_b = stored.as_ref().map(|s| Measure { w, h, water: water_b.as_ref().expect("water"), inside: &s.1, region: &in_box, sea, radius, bar });
    println!("G1A bar {bar:.2} m (relief10 {relief10:.2} m); corridor k_c 2 snapped, land-only snap (#163 applied locally)");
    let mut res: HashMap<(&str, &str), Vec<Option<Reach>>> = HashMap::new();
    let t = Instant::now();
    for (gname, h1) in [("today", &t0), ("K6'", &t1), ("control", &tc), ("S0+-0.5", &t1_p0), ("sills+-1", &t1_ps)] {
        let (ls, mouth) = lines(h1);
        let mut surfaces: Vec<(&str, &Measure)> = vec![("S0", &m_s0)];
        if let Some(m) = &m_b {
            if gname == "today" || gname == "K6'" {
                surfaces.push(("B", m));
            }
        }
        for (sname, m) in surfaces {
            let r = measure_all(m, &ls, &mouth, threads);
            let measured = r.iter().flatten().count();
            let failed = r.iter().flatten().filter(|x| x.failed).count();
            let sea_end = r.iter().flatten().filter(|x| x.ended_at_sea).count();
            println!(
                "G1A corridor {gname} on {sname}: {} of {measured} reaches pass ({:.2} %), {failed} fail; {sea_end} routes end at the sea",
                measured - failed,
                100.0 * (measured - failed) as f64 / measured.max(1) as f64
            );
            res.insert((gname, sname), r);
        }
    }
    println!("G1A corridor time {:.1} s", t.elapsed().as_secs_f64());

    // relief and coast class of a window L cell (S0, 21 x 21 cells around it)
    let class_of = |x: usize, y: usize| -> (f64, bool, &'static str) {
        let (mut lo, mut hi, mut coast) = (f64::INFINITY, f64::NEG_INFINITY, false);
        for yy in y.saturating_sub(10)..(y + 11).min(h) {
            for xx in x.saturating_sub(10)..(x + 11).min(w) {
                let v = start[yy * w + xx];
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        for yy in y.saturating_sub(20)..(y + 21).min(h) {
            for xx in x.saturating_sub(20)..(x + 21).min(w) {
                coast |= start[yy * w + xx] <= sea;
            }
        }
        let r = hi - lo.max(sea);
        let c = if coast {
            "coast"
        } else if r < 50.0 {
            "low"
        } else if r < 200.0 {
            "hilly"
        } else if r < 500.0 {
            "rugged"
        } else {
            "mountain"
        };
        (r, coast, c)
    };
    // the reaches as CSV (all graphs and surfaces) and the trunk lines (today, K6')
    {
        let mut f = std::fs::File::create(out_dir.join(format!("{tag}-reaches.csv"))).map_err(|e| e.to_string())?;
        writeln!(f, "window,graph,surface,reach,segment,mouth,failed,rise_m,x,y,relief_m,class,ended_at_sea").map_err(|e| e.to_string())?;
        let mut keys: Vec<_> = res.keys().copied().collect();
        keys.sort();
        for key in keys {
            let h1 = match key.0 {
                "today" => &t0,
                "K6'" => &t1,
                "control" => &tc,
                "S0+-0.5" => &t1_p0,
                _ => &t1_ps,
            };
            for (k, r) in res[&key].iter().enumerate() {
                let Some(r) = r else { continue };
                let (rel, _, cls) = class_of(r.at.0, r.at.1);
                let seg = h1.trunk_seg[k];
                writeln!(
                    f,
                    "{tag},{},{},{k},{seg},{},{},{:.2},{},{},{rel:.1},{cls},{}",
                    key.0,
                    key.1,
                    h1.hydro.segments[seg].downstream == NONE,
                    r.failed,
                    r.rise,
                    x0 as usize + r.at.0,
                    y0 as usize + r.at.1,
                    r.ended_at_sea
                )
                .map_err(|e| e.to_string())?;
            }
        }
        let mut f = std::fs::File::create(out_dir.join(format!("{tag}-trunks.csv"))).map_err(|e| e.to_string())?;
        writeln!(f, "graph,reach,k,x,y,q_m3s").map_err(|e| e.to_string())?;
        for (gname, h1) in [("today", &t0), ("K6'", &t1)] {
            for (k, cs) in h1.trunk_cells.iter().enumerate() {
                for (i, &c) in cs.iter().enumerate() {
                    let p = to_local(c);
                    writeln!(f, "{gname},{k},{i},{},{},{:.3}", p.0 as usize, p.1 as usize, h1.hydro.qw[c as usize]).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    // the exception summary: failing reaches by class
    for key in [("today", "S0"), ("K6'", "S0"), ("today", "B"), ("K6'", "B")] {
        let Some(r) = res.get(&key) else { continue };
        let mut by: HashMap<&str, usize> = HashMap::new();
        for x in r.iter().flatten().filter(|x| x.failed) {
            *by.entry(class_of(x.at.0, x.at.1).2).or_default() += 1;
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort();
        println!("G1A exceptions {} on {}: {:?}", key.0, key.1, v);
    }

    // ---- the D-2 list: SPIKE-126's large reaches (B's store, final measure) that also fail on B under the land-only snap; each re-traced from its upstream node on K6'
    if let (Some(log), Some(mb)) = (d2_log, &m_b) {
        let text = std::fs::read_to_string(log).map_err(|e| format!("{}: {e}", log.display()))?;
        let d2: Vec<usize> = text
            .lines()
            .filter_map(|l| l.strip_prefix("PONDING large reach "))
            .filter_map(|l| l.split(':').next()?.parse().ok())
            .collect();
        let today_b = &res[&("today", "B")];
        let still: Vec<usize> = d2.iter().copied().filter(|&k| today_b.get(k).and_then(|r| r.as_ref()).is_some_and(|r| r.failed)).collect();
        println!("G1A D-2 list: {} reaches (SPIKE-126); {} still fail on B under the land-only snap (#163 locally)", d2.len(), still.len());
        let mut f = std::fs::File::create(out_dir.join(format!("{tag}-d2.csv"))).map_err(|e| e.to_string())?;
        writeln!(f, "window,reach,land_snap_fails_b,u_trunk_k6p,path,cells_old,cells_new,b_failed,b_rise_m,s0_failed,s0_rise_m,today_s0_failed,today_s0_rise_m,x,y,relief_m,class")
            .map_err(|e| e.to_string())?;
        let q1: Vec<bool> = {
            let mut v = vec![false; cells];
            for cs in &t1.trunk_cells {
                for &c in cs {
                    v[c as usize] = true;
                }
            }
            v
        };
        let (mut gain_b, mut gain_s0, mut counts) = (0usize, 0usize, HashMap::<&str, usize>::new());
        for &k in &d2 {
            let sg = &t0.hydro.segments[t0.trunk_seg[k]];
            let old_len = t0.trunk_cells[k].len();
            let (dx, dy) = ((sg.down as usize % rw) as i64, (sg.down as usize / rw) as i64);
            let mut path = vec![sg.up];
            let mut c = sg.up;
            let kind = loop {
                let (cx, cy) = ((c as usize % rw) as i64, (c as usize / rw) as i64);
                if path.len() > 1 && (cx - dx).abs() <= 1 && (cy - dy).abs() <= 1 {
                    break "rejoins";
                }
                let r = t1.routing.receiver[c as usize];
                if r == NONE {
                    break "outlet";
                }
                if path.len() > 3 * old_len + 10 {
                    break "diverted";
                }
                c = r;
                path.push(c);
            };
            *counts.entry(kind).or_default() += 1;
            let line: Vec<(f64, f64)> = path.iter().map(|&c| to_local(c)).collect();
            let mouth = kind == "outlet" || (kind == "rejoins" && sg.downstream == NONE);
            let rb = if line.len() >= 2 { mb.reach(&line, mouth) } else { None };
            let rs = if line.len() >= 2 { m_s0.reach(&line, mouth) } else { None };
            let ts = res[&("today", "S0")].get(k).cloned().flatten();
            let lf = still.contains(&k);
            if lf && rb.as_ref().is_some_and(|r| !r.failed) {
                gain_b += 1;
            }
            if lf && rs.as_ref().is_some_and(|r| !r.failed) {
                gain_s0 += 1;
            }
            let at = today_b[k].as_ref().map(|r| r.at).unwrap_or((0, 0));
            let (rel, _, cls) = class_of(at.0, at.1);
            let fmt = |r: &Option<Reach>| r.as_ref().map_or(",".to_string(), |r| format!("{},{:.2}", r.failed, r.rise));
            writeln!(
                f,
                "{tag},{k},{lf},{},{kind},{old_len},{},{},{},{},{},{},{rel:.1},{cls}",
                q1[sg.up as usize],
                path.len(),
                fmt(&rb),
                fmt(&rs),
                fmt(&ts),
                x0 as usize + at.0,
                y0 as usize + at.1
            )
            .map_err(|e| e.to_string())?;
        }
        let mut cv: Vec<_> = counts.into_iter().collect();
        cv.sort();
        println!(
            "G1A D-2 re-traced on K6' from the reach's upstream node: {cv:?}; of the {} failing under the land-only snap, {gain_b} gain a route under the bar on B, {gain_s0} on S0",
            still.len()
        );
    }
    println!("G1A done in {:.1} s", t_all.elapsed().as_secs_f64());
    Ok(())
}
