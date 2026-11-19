//! SPIKE-135 (spike/repair-debt, never merged): a copy of SPIKE-128's `ks_g1b` (spike/ks-g1b e359dfae) whose `--dump-only` writes KS's lowering at L for RG-1 (KS0) and RG-2 (KS1)
//! (`ks-rg1-used-<w>x<h>.f32`, `ks-rg2-used-<w>x<h>.f32`, little-endian f32, row-major) and stops before the divides and the gate measures. The rest is explorer's code unchanged.
//! SPIKE-128 (spike code on spike/ks-g1b, never merged; lead #164, G1b): KS in isolation on a window's start surface `S0`.
//! `ks_g1b --window face,i0,i1,j0,j1,seed --out DIR [--margin M] [--threads T]`, run under the heavy lock.
//! - RG-0: `S0` (the river gate's start surface: `B + C1(z_s - z_b)`).
//! - RG-1: KS0 = `C1(z_s) + min(r, H)`, `r = B - C1(z_b)`, `H` the HAND of today's H1 at 814 m (height over the first H1 channel cell on the receiver chain), read at L through the same C1 read.
//! - RG-2: KS1 = `C1(z_s) + min(r, H_f)`, `H_f` by flow path on a routing of KS0 at L with `A_crit` 1 km2: stand-in lines until G1d.
//! - Budget (G0 contracts section 6): `relief_1km` per 814 m cell (range of `S0` over the L cells within 5 cells of its centre L cell), `relief10` its p50 over the 814 m cells within 25 km,
//!   `budget = min(50, max(10, 0.5 relief10))`; KS lowers at most the budget against `S0` and counts a residual where the conditioning needs more. H1's lake water is never changed.
//! Alpha = 1. Land: `S0 > 0`.
// spike only (never merged): lints allowed as run, the code unchanged
#![allow(clippy::disallowed_methods, clippy::doc_lazy_continuation, clippy::type_complexity, dead_code)]
use std::path::Path;

use planet_cache::global_store::{class_inputs, class_inputs_key};
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{measure, GateInput, GateParams};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use std::collections::BinaryHeap;

use planet_generators::bake::channel_graph::{q_value, ChannelGraph};
use planet_generators::bake::hydro::{hydrology, Hydro, HydroParams};
use planet_generators::bake::route::{catchment_roots, jitter, route_rules, RoundRules, Routing, MFD_EXPONENT, NONE};
use planet_generators::bake::{Grid, NEIGHBOURS};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
/// R1h: face 1, regional cells i 8,056 to 8,675, j 760 to 1,379, seed 14 (the E1 region stores' window: r3a-e1, r3a-p2, r3a-p2e).
const R1H: [usize; 6] = [1, 8056, 8675, 760, 1379, 14];

/// The base lattice's nodes over the region (the C1 read's footprint).
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

fn parsed<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    match value(args, name) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|_| format!("{name} takes a number, not '{v}'")),
    }
}

/// Fills `out` (`w` a row) by `f(cell)` over `threads` scoped threads, in rows: the same values on any thread count.
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

// ---------------------------------------------------------------- SPIKE-127's K6' (copied from spike/g1a-sills 1d4e63f5, lines 84-414)
const L: usize = LOCAL_PER_REGIONAL;
const EDGE_Q: f64 = 1.0;

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

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_key_check = args.iter().any(|a| a == "--no-key-check");
    let k6p = args.iter().any(|a| a == "--k6p");
    let dump_only = args.iter().any(|a| a == "--dump-only");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--dump-only").collect();
    let args: Vec<String> = args.into_iter().filter(|a| a != "--k6p").collect();
    let args: Vec<String> = args.into_iter().filter(|a| a != "--no-key-check").collect();
    let known = ["--window", "--margin", "--store", "--min-cells", "--d", "--threads", "--network-km2", "--grid", "--out", "--k6p"];
    let mut k = 0;
    while k < args.len() {
        if !known.contains(&args[k].as_str()) {
            return Err(format!("unknown river-gate option '{}' (known: {})", args[k], known.join(", ")));
        }
        k += 2;
    }
    let win: Vec<usize> = match value(&args, "--window") {
        None => R1H.to_vec(),
        Some(v) => v
            .split(',')
            .map(|x| x.parse::<usize>().map_err(|_| format!("--window face,i0,i1,j0,j1,seed, not '{v}'")))
            .collect::<Result<_, _>>()?,
    };
    if win.len() != 6 || win[0] > 5 || win[1] > win[2] || win[3] > win[4] || win[2] >= REGIONAL || win[4] >= REGIONAL {
        return Err(format!("--window face,i0,i1,j0,j1,seed: a face 0 to 5 and an ordered box inside 0..{REGIONAL}, not {win:?}"));
    }
    let margin: usize = parsed(&args, "--margin", 200)?;
    let threads: usize = parsed(&args, "--threads", 8)?;
    let network_km2 = match value(&args, "--network-km2") {
        None => GateParams::default().network_km2,
        Some(v) => {
            let a: Vec<f64> = v
                .split(',')
                .map(|x| x.parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("--network-km2 a,b in km2, not '{v}'"))?;
            match a[..] {
                [x, y] if x > 0.0 && y > 0.0 => [x, y],
                _ => return Err(format!("--network-km2 takes two positive areas a,b in km2, not '{v}'")),
            }
        }
    };
    let params = GateParams {
        basin_cells: parsed(&args, "--min-cells", GateParams::default().basin_cells)?,
        lake_depth_m: parsed(&args, "--d", 20.0)?,
        network_km2,
        ..GateParams::default()
    };
    let store = value(&args, "--store").map(Path::new);
    if no_key_check && store.is_none() {
        return Err("--no-key-check reads a store: it goes with --store".to_string());
    }
    let (face, seed) = (Face(win[0] as u8), win[5] as u64);
    let region = (win[1], win[2], win[3], win[4]);
    // The C1 read meets base nodes 3 below and 4 above the margin's (to_node below): the window keeps 4 regional cells of the face beyond its margin on each side.
    if region.0 < margin + 4 || region.2 < margin + 4 || region.1 + margin + 4 >= REGIONAL || region.3 + margin + 4 >= REGIONAL {
        return Err(format!("the window {region:?} with the margin {margin} reaches within 4 regional cells of the face's edge"));
    }
    let def = WorldDefinition::single_planet(R, seed);
    let inputs = class_inputs(&def, seed, threads);
    // The base lattice's nodes the C1 read meets over the region and its margin.
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
    let front = region_front(&spec, &base, &climate)?;
    // The L window: the regional solve's window, 8 L cells a regional cell.
    let l = LOCAL_PER_REGIONAL;
    let (sri0, srj0, srw, srh) = (front.solve.ri0, front.solve.rj0, front.solve.rw, front.solve.rh);
    let (x0, y0, w, h) = ((sri0 * l) as i64, (srj0 * l) as i64, srw * l, srh * l);
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| {
        planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0)
    };
    let mut start = vec![0f64; w * h];
    par_fill(&mut start, w, threads, &|p| {
        let d = dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
        base(d) + front.regional_field.at(d)
    });
    let (bi0, bi1, bj0, bj1) = ((region.0 * l) as i64, ((region.1 + 1) * l) as i64, (region.2 * l) as i64, ((region.3 + 1) * l) as i64);
    let in_box: Vec<bool> = (0..w * h)
        .map(|c| {
            let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
        })
        .collect();

    // ---------------- SPIKE-128 from here (spike code) ----------------
    let out_dir = value(&args, "--out").ok_or("--out DIR is required")?.to_string();
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{out_dir}: {e}"))?;
    let t0 = std::time::Instant::now();
    let sea = 0.0f64;
    let n_l = w * h;
    let solve = &front.solve;
    let (rw, rh) = (solve.rw, solve.rh);
    let rn = rw * rh;
    assert!(rw * l == w && rh * l == h, "the L window is the regional window x {l}");
    let reg = |p: usize| (p / w / l) * rw + (p % w) / l;
    let cell_km2 = (front.solve.grid.dx() / l as f64).powi(2) / 1.0e6;
    let dir_l = |p: usize| dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
    let land = |p: usize| start[p] > sea;
    let inside_land = |p: usize| in_box[p] && start[p] > sea;
    let land_n = (0..n_l).filter(|&p| inside_land(p)).count();

    // r = B - C1(z_b) = S0 - C1(z_s - z_b) - C1(z_b), through the regional field's own read.
    let field =
        |v: Vec<f32>| planet_generators::bake::k7::RegionField::new(face, REGIONAL, (solve.ri0 as i64, solve.rj0 as i64), rw, rh, v);
    let zb_field = field(solve.zb.iter().map(|&v| v as f32).collect());
    let mut r = vec![0f64; n_l];
    par_fill(&mut r, w, threads, &|p| {
        let d = dir_l(p);
        start[p] - front.regional_field.at(d) - zb_field.at(d)
    });
    drop(zb_field);

    // H: today's H1 at 814 m. The routing as region_front runs it; channel cells are the channel graph's cells; H = zs over the first channel cell (or outlet) on the receiver chain.
    let dir_reg = |c: usize| solve.face_spec.halo_direction((solve.ri0 + c % rw) as i64, (solve.rj0 + c / rw) as i64);
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; rn], vec![0.0f32; rn], vec![0.0f32; rn]);
    for c in (0..rn).filter(|&c| solve.zs[c] > 0.0) {
        let (ro, ev, wt) = climate(dir_reg(c));
        runoff[c] = ro as f32;
        evap[c] = ev as f32;
        weight[c] = wt as f32;
    }
    let h1 =
        route_rules(&solve.grid, &solve.zs, &solve.outlet, solve.precision.epsilon_w, solve.stream, 0.0, Some(&weight), RoundRules::ROUND);
    let mut channel_h1 = vec![false; rn];
    front.graph.for_each(|s| {
        for c in s.cells(&solve.grid) {
            channel_h1[c as usize] = true;
        }
    });
    let mut target = vec![0usize; rn];
    for &c in &h1.order {
        let c = c as usize;
        let rcv = h1.receiver[c];
        target[c] = if channel_h1[c] || rcv == NONE { c } else { target[rcv as usize] };
    }
    let mut hand: Vec<f32> = (0..rn).map(|c| (solve.zs[c] - solve.zs[target[c]].max(sea)).max(0.0) as f32).collect();
    drop((h1, target));
    if k6p {
        // H over K6' (SPIKE-127): node heights (the lowest L cell of S0), the sill flood and routing, K6's hydrology and channel graph; H = node height over the first channel cell's.
        let s0_read = |c: usize| start[c];
        let sills = nodes_and_sills(&s0_read, rw, rh, threads, None);
        let sill = |p: usize, d: usize, q: usize| sills.at(p, d, q);
        let eps = solve.precision.epsilon_w;
        let f = flood(&solve.grid, &sills.h, &sill, &solve.outlet, eps);
        let ro = route_sills(&solve.grid, &f, &sill, &solve.outlet, solve.stream, &weight, true);
        let k6 = h1_from(&solve.grid, &sills.h, &f, ro, &solve.outlet, &runoff, &evap);
        let mut ch = vec![false; rn];
        k6.graph.for_each(|s| {
            for c in s.cells(&solve.grid) {
                ch[c as usize] = true;
            }
        });
        let mut tg = vec![0usize; rn];
        for &c in &k6.routing.order {
            let c = c as usize;
            let rcv = k6.routing.receiver[c];
            tg[c] = if ch[c] || rcv == NONE { c } else { tg[rcv as usize] };
        }
        hand = (0..rn).map(|c| (sills.h[c] - sills.h[tg[c]].max(sea)).max(0.0) as f32).collect();
        println!(
            "KS K6': {} channel cells (today's H1 {}); H from node heights",
            ch.iter().filter(|&&b| b).count(),
            channel_h1.iter().filter(|&&b| b).count()
        );
    }
    let hand_field = field(hand.clone());
    let mut hand_l = vec![0f64; n_l];
    par_fill(&mut hand_l, w, threads, &|p| hand_field.at(dir_l(p)).max(0.0));
    drop(hand_field);
    let mut protected_reg = vec![false; rn];
    for &(c, _) in &front.lakes.water {
        protected_reg[c as usize] = true;
    }
    let protected = |p: usize| protected_reg[reg(p)];
    println!("KS setup: {w} x {h} L cells ({rw} x {rh} at 814 m), {land_n} land cells inside, {} H1 channel cells, {} H1 lake cells protected; {:.0} s", channel_h1.iter().filter(|&&b| b).count(), protected_reg.iter().filter(|&&b| b).count(), t0.elapsed().as_secs_f64());

    // The budget field (G0 contracts section 6).
    let mut relief_1km = vec![0f64; rn];
    par_fill(&mut relief_1km, rw, threads, &|c| {
        let (cx, cy) = ((c % rw) * l + l / 2, (c / rw) * l + l / 2);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for y in cy.saturating_sub(5)..=(cy + 5).min(h - 1) {
            for x in cx.saturating_sub(5)..=(cx + 5).min(w - 1) {
                let v = start[y * w + x];
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        hi - lo
    });
    let rad = 25_000.0 / solve.face_spec.dx();
    let ri = rad.ceil() as i64;
    let offsets: Vec<(i64, i64)> =
        (-ri..=ri).flat_map(|dy| (-ri..=ri).map(move |dx| (dx, dy))).filter(|&(dx, dy)| libm::hypot(dx as f64, dy as f64) <= rad).collect();
    let mut relief10 = vec![0f64; rn];
    par_fill(&mut relief10, rw, threads, &|c| {
        let (cx, cy) = ((c % rw) as i64, (c / rw) as i64);
        let mut v: Vec<f64> = offsets
            .iter()
            .filter_map(|&(dx, dy)| {
                let (x, y) = (cx + dx, cy + dy);
                (x >= 0 && y >= 0 && x < rw as i64 && y < rh as i64).then(|| relief_1km[y as usize * rw + x as usize])
            })
            .collect();
        let k = v.len() / 2;
        *v.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1
    });
    let budget: Vec<f64> = relief10.iter().map(|&x| (0.5 * x).clamp(10.0, 50.0)).collect();
    println!(
        "KS budget: 25 km disc of {} cells at 814 m ({:.1} m); {:.0} s",
        offsets.len(),
        solve.face_spec.dx(),
        t0.elapsed().as_secs_f64()
    );

    // KS with the budget: lowering = min(max(0, r - band), budget); a residual where the conditioning needs more.
    let apply = |band: &[f64]| -> (Vec<f64>, Vec<f32>, Vec<f32>) {
        let mut z = start.clone();
        let mut used = vec![0f32; n_l];
        let mut want = vec![0f32; n_l];
        for p in 0..n_l {
            if !land(p) || protected(p) {
                continue;
            }
            let wnt = (r[p] - band[p]).max(0.0);
            let u = wnt.min(budget[reg(p)]);
            z[p] = start[p] - u;
            used[p] = u as f32;
            want[p] = wnt as f32;
        }
        (z, used, want)
    };
    let (ks0, used0, want0) = apply(&hand_l);
    drop(hand_l);
    if k6p {
        // For crusader's G1d extractor (lead, 2026-11-17): KS0 at L, little-endian f64, row-major, w x h, with a text header.
        let bytes: Vec<u8> = ks0.iter().flat_map(|v| v.to_le_bytes()).collect();
        let f = format!("{out_dir}/ks0-k6p-{w}x{h}.f64");
        std::fs::write(&f, bytes).map_err(|e| format!("{f}: {e}"))?;
        let note = format!(
            "KS0 (RG-1, H from SPIKE-127's K6') of the window {:?} (face, i0, i1, j0, j1 in regional cells, seed), margin {margin}
file: {f}
format: little-endian f64, row-major, {w} x {h} L cells, cell (0, 0) = global L cell ({x0}, {y0}) of face {}; L = 8 cells a regional cell; cell size {:.4} m at the face's centre metric
counted region (the box): L cells x {}..{}, y {}..{} of this grid (half-open)
sea level 0 m; land = S0 > 0; KS0 = S0 - min(max(0, r - H), budget) on land outside H1's lake water, S0 elsewhere
S0 = the river gate's start surface of the window (B + C1(z_s - z_b)); spike code: spike/ks-g1b, crates/cache/examples/ks_g1b.rs
",
            win, face.0, front.solve.grid.dx() / l as f64, bi0 - x0, bi1 - x0, bj0 - y0, bj1 - y0
        );
        std::fs::write(format!("{out_dir}/ks0-k6p.txt"), note).map_err(|e| e.to_string())?;
        println!("KS KS0 written: {f}");
    }

    // Routings at L: S0 (channels for saddles, divides), KS0 (H_f, channels), KS1 (channels).
    let lg = planet_generators::bake::Grid::new(w, h, front.solve.grid.dx() / l as f64);
    let route_l = |z: &[f64]| {
        let outlet = lg.outlets(z, sea);
        route_rules(&lg, z, &outlet, 1.0e-3, front.solve.stream, 0.0, None, RoundRules::default())
    };
    let chans = |z: &[f64], ro: &planet_generators::bake::route::Routing| -> (Vec<bool>, Vec<bool>) {
        ((0..n_l).map(|p| ro.area[p] >= 1.0e6 && z[p] > sea).collect(), (0..n_l).map(|p| ro.area[p] >= 5.0e6 && z[p] > sea).collect())
    };
    if dump_only {
        // SPIKE-135: RG-1's and RG-2's lowering at L, as the code below computes them (KS1 from the routing of KS0 at L, A_crit 1 km2).
        let ro = route_l(&ks0);
        let (ch_k0_1, _) = chans(&ks0, &ro);
        let mut tgt = vec![0usize; n_l];
        for &p in &ro.order {
            let p = p as usize;
            let rc = ro.receiver[p];
            tgt[p] = if ch_k0_1[p] || rc == NONE { p } else { tgt[rc as usize] };
        }
        drop(ro);
        let hand_f: Vec<f64> = (0..n_l).map(|p| (ks0[p] - ks0[tgt[p]].max(sea)).max(0.0)).collect();
        drop(tgt);
        let (_ks1, used1, _want1) = apply(&hand_f);
        for (name, v) in [("rg1", &used0), ("rg2", &used1)] {
            let f = format!("{out_dir}/ks-{name}-used-{w}x{h}.f32");
            let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
            std::fs::write(&f, bytes).map_err(|e| format!("{f}: {e}"))?;
            let n = v.iter().filter(|&&x| x > 0.0).count();
            println!("KS dump {name}: {f}; {n} cells lowered; {:.0} s", t0.elapsed().as_secs_f64());
        }
        return Ok(());
    }
    let ro = route_l(&start);
    let (ch_s0_1, ch_s0_5) = chans(&start, &ro);
    // Divides: catchments of the 75 km2 network's streams (a stream runs from a head down through every confluence along its larger donor; direct drainage to an outlet is one catchment); a land cell with a land 4-neighbour of another catchment.
    let river: Vec<bool> = (0..n_l).map(|p| ro.area[p] >= 75.0e6 && land(p)).collect();
    let mut best = vec![(f64::NEG_INFINITY, u32::MAX); n_l];
    let mut seg = vec![u32::MAX; n_l];
    for &p in ro.order.iter().rev() {
        let p = p as usize;
        if !river[p] {
            continue;
        }
        seg[p] = if best[p].1 == u32::MAX { p as u32 } else { seg[best[p].1 as usize] };
        let rc = ro.receiver[p];
        if rc != NONE && river[rc as usize] {
            let cand = (ro.area[p], p as u32);
            let b = &mut best[rc as usize];
            if cand.0 > b.0 || (cand.0 == b.0 && cand.1 < b.1) {
                *b = cand;
            }
        }
    }
    drop(best);
    let mut label = vec![u32::MAX; n_l];
    for &p in &ro.order {
        let p = p as usize;
        let rc = ro.receiver[p];
        // A chain that reaches an outlet without a river cell drains to the coast or the window's edge directly: one shared label, so the coast's own strips are not divides.
        label[p] = if river[p] {
            seg[p]
        } else if rc == NONE {
            u32::MAX - 1
        } else {
            label[rc as usize]
        };
    }
    drop((river, seg, ro));
    let mut dist = vec![u16::MAX; n_l];
    for p in 0..n_l {
        if !land(p) {
            continue;
        }
        let (x, y) = (p % w, p / w);
        let nb = [(x > 0).then(|| p - 1), (x + 1 < w).then(|| p + 1), (y > 0).then(|| p - w), (y + 1 < h).then(|| p + w)];
        if nb.into_iter().flatten().any(|q| land(q) && label[q] != label[p]) {
            dist[p] = 0;
        }
    }
    drop(label);
    // Chebyshev distance by two chamfer passes.
    for y in 0..h {
        for x in 0..w {
            let p = y * w + x;
            let mut d = dist[p];
            for (dx, dy) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0 && ny >= 0 && nx < w as i64 {
                    d = d.min(dist[ny as usize * w + nx as usize].saturating_add(1));
                }
            }
            dist[p] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let p = y * w + x;
            let mut d = dist[p];
            for (dx, dy) in [(1i64, 1i64), (0, 1), (-1, 1), (1, 0)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx >= 0 && nx < w as i64 && ny < h as i64 {
                    d = d.min(dist[ny as usize * w + nx as usize].saturating_add(1));
                }
            }
            dist[p] = d;
        }
    }
    let near_div: Vec<bool> = dist.iter().map(|&d| d <= 10).collect();
    drop(dist);
    println!("KS routing S0 and divides: {:.0} s", t0.elapsed().as_secs_f64());

    let ro = route_l(&ks0);
    let (ch_k0_1, ch_k0_5) = chans(&ks0, &ro);
    let mut tgt = vec![0usize; n_l];
    for &p in &ro.order {
        let p = p as usize;
        let rc = ro.receiver[p];
        tgt[p] = if ch_k0_1[p] || rc == NONE { p } else { tgt[rc as usize] };
    }
    drop(ro);
    let hand_f: Vec<f64> = (0..n_l).map(|p| (ks0[p] - ks0[tgt[p]].max(sea)).max(0.0)).collect();
    drop(tgt);
    let (ks1, used1, want1) = apply(&hand_f);
    drop(hand_f);
    let ro = route_l(&ks1);
    let (ch_k1_1, ch_k1_5) = chans(&ks1, &ro);
    drop(ro);
    println!("KS routings KS0, KS1: {:.0} s", t0.elapsed().as_secs_f64());

    // Ridge cells of S0 (G0 contracts section 9).
    let ridge: Vec<bool> = (0..n_l)
        .map(|p| {
            if !inside_land(p) {
                return false;
            }
            let (x, y) = ((p % w) as i64, (p / w) as i64);
            [(1i64, 0i64), (0, 1), (1, 1), (1, -1)].iter().any(|&(dx, dy)| {
                let a = (x - 2 * dx, y - 2 * dy);
                let b = (x + 2 * dx, y + 2 * dy);
                let ok = |q: (i64, i64)| q.0 >= 0 && q.1 >= 0 && q.0 < w as i64 && q.1 < h as i64;
                ok(a)
                    && ok(b)
                    && start[p] >= start[a.1 as usize * w + a.0 as usize] + 2.0
                    && start[p] >= start[b.1 as usize * w + b.0 as usize] + 2.0
            })
        })
        .collect();
    let ridge_n = ridge.iter().filter(|&&b| b).count();

    let gate = |z: &[f64]| {
        let outlet = lg.outlets(z, sea);
        measure(
            &GateInput {
                grid: &lg,
                start: &start,
                surface: z,
                outlet: &outlet,
                inside: &in_box,
                region: &in_box,
                h1: &[],
                seed: front.solve.stream,
                sea_level_m: sea,
                outside_drains: false,
                trunks: &[],
                g2: None,
            },
            &params,
        )
    };
    let quant = |v: &mut Vec<f64>, f: f64| {
        if v.is_empty() {
            return 0.0;
        }
        let k = ((v.len() as f64 * f) as usize).min(v.len() - 1);
        *v.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1
    };
    let label_w = format!("{},{},{},{},{},{}{}", win[0], win[1], win[2], win[3], win[4], win[5], if k6p { " K6p" } else { "" });
    let mut csv = String::new();
    let rungs: [(&str, &[f64], Option<(&[f32], &[f32])>, Option<(&[bool], &[bool])>); 3] = [
        ("RG-0", &start, None, None),
        ("RG-1", &ks0, Some((&used0, &want0)), Some((&ch_k0_1, &ch_k0_5))),
        ("RG-2", &ks1, Some((&used1, &want1)), Some((&ch_k1_1, &ch_k1_5))),
    ];
    for (name, z, uw, ch) in rungs {
        let rep = gate(z);
        let f = rep.fields();
        let field_of = |k: &str| f.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone()).unwrap_or_default();
        // Lowering against the budget, land cells inside.
        let bins = [0.0, 0.1, 0.25, 0.5, 0.75, 0.95, 1.0];
        let mut hist = [0usize; 9]; // 0: none; 1..=6: (b[k-1], b[k]]; 7: at the budget without residual is in 6; 8: residual
        let (mut lowered, mut vol, mut vol_req, mut resid, mut resid_floor, mut to_sea, mut clipped, mut pos) =
            (Vec::new(), 0.0, 0.0, 0usize, 0usize, 0usize, 0usize, 0usize);
        let (mut near_n, mut near_low, mut near_vol, mut far_n, mut far_low, mut far_vol) = (0usize, 0usize, 0.0, 0usize, 0usize, 0.0);
        let mut near_used: Vec<f64> = Vec::new();
        let mut far_used: Vec<f64> = Vec::new();
        let mut map_ratio = vec![0f32; rn];
        let mut map_resid = vec![0f32; rn];
        for p in (0..n_l).filter(|&p| inside_land(p)) {
            pos += usize::from(r[p] > 0.0);
            if z[p] <= sea {
                to_sea += 1;
            }
            let (u, wn) = uw.map_or((0.0, 0.0), |(u, wn)| (f64::from(u[p]), f64::from(wn[p])));
            let b = budget[reg(p)];
            let ratio = u / b;
            let k = if wn > b + 1e-9 {
                8
            } else if u <= 0.0 {
                0
            } else {
                (1..bins.len()).find(|&k| ratio <= bins[k] + 1e-12).unwrap_or(6)
            };
            hist[k] += 1;
            if u > 0.0 {
                lowered.push(u);
                vol += u;
                clipped += usize::from(r[p] > 0.0);
            }
            vol_req += wn;
            if k == 8 {
                resid += 1;
                resid_floor += usize::from(b <= 10.0);
                map_resid[reg(p)] += 1.0;
            }
            map_ratio[reg(p)] = map_ratio[reg(p)].max(ratio as f32);
            if near_div[p] {
                near_n += 1;
                near_low += usize::from(u > 0.0);
                near_vol += u;
                if u > 0.0 {
                    near_used.push(u);
                }
            } else {
                far_n += 1;
                far_low += usize::from(u > 0.0);
                far_vol += u;
                if u > 0.0 {
                    far_used.push(u);
                }
            }
        }
        let at_p95 = hist[6] + hist[8];
        // Ridges and saddles against S0.
        let (mut ridge20, mut ridge_low) = (0usize, Vec::new());
        let mut saddle1 = vec![false; n_l];
        let mut saddle5 = vec![false; n_l];
        for p in (0..n_l).filter(|&p| ridge[p]) {
            let d = start[p] - z[p];
            if d > 0.0 {
                ridge_low.push(d);
            }
            if d > 20.0 {
                ridge20 += 1;
                if let Some((c1, c5)) = ch {
                    let (x, y) = (p % w, p / w);
                    for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                        for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                            let q = yy * w + xx;
                            saddle1[p] |= c1[q] && !ch_s0_1[q];
                            saddle5[p] |= c5[q] && !ch_s0_5[q];
                        }
                    }
                }
            }
        }
        let components = |m: &[bool]| -> (usize, usize) {
            let mut seen = vec![false; n_l];
            let (mut cells, mut comps) = (0usize, 0usize);
            for s in 0..n_l {
                if !m[s] || seen[s] {
                    continue;
                }
                comps += 1;
                let mut stack = vec![s];
                seen[s] = true;
                while let Some(p) = stack.pop() {
                    cells += 1;
                    let (x, y) = (p % w, p / w);
                    for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                        for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                            let q = yy * w + xx;
                            if m[q] && !seen[q] {
                                seen[q] = true;
                                stack.push(q);
                            }
                        }
                    }
                }
            }
            (cells, comps)
        };
        let (s1_cells, s1_comps) = components(&saddle1);
        let (s5_cells, s5_comps) = components(&saddle5);
        let low_n = lowered.len();
        let (lp50, lp95, lmax) = (quant(&mut lowered, 0.5), quant(&mut lowered, 0.95), lowered.iter().copied().fold(0.0, f64::max));
        let rp95 = quant(&mut ridge_low, 0.95);
        let (np95, fp95) = (quant(&mut near_used, 0.95), quant(&mut far_used, 0.95));
        let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
        println!("KS {name} {label_w}: lowering on {low_n} of {land_n} land cells inside ({:.2} %), p50 {lp50:.2} m, p95 {lp95:.2} m, max {lmax:.2} m, volume {:.3} km3 (requested {:.3} km3); residual {resid} cells ({:.3} %), {resid_floor} of them at the 10 m floor; at 95-100 % of the budget or residual {at_p95} ({:.2} %); land below the sea {to_sea}", pct(low_n, land_n), vol * cell_km2 / 1000.0, vol_req * cell_km2 / 1000.0, pct(resid, land_n), pct(at_p95, land_n));
        println!("KS {name} hist (share of land cells inside by lowering / budget): none {:.2} | <=10 % {:.2} | 10-25 % {:.2} | 25-50 % {:.2} | 50-75 % {:.2} | 75-95 % {:.2} | 95-100 % {:.2} | residual {:.3}", pct(hist[0], land_n), pct(hist[1], land_n), pct(hist[2], land_n), pct(hist[3], land_n), pct(hist[4], land_n), pct(hist[5], land_n), pct(hist[6], land_n), pct(hist[8], land_n));
        println!("KS {name} band: {clipped} of {pos} cells with r > 0 clipped ({:.2} %); near divides (<= 1 km, {near_n} cells): {:.2} % lowered, p95 {np95:.2} m, {:.1} % of the volume on {:.1} % of the land; elsewhere ({far_n}): {:.2} % lowered, p95 {fp95:.2} m", pct(clipped, pos), pct(near_low, near_n), 100.0 * near_vol / (near_vol + far_vol).max(1e-12), pct(near_n, land_n), pct(far_low, far_n));
        println!("KS {name} ridges: {ridge_n} ridge cells of S0; {ridge20} lowered over 20 m ({:.2} %), p95 lowering {rp95:.2} m; cut-through saddles at 1 km2 {s1_comps} ({s1_cells} cells), at 5 km2 {s5_comps} ({s5_cells} cells)", pct(ridge20, ridge_n));
        println!(
            "KS {name} gate: basins {} G-DRAIN {} % (of {} river cells), lowered {} %, relief10 {} / start {} m, uphill {} %",
            field_of("basins"),
            field_of("basin_pct"),
            field_of("river_cells"),
            field_of("lowered_pct"),
            field_of("relief10_m"),
            field_of("relief10_start_m"),
            field_of("uphill_pct")
        );
        csv.push_str(&format!(
            "{name},{label_w},{land_n},{low_n},{lp50:.3},{lp95:.3},{lmax:.3},{:.4},{:.4},{resid},{resid_floor},{at_p95},{to_sea},{},{},{},{},{},{},{},{},{clipped},{pos},{near_n},{near_low},{np95:.3},{:.4},{far_n},{far_low},{fp95:.3},{ridge_n},{ridge20},{rp95:.3},{s1_comps},{s1_cells},{s5_comps},{s5_cells},{},{},{},{},{},{},{}\n",
            vol * cell_km2 / 1000.0,
            vol_req * cell_km2 / 1000.0,
            hist[0], hist[1], hist[2], hist[3], hist[4], hist[5], hist[6], hist[8],
            near_vol / (near_vol + far_vol).max(1e-12),
            field_of("basins"), field_of("basin_pct"), field_of("river_cells"), field_of("lowered_pct"), field_of("relief10_m"), field_of("relief10_start_m"), field_of("uphill_pct")
        ));
        let write = |name2: &str, v: &[f32]| {
            let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
            std::fs::write(format!("{out_dir}/{name2}"), bytes).expect("map written");
        };
        if uw.is_some() {
            write(&format!("{name}-ratio-{rw}x{rh}.f32"), &map_ratio);
            write(&format!("{name}-resid-{rw}x{rh}.f32"), &map_resid);
        }
        println!("KS {name} done: {:.0} s", t0.elapsed().as_secs_f64());
    }
    let zs32: Vec<f32> = solve.zs.iter().map(|&v| v as f32).collect();
    let bud32: Vec<f32> = budget.iter().map(|&v| v as f32).collect();
    for (n2, v) in [("zs", &zs32), ("budget", &bud32), ("hand", &hand)] {
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        std::fs::write(format!("{out_dir}/{n2}-{rw}x{rh}.f32"), bytes).map_err(|e| e.to_string())?;
    }
    let header = "rung,window,land,lowered,low_p50_m,low_p95_m,low_max_m,vol_km3,vol_req_km3,resid,resid_floor,at95,to_sea,h_none,h_10,h_25,h_50,h_75,h_95,h_100,h_resid,clipped,pos_band,near_n,near_low,near_p95_m,near_vol_share,far_n,far_low,far_p95_m,ridge_n,ridge20,ridge_p95_m,saddle1,saddle1_cells,saddle5,saddle5_cells,basins,basin_pct,river_cells,lowered_pct,relief10_m,relief10_start_m,uphill_pct\n";
    std::fs::write(format!("{out_dir}/ks.csv"), format!("{header}{csv}")).map_err(|e| e.to_string())?;
    let mut bsorted = budget.clone();
    println!(
        "KS budget field: p5 {:.1} p50 {:.1} p95 {:.1} m over {rn} cells; total {:.0} s",
        quant(&mut bsorted, 0.05),
        quant(&mut bsorted, 0.5),
        quant(&mut bsorted, 0.95),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}
