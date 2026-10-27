//! SPIKE-33 experiments: light cone, determinism, cost.
use super::lem::*;
use super::*;

fn pctile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q) as usize]
}

fn fnv(z: &[f64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in z {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

pub struct Bench {
    pub cs: CoarseState,
    pub site: Site,
    pub truth: Vec<(u32, Out)>,
}

const CORE: usize = 250; // 5 km at 20 m
pub const LW: usize = 1000; // 20 km

pub fn setup(coarse_n: usize, steps: usize, threads: usize, ds: f64, extent: f64, detail_m: f64) -> (CoarseState, Site) {
    let (cs, log) = coarse_run(coarse_n, threads, steps, 0.6, "sweep");
    let _ = log;
    let (c, lat, lon) = find_site(&cs);
    let site = Site::build(&cs, c, lat, lon, ds, extent, detail_m, threads);
    (cs, site)
}

pub fn run_block(site: &Site, cs: &CoarseState, bi: usize, bj: usize, hc: usize, n: u32, oracle: Option<&Out>) -> (Vec<f64>, Vec<f64>, f64) {
    // core cells [c0, c0 + CORE) in the lattice; the central 2 x 2 blocks start at 250
    let (ci, cj) = (250 + bi * CORE, 250 + bj * CORE);
    let (i0, j0, w) = (ci - hc, cj - hc, CORE + 2 * hc);
    let mut dm = site.domain(cs, i0, j0, w, w, true);
    if let Some(t) = oracle {
        for c in 0..w * w {
            if dm.ring[c] {
                dm.z0[c] = t.z[(j0 + c / w) * site.lw + i0 + c % w];
            }
        }
    }
    let out = if n == 0 { Out { rec: vec![], z: dm.z0.clone(), a: vec![site.ds * site.ds; w * w], ms: 0.0 } } else { evolve(&dm, n, &PARAMS, false) };
    let mut zc = Vec::with_capacity(CORE * CORE);
    let mut ac = Vec::with_capacity(CORE * CORE);
    for j in 0..CORE {
        for i in 0..CORE {
            let c = (j + hc) * w + i + hc;
            zc.push(out.z[c]);
            ac.push(out.a[c]);
        }
    }
    (zc, ac, out.ms)
}

fn blocks(site: &Site, cs: &CoarseState, hc: usize, n: u32, threads: usize, oracle: Option<&Out>) -> Vec<(Vec<f64>, Vec<f64>, f64)> {
    let jobs: Vec<(usize, usize)> = vec![(0, 0), (1, 0), (0, 1), (1, 1)];
    let chunk = jobs.len().div_ceil(threads.max(1));
    let mut res: Vec<(usize, (Vec<f64>, Vec<f64>, f64))> = Vec::new();
    std::thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .chunks(chunk)
            .enumerate()
            .map(|(ti, js)| {
                sc.spawn(move || js.iter().enumerate().map(|(k, &(bi, bj))| (ti * chunk + k, run_block(site, cs, bi, bj, hc, n, oracle))).collect::<Vec<_>>())
            })
            .collect();
        for h in hs {
            res.extend(h.join().unwrap());
        }
    });
    res.sort_by_key(|r| r.0);
    res.into_iter().map(|r| r.1).collect()
}

fn truth_core(t: &Out, site: &Site, bi: usize, bj: usize) -> (Vec<f64>, Vec<f64>) {
    let (ci, cj) = (250 + bi * CORE, 250 + bj * CORE);
    let mut z = Vec::new();
    let mut a = Vec::new();
    for j in 0..CORE {
        for i in 0..CORE {
            let c = (cj + j) * site.lw + ci + i;
            z.push(t.z[c]);
            a.push(t.a[c]);
        }
    }
    (z, a)
}

pub fn sweep(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str) {
    let (cs, site) = setup(coarse_n, coarse_steps, threads, 20.0, 20000.0, 400.0);
    println!("PARAMS K0 {K0} U0 {U0} m {M_EXP} dt {DT} a_crit {} diff {} talus {} detail {} m, ds {} m, lattice {}", PARAMS.a_crit, PARAMS.diff, PARAMS.talus, site.detail_m, site.ds, site.lw);
    let mut report = String::new();
    // coarse upsample alone against truth, and the truth itself
    let ns = [10u32, 30, 100];
    let hs_m = [0.5f64, 1.0, 2.0, 5.0];
    for &n in &ns {
        let dm = site.domain(&cs, 0, 0, LW, LW, true);
        let t = evolve(&dm, n, &PARAMS, false);
        let relief = t.z.iter().cloned().fold(f64::MIN, f64::max) - t.z.iter().cloned().fold(f64::MAX, f64::min);
        let amax = t.a.iter().cloned().fold(0.0, f64::max);
        let line = format!("TRUTH N={n}: {:.1} s for {} cells ({:.0} ms/step), relief {:.0} m, max A {:.0} km2, min z {:.0}", t.ms / 1000.0, LW * LW, t.ms / n as f64, relief, amax / 1e6, t.z.iter().cloned().fold(f64::MAX, f64::min));
        println!("{line}");
        report += &(line + "\n");
        let tc: Vec<(Vec<f64>, Vec<f64>)> = (0..4).map(|b| truth_core(&t, &site, b % 2, b / 2)).collect();
        for hm in std::iter::once(0.0).chain(hs_m.iter().copied()) {
            let hc = (hm * 1000.0 / site.ds).round() as usize;
            let bl = if hm == 0.0 { blocks(&site, &cs, 0, 0, threads, None) } else { blocks(&site, &cs, hc, n, threads, None) };
            let (mut dz, mut dl, mut cnt) = (Vec::new(), Vec::new(), 0usize);
            let mut ms = 0.0;
            for (b, (zb, ab, m)) in bl.iter().enumerate() {
                ms += m;
                for q in 0..zb.len() {
                    dz.push((zb[q] - tc[b].0[q]).abs());
                    if hm > 0.0 {
                        dl.push((libm::log(ab[q].max(1.0)) - libm::log(tc[b].1[q].max(1.0))).abs());
                    }
                    cnt += 1;
                }
            }
            let _ = cnt;
            let mean_l = if dl.is_empty() { 0.0 } else { dl.iter().sum::<f64>() / dl.len() as f64 };
            let line = format!(
                "N={n:3} H={hm:.1} km: |dz| p50 {:.2} p90 {:.2} p99 {:.2} max {:.1} m; |dlnA| mean {:.3} p99 {:.2}; block ms mean {:.0}",
                pctile(&mut dz.clone(), 0.5),
                pctile(&mut dz.clone(), 0.9),
                pctile(&mut dz.clone(), 0.99),
                dz.iter().cloned().fold(0.0, f64::max),
                mean_l,
                pctile(&mut dl.clone(), 0.99),
                ms / 4.0
            );
            println!("{line}");
            report += &(line + "\n");
        }
    }
    std::fs::write(format!("{out}/sweep.txt"), report).ok();
}

/// The same light cone with the ring heights taken from the truth (an oracle boundary): the dynamics alone.
pub fn oracle(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str) {
    let (cs, site) = setup(coarse_n, coarse_steps, threads, 20.0, 20000.0, 400.0);
    let mut report = String::new();
    for &n in &[10u32, 30, 100] {
        let t = evolve(&site.domain(&cs, 0, 0, LW, LW, true), n, &PARAMS, false);
        let tc: Vec<(Vec<f64>, Vec<f64>)> = (0..4).map(|b| truth_core(&t, &site, b % 2, b / 2)).collect();
        for hm in [0.5f64, 1.0, 2.0, 5.0] {
            let hc = (hm * 1000.0 / site.ds).round() as usize;
            let bl = blocks(&site, &cs, hc, n, threads, Some(&t));
            let mut dz = Vec::new();
            let mut dl = Vec::new();
            for (b, (zb, ab, _)) in bl.iter().enumerate() {
                for q in 0..zb.len() {
                    dz.push((zb[q] - tc[b].0[q]).abs());
                    dl.push((libm::log(ab[q].max(1.0)) - libm::log(tc[b].1[q].max(1.0))).abs());
                }
            }
            let line = format!("ORACLE N={n:3} H={hm:.1} km: |dz| p50 {:.2} p90 {:.2} p99 {:.2} max {:.1} m; |dlnA| mean {:.3} p99 {:.2}", pctile(&mut dz.clone(), 0.5), pctile(&mut dz.clone(), 0.9), pctile(&mut dz.clone(), 0.99), dz.iter().cloned().fold(0.0, f64::max), dl.iter().sum::<f64>() / dl.len() as f64, pctile(&mut dl.clone(), 0.99));
            println!("{line}");
            report += &(line + "
");
        }
    }
    std::fs::write(format!("{out}/oracle.txt"), report).ok();
}

pub fn determinism(coarse_n: usize, threads: usize, out: &str) {
    let mut report = String::new();
    // the coarse run: 1 and `threads` threads, twice
    let mut hs = Vec::new();
    for &t in &[1usize, 4, 16] {
        for run in 0..2 {
            let (cs, _) = coarse_run(coarse_n, t, 4, 0.6, "det");
            let h = {
                let mut x = 0xcbf2_9ce4_8422_2325u64;
                for &v in &cs.z {
                    x ^= v.to_bits() as u64;
                    x = x.wrapping_mul(0x0000_0100_0000_01b3);
                }
                x
            };
            let line = format!("DET coarse n={coarse_n} threads {t} run {run}: hash {h:016x}");
            println!("{line}");
            report += &(line + "\n");
            hs.push(h);
        }
    }
    let line = format!("DET coarse: {}", if hs.iter().all(|&h| h == hs[0]) { "PASS (bit-exact)" } else { "FAIL" });
    println!("{line}");
    report += &(line + "\n");
    // the fine blocks
    let (cs, site) = setup(coarse_n, 8, threads.min(8), 20.0, 20000.0, 400.0);
    let mut hf = Vec::new();
    for &t in &[1usize, 4, 16] {
        for run in 0..2 {
            let bl = blocks(&site, &cs, 50, 20, t, None);
            let h = bl.iter().map(|b| fnv(&b.0) ^ fnv(&b.1).rotate_left(7)).fold(0u64, |a, b| a.rotate_left(13) ^ b);
            let line = format!("DET blocks (4 blocks, H 1 km, N 20) threads {t} run {run}: hash {h:016x}");
            println!("{line}");
            report += &(line + "\n");
            hf.push(h);
        }
    }
    let line = format!("DET blocks: {}", if hf.iter().all(|&h| h == hf[0]) { "PASS (bit-exact)" } else { "FAIL" });
    println!("{line}");
    report += &(line + "\n");
    std::fs::write(format!("{out}/determinism.txt"), report).ok();
}

/// Variants at N = 30: coarse or oracle (truth) inflow at the border, dynamic or frozen routing.
pub fn variants(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str) {
    let (cs, site) = setup(coarse_n, coarse_steps, threads, 20.0, 20000.0, 400.0);
    let n = 30u32;
    let t = evolve(&site.domain(&cs, 0, 0, LW, LW, true), n, &PARAMS, false);
    let tf = evolve(&site.domain(&cs, 0, 0, LW, LW, true), n, &PARAMS, true);
    let tc: Vec<(Vec<f64>, Vec<f64>)> = (0..4).map(|b| truth_core(&t, &site, b % 2, b / 2)).collect();
    let mut report = String::new();
    let relief: f64 = tc.iter().map(|c| c.0.iter().cloned().fold(f64::MIN, f64::max) - c.0.iter().cloned().fold(f64::MAX, f64::min)).sum::<f64>() / 4.0;
    let ch_thr = 5.0e4; // m2: channel cells
    let stats = |label: &str, zs: Vec<&Vec<f64>>, as_: Vec<&Vec<f64>>, report: &mut String| {
        let (mut all, mut chn, mut slp, mut dl) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for b in 0..4 {
            for q in 0..zs[b].len() {
                let e = (zs[b][q] - tc[b].0[q]).abs();
                all.push(e);
                if tc[b].1[q] > ch_thr {
                    chn.push(e)
                } else {
                    slp.push(e)
                }
                dl.push((libm::log(as_[b][q].max(1.0)) - libm::log(tc[b].1[q].max(1.0))).abs());
            }
        }
        let n99 = all.iter().filter(|&&e| e > 0.0).count();
        let _ = n99;
        let thr = pctile(&mut all.clone(), 0.99);
        let in_ch = tc.iter().enumerate().map(|(b, c)| (0..c.0.len()).filter(|&q| (zs[b][q] - c.0[q]).abs() >= thr && c.1[q] > ch_thr).count()).sum::<usize>();
        let tot = tc.iter().enumerate().map(|(b, c)| (0..c.0.len()).filter(|&q| (zs[b][q] - c.0[q]).abs() >= thr).count()).sum::<usize>();
        let line = format!(
            "VAR {label}: |dz| p50 {:.2} p90 {:.2} p99 {:.2} m (p99 / core relief {:.3}); channels (A>{ch_thr:.0} m2, {:.1} % of cells) p99 {:.2}, slopes p99 {:.2}; {:.0} % of the p99 tail in channels; |dlnA| mean {:.3} p99 {:.2}",
            pctile(&mut all.clone(), 0.5),
            pctile(&mut all.clone(), 0.9),
            thr,
            thr / relief,
            100.0 * chn.len() as f64 / all.len() as f64,
            pctile(&mut chn, 0.99),
            pctile(&mut slp, 0.99),
            100.0 * in_ch as f64 / tot.max(1) as f64,
            dl.iter().sum::<f64>() / dl.len() as f64,
            pctile(&mut dl.clone(), 0.99)
        );
        println!("{line}");
        report.push_str(&(line + "\n"));
    };
    // the truth with frozen routing against the truth with dynamic routing (the cost of freezing alone)
    let tfc: Vec<(Vec<f64>, Vec<f64>)> = (0..4).map(|b| truth_core(&tf, &site, b % 2, b / 2)).collect();
    stats("truth-frozen routing against truth-dynamic", tfc.iter().map(|c| &c.0).collect(), tfc.iter().map(|c| &c.1).collect(), &mut report);
    report += &format!("core relief (mean of the four cores, truth) {:.0} m\n", relief);
    for hm in [5.0f64] {
        let hc = (hm * 1000.0 / site.ds).round() as usize;
        for (name, oflux, frozen) in [("coarse flux, dynamic", false, false), ("oracle flux, dynamic", true, false), ("coarse flux, frozen", false, true), ("oracle flux, frozen", true, true)] {
            let jobs: Vec<(usize, usize)> = vec![(0, 0), (1, 0), (0, 1), (1, 1)];
            let mut res: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
            let (tr, st) = (&t, &site);
            let csr = &cs;
            std::thread::scope(|sc| {
                let hs: Vec<_> = jobs
                    .iter()
                    .map(|&(bi, bj)| {
                        sc.spawn(move || {
                            let (ci, cj) = (250 + bi * CORE, 250 + bj * CORE);
                            let (i0, j0, w) = (ci - hc, cj - hc, CORE + 2 * hc);
                            let mut dm = st.domain(csr, i0, j0, w, w, !oflux);
                            if oflux {
                                for c in 0..w * w {
                                    if !dm.ring[c] {
                                        continue;
                                    }
                                    let (ri, rj) = ((c % w) as i64, (c / w) as i64);
                                    let rl = ((j0 as i64 + rj) * st.lw as i64 + i0 as i64 + ri) as u32;
                                    for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                                        let (oi, oj) = (ri + dx, rj + dy);
                                        if oi >= 0 && oj >= 0 && oi < w as i64 && oj < w as i64 {
                                            continue;
                                        }
                                        let (gi, gj) = (i0 as i64 + oi, j0 as i64 + oj);
                                        if gi < 0 || gj < 0 || gi >= st.lw as i64 || gj >= st.lw as i64 {
                                            continue;
                                        }
                                        let ol = (gj * st.lw as i64 + gi) as usize;
                                        if tr.rec[ol] == rl {
                                            dm.inflow[c] += tr.a[ol];
                                        }
                                    }
                                }
                            }
                            let o = evolve(&dm, n, &PARAMS, frozen);
                            let mut zc = Vec::new();
                            let mut ac = Vec::new();
                            for j in 0..CORE {
                                for i in 0..CORE {
                                    let c = (j + hc) * w + i + hc;
                                    zc.push(o.z[c]);
                                    ac.push(o.a[c]);
                                }
                            }
                            (zc, ac)
                        })
                    })
                    .collect();
                for h in hs {
                    res.push(h.join().unwrap());
                }
            });
            stats(&format!("H={hm:.0} km {name}"), res.iter().map(|r| &r.0).collect(), res.iter().map(|r| &r.1).collect(), &mut report);
        }
    }
    std::fs::write(format!("{out}/variants.txt"), report).ok();
}
