//! SPIKE-105 F4 (spike branch only, never merged): outer iteration of the domain carve against the whole-window carve of F2's window.
//! `f4_outer_iteration [c1 c2 c4 c10 f coarse]` : variants to run (default all). Domains are D8 catchments of a routing of the current surface; every domain is carved alone, outside = walls.
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve_rock, CarveParams, Rock};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::{route, NONE};
use planet_generators::bake::Grid;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

const THREADS: usize = 4;
const MIN_CELLS: usize = 16;

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

struct Ctx {
    grid: Grid,
    n: usize,
    dx: f64,
    seed: u64,
    i0: i64,
    j0: i64,
    base: Vec<f64>,
    outlet: Vec<bool>,
    uplift: Vec<f32>,
    land: Vec<bool>,
}

fn params(steps: usize) -> CarveParams {
    CarveParams { steps, reroute_every: steps.min(5), uplift_taper_m: 0.0, ..CarveParams::default() }
}

/// The catchment label of every cell: the outlet it finally drains to.
fn catchments(c: &Ctx, z: &[f64]) -> Vec<u32> {
    let p = CarveParams::default();
    let r = route(&c.grid, z, &c.outlet, p.epsilon, c.seed, 0.0);
    let mut root = vec![0u32; c.n * c.n];
    for &x in &r.order {
        let x = x as usize;
        root[x] = if r.receiver[x] == NONE { x as u32 } else { root[r.receiver[x] as usize] };
    }
    root
}

/// Coarse labels: the surface averaged over 4 x 4 blocks, routed there, the root block id given to the 16 cells.
fn coarse_labels(c: &Ctx, z: &[f64]) -> Vec<u32> {
    let f = 4;
    let cn = c.n / f;
    let mut cz = vec![0.0f64; cn * cn];
    for cj in 0..cn {
        for ci in 0..cn {
            let mut s = 0.0;
            for dj in 0..f {
                for di in 0..f {
                    s += z[(cj * f + dj) * c.n + ci * f + di];
                }
            }
            cz[cj * cn + ci] = s / (f * f) as f64;
        }
    }
    let cg = Grid::new(cn, cn, c.dx * f as f64).with_origin(((c.i0 / f as i64) as u32, (c.j0 / f as i64) as u32));
    let co = cg.outlets(&cz, 0.0);
    let p = CarveParams::default();
    let r = route(&cg, &cz, &co, p.epsilon, c.seed, 0.0);
    let mut root = vec![0u32; cn * cn];
    for &x in &r.order {
        let x = x as usize;
        root[x] = if r.receiver[x] == NONE { x as u32 } else { root[r.receiver[x] as usize] };
    }
    let mut lab = vec![0u32; c.n * c.n];
    for j in 0..c.n {
        for i in 0..c.n {
            let (ci, cj) = ((i / f).min(cn - 1), (j / f).min(cn - 1));
            lab[j * c.n + i] = root[cj * cn + ci];
        }
    }
    lab
}

struct CutStats {
    domains: usize,
    skipped_domains: usize,
    skipped_land: usize,
    bbox_cells: usize,
}

/// Carves every domain of `labels` alone for `steps` steps, starting from `start`; returns the new surface.
fn carve_domains(c: &Ctx, start: &[f64], labels: &[u32], steps: usize) -> (Vec<f64>, CutStats) {
    let n = c.n;
    let mut idx: Vec<u32> = (0..(n * n) as u32).collect();
    idx.sort_unstable_by_key(|&x| (labels[x as usize], x));
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut a = 0;
    while a < idx.len() {
        let mut b = a;
        while b < idx.len() && labels[idx[b] as usize] == labels[idx[a] as usize] {
            b += 1;
        }
        groups.push((a, b));
        a = b;
    }
    let mut st = CutStats { domains: 0, skipped_domains: 0, skipped_land: 0, bbox_cells: 0 };
    let mut jobs: Vec<(usize, usize, usize)> = Vec::new(); // (a, b, cells)
    for &(a, b) in &groups {
        let m = &idx[a..b];
        let nonout = m.iter().filter(|&&x| !c.outlet[x as usize]).count();
        let has_out = m.iter().any(|&x| c.outlet[x as usize]);
        if nonout == 0 {
            continue; // pure sea / border: nothing to carve
        }
        if nonout < MIN_CELLS || !has_out {
            st.skipped_domains += 1;
            st.skipped_land += m.iter().filter(|&&x| c.land[x as usize]).count();
            continue;
        }
        st.domains += 1;
        jobs.push((a, b, nonout));
    }
    jobs.sort_by_key(|j| std::cmp::Reverse(j.2));
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(u32, f64)>> = Mutex::new(Vec::new());
    let bbox_total = AtomicUsize::new(0);
    let p = params(steps);
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                if k >= jobs.len() {
                    break;
                }
                let (a, b, _) = jobs[k];
                let m = &idx[a..b];
                let lab = labels[m[0] as usize];
                let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for &x in m {
                    let (i, j) = (x as usize % n, x as usize / n);
                    x0 = x0.min(i);
                    x1 = x1.max(i);
                    y0 = y0.min(j);
                    y1 = y1.max(j);
                }
                let x0 = x0.saturating_sub(1);
                let y0 = y0.saturating_sub(1);
                let x1 = (x1 + 1).min(n - 1);
                let y1 = (y1 + 1).min(n - 1);
                let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
                bbox_total.fetch_add(w * h, Ordering::Relaxed);
                let mut z = vec![0.0f64; w * h];
                let mut o = vec![true; w * h];
                let mut u = vec![0.0f32; w * h];
                for j in 0..h {
                    for i in 0..w {
                        let g = (y0 + j) * n + x0 + i;
                        let l = j * w + i;
                        z[l] = start[g];
                        u[l] = c.uplift[g];
                        if labels[g] == lab {
                            o[l] = c.outlet[g];
                        } else {
                            z[l] += 1.0e5;
                        }
                    }
                }
                let g = Grid::new(w, h, c.dx).with_origin(((c.i0 as usize + x0) as u32, (c.j0 as usize + y0) as u32));
                let rock = Rock { par: &1usize, k: None, uplift: Some(&u), runoff: None };
                carve_rock(&g, &mut z, &o, &p, None, &rock, c.seed);
                let mut res = Vec::with_capacity(m.len());
                for &x in m {
                    if c.outlet[x as usize] {
                        continue;
                    }
                    let (i, j) = (x as usize % n - x0, x as usize / n - y0);
                    res.push((x, z[j * w + i]));
                }
                out.lock().unwrap().extend(res);
            });
        }
    });
    st.bbox_cells = bbox_total.load(Ordering::Relaxed);
    let mut z = start.to_vec();
    for (x, v) in out.into_inner().unwrap() {
        z[x as usize] = v;
    }
    (z, st)
}

fn quant(v: &mut [f32], q: f64) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q) as usize]
}

/// rms, max, share above 1 m of the land cells' difference.
fn diff(c: &Ctx, a: &[f64], b: &[f64]) -> (f64, f64, f64) {
    let (mut s2, mut mx, mut cnt, mut k) = (0.0, 0.0f64, 0usize, 0usize);
    for x in 0..c.n * c.n {
        if c.land[x] {
            let d = (a[x] - b[x]).abs();
            s2 += d * d;
            mx = mx.max(d);
            k += 1;
            if d > 1.0 {
                cnt += 1;
            }
        }
    }
    ((s2 / k as f64).sqrt(), mx, 100.0 * cnt as f64 / k as f64)
}

fn migration(c: &Ctx, used: &[u32], z: &[f64]) -> f64 {
    let fin = catchments(c, z);
    let (mut d, mut k) = (0usize, 0usize);
    // Labels of different kinds (coarse ids) are compared as partitions: each used domain is matched to the catchment holding most of its land.
    let mut cnt = std::collections::HashMap::<(u32, u32), usize>::new();
    for x in 0..c.n * c.n {
        if c.land[x] {
            *cnt.entry((used[x], fin[x])).or_default() += 1;
        }
    }
    let mut best = std::collections::HashMap::<u32, (usize, u32)>::new();
    for (&(u, f), &n) in &cnt {
        let e = best.entry(u).or_insert((0, f));
        if n > e.0 || (n == e.0 && f < e.1) {
            *e = (n, f);
        }
    }
    for x in 0..c.n * c.n {
        if c.land[x] {
            k += 1;
            if best[&used[x]].1 != fin[x] {
                d += 1;
            }
        }
    }
    100.0 * d as f64 / k as f64
}

/// Border pairs (different labels) and inner pairs: |dz| p50/p99 for the result and for the reference at the same pairs.
fn seam(c: &Ctx, used: &[u32], z: &[f64], r: &[f64]) -> String {
    let n = c.n;
    let (mut bz, mut br, mut iz, mut ir) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for j in 0..n - 1 {
        for i in 1..n - 1 {
            let a = j * n + i;
            if !c.land[a] {
                continue;
            }
            for (di, dj) in [(1i64, 0i64), (0, 1), (1, 1), (-1, 1)] {
                let b = ((j as i64 + dj) * n as i64 + i as i64 + di) as usize;
                if !c.land[b] {
                    continue;
                }
                let (dz, dr) = ((z[a] - z[b]).abs() as f32, (r[a] - r[b]).abs() as f32);
                if used[a] != used[b] {
                    bz.push(dz);
                    br.push(dr);
                } else {
                    iz.push(dz);
                    ir.push(dr);
                }
            }
        }
    }
    let nb = bz.len();
    format!(
        "border pairs {nb} ({:.1} % of pairs) |dz| p50/p99 result {:.2}/{:.2} R {:.2}/{:.2}; inner result {:.2}/{:.2} R {:.2}/{:.2}",
        100.0 * nb as f64 / (nb + iz.len()) as f64,
        quant(&mut bz, 0.5),
        quant(&mut bz, 0.99),
        quant(&mut br, 0.5),
        quant(&mut br, 0.99),
        quant(&mut iz, 0.5),
        quant(&mut iz, 0.99),
        quant(&mut ir, 0.5),
        quant(&mut ir, 0.99)
    )
}

fn relief(c: &Ctx, z: &[f64]) -> Vec<f32> {
    let f = 4;
    let cn = c.n / f;
    let mut v = Vec::new();
    for cj in 0..cn {
        for ci in 0..cn {
            let (mut lo, mut hi, mut all) = (f64::MAX, f64::MIN, true);
            for dj in 0..f {
                for di in 0..f {
                    let x = (cj * f + dj) * c.n + ci * f + di;
                    all &= c.land[x];
                    lo = lo.min(z[x]);
                    hi = hi.max(z[x]);
                }
            }
            v.push(if all { (hi - lo) as f32 } else { f32::NAN });
        }
    }
    v
}

fn relief_report(c: &Ctx, z: &[f64], r: &[f64]) -> String {
    let (a, b) = (relief(c, z), relief(c, r));
    let (mut va, mut vb, mut vd) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..a.len() {
        if !a[k].is_nan() {
            va.push(a[k]);
            vb.push(b[k]);
            vd.push((a[k] - b[k]).abs());
        }
    }
    format!("relief(3.3 km blocks, {} all-land) p50 result {:.1} R {:.1}, p50 |block diff| {:.2}", va.len(), quant(&mut va, 0.5), quant(&mut vb, 0.5), quant(&mut vd, 0.5))
}

fn report(c: &Ctx, name: &str, z: &[f64], r: &[f64], used: &[u32], st: &CutStats, secs: f64) {
    let (rms, mx, sh) = diff(c, z, r);
    println!(
        "  {name}: vs R rms {rms:.3} m, max {mx:.2} m, >1 m {sh:.2} %; migration {:.2} % ; domains {} (skipped {} = {} land cells), bbox cells {} ({:.2}x window); {secs:.1} s",
        migration(c, used, z),
        st.domains,
        st.skipped_domains,
        st.skipped_land,
        st.bbox_cells,
        st.bbox_cells as f64 / (c.n * c.n) as f64
    );
    println!("    {}", seam(c, used, z, r));
    println!("    {}", relief_report(c, z, r));
}

fn make(seed: u64) -> Ctx {
    let inputs = BakeInputs::new(14, THREADS);
    let sp = FaceSpec { face: Face(2), cells: 12_288, radius_m: 6_371_000.0, sea_level: 0.0, seed: 14, halo: 0 };
    let n = 1228usize;
    let (i0, j0) = (4800 - 614i64, 4032 - 614i64);
    let grid = Grid::new(n, n, sp.dx()).with_origin((i0 as u32, j0 as u32));
    let base = sample(&inputs, &sp, i0, j0, n);
    let outlet = grid.outlets(&base, 0.0);
    let land: Vec<bool> = base.iter().map(|v| *v > 0.0).collect();
    let d = CarveParams::default();
    let uplift: Vec<f32> = base.iter().map(|h| (d.u_over_k * planet_core::smoothstep(0.0, d.uplift_taper_m, *h - d.sea_level)) as f32).collect();
    Ctx { grid, n, dx: sp.dx(), seed, i0, j0, base, outlet, uplift, land }
}

fn whole(c: &Ctx, start: &[f64], steps: usize, reroute: usize) -> Vec<f64> {
    let mut z = start.to_vec();
    let rock = Rock { par: &1usize, k: None, uplift: Some(&c.uplift), runoff: None };
    let p = CarveParams { reroute_every: reroute, ..params(steps) };
    carve_rock(&c.grid, &mut z, &c.outlet, &p, None, &rock, c.seed);
    z
}

fn run_c(c: &Ctx, rounds: usize) -> Vec<f64> {
    let mut z = c.base.clone();
    for _ in 0..rounds {
        let l = catchments(c, &z);
        z = carve_domains(c, &z, &l, 20 / rounds).0;
    }
    z
}

fn run_f(c: &Ctx, rounds: usize) -> Vec<f64> {
    let mut z = c.base.clone();
    for k in 0..rounds {
        let l = catchments(c, &z);
        z = if k == 0 { carve_domains(c, &c.base, &l, 20).0 } else { carve_domains(c, &c.base, &l, 20).0 };
    }
    z
}

fn hash01(x: u64) -> f64 {
    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = (x ^ (x >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((x ^ (x >> 32)) >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

fn row(c: &Ctx, name: &str, z: &[f64], r: &[f64]) {
    let (rms, mx, sh) = diff(c, z, r);
    println!("{name}: rms {rms:.3} m, max {mx:.2} m, >1 m {sh:.2} %; {}", relief_report(c, z, r));
}

fn sinks(c: &Ctx, name: &str, z: &[f64]) {
    let f = planet_generators::bake::flood::priority_flood(&c.grid, z, &c.outlet, 1e-3);
    let mut d: Vec<f32> = Vec::new();
    let mut land = 0usize;
    for x in 0..c.n * c.n {
        if c.land[x] {
            land += 1;
            if f[x] - z[x] > 0.5 {
                d.push((f[x] - z[x]) as f32);
            }
        }
    }
    let share = 100.0 * d.len() as f64 / land as f64;
    let (p50, p95) = (quant(&mut d, 0.5), quant(&mut d, 0.95));
    let lab = catchments(c, z);
    let mut roots = std::collections::HashSet::new();
    for x in 0..c.n * c.n {
        if !c.outlet[x] {
            roots.insert(lab[x]);
        }
    }
    println!("{name}: pit share {share:.2} % of land, depth p50 {p50:.1} p95 {p95:.1} m, D8 catchments {}", roots.len());
}

fn extra() {
    let c = make(14);
    let r = whole(&c, &c.base, 20, 5);
    for s in [101u64, 202] {
        let c2 = Ctx { seed: s, ..make(14) };
        row(&c, &format!("1 R seed {s} vs R seed 14"), &whole(&c2, &c2.base, 20, 5), &r);
    }
    for amp in [0.01f64, 1.0] {
        let pb: Vec<f64> = c.base.iter().enumerate().map(|(i, h)| if c.outlet[i] { *h } else { h + amp * hash01(i as u64 + 77) }).collect();
        row(&c, &format!("2 R base +/-{amp} m noise vs R"), &whole(&c, &pb, 20, 5), &r);
    }
    row(&c, "3 R reroute_every 2 vs R", &whole(&c, &c.base, 20, 2), &r);
    let c4 = run_c(&c, 4);
    let f4 = run_f(&c, 4);
    sinks(&c, "4 R", &r);
    sinks(&c, "4 C R=4", &c4);
    sinks(&c, "4 F round 4", &f4);
    row(&c, "5 C R=4 seed 14 vs R (reroute 5, same rhythm)", &c4, &r);
    for s in [101u64, 202] {
        let c2 = Ctx { seed: s, ..make(14) };
        let r2 = whole(&c2, &c2.base, 20, 5);
        row(&c, &format!("5 C R=4 seed {s} vs R seed {s}"), &run_c(&c2, 4), &r2);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.get(1).map(|s| s.as_str()) == Some("extra") {
        extra();
        return;
    }
    let want = |s: &str| a.len() < 2 || a[1..].iter().any(|x| x == s);
    let seed = 14u64;
    let inputs = BakeInputs::new(seed, THREADS);
    let sp = FaceSpec { face: Face(2), cells: 12_288, radius_m: 6_371_000.0, sea_level: 0.0, seed: 14, halo: 0 };
    let n = 1228usize;
    let (i0, j0) = (4800 - 614i64, 4032 - 614i64);
    let grid = Grid::new(n, n, sp.dx()).with_origin((i0 as u32, j0 as u32));
    let base = sample(&inputs, &sp, i0, j0, n);
    let outlet = grid.outlets(&base, 0.0);
    let land: Vec<bool> = base.iter().map(|v| *v > 0.0).collect();
    let d = CarveParams::default();
    let uplift: Vec<f32> = base.iter().map(|h| (d.u_over_k * planet_core::smoothstep(0.0, d.uplift_taper_m, *h - d.sea_level)) as f32).collect();
    let c = Ctx { grid, n, dx: sp.dx(), seed, i0, j0, base, outlet, uplift, land };
    println!("window {n}x{n}, land {:.1} %", 100.0 * c.land.iter().filter(|x| **x).count() as f64 / (n * n) as f64);

    // Reference R: whole window, plain carve as in F2, and with the uplift array (taper fixed at the base) that the domain carves use.
    let t = Instant::now();
    let mut zp = c.base.clone();
    planet_generators::bake::carve::carve(&c.grid, &mut zp, &c.outlet, &d, None, seed);
    println!("R plain whole-window carve {:.1} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let mut r = c.base.clone();
    let rock = Rock { par: &1usize, k: None, uplift: Some(&c.uplift), runoff: None };
    carve_rock(&c.grid, &mut r, &c.outlet, &params(20), None, &rock, seed);
    let (rms, mx, sh) = diff(&c, &r, &zp);
    println!("R with uplift array {:.1} s; vs R plain rms {rms:.4} m max {mx:.3} m >1 m {sh:.3} % (rounding of the f32 taper)", t.elapsed().as_secs_f64());
    let base_labels = catchments(&c, &c.base);
    let t_all = Instant::now();

    let mut finals: Vec<(String, Vec<f64>)> = Vec::new();
    for (key, rounds) in [("c1", 1usize), ("c2", 2), ("c4", 4), ("c10", 10)] {
        if !want(key) {
            continue;
        }
        println!("C R={rounds} ({} steps a round)", 20 / rounds);
        let mut z = c.base.clone();
        let tv = Instant::now();
        let mut last = (z.clone(), CutStats { domains: 0, skipped_domains: 0, skipped_land: 0, bbox_cells: 0 }, Vec::new(), 0.0);
        for k in 1..=rounds {
            let t = Instant::now();
            let labels = if k == 1 { base_labels.clone() } else { catchments(&c, &z) };
            let (nz, st) = carve_domains(&c, &z, &labels, 20 / rounds);
            z = nz;
            let secs = t.elapsed().as_secs_f64();
            if k < rounds {
                println!("  round {k}: migration {:.2} % ; domains {} (skipped {} = {} land); bbox {:.2}x; {secs:.1} s", migration(&c, &labels, &z), st.domains, st.skipped_domains, st.skipped_land, st.bbox_cells as f64 / (n * n) as f64);
            }
            last = (z.clone(), st, labels, secs);
        }
        report(&c, &format!("R={rounds} final (round {rounds} {:.1} s, total {:.1} s)", last.3, tv.elapsed().as_secs_f64()), &last.0, &r, &last.2, &last.1, last.3);
        finals.push((key.to_string(), last.0));
    }
    for w in finals.windows(2) {
        let (rms, mx, sh) = diff(&c, &w[1].1, &w[0].1);
        println!("successive C {} -> {}: rms {rms:.3} m max {mx:.2} m >1 m {sh:.2} %", w[0].0, w[1].0);
    }
    if want("f") {
        println!("F fresh restart from base, domains from previous final");
        let mut prev: Option<Vec<f64>> = None;
        for k in 1..=4 {
            let t = Instant::now();
            let labels = match &prev {
                None => base_labels.clone(),
                Some(p) => catchments(&c, p),
            };
            let (z, st) = carve_domains(&c, &c.base, &labels, 20);
            let secs = t.elapsed().as_secs_f64();
            report(&c, &format!("F round {k}"), &z, &r, &labels, &st, secs);
            if let Some(p) = &prev {
                let (rms, mx, sh) = diff(&c, &z, p);
                println!("    vs round {}: rms {rms:.3} m max {mx:.2} m >1 m {sh:.2} %", k - 1);
            }
            prev = Some(z);
        }
    }
    if want("coarse") {
        println!("C R=4 with coarse (4x) labels");
        let mut z = c.base.clone();
        let tv = Instant::now();
        let mut last = None;
        for k in 1..=4 {
            let t = Instant::now();
            let labels = coarse_labels(&c, &z);
            let (nz, st) = carve_domains(&c, &z, &labels, 5);
            z = nz;
            let secs = t.elapsed().as_secs_f64();
            println!("  round {k}: migration vs fine catchments {:.2} % ; domains {} (skipped {} = {} land); bbox {:.2}x; {secs:.1} s", migration(&c, &labels, &z), st.domains, st.skipped_domains, st.skipped_land, st.bbox_cells as f64 / (n * n) as f64);
            last = Some((st, labels, secs));
        }
        let (st, labels, secs) = last.unwrap();
        report(&c, &format!("coarse R=4 final (total {:.1} s)", tv.elapsed().as_secs_f64()), &z, &r, &labels, &st, secs);
    }
    println!("total variants {:.1} s", t_all.elapsed().as_secs_f64());
}
