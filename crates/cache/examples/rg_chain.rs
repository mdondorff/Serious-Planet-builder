//! G1f chain runner (explorer #166, spike branch spike/rg-chain, never merged): the RG chain composed on a window from the lanes' spike code, each stage switchable, writing what
//! `river-gate --graph --stages --partition` reads (BUILD-060) and the final surface, with time and memory per stage. Plumbing only: no tuning and no judgement.
//! Stages: H1 base (K6' default, `--base today`), KH (c1), KS0 / KS1 (`--ks`), KG-trunk (spikes' G1c, below), basins (crusader's G1e), KG-trib (crusader's G1d), KV and KC (crusader's
//! G1f), `--skip` any of kg-trunk, basins, kg-trib, kv, kc; `--regions 2` evaluates the pointwise fields over two regions cut at a tile column (the partition run).
//! What follows is crusader's kg_trib.rs (SPIKE-133) up to the H1 base; its header:
//! SPIKE-133 (G1d, lead #158; spike branch spike/g1d only, never merged): SPIKE-129's KG-trunk example (copied from spike/g1c-trunk-k6p-2 at e29c0bd6, on main's types) with KG-trib
//! extractor-1 run on its trunks (the G1d block; the rest of SPIKE-129's report after the graph is not run).
//! SPIKE-129 (G1c, lead #132): KG-trunk, the L images of the H1 trunk graph (G0 contracts, GEN-189 types), without an L erosion round.
//! H1 base: K6' on the sills of S0 (SPIKE-127; DEC spikes-3 default) or today's K6 (`--base today`). KS0 = C1(z_s) + min(r, C1(HAND)); L anchors by contract §2.1 on S0;
//! centrelines by BUILD-057's corridor route on KS0; beds by §3 with the §6 budget remainder (rise_over_budget beyond it); rasterisation by §5.
//! The helpers below are SPIKE-127's and SPIKE-131's (g1a_sills.rs, k5p_sills.rs).
// spike only (never merged): lints allowed as run, the code unchanged
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion,
    dead_code,
    unused_imports,
    unused_variables
)]
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
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
use planet_generators::bake::hydro::{hydrology, id_hash, Hydro, HydroParams};
use planet_generators::bake::k7::RegionField;
use planet_generators::bake::river_graph::{
    diff, node_id, rasterise, reach_id, validate, validate_raster, Attrs, Checks, Domain, Exception, Graph, Level, Marker, Node, NodeKind,
    Reach as GReach, Reason, Violation,
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

fn h1_from(
    grid: &Grid,
    z: &[f64],
    filled: &[f64],
    routing: Routing,
    outlet: &[bool],
    runoff: &[f32],
    evap: &[f32],
    budget: Option<&[f64]>,
) -> H1 {
    let mut hydro = hydrology(grid, z, filled, &routing, outlet, Some(runoff), Some(evap), &HydroParams::default());
    // spikes #142, reading 16: a kept basin deeper than the budget at its floor is crossed (reading 13), not a lake: no open-water loss there, so q_w = q through it.
    // The basins are the same (the loss enters only the balance); the rerun changes q_w and the balance only.
    // reading 24 (lead, 2026-11-17; owner decision 6 c): no H1 basin is a lake in the prototype, every one is crossed (q_w = q); `budget` only switches the rule on
    if budget.is_some() {
        let crossed: Vec<bool> = vec![true; hydro.basins.len()];
        if crossed.iter().any(|&x| x) {
            let ev2: Vec<f32> =
                (0..evap.len()).map(|c| if hydro.label[c] != NONE && crossed[hydro.label[c] as usize] { 0.0 } else { evap[c] }).collect();
            let again = hydrology(grid, z, filled, &routing, outlet, Some(runoff), Some(&ev2), &HydroParams::default());
            assert_eq!(again.basins.len(), hydro.basins.len(), "the loss does not change the basins");
            hydro = again;
        }
        // q_w as a running sum along the routing: q less the losses of the basins upstream (each basin's q_in - q_out, entered at its spill cell; zero for a crossed
        // basin). The cascade's q_w inside a basin is no channel discharge (a basin's cells are taken in index order on its flat), so a trunk through a crossed basin reads this one.
        let mut loss = vec![0f64; hydro.q.len()];
        // entered where the routing leaves the basin (the first cell outside it on the floor's receiver chain), which need not be its spill cell
        for (k, b) in hydro.basins.iter().enumerate() {
            let mut c = b.floor_cell;
            while hydro.label[c as usize] == k as u32 && routing.receiver[c as usize] != NONE {
                c = routing.receiver[c as usize];
            }
            loss[c as usize] += b.q_in - b.q_out;
        }
        for &c in routing.order.iter().rev() {
            let r = routing.receiver[c as usize];
            if r != NONE {
                loss[r as usize] += loss[c as usize];
            }
        }
        hydro.qw = (0..hydro.q.len()).map(|c| (hydro.q[c] - loss[c]).max(0.0)).collect();
    }
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
    /// reading 12: the sea is the ocean (cells at or below the sea level connected to an H1 sea outlet or the window's edge); `None`: every cell at or below it
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
fn route_through(m: &Measure, line: &[(f64, f64)], points: &[usize], avoid: &HashSet<usize>) -> Option<(Vec<usize>, bool, f64)> {
    let (mut index, cells) = m.corridor(line)?;
    index.retain(|c, _| !avoid.contains(c) || points.contains(c));
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
fn route_low(m: &Measure, line: &[(f64, f64)], points: &[usize], avoid: &HashSet<usize>) -> Option<(Vec<usize>, bool, f64)> {
    let (mut index, cells) = m.corridor(line)?;
    index.retain(|c, _| !avoid.contains(c) || points.contains(c));
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
    // reading 24 (replaces reading 13 for the prototype): no H1 basin cuts a trunk; every trunk crosses, and a crossing over the budget is KG-trunk's rise_over_budget on S_KH
    let _ = budget;
    let kept = |_c: u32| -> Option<usize> { None };
    // reading 26 (spikes, lead 2026-11-18): inside a crossed basin only the segments fed from outside it stay trunk reaches. A segment is fed when its up cell lies in no
    // H1 basin, or when a segment flowing into it is fed; an unfed segment (it starts inside a basin and only basin-floor segments flow into it) is the floor's own drainage
    // and makes no reach. Fed propagates downstream, so a kept segment's downstream segment is kept: the trunk enters the basin and runs on to the spill as one path per inflow.
    let label = &h1.hydro.label;
    let mut ups: Vec<Vec<usize>> = vec![Vec::new(); segs.len()];
    for (k, s) in segs.iter().enumerate() {
        if s.downstream != NONE {
            ups[s.downstream as usize].push(k);
        }
    }
    let pos: std::collections::HashMap<u32, usize> = h1.routing.order.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let mut by_up: Vec<usize> = (0..segs.len()).collect();
    // upstream first: the routing order lists a cell after its receiver
    by_up.sort_by_key(|&k| std::cmp::Reverse(pos.get(&segs[k].up).copied().unwrap_or(0)));
    let mut fed = vec![false; segs.len()];
    for &k in &by_up {
        fed[k] = label[segs[k].up as usize] == NONE || ups[k].iter().any(|&u| fed[u]);
    }
    {
        let dropped = fed.iter().filter(|&&f| !f).count();
        let cells_dropped: usize = (0..segs.len())
            .filter(|&k| !fed[k])
            .map(|k| {
                let (mut c, mut n) = (segs[k].up, 1usize);
                while c != segs[k].down {
                    c = h1.routing.receiver[c as usize];
                    n += 1;
                }
                n
            })
            .sum();
        let broken = (0..segs.len()).filter(|&k| fed[k] && segs[k].downstream != NONE && !fed[segs[k].downstream as usize]).count();
        println!("KG reading 26: {dropped} of {} H1 segments unfed inside basins ({cells_dropped} cells) make no trunk reach; fed segments flowing into an unfed one {broken}", segs.len());
    }
    let mut out = Vec::new();
    let mut edge_ends = 0usize;
    for (k, s) in segs.iter().enumerate() {
        if !fed[k] {
            continue;
        }
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
    skh: &[f64],
    ks0: &[f64],
    budget_h1: &[f64],
    window: (u64, u64, usize, usize, usize),
    sea: f64,
    bar: f64,
    order: &[u8],
    threads: usize,
    low_centre: bool,
    ocean: &[bool],
    runoff_mean: f64,
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
            // lead #180: a runoff-weighted area (m2), the discharge over KG-trib's uniform runoff; it held q (m3/s) before
            a_run: r.cells.iter().map(|&c| h1.hydro.q[c as usize] / runoff_mean).collect(),
            a_run_no_ext: r.cells.iter().map(|&c| h1.hydro.q[c as usize] / runoff_mean).collect(),
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
                    route_low(&mks, line, &pts, &taken)
                } else {
                    route_through(&mks, line, &pts, &taken)
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
            let ks_used = (skh[c] - ks0[c]).max(0.0);
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
        // spikes #142: a rise at the last vertex is taken one vertex earlier (bed[n-2] raised to bed[n-1]: less cut, never more); its marker would share the
        // down node's anchor (reading 1). A two-vertex reach drops it (its bed then starts at its end's level).
        if rise_at.last() == Some(&(n - 1)) {
            rise_at.pop();
            if n == 2 && TRUNK_BED_SHARED.load(std::sync::atomic::Ordering::Relaxed) {
                // crusader #168b (--trunk-bed shared, default; reading 32): a two-vertex reach has no vertex for the rise's marker, and raising its first vertex breaks the up
                // node's shared bed (bed_not_shared); its first vertex keeps the up node's bed and the rise at its down end stays, excused by its rise_over_budget exception
            } else if n >= 2 {
                bed[n - 2] = bed[n - 2].max(bed[n - 1]);
                if n >= 3 && rise_at.last() != Some(&(n - 2)) && bed[n - 2] > bed[n - 3] {
                    rise_at.push(n - 2);
                }
            }
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
            vec![Exception { reason: Reason::RiseOverBudget, value: over }]
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
            a_run: (0..n).map(|v| h1.hydro.q[at(v)] / runoff_mean).collect(),
            a_run_no_ext: (0..n).map(|v| h1.hydro.q[at(v)] / runoff_mean).collect(),
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

/// lead #173: `--kv-sea-floor on` (KV cuts no target below the sea level) and `--kc-lake-outlets on` (KC's depressions drain into the kept lakes).
static KV_SEA_FLOOR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static KC_LAKE_OUTLETS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// lead #176a: `--kv-rims on` (default) holds the kept basins' rims at their spill against KV's cut and KC's breach; `off` the form before.
static KV_RIMS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// lead #177: `--kv-connect on` (default, DEC crusader-23) lowers one edge cell on each diagonal step of an L reach to the step's mean bed; `off` the vertices alone.
static KV_CONNECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// crusader #172 (owner, 2026-11-20): `--exception-lakes on` keeps the hollows of over-budget rises as lakes.
static EXCEPTION_LAKES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// crusader #171 (DEC crusader-20): `--kept-drained drop` drains a kept basin whose rim an L line crosses below its spill.
static KEPT_DRAINED_DROP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// crusader #172: `--kept-drained exit` counts only the crossings where an L line leaves the basin (its vertex inside, the next one outside, below spill - 0.5 m); a line
/// that enters below the spill floods an arm up its own valley and drains nothing.
static KEPT_DRAINED_EXIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// crusader #169 item 3: `--kv-cross`'s grade in micro (m per m), 0 off.
static KV_CROSS_MICRO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// lead #158 (spikes #157), default since reading 38 (#158a): `--ks-connect 4`: KS, band6 and KH's h5 carve diagonal lines 4-connected; `8`: the former rule.
static KS_CONNECT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// crusader #168b (reading 32): `--trunk-bed shared` (default) keeps a two-vertex trunk image's first vertex at its up node's bed when it rises over the budget, the rise at its down end
/// excused by its rise_over_budget exception; `lift` raises its first vertex (spikes #142's form).
static TRUNK_BED_SHARED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

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
fn main() -> Result<(), String> {
    let t_all = Instant::now();
    // the commit and the tree's state at the run's start (lead #169: chain.txt names them; read now, since the worktree may change branch before the run ends)
    let run_commit = std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or("unknown".to_string(), |v| v.trim().to_string());
    let run_dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .map_or(true, |o| !o.stdout.is_empty());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let win: Vec<usize> = value(&args, "--window")
        .ok_or("--window face,i0,i1,j0,j1,seed")?
        .split(',')
        .map(|x| x.parse::<usize>().map_err(|_| "--window numbers".to_string()))
        .collect::<Result<_, _>>()?;
    let margin: usize = value(&args, "--margin").map_or(Ok(200), str::parse).map_err(|_| "--margin")?;
    let threads: usize = value(&args, "--threads").map_or(Ok(8), str::parse).map_err(|_| "--threads")?;
    let out_dir = PathBuf::from(value(&args, "--out").unwrap_or("target/kg"));
    let tag = value(&args, "--tag").unwrap_or("w").to_string();
    // stage 4 (lead #170 item 1): H1 is today's K6 by default; `--h1 k6p` is K6' on the sills of S0 (SPIKE-127). `--base aplus|today` is the base stages 1, 3, 6 (spikes' fold-in;
    // until it lands only `today`)
    let h1_form = value(&args, "--h1").unwrap_or("k6");
    if !["k6", "k6p"].contains(&h1_form) {
        return Err(format!("--h1 k6 | k6p, not '{h1_form}'"));
    }
    // spikes #146 (SPIKE-138): `--base aplus` (the default) is stages 1 and 3, the selective drain of `z_b` before K5 and of `z_s` after it (a breach only within 50 m of
    // cut, a fill only within 50 m of depth; deeper basins stay), and stage 6, the same rule on the KS1 output with the budget at the floor as both caps (reading 23 post).
    // `--base today` is today's base, no drain.
    let base_form = value(&args, "--base").unwrap_or("aplus");
    if !["aplus", "today"].contains(&base_form) {
        return Err(format!("--base aplus | today, not '{base_form}'"));
    }
    // spikes #147: `--base-stages 13` (stages 1 and 3, the default) or `3` (stage 3 only: no drain before K5)
    let base_stages = value(&args, "--base-stages").unwrap_or("13");
    if !["13", "3"].contains(&base_stages) {
        return Err(format!("--base-stages 13 | 3, not '{base_stages}'"));
    }
    if base_form == "aplus" {
        planet_cache::region_bake::install_s138(if base_stages == "3" { "post-only" } else { "sel-post" }, f64::INFINITY, &tag)?;
    }
    // spikes #147: `--breach-cost cell` (SPIKE-138) or `length` (the cut cost times the step's length, plus a length term)
    let breach_cost = value(&args, "--breach-cost").unwrap_or("cell");
    if !["cell", "length", "square"].contains(&breach_cost) {
        return Err(format!("--breach-cost cell | length | square, not '{breach_cost}'"));
    }
    planet_generators::bake::drain::BREACH_BY_LENGTH.store(breach_cost == "length", std::sync::atomic::Ordering::Relaxed);
    planet_generators::bake::drain::BREACH_SQUARE.store(breach_cost == "square", std::sync::atomic::Ordering::Relaxed);
    // spikes #147: `--drain-rule volume` (SPIKE-138: the smaller volume when both are within the caps) or `fill` (the fill first; a breach only where the fill is over its cap)
    let drain_rule = value(&args, "--drain-rule").unwrap_or("volume");
    if !["volume", "fill"].contains(&drain_rule) {
        return Err(format!("--drain-rule volume | fill, not '{drain_rule}'"));
    }
    planet_generators::bake::drain::PREFER_FILL.store(drain_rule == "fill", std::sync::atomic::Ordering::Relaxed);
    // spikes #147: `--breach-max-len N` (cells of the grid the drain runs on; 0: none), the hybrid breach-fill's limit
    let breach_max_len: usize = value(&args, "--breach-max-len").map_or(Ok(0), str::parse).map_err(|_| "--breach-max-len cells")?;
    planet_generators::bake::drain::BREACH_MAX_LEN.store(breach_max_len, std::sync::atomic::Ordering::Relaxed);
    let base_today = h1_form == "k6";
    // SPIKE-139 (lead #148 item 2, control): `--zs-read c1` (default) reads z_s at L by C1; `contour` first smooths z_s along its contour (5 taps [1 4 6 4 1]/16 at half-cell
    // steps, bilinear, perpendicular to the central-difference gradient) on every cell but H1's channel cells, so S0 = C1(z_s') + r and KH's C1 reads z_s'; K5, H1, HAND, KH's
    // routing and its h4c on the zone keep z_s (spike only)
    let zs_read = value(&args, "--zs-read").unwrap_or("c1");
    if !["c1", "contour", "gauss"].contains(&zs_read) {
        return Err(format!("--zs-read c1 | contour | gauss, not '{zs_read}'"));
    }
    // SPIKE-139 (lead #148 item 2): `--k5-route d8` (default: the jittered steepest D8), `ltd` (D8-LTD, Orlandini et al. 2003) or `dinf` (D-infinity's two receivers in
    // K5's steps, Tarboton 1997) for K5's rounds only (spike only)
    let k5_route = value(&args, "--k5-route").unwrap_or("d8");
    if !["d8", "ltd", "dinf"].contains(&k5_route) {
        return Err(format!("--k5-route d8 | ltd | dinf, not '{k5_route}'"));
    }
    planet_cache::region_bake::K5_LTD.store(k5_route == "ltd", std::sync::atomic::Ordering::Relaxed);
    planet_cache::region_bake::K5_DINF.store(k5_route == "dinf", std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139: `--k5-diffuse C` (default 0, off): a hillslope term in K5's rounds, C per sweep on the 5-point Laplacian (round::K5_DIFFUSE; spike only)
    let k5_diffuse: f64 = value(&args, "--k5-diffuse").map_or(Ok(0.0), str::parse).map_err(|_| "--k5-diffuse C")?;
    if !(0.0..=0.25).contains(&k5_diffuse) {
        return Err("--k5-diffuse C needs 0 <= C <= 0.25".into());
    }
    planet_generators::bake::round::K5_DIFFUSE.store(k5_diffuse.to_bits(), std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139 (lead #149a): `--k5-diffuse-stencil 5` (default) or `9` (the isotropic 9-point Laplacian; round::K5_DIFFUSE_NINE; spike only)
    let k5_stencil = value(&args, "--k5-diffuse-stencil").unwrap_or("5");
    if !["5", "9"].contains(&k5_stencil) {
        return Err(format!("--k5-diffuse-stencil 5 | 9, not '{k5_stencil}'"));
    }
    planet_generators::bake::round::K5_DIFFUSE_NINE.store(k5_stencil == "9", std::sync::atomic::Ordering::Relaxed);
    // spikes #154, lead #156 (reading 36): `--k5-edge free` (default: a window-cut catchment's edge outlet is breached like any receiver, a free outflow edge; round::K5_EDGE_FREE) or `dam` (the edge outlet keeps its height, the former rule and main's; spike only)
    // lead #158, #158a (reading 38): `--ks-connect 4` (default: KS, band6 and KH h5 lower one shared edge cell on each diagonal step) or `8` (the former rule; spike only)
    let ks_connect = value(&args, "--ks-connect").unwrap_or("4");
    if !["4", "8"].contains(&ks_connect) {
        return Err(format!("--ks-connect 4 | 8, not '{ks_connect}'"));
    }
    KS_CONNECT.store(ks_connect == "4", std::sync::atomic::Ordering::Relaxed);
    // lead #159: `--dump-ks-steps` writes KS's steps into the chain directory (ks0, ks1, ks1c after KS's connect, band6, band6c after its connect; one region only)
    let dump_ks_steps = args.iter().any(|a| a == "--dump-ks-steps");
    // lead #160 (RGv0 section 25, reading 12 in every stage): `--sea-mask connected`: KS, KG-trib, the basin passes, band6, KV and KC take KG-trunk's ocean (cells at or below
    // the sea connected to an H1 sea outlet or the window's edge) as the sea; an inland cell below the sea level is land; the default since reading 39 (#160). `level`: every cell at or below it (spike only)
    let sea_mask = value(&args, "--sea-mask").unwrap_or("connected");
    // lead #161: `--band6-hump M` (m; 0 off, the default): band6 takes no breach whose deepest cut exceeds M; it fills the depression within the budget, else leaves it (spike only)
    // lead #161, #161a (reading 40): `--kh-lower-only on` (default): KH's h5 blend lowers only (KH = C1 + w min(0, h4c - C1)); `off`: the former blend, which also raises (spike only)
    // lead #162 (review-fable-rgv0 section 1, candidate 1): `--trib-once`: no basins pass 1 and no KG-trib pass 1; KS1 clips along a routing of KS0 itself (KG-trib's rules:
    // route_rules_filled, eps 1e-3, seed 1, D8 area with the H1 inflow, channels from 1 km2 and downstream; a channel cell without a trunk bed takes the running minimum
    // down its channel), and KG-trib runs once, after band6 (--trib-on post). Default off (spike only)
    let trib_once = args.iter().any(|a| a == "--trib-once");
    let kh_lower_only = match value(&args, "--kh-lower-only").unwrap_or("on") {
        "on" => true,
        "off" => false,
        v => return Err(format!("--kh-lower-only on | off, not '{v}'")),
    };
    let band6_hump: f64 =
        value(&args, "--band6-hump").map_or(Ok(0.0), |v| v.parse().map_err(|_| format!("--band6-hump <m>, not '{v}'")))?;
    if !["level", "connected"].contains(&sea_mask) {
        return Err(format!("--sea-mask level | connected, not '{sea_mask}'"));
    }
    let k5_edge = value(&args, "--k5-edge").unwrap_or("free");
    if !["dam", "free"].contains(&k5_edge) {
        return Err(format!("--k5-edge dam | free, not '{k5_edge}'"));
    }
    planet_generators::bake::round::K5_EDGE_FREE.store(k5_edge == "free", std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139 (lead #149b): `--k5-uok-scale X` (default 1): K5's uplift ratio u_over_k times X, k_dt kept (region_bake::K5_UOK_SCALE; spike only)
    let k5_uok_scale: f64 = value(&args, "--k5-uok-scale").map_or(Ok(1.0), str::parse).map_err(|_| "--k5-uok-scale X")?;
    if !(k5_uok_scale > 0.0 && k5_uok_scale <= 8.0) {
        return Err("--k5-uok-scale X needs 0 < X <= 8".into());
    }
    planet_cache::region_bake::K5_UOK_SCALE.store(k5_uok_scale.to_bits(), std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139 (lead #149 item 3): `--k5-kdt-scale X` (default 1): K5's k_dt times X, the uplift per step with it (region_bake::K5_KDT_SCALE; spike only)
    let k5_kdt_scale: f64 = value(&args, "--k5-kdt-scale").map_or(Ok(1.0), str::parse).map_err(|_| "--k5-kdt-scale X")?;
    if !(k5_kdt_scale > 0.0 && k5_kdt_scale <= 8.0) {
        return Err("--k5-kdt-scale X needs 0 < X <= 8".into());
    }
    planet_cache::region_bake::K5_KDT_SCALE.store(k5_kdt_scale.to_bits(), std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139: `--k5-diffuse-amax KM2` (default 0: every cell): the hillslope term moves only cells of a smaller area (round::K5_DIFFUSE_AMAX; spike only)
    let k5_diffuse_amax: f64 = value(&args, "--k5-diffuse-amax").map_or(Ok(0.0), str::parse).map_err(|_| "--k5-diffuse-amax KM2")?;
    planet_generators::bake::round::K5_DIFFUSE_AMAX.store((k5_diffuse_amax * 1e6).to_bits(), std::sync::atomic::Ordering::Relaxed);
    // SPIKE-139: `--k5-parity C` (default 0, off): damps the 814 m checkerboard mode after each K5 round (round::K5_PARITY; spike only)
    let k5_parity: f64 = value(&args, "--k5-parity").map_or(Ok(0.0), str::parse).map_err(|_| "--k5-parity C")?;
    if !(0.0..=2.0).contains(&k5_parity) {
        return Err("--k5-parity C needs 0 <= C <= 2".into());
    }
    planet_generators::bake::round::K5_PARITY.store(k5_parity.to_bits(), std::sync::atomic::Ordering::Relaxed);
    if zs_read != "c1" && !base_today {
        return Err("--zs-read contour needs --h1 k6".into());
    }
    let low = value(&args, "--centre") != Some("maximin");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    // --chain DIR (lead #167a): the chain layout of `planet_cache::stage_sheet` (BUILD-060's chain sheet), one surface per stage
    let chain_dir: Option<PathBuf> = value(&args, "--chain").map(PathBuf::from);
    if let Some(d) = &chain_dir {
        std::fs::create_dir_all(d.join("stages")).map_err(|e| e.to_string())?;
    }
    let chain_f32 = |name: &str, z: &[f64]| -> Result<(), String> {
        if let Some(d) = &chain_dir {
            let b: Vec<u8> = z.iter().flat_map(|&v| (v as f32).to_le_bytes()).collect();
            std::fs::write(d.join(name), b).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    let (face, seed) = (Face(win[0] as u8), win[5] as u64);
    let region = (win[1], win[2], win[3], win[4]);
    let def = WorldDefinition::single_planet(R, seed);
    // reading 30 (lead #173): GEN-140's per-octave lattice turn of the band and crease noise is on by default for the whole run; `--noise axis` is the former, axis-aligned
    // base (every chain run before 2026-11-19); `--turned-noise` is kept as a no-op for older command lines
    let noise = value(&args, "--noise").unwrap_or("turned");
    if !["turned", "axis"].contains(&noise) {
        return Err(format!("--noise turned | axis, not '{noise}'"));
    }
    planet_generators::height::set_turned_noise(noise == "turned");
    // crusader #169 item 3: --kv-cross <grade> (m per m; 0 = off, the default until the rows): KV's floor falls toward the line from any channel on it
    let kv_cross: f64 = value(&args, "--kv-cross").map_or(Ok(0.0), |v| v.parse().map_err(|_| format!("--kv-cross <grade>, not '{v}'")))?;
    if !(0.0..=1.0e-3).contains(&kv_cross) {
        return Err(format!(
            "--kv-cross in [0, 0.001] (at most KV's s_floor; a larger grade lowers the whole floor below its cone), not {kv_cross}"
        ));
    }
    KV_CROSS_MICRO.store((kv_cross * 1.0e6).round() as u64, std::sync::atomic::Ordering::Relaxed);
    // DEC crusader-19 (a switch, default off): --kv-cross-side <m>,<grade>: the cross rule also on flat side cells within <m> of a vertex standing at most <grade> a metre above it
    let kv_cross_side: (f64, f64) = match value(&args, "--kv-cross-side") {
        None => (0.0, 0.0),
        Some(v) => {
            let bad = || format!("--kv-cross-side <m>,<grade>, not '{v}'");
            let (a, b) = v.split_once(',').ok_or_else(bad)?;
            let (a, b): (f64, f64) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
            if !(0.0..=600.0).contains(&a) || !(0.0..=0.1).contains(&b) || kv_cross == 0.0 {
                return Err(format!("--kv-cross-side: <m> in [0, 600], <grade> in [0, 0.1], with --kv-cross; not '{v}'"));
            }
            (a, b)
        }
    };
    // crusader #171 (DEC crusader-20, reading 35): --kept-drained exit (default, owner's option C, reading 37: only a line leaving a kept basin below its spill drains it) | drop
    // (every crossing below the spill, the default before) | off (every kept basin protected)
    let kept_drained = value(&args, "--kept-drained").unwrap_or("exit");
    // lead #173: --kv-sea-floor off | on, --kc-lake-outlets off | on (both off by default until the rows)
    for (opt, flag, default) in [
        ("--kv-sea-floor", &KV_SEA_FLOOR, "off"),
        ("--kc-lake-outlets", &KC_LAKE_OUTLETS, "off"),
        ("--kv-rims", &KV_RIMS, "on"),
        ("--kv-connect", &KV_CONNECT, "on"),
    ] {
        let v = value(&args, opt).unwrap_or(default);
        if !["off", "on"].contains(&v) {
            return Err(format!("{opt} off | on, not '{v}'"));
        }
        flag.store(v == "on", std::sync::atomic::Ordering::Relaxed);
    }
    // crusader #172 (owner, 2026-11-20: measure it): --exception-lakes off (default) | on
    let exception_lakes = value(&args, "--exception-lakes").unwrap_or("off");
    if !["off", "on"].contains(&exception_lakes) {
        return Err(format!("--exception-lakes off | on, not '{exception_lakes}'"));
    }
    EXCEPTION_LAKES.store(exception_lakes == "on", std::sync::atomic::Ordering::Relaxed);
    if !["off", "drop", "exit"].contains(&kept_drained) {
        return Err(format!("--kept-drained off | drop | exit, not '{kept_drained}'"));
    }
    KEPT_DRAINED_DROP.store(kept_drained == "drop" || kept_drained == "exit", std::sync::atomic::Ordering::Relaxed);
    KEPT_DRAINED_EXIT.store(kept_drained == "exit", std::sync::atomic::Ordering::Relaxed);
    let trunk_bed = value(&args, "--trunk-bed").unwrap_or("shared");
    if !["shared", "lift"].contains(&trunk_bed) {
        return Err(format!("--trunk-bed shared | lift, not '{trunk_bed}'"));
    }
    TRUNK_BED_SHARED.store(trunk_bed == "shared", std::sync::atomic::Ordering::Relaxed);
    let inputs = class_inputs(&def, seed, threads);
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f64; nw * nh];
    par_fill(&mut nodes, nw, threads, &|p| inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)));
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes.iter().map(|&v| v as f32).collect() };
    let base_read = |d: Vec3| -> f64 { f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle")) };
    // spikes #151 (G2's ridge edit, spike only): `--edit ridge:X0,Y0,X1,Y1,H,W` adds H m along the segment (global face L cells) with a Gaussian cross-section of width W m
    // (H exp(-(d/W)^2), d the distance to the segment); `--edit saddle:X,Y,H,W` subtracts H exp(-(r/W)^2) around a point. The edit is in the base B itself, so every stage reads it.
    let edit: Option<(bool, [f64; 6])> = match value(&args, "--edit") {
        None => None,
        Some(e) => {
            let (kind, nums) = e.split_once(':').ok_or("--edit ridge:X0,Y0,X1,Y1,H,W | saddle:X,Y,H,W")?;
            let v: Vec<f64> = nums.split(',').map(str::parse).collect::<Result<_, _>>().map_err(|_| "--edit: numbers")?;
            match (kind, v.len()) {
                ("ridge", 6) => Some((true, [v[0], v[1], v[2], v[3], v[4], v[5]])),
                ("saddle", 4) => Some((false, [v[0], v[1], v[0], v[1], v[2], v[3]])),
                _ => return Err("--edit ridge:X0,Y0,X1,Y1,H,W | saddle:X,Y,H,W".into()),
            }
        }
    };
    let n_face = (REGIONAL * L) as f64;
    // an L cell's side at the face's middle (the edit's widths in metres; 101.8 m at Earth's radius)
    let cell_l_m = R * std::f64::consts::FRAC_PI_2 / n_face;
    let edit_m = |d: Vec3| -> f64 {
        let Some((raise, [x0, y0, x1, y1, hgt, wid])) = edit else { return 0.0 };
        let (f, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
        if f != face {
            return 0.0;
        }
        let (x, y) = ((s + 1.0) * n_face / 2.0 - 0.5, (t + 1.0) * n_face / 2.0 - 0.5);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = dx * dx + dy * dy;
        let u = if len2 > 0.0 { (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let (ex, ey) = (x - (x0 + u * dx), y - (y0 + u * dy));
        let dist_m = (ex * ex + ey * ey).sqrt() * cell_l_m;
        let bump = hgt * libm::exp(-(dist_m / wid) * (dist_m / wid));
        if raise {
            bump
        } else {
            -bump
        }
    };
    let base = |d: Vec3| -> f64 { base_read(d) + edit_m(d) };
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
    let dir_of = |x: i64, y: i64| {
        planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0)
    };
    let grid = &solve.grid;
    let outlet = &solve.outlet;
    let cells = rw * rh;
    let sea = 0.0;

    // SPIKE-139: the read's z_s (z_s itself unless `--zs-read contour`) and its regional field
    let zs_l: Vec<f64> = if zs_read != "c1" {
        let dir_h1 = |c: usize| solve.face_spec.halo_direction((sri0 + c % rw) as i64, (srj0 + c / rw) as i64);
        let (mut weight, mut runoff, mut evap) = (vec![1.0f32; cells], vec![0.0f32; cells], vec![0.0f32; cells]);
        for c in (0..cells).filter(|&c| solve.zs[c] > 0.0) {
            let (ro, ev, wt) = climate(dir_h1(c));
            runoff[c] = ro as f32;
            evap[c] = ev as f32;
            weight[c] = wt as f32;
        }
        let r0 = route_rules(grid, &solve.zs, outlet, solve.precision.epsilon_w, solve.stream, 0.0, Some(&weight), RoundRules::ROUND);
        let f0 = priority_flood_with(grid, &solve.zs, outlet, solve.precision.epsilon_w, true);
        let chan = h1_from(grid, &solve.zs, &f0, r0, outlet, &runoff, &evap, Some(&Vec::new())).hydro.channel;
        let z = &solve.zs;
        let bil = |x: f64, y: f64| -> f64 {
            let (xf, yf) = (x.floor(), y.floor());
            let (i, j) = (xf as usize, yf as usize);
            let (u, v) = (x - xf, y - yf);
            let at = |a: usize, b: usize| z[b * rw + a];
            (1.0 - v) * ((1.0 - u) * at(i, j) + u * at(i + 1, j)) + v * ((1.0 - u) * at(i, j + 1) + u * at(i + 1, j + 1))
        };
        let (taps, wts) = ([-1.0, -0.5, 0.0, 0.5, 1.0], [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0]);
        let mut changed = 0usize;
        let out: Vec<f64> = (0..cells)
            .map(|c| {
                let (x, y) = (c % rw, c / rw);
                if chan[c] || x < 2 || y < 2 || x + 2 >= rw || y + 2 >= rh || z[c] <= sea {
                    return z[c];
                }
                if zs_read == "gauss" {
                    // isotropic: the 3 x 3 binomial [1 2 1] x [1 2 1] / 16 (sigma about 0.7 cell)
                    changed += 1;
                    let row = |c: usize| (z[c - 1] + 2.0 * z[c] + z[c + 1]) / 4.0;
                    return (row(c - rw) + 2.0 * row(c) + row(c + rw)) / 4.0;
                }
                let (gx, gy) = ((z[c + 1] - z[c - 1]) / 2.0, (z[c + rw] - z[c - rw]) / 2.0);
                let g = (gx * gx + gy * gy).sqrt();
                if g < 1e-9 {
                    return z[c];
                }
                let (tx, ty) = (-gy / g, gx / g);
                changed += 1;
                taps.iter().zip(wts).map(|(&s, w)| w * bil(x as f64 + s * tx, y as f64 + s * ty)).sum()
            })
            .collect();
        let d: Vec<f64> = out.iter().zip(z).map(|(a, b)| (a - b).abs()).collect();
        let mut ds = d.clone();
        ds.sort_by(f64::total_cmp);
        println!(
            "ZSREAD {tag} {zs_read}: {changed} of {cells} H1 cells smoothed (channel cells kept); |z_s' - z_s| p50 {:.2} p99 {:.2} max {:.2} m",
            ds[ds.len() / 2],
            ds[ds.len() * 99 / 100],
            ds[ds.len() - 1]
        );
        out
    } else {
        solve.zs.clone()
    };
    let read_field = (zs_read != "c1").then(|| {
        RegionField::new(
            face,
            REGIONAL,
            (sri0 as i64, srj0 as i64),
            rw,
            rh,
            zs_l.iter().zip(&solve.zb).map(|(s, b)| (s - b) as f32).collect(),
        )
    });
    let regional_read = read_field.as_ref().unwrap_or(&front.regional_field);

    // ---- B, r, S0 at L
    let mut bl = vec![0f64; w * h];
    par_fill(&mut bl, w, threads, &|p| base(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    let zb_field = RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, solve.zb.iter().map(|&v| v as f32).collect());
    let mut rl = vec![0f64; w * h];
    par_fill(&mut rl, w, threads, &|p| bl[p] - zb_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    let mut start = vec![0f64; w * h];
    par_fill(&mut start, w, threads, &|p| bl[p] + regional_read.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
    chain_f32("base.f32", &bl)?;
    chain_f32("zs.f32", &solve.zs)?;
    drop(bl);

    // ---- the ocean at L: cells at or below the sea level connected (8 neighbours) to an L cell of an H1 sea outlet or to the window's edge
    let ocean: Vec<bool> = {
        let mut o = vec![false; w * h];
        let mut stack: Vec<usize> = Vec::new();
        for c in 0..w * h {
            let (x, y) = (c % w, c / w);
            let hc = (y / L) * rw + x / L;
            let seed_cell = (solve.zb[hc] <= sea && !grid.is_edge(hc)) || x == 0 || y == 0 || x == w - 1 || y == h - 1;
            if start[c] <= sea && seed_cell {
                o[c] = true;
                stack.push(c);
            }
        }
        while let Some(c) = stack.pop() {
            let (x, y) = ((c % w) as i64, (c / w) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let n = ny as usize * w + nx as usize;
                    if !o[n] && start[n] <= sea {
                        o[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        o
    };
    {
        let at_sea = (0..w * h).filter(|&c| start[c] <= sea).count();
        let inland = (0..w * h).filter(|&c| start[c] <= sea && !ocean[c]).count();
        println!("KG sea at L: {at_sea} cells of S0 at or below the sea level, {inland} of them inland (not connected to an H1 sea outlet or the window's edge; {:.3} % of the window)", 100.0 * inland as f64 / (w * h) as f64);
    }

    // lead #160: the sea of the stages after KG-trunk (`--sea-mask`)
    let is_sea = |c: usize| if sea_mask == "connected" { ocean[c] } else { start[c] <= sea };
    // reading 24: H1 reads no budget (every H1 basin is crossed); the budget is S_KH's, after KH (reading 21, lead #168)
    let budget: Vec<f64> = Vec::new();

    // ---- H1: today's K6 and K6' (sills of S0)
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
    // K6' on the sills, and on the same node heights without the sills (each edge at the higher node): the basins the sills alone close
    let (h1, h1_nosill) = if base_today {
        let r0 = route_rules(grid, &solve.zs, outlet, eps, stream, 0.0, Some(&weight), RoundRules::ROUND);
        let f0 = priority_flood_with(grid, &solve.zs, outlet, eps, true);
        (h1_from(grid, &solve.zs, &f0, r0, outlet, &runoff, &evap, Some(&budget)), None)
    } else {
        let s = nodes_and_sills(&|c: usize| start[c], rw, rh, threads, None);
        let sill = |p: usize, d: usize, q: usize| s.at(p, d, q);
        let f = flood(grid, &s.h, &sill, outlet, eps);
        let r = route_sills(grid, &f, &sill, outlet, stream, &weight, true);
        let with = h1_from(grid, &s.h, &f, r, outlet, &runoff, &evap, Some(&budget));
        let hh = &s.h;
        let plain = |p: usize, _d: usize, q: usize| hh[p].max(hh[q]);
        let f2 = flood(grid, hh, &plain, outlet, eps);
        let r2 = route_sills(grid, &f2, &plain, outlet, stream, &weight, true);
        (with, Some(h1_from(grid, hh, &f2, r2, outlet, &runoff, &evap, Some(&budget))))
    };
    // spikes #147: `--probe-base` stops after H1: the channel steps (H1's channel cells to their receivers, in the region box) by direction, the straight runs along each
    // segment (consecutive steps in one direction, in steps of 814 m), and the 814 m fields z_b, z_b_in, z_s and the channel mask for the sheets
    if args.iter().any(|a| a == "--probe-base") {
        let in_core = |c: usize| {
            let (i, j) = (sri0 + c % rw, srj0 + c / rw);
            i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3
        };
        let (mut diag, mut axis) = (0usize, 0usize);
        let mut runs: Vec<usize> = Vec::new();
        for sg in &h1.hydro.segments {
            let (mut c, mut last, mut run) = (sg.up as usize, (0i64, 0i64), 0usize);
            while c != sg.down as usize {
                let r = h1.routing.receiver[c] as usize;
                let dir = ((r % rw) as i64 - (c % rw) as i64, (r / rw) as i64 - (c / rw) as i64);
                // outside H1's kept basins (a basin's flat floor is channel at H1 and runs straight by the flood's grade)
                if in_core(c) && h1.hydro.label[c] == NONE {
                    if dir.0 != 0 && dir.1 != 0 {
                        diag += 1;
                    } else {
                        axis += 1;
                    }
                }
                if h1.hydro.label[c] != NONE {
                    if run > 0 {
                        runs.push(run);
                    }
                    run = 0;
                    last = (0, 0);
                } else if dir == last {
                    run += 1;
                } else {
                    if run > 0 {
                        runs.push(run);
                    }
                    run = 1;
                    last = dir;
                }
                c = r;
            }
            if run > 0 {
                runs.push(run);
            }
        }
        runs.sort_unstable();
        let tot = (diag + axis).max(1);
        let long8 = runs.iter().filter(|&&r| r >= 8).count();
        let cells8: usize = runs.iter().filter(|&&r| r >= 8).sum();
        let all_steps: usize = runs.iter().sum();
        println!(
            "PROBE {tag} (base {base_form}, stages {base_stages}, breach cost {breach_cost}, drain rule {drain_rule}, breach max len {breach_max_len}): H1 channel steps in the box outside kept basins {tot}, diagonal {diag} ({:.1} %); straight runs: p50 {} p90 {} p99 {} max {} steps; runs of 8 steps (6.5 km) or more: {long8}, holding {:.1} % of the steps",
            100.0 * diag as f64 / tot as f64,
            runs[runs.len() / 2],
            runs[runs.len() * 9 / 10],
            runs[runs.len() * 99 / 100],
            runs.last().copied().unwrap_or(0),
            100.0 * cells8 as f64 / all_steps.max(1) as f64
        );
        let dump = |name: &str, v: &[f32]| -> Result<(), String> {
            let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-h1-{name}-{rw}x{rh}.f32")), b).map_err(|e| e.to_string())
        };
        dump("zb", &solve.zb.iter().map(|&v| v as f32).collect::<Vec<_>>())?;
        dump("zbin", &solve.zb_in.iter().map(|&v| v as f32).collect::<Vec<_>>())?;
        dump("zs", &solve.zs.iter().map(|&v| v as f32).collect::<Vec<_>>())?;
        dump("chan", &h1.hydro.channel.iter().map(|&c| if c { 1.0 } else { 0.0 }).collect::<Vec<_>>())?;
        return Ok(());
    }
    if let Some(d) = &chain_dir {
        // H1 rivers: the cells of the H1 channel segments the chain hands over (q at least EDGE_Q)
        let mut riv = vec![0u8; cells];
        for c in h1.trunk_cells.iter().flatten() {
            riv[*c as usize] = 1;
        }
        std::fs::write(d.join("h1-rivers.u8"), riv).map_err(|e| e.to_string())?;
    }
    // the basin census (lead #132a): kept H1 basins, their depth and size against GEN-145's artefact floor (fewer than 4 cells and shallower than 2 m) and the
    // 0.5 m pit limit, the channel-segment cells they hold by depth class, and (K6') those that the sills alone close
    {
        let hy = &h1.hydro;
        let mut seg_cells: Vec<u32> = Vec::new();
        for sg in &hy.segments {
            let mut c = sg.up;
            seg_cells.push(c);
            while c != sg.down {
                c = h1.routing.receiver[c as usize];
                seg_cells.push(c);
            }
        }
        let depth = |b: usize| hy.basins[b].spill_m - hy.basins[b].floor_m;
        let nb = hy.basins.len();
        let mut d: Vec<f64> = (0..nb).map(depth).collect();
        d.sort_by(f64::total_cmp);
        let mut sz: Vec<u32> = hy.basins.iter().map(|b| b.cells).collect();
        sz.sort();
        let q = |v: &[f64], f: f64| v.get(((v.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(0.0);
        let shallow2 = (0..nb).filter(|&b| depth(b) < 2.0).count();
        let small4 = hy.basins.iter().filter(|b| b.cells < 4).count();
        let shallow05 = (0..nb).filter(|&b| depth(b) < 0.5).count();
        let classes = [0.5, 2.0, 5.0, 10.0, 50.0, f64::INFINITY];
        let mut held = vec![0usize; classes.len()];
        for &c in &seg_cells {
            let b = hy.label[c as usize];
            if b != NONE {
                let dd = depth(b as usize);
                held[classes.iter().position(|&t| dd < t).expect("a class")] += 1;
            }
        }
        println!(
            "KG basin census: {nb} kept H1 basins; depth spill - floor p10 {:.1} p50 {:.1} p90 {:.1} m; cells p50 {} p90 {} max {}; shallower than 2 m {shallow2} (of them under 0.5 m {shallow05}), fewer than 4 cells {small4}",
            q(&d, 0.1),
            q(&d, 0.5),
            q(&d, 0.9),
            sz.get(sz.len() / 2).copied().unwrap_or(0),
            sz.get(sz.len() * 9 / 10).copied().unwrap_or(0),
            sz.last().copied().unwrap_or(0)
        );
        println!(
            "KG basin census: channel-segment cells in kept basins by basin depth: < 0.5 m {}, 0.5-2 m {}, 2-5 m {}, 5-10 m {}, 10-50 m {}, >= 50 m {} (of {} segment cells)",
            held[0], held[1], held[2], held[3], held[4], held[5], seg_cells.len()
        );
        if let Some(ns) = &h1_nosill {
            let nh = &ns.hydro;
            let nbc = nh.label.iter().filter(|&&b| b != NONE).count();
            let sill_only: Vec<usize> = (0..nb).filter(|&b| nh.label[hy.basins[b].floor_cell as usize] == NONE).collect();
            let so: HashSet<usize> = sill_only.iter().copied().collect();
            let so_cells = hy.label.iter().filter(|&&b| b != NONE && so.contains(&(b as usize))).count();
            let so_seg =
                seg_cells.iter().filter(|&&c| hy.label[c as usize] != NONE && so.contains(&(hy.label[c as usize] as usize))).count();
            let in_seg = seg_cells.iter().filter(|&&c| hy.label[c as usize] != NONE).count();
            println!(
                "KG basin census (K6'): without the sills (each edge at the higher node height) {} kept basins, {nbc} basin cells; {} of the {nb} K6' basins ({so_cells} cells) have their floor in no basin there: the sills alone close them; they hold {so_seg} of the {in_seg} segment cells in kept basins",
                nh.basins.len(),
                sill_only.len()
            );
        }
    }
    println!(
        "KG centreline: {}",
        if low {
            "lowest path (Dijkstra, step cost length x (1 + height over the corridor's minimum / 10 m))"
        } else {
            "maximin corridor route (BUILD-057's measure)"
        }
    );
    println!(
        "KG base H1: {} ({} segments, {} kept basins)",
        if base_today { "today's K6" } else { "K6' on the sills of S0 (SPIKE-127)" },
        h1.hydro.segments.len(),
        h1.hydro.basins.len()
    );

    let (bi0, bi1, bj0, bj1) = ((region.0 * L) as i64, ((region.1 + 1) * L) as i64, (region.2 * L) as i64, ((region.3 + 1) * L) as i64);
    let in_box: Vec<bool> = (0..w * h)
        .map(|c| {
            let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
        })
        .collect();
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
    let sea_outlet = |c: u32| solve.zb[c as usize] <= sea && !grid.is_edge(c as usize);
    let (reaches, edge_ends) = h1_reaches(&h1, grid, &sea_outlet, &budget);
    let order = strahler(&h1);
    {
        let segs = &h1.hydro.segments;
        let (mut sc, mut lab, mut full) = (0usize, 0usize, 0usize);
        for s in segs {
            let mut c = s.up;
            let (mut n_, mut l_) = (1usize, usize::from(h1.hydro.label[c as usize] != NONE));
            while c != s.down {
                c = h1.routing.receiver[c as usize];
                n_ += 1;
                l_ += usize::from(h1.hydro.label[c as usize] != NONE);
            }
            sc += n_;
            lab += l_;
            full += usize::from(l_ + 1 >= n_);
        }
        let basin_cells = h1.hydro.label.iter().filter(|&&b| b != NONE).count();
        println!(
            "KG segments: {} ({sc} cells, {lab} in kept basins, {full} segments inside one); kept-basin cells {basin_cells} of {cells}",
            segs.len()
        );
    }
    println!(
        "KG H1 trunk reaches: {} ({} end on the window's edge; {} lake inlets, {} lake spills)",
        reaches.len(),
        edge_ends,
        reaches.iter().filter(|r| r.down_kind == NodeKind::LakeInlet).count(),
        reaches.iter().filter(|r| r.up_kind == NodeKind::LakeSpill).count()
    );
    // ================================================================ the RG chain (explorer #166, G1f plumbing; no tuning, no judgement)
    // switches: --kh c1 | h2 | h4 | h4c, default h4c (spikes #139; KS starts from KH(z_s) + r: h2 the triangulation, reading 19, spikes #137; h4 Clough-Tocher on its split and h4c, h4 clamped to
    // its quad's corners, reading 20, spikes #138 and #139); --ks off | ks0 | ks1 (default ks1); --skip a,b,... of kg-trunk, basins, kg-trib, kv, kc;
    // --regions 1 | 2
    // reading 25 (lead #168b): KH is h5 by default
    let kh = value(&args, "--kh").unwrap_or("h5");
    // spikes #147a: `h5d` is h5 plus h4c within 2 L cells of the D8 diagonal steps C1 dams (smoothstep to C1 by 4 L cells), the owner's decision-7 fallback
    if !["c1", "h2", "h4", "h4c", "h5", "h5d"].contains(&kh) {
        return Err(format!("--kh c1 | h2 | h4 | h4c | h5 | h5d, not '{kh}'"));
    }
    let ks_mode = value(&args, "--ks").unwrap_or("ks1");
    if !["off", "ks0", "ks1"].contains(&ks_mode) {
        return Err(format!("--ks off | ks0 | ks1, not '{ks_mode}'"));
    }
    let skip: Vec<&str> = value(&args, "--skip").map(|v| v.split(',').collect()).unwrap_or_default();
    // crusader #168 (DEC crusader-17), reading 34 (owner, 2026-11-19): --trib-on post (default) re-places the tributaries on the band6 output, the surface basins-2, KV, KC and the
    // gate see (the first pass's routing still drives KS1); pre places them once, on KS0 (reading 14)
    let trib_on = value(&args, "--trib-on").unwrap_or("post");
    if !["pre", "post"].contains(&trib_on) {
        return Err(format!("--trib-on pre | post, not '{trib_on}'"));
    }
    for s in &skip {
        if !["kg-trunk", "basins", "kg-trib", "kv", "kc"].contains(s) {
            return Err(format!("--skip: kg-trunk, basins, kg-trib, kv, kc; not '{s}'"));
        }
    }
    let on = |s: &str| !skip.contains(&s);
    let regions: usize = value(&args, "--regions").map_or(Ok(1), str::parse).map_err(|_| "--regions 1 | 2")?;
    let mut timing: Vec<(String, f64, u64, u64)> = Vec::new();
    let mut stamp = |name: &str, secs: f64| {
        let (peak, now) = memory_mib();
        println!("RG stage {name}: {secs:.1} s; peak working set {peak} MiB, now {now} MiB");
        timing.push((name.to_string(), secs, peak, now));
    };
    stamp("setup (S0, ocean, H1 base)", t_all.elapsed().as_secs_f64());
    let n_l = w * h;
    let dx_l = grid.dx() / L as f64;
    let gl = Grid::new(w, h, dx_l);
    let parent = |p: usize| ((p / w) / L) * rw + (p % w) / L;
    // the regions: one, or the window cut at a tile column into two; a pointwise field is evaluated per region and stitched (the partition run)
    // the cut sits on a global tile boundary (the window's origin need not), so a tile lies wholly in one region
    let half = ((((x0 as u64) + (w / 2) as u64) / TILE) * TILE - x0 as u64) as usize;
    let region_of = |p: usize| usize::from(regions == 2 && p % w >= half);
    let per_region = |f: &(dyn Fn(usize) -> f64 + Sync)| -> Vec<f64> {
        let mut out = vec![0f64; n_l];
        for r in 0..regions.max(1) {
            let cells: Vec<usize> = (0..n_l).filter(|&p| region_of(p) == r).collect();
            let vals: Vec<f64> = cells.iter().map(|&p| f(p)).collect();
            for (&p, v) in cells.iter().zip(vals) {
                out[p] = v;
            }
        }
        out
    };

    // ---- KH: the hand-over (c1: C1(z_s) + r, the start surface S0 itself)
    // ---- HAND at H1 (z_s over the channel cell the 814 m flow path reaches)
    let t = Instant::now();
    let mut ch = vec![u32::MAX; cells];
    for &c in &h1.routing.order {
        let c = c as usize;
        let r = h1.routing.receiver[c];
        ch[c] = if h1.hydro.channel[c] || r == NONE { c as u32 } else { ch[r as usize] };
    }
    let hand: Vec<f32> = (0..cells).map(|c| (solve.zs[c] - solve.zs[ch[c] as usize]).max(0.0) as f32).collect();
    let hand_field = RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, hand);
    let hand_l = per_region(&|p| hand_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)).max(0.0));
    drop(hand_field);
    // KS with the budget and reading 17: lowering = min(max(0, r - band), budget), never to the sea level + 0.5 m or below; a clamp is a residual (cell, stage, needed, available)
    // reading 24 (lead #170 item 5): no H1 basin is a lake in the prototype, so KS, KV and KC protect no H1 lake water; `--h1-lakes keep` is the former form (region_front's lakes)
    let h1_lakes = value(&args, "--h1-lakes").unwrap_or("off");
    if !["off", "keep"].contains(&h1_lakes) {
        return Err(format!("--h1-lakes off | keep, not '{h1_lakes}'"));
    }
    let protected_h1: Vec<bool> = {
        let mut p = vec![false; cells];
        if h1_lakes == "keep" {
            for &(c, _) in &front.lakes.water {
                p[c as usize] = true;
            }
        }
        p
    };
    // ---- KH (spikes #137, reading 19): the surface KS starts from. c1: S0 itself; h2: KH(z_s) + r, KH the flow-diagonal triangulation of z_s (planet_generators::bake::kh). S0 stays
    // the reference of the ocean, the sills, the budget and the measures (explorer, 2026-11-17).
    let t = Instant::now();
    let kh_form = kh;
    // spikes #147: `--census` counts, per stage surface, the depressions the selective drain leaves over the budget both ways (band6's rule), and traces band6's
    let census = args.iter().any(|a| a == "--census");
    let mut kh_only: Option<Vec<f64>> = None;
    let ks_base: Vec<f64> = if kh != "c1" {
        use planet_generators::bake::kh;
        // KH's routing of z_s at 814 m: reading 12's outlets (the edge, and cells at or below the sea connected to it or to a K5 sea outlet)
        let mut ol: Vec<bool> = (0..cells).map(|c| grid.is_edge(c)).collect();
        let mut stack: Vec<usize> = (0..cells).filter(|&c| solve.zs[c] <= sea && (grid.is_edge(c) || outlet[c])).collect();
        for &c in &stack {
            ol[c] = true;
        }
        while let Some(c) = stack.pop() {
            for d in 0..8 {
                if let Some(q) = grid.neighbour(c, d) {
                    if !ol[q] && solve.zs[q] <= sea {
                        ol[q] = true;
                        stack.push(q);
                    }
                }
            }
        }
        let (recv, area) = kh::routing(grid, &solve.zs, &ol);
        let (split, st) = kh::split(&solve.zs, rw, rh, &|c| (recv[c] != NONE).then(|| recv[c] as usize), &|c| area[c]);
        let lr = L as f64;
        let grad = kh::gradients(&solve.zs, rw, rh);
        let step = |p: usize, q: usize| recv[p] == q as u32 || recv[q] == p as u32;
        // h5 (reading 25, spikes' spike/drain-aplus 91b38d6f): h4c on the channel zone (H1's channel cells and their 8 neighbours, reading 27),
        // C1(z_s) elsewhere, blended by kh::zone_weight over one 814 m cell
        // reading 27 (lead #170 item 2): the zone is H1's channel cells (GEN-145: runoff-weighted area of at least 75 km2, then downstream) and their 8 neighbours, not KH's
        // own D8 area on z_s; the channel mask goes in as a 0/1 field
        let zone = (kh_form == "h5" || kh_form == "h5d").then(|| {
            let mask: Vec<f64> = h1.hydro.channel.iter().map(|&c| if c { 1.0 } else { 0.0 }).collect();
            kh::channel_zone(&mask, rw, rh, 0.5)
        });
        let zs_field = RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, zs_l.iter().map(|&v| v as f32).collect());
        // h5d: the quads whose D8 diagonal step C1 dams (C1 at the quad's centre over the step line by more than 1 m); C1 at the centre is the mean of the four L cells there
        let dams = (kh_form == "h5d").then(|| {
            let rcv = |c: usize| (recv[c] != NONE).then(|| recv[c] as usize);
            let diag = |q: usize| -> Option<(usize, usize)> {
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
            let centre = |q: usize| {
                let (cx, cy) = ((q % rw * L + L - 1) as i64, (q / rw * L + L - 1) as i64);
                [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .iter()
                    .map(|&(dx, dy)| f64::from(zs_field.at(dir_of(x0 + cx + dx, y0 + cy + dy))))
                    .sum::<f64>()
                    / 4.0
            };
            let steps = kh::dammed_steps(&solve.zs, rw, rh, &diag, &centre, 1.0);
            let all = (0..cells).filter(|&q| q % rw + 1 < rw && q / rw + 1 < rh && diag(q).is_some()).count();
            println!(
                "RG KH h5d: C1 dams {} of {all} D8 diagonal steps (by over 1 m at the quad's centre)",
                steps.iter().filter(|s| s.is_some()).count()
            );
            steps
        });
        // readings 17 and 19 (lead #170 item 3): S_KH = KH(z_s) + r', the band floored so that a cell whose KH lies above the sea + 0.5 m stays there (as ks_kh.rs)
        let floor = sea + 0.5;
        let band = |k: f64, r: f64| if k > floor { r.max(floor - k) } else { r };
        let kval = |p: usize| {
            let (fx, fy) = (((p % w) as f64 - (lr - 1.0) / 2.0) / lr, ((p / w) as f64 - (lr - 1.0) / 2.0) / lr);
            match kh_form {
                "h2" => kh::read(&solve.zs, rw, rh, &split, fx, fy),
                "h4" => kh::read_ct(&solve.zs, rw, rh, &split, &grad, &step, fx, fy).0,
                "h5" | "h5d" => {
                    let c1 = f64::from(zs_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)));
                    let h4c = kh::read_ct_clamped(&solve.zs, rw, rh, &split, &grad, &step, fx, fy).0;
                    let mut wgt = kh::zone_weight(zone.as_ref().expect("h5's zone"), rw, rh, fx, fy);
                    if let Some(s) = &dams {
                        wgt = wgt.max(kh::dam_weight(s, rw, rh, fx, fy, lr, 2.0, 4.0));
                    }
                    // lead #161: `--kh-lower-only`: h5 only lowers C1 toward h4c (never raises it), as h3 never raises
                    if kh_lower_only {
                        c1 + wgt * (h4c - c1).min(0.0)
                    } else {
                        c1 + wgt * (h4c - c1)
                    }
                }
                _ => kh::read_ct_clamped(&solve.zs, rw, rh, &split, &grad, &step, fx, fy).0,
            }
        };
        let at = |p: usize| {
            let k = kval(p);
            k + band(k, rl[p])
        };
        if let Some(z) = &zone {
            println!("RG KH h5: h4c on {} zone cells of 814 m, C1 elsewhere", z.iter().filter(|&&x| x).count());
        }
        if census {
            let mut k = vec![0f64; n_l];
            par_fill(&mut k, w, threads, &|p| kval(p));
            kh_only = Some(k);
        }
        // the run's read on `threads` threads; tile-locality: the same read over two regions cut at a tile column, and on one thread
        let mut one = vec![0f64; n_l];
        par_fill(&mut one, w, threads, &at);
        let two: Vec<f64> = {
            let mut out = vec![0f64; n_l];
            for r in 0..2 {
                for p in (0..n_l).filter(|&p| usize::from(p % w >= half) == r) {
                    out[p] = at(p);
                }
            }
            out
        };
        let mut serial = vec![0f64; n_l];
        for p in 0..n_l {
            serial[p] = at(p);
        }
        let differ2 = (0..n_l).filter(|&p| one[p].to_bits() != two[p].to_bits()).count();
        let differ1 = (0..n_l).filter(|&p| one[p].to_bits() != serial[p].to_bits()).count();
        let land = (0..n_l).filter(|&p| start[p] > sea).count();
        let (mut lo10, mut hi10) = (0usize, 0usize);
        for p in (0..n_l).filter(|&p| start[p] > sea) {
            lo10 += usize::from(start[p] - one[p] > 10.0);
            hi10 += usize::from(one[p] - start[p] > 10.0);
        }
        println!(
            "RG KH {kh_form}: quads split along a D8 diagonal step {} (both diagonals {}), along the higher pair {}; two regions against one: {differ2} cells differ, one thread against the run: {differ1}; against S0 on {land} land cells: lower over 10 m {lo10}, higher over 10 m {hi10}",
            st.by_step, st.both, st.by_pair
        );
        let b: Vec<u8> = split.clone();
        std::fs::write(out_dir.join(format!("{tag}-kh-split-{rw}x{rh}.u8")), b).map_err(|e| e.to_string())?;
        // lead #158 (spikes #157): with `--ks-connect 4`, KH's h5 closes its own corner-only links in the channel zone: a 2 x 2 block whose diagonal pair lies more than 1 m
        // below both edge cells gets the lower edge cell lowered to the pair's mean. Claims are read from the unchanged read and applied as a minimum (order-free, CON-14).
        if KS_CONNECT.load(std::sync::atomic::Ordering::Relaxed) {
            if let Some(zr) = &zone {
                let mut want = vec![f64::INFINITY; n_l];
                for y in 0..h - 1 {
                    for x in 0..w - 1 {
                        let (a, b, c, d) = (y * w + x, y * w + x + 1, (y + 1) * w + x, (y + 1) * w + x + 1);
                        if !(zr[parent(a)] || zr[parent(d)]) {
                            continue;
                        }
                        // a-d diagonal low against b and c, or b-c low against a and d
                        for (p, q, e1, e2) in [(a, d, b, c), (b, c, a, d)] {
                            if one[e1].min(one[e2]) - one[p].max(one[q]) > 1.0 {
                                let e = if one[e1] < one[e2] || (one[e1] == one[e2] && e1 < e2) { e1 } else { e2 };
                                want[e] = want[e].min(0.5 * (one[p] + one[q]));
                            }
                        }
                    }
                }
                let cut = (0..n_l).filter(|&e| want[e] < one[e]).count();
                for e in 0..n_l {
                    one[e] = one[e].min(want[e]);
                }
                println!("RG KH h5 connect 4 (lead #158): {cut} edge cells lowered in the channel zone");
            }
        }
        one
    } else {
        start.clone()
    };
    chain_f32("kh.f32", &ks_base)?;
    stamp(&format!("KH ({kh})"), t.elapsed().as_secs_f64());
    // ---- the budget (G0 contracts section 6; BUILD-060's field, the one river-gate reads), per L cell and per H1 cell. Reading 21 (owner decision 3, 2026-11-17, lead #168):
    // budgets and the L stages' deltas are against S_KH = KH(z_s) + r' (ks_base), not S0; S0 stays the reference of the ocean and the sea tests; KH against S0 is its own field.
    let t = Instant::now();
    // reading 28 (lead #170 item 7): the budget's size field relief10 on S0; the lowering it limits is measured against S_KH (reading 21)
    let budget_l = planet_cache::river_gate_graph::budget_field(&start, w, h, L, dx_l, 25_000.0);
    let budget: Vec<f64> = (0..cells).map(|c| budget_l[((c / rw) * L) * w + (c % rw) * L]).collect();
    if args.iter().any(|a| a == "--dump-s0") {
        let b: Vec<u8> = ks_base.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(out_dir.join(format!("{tag}-skh-{w}x{h}.f64")), b).map_err(|e| e.to_string())?;
    }
    // spikes #147a: `--stop-after-kh` writes S0 and S_KH (with --dump-s0) and stops: the KH sheets' inputs without the rest of the chain
    if args.iter().any(|a| a == "--stop-after-kh") {
        let b: Vec<u8> = start.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(out_dir.join(format!("{tag}-s0-{w}x{h}.f64")), b).map_err(|e| e.to_string())?;
        // SPIKE-139: `--dump-c1` also writes C1(z_s) at L (S0 - r, f32), z_s and H1's channel mask at 814 m (f32), for the washboard measure
        if args.iter().any(|a| a == "--dump-c1") {
            let b: Vec<u8> = (0..n_l).flat_map(|p| ((start[p] - rl[p]) as f32).to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-c1-{w}x{h}.f32")), b).map_err(|e| e.to_string())?;
            let b: Vec<u8> = solve.zs.iter().flat_map(|&v| (v as f32).to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-zs-{rw}x{rh}.f32")), b).map_err(|e| e.to_string())?;
            let b: Vec<u8> = h1.hydro.channel.iter().flat_map(|&c| (if c { 1.0f32 } else { 0.0 }).to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-chan-{rw}x{rh}.f32")), b).map_err(|e| e.to_string())?;
            // the same read of the base before stage 1 (C1(z_b) = B - r) and after it (C1(z_b_in), the base K5 starts from)
            let b: Vec<u8> =
                (0..n_l).flat_map(|p| (zb_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)) as f32).to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-c1b-{w}x{h}.f32")), b).map_err(|e| e.to_string())?;
            let zbin_field =
                RegionField::new(face, REGIONAL, (sri0 as i64, srj0 as i64), rw, rh, solve.zb_in.iter().map(|&v| v as f32).collect());
            let b: Vec<u8> =
                (0..n_l).flat_map(|p| (zbin_field.at(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64)) as f32).to_le_bytes()).collect();
            std::fs::write(out_dir.join(format!("{tag}-c1bin-{w}x{h}.f32")), b).map_err(|e| e.to_string())?;
        }
        println!("RG stopped after KH (--stop-after-kh)");
        return Ok(());
    }
    stamp("budget field", t.elapsed().as_secs_f64());
    let ks = |band: &[f64]| -> (Vec<f64>, Vec<(usize, f64, f64)>) {
        let mut z = ks_base.clone();
        let mut res = Vec::new();
        for p in 0..n_l {
            if is_sea(p) || protected_h1[parent(p)] {
                continue;
            }
            let want = (rl[p] - band[p]).max(0.0);
            let avail = budget_l[p].min((ks_base[p] - (sea + 0.5)).max(0.0));
            let u = want.min(avail);
            z[p] = ks_base[p] - u;
            if want > avail + 1e-9 {
                res.push((p, want, avail));
            }
        }
        (z, res)
    };
    let (ks0, res0) = if ks_mode == "off" { (ks_base.clone(), Vec::new()) } else { ks(&hand_l) };
    drop(hand_l);
    println!(
        "RG KS0 ({}): {} cells lowered, {} residuals",
        if ks_mode == "off" { "off: S0" } else { "S0 - min(max(0, r - C1(HAND)), budget)" },
        (0..n_l).filter(|&p| ks0[p] < start[p]).count(),
        res0.len()
    );
    stamp("KS0", t.elapsed().as_secs_f64());

    // ---- KG-trunk (spikes' code, G1c) on KS0
    let t = Instant::now();
    // KG-trib's uniform runoff (m/s), q = a_run r; lead #180: the H1 reaches' a_run is a runoff-weighted area q / r, as KG-trib's L reaches carry it
    let land_h1: Vec<f64> = (0..cells).filter(|&c| solve.zs[c] > 0.0).map(|c| f64::from(runoff[c])).collect();
    let runoff_mean = land_h1.iter().sum::<f64>() / land_h1.len().max(1) as f64;
    let window = (x0 as u64, y0 as u64, w, h, rw);
    let mut g = if on("kg-trunk") {
        build_graph(&h1, &reaches, grid, &start, &ks_base, &ks0, &budget, window, sea, bar, &order, threads, low, &ocean, runoff_mean).0
    } else {
        Graph { body: BODY, nodes: Vec::new(), reaches: Vec::new(), basins: Vec::new() }
    };
    println!("RG KG-trunk: {} nodes, {} reaches", g.nodes.len(), g.reaches.len());
    stamp("KG-trunk", t.elapsed().as_secs_f64());

    // ---- the L stages per domain (#166a gap 2): one domain (the window), or two regions cut at a tile column, each run on its own window: its core and a halo of 16 H1 cells
    // (13 km, plan section 3.2), the window's border its exits except the entry cells, which carry the A_d of the H1 cell outside that drains into them (crusader's G1d rule).
    // Each domain keeps the graph items its core's tiles own and its core's cells; the stitched result is the regions' answer.
    let gid_l = |c: usize| gid(x0 as u64 + (c % w) as u64, y0 as u64 + (c / w) as u64);
    let tile_l = |c: usize| Domain::Tile {
        face: face.0,
        ti: ((x0 as u64 + (c % w) as u64) / TILE) as u32,
        tj: ((y0 as u64 + (c / w) as u64) / TILE) as u32,
    };
    let sea_l: Vec<bool> = (0..n_l).map(is_sea).collect();
    println!(
        "RG sea mask ({sea_mask}): {} sea cells of {} at or below the sea level",
        sea_l.iter().filter(|&&s| s).count(),
        (0..n_l).filter(|&c| start[c] <= sea).count()
    );
    let to_local = |id: u64| -> Option<usize> {
        let (x, y) = gxy(id);
        let (lx, ly) = (x as i64 - x0, y as i64 - y0);
        (lx >= 0 && ly >= 0 && (lx as usize) < w && (ly as usize) < h).then(|| ly as usize * w + lx as usize)
    };
    // the region of a tile-owned item (by the tile's column), None for the landmass
    let owner_region = |d: Domain| -> Option<usize> {
        match d {
            Domain::Landmass => None,
            Domain::Tile { ti, .. } => Some(region_of(((u64::from(ti) * TILE) as i64 - x0).max(0) as usize)),
        }
    };
    let l_trunks: Vec<GReach> = g.reaches.iter().filter(|r| r.level == Level::L).cloned().collect();
    let on_trunk: HashSet<usize> = l_trunks.iter().flat_map(|r| r.vertices.iter().filter_map(|&c| to_local(c))).collect();
    // the stitched KS, KV and KC outputs (lead #168 item 2, memory): made after the first domain's KV/KC, not before; with one region the domain's own fields are moved in
    let mut outs: Option<(Vec<f64>, Vec<f64>, Vec<f64>)> = None;
    // the cross-slope pass's cut per window cell (DEC crusader-19's sheet; empty when off)
    let mut cross_out: Vec<f32> = Vec::new();
    // reading 18: one region's accumulated L trunk attributes, written into the stitched graph
    let mut acc_trunks: HashMap<u64, planet_generators::bake::river_graph::Attrs> = HashMap::new();
    let mut res_ks: Vec<(usize, f64, f64)> = Vec::new();
    let mut res_kvkc: Vec<planet_generators::bake::river_graph::Residual> = Vec::new();
    // KC's patches, (cells, volume m3), for `kc-patches.csv` (lead #172; with two regions a halo's patch is counted in each domain)
    let mut kc_patches: Vec<(usize, f64)> = Vec::new();
    let mut extra = Graph { body: BODY, nodes: Vec::new(), reaches: Vec::new(), basins: Vec::new() };
    let (mut t_basins, mut t_trib, mut t_ks1, mut t_kvkc) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut landmass_seen: HashMap<u64, String> = HashMap::new();
    let mut landmass_conflicts = 0usize;
    for d in 0..regions.max(1) {
        // the domain's window: its core's H1 cells and 16 rings around them, inside the whole window
        let core_h1: Vec<bool> = (0..cells).map(|c| region_of((c % rw) * L) == d).collect();
        let mut win_h1 = core_h1.clone();
        if regions == 2 {
            for _ in 0..16 {
                let prev = win_h1.clone();
                for c in 0..cells {
                    if !prev[c] {
                        let (cx, cy) = ((c % rw) as i64, (c / rw) as i64);
                        win_h1[c] = (-1..=1i64).any(|dy| {
                            (-1..=1i64).any(|dx| {
                                let (x, y) = (cx + dx, cy + dy);
                                x >= 0 && y >= 0 && (x as usize) < rw && (y as usize) < rh && prev[y as usize * rw + x as usize]
                            })
                        });
                    }
                }
            }
        }
        let active: Vec<bool> = (0..n_l).map(|p| win_h1[parent(p)]).collect();
        let core: Vec<bool> = (0..n_l).map(|p| core_h1[parent(p)]).collect();
        let border: Vec<bool> = (0..n_l)
            .map(|p| {
                let (x, y) = ((p % w) as i64, (p / w) as i64);
                active[p]
                    && (-1..=1i64).any(|dy| {
                        (-1..=1i64).any(|dx| {
                            let (a, b) = (x + dx, y + dy);
                            a < 0 || b < 0 || a as usize >= w || b as usize >= h || !active[b as usize * w + a as usize]
                        })
                    })
            })
            .collect();
        let mut seed = vec![0.0f64; n_l];
        let mut entry = vec![false; n_l];
        if regions == 2 {
            for q in 0..cells {
                let to = h1.routing.receiver[q];
                if win_h1[q] || to == NONE || !win_h1[to as usize] {
                    continue;
                }
                let (tx, ty) = ((to as usize % rw) * L, (to as usize / rw) * L);
                let mut best: Option<usize> = None;
                for y in ty..ty + L {
                    for x in tx..tx + L {
                        let p = y * w + x;
                        if !border[p] || sea_l[p] || on_trunk.contains(&p) {
                            continue;
                        }
                        let touches = (-1..=1i64).any(|dy| {
                            (-1..=1i64).any(|dx| {
                                let (a, b) = (x as i64 + dx, y as i64 + dy);
                                a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h && parent(b as usize * w + a as usize) == q
                            })
                        });
                        if touches && best.is_none_or(|b| start[p] < start[b] || (start[p] == start[b] && p < b)) {
                            best = Some(p);
                        }
                    }
                }
                if let Some(p) = best {
                    entry[p] = true;
                    seed[p] += h1.d8[q];
                }
            }
        }
        let exit: Vec<bool> = (0..n_l).map(|p| border[p] && !sea_l[p] && !entry[p]).collect();
        let outlet_d: Vec<bool> = (0..n_l).map(|p| active[p] && (sea_l[p] || exit[p])).collect();
        let grid_d = Grid::new(w, h, dx_l).with_active(active.clone());
        let mut kept = vec![false; n_l];
        // lead #176a: each kept basin's rim (the cells beside its cells) at its recorded spill, a floor KV's cut and KC's breach keep (empty: no floor)
        let mut rim_floor: Vec<f64> = Vec::new();
        for p in 0..n_l {
            kept[p] = protected_h1[parent(p)];
        }
        let mut gd = Graph { body: BODY, nodes: g.nodes.clone(), reaches: l_trunks.clone(), basins: Vec::new() };
        let mut first_nodes: Vec<Node> = Vec::new();
        // basins (crusader's G1e) on KS0, the L trunk reaches as spill receivers
        let t = Instant::now();
        if on("basins") && !trib_once {
            use planet_generators::bake::basins_rg::BasinParams;
            use planet_generators::bake::river_graph::Represented;
            // crusader's produce_ids (spike/g1d c75c8641): reach vertices read and every anchor and id written as global ids; Produced.cells in grid cells
            let ids = planet_generators::bake::basins_rg::CellIds { gid: &gid_l, local: &to_local };
            {
                let (pk, nw) = memory_mib();
                println!("RG mem before basin pass 1: peak {pk} MiB, now {nw} MiB");
            }
            let pb =
                planet_generators::bake::basins_rg::produce_ids(&grid_d, &ks0, &outlet_d, &l_trunks, &tile_l, &BasinParams::l(BODY), &ids);
            {
                let (pk, nw) = memory_mib();
                println!("RG mem after basin pass 1: peak {pk} MiB, now {nw} MiB");
            }
            for b in pb.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
                for &c in &pb.cells[&b.id] {
                    kept[c] = true;
                }
            }
            // the first pass's nodes enter the graph only when no second pass follows (crusader: the final graph takes the second pass's nodes)
            first_nodes = pb.nodes.clone();
            gd.basins.extend(pb.basins.iter().cloned());
            println!(
                "RG domain {d} basins: {} L basins ({} kept), {} unresolved spills",
                pb.basins.len(),
                pb.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count(),
                pb.unresolved.len()
            );
        }
        t_basins += t.elapsed().as_secs_f64();
        // KG-trib (crusader's G1d extractor-1) on KS0, A_crit 1 km2
        let t = Instant::now();
        let mut fine: Option<(Vec<u32>, Vec<bool>)> = None;
        // the first pass's tributary reaches and nodes (crusader #168: --trib-on post replaces them)
        let mut trib_first: (BTreeSet<u64>, BTreeSet<u64>) = (BTreeSet::new(), BTreeSet::new());
        // lead #162: --trib-once's routing of KS0 for KS1, and the running-minimum bed of its channels
        let mut bed_min: Option<Vec<f64>> = None;
        if trib_once && trib_on == "post" {
            use planet_generators::bake::route::route_rules_filled;
            let outlet: Vec<bool> = (0..n_l).map(|c| active[c] && (sea_l[c] || exit[c] || on_trunk.contains(&c))).collect();
            let (r, _filled) = route_rules_filled(&grid_d, &ks0[..], &outlet, 1.0e-3, 1, 0.0, None, RoundRules::ROUND);
            let mut a: Vec<f64> = (0..n_l).map(|c| if active[c] && !outlet[c] { dx_l * dx_l + seed[c] } else { 0.0 }).collect();
            let mut channel = vec![false; n_l];
            let mut bm: Vec<f64> = ks0.clone();
            for &c in r.order.iter().rev() {
                let c = c as usize;
                let to = r.receiver[c];
                if active[c] && !outlet[c] && !protected_h1[parent(c)] && a[c] >= 1.0e6 {
                    channel[c] = true;
                }
                if to != NONE {
                    let to = to as usize;
                    a[to] += a[c];
                    if channel[c] {
                        if active[to] && !outlet[to] && !protected_h1[parent(to)] {
                            channel[to] = true;
                        }
                        bm[to] = bm[to].min(bm[c]);
                    }
                }
            }
            println!(
                "RG domain {d} trib-once (lead #162): KS1 on a routing of KS0, {} channel cells; no basins pass 1, no KG-trib pass 1",
                channel.iter().filter(|&&x| x).count()
            );
            bed_min = Some(bm);
            fine = Some((r.receiver, channel));
        }
        if on("kg-trib") && !(trib_once && trib_on == "post") {
            use planet_generators::bake::kg_trib::{accumulate_trunks, extract, Input, Params, Trunk};
            let trunks: Vec<Trunk> = l_trunks
                .iter()
                .filter_map(|r| {
                    let v: Option<Vec<usize>> = r.vertices.iter().map(|&c| to_local(c)).collect();
                    let down = g.nodes.iter().find(|x| x.id == r.down)?.clone();
                    v.map(|vertices| Trunk { id: r.id, vertices, attrs: r.attrs.clone(), down })
                })
                .collect();
            let inp = Input {
                grid: &grid_d,
                z: &ks0,
                sea: &sea_l,
                exit: &exit,
                trunks: &trunks,
                kept: &kept,
                seed_ext: &seed,
                tile: &tile_l,
                gid: &gid_l,
                keep_fields: false,
            };
            let ex = extract(&inp, &Params { mfd_init: false, ..Params::plan(BODY, runoff_mean) });
            // only what the pass adds: a tributary that ends at a trunk's last vertex returns the trunk's own down node, which must stay (numerics review of f5adb6d8)
            let had: HashSet<u64> = gd.nodes.iter().map(|x| x.id).collect();
            trib_first =
                (ex.reaches.iter().map(|r| r.id).collect(), ex.nodes.iter().map(|x| x.id).filter(|id| !had.contains(id)).collect());
            gd.nodes.extend(ex.nodes.iter().cloned());
            gd.reaches.extend(ex.reaches.iter().cloned());
            // reading 18 (lead #170 item 4): the landmass pass, the L trunks' areas and discharge (q_w, KV's width) accumulated at L from their tributaries and hillslope
            let trunk_ids: BTreeSet<u64> = l_trunks.iter().map(|r| r.id).collect();
            let inflow = |id: u64| to_local(id).map_or((0.0, 0.0), |c| (dx_l * dx_l + ex.d8[c], dx_l * dx_l + ex.d8_no_ext[c]));
            let acc = accumulate_trunks(&mut gd, &trunk_ids, &inflow, runoff_mean);
            println!(
                "RG domain {d} reading 18: {} trunk reaches accumulated at L, {} unordered, {} heads over their H1 seed",
                acc.reaches, acc.unordered, acc.heads_over_seed
            );
            if regions <= 1 {
                acc_trunks = gd.reaches.iter().filter(|r| trunk_ids.contains(&r.id)).map(|r| (r.id, r.attrs.clone())).collect();
            }
            println!(
                "RG domain {d} KG-trib: {} reaches, {} nodes, {} open ends, {} entry cells",
                ex.reaches.len(),
                ex.nodes.len(),
                ex.open_ends,
                entry.iter().filter(|&&e| e).count()
            );
            fine = Some((ex.receiver, ex.channel));
        }
        t_trib += t.elapsed().as_secs_f64();
        // KS1: H_f by flow path on the domain's KG-trib routing of KS0 (the rasterised reach's bed code; the cell's KS0 where no reach owns it)
        let t = Instant::now();
        let (ks_d, res_d) = match (ks_mode, &fine) {
            ("ks1", Some((receiver, channel))) => {
                let raster = rasterise(&gd);
                let mut bed = vec![f64::NAN; n_l];
                let reach_at: HashMap<u64, &GReach> = gd.reaches.iter().map(|r| (r.id, r)).collect();
                for (&cell, &(rid, k)) in &raster {
                    if let Some(c) = to_local(cell) {
                        bed[c] = f64::from(reach_at[&rid].attrs.bed[k]) * 0.5;
                    }
                }
                let mut tgt = vec![f64::NAN; n_l];
                for &c in &topological_order(receiver) {
                    let c = c as usize;
                    let r = receiver[c];
                    tgt[c] = if channel[c] || r == NONE {
                        if bed[c].is_finite() {
                            bed[c]
                        } else if let Some(bm) = &bed_min {
                            bm[c]
                        } else {
                            ks0[c]
                        }
                    } else {
                        tgt[r as usize]
                    };
                }
                let hf: Vec<f64> = (0..n_l).map(|p| if active[p] { (ks0[p] - tgt[p].max(sea)).max(0.0) } else { f64::INFINITY }).collect();
                ks(&hf)
            }
            ("ks1", None) => return Err("--ks ks1 needs KG-trib's routing: do not skip kg-trib".into()),
            _ => (ks0.clone(), res0.iter().filter(|r| core[r.0]).cloned().collect()),
        };
        if dump_ks_steps {
            chain_f32("ks0.f32", &ks0)?;
            chain_f32("ks1.f32", &ks_d)?;
        }
        // lead #158 (spikes #157): `--ks-connect 4`: on each D8 diagonal step of an L reach, KS also lowers the lower of the two cells sharing an edge with both line cells to the
        // step's mean height, so the carved line is 4-connected (continuous at the mesh, no bead of corner-joined pits); within the budget left, a shortfall is a residual.
        // Claims are read from KS1's output and applied as a minimum, so the order of reaches changes nothing.
        let (ks_d, res_d) = if KS_CONNECT.load(std::sync::atomic::Ordering::Relaxed) {
            let (mut z, mut res) = (ks_d, res_d);
            let mut want = vec![f64::INFINITY; n_l];
            let mut steps = 0usize;
            // the steps: consecutive vertices of the L reaches KS1 rasterised, and every channel cell's step to its receiver on KG-trib's routing KS1 cut along (the
            // second KG-trib pass, --trib-on post, draws its tributaries later on the band6 output, so KS1's slots follow this routing, not those reaches)
            let mut pairs: Vec<(usize, usize)> = Vec::new();
            for r in gd.reaches.iter().filter(|r| r.level == Level::L) {
                let cells: Vec<Option<usize>> = r.vertices.iter().map(|&c| to_local(c)).collect();
                for k in 1..cells.len() {
                    if let (Some(a), Some(b)) = (cells[k - 1], cells[k]) {
                        pairs.push((a, b));
                    }
                }
            }
            if let Some((receiver, channel)) = &fine {
                for c in 0..n_l {
                    if channel[c] && receiver[c] != NONE {
                        pairs.push((c, receiver[c] as usize));
                    }
                }
            }
            for (a, b) in pairs {
                let (ax, ay, bx, by) = (a % w, a / w, b % w, b / w);
                if ax.abs_diff(bx) != 1 || ay.abs_diff(by) != 1 {
                    continue;
                }
                steps += 1;
                let (e1, e2) = (ay * w + bx, by * w + ax);
                let t = 0.5 * (z[a] + z[b]);
                if z[e1].min(z[e2]) <= t + 1e-9 {
                    continue;
                }
                let e = if z[e1] < z[e2] || (z[e1] == z[e2] && e1 < e2) { e1 } else { e2 };
                want[e] = want[e].min(t);
            }
            let (mut cut, mut short) = (0usize, 0usize);
            for e in 0..n_l {
                if !want[e].is_finite() || !active[e] || is_sea(e) || protected_h1[parent(e)] {
                    continue;
                }
                let need = z[e] - want[e];
                let avail = (budget_l[e] - (ks_base[e] - z[e])).max(0.0).min((z[e] - (sea + 0.5)).max(0.0));
                let u = need.min(avail);
                if u > 0.0 {
                    z[e] -= u;
                    cut += 1;
                }
                if need > avail + 1e-9 {
                    short += 1;
                    res.push((e, need, avail));
                }
            }
            println!("RG domain {d} KS connect 4 (lead #158): {steps} diagonal steps (L reaches and KG-trib's channel routing), {cut} edge cells lowered, {short} short at the budget (residuals)");
            (z, res)
        } else {
            (ks_d, res_d)
        };
        if dump_ks_steps {
            chain_f32("ks1c.f32", &ks_d)?;
        }
        println!(
            "RG domain {d} KS: {} core cells lowered, {} core residuals",
            (0..n_l).filter(|&p| core[p] && ks_d[p] < start[p]).count(),
            res_d.iter().filter(|r| core[r.0]).count()
        );
        // stage 6 (spikes #146, reading 23 post): the band drain after KS1, capped by the budget at the floor; the domain's outlets and the cells outside it are its outlets
        let ks_d = if (base_form == "aplus" || census) && ks_mode != "off" {
            let t6 = Instant::now();
            let outl: Vec<bool> = (0..n_l).map(|p| outlet_d[p] || !active[p]).collect();
            // spikes #147a: the drain switches (--breach-cost, --drain-rule, --breach-max-len) are the base stages' (1 and 3, at 814 m); band6 keeps SPIKE-138's rule
            {
                use planet_generators::bake::drain as dr;
                use std::sync::atomic::Ordering::Relaxed;
                dr::BREACH_BY_LENGTH.store(false, Relaxed);
                dr::BREACH_SQUARE.store(false, Relaxed);
                dr::PREFER_FILL.store(false, Relaxed);
                dr::BREACH_MAX_LEN.store(0, Relaxed);
            }
            // spikes #147: the census, the same drain on each earlier stage's surface (KH alone, S_KH = KH + r', KS0, KS1); band6's deep depressions are traced to the first
            // stage with a deep depression whose floor lies within 4 L cells
            let run_deep = |z: &[f64]| {
                planet_generators::bake::drain::drain_selective_report(
                    z,
                    w,
                    h,
                    &outl,
                    &|_| false,
                    &|_| dx_l * dx_l,
                    8,
                    4096,
                    &|p| budget_l[p],
                    &|p| budget_l[p],
                )
                .2
            };
            let stages_deep: Vec<(&str, Vec<planet_generators::bake::drain::Deep>)> = if census {
                let t9 = Instant::now();
                let mut v = Vec::new();
                if let Some(k) = &kh_only {
                    v.push(("KH alone", run_deep(k)));
                }
                v.push(("S_KH = KH + r'", run_deep(&ks_base)));
                v.push(("KS0", run_deep(&ks0)));
                v.push(("KS1", run_deep(&ks_d)));
                println!("RG domain {d} census: {:.0} s", t9.elapsed().as_secs_f64());
                v
            } else {
                Vec::new()
            };
            planet_generators::bake::drain::HUMP_LIMIT_MICRO
                .store((band6_hump * 1.0e6).round() as u64, std::sync::atomic::Ordering::Relaxed);
            let (d6, st, deep6) = planet_generators::bake::drain::drain_selective_report(
                &ks_d,
                w,
                h,
                &outl,
                &|_| false,
                &|_| dx_l * dx_l,
                8,
                4096,
                &|p| budget_l[p],
                &|p| budget_l[p],
            );
            planet_generators::bake::drain::HUMP_LIMIT_MICRO.store(0, std::sync::atomic::Ordering::Relaxed);
            let (mut lo, mut hi) = (0.0f64, 0.0f64);
            for p in (0..n_l).filter(|&p| core[p]) {
                let dz = (d6[p] - ks_d[p]) * dx_l * dx_l / 1e9;
                if dz < 0.0 {
                    lo -= dz;
                } else {
                    hi += dz;
                }
            }
            if census {
                let in_core = |f: usize| core[f];
                let near =
                    |a: usize, b: usize| ((a % w) as i64 - (b % w) as i64).abs() <= 4 && ((a / w) as i64 - (b / w) as i64).abs() <= 4;
                let summary = |name: &str, ds: &[planet_generators::bake::drain::Deep]| {
                    let ds: Vec<_> = ds.iter().filter(|x| in_core(x.floor)).collect();
                    let mut dep: Vec<f64> = ds.iter().map(|x| x.depth).collect();
                    dep.sort_by(f64::total_cmp);
                    let cells: usize = ds.iter().map(|x| x.cells).sum();
                    let p = |q: f64| if dep.is_empty() { 0.0 } else { dep[((dep.len() - 1) as f64 * q) as usize] };
                    println!(
                        "RG census {name}: {} deep depressions in the core ({cells} cells), depth p50 {:.1} p90 {:.1} max {:.1} m",
                        ds.len(),
                        p(0.5),
                        p(0.9),
                        p(1.0)
                    );
                };
                for (name, ds) in &stages_deep {
                    summary(name, ds);
                }
                summary("band6 (KS1 + drain)", &deep6);
                // the first stage holding each of band6's deep depressions (a deep floor within 4 L cells)
                let mut first: Vec<usize> = vec![0; stages_deep.len() + 1];
                for x in deep6.iter().filter(|x| in_core(x.floor)) {
                    let k = stages_deep.iter().position(|(_, ds)| ds.iter().any(|y| near(x.floor, y.floor))).unwrap_or(stages_deep.len());
                    first[k] += 1;
                }
                let names: Vec<&str> = stages_deep.iter().map(|x| x.0).chain(["none before band6"]).collect();
                println!(
                    "RG census: band6's deep depressions by the first stage that has one within 4 L cells: {}",
                    names.iter().zip(&first).map(|(n, c)| format!("{n} {c}")).collect::<Vec<_>>().join(", ")
                );
                let b: Vec<u8> = deep6
                    .iter()
                    .flat_map(|x| {
                        [
                            (x.floor as u32).to_le_bytes(),
                            (x.cells as u32).to_le_bytes(),
                            (x.depth as f32).to_le_bytes(),
                            (x.cut.min(1e9) as f32).to_le_bytes(),
                        ]
                        .concat()
                    })
                    .collect();
                std::fs::write(out_dir.join(format!("{tag}-band6-deep.bin")), b).map_err(|e| e.to_string())?;
            }
            println!(
                "RG domain {d} band6 (reading 23 post): {} rounds, {} depressions met ({} filled, {} breached), {} left of them {} over the budget both ways; core lowered {lo:.4} km3, raised {hi:.4} km3; {:.1} s",
                st.rounds,
                st.depressions,
                st.filled,
                st.breached,
                st.left,
                st.deep,
                t6.elapsed().as_secs_f64()
            );
            // under `--base today` the census runs but band6 is not applied
            if base_form == "aplus" {
                d6
            } else {
                ks_d
            }
        } else {
            ks_d
        };
        if dump_ks_steps {
            chain_f32("band6.f32", &ks_d)?;
        }
        // lead #158 (spikes #157: the frame-44376 slot is a band6 breach path, flat at its spill level): with `--ks-connect 4`, after band6 every corner-only link of two
        // cells KS or band6 lowered (each more than 1 m under S_KH, the pair more than 1 m under both edge cells of its 2 x 2 block) gets the lower edge cell lowered to the
        // pair's mean, within the budget left; claims read from the unchanged surface, applied as a minimum
        let ks_d = if KS_CONNECT.load(std::sync::atomic::Ordering::Relaxed) {
            let mut z = ks_d;
            let mut want = vec![f64::INFINITY; n_l];
            let low = |p: usize, z: &[f64]| z[p] < ks_base[p] - 1.0;
            for y in 0..h - 1 {
                for x in 0..w - 1 {
                    let (a, b, c, e) = (y * w + x, y * w + x + 1, (y + 1) * w + x, (y + 1) * w + x + 1);
                    for (p, q, e1, e2) in [(a, e, b, c), (b, c, a, e)] {
                        if low(p, &z) && low(q, &z) && z[e1].min(z[e2]) - z[p].max(z[q]) > 1.0 {
                            let k = if z[e1] < z[e2] || (z[e1] == z[e2] && e1 < e2) { e1 } else { e2 };
                            want[k] = want[k].min(0.5 * (z[p] + z[q]));
                        }
                    }
                }
            }
            let (mut cut, mut short) = (0usize, 0usize);
            for k in 0..n_l {
                if !want[k].is_finite() || !active[k] || is_sea(k) || protected_h1[parent(k)] {
                    continue;
                }
                let need = z[k] - want[k];
                let avail = (budget_l[k] - (ks_base[k] - z[k]).max(0.0)).max(0.0).min((z[k] - (sea + 0.5)).max(0.0));
                let u = need.min(avail);
                if u > 0.0 {
                    z[k] -= u;
                    cut += 1;
                }
                if need > avail + 1e-9 {
                    short += 1;
                }
            }
            println!("RG domain {d} band6 connect 4 (lead #158): {cut} edge cells lowered at corner-only links of lowered cells, {short} short at the budget");
            z
        } else {
            ks_d
        };
        if dump_ks_steps {
            chain_f32("band6c.f32", &ks_d)?;
        }
        t_ks1 += t.elapsed().as_secs_f64();
        // crusader #168 (DEC crusader-17, --trib-on post): KG-trib again on the band6 output; the first pass's tributaries and nodes leave the graph, the trunks take their KG-trunk
        // attributes back before reading 18 accumulates them again
        if trib_on == "post" && on("kg-trib") {
            use planet_generators::bake::kg_trib::{accumulate_trunks, extract, Input, Params, Trunk};
            let t2 = Instant::now();
            gd.reaches.retain(|r| !trib_first.0.contains(&r.id));
            gd.nodes.retain(|x| !trib_first.1.contains(&x.id));
            // the kept set on the band6 output (numerics review of f5adb6d8: basin pass 1's set is KS0's, and band6 may have drained those basins): H1's lake water and the kept
            // basins of a trunk-only basin pass on ks_d, as pass 1 does on KS0
            if on("basins") {
                use planet_generators::bake::basins_rg::BasinParams;
                use planet_generators::bake::river_graph::Represented;
                let ids = planet_generators::bake::basins_rg::CellIds { gid: &gid_l, local: &to_local };
                let pk = planet_generators::bake::basins_rg::produce_ids(
                    &grid_d,
                    &ks_d,
                    &outlet_d,
                    &l_trunks,
                    &tile_l,
                    &BasinParams::l(BODY),
                    &ids,
                );
                for p in 0..n_l {
                    kept[p] = protected_h1[parent(p)];
                }
                for b in pk.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
                    for &c in &pk.cells[&b.id] {
                        kept[c] = true;
                    }
                }
                println!(
                    "RG domain {d} KG-trib post: kept set on the band6 output, {} kept basins of {}",
                    pk.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count(),
                    pk.basins.len()
                );
            }
            let first_attrs: HashMap<u64, planet_generators::bake::river_graph::Attrs> =
                l_trunks.iter().map(|r| (r.id, r.attrs.clone())).collect();
            for r in gd.reaches.iter_mut() {
                if let Some(a) = first_attrs.get(&r.id) {
                    r.attrs = a.clone();
                }
            }
            let trunks: Vec<Trunk> = l_trunks
                .iter()
                .filter_map(|r| {
                    let v: Option<Vec<usize>> = r.vertices.iter().map(|&c| to_local(c)).collect();
                    let down = g.nodes.iter().find(|x| x.id == r.down)?.clone();
                    v.map(|vertices| Trunk { id: r.id, vertices, attrs: r.attrs.clone(), down })
                })
                .collect();
            let inp = Input {
                grid: &grid_d,
                z: &ks_d,
                sea: &sea_l,
                exit: &exit,
                trunks: &trunks,
                kept: &kept,
                seed_ext: &seed,
                tile: &tile_l,
                gid: &gid_l,
                keep_fields: false,
            };
            let ex = extract(&inp, &Params { mfd_init: false, ..Params::plan(BODY, runoff_mean) });
            gd.nodes.extend(ex.nodes.iter().cloned());
            gd.reaches.extend(ex.reaches.iter().cloned());
            let trunk_ids: BTreeSet<u64> = l_trunks.iter().map(|r| r.id).collect();
            let inflow = |id: u64| to_local(id).map_or((0.0, 0.0), |c| (dx_l * dx_l + ex.d8[c], dx_l * dx_l + ex.d8_no_ext[c]));
            let acc = accumulate_trunks(&mut gd, &trunk_ids, &inflow, runoff_mean);
            if regions <= 1 {
                acc_trunks = gd.reaches.iter().filter(|r| trunk_ids.contains(&r.id)).map(|r| (r.id, r.attrs.clone())).collect();
            }
            // every reach's ends are held, and every KG-trunk node is still in the domain graph
            let held: HashSet<u64> = gd.nodes.iter().map(|x| x.id).collect();
            let dangling = gd.reaches.iter().filter(|r| r.level == Level::L && (!held.contains(&r.up) || !held.contains(&r.down))).count();
            let lost = g.nodes.iter().filter(|x| !held.contains(&x.id)).count();
            println!("RG domain {d} KG-trib post check: {dangling} L reaches with an end not held, {lost} KG-trunk nodes missing");
            println!(
                "RG domain {d} KG-trib post (crusader #168): {} reaches, {} nodes, {} open ends on the band6 output ({} first-pass reaches replaced); reading 18: {} trunk reaches, {} unordered; {:.1} s",
                ex.reaches.len(),
                ex.nodes.len(),
                ex.open_ends,
                trib_first.0.len(),
                acc.reaches,
                acc.unordered,
                t2.elapsed().as_secs_f64()
            );
            t_trib += t2.elapsed().as_secs_f64();
        }
        // the basins again, on the KS output with every L reach as spill receivers and inlets (reading 5: the L kept rule reads KS1; crusader's G1d order: the first pass on KS0 gave
        // KG-trib its kept set); the graph takes this pass's basins and nodes; its kept basins and H1's lake water are KV's and KC's protected cells
        if on("basins") && on("kg-trib") {
            use planet_generators::bake::basins_rg::{produce_drain_avoiding_forced, BasinParams, CellIds};
            use planet_generators::bake::river_graph::Represented;
            let t2 = Instant::now();
            let all: Vec<GReach> = gd.reaches.iter().filter(|r| r.level == Level::L).cloned().collect();
            let ids = CellIds { gid: &gid_l, local: &to_local };
            // spikes #142: the cells of the markers on the reaches (KG-trunk's exceptions, KG-trib's junctions); an inlet passes them (reading 1)
            let marker_cells: BTreeSet<usize> =
                gd.nodes.iter().filter(|x| x.level == Level::L && x.kind.is_marker()).filter_map(|x| to_local(x.anchor)).collect();
            {
                let (pk, nw) = memory_mib();
                println!("RG mem before basin pass 2: peak {pk} MiB, now {nw} MiB");
            }
            // crusader #161, reading 22: a kept-size depression drains unless neither its fill nor a bounded breach fits the budget KS left against S_KH (reading 21)
            let remaining: Vec<f64> = (0..n_l).map(|p| (budget_l[p] - (ks_base[p] - ks_d[p]).max(0.0)).max(0.0)).collect();
            // crusader #172 (owner, 2026-11-20; --exception-lakes on): the hollow each over-budget rise of an L reach crosses is a kept lake; the cell the bed rises from and the
            // bed it rises to
            let forced: Vec<(usize, f64)> = if EXCEPTION_LAKES.load(std::sync::atomic::Ordering::Relaxed) {
                all.iter()
                    .filter(|r| r.has_exception(planet_generators::bake::river_graph::Reason::RiseOverBudget))
                    .flat_map(|r| {
                        (1..r.vertices.len())
                            .filter(|&k| r.attrs.bed[k] > r.attrs.bed[k - 1])
                            .filter_map(move |k| to_local(r.vertices[k - 1]).map(|c| (c, f64::from(r.attrs.bed[k]) * 0.5)))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let mut pb = produce_drain_avoiding_forced(
                &grid_d,
                &ks_d,
                &outlet_d,
                &all,
                &tile_l,
                &BasinParams::l(BODY),
                &ids,
                &remaining,
                &marker_cells,
                &forced,
            );
            if !forced.is_empty() {
                println!(
                    "RG domain {d} exception lakes (crusader #172): {} over-budget rises, {} hollows kept as lakes",
                    forced.len(),
                    pb.forced
                );
            }
            drop(remaining);
            // crusader #171 (DEC crusader-20, reading 35; --kept-drained drop): a kept basin whose rim an L line crosses with a bed more than 0.5 m below its spill is drained
            // by that line (the graph's line is the authority; KV carves it): the basin is not protected
            if KEPT_DRAINED_DROP.load(std::sync::atomic::Ordering::Relaxed) {
                let mut low_bed: HashMap<usize, f64> = HashMap::new();
                for r in &all {
                    for (k, &v) in r.vertices.iter().enumerate() {
                        if let Some(c) = to_local(v) {
                            let b = f64::from(r.attrs.bed[k]) * 0.5;
                            let e = low_bed.entry(c).or_insert(b);
                            *e = e.min(b);
                        }
                    }
                }
                // --kept-drained exit: each vertex cell's next vertex downstream and its bed (a reach's last vertex has none)
                let exit_only = KEPT_DRAINED_EXIT.load(std::sync::atomic::Ordering::Relaxed);
                let mut next_of: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
                if exit_only {
                    for r in &all {
                        for k in 0..r.vertices.len().saturating_sub(1) {
                            if let (Some(c), Some(q)) = (to_local(r.vertices[k]), to_local(r.vertices[k + 1])) {
                                next_of.entry(c).or_default().push((q, f64::from(r.attrs.bed[k + 1]) * 0.5));
                            }
                        }
                    }
                }
                let (mut dropped, mut checked, mut at_edge) = (0usize, 0usize, 0usize);
                // lead (DEC crusader-21 b): a basin whose spill leaves the window (its spill path ends at the window's edge unresolved) or that holds a border cell spills beyond
                // the window; its level is not known here, so it is no kept lake
                let unresolved: HashSet<u64> = pb.unresolved.iter().copied().collect();
                for b in pb.basins.iter_mut().filter(|b| b.represented == Represented::ProtectedSurface) {
                    checked += 1;
                    let cells = &pb.cells[&b.id];
                    if unresolved.contains(&b.id) || cells.iter().any(|&c| border[c]) {
                        b.represented = Represented::None;
                        at_edge += 1;
                        continue;
                    }
                    let inside: HashSet<usize> = cells.iter().copied().collect();
                    let drains = if exit_only {
                        cells.iter().any(|&c| {
                            next_of.get(&c).is_some_and(|v| v.iter().any(|&(q, bed)| !inside.contains(&q) && bed < b.spill_m - 0.5))
                        })
                    } else {
                        cells.iter().any(|&c| {
                            (0..8)
                                .filter_map(|k| grid_d.neighbour(c, k))
                                .any(|q| !inside.contains(&q) && low_bed.get(&q).is_some_and(|&bed| bed < b.spill_m - 0.5))
                        })
                    };
                    if drains {
                        b.represented = Represented::None;
                        dropped += 1;
                    }
                }
                println!("RG domain {d} kept-drained (crusader #171): {dropped} of {checked} kept L basins drained by an L line crossing the rim below spill - 0.5 m, {at_edge} spilling beyond the window's edge; no longer protected");
            }
            println!("RG domain {d} second pass, reading 22: {} kept-size depressions drained", pb.drained);
            {
                let (pk, nw) = memory_mib();
                println!("RG mem after basin pass 2: peak {pk} MiB, now {nw} MiB");
            }
            for p in 0..n_l {
                kept[p] = protected_h1[parent(p)];
            }
            for b in pb.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface) {
                for &c in &pb.cells[&b.id] {
                    kept[c] = true;
                }
            }
            if KV_RIMS.load(std::sync::atomic::Ordering::Relaxed) {
                rim_floor = vec![f64::NEG_INFINITY; n_l];
                let mut rims = 0usize;
                for b in pb.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface && b.spill_m.is_finite()) {
                    let cells = &pb.cells[&b.id];
                    let inside: HashSet<usize> = cells.iter().copied().collect();
                    for &c in cells {
                        for q in (0..8).filter_map(|k| grid_d.neighbour(c, k)) {
                            if !inside.contains(&q) && b.spill_m > rim_floor[q] {
                                if rim_floor[q] == f64::NEG_INFINITY {
                                    rims += 1;
                                }
                                rim_floor[q] = b.spill_m;
                            }
                        }
                    }
                }
                println!("RG domain {d} kept rims (lead #176a): {rims} rim cells held at their basin's spill against KV and KC");
            }
            gd.basins = pb.basins.clone();
            gd.nodes.extend(pb.nodes.iter().cloned());
            first_nodes.clear();
            let kept_land = (0..n_l).filter(|&p| core[p] && kept[p] && start[p] > sea).count();
            println!("RG domain {d} basins on the KS output (second pass): {} L basins ({} kept, {kept_land} protected core land cells), {} unresolved spills; {:.1} s", pb.basins.len(), pb.basins.iter().filter(|b| b.represented == Represented::ProtectedSurface).count(), pb.unresolved.len(), t2.elapsed().as_secs_f64());
            t_basins += t2.elapsed().as_secs_f64();
        }
        gd.nodes.append(&mut first_nodes);
        let mut seen: HashSet<u64> = HashSet::new();
        gd.nodes.retain(|x| seen.insert(x.id));
        gd.sort();
        // KV and KC (crusader's G1f) on the domain's KS output, within the budget KS left; the domain's L reaches in window cells (the module reads vertices as grid cells)
        let t = Instant::now();
        let (kv_d, kc_d, rkc) = if on("kv") || on("kc") {
            use planet_generators::bake::kv_kc::{run_with_rims, KvParams, Switch};
            let sw = match (on("kv"), on("kc")) {
                (true, true) => Switch::Both,
                (true, false) => Switch::KvOnly,
                _ => Switch::KcOnly,
            };
            let mut g_local = gd.clone();
            g_local.reaches = gd
                .reaches
                .iter()
                .filter(|r| r.level == Level::L)
                .filter_map(|r| {
                    let v: Option<Vec<u64>> = r.vertices.iter().map(|&c| to_local(c).map(|x| x as u64)).collect();
                    v.map(|vertices| GReach { vertices, ..r.clone() })
                })
                .collect();
            // crusader #161: the budget left is budget - max(0, S_KH - KS's output) inside run_with (reading 21); KC breaches before it fills (reading 22)
            // KV/KC's outlets: the sea, and the window's edge (as H1's; a window without sea, M1, has no other: the flood needs one)
            let outlet_kv: Vec<bool> = (0..n_l).map(|p| sea_l[p] || p % w == 0 || p % w == w - 1 || p / w == 0 || p / w == h - 1).collect();
            let r = run_with_rims(
                &gl,
                &ks_d,
                &outlet_kv,
                &g_local,
                &kept,
                &budget_l,
                Some(&ks_base),
                &KvParams {
                    cross_grade: KV_CROSS_MICRO.load(std::sync::atomic::Ordering::Relaxed) as f64 * 1.0e-6,
                    cross_side_m: kv_cross_side.0,
                    bed_floor_m: if KV_SEA_FLOOR.load(std::sync::atomic::Ordering::Relaxed) { sea } else { f64::NEG_INFINITY },
                    kc_lake_outlets: KC_LAKE_OUTLETS.load(std::sync::atomic::Ordering::Relaxed),
                    connect4: KV_CONNECT.load(std::sync::atomic::Ordering::Relaxed),
                    cross_side_grade: kv_cross_side.1,
                    ..KvParams::plan(dx_l)
                },
                sw,
                (!rim_floor.is_empty()).then_some(&rim_floor[..]),
            );
            drop(outlet_kv);
            println!("RG domain {d} KC breaches: {} ({:.4} km3)", r.breaches, r.breach_volume / 1.0e9);
            // reading 10 (crusader #165): the divide cells within W_floor/2 + d_buf of a graph line that put no buffer
            let (div, exempt) = (r.divide.iter().filter(|&&x| x).count(), r.divide_exempt.iter().filter(|&&x| x).count());
            println!(
                "RG domain {d} KV reading 10: {exempt} of {div} divide cells exempt from the divide buffer ({:.2} %)",
                100.0 * exempt as f64 / div.max(1) as f64
            );
            drop(g_local);
            let kv = if on("kv") { r.kv } else { ks_d.clone() };
            let kc = if on("kc") { r.kc } else { kv.clone() };
            println!("RG domain {d} KV/KC ({sw:?}): {} residuals, {} KC patches", r.residuals.len(), r.patches.len());
            println!("RG domain {d} KV 4-connected rule (lead #177): {} edge cells lowered", r.connect_cells);
            println!(
                "RG domain {d} KV cross-slope (crusader #169): {} cells lowered, {:.6} km3; {} cells short at the budget, {:.1} m in all",
                r.cross_cells,
                r.cross_volume / 1.0e9,
                r.cross_short,
                r.cross_short_m
            );
            // lead #173: why KV did or did not cut the L vertex cells above their bed
            for (label, trunk, other) in &r.census {
                println!("RG domain {d} KV vertex census (lead #173): {label}: {trunk} trunk-image, {other} other");
            }
            // lead #175: the census per vertex (window cell x, y; class index as printed above, 0 first; trunk image; bed, height before and after KV)
            if regions <= 1 {
                let mut f = String::from(
                    "x,y,class,trunk,bed_m,before_m,after_kv_m,sea,window_edge
",
                );
                for &(c, class, trunk, bed, before, after) in &r.vertex_class {
                    f += &format!(
                        "{},{},{class},{},{bed:.2},{before:.3},{after:.3},{},{}
",
                        c % w,
                        c / w,
                        u8::from(trunk),
                        u8::from(sea_l[c]),
                        u8::from(c % w == 0 || c % w == w - 1 || c / w == 0 || c / w == h - 1)
                    );
                }
                std::fs::write(out_dir.join(format!("{tag}-kv-vertices.csv")), f).map_err(|e| e.to_string())?;
            }
            if !r.cross_cut.is_empty() {
                // DEC crusader-19's report: how deep the pass cut (p50, p95, max over the cells it lowered)
                let mut cuts: Vec<f32> = r.cross_cut.iter().copied().filter(|&x| x > 0.0).collect();
                cuts.sort_unstable_by(|a, b| a.total_cmp(b));
                let q = |f: f64| cuts.get(((cuts.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(0.0);
                println!(
                    "RG domain {d} KV cross-slope cut: p50 {:.3} m, p95 {:.3} m, max {:.3} m over {} cells",
                    q(0.5),
                    q(0.95),
                    q(1.0),
                    cuts.len()
                );
                if cross_out.is_empty() {
                    cross_out = vec![0.0f32; n_l];
                }
                for p in (0..n_l).filter(|&p| core[p]) {
                    cross_out[p] = r.cross_cut[p];
                }
            }
            kc_patches.extend(r.patches.iter().copied());
            (kv, kc, r.residuals)
        } else {
            (ks_d.clone(), ks_d.clone(), Vec::new())
        };
        t_kvkc += t.elapsed().as_secs_f64();
        // the domain's share: its core's cells and residuals, the items its core's tiles own
        if regions <= 1 && outs.is_none() {
            let (mut a, mut b, mut c) = (ks_d, kv_d, kc_d);
            for p in (0..n_l).filter(|&p| !core[p]) {
                (a[p], b[p], c[p]) = (ks0[p], ks0[p], ks0[p]);
            }
            outs = Some((a, b, c));
        } else {
            let (ks_out, kv_out, kc_out) = outs.get_or_insert_with(|| (ks0.clone(), ks0.clone(), ks0.clone()));
            for p in (0..n_l).filter(|&p| core[p]) {
                (ks_out[p], kv_out[p], kc_out[p]) = (ks_d[p], kv_d[p], kc_d[p]);
            }
        }
        res_ks.extend(res_d.into_iter().filter(|r| core[r.0]));
        res_kvkc.extend(rkc.into_iter().filter(|r| r.cell < n_l && core[r.cell]));
        // a tile item belongs to the domain whose core holds its tile; a landmass item new in this domain (not KG-trunk's) is kept from every domain, and two domains that
        // give one id different contents are counted (a partition difference the dedupe would otherwise hide)
        let mine = |o: Domain| regions == 1 || o == Domain::Landmass || owner_region(o) == Some(d);
        let trunk_ids: HashSet<u64> = g.nodes.iter().map(|x| x.id).chain(l_trunks.iter().map(|x| x.id)).collect();
        for x in gd.nodes.iter().filter(|x| mine(x.owner) && !trunk_ids.contains(&x.id)) {
            match landmass_seen.get(&x.id) {
                Some(prev) if x.owner == Domain::Landmass && prev != &format!("{x:?}") => landmass_conflicts += 1,
                _ => {}
            }
            if x.owner == Domain::Landmass {
                landmass_seen.insert(x.id, format!("{x:?}"));
            }
            extra.nodes.push(x.clone());
        }
        for x in gd.reaches.iter().filter(|x| mine(x.owner) && !trunk_ids.contains(&x.id)) {
            match landmass_seen.get(&x.id) {
                Some(prev) if x.owner == Domain::Landmass && prev != &format!("{x:?}") => landmass_conflicts += 1,
                _ => {}
            }
            if x.owner == Domain::Landmass {
                landmass_seen.insert(x.id, format!("{x:?}"));
            }
            extra.reaches.push(x.clone());
        }
        extra.basins.extend(gd.basins.iter().filter(|x| mine(x.owner)).cloned());
    }
    let have: HashSet<u64> = g.nodes.iter().map(|x| x.id).collect();
    let mut seen: HashSet<u64> = have.clone();
    g.nodes.extend(extra.nodes.into_iter().filter(|x| seen.insert(x.id)));
    let mut seen_r: HashSet<u64> = g.reaches.iter().map(|x| x.id).collect();
    g.reaches.extend(extra.reaches.into_iter().filter(|x| seen_r.insert(x.id)));
    let mut seen_b: HashSet<u64> = g.basins.iter().map(|x| x.id).collect();
    g.basins.extend(extra.basins.into_iter().filter(|x| seen_b.insert(x.id)));
    // a junction marker a domain made for a tributary its core does not own (one it extracted in its halo) has no reach ending at it and no basin spilling to it in the stitched graph: dropped
    let ends: HashSet<u64> = g.reaches.iter().map(|r| r.down).chain(g.basins.iter().map(|b| b.spill_node)).collect();
    let before = g.nodes.len();
    g.nodes.retain(|x| x.kind != NodeKind::TribJunction || ends.contains(&x.id));
    println!("RG stitch: {} junction markers without a reach ending at them dropped", before - g.nodes.len());
    g.sort();
    // gap 3 (crusader's KV/KC writer, lead #159): every KV/KC residual's reach flagged exception(budget_residual); KS's residuals name no reach
    let flagged = planet_generators::bake::kv_kc::flag_residuals(&mut g, &res_kvkc);
    println!("RG residual flags: {flagged:?}");
    drop(ks0);
    let (ks_out, kv_out, kc_out) = outs.expect("at least one domain");
    for r in g.reaches.iter_mut() {
        if let Some(a) = acc_trunks.remove(&r.id) {
            r.attrs = a;
        }
    }
    println!("RG domains: {} nodes, {} reaches, {} basins after the stitch; landmass items with different contents in two domains: {landmass_conflicts}", g.nodes.len(), g.reaches.len(), g.basins.len());
    for (name, secs) in [("basins", t_basins), ("KG-trib", t_trib), ("KS1", t_ks1), ("KV and KC", t_kvkc)] {
        stamp(name, secs);
    }

    // ---- outputs: what river-gate --graph --stages --partition reads (BUILD-060), and the final surface
    let t = Instant::now();
    let stages_dir = out_dir.join(format!("{tag}-stages"));
    std::fs::create_dir_all(&stages_dir).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join(format!("{tag}-graph.txt")), g.canonical()).map_err(|e| e.to_string())?;
    let field = |v: &dyn Fn(usize) -> f64| -> Vec<u8> {
        (0..n_l).flat_map(|p| ((v(p).max(0.0) / 0.1).floor().min(65535.0) as u16).to_le_bytes()).collect()
    };
    let write = |name: &str, bytes: Vec<u8>| -> Result<(), String> {
        if let Some(d) = &chain_dir {
            std::fs::write(d.join("stages").join(name), &bytes).map_err(|e| e.to_string())?;
        }
        std::fs::write(stages_dir.join(name), bytes).map_err(|e| e.to_string())
    };
    // lead #169 item 2, #170 item 9: the run's chain (`{tag}-chain.txt`, and the chain layout's chain.txt): the commit, every switch's value and every stage's form, in the
    // execution order of pipeline-technical.md section 0.1 (`stage <step> <stage number> <name> <form>`)
    let chain_text = {
        let (commit, dirty) = (&run_commit, run_dirty);
        let on_off = |s: &str, form: &str| if on(s) { form.to_string() } else { "skipped".to_string() };
        let ks_form = |pass: &str| match ks_mode {
            "off" => "off".to_string(),
            "ks0" if pass == "ks1" => "off".to_string(),
            _ => pass.to_string(),
        };
        let mut t = format!("stage-chain 1\nwindow {x0} {y0} {w} {h}\ncoarse {L}\ncell_m {dx_l}\nsea_m {sea}\nlabel {tag}\n");
        t += &format!("commit {commit}{}\n", if dirty { "-dirty" } else { "" });
        for (k, v) in [
            ("h1", h1_form.to_string()),
            ("base", base_form.to_string()),
            ("kh", kh.to_string()),
            ("zs-read", zs_read.to_string()),
            ("k5-route", k5_route.to_string()),
            ("k5-diffuse", k5_diffuse.to_string()),
            ("k5-kdt-scale", k5_kdt_scale.to_string()),
            ("k5-diffuse-stencil", k5_stencil.to_string()),
            ("edit", value(&args, "--edit").unwrap_or("-").to_string()),
            ("k5-uok-scale", k5_uok_scale.to_string()),
            ("k5-edge", k5_edge.to_string()),
            ("ks-connect", ks_connect.to_string()),
            ("sea-mask", sea_mask.to_string()),
            ("band6-hump", band6_hump.to_string()),
            ("kh-lower-only", if kh_lower_only { "on" } else { "off" }.to_string()),
            ("trib-once", if trib_once { "on" } else { "off" }.to_string()),
            ("k5-diffuse-amax", k5_diffuse_amax.to_string()),
            ("k5-parity", k5_parity.to_string()),
            ("ks", ks_mode.to_string()),
            ("h1-lakes", h1_lakes.to_string()),
            ("regions", regions.to_string()),
            ("skip", if skip.is_empty() { "-".to_string() } else { skip.join(",") }),
            ("window", win.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")),
            ("margin", margin.to_string()),
            ("turned-noise", planet_generators::height::turned_noise().to_string()),
            ("kv-cross", format!("{}", KV_CROSS_MICRO.load(std::sync::atomic::Ordering::Relaxed) as f64 * 1.0e-6)),
            ("kv-connect", if KV_CONNECT.load(std::sync::atomic::Ordering::Relaxed) { "on" } else { "off" }.to_string()),
            ("kv-rims", if KV_RIMS.load(std::sync::atomic::Ordering::Relaxed) { "on" } else { "off" }.to_string()),
            ("kv-sea-floor", if KV_SEA_FLOOR.load(std::sync::atomic::Ordering::Relaxed) { "on" } else { "off" }.to_string()),
            ("kc-lake-outlets", if KC_LAKE_OUTLETS.load(std::sync::atomic::Ordering::Relaxed) { "on" } else { "off" }.to_string()),
            ("exception-lakes", if EXCEPTION_LAKES.load(std::sync::atomic::Ordering::Relaxed) { "on" } else { "off" }.to_string()),
            (
                "kept-drained",
                if KEPT_DRAINED_EXIT.load(std::sync::atomic::Ordering::Relaxed) {
                    "exit"
                } else if KEPT_DRAINED_DROP.load(std::sync::atomic::Ordering::Relaxed) {
                    "drop"
                } else {
                    "off"
                }
                .to_string(),
            ),
            ("kv-cross-side", format!("{},{}", kv_cross_side.0, kv_cross_side.1)),
            ("trunk-bed", if TRUNK_BED_SHARED.load(std::sync::atomic::Ordering::Relaxed) { "shared" } else { "lift" }.to_string()),
            ("trib-on", trib_on.to_string()),
            // lead #180 (RGv0 section 25): every switch, so a chain is fully traceable
            ("base-stages", base_stages.to_string()),
            ("drain-rule", drain_rule.to_string()),
            ("breach-cost", breach_cost.to_string()),
            ("breach-max-len", breach_max_len.to_string()),
            ("centre", if low { "low" } else { "maximin" }.to_string()),
            ("threads", threads.to_string()),
            ("census", args.iter().any(|a| a == "--census").to_string()),
            ("probe-base", args.iter().any(|a| a == "--probe-base").to_string()),
            ("stop-after-kh", args.iter().any(|a| a == "--stop-after-kh").to_string()),
            ("dump-s0", args.iter().any(|a| a == "--dump-s0").to_string()),
            ("dump-c1", args.iter().any(|a| a == "--dump-c1").to_string()),
        ] {
            t += &format!("switch {k} {v}\n");
        }
        let today = base_form == "today";
        for (step, stage, name, form) in [
            (1, "1", "base1", if today { "off" } else { "sel" }.to_string()),
            (
                2,
                "2",
                "k5",
                match k5_route {
                    "ltd" => "main, D8-LTD receivers",
                    "dinf" => "main, D-infinity steps",
                    _ => "main",
                }
                .to_string(),
            ),
            (3, "3", "base3", if today { "off" } else { "post" }.to_string()),
            (4, "4", "h1", h1_form.to_string()),
            (5, "5", "kh", format!("{kh}, band floored at the sea + 0.5 m")),
            (6, "13", "budget", "relief10 on S0, lowering against S_KH".to_string()),
            (7, "7", "ks0", ks_form("ks0")),
            (8, "8", "kg-trunk", on_off("kg-trunk", "corridor, readings 24 and 26")),
            (9, "10", "basins-1", on_off("basins", "kept set on KS0")),
            (10, "9", "kg-trib", on_off("kg-trib", "1 km2, readings 14 and 18")),
            (11, "7", "ks1", ks_form("ks1")),
            (12, "6", "band6", if today { "off" } else { "post" }.to_string()),
            (
                12,
                "9",
                "kg-trib-post",
                if trib_on == "post" { on_off("kg-trib", "on the band6 output, crusader #168") } else { "off".to_string() },
            ),
            (13, "10", "basins-2", on_off("basins", "on the KS output, reading 22")),
            (14, "11", "kv-kc", format!("kv {}, kc {}, outlets sea and window edge, against S_KH", on_off("kv", "on"), on_off("kc", "on"))),
            (15, "12", "k7-hillslope", "off".to_string()),
        ] {
            t += &format!("stage {step} {stage} {name} {form}\n");
        }
        t
    };
    std::fs::write(out_dir.join(format!("{tag}-chain.txt")), &chain_text).map_err(|e| e.to_string())?;
    if let Some(d) = &chain_dir {
        std::fs::write(d.join("graph.txt"), g.canonical()).map_err(|e| e.to_string())?;
        std::fs::write(d.join("chain.txt"), &chain_text).map_err(|e| e.to_string())?;
    }
    chain_f32("s0.f32", &start)?;
    chain_f32("ks.f32", &ks_out)?;
    chain_f32("kv.f32", &kv_out)?;
    chain_f32("final.f32", &kc_out)?;
    // reading 21: KH against S0 in its own field (BUILD-060 stage `kh`); ks, kv and kc against S_KH
    write("kh-lower.u16", field(&|p| start[p] - ks_base[p]))?;
    write("kh-fill.u16", field(&|p| ks_base[p] - start[p]))?;
    if ks_mode != "off" {
        write("ks-lower.u16", field(&|p| ks_base[p] - ks_out[p]))?;
    }
    if on("kv") {
        write("kv-lower.u16", field(&|p| ks_out[p] - kv_out[p]))?;
        write("kv-fill.u16", field(&|p| kv_out[p] - ks_out[p]))?;
    }
    if !cross_out.is_empty() {
        let bytes: Vec<u8> = cross_out.iter().flat_map(|x| x.to_le_bytes()).collect();
        std::fs::write(stages_dir.join("kv-cross.f32"), bytes).map_err(|e| e.to_string())?;
    }
    if on("kc") {
        write("kc-lower.u16", field(&|p| kv_out[p] - kc_out[p]))?;
        write("kc-fill.u16", field(&|p| kc_out[p] - kv_out[p]))?;
    }
    {
        let mut f = std::fs::File::create(stages_dir.join("residuals.csv")).map_err(|e| e.to_string())?;
        writeln!(f, "cell,stage,needed_m,available_m,feature").map_err(|e| e.to_string())?;
        for &(p, need, avail) in &res_ks {
            writeln!(f, "{p},ks,{need:.4},{avail:.4},-").map_err(|e| e.to_string())?;
        }
        for r in &res_kvkc {
            writeln!(f, "{},{},{:.4},{:.4},{:016x}", r.cell, r.stage, r.needed_m, r.available_m, r.feature).map_err(|e| e.to_string())?;
        }
    }
    // BUILD-060 (lead #172): KC's patches beside the stage fields, read by river-gate as `b_kc_patches` and `b_kc_patch_km3`
    {
        let text: String =
            std::iter::once("cells,volume_m3\n".to_string()).chain(kc_patches.iter().map(|(c, v)| format!("{c},{v:.3}\n"))).collect();
        std::fs::write(stages_dir.join("kc-patches.csv"), &text).map_err(|e| e.to_string())?;
        if let Some(d) = &chain_dir {
            std::fs::write(d.join("stages").join("kc-patches.csv"), &text).map_err(|e| e.to_string())?;
        }
    }
    if args.iter().any(|a| a == "--dump-s0") {
        let b: Vec<u8> = start.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(out_dir.join(format!("{tag}-s0-{w}x{h}.f64")), b).map_err(|e| e.to_string())?;
    }
    let bytes: Vec<u8> = kc_out.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(out_dir.join(format!("{tag}-final-{w}x{h}.f64")), bytes).map_err(|e| e.to_string())?;
    stamp("write", t.elapsed().as_secs_f64());
    {
        let mut f = std::fs::File::create(out_dir.join(format!("{tag}-timing.csv"))).map_err(|e| e.to_string())?;
        writeln!(f, "stage,seconds,peak_mib,now_mib").map_err(|e| e.to_string())?;
        for (s, sec, pk, nw) in &timing {
            writeln!(f, "{s},{sec:.1},{pk},{nw}").map_err(|e| e.to_string())?;
        }
    }
    println!(
        "RG done: kh {kh}, ks {ks_mode}, skipped {:?}, regions {regions}; {} nodes, {} reaches, {} basins; files in {}: {tag}-graph.txt, {tag}-stages/, {tag}-final-{w}x{h}.f64, {tag}-timing.csv; {:.1} s",
        skip,
        g.nodes.len(),
        g.reaches.len(),
        g.basins.len(),
        out_dir.display(),
        t_all.elapsed().as_secs_f64()
    );
    Ok(())
}

/// The process's peak and current working set (MiB; Windows: `PeakWorkingSetSize`, `WorkingSetSize`; Linux: `VmHWM`, `VmRSS`); bake_layer.rs's helper.
#[cfg(windows)]
fn memory_mib() -> (u64, u64) {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c =
        Counters { cb: core::mem::size_of::<Counters>() as u32, page_fault_count: 0, peak_working_set: 0, working_set: 0, rest: [0; 6] };
    // SAFETY: `c` is a writable PROCESS_MEMORY_COUNTERS of the size it declares; the pseudo handle needs no closing.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 {
        return (0, 0);
    }
    ((c.peak_working_set >> 20) as u64, (c.working_set >> 20) as u64)
}

#[cfg(not(windows))]
fn memory_mib() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0);
    (kb("VmHWM:") >> 10, kb("VmRSS:") >> 10)
}
