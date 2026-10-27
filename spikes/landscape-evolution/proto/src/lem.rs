//! SPIKE-33: coupled stream-power landscape evolution. The coarse global run on the cube sphere, the planar fine solver for a
//! rectangle (block, halo, truth) and the coarse-to-fine mapping.
use super::*;

pub const M_EXP: f64 = 0.45;
pub const K0: f64 = 2.0e-5;
pub const U0: f64 = 1.0e-3;
/// A model step is 10 000 years.
pub const DT: f64 = 1.0e4;
/// The coarse run takes larger steps (implicit, stable): 100 000 years.
pub const DT_C: f64 = 1.0e5;

// ---------------------------------------------------------------- the coarse global run

pub struct CoarseState {
    pub sph: Sphere,
    pub z: Vec<f32>,
    pub z0: Vec<f32>,
    pub a_m2: Vec<f32>,
    pub rec: Vec<u32>,
    pub u: Vec<f32>,
    pub k: Vec<f32>,
}

pub struct StepLog {
    pub step: usize,
    pub mean_dz: f64,
    pub max_dz: f64,
    pub ms: f64,
}

fn par_collect<R: Send, F: Fn(usize) -> R + Sync>(len: usize, threads: usize, f: F) -> Vec<R> {
    let chunk = len.div_ceil(threads.max(1));
    let mut parts: Vec<Vec<R>> = Vec::new();
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads.max(1))
            .map(|t| {
                let f = &f;
                sc.spawn(move || (t * chunk..((t + 1) * chunk).min(len)).map(f).collect::<Vec<R>>())
            })
            .collect();
        for h in hs {
            parts.push(h.join().unwrap());
        }
    });
    parts.into_iter().flatten().collect()
}

/// The coarse run: uplift from the base height, stream power with a single (steepest) receiver, implicit update, `steps` steps.
pub fn coarse_run(n: usize, threads: usize, steps: usize, start_frac: f32, tag: &str) -> (CoarseState, Vec<StepLog>) {
    let sph = Sphere::new(n);
    let t0 = Instant::now();
    let Fields { z: z0, lith } = sample_sphere(&sph, threads);
    println!("COARSE {tag} n={n}: base sampled in {:.1}s ({threads} threads)", t0.elapsed().as_secs_f64());
    let len = sph.len();
    let u: Vec<f32> = z0.iter().map(|&h| (U0 * (h as f64 / 1000.0).clamp(0.05, 3.0)) as f32).collect();
    let k: Vec<f32> = lith.iter().map(|&l| (K0 * (0.5 + l as f64 / 255.0)) as f32).collect();
    let mut z: Vec<f32> = z0.iter().map(|&h| if h > 0.0 { (h * start_frac).max(0.5) } else { h }).collect();
    let mut a = vec![0f32; len];
    let mut rec = vec![NONE; len];
    let mut log = Vec::new();
    let mut fc = vec![0f64; len];
    for step in 0..steps {
        let ts = Instant::now();
        let (filled, order, _) = priority_flood(&sph, &z);
        // receivers: steepest descent on the filled surface (per cell, parallel, pure)
        let recs: Vec<(u32, u8)> = par_collect(order.len(), threads, |i| {
            let c = order[i] as usize;
            let nb = sph.nbrs(c);
            let mut best = (0f32, NONE, 0u8);
            for kk in 0..8 {
                if nb[kk] == NONE {
                    continue;
                }
                let nn = nb[kk] as usize;
                let drop = filled[c] - filled[nn];
                if drop > 0.0 {
                    let s = drop / sph.dist(c, kk, nn);
                    if s > best.0 {
                        best = (s, nn as u32, kk as u8);
                    }
                }
            }
            (best.1, best.2)
        });
        let mut recd = vec![0u8; 0];
        recd.reserve(order.len());
        for (i, &c) in order.iter().enumerate() {
            rec[c as usize] = recs[i].0;
            recd.push(recs[i].1);
            a[c as usize] = sph.area_km2(c as usize) * 1.0e6;
        }
        for &c in order.iter().rev() {
            let r = rec[c as usize];
            if r != NONE && z[r as usize] > 0.0 {
                a[r as usize] += a[c as usize];
            }
        }
        let coef: Vec<f64> = par_collect(order.len(), threads, |i| {
            let c = order[i] as usize;
            let r = rec[c];
            if r == NONE {
                return 0.0;
            }
            let kk = recd[i] as usize;
            let dist = sph.dist(c, kk, r as usize) as f64;
            k[c] as f64 * libm::pow(a[c] as f64, M_EXP) * DT_C / dist
        });
        for (i, &c) in order.iter().enumerate() {
            fc[c as usize] = coef[i];
        }
        let (mut sum, mut mx, mut cnt) = (0f64, 0f64, 0usize);
        for &c in order.iter() {
            let c = c as usize;
            let old = z[c] as f64;
            let r = rec[c];
            let zr = if r == NONE { 0.0 } else { (z[r as usize] as f64).max(0.0) };
            let f = fc[c];
            let new = ((old + DT_C * u[c] as f64 + f * zr) / (1.0 + f)).max(0.5);
            z[c] = new as f32;
            let d = (new - old).abs();
            sum += d;
            mx = mx.max(d);
            cnt += 1;
        }
        log.push(StepLog { step, mean_dz: sum / cnt as f64, max_dz: mx, ms: ts.elapsed().as_secs_f64() * 1000.0 });
        if step % 5 == 0 || step + 1 == steps {
            let l = log.last().unwrap();
            println!("COARSE {tag} step {step}: mean |dz| {:.3} m, max {:.1} m, {:.0} ms, peak {:.0} MB", l.mean_dz, l.max_dz, l.ms, peak_ws_mb());
        }
    }
    (CoarseState { sph, z, z0, a_m2: a, rec, u, k }, log)
}

// ---------------------------------------------------------------- the planar frame

#[derive(Clone, Copy)]
pub struct Frame {
    pub c: Vec3,
    pub e1: Vec3,
    pub e2: Vec3,
}

impl Frame {
    pub fn new(c: Vec3) -> Frame {
        let up = if c.z.abs() > 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 0.0, 1.0) };
        let e1 = up.cross(c).normalized();
        let e2 = c.cross(e1).normalized();
        Frame { c, e1, e2 }
    }
    pub fn dir(&self, x: f64, y: f64) -> Vec3 {
        (self.c + self.e1 * (x / R_M) + self.e2 * (y / R_M)).normalized()
    }
    pub fn xy(&self, d: Vec3) -> (f64, f64) {
        ((d - self.c).dot(self.e1) * R_M, (d - self.c).dot(self.e2) * R_M)
    }
}

impl CoarseState {
    fn face_xy(&self, d: Vec3) -> (usize, f64, f64) {
        let (f, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.sph.n as f64;
        (f.0 as usize, (s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5)
    }
    pub fn bilinear(&self, v: &[f32], d: Vec3) -> f64 {
        let (f, x, y) = self.face_xy(d);
        let n = self.sph.n as i64;
        let (xi, yi) = (x.floor(), y.floor());
        let (fx, fy) = (x - xi, y - yi);
        let at = |i: i64, j: i64| v[f * (n * n) as usize + (j.clamp(0, n - 1) * n + i.clamp(0, n - 1)) as usize] as f64;
        let (i, j) = (xi as i64, yi as i64);
        at(i, j) * (1.0 - fx) * (1.0 - fy) + at(i + 1, j) * fx * (1.0 - fy) + at(i, j + 1) * (1.0 - fx) * fy + at(i + 1, j + 1) * fx * fy
    }
}

// ---------------------------------------------------------------- the fine detail (SPIKE-32's ridged multifractal)

fn hash2(seed: u32, x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn gradient_dot(seed: u32, x: i32, y: i32, dx: f64, dy: f64) -> f64 {
    let a = f64::from(hash2(seed, x, y) & 0xffff) * (core::f64::consts::TAU / 65536.0);
    libm::cos(a) * dx + libm::sin(a) * dy
}
pub fn perlin(seed: u32, x: f64, y: f64) -> f64 {
    let (xf, yf) = (libm::floor(x), libm::floor(y));
    let (xi, yi) = (xf as i32, yf as i32);
    let (fx, fy) = (x - xf, y - yf);
    let (u, v) = (fade(fx), fade(fy));
    let n00 = gradient_dot(seed, xi, yi, fx, fy);
    let n10 = gradient_dot(seed, xi + 1, yi, fx - 1.0, fy);
    let n01 = gradient_dot(seed, xi, yi + 1, fx, fy - 1.0);
    let n11 = gradient_dot(seed, xi + 1, yi + 1, fx - 1.0, fy - 1.0);
    lerp(lerp(n00, n10, u), lerp(n01, n11, u), v) * 1.4142
}
pub fn ridged(seed: u32, x: f64, y: f64, base_freq: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut f, mut weight, mut norm) = (0.0, 1.0, base_freq, 1.0, 0.0);
    for o in 0..octaves {
        let n = perlin(seed.wrapping_add(o * 977), x * f + 31.7 * o as f64, y * f - 17.3 * o as f64);
        let r = 1.0 - n.abs();
        let v = r * r * weight;
        weight = (v * 1.6).clamp(0.0, 1.0);
        sum += v * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.07;
    }
    sum / norm
}

// ---------------------------------------------------------------- the planar fine solver

#[derive(Clone, Copy)]
pub struct Params {
    pub a_crit: f64,
    pub diff: f64,
    pub talus: f64,
}
pub const PARAMS: Params = Params { a_crit: 2500.0, diff: 0.005, talus: 0.9 };

pub struct Domain {
    pub w: usize,
    pub h: usize,
    pub ds: f64,
    pub z0: Vec<f64>,
    pub u: Vec<f64>,
    pub k: Vec<f64>,
    pub ring: Vec<bool>,
    pub inflow: Vec<f64>,
    pub corr: Vec<(f32, f32, f32)>,
}

pub struct Out {
    pub rec: Vec<u32>,
    pub z: Vec<f64>,
    pub a: Vec<f64>,
    pub ms: f64,
}

const NB: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// `steps` steps of uplift, flood, steepest receivers, implicit stream power, diffusion and thermal slumping on the rectangle.
/// The ring cells keep their height. Sequential and bit-exact: the order is the flood's (height, index) order.
pub fn evolve(d: &Domain, steps: u32, p: &Params, frozen: bool) -> Out {
    let t0 = Instant::now();
    let (w, h) = (d.w, d.h);
    let n = w * h;
    let mut z = d.z0.clone();
    let mut a = vec![0f64; n];
    let area0 = d.ds * d.ds;
    let eps = 1.0e-6;
    let mut filled = vec![0f64; n];
    let mut seen = vec![false; n];
    let mut rec = vec![u32::MAX; n];
    let mut order: Vec<u32> = Vec::with_capacity(n);
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let diag = d.ds * core::f64::consts::SQRT_2;
    for step in 0..steps {
        for c in 0..n {
            if !d.ring[c] {
                z[c] += d.u[c] * DT;
            }
        }
        if !(frozen && step > 0) {
        // flood from the ring (the ring is the outlet)
        filled.copy_from_slice(&z);
        seen.iter_mut().for_each(|s| *s = false);
        order.clear();
        heap.clear();
        for c in 0..n {
            if d.ring[c] {
                seen[c] = true;
                heap.push(Reverse((z[c].max(0.0).to_bits(), c as u32)));
            }
        }
        while let Some(Reverse((_, c))) = heap.pop() {
            let ci = c as usize;
            order.push(c);
            let (cx, cy) = ((ci % w) as i32, (ci / w) as i32);
            for (dx, dy) in NB {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let ni = ny as usize * w + nx as usize;
                if seen[ni] {
                    continue;
                }
                seen[ni] = true;
                if filled[ni] <= filled[ci] {
                    filled[ni] = filled[ci] + eps;
                }
                heap.push(Reverse((filled[ni].max(0.0).to_bits(), ni as u32)));
            }
        }
        // steepest receivers and the area
        for &c in &order {
            let ci = c as usize;
            let (cx, cy) = ((ci % w) as i32, (ci / w) as i32);
            let mut best = (0.0f64, u32::MAX);
            for (dx, dy) in NB {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let ni = ny as usize * w + nx as usize;
                let drop = filled[ci] - filled[ni];
                if drop > 0.0 {
                    let s = drop / if dx != 0 && dy != 0 { diag } else { d.ds };
                    if s > best.0 {
                        best = (s, ni as u32);
                    }
                }
            }
            rec[ci] = best.1;
            a[ci] = area0 + d.inflow[ci];
        }
        for &c in order.iter().rev() {
            let r = rec[c as usize];
            if r != u32::MAX {
                a[r as usize] += a[c as usize];
            }
        }
        }
        // implicit stream power in the order (receivers first)
        for &c in &order {
            let ci = c as usize;
            if d.ring[ci] {
                continue;
            }
            let r = rec[ci];
            if r == u32::MAX {
                continue;
            }
            let ri = r as usize;
            let dist = if (ci % w).abs_diff(ri % w) + (ci / w).abs_diff(ri / w) == 2 { diag } else { d.ds };
            let f = d.k[ci] * libm::pow((a[ci] - p.a_crit).max(0.0), M_EXP) * DT / dist;
            z[ci] = (z[ci] + f * z[ri]) / (1.0 + f);
        }
        // hillslope diffusion (explicit, stable) on the cells whose four neighbours exist
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
        // thermal slumping: a cell lowers by part of the excess over the talus angle towards each lower neighbour (ring cells never change)
        {
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
    }
    Out { rec, z, a, ms: t0.elapsed().as_secs_f64() * 1000.0 }
}

// ---------------------------------------------------------------- the site and the lattice

pub struct Site {
    pub fr: Frame,
    pub ds: f64,
    pub lw: usize,
    pub lat: f64,
    pub lon: f64,
    /// Lattice arrays over the whole truth rectangle (lw x lw cells, origin at (-half, -half)).
    pub z0: Vec<f64>,
    pub zc: Vec<f64>,
    pub u: Vec<f64>,
    pub k: Vec<f64>,
    pub a_c: Vec<f64>,
    pub detail_m: f64,
    pub segs: Vec<[f64; 4]>,
}

pub fn detail(x: f64, y: f64) -> f64 {
    ridged(77, x, y, 1.0 / 3000.0, 10) - 0.35
}

/// The best-relief inland coarse cell that is well inside a face.
pub fn find_site(cs: &CoarseState) -> (Vec3, f64, f64) {
    let n = cs.sph.n;
    let r = 3usize;
    let mut best = (0f32, 0usize);
    for f in 0..6 {
        for j in (n / 8..n - n / 8).step_by(2) {
            for i in (n / 8..n - n / 8).step_by(2) {
                let c = f * n * n + j * n + i;
                if cs.z[c] < 300.0 {
                    continue;
                }
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for dj in 0..=2 * r {
                    for di in 0..=2 * r {
                        let q = f * n * n + (j + dj - r) * n + (i + di - r);
                        lo = lo.min(cs.z[q]);
                        hi = hi.max(cs.z[q]);
                    }
                }
                if lo > 50.0 && hi - lo > best.0 {
                    best = (hi - lo, c);
                }
            }
        }
    }
    let (f, i, j) = cs.sph.split(best.1);
    let d = cs.sph.direction(f, i, j);
    let lat = libm::asin(d.z).to_degrees();
    let lon = libm::atan2(d.y, d.x).to_degrees();
    println!("SITE face {f} cell ({i},{j}) of {n}: lat {lat:.3} lon {lon:.3}, coarse relief {:.0} m in 7x7 cells, z {:.0} m", best.0, cs.z[best.1]);
    (d, lat, lon)
}

impl Site {
    pub fn build(cs: &CoarseState, c: Vec3, lat: f64, lon: f64, ds: f64, extent_m: f64, detail_m: f64, threads: usize) -> Site {
        let fr = Frame::new(c);
        let lw = (extent_m / ds).round() as usize;
        let half = extent_m / 2.0;
        let rows: Vec<(f64, f64, f64, f64, f64)> = par_collect(lw * lw, threads, |i| {
            let (x, y) = (((i % lw) as f64 + 0.5) * ds - half, ((i / lw) as f64 + 0.5) * ds - half);
            let dv = fr.dir(x, y);
            let zc = cs.bilinear(&cs.z, dv);
            let u = cs.bilinear(&cs.u, dv);
            let k = cs.bilinear(&cs.k, dv);
            (zc, zc + detail_m * detail(x, y), 0.0 * u, k, 0.0)
        });
        let mut s = Site {
            fr,
            ds,
            lw,
            lat,
            lon,
            z0: rows.iter().map(|r| r.1).collect(),
            zc: rows.iter().map(|r| r.0).collect(),
            u: rows.iter().map(|r| r.2).collect(),
            k: rows.iter().map(|r| r.3).collect(),
            a_c: vec![],
            detail_m,
            segs: vec![],
        };
        s.a_c = Vec::new();
        let _ = &mut s;
        s
    }
    pub fn half(&self) -> f64 {
        self.lw as f64 * self.ds / 2.0
    }
    /// Extract the rectangle [i0, i0+w) x [j0, j0+h) of the lattice as a domain, ring = its outermost cells, and the coarse inflow.
    pub fn domain(&self, cs: &CoarseState, i0: usize, j0: usize, w: usize, h: usize, with_inflow: bool) -> Domain {
        let mut dm = Domain { w, h, ds: self.ds, z0: vec![0.0; w * h], u: vec![0.0; w * h], k: vec![0.0; w * h], ring: vec![false; w * h], inflow: vec![0.0; w * h], corr: vec![] };
        for j in 0..h {
            for i in 0..w {
                let (c, s) = (j * w + i, (j0 + j) * self.lw + i0 + i);
                dm.z0[c] = self.z0[s];
                dm.u[c] = self.u[s];
                dm.k[c] = self.k[s];
                dm.ring[c] = i == 0 || j == 0 || i == w - 1 || j == h - 1;
            }
        }
        if with_inflow {
            self.inject(cs, &mut dm, i0, j0);
        }
        dm
    }
    /// The coarse flux that crosses the rectangle's border: a coarse cell centred outside whose receiver is centred inside gives its
    /// upstream area to the ring cells near the crossing, weighted by the valley depth (zmax - z + 1)^2 and a Gaussian of 0.5 coarse cells.
    fn inject(&self, cs: &CoarseState, dm: &mut Domain, i0: usize, j0: usize) {
        let half = self.half();
        let (x0, y0) = (-half + i0 as f64 * self.ds, -half + j0 as f64 * self.ds);
        let (x1, y1) = (x0 + dm.w as f64 * self.ds, y0 + dm.h as f64 * self.ds);
        let inside = |x: f64, y: f64| x >= x0 && x < x1 && y >= y0 && y < y1;
        let cell_m = 2.0 * core::f64::consts::PI * R_M / 4.0 / cs.sph.n as f64;
        let mut cells = std::collections::BTreeSet::new();
        let step = cell_m * 0.5;
        let (mut y, ymax) = (y0 - 2.0 * cell_m, y1 + 2.0 * cell_m);
        while y <= ymax {
            let mut x = x0 - 2.0 * cell_m;
            while x <= x1 + 2.0 * cell_m {
                cells.insert(cs.sph.cell_of(self.fr.dir(x, y)));
                x += step;
            }
            y += step;
        }
        let ringc: Vec<usize> = (0..dm.w * dm.h).filter(|&c| dm.ring[c]).collect();
        let zmax = ringc.iter().map(|&c| dm.z0[c]).fold(f64::MIN, f64::max);
        for &dcell in &cells {
            let (f, i, j) = cs.sph.split(dcell);
            let (dx, dy) = self.fr.xy(cs.sph.direction(f, i, j));
            if inside(dx, dy) {
                continue;
            }
            let r = cs.rec[dcell];
            if r == NONE {
                continue;
            }
            let (rf, ri, rj) = cs.sph.split(r as usize);
            let (rx, ry) = self.fr.xy(cs.sph.direction(rf, ri, rj));
            if !inside(rx, ry) {
                continue;
            }
            // the crossing: the point of the segment D -> R nearest the rectangle border (bisection on the segment)
            let (mut lo, mut hi) = (0.0f64, 1.0f64);
            for _ in 0..30 {
                let m = 0.5 * (lo + hi);
                if inside(dx + (rx - dx) * m, dy + (ry - dy) * m) {
                    hi = m;
                } else {
                    lo = m;
                }
            }
            let (px, py) = (dx + (rx - dx) * hi, dy + (ry - dy) * hi);
            let flux = cs.a_m2[dcell] as f64;
            let sig = 0.5 * cell_m;
            let mut ws = Vec::with_capacity(ringc.len());
            let mut tot = 0.0;
            for &c in &ringc {
                let (cx, cy) = (x0 + ((c % dm.w) as f64 + 0.5) * self.ds, y0 + ((c / dm.w) as f64 + 0.5) * self.ds);
                let d2 = (cx - px) * (cx - px) + (cy - py) * (cy - py);
                let g = libm::exp(-d2 / (2.0 * sig * sig));
                let v = (zmax - dm.z0[c] + 1.0) * (zmax - dm.z0[c] + 1.0);
                let wgt = g * v;
                tot += wgt;
                ws.push(wgt);
            }
            if tot > 0.0 {
                for (q, &c) in ringc.iter().enumerate() {
                    dm.inflow[c] += flux * ws[q] / tot;
                }
            }
        }
    }
}
