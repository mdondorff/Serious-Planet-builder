//! SPIKE-35 experiments: the breach rule, the chunk halo, the receiver rules, determinism across threads.
use super::look::{hillshade, save_gray};
use super::lem::*;
use super::net2::*;
use super::*;

const CORE: usize = 250;
const LW: usize = 1000;

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() - 1) as f64 * q) as usize]
}

fn put<T: Copy>(f: &mut std::io::BufWriter<std::fs::File>, v: &[T]) {
    use std::io::Write;
    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) };
    f.write_all(bytes).unwrap();
}

fn get<T: Copy + Default>(f: &mut std::io::BufReader<std::fs::File>, n: usize) -> Vec<T> {
    use std::io::Read;
    let mut v = vec![T::default(); n];
    let bytes = unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, std::mem::size_of_val(&v[..])) };
    f.read_exact(bytes).unwrap();
    v
}

/// The coarse run, cached in `target/` of the prototype (the arrays the site needs).
fn coarse(n: usize, steps: usize, threads: usize) -> CoarseState {
    let path = format!("target/coarse-{n}-{steps}.bin");
    if std::path::Path::new(&path).exists() {
        let t = Instant::now();
        let sph = Sphere::new(n);
        let len = sph.len();
        let mut f = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
        let z: Vec<f32> = get(&mut f, len);
        let a_m2: Vec<f32> = get(&mut f, len);
        let rec: Vec<u32> = get(&mut f, len);
        let u: Vec<f32> = get(&mut f, len);
        let k: Vec<f32> = get(&mut f, len);
        println!("COARSE cache {path}: loaded in {:.1}s", t.elapsed().as_secs_f64());
        return CoarseState { sph, z, z0: vec![], a_m2, rec, u, k };
    }
    let (cs, _) = coarse_run(n, threads, steps, 0.6, "sp35");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    put(&mut f, &cs.z);
    put(&mut f, &cs.a_m2);
    put(&mut f, &cs.rec);
    put(&mut f, &cs.u);
    put(&mut f, &cs.k);
    cs
}

fn site(cs: &CoarseState, ds: f64, extent: f64, threads: usize) -> Site {
    let (c, lat, lon) = find_site(cs);
    let mut s = Site::build(cs, c, lat, lon, ds, extent, 400.0, threads);
    s.build_net(cs, 100.0e6);
    s
}

fn opts_of(name: &str) -> (NetOpts, f64) {
    // (options, meander amplitude in m)
    match name {
        "c0" => (NetOpts { route: Rule::D8, blend: true, protect: false, breach: true, valley: false }, 0.0),
        "c1" => (NetOpts { route: Rule::D8, blend: true, protect: true, breach: true, valley: false }, 0.0),
        "c2" => (NetOpts { route: Rule::D8, blend: true, protect: true, breach: true, valley: true }, 700.0),
        "c3" => (NetOpts { route: Rule::D8, blend: false, protect: true, breach: true, valley: true }, 700.0),
        "c4" => (NetOpts { route: Rule::D8, blend: false, protect: true, breach: true, valley: true }, 700.0),
        "jit" => (NetOpts { route: Rule::Jitter(0.4), blend: false, protect: true, breach: true, valley: true }, 700.0),
        "dinf" => (NetOpts { route: Rule::DInf, blend: false, protect: true, breach: true, valley: true }, 700.0),
        other => panic!("unknown options {other}"),
    }
}

fn groove_depth(z: &[f64], d: &Domain) -> (f64, f64, usize) {
    let mut v = Vec::new();
    for j in 10..d.h - 10 {
        for i in 10..d.w - 10 {
            let c = j * d.w + i;
            if d.corr[c].0 < 30.0 && !d.ring[c] {
                let mut m = 0.0;
                for (dx, dy) in [(-8i32, 0i32), (8, 0), (0, -8), (0, 8), (-6, -6), (6, -6), (-6, 6), (6, 6)] {
                    m += z[((j as i32 + dy) as usize) * d.w + (i as i32 + dx) as usize];
                }
                v.push(m / 8.0 - z[c]);
            }
        }
    }
    (pct(&v, 0.5), pct(&v, 0.9), v.len())
}

fn sheet(path: &str, zs: &[&[f64]], w: usize, ds: f64, az: f64, el: f64, x0: usize, cw: usize, step: usize) {
    let pw = cw / step;
    let mut img = vec![0u8; pw * zs.len() * pw];
    for (k, z) in zs.iter().enumerate() {
        let g = hillshade(z, w, ds, az, el, x0, x0, cw, step);
        for r in 0..pw {
            img[r * pw * zs.len() + k * pw..r * pw * zs.len() + (k + 1) * pw].copy_from_slice(&g[r * pw..(r + 1) * pw]);
        }
    }
    save_gray(path, (pw * zs.len()) as u32, pw as u32, &img);
}

pub fn run(a: &[String], out: &str) {
    let cmd = a.get(2).map(String::as_str).unwrap_or("");
    let threads = 6;
    let cs = coarse(2048, 100, threads);
    let mut rep = String::new();
    macro_rules! log {
        ($($t:tt)*) => {{ let s = format!($($t)*); println!("{s}"); rep += &s; rep += "\n"; }};
    }
    match cmd {
        "breach" => {
            let mut s = site(&cs, 20.0, 20000.0, threads);
            let mut zs: Vec<(String, Vec<f64>)> = Vec::new();
            for name in ["c0", "c1", "c2", "c3", "c4"] {
                let (o, amp) = opts_of(name);
                s.build_paths(amp);
                let d = s.domain_net2(&cs, 0, 0, LW, LW, o.blend);
                let (r, st) = evolve_net2(&d, 30, &PARAMS, &o);
                let (up, ch, upr) = uphill_share(&r, &d);
                let (r0, _) = evolve_net2(&d, 0, &PARAMS, &o);
                let (up0, _, up0r) = uphill_share(&r0, &d);
                let (g50, g90, gn) = groove_depth(&r.z, &d);
                log!("BREACH {name} ({o:?}, meander {amp} m): uphill channel cells {up} of {ch} ({:.2} %; {upr} with a ring receiver; right after the breach, before any step: {up0}, {up0r} with a ring receiver); breach carved {} cells, mean {:.1} m, max {:.0} m; cycle cells {}, ring outlets {}; corridor bed below its surroundings (8 points at 160 m) p50 {:.1} m p90 {:.1} m over {gn} cells; {:.2} s", 100.0 * up as f64 / ch.max(1) as f64, st.carved, st.carve_mean, st.carve_max, st.cyc, st.outlets, g50, g90, r.ms / 1000.0);
                if name == "c3" {
                    let mut shown = 0;
                    for c in 0..LW * LW {
                        if !d.ring[c] && r.a[c] > CH_A && r.rec[c] != u32::MAX && r.z[r.rec[c] as usize] > r.z[c] + 1.0e-6 && shown < 6 {
                            let rr = r.rec[c] as usize;
                            log!("DEBUG uphill cell ({}, {}): A {:.0} z {:.3}; receiver A {:.0} z {:.3} ring {} corridor d {:.0}/{:.0}; its receiver {}", c % LW, c / LW, r.a[c], r.z[c], r.a[rr], r.z[rr], d.ring[rr], d.corr[c].0, d.corr[rr].0, if r.rec[rr] == u32::MAX { "none".to_string() } else { format!("z {:.3}", r.z[r.rec[rr] as usize]) });
                            shown += 1;
                        }
                    }
                }
                zs.push((name.to_string(), r.z));
            }
            // natural channels away from the corridor, for scale
            let d = s.domain_net2(&cs, 0, 0, LW, LW, false);
            let (r, _) = evolve_net2(&d, 30, &PARAMS, &opts_of("c3").0);
            let mut v = Vec::new();
            for j in 10..LW - 10 {
                for i in 10..LW - 10 {
                    let c = j * LW + i;
                    if r.a[c] > CH_A && d.corr[c].0 > 300.0 && !d.ring[c] {
                        let mut m = 0.0;
                        for (dx, dy) in [(-8i32, 0i32), (8, 0), (0, -8), (0, 8), (-6, -6), (6, -6), (-6, 6), (6, 6)] {
                            m += r.z[((j as i32 + dy) as usize) * LW + (i as i32 + dx) as usize];
                        }
                        v.push(m / 8.0 - r.z[c]);
                    }
                }
            }
            log!("BREACH natural channel cells (A > {CH_A:.0} m2, over 300 m from the corridor): bed below its surroundings p50 {:.1} m p90 {:.1} m over {} cells", pct(&v, 0.5), pct(&v, 0.9), v.len());
            let (x0, cw) = (250, 500);
            let refs: Vec<&[f64]> = vec![&zs[0].1, &zs[3].1, &zs[4].1];
            sheet(&format!("{out}/breach_sunset.png"), &refs, LW, 20.0, 262.0, 25.0, x0, cw, 2);
            sheet(&format!("{out}/breach_midday.png"), &refs, LW, 20.0, 200.0, 55.0, x0, cw, 2);
            std::fs::write(format!("{out}/breach.txt"), rep).ok();
        }
        "halo" => {
            let name = a.get(3).map(String::as_str).unwrap_or("c3");
            let (o, amp) = opts_of(name);
            let mut s = site(&cs, 20.0, 20000.0, threads);
            s.build_paths(amp);
            let dt = s.domain_net2(&cs, 0, 0, LW, LW, o.blend);
            let (truth, _) = evolve_net2(&dt, 30, &PARAMS, &o);
            log!("HALO options {name}; truth: whole 20 km lattice, {:.1} s", truth.ms / 1000.0);
            for hm in [0.5f64, 1.0, 1.5, 2.0, 3.0, 5.0] {
                let hc = (hm * 1000.0 / 20.0).round() as usize;
                let run1 = |bi: usize| {
                    let ci = 250 + bi * CORE;
                    let (i0, j0, w) = (ci - hc, 250 - hc, CORE + 2 * hc);
                    let d = s.domain_net2(&cs, i0, j0, w, w, o.blend);
                    let (r, _) = evolve_net2(&d, 30, &PARAMS, &o);
                    (i0, j0, w, r)
                };
                let (ca, cb) = std::thread::scope(|sc| {
                    let h1 = sc.spawn(|| run1(0));
                    let h2 = sc.spawn(|| run1(1));
                    (h1.join().unwrap(), h2.join().unwrap())
                });
                let (mut dz, mut diff_bits, mut dl) = (Vec::new(), 0usize, Vec::new());
                for (bi, ch) in [&ca, &cb].iter().enumerate() {
                    let ci = 250 + bi * CORE;
                    for y in 250..250 + CORE {
                        for x in ci..ci + CORE {
                            let (zc, ac) = (ch.3.z[(y - ch.1) * ch.2 + x - ch.0], ch.3.a[(y - ch.1) * ch.2 + x - ch.0]);
                            let (zt, at) = (truth.z[y * LW + x], truth.a[y * LW + x]);
                            if zc.to_bits() != zt.to_bits() {
                                diff_bits += 1;
                            }
                            dz.push((zc - zt).abs());
                            dl.push((libm::log(ac.max(1.0)) - libm::log(at.max(1.0))).abs());
                        }
                    }
                }
                // the seam at x = 500 between the two chunks, against the truth's step at the same place
                let (mut excess, mut brk, mut allc, mut up, mut chn) = (Vec::new(), 0usize, 0usize, 0usize, 0usize);
                let za = |x: usize, y: usize| ca.3.z[(y - ca.1) * ca.2 + x - ca.0];
                let zb = |x: usize, y: usize| cb.3.z[(y - cb.1) * cb.2 + x - cb.0];
                for y in 250..250 + CORE {
                    let step = (zb(500, y) - za(499, y)).abs();
                    let tstep = (truth.z[y * LW + 500] - truth.z[y * LW + 499]).abs();
                    excess.push((step - tstep).abs());
                }
                let (lo, hi) = (475usize, 525usize); // a band of 1 km round the seam that both chunks cover at every halo
                for y in 250..250 + CORE {
                    for x in lo..hi {
                        let (aa, ab) = (ca.3.a[(y - ca.1) * ca.2 + x - ca.0], cb.3.a[(y - cb.1) * cb.2 + x - cb.0]);
                        if aa > CH_A || ab > CH_A {
                            allc += 1;
                            if aa.min(ab) < CH_A / 5.0 {
                                brk += 1;
                            }
                        }
                    }
                    for x in 497..503usize {
                        let (c, ix) = if x < 500 { (&ca, x) } else { (&cb, x) };
                        let (ai, rr) = ((y - c.1) * c.2 + ix - c.0, c.3.rec[(y - c.1) * c.2 + ix - c.0]);
                        if c.3.a[ai] > CH_A && rr != u32::MAX {
                            chn += 1;
                            let (rx, ry) = (c.0 + rr as usize % c.2, c.1 + rr as usize / c.2);
                            let joined = |x: usize, y: usize| if x < 500 { za(x, y) } else { zb(x, y) };
                            if joined(rx, ry) > joined(ix, y) {
                                up += 1;
                            }
                        }
                    }
                }
                log!("HALO {hm:.1} km: cores against truth: {diff_bits} of {} cells differ in bits, |dz| p99 {:.4} max {:.3} m, |dlnA| mean {:.5}; seam step minus the truth's step p50 {:.3} p99 {:.3} max {:.3} m; channel cells (A > {CH_A:.0}) in the 1 km band round the seam that the other chunk sees under a fifth: {brk} of {allc}; hard join uphill channel cells {up} of {chn}; chunks {:.1} s and {:.1} s", dz.len(), pct(&dz, 0.99), dz.iter().cloned().fold(0.0, f64::max), dl.iter().sum::<f64>() / dl.len() as f64, pct(&excess, 0.5), pct(&excess, 0.99), excess.iter().cloned().fold(0.0, f64::max), ca.3.ms / 1000.0, cb.3.ms / 1000.0);
            }
            std::fs::write(format!("{out}/halo-{name}.txt"), rep).ok();
        }
        "routes" => {
            let mut s = site(&cs, 4.0, 7000.0, threads);
            s.build_paths(250.0);
            let n = s.lw;
            let d = s.domain_net2(&cs, 0, 0, n, n, false);
            let mut zs: Vec<(String, Vec<f64>)> = Vec::new();
            for (name, route) in [("D8", Rule::D8), ("jitter 0.4", Rule::Jitter(0.4)), ("jitter 0.8", Rule::Jitter(0.8)), ("D-inf", Rule::DInf)] {
                let o = NetOpts { route, blend: false, protect: true, breach: true, valley: true };
                let (r, st) = evolve_net2(&d, 30, &PARAMS, &o);
                let (up, ch, _) = uphill_share(&r, &d);
                let (ax1, n1) = axis_share(&r, &d, 2.0e4);
                let (ax2, n2) = axis_share(&r, &d, 2.0e5);
                log!("ROUTE {name}: {:.2} s for 49 km2 at 4 m ({:.0} ns per cell-step); axis share of the receivers over 2e4 m2 {:.3} ({n1} cells), over 2e5 m2 {:.3} ({n2} cells) (0.5 = no preferred direction); uphill channel cells {up} of {ch}; cycle cells {}", r.ms / 1000.0, r.ms * 1e6 / (n * n) as f64 / 30.0, ax1, ax2, st.cyc);
                zs.push((name.to_string(), r.z));
            }
            let x0 = (n - 1248) / 2;
            let refs: Vec<&[f64]> = vec![&zs[0].1, &zs[1].1, &zs[3].1];
            sheet(&format!("{out}/routes_sunset.png"), &refs, n, 4.0, 262.0, 6.0, x0, 1248, 4);
            sheet(&format!("{out}/routes_midday.png"), &refs, n, 4.0, 200.0, 55.0, x0, 1248, 4);
            std::fs::write(format!("{out}/routes.txt"), rep).ok();
        }
        "det" => {
            let name = a.get(3).map(String::as_str).unwrap_or("c3");
            let (o, amp) = opts_of(name);
            let mut s = site(&cs, 20.0, 20000.0, threads);
            s.build_paths(amp);
            let (core, hc) = (125usize, 25usize);
            let jobs: Vec<(usize, usize)> = (0..16).map(|k| (250 + (k % 4) * core, 250 + (k / 4) * core)).collect();
            let hash = |threads: usize| -> u64 {
                let chunk = jobs.len().div_ceil(threads);
                let mut res: Vec<(usize, u64)> = Vec::new();
                std::thread::scope(|sc| {
                    let hs: Vec<_> = jobs
                        .chunks(chunk)
                        .enumerate()
                        .map(|(ti, js)| {
                            let (s, cs) = (&s, &cs);
                            sc.spawn(move || {
                                js.iter()
                                    .enumerate()
                                    .map(|(k, &(ci, cj))| {
                                        let (i0, j0, w) = (ci - hc, cj - hc, core + 2 * hc);
                                        let d = s.domain_net2(cs, i0, j0, w, w, o.blend);
                                        let (r, _) = evolve_net2(&d, 30, &PARAMS, &o);
                                        let mut h = 0xcbf2_9ce4_8422_2325u64;
                                        for y in hc..hc + core {
                                            for x in hc..hc + core {
                                                for v in [r.z[y * w + x], r.a[y * w + x]] {
                                                    for b in v.to_bits().to_le_bytes() {
                                                        h ^= b as u64;
                                                        h = h.wrapping_mul(0x0000_0100_0000_01b3);
                                                    }
                                                }
                                            }
                                        }
                                        (ti * chunk + k, h)
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    for h in hs {
                        res.extend(h.join().unwrap());
                    }
                });
                res.sort_by_key(|r| r.0);
                res.iter().fold(0u64, |acc, r| acc.rotate_left(13) ^ r.1)
            };
            let mut hs = Vec::new();
            for t in [1usize, 4, 16] {
                for run in 0..2 {
                    let t0 = Instant::now();
                    let h = hash(t);
                    log!("DET A-net {name}: 16 chunks (core 2.5 km, halo 0.5 km, N = 30), {t} threads, run {run}: hash {h:016x} ({:.1} s)", t0.elapsed().as_secs_f64());
                    hs.push(h);
                }
            }
            log!("DET A-net: {}", if hs.iter().all(|&h| h == hs[0]) { "PASS (bit-exact)" } else { "FAIL" });
            std::fs::write(format!("{out}/det.txt"), rep).ok();
        }
        _ => eprintln!("sp35 breach | halo [opts] | routes | det [opts]"),
    }
}
