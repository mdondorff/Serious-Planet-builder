//! SPIKE-47 (spike branch only, never merged): tributary variants of the A-net chunk solve. The variants route over a priority-flood filled surface
//! (SPIKE-43's rule), may route again during the steps, and may cap the incision by a depth law of the drainage area.

use planet_core::Vec3;
use planet_generators::anet::{diffuse, slump_deltas, ANet, ANetParams, Network, NO_NEXT};
use planet_generators::chunkfield::{Cell, ChunkLattice, Region};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const NONE: u32 = u32::MAX;

/// One variant of the solve; `Variant::parse` reads `name:key=value,...` over the production values.
#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub steps: u32,
    /// Route again every this many steps (0: route once on the initial surface).
    pub reroute_every: u32,
    /// A cell filled by more than this many metres is a lake (an outlet, not incised); infinite: every pit is filled.
    pub max_fill_m: f64,
    /// The cap of `f` off the corridor, the multiplier of `K` there, and whether a cell is only ever lowered.
    pub f_max: f64,
    pub k_mult: f64,
    pub detach_only: bool,
    pub diffusion: f64,
    pub slump_rate: f64,
    pub channel_area_m2: f64,
    /// The incision cap: a cell lies at most `cap_m (A / cap_ref_m2)^cap_exp` under the base height (0: no cap), at most `cap_max_m`.
    pub cap_m: f64,
    pub cap_ref_m2: f64,
    pub cap_exp: f64,
    pub cap_max_m: f64,
    /// Production's own receivers (pits are outlets, the surface of the start): the `fill` of the variants is off.
    pub fill: bool,
}

impl Variant {
    pub fn parse(spec: &str, p: &ANetParams) -> Variant {
        let (name, rest) = spec.split_once(':').unwrap_or((spec, ""));
        let mut v = Variant {
            name: name.to_string(),
            steps: p.steps,
            reroute_every: 0,
            max_fill_m: f64::INFINITY,
            f_max: p.tributary_f_max,
            k_mult: 1.0,
            detach_only: true,
            diffusion: p.diffusion,
            slump_rate: p.slump_rate,
            channel_area_m2: p.channel_area_m2,
            cap_m: 0.0,
            cap_ref_m2: 1.0e5,
            cap_exp: 0.5,
            cap_max_m: 60.0,
            fill: true,
        };
        for kv in rest.split(',').filter(|s| !s.is_empty()) {
            let (k, val) = kv.split_once('=').expect("key=value");
            let f: f64 = val.parse().unwrap_or_else(|_| panic!("number for {k}"));
            match k {
                "steps" => v.steps = f as u32,
                "reroute" => v.reroute_every = f as u32,
                "maxfill" => v.max_fill_m = f,
                "fmax" => v.f_max = f,
                "k" => v.k_mult = f,
                "detach" => v.detach_only = f != 0.0,
                "diff" => v.diffusion = f,
                "slump" => v.slump_rate = f,
                "thr" => v.channel_area_m2 = f,
                "cap" => v.cap_m = f,
                "capref" => v.cap_ref_m2 = f,
                "capexp" => v.cap_exp = f,
                "capmax" => v.cap_max_m = f,
                "fill" => v.fill = f != 0.0,
                other => panic!("unknown key {other}"),
            }
        }
        v
    }
}

fn ord(z: f64) -> u64 {
    let b = z.to_bits();
    if b >> 63 == 1 {
        !b
    } else {
        b | (1 << 63)
    }
}

/// Receivers over a priority-flood filled surface: the ring is the outlet, the filled height rises by `EPS` per cell across a flat, a cell drains to the
/// steepest (jittered) neighbour strictly lower on the filled surface; corridor cells keep their link. A cell filled by more than `max_fill_m` is an outlet.
/// Returns the receivers and the fill depth of every cell.
#[allow(clippy::too_many_arguments)]
fn route(
    z: &[f64],
    reg: &Region,
    dirs: &[Vec3],
    radius: f64,
    corridor: Option<&Network>,
    jitter: f64,
    seed: u64,
    max_fill_m: f64,
) -> (Vec<u32>, Vec<f64>) {
    const EPS: f64 = 1.0e-6;
    let len = reg.cells.len();
    let key = |c: usize| reg.cells[c].key();
    let ring = |c: usize| reg.depth[c] >= reg.halo;
    let mut zf = z.to_vec();
    let mut closed = vec![false; len];
    let mut heap: BinaryHeap<Reverse<(u64, u64, u32)>> = BinaryHeap::new();
    for c in 0..len {
        if ring(c) {
            closed[c] = true;
            heap.push(Reverse((ord(z[c]), key(c), c as u32)));
        }
    }
    while let Some(Reverse((_, _, c))) = heap.pop() {
        let c = c as usize;
        for k in 0..8 {
            let q = reg.neighbours[c][k];
            if q == NONE || closed[q as usize] {
                continue;
            }
            let q = q as usize;
            closed[q] = true;
            zf[q] = z[q].max(zf[c] + EPS);
            heap.push(Reverse((ord(zf[q]), key(q), q as u32)));
        }
    }
    let dist = |c: usize, r: usize| (dirs[c] - dirs[r]).length() * radius;
    let mut rec = vec![NONE; len];
    let mut fill = vec![0.0f64; len];
    for c in 0..len {
        if ring(c) {
            continue;
        }
        fill[c] = zf[c] - z[c];
        if let Some(net) = corridor {
            if net.corridor[c] && net.next[c] != NO_NEXT {
                rec[c] = net.next[c];
                continue;
            }
        }
        if fill[c] > max_fill_m {
            continue;
        }
        let mut best = (0.0f64, NONE);
        for k in 0..8 {
            let q = reg.neighbours[c][k];
            if q == NONE {
                continue;
            }
            let drop = zf[c] - zf[q as usize];
            if drop > 0.0 {
                let s = drop / dist(c, q as usize) * planet_generators::coupled::jitter_of(seed, key(c), k, jitter);
                if s > best.0 {
                    best = (s, q);
                }
            }
        }
        rec[c] = best.1;
    }
    (rec, fill)
}

/// The order (receivers first) and the drainage area of a receiver table; a cycle's cells become outlets.
fn order_and_area(rec: &mut [u32], base_area: &[f64], reg: &Region) -> (Vec<usize>, Vec<f64>) {
    let len = rec.len();
    let key = |c: usize| reg.cells[c].key();
    loop {
        let mut indeg = vec![0u32; len];
        for c in 0..len {
            if rec[c] != NONE {
                indeg[rec[c] as usize] += 1;
            }
        }
        let mut order: Vec<usize> = (0..len).filter(|&c| indeg[c] == 0).collect();
        order.sort_unstable_by_key(|&c| key(c));
        let mut at = 0;
        while at < order.len() {
            let c = order[at];
            at += 1;
            let r = rec[c];
            if r != NONE {
                indeg[r as usize] -= 1;
                if indeg[r as usize] == 0 {
                    order.push(r as usize);
                }
            }
        }
        if order.len() == len {
            let mut area = base_area.to_vec();
            for &c in &order {
                if rec[c] != NONE {
                    let a = area[c];
                    area[rec[c] as usize] += a;
                }
            }
            order.reverse();
            return (order, area);
        }
        let mut seen = vec![false; len];
        for &c in &order {
            seen[c] = true;
        }
        for c in 0..len {
            if !seen[c] {
                rec[c] = NONE;
            }
        }
    }
}

pub struct Out {
    pub z: Vec<f64>,
    pub area_m2: Vec<f64>,
}

/// The variant's solve. The ring and the corridor's anchors stay at their heights, as in the production solve.
pub fn variant_solve(
    net: &Network,
    lat: &ChunkLattice,
    reg: &Region,
    radius: f64,
    cell_area: &dyn Fn(Cell) -> f64,
    p: &ANetParams,
    v: &Variant,
) -> Out {
    let len = reg.cells.len();
    let key = |c: usize| reg.cells[c].key();
    let dirs: &[Vec3] = &net.dirs;
    let z_base = &net.base_z;
    let mut z = z_base.clone();
    for (h, &a) in z.iter_mut().zip(&net.anchor_z) {
        if !a.is_nan() {
            *h = h.min(a);
        }
    }
    let ring = |c: usize| reg.depth[c] >= reg.halo;
    let fixed = |c: usize| ring(c) || !net.anchor_z[c].is_nan();
    let dist = |c: usize, r: usize| (dirs[c] - dirs[r]).length() * radius;
    let base_area: Vec<f64> = (0..len).map(|c| cell_area(reg.cells[c]) + net.inflow_m2[c]).collect();
    let cell_m = lat.cell_m(radius);

    let flow = |z: &[f64]| -> (Vec<u32>, Vec<usize>, Vec<f64>) {
        let (mut rec, _fill) = route(z, reg, dirs, radius, Some(net), p.jitter, p.seed, v.max_fill_m);
        let (order, mut area) = order_and_area(&mut rec, &base_area, reg);
        for c in 0..len {
            if !net.river_area_m2[c].is_nan() {
                area[c] = net.river_area_m2[c].max(base_area[c]);
            }
        }
        (rec, order, area)
    };

    let hill = v.diffusion > 0.0 || v.slump_rate > 0.0;
    let reach0 = (v.steps as u16).min(reg.halo.saturating_sub(2));
    let inner = if reg.halo < 2 { 0 } else { reg.depth.partition_point(|&d| d <= reach0) };
    let nd: Vec<[f64; 8]> =
        if hill { (0..inner).map(|c| core::array::from_fn(|k| dist(c, reg.neighbours[c][k] as usize))).collect() } else { Vec::new() };
    let cf = (v.diffusion * p.dt_years / (cell_m * cell_m)).min(0.12);

    let (mut rec, mut order, mut area) = (Vec::new(), Vec::new(), Vec::new());
    let mut sweep: Vec<(usize, usize, f64)> = Vec::new();
    for t in 0..v.steps {
        if t == 0 || (v.reroute_every > 0 && t % v.reroute_every == 0) {
            if v.fill {
                (rec, order, area) = flow(&z);
            } else {
                // Production's own receivers: the steepest jittered neighbour down on the surface itself, a pit an outlet.
                let mut r = vec![NONE; len];
                for c in 0..len {
                    if ring(c) {
                        continue;
                    }
                    if net.corridor[c] && net.next[c] != NO_NEXT {
                        r[c] = net.next[c];
                        continue;
                    }
                    let mut best = (0.0f64, NONE);
                    for k in 0..8 {
                        let q = reg.neighbours[c][k];
                        let drop = z[c] - z[q as usize];
                        if drop > 0.0 {
                            let s = drop / dist(c, q as usize) * planet_generators::coupled::jitter_of(p.seed, key(c), k, p.jitter);
                            if s > best.0 {
                                best = (s, q);
                            }
                        }
                    }
                    r[c] = best.1;
                }
                let (o, mut a) = order_and_area(&mut r, &base_area, reg);
                for c in 0..len {
                    if !net.river_area_m2[c].is_nan() {
                        a[c] = net.river_area_m2[c].max(base_area[c]);
                    }
                }
                (rec, order, area) = (r, o, a);
            }
            sweep.clear();
            for &c in &order {
                if rec[c] != NONE && !fixed(c) {
                    let r = rec[c] as usize;
                    let mut f = p.k * v.k_mult * (area[c]).powf(p.m) * p.dt_years / dist(c, r);
                    if net.river_area_m2[c].is_nan() {
                        f = f.min(v.f_max);
                    }
                    sweep.push((c, r, f));
                }
            }
        }
        for &(c, r, f) in &sweep {
            let new = (z[c] + f * z[r]) / (1.0 + f);
            z[c] = if v.detach_only { z[c].min(new) } else { new };
        }
        if hill {
            let protected = |c: usize| fixed(c) || net.corridor[c] || area[c] > v.channel_area_m2;
            let cells = reg.depth.partition_point(|&d| d <= (v.steps - t) as u16).min(inner);
            if v.diffusion > 0.0 {
                z = diffuse(&z, &reg.neighbours, cells, cf, &protected);
            }
            if v.slump_rate > 0.0 {
                let delta = slump_deltas(&z, &reg.neighbours, &nd, cells, p.talus, v.slump_rate, &protected, &key);
                for c in 0..cells {
                    z[c] += delta[c];
                }
            }
        }
        if v.cap_m > 0.0 {
            for c in 0..len {
                if !fixed(c) {
                    let d = (v.cap_m * (area[c] / v.cap_ref_m2).powf(v.cap_exp)).min(v.cap_max_m);
                    z[c] = z[c].max(z_base[c] - d);
                }
            }
        }
    }
    Out { z, area_m2: area }
}

fn percentile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q) as usize]
}

/// Heights of the core as a raster (the core's cells are in `j`-major order), box-averaged by `b` cells.
fn block_mean(z: &[f64], n: usize, b: usize) -> (Vec<f64>, usize) {
    let m = n / b;
    let mut out = vec![0.0; m * m];
    for j in 0..m {
        for i in 0..m {
            let mut s = 0.0;
            for dj in 0..b {
                for di in 0..b {
                    s += z[(j * b + dj) * n + i * b + di];
                }
            }
            out[j * m + i] = s / (b * b) as f64;
        }
    }
    (out, m)
}

fn slope_stats(z: &[f64], n: usize, b: usize, cell_m: f64) -> (f64, f64) {
    let (a, m) = block_mean(z, n, b);
    let h = b as f64 * cell_m;
    let mut s = Vec::with_capacity(m * m);
    for j in 1..m - 1 {
        for i in 1..m - 1 {
            let gx = (a[j * m + i + 1] - a[j * m + i - 1]) / (2.0 * h);
            let gy = (a[(j + 1) * m + i] - a[(j - 1) * m + i]) / (2.0 * h);
            s.push((gx * gx + gy * gy).sqrt().atan().to_degrees());
        }
    }
    (percentile(&mut s.clone(), 0.5), percentile(&mut s, 0.9))
}

/// The census of a height field over the core: the drainage density of cells off the corridor over three thresholds (flow over the filled surface, no inflow),
/// the share of the core filled by over 0.5 and 5 m, the slope at 32 and 88 m, the relief in windows of 1 km, and the change against the base.
pub fn census(
    label: &str,
    z: &[f64],
    net: &Network,
    lat: &ChunkLattice,
    reg: &Region,
    radius: f64,
    cell_area: &dyn Fn(Cell) -> f64,
) -> String {
    let core = reg.core;
    let n = (core as f64).sqrt() as usize;
    let cell_m = lat.cell_m(radius);
    let dirs: &[Vec3] = &net.dirs;
    let (mut rec, fill) = route(z, reg, dirs, radius, None, 0.4, 0, f64::INFINITY);
    let base_area: Vec<f64> = (0..reg.cells.len()).map(|c| cell_area(reg.cells[c])).collect();
    let (_order, area) = order_and_area(&mut rec, &base_area, reg);
    let core_km2 = core as f64 * cell_m * cell_m / 1.0e6;
    let mut out = format!("{label}: ");
    for t in [1.0e5, 3.0e5, 1.0e6] {
        let mut len_m = 0.0;
        for c in 0..core {
            if !net.corridor[c] && area[c] >= t && rec[c] != NONE {
                len_m += (dirs[c] - dirs[rec[c] as usize]).length() * radius;
            }
        }
        out += &format!("dens {t:.0e}: {:.2}, ", len_m / 1000.0 / core_km2);
    }
    let share = |m: f64| (0..core).filter(|&c| fill[c] > m).count() as f64 / core as f64 * 100.0;
    out += &format!("filled>0.5m {:.1}%, >5m {:.1}%; ", share(0.5), share(5.0));
    let zc = &z[..core];
    for b in [4usize, 11] {
        let (m50, m90) = slope_stats(zc, n, b, cell_m);
        out += &format!("slope {:.0} m p50 {m50:.1} p90 {m90:.1} deg, ", b as f64 * cell_m);
    }
    let w = (1000.0 / cell_m) as usize;
    let mut relief: Vec<f64> = Vec::new();
    for bj in 0..n / w {
        for bi in 0..n / w {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for j in 0..w {
                for i in 0..w {
                    let h = zc[(bj * w + j) * n + bi * w + i];
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
            relief.push(hi - lo);
        }
    }
    let (lo, hi) = zc.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, &h| (a.0.min(h), a.1.max(h)));
    out += &format!("relief 1 km p50 {:.0} m, core {:.0} m", percentile(&mut relief, 0.5), hi - lo);
    out += "\n";
    out
}

/// The change of the core's heights against the base, off the corridor: p50, p90 of the lowering, and the shares lowered by over 5 and 50 m.
pub fn change_line(label: &str, z: &[f64], net: &Network, reg: &Region) -> String {
    let core = reg.core;
    let mut d: Vec<f64> = (0..core).filter(|&c| !net.corridor[c]).map(|c| net.base_z[c] - z[c]).collect();
    let n = d.len() as f64;
    let share = |m: f64| d.iter().filter(|&&x| x > m).count() as f64 / n * 100.0;
    let (s5, s50) = (share(5.0), share(50.0));
    format!(
        "{label}: lowering off the corridor p50 {:.2} m, p90 {:.1} m, max {:.0} m; over 5 m {s5:.1}%, over 50 m {s50:.1}%\n",
        percentile(&mut d, 0.5),
        percentile(&mut d.clone(), 0.9),
        d.last().copied().unwrap_or(0.0)
    )
}

fn hash_z(z: &[f64], core: usize) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in &z[..core] {
        for b in x.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// What the example needs of a run: the report, and the heights of the core per variant (the base first) for the stills.
pub struct SiteRun {
    pub report: String,
    pub cores: Vec<(String, Vec<f64>)>,
    pub n: usize,
}

/// One site: the production solve, then each variant; the census of each; the time alone and with `threads` of the same chunk at once; the hash of
/// the core of every thread's result.
pub fn run_site(stage: &ANet, lat: &ChunkLattice, reg: &Region, variants: &[Variant], threads: usize) -> SiteRun {
    let radius = stage.radius_m;
    let started = std::time::Instant::now();
    let net = Network::build_with(&*stage.run, &*stage.base, lat, reg, radius, &stage.params.anchor);
    let net_s = started.elapsed().as_secs_f64();
    let n_coarse = stage.run.lattice().n();
    let per = lat.n() as usize / n_coarse.max(1);
    let cells = (per * per) as f64;
    let area = |c: Cell| {
        let coarse = usize::from(c.face) * n_coarse * n_coarse + (c.j as usize / per) * n_coarse + (c.i as usize / per);
        stage.run.lattice().area_m2(coarse) / cells
    };
    let core = reg.core;
    let n = (core as f64).sqrt() as usize;
    let mut report = format!("network build {net_s:.2} s, region {} cells, core {n} x {n}\n", reg.cells.len());
    let mut cores: Vec<(String, Vec<f64>)> = vec![("base".into(), net.base_z[..core].to_vec())];
    report += &census("base", &net.base_z, &net, lat, reg, radius, &area);
    report += &crease_report("base", &net.base_z, reg);

    // Production.
    let t0 = std::time::Instant::now();
    let prod = planet_generators::anet::solve(&net, &*stage.base, lat, reg, radius, &area, &stage.params);
    let prod_s = t0.elapsed().as_secs_f64();
    report += &format!("prod solve (without network) {prod_s:.2} s\n");
    report += &census("prod", &prod.z, &net, lat, reg, radius, &area);
    report += &crease_report("prod", &prod.z, reg);
    report += &change_line("prod", &prod.z, &net, reg);
    cores.push(("prod".into(), prod.z[..core].to_vec()));

    for v in variants {
        let t0 = std::time::Instant::now();
        let out = variant_solve(&net, lat, reg, radius, &area, &stage.params, v);
        let s = t0.elapsed().as_secs_f64();
        report += &format!("{}: solve alone {s:.2} s (+ network {net_s:.2} s)\n", v.name);
        report += &census(&v.name, &out.z, &net, lat, reg, radius, &area);
        report += &change_line(&v.name, &out.z, &net, reg);
        // Channel cells the variant's own routing counts (area over the variant's threshold, off the corridor).
        let own = (0..core).filter(|&c| !net.corridor[c] && out.area_m2[c] >= 1.0e5).count();
        report += &format!("  own routing: {own} core cells off the corridor over 1e5 m2\n");
        cores.push((v.name.clone(), out.z[..core].to_vec()));
        // The same chunk on `threads` threads at once: times and hashes.
        if threads > 1 {
            let t1 = std::time::Instant::now();
            let mut res: Vec<(f64, u64)> = Vec::new();
            std::thread::scope(|sc| {
                let hs: Vec<_> = (0..threads)
                    .map(|_| {
                        sc.spawn(|| {
                            let t = std::time::Instant::now();
                            let o = variant_solve(&net, lat, reg, radius, &area, &stage.params, v);
                            (t.elapsed().as_secs_f64(), hash_z(&o.z, core))
                        })
                    })
                    .collect();
                for h in hs {
                    res.push(h.join().unwrap());
                }
            });
            let wall = t1.elapsed().as_secs_f64();
            let same = res.iter().all(|r| r.1 == res[0].1) && res[0].1 == hash_z(&out.z, core);
            let mean = res.iter().map(|r| r.0).sum::<f64>() / res.len() as f64;
            report += &format!(
                "  {threads} concurrent copies: mean {mean:.2} s each, wall {wall:.2} s, core hash {:016x}, identical: {same}\n",
                res[0].1
            );
        }
    }
    if threads > 1 {
        let t1 = std::time::Instant::now();
        let mut res: Vec<(f64, u64)> = Vec::new();
        std::thread::scope(|sc| {
            let hs: Vec<_> = (0..threads)
                .map(|_| {
                    sc.spawn(|| {
                        let t = std::time::Instant::now();
                        let o = planet_generators::anet::solve(&net, &*stage.base, lat, reg, radius, &area, &stage.params);
                        (t.elapsed().as_secs_f64(), hash_z(&o.z, core))
                    })
                })
                .collect();
            for h in hs {
                res.push(h.join().unwrap());
            }
        });
        let mean = res.iter().map(|r| r.0).sum::<f64>() / res.len() as f64;
        report += &format!(
            "prod: {threads} concurrent copies: mean {mean:.2} s each, wall {:.2} s, identical: {}\n",
            t1.elapsed().as_secs_f64(),
            res.iter().all(|r| r.1 == res[0].1)
        );
    }
    SiteRun { report, cores, n }
}

/// SPIKE-47 follow-up (seams): over the whole region as a raster of the cube-face lattice, the mean absolute and the mean signed height step across
/// every column and every row boundary, and the boundaries whose absolute step is over 2.5 times the median (with their absolute lattice index).
pub fn seam_report(label: &str, z: &[f64], reg: &Region) -> String {
    let (mut i0, mut j0, mut i1, mut j1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for c in &reg.cells {
        i0 = i0.min(c.i);
        j0 = j0.min(c.j);
        i1 = i1.max(c.i);
        j1 = j1.max(c.j);
    }
    let (w, h) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
    let mut grid = vec![f64::NAN; w * h];
    for (c, cell) in reg.cells.iter().enumerate() {
        grid[(cell.j - j0) as usize * w + (cell.i - i0) as usize] = z[c];
    }
    let mut out = format!(
        "{label}: region lattice i {i0}..{i1}, j {j0}..{j1} ({w} x {h}); core starts at i {} j {}\n",
        reg.cells[0].i, reg.cells[0].j
    );
    for (axis, len, other) in [("column", w - 1, h), ("row", h - 1, w)] {
        let at = |a: usize, b: usize| if axis == "column" { grid[b * w + a] } else { grid[a * w + b] };
        let (mut abs, mut signed) = (vec![0.0f64; len], vec![0.0f64; len]);
        for a in 0..len {
            let (mut sa, mut ss, mut n) = (0.0, 0.0, 0usize);
            for b in 0..other {
                let (p, q) = (at(a, b), at(a + 1, b));
                if p.is_finite() && q.is_finite() {
                    sa += (q - p).abs();
                    ss += q - p;
                    n += 1;
                }
            }
            if n > 0 {
                abs[a] = sa / n as f64;
                signed[a] = ss / n as f64;
            }
        }
        let mut sorted = abs.clone();
        let median = percentile(&mut sorted, 0.5);
        let base = if axis == "column" { i0 } else { j0 };
        let mut hits: Vec<(usize, f64)> = (0..len).filter(|&a| abs[a] > 2.5 * median).map(|a| (a, abs[a] / median)).collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1));
        out += &format!("  {axis} steps: median |dz| {median:.3} m per cell; {} boundaries over 2.5x:\n", hits.len());
        for (a, r) in hits.iter().take(24) {
            let step = signed[*a] - 0.5 * (signed[a.saturating_sub(1)] + signed[(*a + 1).min(len - 1)]);
            out += &format!(
                "    {axis} {} (between {} and {}): {r:.1}x, step {step:+.2} m, abs {}\n",
                a,
                a,
                a + 1,
                u64::from(base) + *a as u64
            );
        }
    }
    out
}

/// SPIKE-47 follow-up: the crease detector: the signed mean over a line of the second difference across it (the change of the mean gradient), per column
/// and per row boundary, against the median absolute of those means; the strongest boundaries with their absolute lattice index and the gradient jump in
/// metres per cell.
pub fn crease_report(label: &str, z: &[f64], reg: &Region) -> String {
    let (mut i0, mut j0, mut i1, mut j1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for c in &reg.cells {
        i0 = i0.min(c.i);
        j0 = j0.min(c.j);
        i1 = i1.max(c.i);
        j1 = j1.max(c.j);
    }
    let (w, h) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
    let mut grid = vec![f64::NAN; w * h];
    for (c, cell) in reg.cells.iter().enumerate() {
        grid[(cell.j - j0) as usize * w + (cell.i - i0) as usize] = z[c];
    }
    let mut out = String::new();
    for (axis, len, other, base) in [("column", w, h, i0), ("row", h, w, j0)] {
        let at = |a: usize, b: usize| if axis == "column" { grid[b * w + a] } else { grid[a * w + b] };
        let mut d2 = vec![0.0f64; len];
        for a in 1..len - 1 {
            let (mut s, mut n) = (0.0, 0usize);
            for b in 0..other {
                let (p, q, r) = (at(a - 1, b), at(a, b), at(a + 1, b));
                if p.is_finite() && q.is_finite() && r.is_finite() {
                    s += p - 2.0 * q + r;
                    n += 1;
                }
            }
            if n > 0 {
                d2[a] = s / n as f64;
            }
        }
        let mut abs: Vec<f64> = d2.iter().map(|x| x.abs()).collect();
        let med = percentile(&mut abs, 0.5);
        let mut hits: Vec<(usize, f64)> = (1..len - 1).map(|a| (a, d2[a])).filter(|(_, v)| v.abs() > 6.0 * med).collect();
        hits.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        out += &format!("{label} {axis} creases: median |mean d2| {med:.4} m; {} over 6x:\n", hits.len());
        for (a, v) in hits.iter().take(30) {
            out += &format!("    {axis} {} (abs {}): d2 {v:+.3} m ({:.1}x)\n", a, u64::from(base) + *a as u64, v.abs() / med);
        }
    }
    out
}
