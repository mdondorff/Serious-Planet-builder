//! SPIKE-105 F7 (spike branch only, never merged): is the per-round decomposition of the carve exact?
//! Q1: rounds of 5 steps, one whole-window routing per round, breach and 5 sweeps per D8 catchment of that routing (forward, reverse, 4 threads) against `carve` (20 steps, reroute 5).
//! Q2: routing locality of the 5 largest catchments not reaching the border. Q3: cost split of a round.
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::{route_weighted, Routing, NONE};
use planet_generators::bake::Grid;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const THREADS: usize = 4;

fn sample(inputs: &BakeInputs, spec: &FaceSpec, i0: i64, j0: i64, n: usize) -> Vec<f64> {
    let mut z = vec![0.0f64; n * n];
    let rows_per = n.div_ceil(THREADS);
    std::thread::scope(|s| {
        for (k, chunk) in z.chunks_mut(rows_per * n).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows_per * n + c;
                    let d: Vec3 = spec.halo_direction(i0 + (p % n) as i64, j0 + (p / n) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    z
}

fn distance(grid: &Grid, a: usize, b: usize) -> f64 {
    let (ai, aj, bi, bj) = (a % grid.w(), a / grid.w(), b % grid.w(), b / grid.w());
    if ai == bi || aj == bj {
        grid.dx()
    } else {
        grid.dx() * core::f64::consts::SQRT_2
    }
}

#[derive(Clone, Copy)]
struct Ptr(*mut f64);
unsafe impl Send for Ptr {}
unsafe impl Sync for Ptr {}

struct Cfg<'a> {
    grid: &'a Grid,
    outlet: &'a [bool],
    p: &'a CarveParams,
    taper: &'a [f32],
    uplift: f64,
    floor: f64,
}

/// Copy of the breach loop of `carve_core` (height scale off, no rock) over the cells `cells` (a catchment's slice of `order`, outlets first).
unsafe fn breach(cfg: &Cfg, r: &Routing, cells: &[u32], z: Ptr) {
    for &c in cells.iter().rev() {
        let c = c as usize;
        let rec = r.receiver[c];
        if rec == NONE || cfg.outlet[rec as usize] {
            continue;
        }
        let rec = rec as usize;
        let want = (*z.0.add(c) - cfg.p.breach_slope * distance(cfg.grid, c, rec)).max(cfg.floor);
        if *z.0.add(rec) > want {
            *z.0.add(rec) = want.max(*z.0.add(rec) - cfg.p.breach_max_m);
        }
    }
}

/// Copy of one non-deposition sweep of `carve_core` (no factor, no rock, keep_pits).
unsafe fn sweep(cfg: &Cfg, r: &Routing, cells: &[u32], z: Ptr) {
    for &c in cells {
        let c = c as usize;
        let rec = r.receiver[c];
        if rec == NONE {
            continue;
        }
        let rec = rec as usize;
        let f = cfg.p.k_dt * libm::sqrt(r.area[c]) / distance(cfg.grid, c, rec);
        let hr = *z.0.add(rec);
        let t = f64::from(cfg.taper[c]);
        let up = cfg.uplift * t;
        let zc = *z.0.add(c);
        let hn = (zc + up + f * hr) / (1.0 + f);
        let new = if cfg.p.keep_pits && zc < hr { (zc + up).max(cfg.floor) } else { hn.max(hr + cfg.p.epsilon).max(cfg.floor) };
        *z.0.add(c) = new;
    }
}

/// Catchments of a routing: the cells of each root in the order of `order`.
fn catchments(r: &Routing, n: usize) -> Vec<Vec<u32>> {
    let mut root = vec![0u32; n];
    let mut idx = vec![u32::MAX; n];
    let mut cats: Vec<Vec<u32>> = Vec::new();
    for &c in &r.order {
        let c = c as usize;
        if r.receiver[c] == NONE {
            root[c] = c as u32;
            idx[c] = cats.len() as u32;
            cats.push(Vec::new());
        } else {
            root[c] = root[r.receiver[c] as usize];
        }
        cats[idx[root[c] as usize] as usize].push(c as u32);
    }
    cats
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Forward,
    Reverse,
    Threads,
    Whole, // whole-window loops with the shared routing (control of the copy)
}

fn run_round(cfg: &Cfg, r: &Routing, cats: &[Vec<u32>], z: &mut [f64], mode: Mode, steps: usize) {
    let zp = Ptr(z.as_mut_ptr());
    let one = |cells: &[u32]| unsafe {
        breach(cfg, r, cells, zp);
        for _ in 0..steps {
            sweep(cfg, r, cells, zp);
        }
    };
    match mode {
        Mode::Forward => cats.iter().for_each(|c| one(c)),
        Mode::Reverse => cats.iter().rev().for_each(|c| one(c)),
        Mode::Whole => unsafe {
            breach(cfg, r, &r.order, zp);
            for _ in 0..steps {
                sweep(cfg, r, &r.order, zp);
            }
        },
        Mode::Threads => {
            // largest first, dynamic queue
            let mut ord: Vec<usize> = (0..cats.len()).collect();
            ord.sort_by_key(|&k| std::cmp::Reverse(cats[k].len()));
            let next = AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..THREADS {
                    s.spawn(|| loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        if k >= ord.len() {
                            break;
                        }
                        one(&cats[ord[k]]);
                    });
                }
            });
        }
    }
}

fn compare(name: &str, a: &[f64], b: &[f64], land: &[bool]) {
    let (mut mx, mut cnt, mut mxl, mut cntl, mut nl) = (0.0f64, 0usize, 0.0f64, 0usize, 0usize);
    for i in 0..a.len() {
        let d = (a[i] - b[i]).abs();
        if d > 0.0 {
            cnt += 1;
        }
        mx = mx.max(d);
        if land[i] {
            nl += 1;
            if d > 0.0 {
                cntl += 1;
            }
            mxl = mxl.max(d);
        }
    }
    println!("{name}: land ({nl} cells) max |dz| {mxl:e}, differing {cntl}; all cells max {mx:e}, differing {cnt}");
}

fn q2(name: &str, grid: &Grid, z: &[f64], outlet: &[bool], p: &CarveParams, seed: u64) {
    let n = grid.len();
    let r = route_weighted(grid, z, outlet, p.epsilon, seed, 0.0, None);
    let fl = priority_flood(grid, z, outlet, p.epsilon);
    let cats = catchments(&r, n);
    let mut big: Vec<&Vec<u32>> = cats.iter().filter(|c| !grid.is_edge(c[0] as usize)).collect();
    big.sort_by_key(|c| std::cmp::Reverse(c.len()));
    println!("Q2 {name}: {} catchments, {} not at the border; 5 largest (cells): {:?}", cats.len(), big.len(), big.iter().take(5).map(|c| c.len()).collect::<Vec<_>>());
    for (k, d) in big.iter().take(5).enumerate() {
        let mut in_d = vec![false; n];
        for &c in d.iter() {
            in_d[c as usize] = true;
        }
        let mut zd = z.to_vec();
        let mut od = vec![true; n];
        for c in 0..n {
            if in_d[c] {
                od[c] = outlet[c];
            } else {
                zd[c] += 1.0e5;
            }
        }
        let rd = route_weighted(grid, &zd, &od, p.epsilon, seed, 0.0, None);
        let fd = priority_flood(grid, &zd, &od, p.epsilon);
        let (mut rc, mut ar, mut fdiff, mut fmax, mut cross_in, mut cross_out) = (0usize, 0.0f64, 0usize, 0.0f64, 0usize, 0usize);
        let mut area_cnt = 0usize;
        for &c in d.iter() {
            let c = c as usize;
            if r.receiver[c] != rd.receiver[c] {
                rc += 1;
            }
            let rel = (r.area[c] - rd.area[c]).abs() / r.area[c];
            if rel > 0.0 {
                area_cnt += 1;
            }
            ar = ar.max(rel);
            let df = (fl[c] - fd[c]).abs();
            if df > 0.0 {
                fdiff += 1;
            }
            fmax = fmax.max(df);
        }
        // MFD leaks: cells of D with a lower neighbour (on the window's filled surface) outside D, and outside cells sending into D.
        for c in 0..n {
            if outlet[c] {
                continue;
            }
            for dir in 0..8 {
                if let Some(q) = grid.neighbour(c, dir) {
                    if fl[c] > fl[q] && in_d[c] && !in_d[q] {
                        cross_out += 1;
                    }
                    if fl[c] > fl[q] && !in_d[c] && in_d[q] {
                        cross_in += 1;
                    }
                }
            }
        }
        println!(
            "  D{k} ({} cells): receiver differs {rc}; area differs in {area_cnt} cells, max rel {ar:e}; filled differs in {fdiff} cells, max {fmax:e} m; MFD links window-filled D->outside {cross_out}, outside->D {cross_in}",
            d.len()
        );
    }
}

fn main() {
    let seed = 14u64;
    let inputs = BakeInputs::new(seed, THREADS);
    let sp = FaceSpec { face: Face(2), cells: 12_288, radius_m: 6_371_000.0, sea_level: 0.0, seed: 14, halo: 0 };
    let n = 1228usize;
    let (i0, j0) = (4186i64, 3418i64);
    let grid = Grid::new(n, n, sp.dx()).with_origin((i0 as u32, j0 as u32));
    let base = sample(&inputs, &sp, i0, j0, n);
    let outlet = grid.outlets(&base, 0.0);
    let land: Vec<bool> = base.iter().map(|v| *v > 0.0).collect();
    println!("window {n}x{n} dx {:.1} m, land {:.1} %", sp.dx(), 100.0 * land.iter().filter(|x| **x).count() as f64 / (n * n) as f64);
    let p = CarveParams::default();
    let t = Instant::now();
    let mut zr = base.clone();
    carve(&grid, &mut zr, &outlet, &p, None, seed);
    println!("R (carve, 20 steps, reroute 5): {:.1} s", t.elapsed().as_secs_f64());

    let taper: Vec<f32> = base.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect();
    let cfg = Cfg { grid: &grid, outlet: &outlet, p: &p, taper: &taper, uplift: p.u_over_k * p.k_dt, floor: p.sea_level + p.epsilon };
    let rounds = p.steps / p.reroute_every;
    let mut z1: Option<Vec<f64>> = None;
    for (name, mode) in [("whole loops, shared routing (copy control)", Mode::Whole), ("catchments forward", Mode::Forward), ("catchments reverse", Mode::Reverse), ("catchments on 4 threads", Mode::Threads)] {
        let mut z = base.clone();
        let (mut t_route, mut t_cat, mut t_run) = (0.0, 0.0, 0.0);
        for round in 0..rounds {
            let t = Instant::now();
            let r = route_weighted(&grid, &z, &outlet, p.epsilon, seed, 0.0, None);
            t_route += t.elapsed().as_secs_f64();
            let t = Instant::now();
            let cats = catchments(&r, n * n);
            t_cat += t.elapsed().as_secs_f64();
            if round == 0 && mode == Mode::Forward && z1.is_none() {
                // Q2 on the first routing
                q2("routing 1 (base)", &grid, &z, &outlet, &p, seed);
            }
            let t = Instant::now();
            run_round(&cfg, &r, &cats, &mut z, mode, p.reroute_every);
            t_run += t.elapsed().as_secs_f64();
            if round == 0 && mode == Mode::Forward {
                z1 = Some(z.clone());
            }
        }
        compare(name, &z, &zr, &land);
        println!("   Q3 over {rounds} rounds: route {:.2} s ({:.2} s a round), catchment split {:.2} s, breach+sweeps {:.2} s ({:.2} s a round)", t_route, t_route / rounds as f64, t_cat, t_run, t_run / rounds as f64);
    }
    if let Some(z1) = z1 {
        q2("routing 2 (after round 1)", &grid, &z1, &outlet, &p, seed);
    }
}
