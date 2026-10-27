//! SPIKE-33 A-net: a frozen river network from the coarse run, the same in every block.
use super::lem::*;
use super::*;

pub const NET_WIDTH_M: f64 = 80.0;
const NB: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

impl Site {
    /// The coarse cells above `a_thr_m2` (and their receivers) whose path touches the lattice, as segments.
    pub fn build_net(&mut self, cs: &CoarseState, a_thr_m2: f64) {
        let half = self.half();
        let cell_m = core::f64::consts::PI * R_M / 2.0 / cs.sph.n as f64;
        let mut cells = std::collections::BTreeSet::new();
        let (mut y, step) = (-half - 3.0 * cell_m, cell_m * 0.5);
        while y <= half + 3.0 * cell_m {
            let mut x = -half - 3.0 * cell_m;
            while x <= half + 3.0 * cell_m {
                cells.insert(cs.sph.cell_of(self.fr.dir(x, y)));
                x += step;
            }
            y += step;
        }
        let mut segs = Vec::new();
        for &d in &cells {
            if (cs.a_m2[d] as f64) < a_thr_m2 || cs.rec[d] == NONE {
                continue;
            }
            let (f, i, j) = cs.sph.split(d);
            let (x0, y0) = self.fr.xy(cs.sph.direction(f, i, j));
            let (rf, ri, rj) = cs.sph.split(cs.rec[d] as usize);
            let (x1, y1) = self.fr.xy(cs.sph.direction(rf, ri, rj));
            segs.push([x0, y0, x1, y1]);
        }
        self.segs = segs;
    }

    fn corridor_at(&self, x: f64, y: f64) -> (f32, f32, f32) {
        let mut best = (f64::MAX, 0.0, 0.0);
        for sg in &self.segs {
            let (vx, vy) = (sg[2] - sg[0], sg[3] - sg[1]);
            let l2 = vx * vx + vy * vy;
            let t = (((x - sg[0]) * vx + (y - sg[1]) * vy) / l2).clamp(0.0, 1.0);
            let (px, py) = (sg[0] + t * vx, sg[1] + t * vy);
            let d = libm::sqrt((x - px) * (x - px) + (y - py) * (y - py));
            if d < best.0 {
                let l = libm::sqrt(l2);
                best = (d, vx / l, vy / l);
            }
        }
        (best.0 as f32, best.1 as f32, best.2 as f32)
    }

    /// Like `domain`, with the corridor of the coarse network: heights in it are blended to the coarse height minus 15 m.
    pub fn domain_net(&self, cs: &CoarseState, i0: usize, j0: usize, w: usize, h: usize) -> Domain {
        let mut dm = self.domain(cs, i0, j0, w, h, true);
        let half = self.half();
        dm.corr = vec![(f32::MAX, 0.0, 0.0); w * h];
        for j in 0..h {
            for i in 0..w {
                let (x, y) = (-half + ((i0 + i) as f64 + 0.5) * self.ds, -half + ((j0 + j) as f64 + 0.5) * self.ds);
                let cr = self.corridor_at(x, y);
                let c = j * w + i;
                if (cr.0 as f64) < 3.0 * NET_WIDTH_M {
                    let g = libm::exp(-(cr.0 as f64 / NET_WIDTH_M).powi(2));
                    let zc = self.zc[(j0 + j) * self.lw + i0 + i] - 15.0;
                    if !dm.ring[c] {
                        dm.z0[c] = dm.z0[c] * (1.0 - g) + zc * g;
                    }
                    dm.corr[c] = cr;
                }
            }
        }
        dm
    }
}

fn topo_order(rec: &[u32]) -> (Vec<u32>, usize) {
    let n = rec.len();
    let mut indeg = vec![0u32; n];
    for ci in 0..n {
        if rec[ci] != u32::MAX {
            indeg[rec[ci] as usize] += 1;
        }
    }
    let mut q: Vec<u32> = (0..n as u32).filter(|&c| indeg[c as usize] == 0).collect();
    let mut qi = 0;
    while qi < q.len() {
        let c = q[qi] as usize;
        qi += 1;
        let r = rec[c];
        if r != u32::MAX {
            indeg[r as usize] -= 1;
            if indeg[r as usize] == 0 {
                q.push(r);
            }
        }
    }
    let cyc = n - q.len();
    (q, cyc)
}

/// `evolve` with a frozen network: the receivers are a position-only rule (along the coarse path inside its corridor, steepest descent on the
/// initial heights elsewhere), the area is accumulated once, and every step is implicit stream power along that network, diffusion and slumping.
pub fn evolve_net(d: &Domain, steps: u32, p: &Params) -> (Out, usize) {
    let t0 = Instant::now();
    let (w, h) = (d.w, d.h);
    let n = w * h;
    let mut z = d.z0.clone();
    let diag = d.ds * core::f64::consts::SQRT_2;
    let mut rec = vec![u32::MAX; n];
    for ci in 0..n {
        let (cx, cy) = ((ci % w) as i32, (ci / w) as i32);
        let cr = d.corr[ci];
        let mut best = (-1.0f64, u32::MAX);
        if cr.0 < (3.0 * NET_WIDTH_M) as f32 {
            for (dx, dy) in NB {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let l = libm::sqrt((dx * dx + dy * dy) as f64);
                let dot = (dx as f64 * cr.1 as f64 + dy as f64 * cr.2 as f64) / l;
                if dot > best.0 && dot > 0.3 {
                    best = (dot, (ny as usize * w + nx as usize) as u32);
                }
            }
        }
        if best.1 == u32::MAX {
            let mut bs = (0.0f64, u32::MAX);
            for (dx, dy) in NB {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let ni = ny as usize * w + nx as usize;
                let drop = z[ci] - z[ni];
                if drop > 0.0 {
                    let s = drop / if dx != 0 && dy != 0 { diag } else { d.ds };
                    if s > bs.0 {
                        bs = (s, ni as u32);
                    }
                }
            }
            best = bs;
        }
        rec[ci] = best.1;
    }
    let (mut topo, mut cyc) = topo_order(&rec);
    if cyc > 0 {
        // cells never reached are in a cycle or drain into one: cut every cell whose in-degree stayed positive
        let mut seen = vec![false; n];
        for &c in &topo {
            seen[c as usize] = true;
        }
        for c in 0..n {
            if !seen[c] {
                rec[c] = u32::MAX;
            }
        }
        let (t2, c2) = topo_order(&rec);
        topo = t2;
        cyc += c2 * 0;
    }
    // the breach: walk donors first and lower each receiver to at least 0.2 % below its donor, so every receiver is downhill (ring cells keep their height)
    let (mut carved, mut carve_m) = (0usize, 0f64);
    for &c in &topo {
        let ci = c as usize;
        let r = rec[ci];
        if r == u32::MAX || d.ring[r as usize] {
            continue;
        }
        let ri = r as usize;
        let dist = if (ci % w).abs_diff(ri % w) + (ci / w).abs_diff(ri / w) == 2 { diag } else { d.ds };
        let lim = z[ci] - 0.002 * dist;
        if z[ri] > lim {
            carve_m += z[ri] - lim;
            z[ri] = lim;
            carved += 1;
        }
    }
    println!("NET breach: {carved} cells carved, mean {:.2} m", carve_m / carved.max(1) as f64);
    let mut a: Vec<f64> = (0..n).map(|c| d.ds * d.ds + d.inflow[c]).collect();
    for &c in &topo {
        let r = rec[c as usize];
        if r != u32::MAX {
            a[r as usize] += a[c as usize];
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
                    if !d.ring[c] {
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
    (Out { rec, z, a, ms: t0.elapsed().as_secs_f64() * 1000.0 }, cyc)
}
