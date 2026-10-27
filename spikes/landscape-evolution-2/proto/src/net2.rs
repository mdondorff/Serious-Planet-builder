//! SPIKE-35: A-net with a meandered coarse path, a breach that protects the channel bed, and three receiver rules for the hillslopes
//! (D8, a jittered D8, D-infinity with the area split over two receivers).
use super::lem::*;
use super::*;

pub const CH_A: f64 = 5.0e4;
pub const NET_W: f64 = 80.0;
const BS: f64 = 120.0;
const NB: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Rule {
    D8,
    /// D8 with the slope of each direction scaled by `1 + j (2 h - 1)`, `h` a hash of the lattice cell and the direction.
    Jitter(f64),
    DInf,
}

#[derive(Clone, Copy, Debug)]
pub struct NetOpts {
    pub route: Rule,
    /// The corridor heights are blended to the coarse height minus 15 m (SPIKE-33).
    pub blend: bool,
    /// Channel cells (area over `CH_A`) take no diffusion and no slumping: the bed is shaped by stream power alone.
    pub protect: bool,
    pub breach: bool,
    /// Inside the corridor a cell drains to the lowest of the neighbours that point along the path (not the best aligned one).
    pub valley: bool,
}

impl NetOpts {
    pub const SPIKE33: NetOpts = NetOpts { route: Rule::D8, blend: true, protect: false, breach: true, valley: false };
}

pub struct NetStats {
    pub cyc: usize,
    pub carved: usize,
    pub carve_mean: f64,
    pub carve_max: f64,
    pub outlets: usize,
}

impl Site {
    /// The coarse river paths of `build_net` as polylines of points every 40 m, displaced across the path by a deterministic noise
    /// of amplitude `amp_m` and wavelength 0.9 km (zero at the ends, so the paths stay joined at the coarse cell centres).
    pub fn build_paths(&mut self, amp_m: f64) {
        let mut pts: Vec<[f32; 4]> = Vec::new();
        for (s, sg) in self.segs.iter().enumerate() {
            let (vx, vy) = (sg[2] - sg[0], sg[3] - sg[1]);
            let l = libm::sqrt(vx * vx + vy * vy);
            let (ux, uy) = (vx / l, vy / l);
            let n = (l / 40.0).ceil() as usize;
            let mut line: Vec<(f64, f64)> = Vec::with_capacity(n + 1);
            for k in 0..=n {
                let t = (k as f64 * 40.0).min(l);
                let off = amp_m * libm::sin(core::f64::consts::PI * t / l) * perlin(31 + s as u32, t / 900.0 + 7.3 * s as f64, 0.37);
                line.push((sg[0] + ux * t - uy * off, sg[1] + uy * t + ux * off));
            }
            for k in 0..line.len() {
                let (a, b) = (line[k.saturating_sub(1)], line[(k + 1).min(line.len() - 1)]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let dl = libm::sqrt(dx * dx + dy * dy).max(1e-9);
                pts.push([line[k].0 as f32, line[k].1 as f32, (dx / dl) as f32, (dy / dl) as f32]);
            }
        }
        let half = self.half();
        let nb = ((2.0 * half / BS).ceil() as usize) + 3;
        let mut buckets = vec![Vec::new(); nb * nb];
        for (i, p) in pts.iter().enumerate() {
            let (bx, by) = ((((p[0] as f64 + half) / BS).floor() as i64 + 1).clamp(0, nb as i64 - 1) as usize, (((p[1] as f64 + half) / BS).floor() as i64 + 1).clamp(0, nb as i64 - 1) as usize);
            buckets[by * nb + bx].push(i as u32);
        }
        self.path_pts = pts;
        self.buckets = buckets;
    }

    fn corridor2(&self, x: f64, y: f64) -> (f32, f32, f32) {
        let half = self.half();
        let nb = ((2.0 * half / BS).ceil() as usize) + 3;
        let (bx, by) = (((x + half) / BS).floor() as i64 + 1, ((y + half) / BS).floor() as i64 + 1);
        let mut best = (f64::MAX, 0.0f32, 0.0f32);
        for jy in -2..=2i64 {
            for jx in -2..=2i64 {
                let (qx, qy) = (bx + jx, by + jy);
                if qx < 0 || qy < 0 || qx >= nb as i64 || qy >= nb as i64 {
                    continue;
                }
                for &pi in &self.buckets[qy as usize * nb + qx as usize] {
                    let p = self.path_pts[pi as usize];
                    let (ddx, ddy) = (x - p[0] as f64, y - p[1] as f64);
                    let d = libm::sqrt(ddx * ddx + ddy * ddy);
                    if d < best.0 {
                        best = (d, p[2], p[3]);
                    }
                }
            }
        }
        (best.0 as f32, best.1, best.2)
    }

    /// `domain` with the corridor of the meandered paths; with `blend` its heights are pulled to the coarse height minus 15 m.
    pub fn domain_net2(&self, cs: &CoarseState, i0: usize, j0: usize, w: usize, h: usize, blend: bool) -> Domain {
        let mut dm = self.domain(cs, i0, j0, w, h, true);
        let half = self.half();
        dm.corr = vec![(f32::MAX, 0.0, 0.0); w * h];
        for j in 0..h {
            for i in 0..w {
                let (x, y) = (-half + ((i0 + i) as f64 + 0.5) * self.ds, -half + ((j0 + j) as f64 + 0.5) * self.ds);
                let cr = self.corridor2(x, y);
                let c = j * w + i;
                if (cr.0 as f64) < 3.0 * NET_W {
                    if blend && !dm.ring[c] {
                        let g = libm::exp(-(cr.0 as f64 / NET_W).powi(2));
                        let zc = self.zc[(j0 + j) * self.lw + i0 + i] - 15.0;
                        dm.z0[c] = dm.z0[c] * (1.0 - g) + zc * g;
                    }
                    dm.corr[c] = cr;
                }
            }
        }
        dm
    }
}

fn hash01(gi: usize, gj: usize, k: usize) -> f64 {
    let h = planet_core::hash::mix64((gi as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (gj as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (k as u64 + 1).wrapping_mul(0x1656_67B1_9E37_79F9));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn topo_order2(rec: &[u32], rec2: &[u32]) -> (Vec<u32>, usize) {
    let n = rec.len();
    let mut indeg = vec![0u32; n];
    for c in 0..n {
        if rec[c] != u32::MAX {
            indeg[rec[c] as usize] += 1;
        }
        if rec2[c] != u32::MAX {
            indeg[rec2[c] as usize] += 1;
        }
    }
    let mut q: Vec<u32> = (0..n as u32).filter(|&c| indeg[c as usize] == 0).collect();
    let mut qi = 0;
    while qi < q.len() {
        let c = q[qi] as usize;
        qi += 1;
        for r in [rec[c], rec2[c]] {
            if r != u32::MAX {
                indeg[r as usize] -= 1;
                if indeg[r as usize] == 0 {
                    q.push(r);
                }
            }
        }
    }
    let cyc = n - q.len();
    (q, cyc)
}

pub fn evolve_net2(d: &Domain, steps: u32, p: &Params, o: &NetOpts) -> (Out, NetStats) {
    let t0 = Instant::now();
    let (w, h) = (d.w, d.h);
    let n = w * h;
    let mut z = d.z0.clone();
    let diag = d.ds * core::f64::consts::SQRT_2;
    let mut rec = vec![u32::MAX; n];
    let mut rec2 = vec![u32::MAX; n];
    let mut w1 = vec![1.0f64; n];
    for ci in 0..n {
        let (cx, cy) = ((ci % w) as i32, (ci / w) as i32);
        let cr = d.corr[ci];
        let mut best = (-1.0f64, u32::MAX);
        if cr.0 < (3.0 * NET_W) as f32 {
            for (dx, dy) in NB {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let l = libm::sqrt((dx * dx + dy * dy) as f64);
                let dot = (dx as f64 * cr.1 as f64 + dy as f64 * cr.2 as f64) / l;
                if dot > 0.3 {
                    let ni = ny as usize * w + nx as usize;
                    // the score: the alignment, or (valley) the lowness of the neighbour with the alignment as the tie-break
                    let score = if o.valley { -z[ni] + 1.0e-6 * dot } else { dot };
                    if best.1 == u32::MAX || score > best.0 {
                        best = (score, ni as u32);
                    }
                }
            }
            rec[ci] = best.1;
            if best.1 != u32::MAX {
                continue;
            }
        }
        match o.route {
            Rule::D8 | Rule::Jitter(_) => {
                let jit = if let Rule::Jitter(j) = o.route { j } else { 0.0 };
                let mut bs = (0.0f64, u32::MAX);
                for (k, (dx, dy)) in NB.iter().enumerate() {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    let drop = z[ci] - z[ni];
                    if drop > 0.0 {
                        let mut s = drop / if *dx != 0 && *dy != 0 { diag } else { d.ds };
                        if jit > 0.0 {
                            s *= 1.0 + jit * (2.0 * hash01(d.off.0 + cx as usize, d.off.1 + cy as usize, k) - 1.0);
                        }
                        if s > bs.0 {
                            bs = (s, ni as u32);
                        }
                    }
                }
                rec[ci] = bs.1;
            }
            Rule::DInf => {
                // Tarboton's eight facets: the axis neighbour e1 and the next diagonal e2 (a facet lies between them)
                const FACETS: [(usize, usize); 8] = [(1, 7), (7, 3), (3, 6), (6, 0), (0, 4), (4, 2), (2, 5), (5, 1)];
                let at = |k: usize| -> Option<(usize, f64)> {
                    let (nx, ny) = (cx + NB[k].0, cy + NB[k].1);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        None
                    } else {
                        let ni = ny as usize * w + nx as usize;
                        Some((ni, z[ni]))
                    }
                };
                let mut best = (0.0f64, u32::MAX, u32::MAX, 1.0f64);
                for (a, b) in FACETS {
                    let (axis, dg) = if NB[a].0 != 0 && NB[a].1 != 0 { (b, a) } else { (a, b) };
                    let (Some((i1, z1)), Some((i2, z2))) = (at(axis), at(dg)) else { continue };
                    let s1 = (z[ci] - z1) / d.ds;
                    let s2 = (z1 - z2) / d.ds;
                    let mut r = libm::atan2(s2, s1);
                    let mut s = libm::sqrt(s1 * s1 + s2 * s2);
                    if r < 0.0 {
                        r = 0.0;
                        s = s1;
                    } else if r > core::f64::consts::FRAC_PI_4 {
                        r = core::f64::consts::FRAC_PI_4;
                        s = (z[ci] - z2) / diag;
                    }
                    if s > best.0 && z1 < z[ci] {
                        let wd = if z2 < z[ci] { r / core::f64::consts::FRAC_PI_4 } else { 0.0 };
                        best = (s, i1 as u32, if wd > 0.0 { i2 as u32 } else { u32::MAX }, 1.0 - wd);
                    }
                }
                if best.1 != u32::MAX {
                    if best.2 != u32::MAX && best.3 < 0.5 {
                        rec[ci] = best.2;
                        rec2[ci] = best.1;
                        w1[ci] = 1.0 - best.3;
                    } else {
                        rec[ci] = best.1;
                        rec2[ci] = best.2;
                        w1[ci] = best.3;
                    }
                }
            }
        }
    }
    let (mut topo, mut cyc) = topo_order2(&rec, &rec2);
    if cyc > 0 {
        let mut seen = vec![false; n];
        for &c in &topo {
            seen[c as usize] = true;
        }
        for c in 0..n {
            if !seen[c] {
                rec[c] = u32::MAX;
                rec2[c] = u32::MAX;
                w1[c] = 1.0;
            }
        }
        // cells that drained into a cut cell stay valid; only a cell whose receiver lost its own order needs the second pass
        let (t2, c2) = topo_order2(&rec, &rec2);
        topo = t2;
        cyc += c2 * 0;
    }
    let (mut carved, mut carve_sum, mut carve_max, mut outlets) = (0usize, 0f64, 0f64, 0usize);
    if o.breach {
        for &c in &topo {
            let ci = c as usize;
            let r = rec[ci];
            if r == u32::MAX {
                continue;
            }
            let ri = r as usize;
            let dist = if (ci % w).abs_diff(ri % w) + (ci / w).abs_diff(ri / w) == 2 { diag } else { d.ds };
            let lim = z[ci] - 0.002 * dist;
            if d.ring[ri] {
                // a ring cell keeps its height: a cell that would drain uphill into it becomes an outlet instead (no cascade upstream)
                if z[ri] > lim {
                    rec[ci] = u32::MAX;
                    outlets += 1;
                }
                continue;
            }
            if z[ri] > lim {
                let cut = z[ri] - lim;
                carve_sum += cut;
                carve_max = carve_max.max(cut);
                z[ri] = lim;
                carved += 1;
            }
        }
    }
    let mut a: Vec<f64> = (0..n).map(|c| d.ds * d.ds + d.inflow[c]).collect();
    for &c in &topo {
        let ci = c as usize;
        let (r, r2) = (rec[ci], rec2[ci]);
        if r != u32::MAX {
            let share = if r2 != u32::MAX { w1[ci] } else { 1.0 };
            a[r as usize] += a[ci] * share;
            if r2 != u32::MAX {
                a[r2 as usize] += a[ci] * (1.0 - share);
            }
        }
    }
    let coef: Vec<f64> = (0..n)
        .map(|ci| {
            let r = rec[ci];
            if r == u32::MAX {
                return 0.0;
            }
            let ri = r as usize;
            let dist = if (ci % w).abs_diff(ri % w) + (ci / w).abs_diff(ri / w) == 2 { diag } else { d.ds };
            d.k[ci] * libm::pow((a[ci] - p.a_crit).max(0.0), M_EXP) * DT / dist
        })
        .collect();
    let prot: Vec<bool> = (0..n).map(|c| o.protect && a[c] > CH_A && !d.ring[c]).collect();
    for _ in 0..steps {
        for &c in topo.iter().rev() {
            let ci = c as usize;
            let r = rec[ci];
            if d.ring[ci] || r == u32::MAX {
                continue;
            }
            let f = coef[ci];
            z[ci] = (z[ci] + f * z[r as usize]) / (1.0 + f);
        }
        if p.diff > 0.0 {
            let src = z.clone();
            let cf = (p.diff * DT / (d.ds * d.ds)).min(0.12);
            for j in 1..h - 1 {
                for i in 1..w - 1 {
                    let c = j * w + i;
                    if !d.ring[c] && !prot[c] {
                        z[c] = src[c] + cf * (src[c - 1] + src[c + 1] + src[c - w] + src[c + w] - 4.0 * src[c]);
                    }
                }
            }
        }
        let mut delta = vec![0f64; n];
        for j in 1..h - 1 {
            for i in 1..w - 1 {
                let c = j * w + i;
                for (dx, dy) in NB {
                    let q = (j as i32 + dy) as usize * w + (i as i32 + dx) as usize;
                    if prot[c] || prot[q] {
                        continue;
                    }
                    let dist = if dx != 0 && dy != 0 { diag } else { d.ds };
                    let slope = (z[c] - z[q]) / dist;
                    if slope > p.talus {
                        let m = 0.09 * (slope - p.talus) * dist;
                        if !d.ring[c] {
                            delta[c] -= m;
                        }
                        if !d.ring[q] {
                            delta[q] += m;
                        }
                    }
                }
            }
        }
        for c in 0..n {
            z[c] += delta[c];
        }
    }
    let stats = NetStats { cyc, carved, carve_mean: carve_sum / carved.max(1) as f64, carve_max, outlets };
    (Out { rec, z, a, ms: t0.elapsed().as_secs_f64() * 1000.0 }, stats)
}

/// Channel cells (area over `CH_A`, not on the ring) whose receiver is higher by more than 1 micrometre (the implicit step makes a bed
/// converge onto its receiver, so ties are not uphill): (uphill, channel cells, uphill with a ring receiver).
pub fn uphill_share(out: &Out, d: &Domain) -> (usize, usize, usize) {
    let (mut up, mut ch, mut ring) = (0usize, 0usize, 0usize);
    for c in 0..d.w * d.h {
        if d.ring[c] || out.a[c] <= CH_A || out.rec[c] == u32::MAX {
            continue;
        }
        ch += 1;
        if out.z[out.rec[c] as usize] > out.z[c] + 1.0e-6 {
            up += 1;
            if d.ring[out.rec[c] as usize] {
                ring += 1;
            }
        }
    }
    (up, ch, ring)
}

/// The share of the four axis directions among the receivers of the cells over `thr` m2 (0.5 for no preferred direction).
pub fn axis_share(out: &Out, d: &Domain, thr: f64) -> (f64, usize) {
    let (mut ax, mut n) = (0usize, 0usize);
    for c in 0..d.w * d.h {
        if d.ring[c] || out.a[c] <= thr || out.rec[c] == u32::MAX || d.corr[c].0 < (3.0 * NET_W) as f32 {
            continue;
        }
        let r = out.rec[c] as usize;
        let (dx, dy) = ((c % d.w).abs_diff(r % d.w), (c / d.w).abs_diff(r / d.w));
        n += 1;
        if dx + dy == 1 {
            ax += 1;
        }
    }
    (ax as f64 / n.max(1) as f64, n)
}
