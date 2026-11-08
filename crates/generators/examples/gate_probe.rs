//! SPIKE-106 (inbox #103): an independent measurement of the prototype's gates. Probe code of a spike branch, never merged.
//!
//! What is independent of `region_scale`: the window, the outlets, the inputs (read in another thread layout and cross-checked against a single-thread read), the filled surface (a Dijkstra with the
//! opposite tie order, and for the small window a sweep relaxation of the definition), the receivers, the clipped MFD area, `Q`, the channel cells, the pit components, the cell areas (Gauss-Legendre
//! of the gnomonic solid angle) and distances (great circle between the cell directions). What is not: the code under test (`carve`, `solve_rounds`, `partitioned_route`, `hydrology`), `jitter`,
//! `Grid` neighbours and the runoff functions.
//!
//! Usage: gate_probe survey <seed> | gate_probe gates <name> <face> <i0> <i1> <j0> <j1> <seed> <margin> <threads> | gate_probe spiral
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{jitter, route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::Instant;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

// ---------------------------------------------------------------------------------------------------------------------------------------------------
// the inputs

fn face_spec(face: u8, seed: u64) -> FaceSpec {
    FaceSpec { face: Face(face), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 }
}

/// The base over a window, rows dealt round-robin to the threads (explorer's example gives each thread a block of rows).
fn sample(inputs: &BakeInputs, spec: &FaceSpec, i0: usize, j0: usize, w: usize, h: usize, threads: usize) -> Vec<f64> {
    let mut out = vec![0.0f64; w * h];
    let rows: Vec<(usize, &mut [f64])> = out.chunks_mut(w).enumerate().collect();
    let mut bins: Vec<Vec<(usize, &mut [f64])>> = (0..threads).map(|_| Vec::new()).collect();
    for (r, row) in rows {
        bins[r % threads].push((r, row));
    }
    std::thread::scope(|s| {
        for bin in bins {
            s.spawn(move || {
                for (r, row) in bin {
                    for (c, v) in row.iter_mut().enumerate() {
                        let d: Vec3 = spec.halo_direction((i0 + c) as i64, (j0 + r) as i64);
                        *v = inputs.base(d);
                    }
                }
            });
        }
    });
    out
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------
// independent kernels

#[derive(PartialEq)]
struct Item(f64, u32);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Item) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    /// A max-heap: the lowest level first, and on equal levels the HIGHEST cell id first (the opposite of the canonical flood's tie rule).
    fn cmp(&self, o: &Item) -> Ordering {
        o.0.total_cmp(&self.0).then(self.1.cmp(&o.1))
    }
}

/// The filled surface by a Dijkstra with the opposite tie order: the fixed point of `F(c) = max(z(c), min over neighbours F(q) + eps)`, an outlet `z`.
fn flood_dijkstra(grid: &Grid, z: &[f64], outlet: &[bool], eps: f64) -> Vec<f64> {
    let n = grid.len();
    let mut f = vec![f64::INFINITY; n];
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    for p in 0..n {
        if outlet[p] {
            f[p] = z[p];
            heap.push(Item(f[p], p as u32));
        }
    }
    while let Some(Item(v, p)) = heap.pop() {
        let p = p as usize;
        if done[p] || v > f[p] {
            continue;
        }
        done[p] = true;
        for d in 0..8 {
            if let Some(q) = grid.neighbour(p, d) {
                if done[q] || outlet[q] {
                    continue;
                }
                let cand = z[q].max(f[p] + eps);
                if cand < f[q] {
                    f[q] = cand;
                    heap.push(Item(cand, q as u32));
                }
            }
        }
    }
    f
}

/// The same fixed point by alternating sweeps of the definition (no queue at all). Returns the surface and the sweeps.
fn flood_sweeps(grid: &Grid, z: &[f64], outlet: &[bool], eps: f64, max_sweeps: usize) -> (Vec<f64>, usize) {
    let n = grid.len();
    let mut f: Vec<f64> = (0..n).map(|p| if outlet[p] { z[p] } else { f64::INFINITY }).collect();
    let mut sweeps = 0;
    loop {
        let mut changed = false;
        sweeps += 1;
        for pass in 0..2 {
            let mut visit = |p: usize| {
                if outlet[p] {
                    return;
                }
                let mut m = f64::INFINITY;
                for d in 0..8 {
                    if let Some(q) = grid.neighbour(p, d) {
                        m = m.min(f[q]);
                    }
                }
                let v = z[p].max(m + eps);
                if v < f[p] {
                    f[p] = v;
                    changed = true;
                }
            };
            if pass == 0 {
                (0..n).for_each(&mut visit);
            } else {
                (0..n).rev().for_each(&mut visit);
            }
        }
        if !changed || sweeps >= max_sweeps {
            break;
        }
    }
    (f, sweeps)
}

/// The receiver by the documented rule: the neighbour of the steepest jittered slope `slope (1 + 0.5 u)` over the strictly lower neighbours; the first of equal slopes in direction order.
fn my_receivers(grid: &Grid, f: &[f64], outlet: &[bool], seed: u64) -> Vec<u32> {
    let n = grid.len();
    let mut rec = vec![NONE; n];
    for p in 0..n {
        if outlet[p] {
            continue;
        }
        let (mut best, mut arg) = (0.0f64, NONE);
        for d in 0..8 {
            if let Some(q) = grid.neighbour(p, d) {
                let mut s = (f[p] - f[q]) / grid.distance_at(p, d);
                if s > 0.0 {
                    s *= 1.0 + 0.5 * jitter(seed, grid.global_id(p), d);
                    if s > best {
                        best = s;
                        arg = q as u32;
                    }
                }
            }
        }
        rec[p] = arg;
    }
    rec
}

/// The cells by (filled level, cell id) ascending: a receiver comes before the cell that drains into it.
fn order_asc(f: &[f64]) -> Vec<u32> {
    let mut o: Vec<u32> = (0..f.len() as u32).collect();
    o.sort_unstable_by(|&a, &b| f[a as usize].total_cmp(&f[b as usize]).then(a.cmp(&b)));
    o
}

fn my_roots(rec: &[u32], asc: &[u32]) -> Vec<u32> {
    let mut root = vec![NONE; rec.len()];
    for &c in asc {
        let c = c as usize;
        root[c] = if rec[c] == NONE { c as u32 } else { root[rec[c] as usize] };
    }
    root
}

/// The clipped MFD effective area, from the definition: cells highest first (ties by id), each shares what it holds among its strictly lower neighbours of its own catchment by `slope^1.1`.
fn my_area(grid: &Grid, f: &[f64], outlet: &[bool], root: &[u32], w: Option<&[f32]>, asc: &[u32]) -> Vec<f64> {
    let n = grid.len();
    let mut area: Vec<f64> = (0..n).map(|p| grid.cell_area(p) * w.map_or(1.0, |w| f64::from(w[p]))).collect();
    // The donor order is (level descending, id ascending): ascending order reversed gives (descending, id descending), so equal levels are re-ordered by id ascending below.
    let mut desc: Vec<u32> = asc.to_vec();
    desc.sort_by(|&a, &b| f[b as usize].total_cmp(&f[a as usize]).then(a.cmp(&b)));
    for &p in &desc {
        let p = p as usize;
        if outlet[p] {
            continue;
        }
        let mut wt = [0.0f64; 8];
        let mut sum = 0.0;
        for d in 0..8 {
            if let Some(q) = grid.neighbour(p, d) {
                let slope = (f[p] - f[q]) / grid.distance_at(p, d);
                if slope > 0.0 && root[q] == root[p] {
                    wt[d] = libm::pow(slope, 1.1);
                    sum += wt[d];
                }
            }
        }
        if sum > 0.0 {
            let held = area[p];
            for d in 0..8 {
                if wt[d] > 0.0 {
                    let q = grid.neighbour(p, d).unwrap();
                    area[q] += held * wt[d] / sum;
                }
            }
        }
    }
    area
}

fn diff_u32(a: &[u32], b: &[u32]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}
fn diff_bits(a: &[f64], b: &[f64]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}
fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs() / x.abs().max(y.abs()).max(1e-300)).fold(0.0, f64::max)
}

/// Routing of `z` checked against the independent kernels: returns the filled surface.
fn check_routing(label: &str, grid: &Grid, z: &[f64], outlet: &[bool], eps: f64, seed: u64, w: Option<&[f32]>, tile: &[usize], sweeps: bool) -> Vec<f64> {
    let t = Instant::now();
    let fd = flood_dijkstra(grid, z, outlet, eps);
    let td = t.elapsed().as_secs_f64();
    let rules = RoundRules::ROUND;
    let res = route_rules(grid, z, outlet, eps, seed, 0.0, w, rules);
    let lib_f = planet_generators::bake::flood::priority_flood_with(grid, z, outlet, eps, true);
    print!("[{label}] flood: Dijkstra(opposite ties) {td:.1} s, {} cells differ from the canonical flood", diff_bits(&fd, &lib_f));
    if sweeps {
        let t = Instant::now();
        let (fs, n) = flood_sweeps(grid, z, outlet, eps, 5000);
        print!("; sweep relaxation {n} sweeps {:.1} s, {} differ", t.elapsed().as_secs_f64(), diff_bits(&fs, &fd));
    }
    println!();
    let rec = my_receivers(grid, &fd, outlet, seed);
    let asc = order_asc(&fd);
    let root = my_roots(&rec, &asc);
    let area = my_area(grid, &fd, outlet, &root, w, &asc);
    println!(
        "[{label}] resident route vs own: {} receivers, {} A_d (bits) differ; max rel A_d {:.1e}",
        diff_u32(&rec, &res.receiver),
        diff_bits(&area, &res.area),
        max_rel(&area, &res.area)
    );
    if w.is_none() {
        for &tl in tile {
            let mut cache = PartCache::default();
            let t = Instant::now();
            let part = partitioned_route(grid, z, outlet, eps, seed, tl, rules, &mut cache);
            println!(
                "[{label}] G-PART tile {tl}: {} tiles, {} sweeps, {} floods, {:.1} s; vs resident: {} filled, {} receivers, {} A_d; vs own: {} filled, {} receivers, {} A_d",
                part.stats.tiles,
                part.stats.sweeps,
                part.stats.floods,
                t.elapsed().as_secs_f64(),
                diff_bits(&part.filled, &lib_f),
                diff_u32(&part.routing.receiver, &res.receiver),
                diff_bits(&part.routing.area, &res.area),
                diff_bits(&part.filled, &fd),
                diff_u32(&part.routing.receiver, &rec),
                diff_bits(&part.routing.area, &area)
            );
        }
    }
    fd
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------
// geometry

/// Gauss-Legendre 4-point nodes and weights on [-1, 1].
const GL: [(f64, f64); 4] =
    [(-0.861_136_311_594_052_6, 0.347_854_845_137_453_85), (-0.339_981_043_584_856_26, 0.652_145_154_862_546_1), (0.339_981_043_584_856_26, 0.652_145_154_862_546_1), (0.861_136_311_594_052_6, 0.347_854_845_137_453_85)];

fn geometry_check(grid: &Grid, spec: &FaceSpec, i0: usize, j0: usize) {
    let (w, h) = (grid.w(), grid.h());
    let n = CELLS as f64;
    let tan = |s: f64| libm::tan(core::f64::consts::FRAC_PI_4 * s);
    let mut worst_area = 0.0f64;
    let mut worst_dist = 0.0f64;
    let mut seedx = 0x1234_5678_9ABC_DEF0u64;
    for _ in 0..3000 {
        seedx = seedx.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let p = (seedx >> 33) as usize % (w * h);
        let (i, j) = (i0 + p % w, j0 + p / w);
        let (x0, x1) = (tan(2.0 * i as f64 / n - 1.0), tan(2.0 * (i + 1) as f64 / n - 1.0));
        let (y0, y1) = (tan(2.0 * j as f64 / n - 1.0), tan(2.0 * (j + 1) as f64 / n - 1.0));
        let mut a = 0.0;
        for (u, wu) in GL {
            for (v, wv) in GL {
                let x = 0.5 * (x0 + x1) + 0.5 * (x1 - x0) * u;
                let y = 0.5 * (y0 + y1) + 0.5 * (y1 - y0) * v;
                a += wu * wv * libm::pow(1.0 + x * x + y * y, -1.5);
            }
        }
        a *= 0.25 * (x1 - x0) * (y1 - y0) * R * R;
        worst_area = worst_area.max((a - grid.cell_area(p)).abs() / a);
        for d in 0..8 {
            if let Some(q) = grid.neighbour(p, d) {
                let (qi, qj) = (i0 + q % w, j0 + q / w);
                let (da, db) = (spec.halo_direction(i as i64, j as i64), spec.halo_direction(qi as i64, qj as i64));
                let cross = [da.y * db.z - da.z * db.y, da.z * db.x - da.x * db.z, da.x * db.y - da.y * db.x];
                let sin = libm::sqrt(cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]);
                let dot = da.x * db.x + da.y * db.y + da.z * db.z;
                let dist = R * libm::atan2(sin, dot);
                worst_dist = worst_dist.max((dist - grid.distance_at(p, d)).abs() / dist);
            }
        }
    }
    println!("geometry (3000 cells, 8 neighbours each): cell area vs Gauss-Legendre of the gnomonic solid angle: max rel {worst_area:.1e}; distance vs great circle of the cell directions: max rel {worst_dist:.1e}");
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------
// H1

#[allow(clippy::too_many_arguments)]
fn h1_check(grid: &Grid, z: &[f64], z0: &[f64], outlet: &[bool], spec: &FaceSpec, inputs: &BakeInputs, seed: u64, i0: usize, j0: usize, threads: usize) {
    let (w, n) = (grid.w(), grid.len());
    let o = inputs.orography();
    let mut weight = vec![1.0f32; n];
    let mut runoff = vec![0.0f32; n];
    let mut evap = vec![0.0f32; n];
    let t = Instant::now();
    {
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (((wc, rc), ec), k) in weight.chunks_mut(rows).zip(runoff.chunks_mut(rows)).zip(evap.chunks_mut(rows)).zip(0..) {
                s.spawn(move || {
                    for c in 0..wc.len() {
                        let p = k * rows + c;
                        if z0[p] <= 0.0 {
                            continue;
                        }
                        let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                        let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                        let ro = runoff_mm(pm, tc);
                        wc[c] = discharge_weight(ro) as f32;
                        rc[c] = (ro / 1000.0 / SECONDS_PER_YEAR) as f32;
                        ec[c] = ((pet_mm(tc) - pm).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
                    }
                });
            }
        });
    }
    println!("[H1] climate read {:.1} s", t.elapsed().as_secs_f64());
    let eps = 1e-3;
    let fd = check_routing("H1 carved, weighted", grid, z, outlet, eps, seed, Some(&weight), &[], false);
    let routing = route_rules(grid, z, outlet, eps, seed, 0.0, Some(&weight), RoundRules::ROUND);
    let params = HydroParams::default();
    let hy = hydrology(grid, z, &fd, &routing, outlet, Some(&runoff), Some(&evap), &params);
    // own Q: runoff over every active non-outlet cell, along the D8 tree, highest filled level first
    let rec = my_receivers(grid, &fd, outlet, seed);
    let mut desc = order_asc(&fd);
    desc.sort_by(|&a, &b| fd[b as usize].total_cmp(&fd[a as usize]).then(a.cmp(&b)));
    let mut q: Vec<f64> = (0..n).map(|p| if outlet[p] { 0.0 } else { grid.cell_area(p) * f64::from(runoff[p]) }).collect();
    for &c in &desc {
        let c = c as usize;
        if rec[c] != NONE {
            let r = rec[c] as usize;
            q[r] += q[c];
        }
    }
    let q_rel = max_rel(&q, &hy.q);
    // own channel set: initiated, then downstream
    let mut ch: Vec<bool> = (0..n).map(|p| !outlet[p] && routing.area[p] >= params.initiation_m2).collect();
    for &c in &desc {
        let c = c as usize;
        if ch[c] && rec[c] != NONE {
            ch[rec[c] as usize] = true;
        }
    }
    let ch_diff = ch.iter().zip(&hy.channel).filter(|(a, b)| a != b).count();
    // channel structure: heads, confluences, mouths, no mid-course end, no restart
    let mut donors = vec![0u32; n];
    for p in 0..n {
        if ch[p] && rec[p] != NONE {
            donors[rec[p] as usize] += 1;
        }
    }
    let (mut heads, mut conf, mut mouths, mut broken, mut restart) = (0, 0, 0, 0, 0);
    for p in 0..n {
        if !ch[p] {
            continue;
        }
        if rec[p] == NONE {
            mouths += 1;
        } else if !ch[rec[p] as usize] && !outlet[rec[p] as usize] {
            broken += 1;
        }
        if donors[p] == 0 && rec[p] != NONE {
            heads += 1;
            if routing.area[p] < params.initiation_m2 {
                restart += 1;
            }
        }
        if donors[p] >= 2 {
            conf += 1;
        }
    }
    // conservation, own sums over active cells
    let a_local: f64 = (0..n).filter(|&p| !outlet[p]).map(|p| grid.cell_area(p) * f64::from(runoff[p])).sum();
    let q_out: f64 = (0..n).filter(|&p| rec[p] == NONE).map(|p| q[p]).sum();
    let ad_out: f64 = (0..n).filter(|&p| rec[p] == NONE).map(|p| routing.area[p]).sum();
    let ad_in: f64 = (0..n).map(|p| grid.cell_area(p) * f64::from(weight[p])).sum();
    // confluence: Q at every channel cell against its donors, from my Q and my receivers
    let mut conf_err = 0.0f64;
    let mut sums = vec![0.0f64; n];
    for p in 0..n {
        if rec[p] != NONE {
            sums[rec[p] as usize] += q[p];
        }
    }
    for p in 0..n {
        if ch[p] && !outlet[p] {
            let local = grid.cell_area(p) * f64::from(runoff[p]);
            conf_err = conf_err.max(((sums[p] + local) - q[p]).abs() / q[p].abs().max(1e-300));
        }
    }
    println!(
        "[H1] own Q vs lib Q: max rel {q_rel:.1e}; channel cells: own {} vs lib {} ({ch_diff} differ); heads {heads} (lib {}), confluences {conf} (lib {}), mouths {mouths} (lib {}); channel ends mid-course {broken}, heads under T (a restart) {restart}",
        ch.iter().filter(|b| **b).count(),
        hy.stats.channel_cells,
        hy.stats.heads,
        hy.stats.confluences,
        hy.stats.mouths
    );
    println!(
        "[H1] conservation (own sums): sum Q at outlets vs sum a r: {:.1e}; sum A_d at outlets vs sum a w: {:.1e}; own confluence error {conf_err:.1e}",
        (q_out - a_local).abs() / a_local.max(1e-300),
        (ad_out - ad_in).abs() / ad_in.max(1e-300)
    );
    // pit components, own
    let pit = |p: usize| !outlet[p] && fd[p] - z[p] > params.pit_min_m;
    let mut seen = vec![false; n];
    let (mut comps, mut cells, mut stack) = (0usize, 0usize, Vec::new());
    for p0 in 0..n {
        if seen[p0] || !pit(p0) {
            continue;
        }
        comps += 1;
        seen[p0] = true;
        stack.push(p0);
        while let Some(c) = stack.pop() {
            cells += 1;
            for d in 0..8 {
                if let Some(q2) = grid.neighbour(c, d) {
                    if !seen[q2] && pit(q2) {
                        seen[q2] = true;
                        stack.push(q2);
                    }
                }
            }
        }
    }
    let land: usize = (0..n).filter(|&p| z[p] > 0.0).count();
    let kept: u32 = hy.basins.iter().map(|b| b.cells).sum();
    println!(
        "[H1] pit components (F - z > {} m): own {comps} vs lib basins {} + artefacts {} + ramps {} = {}; pit cells own {cells} vs lib {}; kept basin cells {kept}; unkept pit cells {} = {:.3} % of land ({land} land cells)",
        params.pit_min_m,
        hy.stats.basins,
        hy.stats.artefacts,
        hy.stats.ramps,
        hy.stats.basins + hy.stats.artefacts + hy.stats.ramps,
        hy.stats.basin_cells,
        cells as i64 - i64::from(kept),
        100.0 * (cells as i64 - i64::from(kept)) as f64 / land as f64
    );
    let land_km2: f64 = (0..n).filter(|&p| z[p] > 0.0).map(|p| grid.cell_area(p)).sum::<f64>() / 1e6;
    let lakes = hy.basins.iter().filter(|b| b.class == planet_generators::bake::hydro::BasinClass::Lake).count();
    println!(
        "[H1] channel density {:.1} % of land cells; {:.1} heads and {:.1} segments per 1,000 km2; basins {} kept ({lakes} lakes, {} endorheic)",
        100.0 * hy.stats.channel_cells as f64 / land as f64,
        1000.0 * hy.stats.heads as f64 / land_km2,
        1000.0 * hy.stats.segments as f64 / land_km2,
        hy.basins.len(),
        hy.basins.len() - lakes
    );
}

// ---------------------------------------------------------------------------------------------------------------------------------------------------

fn quantile(v: &mut [f64], f: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * f) as usize]
}

fn survey(seed: u64) {
    let threads = 8;
    let inputs = BakeInputs::new(seed, threads);
    let stride = 32usize;
    let m = CELLS / stride;
    let span = 620 / stride + 1;
    let mut rows: Vec<(u8, usize, usize, f64, f64, f64, f64, f64)> = Vec::new();
    for face in 0..6u8 {
        let spec = face_spec(face, seed);
        let t = Instant::now();
        let mut z = vec![0.0f64; m * m];
        let mut p_ = vec![0.0f64; m * m];
        {
            let zs: Vec<(usize, (&mut [f64], &mut [f64]))> = z.chunks_mut(m).zip(p_.chunks_mut(m)).enumerate().collect();
            let mut bins: Vec<Vec<(usize, (&mut [f64], &mut [f64]))>> = (0..threads).map(|_| Vec::new()).collect();
            for (r, row) in zs {
                bins[r % threads].push((r, row));
            }
            std::thread::scope(|s| {
                for bin in bins {
                    let inputs = &inputs;
                    s.spawn(move || {
                        for (r, (zr, pr)) in bin {
                            for c in 0..zr.len() {
                                let d = spec.halo_direction((c * stride + stride / 2) as i64, (r * stride + stride / 2) as i64);
                                zr[c] = inputs.base(d);
                                if zr[c] > 0.0 {
                                    pr[c] = inputs.orography().precipitation_mm(d);
                                }
                            }
                        }
                    });
                }
            });
        }
        eprintln!("face {face}: {:.1} s", t.elapsed().as_secs_f64());
        let margin_s = 200 / stride + 1;
        let mut b = margin_s;
        while b + span + margin_s < m {
            let mut a = margin_s;
            while a + span + margin_s < m {
                let (mut lz, mut pz) = (Vec::new(), Vec::new());
                let mut cells = 0;
                for j in b..b + span {
                    for i in a..a + span {
                        cells += 1;
                        if z[j * m + i] > 0.0 {
                            lz.push(z[j * m + i]);
                            pz.push(p_[j * m + i]);
                        }
                    }
                }
                let land = lz.len() as f64 / cells as f64;
                if lz.len() > 20 {
                    let p50 = quantile(&mut lz.clone(), 0.5);
                    let p95 = quantile(&mut lz.clone(), 0.95);
                    let mx = quantile(&mut lz, 1.0);
                    let pr = quantile(&mut pz, 0.5);
                    rows.push((face, a * stride + stride / 2, b * stride + stride / 2, land, p50, p95, mx, pr));
                }
                a += 4;
            }
            b += 4;
        }
    }
    // hills and plains: mostly land with a coast, land p95 200-800 m, nothing above 1,500 m, p50 precipitation 500-2,500 mm
    let mut ok: Vec<_> = rows.iter().filter(|r| r.3 > 0.7 && r.3 < 0.92 && r.5 > 200.0 && r.5 < 800.0 && r.6 < 1500.0 && r.7 > 500.0 && r.7 < 2500.0).collect();
    ok.sort_by(|a, b| (a.5 - 450.0).abs().total_cmp(&(b.5 - 450.0).abs()));
    println!("seed {seed}: {} windows scanned, {} hills-and-plains candidates; best 12 by land p95 near 450 m (face, i0, j0 of the window start at 12,288 cells; land, z p50, p95, max; precipitation p50):", rows.len(), ok.len());
    let mut taken: Vec<(u8, usize, usize)> = Vec::new();
    for r in ok {
        if taken.iter().any(|t| t.0 == r.0 && t.1.abs_diff(r.1) < 1400 && t.2.abs_diff(r.2) < 1400) {
            continue;
        }
        taken.push((r.0, r.1, r.2));
        println!("  face {} i {} j {} | land {:.0} % | z p50 {:.0} p95 {:.0} max {:.0} | P p50 {:.0}", r.0, r.1, r.2, 100.0 * r.3, r.4, r.5, r.6, r.7);
        if taken.len() >= 12 {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn gates(name: &str, face: u8, i0: usize, i1: usize, j0: usize, j1: usize, seed: u64, margin: usize, threads: usize) {
    let (wi0, wi1) = (i0.saturating_sub(margin), (i1 + margin).min(CELLS - 1));
    let (wj0, wj1) = (j0.saturating_sub(margin), (j1 + margin).min(CELLS - 1));
    let (w, h) = (wi1 - wi0 + 1, wj1 - wj0 + 1);
    let n = w * h;
    let spec = face_spec(face, seed);
    let inputs = BakeInputs::new(seed, threads);
    let t = Instant::now();
    let z0 = sample(&inputs, &spec, wi0, wj0, w, h, threads);
    // cross-check: 200 cells read one by one on this thread
    let mut bad = 0;
    for k in 0..200usize {
        let p = (k * 7919 + 13) % n;
        let d: Vec3 = spec.halo_direction((wi0 + p % w) as i64, (wj0 + p / w) as i64);
        if inputs.base(d).to_bits() != z0[p].to_bits() {
            bad += 1;
        }
    }
    let land = z0.iter().filter(|v| **v > 0.0).count();
    println!(
        "== {name}: face {face} seed {seed} window i {wi0}-{wi1} j {wj0}-{wj1} = {w} x {h} = {n} cells; land {:.1} %; inputs read in {:.1} s, {bad} of 200 spot reads differ from a single-thread read; z0 land p50 {:.0} m p95 {:.0} m max {:.0} m",
        100.0 * land as f64 / n as f64,
        t.elapsed().as_secs_f64(),
        quantile(&mut z0.iter().copied().filter(|v| *v > 0.0).collect::<Vec<_>>(), 0.5),
        quantile(&mut z0.iter().copied().filter(|v| *v > 0.0).collect::<Vec<_>>(), 0.95),
        z0.iter().copied().fold(f64::MIN, f64::max)
    );
    let grid = Grid::new(w, h, spec.dx()).with_origin((wi0 as u32, wj0 as u32)).with_metric(Metric::cube_face(CELLS, R, (wi0 as i64, wj0 as i64), w, h));
    let outlet: Vec<bool> = (0..n).map(|p| p % w == 0 || p / w == 0 || p % w == w - 1 || p / w == h - 1 || z0[p] <= 0.0).collect();
    println!("outlets: own mask vs Grid::outlets: {} cells differ", outlet.iter().zip(grid.outlets(&z0, 0.0)).filter(|(a, b)| **a != *b).count());
    geometry_check(&grid, &spec, wi0, wj0);
    let stream = spec.stream();
    // G-ROUND and G-DET, with thread counts and a carve other than explorer's
    for (steps, every, rounds) in [(20usize, 5usize, 4usize), (12, 4, 3)] {
        let p = CarveParams { steps, reroute_every: every, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
        let mut want = z0.clone();
        let t = Instant::now();
        carve(&grid, &mut want, &outlet, &p, None, stream);
        let tc = t.elapsed().as_secs_f64();
        let mut line = format!("[ROUND {steps}/{every}] whole-region carve {tc:.1} s;");
        let mut first: Option<Vec<f64>> = None;
        for th in [1usize, 2, 5, 8] {
            let mut got = z0.clone();
            let t = Instant::now();
            solve_rounds(&grid, &mut got, &outlet, &p, rounds, &th, stream);
            line += &format!(" {th} thr {:.1} s: {} differ from the carve;", t.elapsed().as_secs_f64(), diff_bits(&got, &want));
            if let Some(f) = &first {
                line += &format!(" ({} vs 1 thr)", diff_bits(&got, f));
            } else {
                first = Some(got);
            }
        }
        println!("{line}");
    }
    let tiles = [128usize, 256];
    let _ = check_routing("route z0", &grid, &z0, &outlet, CarveParams::default().epsilon, stream, None, &tiles, n < 1_300_000);
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
    let mut z = z0.clone();
    solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, stream);
    let _ = check_routing("route carved z", &grid, &z, &outlet, CarveParams::default().epsilon, stream, None, &tiles, false);
    let mut dz: Vec<f64> = (0..n).filter(|&p| z0[p] > 0.0 && !outlet[p]).map(|p| z[p] - z0[p]).collect();
    println!("land change (m): p5 {:.1} p50 {:.1} p95 {:.1} max {:.1}", quantile(&mut dz.clone(), 0.05), quantile(&mut dz.clone(), 0.5), quantile(&mut dz.clone(), 0.95), quantile(&mut dz, 1.0));
    h1_check(&grid, &z, &z0, &outlet, &spec, &inputs, stream, wi0, wj0, threads);
}

/// A spiral corridor: the flood path winds through many tile borders (the worst case for the sweeps of A3 section 6.8).
fn spiral() {
    for (size, tile, wall) in [(257usize, 32usize, 500.0f64), (401, 48, 500.0), (401, 16, 500.0)] {
        let (w, h) = (size, size);
        let n = w * h;
        let mut z = vec![wall; n];
        // corridors one cell wide, walls one cell wide: concentric rings joined by a gap that alternates sides
        let mut ring = 0;
        let mut path: Vec<usize> = Vec::new();
        // build a rectangular spiral path of cells starting at the outer left
        let (mut x0, mut y0, mut x1, mut y1) = (0i64, 0i64, w as i64 - 1, h as i64 - 1);
        while x0 <= x1 && y0 <= y1 {
            for x in x0..=x1 {
                path.push(y0 as usize * w + x as usize);
            }
            for y in y0 + 1..=y1 {
                path.push(y as usize * w + x1 as usize);
            }
            if y1 > y0 {
                for x in (x0..x1).rev() {
                    path.push(y1 as usize * w + x as usize);
                }
            }
            if x1 > x0 {
                // the left side stops two short of the corner, and the connector joins the next ring: only the last cell of a ring touches the next one
                for y in (y0 + 2..y1).rev() {
                    path.push(y as usize * w + x0 as usize);
                }
                path.push((y0 + 2) as usize * w + (x0 + 1) as usize);
            }
            x0 += 2;
            y0 += 2;
            x1 -= 2;
            y1 -= 2;
            ring += 1;
        }
        let mut seen = vec![false; n];
        let mut uniq = Vec::new();
        for &p in &path {
            if !seen[p] {
                seen[p] = true;
                uniq.push(p);
            }
        }
        for &p in &uniq {
            z[p] = 0.0;
        }
        // the spiral joins its rings at the corners: the corridor is the path; connect consecutive cells already adjacent
        let grid = Grid::new(w, h, 1000.0);
        let mut outlet = vec![false; n];
        outlet[uniq[0]] = true;
        let eps = 1e-3;
        let fd = flood_dijkstra(&grid, &z, &outlet, eps);
        let rules = RoundRules::ROUND;
        let res = route_rules(&grid, &z, &outlet, eps, 7, 0.0, None, rules);
        let mut cache = PartCache::default();
        let t = Instant::now();
        let part = partitioned_route(&grid, &z, &outlet, eps, 7, tile, rules, &mut cache);
        let crossings = uniq.windows(2).filter(|p| (p[0] % w / tile, p[0] / w / tile) != (p[1] % w / tile, p[1] / w / tile)).count();
        println!(
            "spiral {size}x{size} ({ring} rings, corridor {} cells), tile {tile}: {} tiles, corridor crosses a tile border {crossings} times; partitioned {} sweeps, {} floods, {:.1} s; vs resident {} filled / {} receivers / {} A_d; filled vs Dijkstra {}; max F {:.1}",
            uniq.len(),
            part.stats.tiles,
            part.stats.sweeps,
            part.stats.floods,
            t.elapsed().as_secs_f64(),
            diff_bits(&part.filled, &planet_generators::bake::flood::priority_flood_with(&grid, &z, &outlet, eps, true)),
            diff_u32(&part.routing.receiver, &res.receiver),
            diff_bits(&part.routing.area, &res.area),
            diff_bits(&part.filled, &fd),
            fd.iter().copied().fold(f64::MIN, f64::max)
        );
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("survey") => survey(a[2].parse().unwrap()),
        Some("spiral") => spiral(),
        Some("gates") => {
            let g = |k: usize| a[k].parse::<usize>().unwrap();
            gates(&a[2], g(3) as u8, g(4), g(5), g(6), g(7), a[8].parse().unwrap(), g(9), g(10));
        }
        _ => eprintln!("usage: gate_probe survey <seed> | gates <name> <face> <i0> <i1> <j0> <j1> <seed> <margin> <threads> | spiral"),
    }
}
