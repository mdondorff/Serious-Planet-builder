//! SPIKE-32 (spikes/realism): a 5 km alpine region for the realism prototype. NOT production code, never merged (the patch is kept in
//! `spikes/realism/results/`).
//!
//! Pieces, in the order the pipeline uses them:
//! 1. [`synthesize`]: a ridged multifractal massif inside an uplift envelope, then stream-power incision (implicit scheme of Braun and
//!    Willett, receivers from a priority flood), thermal erosion at a talus angle and a little hillslope diffusion, on a 4 m grid.
//! 2. [`build_materials`]: five material weights (rock, scree, snow, grass, soil) plus wetness, convexity and a snow potential per 2 m texel,
//!    from slope, curvature, aspect, flow area, altitude and noise.
//! 3. [`build_horizon`]: a terrain horizon map (sixteen azimuths, 4 m texels) for shadows from any sun direction.
//! 4. [`RegionStage`]: a [`HeightField`] that replaces the base heights inside the region (bicubic over the grid plus sub-grid rock detail).
//!
//! Spike code: platform maths is fine here (not a canonical field, no determinism claim).
#![allow(clippy::disallowed_methods, clippy::needless_range_loop, clippy::too_many_arguments)]

use crate::height::HeightField;
use planet_core::Vec3;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

/// Cells of the height grid per side (4 m cells): the region is 5,120 m wide.
pub const GRID_N: usize = 1280;
pub const CELL_M: f64 = 4.0;
pub const EXTENT_M: f64 = GRID_N as f64 * CELL_M;
/// The width over which the region blends into the base height outside its square.
pub const SKIRT_M: f64 = 700.0;
/// Texels of the material map per side (2 m texels).
pub const MAT_N: usize = 2560;
pub const MAT_TEXEL_M: f64 = EXTENT_M / MAT_N as f64;
/// Azimuths of the horizon map.
pub const HORIZON_AZ: usize = 16;
/// The horizon angle is stored over `[HORIZON_MIN_DEG, HORIZON_MIN_DEG + HORIZON_SPAN_DEG]`.
pub const HORIZON_MIN_DEG: f64 = -10.0;
pub const HORIZON_SPAN_DEG: f64 = 80.0;

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub seed: u32,
    /// Amplitude of the ridged multifractal before erosion (metres).
    pub relief_m: f64,
    /// 0: no domain warp.
    pub warp_m: f64,
    /// Fluvial steps (0: none) and `K dt` of the stream-power law (n = 1, m = 0.5), uplift per step at the middle of the massif (metres).
    pub fluvial_steps: u32,
    pub k_dt: f64,
    pub uplift_m: f64,
    /// Thermal erosion: passes and the talus angle in degrees (0 passes: none).
    pub thermal_steps: u32,
    pub talus_deg: f64,
    /// Hillslope diffusion passes (a light smoothing of the thermal result).
    pub diffuse_steps: u32,
    /// Share of the broad dome in the initial surface (the rest is the ridged multifractal).
    pub dome: f64,
    /// Contributing area (m2) below which stream power does not act, and the diffusion coefficient (per step, at most 0.25) of the slopes.
    pub a_crit: f64,
    pub diff_per_step: f64,
    /// Droplet erosion: droplets, their life in steps and the sediment capacity factor.
    pub droplets: u32,
    pub droplet_life: u32,
    pub capacity_k: f64,
    /// The scree map from the proximity of cliffs instead of the thermal deposit.
    pub scree_from_cliffs: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params { seed: 7, relief_m: 1500.0, warp_m: 500.0, fluvial_steps: 40, k_dt: 2.0e-3, uplift_m: 14.0, thermal_steps: 120, talus_deg: 38.0, diffuse_steps: 4, dome: 0.3, a_crit: 3000.0, diff_per_step: 0.12, droplets: 0, droplet_life: 250, capacity_k: 6.0, scree_from_cliffs: true }
    }
}

/// Cells of the grid a consumer may read: heights above the valley floor (metres), the flow area (cells) and the mass thermal erosion
/// deposited (metres).
pub struct Region {
    pub n: usize,
    pub cell: f64,
    pub h: Vec<f32>,
    pub area: Vec<f32>,
    pub deposit: Vec<f32>,
    pub max_h: f32,
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Noise

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
    let a = f64::from(hash2(seed, x, y) & 0xffff) * (std::f64::consts::TAU / 65536.0);
    a.cos() * dx + a.sin() * dy
}

/// Gradient noise in about [-1, 1].
pub fn perlin(seed: u32, x: f64, y: f64) -> f64 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i32, yf as i32);
    let (fx, fy) = (x - xf, y - yf);
    let (u, v) = (fade(fx), fade(fy));
    let n00 = gradient_dot(seed, xi, yi, fx, fy);
    let n10 = gradient_dot(seed, xi + 1, yi, fx - 1.0, fy);
    let n01 = gradient_dot(seed, xi, yi + 1, fx, fy - 1.0);
    let n11 = gradient_dot(seed, xi + 1, yi + 1, fx - 1.0, fy - 1.0);
    lerp(lerp(n00, n10, u), lerp(n01, n11, u), v) * 1.4142
}

/// Plain fractal noise in about [-1, 1]: `octaves` octaves of persistence 0.5 starting at `freq` cycles per metre.
pub fn fbm(seed: u32, x: f64, y: f64, freq: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 1.0, freq, 0.0);
    for o in 0..octaves {
        sum += amp * perlin(seed.wrapping_add(o * 101), x * f, y * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

/// Ridged multifractal (Musgrave): creased ridges whose detail grows where the lower octaves are high. About [0, 1].
fn ridged(seed: u32, x: f64, y: f64, base_freq: f64, octaves: u32) -> f64 {
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

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn par_rows<F: Fn(usize, &mut [T]) + Sync, T: Send>(data: &mut [T], row_len: usize, f: F) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(6);
    let rows = data.len() / row_len;
    let per = rows.div_ceil(threads);
    std::thread::scope(|s| {
        for (ci, chunk) in data.chunks_mut(per * row_len).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (ri, row) in chunk.chunks_mut(row_len).enumerate() {
                    f(ci * per + ri, row);
                }
            });
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Terrain synthesis

const NB: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// Priority flood with an epsilon (Barnes et al.): the visiting order (outlets first, every cell after its receiver) and the receiver of
/// every cell on the filled surface. The boundary cells are outlets.
fn route(h: &[f64], n: usize) -> (Vec<u32>, Vec<u32>) {
    let eps = 1.0e-4;
    let mut filled = h.to_vec();
    let mut seen = vec![false; n * n];
    let mut recv = vec![u32::MAX; n * n];
    let mut order = Vec::with_capacity(n * n);
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    for i in 0..n {
        for j in [0, n - 1] {
            for idx in [i * n + j, j * n + i] {
                if !seen[idx] {
                    seen[idx] = true;
                    recv[idx] = idx as u32;
                    heap.push(Reverse((filled[idx].max(0.0).to_bits(), idx as u32)));
                }
            }
        }
    }
    while let Some(Reverse((_, c))) = heap.pop() {
        let ci = c as usize;
        order.push(c);
        let (cx, cy) = ((ci % n) as i32, (ci / n) as i32);
        for (dx, dy) in NB {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                continue;
            }
            let ni = ny as usize * n + nx as usize;
            if seen[ni] {
                continue;
            }
            seen[ni] = true;
            recv[ni] = c;
            if filled[ni] <= filled[ci] {
                filled[ni] = filled[ci] + eps;
            }
            heap.push(Reverse((filled[ni].max(0.0).to_bits(), ni as u32)));
        }
    }
    // Steepest descent on the filled surface (the discoverer is the lowest neighbour, which lines the flow up with the grid diagonals).
    for &c in &order {
        let ci = c as usize;
        if recv[ci] == c {
            continue;
        }
        let (cx, cy) = ((ci % n) as i32, (ci / n) as i32);
        let mut best = (-1.0f64, recv[ci]);
        for (dx, dy) in NB {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                continue;
            }
            let ni = ny as usize * n + nx as usize;
            let drop = filled[ci] - filled[ni];
            if drop <= 0.0 {
                continue;
            }
            let s = drop / if dx != 0 && dy != 0 { std::f64::consts::SQRT_2 } else { 1.0 };
            if s > best.0 {
                best = (s, ni as u32);
            }
        }
        recv[ci] = best.1;
    }
    (order, recv)
}

fn flow_area(order: &[u32], recv: &[u32]) -> Vec<f64> {
    let mut area = vec![1.0f64; recv.len()];
    for &c in order.iter().rev() {
        let r = recv[c as usize];
        if r != c {
            area[r as usize] += area[c as usize];
        }
    }
    area
}

/// The uplift envelope: a round massif, 0 outside 2,500 m from the centre (the square's corners stay at the valley floor).
fn envelope(x: f64, y: f64) -> f64 {
    let half = EXTENT_M / 2.0;
    let d = (half - 60.0 - (x * x + y * y).sqrt()).max(0.0);
    smoothstep(0.0, 1500.0, d)
}

fn bilinear_h(h: &[f64], n: usize, x: f64, y: f64) -> (f64, f64, f64) {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let i = (yi as usize) * n + xi as usize;
    let (h00, h10, h01, h11) = (h[i], h[i + 1], h[i + n], h[i + n + 1]);
    let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;
    (h00 * (1.0 - fx) * (1.0 - fy) + h10 * fx * (1.0 - fy) + h01 * (1.0 - fx) * fy + h11 * fx * fy, gx, gy)
}

fn droplet_erosion(h: &mut [f64], n: usize, p: &Params) {
    let mut state = u64::from(p.seed) * 0x9e37_79b9_7f4a_7c15 + 1;
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let radius = 3i32;
    // Brush weights (cone) over the cells within `radius`.
    let mut brush: Vec<(i32, i32, f64)> = Vec::new();
    let mut total = 0.0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d = ((dx * dx + dy * dy) as f64).sqrt();
            if d <= f64::from(radius) {
                let w = 1.0 - d / (f64::from(radius) + 1.0);
                brush.push((dx, dy, w));
                total += w;
            }
        }
    }
    for b in &mut brush {
        b.2 /= total;
    }
    let (inertia, capacity_k, erode_k, deposit_k, evaporate, gravity) = (0.25, p.capacity_k, 0.3, 0.3, 0.012, 4.0);
    let hi = (n - 5) as f64;
    for _ in 0..p.droplets {
        let (mut x, mut y) = (4.0 + rnd() * (hi - 4.0), 4.0 + rnd() * (hi - 4.0));
        let (mut dx, mut dy, mut speed, mut water, mut sediment) = (0.0f64, 0.0f64, 1.0f64, 1.0f64, 0.0f64);
        for _ in 0..p.droplet_life {
            let (h0, gx, gy) = bilinear_h(h, n, x, y);
            dx = dx * inertia - gx * (1.0 - inertia);
            dy = dy * inertia - gy * (1.0 - inertia);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-9 {
                let a = rnd() * std::f64::consts::TAU;
                dx = a.cos();
                dy = a.sin();
            } else {
                dx /= len;
                dy /= len;
            }
            let (ox, oy) = (x, y);
            x += dx;
            y += dy;
            if x < 4.0 || y < 4.0 || x >= hi || y >= hi {
                break;
            }
            let (h1, _, _) = bilinear_h(h, n, x, y);
            let dh = h1 - h0;
            let capacity = (-dh).max(0.02) * speed * water * capacity_k;
            let (xi, yi) = (ox.floor(), oy.floor());
            let (fx, fy) = (ox - xi, oy - yi);
            let ci = (yi as usize) * n + xi as usize;
            if sediment > capacity || dh > 0.0 {
                let amount = if dh > 0.0 { dh.min(sediment) } else { (sediment - capacity) * deposit_k };
                sediment -= amount;
                h[ci] += amount * (1.0 - fx) * (1.0 - fy);
                h[ci + 1] += amount * fx * (1.0 - fy);
                h[ci + n] += amount * (1.0 - fx) * fy;
                h[ci + n + 1] += amount * fx * fy;
            } else {
                let amount = ((capacity - sediment) * erode_k).min(-dh);
                for &(bx, by, w) in &brush {
                    let q = ((yi as i32 + by) as usize) * n + (xi as i32 + bx) as usize;
                    let take = (amount * w).min(h[q].max(0.0));
                    h[q] -= take;
                    sediment += take;
                }
            }
            speed = (speed * speed - dh * gravity).max(0.0).sqrt();
            water *= 1.0 - evaporate;
        }
    }
}

/// Build the region's height grid.
pub fn synthesize(p: &Params) -> Region {
    let n = GRID_N;
    let half = EXTENT_M / 2.0;
    let pos = |i: usize| (i as f64 + 0.5) * CELL_M - half;
    let mut h = vec![0.0f64; n * n];
    // 1. Ridged multifractal inside the uplift envelope: wavelength 3 km down to about 12 m.
    {
        let mut tmp = vec![0.0f64; n * n];
        par_rows(&mut tmp, n, |j, row| {
            for i in 0..n {
                let (x, y) = (pos(i), pos(j));
                let env = envelope(x, y);
                let (wx, wy) = if p.warp_m > 0.0 {
                    (x + p.warp_m * fbm(p.seed + 11, x, y, 1.0 / 2400.0, 3), y + p.warp_m * fbm(p.seed + 12, x, y, 1.0 / 2400.0, 3))
                } else {
                    (x, y)
                };
                let r = ridged(p.seed, wx, wy, 1.0 / 3000.0, 10);
                // A broad dome so the massif has a middle, and gentle valley-floor relief outside.
                let rad = (x * x + y * y).sqrt() / half;
                let dome = (1.0 - rad * rad).max(0.0).powf(1.2);
                row[i] = env * p.relief_m * (p.dome * dome + (1.0 - p.dome) * r.powf(1.35));
            }
        });
        h.copy_from_slice(&tmp);
    }
    let mut deposit = vec![0.0f32; n * n];
    // 2. Stream-power incision with uplift (implicit: h_i = (h_i + F h_rec) / (1 + F), F = K dt A^0.5 / dist).
    let mut last_area = vec![1.0f64; n * n];
    for step in 0..p.fluvial_steps {
        for j in 0..n {
            for i in 0..n {
                let env = envelope(pos(i), pos(j));
                h[j * n + i] += p.uplift_m * env;
            }
        }
        if p.diff_per_step > 0.0 {
            let src = h.clone();
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let c = j * n + i;
                    h[c] = src[c] + p.diff_per_step * (src[c - 1] + src[c + 1] + src[c - n] + src[c + n] - 4.0 * src[c]);
                }
            }
        }
        let (order, recv) = route(&h, n);
        let area = flow_area(&order, &recv);
        for &c in &order {
            let ci = c as usize;
            let r = recv[ci] as usize;
            if r == ci {
                h[ci] = 0.0;
                continue;
            }
            let dist = if (ci % n).abs_diff(r % n) + (ci / n).abs_diff(r / n) == 2 { CELL_M * std::f64::consts::SQRT_2 } else { CELL_M };
            let f = p.k_dt * ((area[ci] * CELL_M * CELL_M) - p.a_crit).max(0.0).sqrt() / dist;
            h[ci] = (h[ci] + f * h[r]) / (1.0 + f);
        }
        if step + 1 == p.fluvial_steps {
            last_area = area;
        }
    }
    // 2b. Particle hydraulic erosion: droplets follow the gradient with inertia, pick up sediment on steep ground, drop it where they slow.
    if p.droplets > 0 {
        droplet_erosion(&mut h, n, p);
    }
    // 3. Thermal erosion at the talus angle: every neighbour below the angle takes a share of the excess; the receiving cells collect the
    //    scree map.
    let talus = p.talus_deg.to_radians().tan();
    for _ in 0..p.thermal_steps {
        let mut delta = vec![0.0f64; n * n];
        let mut dep = vec![0.0f32; n * n];
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let c = j * n + i;
                for (dx, dy) in NB {
                    let q = (j as i32 + dy) as usize * n + (i as i32 + dx) as usize;
                    let dist = if dx != 0 && dy != 0 { CELL_M * std::f64::consts::SQRT_2 } else { CELL_M };
                    let drop = (h[c] - h[q]) / dist;
                    if drop > talus {
                        let m = 0.09 * (drop - talus) * dist;
                        delta[c] -= m;
                        delta[q] += m;
                        dep[q] += m as f32;
                    }
                }
            }
        }
        for c in 0..n * n {
            h[c] += delta[c];
            deposit[c] += dep[c];
        }
    }
    // 4. Hillslope diffusion.
    for _ in 0..p.diffuse_steps {
        let src = h.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let c = j * n + i;
                let lap = src[c - 1] + src[c + 1] + src[c - n] + src[c + n] - 4.0 * src[c];
                h[c] = src[c] + 0.12 * lap;
            }
        }
    }
    // The scree map: where steep rock (over 45 degrees) lies within about 50 m, a box blur of the steep mask (scaled so `/ 2.5` maps it to 0..1).
    if p.scree_from_cliffs {
        let mut steep = vec![0.0f32; n * n];
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let c = j * n + i;
                let gx = (h[c + 1] - h[c - 1]) / (2.0 * CELL_M);
                let gy = (h[c + n] - h[c - n]) / (2.0 * CELL_M);
                steep[c] = if (gx * gx + gy * gy).sqrt() > 1.0 { 1.0 } else { 0.0 };
            }
        }
        let rad = 12usize;
        let mut tmp = vec![0.0f32; n * n];
        for j in 0..n {
            let mut acc = 0.0f32;
            for i in 0..n + rad {
                if i < n {
                    acc += steep[j * n + i];
                }
                if i >= 2 * rad + 1 {
                    acc -= steep[j * n + i - 2 * rad - 1];
                }
                if i >= rad {
                    tmp[j * n + i - rad] = acc / (2 * rad + 1) as f32;
                }
            }
        }
        for i in 0..n {
            let mut acc = 0.0f32;
            for j in 0..n + rad {
                if j < n {
                    acc += tmp[j * n + i];
                }
                if j >= 2 * rad + 1 {
                    acc -= tmp[(j - 2 * rad - 1) * n + i];
                }
                if j >= rad {
                    deposit[(j - rad) * n + i] = (acc / (2 * rad + 1) as f32 * 7.0).min(1.0) * 2.5;
                }
            }
        }
    }
    let max_h = h.iter().fold(0.0f64, |a, &b| a.max(b)) as f32;
    Region {
        n,
        cell: CELL_M,
        h: h.iter().map(|&v| v as f32).collect(),
        area: last_area.iter().map(|&v| v as f32).collect(),
        deposit,
        max_h,
    }
}

impl Region {
    fn at(&self, i: i32, j: i32) -> f64 {
        let n = self.n as i32;
        f64::from(self.h[(j.clamp(0, n - 1) * n + i.clamp(0, n - 1)) as usize])
    }

    fn at_f(&self, v: &[f32], i: i32, j: i32) -> f64 {
        let n = self.n as i32;
        f64::from(v[(j.clamp(0, n - 1) * n + i.clamp(0, n - 1)) as usize])
    }

    /// Height above the floor at (x, y) metres from the centre: Catmull-Rom over the grid (edge cells repeat).
    pub fn height(&self, x: f64, y: f64) -> f64 {
        let half = EXTENT_M / 2.0;
        let (u, v) = ((x + half) / self.cell - 0.5, (y + half) / self.cell - 0.5);
        let (iu, iv) = (u.floor(), v.floor());
        let (fu, fv) = (u - iu, v - iv);
        let (i0, j0) = (iu as i32, iv as i32);
        let cr = |p: [f64; 4], t: f64| {
            0.5 * ((2.0 * p[1]) + (-p[0] + p[2]) * t + (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]) * t * t + (-p[0] + 3.0 * p[1] - 3.0 * p[2] + p[3]) * t * t * t)
        };
        let mut col = [0.0; 4];
        for (k, c) in col.iter_mut().enumerate() {
            let j = j0 - 1 + k as i32;
            *c = cr([self.at(i0 - 1, j), self.at(i0, j), self.at(i0 + 1, j), self.at(i0 + 2, j)], fu);
        }
        cr(col, fv).max(0.0)
    }

    /// Bilinear value of one of the extra grids (`area`, `deposit`).
    pub fn bilinear(&self, v: &[f32], x: f64, y: f64) -> f64 {
        let half = EXTENT_M / 2.0;
        let (u, w) = ((x + half) / self.cell - 0.5, (y + half) / self.cell - 0.5);
        let (iu, iw) = (u.floor(), w.floor());
        let (fu, fw) = (u - iu, w - iw);
        let (i, j) = (iu as i32, iw as i32);
        lerp(
            lerp(self.at_f(v, i, j), self.at_f(v, i + 1, j), fu),
            lerp(self.at_f(v, i, j + 1), self.at_f(v, i + 1, j + 1), fu),
            fw,
        )
    }

    /// The gradient (dh/dx, dh/dy) from central differences of the interpolated height over `d` metres.
    pub fn gradient(&self, x: f64, y: f64, d: f64) -> (f64, f64) {
        (
            (self.height(x + d, y) - self.height(x - d, y)) / (2.0 * d),
            (self.height(x, y + d) - self.height(x, y - d)) / (2.0 * d),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Material map

/// Eight bytes per texel in two RGBA8 layers:
/// layer 0: rock, scree, snow, grass weights (sum with soil = 1);
/// layer 1: soil weight, wetness, convexity (0.5 = flat), snow potential.
pub struct Materials {
    pub n: usize,
    pub layer0: Vec<u8>,
    pub layer1: Vec<u8>,
}

/// Parameters of the rules (fractions of the massif's height are used instead of metres so the rules do not depend on the site).
#[derive(Clone, Copy, Debug)]
pub struct MaterialRules {
    pub snow_line: f64,
    pub snow_width: f64,
    pub grass_line: f64,
    pub snow_slope_lo_deg: f64,
    pub snow_slope_hi_deg: f64,
    pub rock_slope_lo_deg: f64,
    pub rock_slope_hi_deg: f64,
    pub hollow_gain: f64,
}

impl Default for MaterialRules {
    fn default() -> Self {
        MaterialRules {
            snow_line: 0.33,
            snow_width: 0.18,
            grass_line: 0.34,
            snow_slope_lo_deg: 34.0,
            snow_slope_hi_deg: 62.0,
            rock_slope_lo_deg: 40.0,
            rock_slope_hi_deg: 54.0,
            hollow_gain: 0.55,
        }
    }
}

pub fn build_materials(r: &Region, rules: &MaterialRules, seed: u32) -> Materials {
    let n = MAT_N;
    let half = EXTENT_M / 2.0;
    let top = f64::from(r.max_h).max(1.0);
    let mut layer0 = vec![0u8; n * n * 4];
    let mut layer1 = vec![0u8; n * n * 4];
    let mut both: Vec<[u8; 8]> = vec![[0; 8]; n * n];
    par_rows(&mut both, n, |j, row| {
        for i in 0..n {
            let x = (i as f64 + 0.5) * MAT_TEXEL_M - half;
            let y = (j as f64 + 0.5) * MAT_TEXEL_M - half;
            let hh = r.height(x, y);
            let a = hh / top;
            let (gx, gy) = r.gradient(x, y, 3.0);
            let g = (gx * gx + gy * gy).sqrt();
            let slope = g.atan().to_degrees();
            // Convexity over 12 m: positive on ridges and shoulders, negative in hollows.
            let d = 12.0;
            let lap = r.height(x - d, y) + r.height(x + d, y) + r.height(x, y - d) + r.height(x, y + d) - 4.0 * hh;
            let convex = (-lap / 5.0).clamp(-1.5, 1.5);
            let north = if g > 1e-4 { (-gy / g).clamp(-1.0, 1.0) } else { 0.0 };
            let area_m2 = r.bilinear(&r.area, x, y) * r.cell * r.cell;
            let wet = smoothstep(3.0, 6.2, area_m2.max(1.0).log10()) * (1.0 - smoothstep(14.0, 38.0, slope));
            let dep = (r.bilinear(&r.deposit, x, y) / 2.5).clamp(0.0, 1.0);
            let n1 = fbm(seed + 1, x, y, 1.0 / 90.0, 4);
            let n2 = fbm(seed + 2, x, y, 1.0 / 14.0, 3);
            let n3 = fbm(seed + 3, x, y, 1.0 / 3.5, 2);
            // Snow: above a noisy line, sticking on gentle slopes, deeper in hollows and on the shaded side.
            let line = rules.snow_line + 0.07 * n1 + 0.02 * n2;
            let pot = smoothstep(line - rules.snow_width * 0.5, line + rules.snow_width * 0.5, a);
            let stick = 1.0 - smoothstep(rules.snow_slope_lo_deg, rules.snow_slope_hi_deg, slope);
            let hollow = (0.5 - 0.5 * convex.clamp(-1.0, 1.0)).clamp(0.0, 1.0);
            let depth = pot * stick * (0.35 + rules.hollow_gain * hollow + 0.20 * north + 0.15 * n2);
            let snow = smoothstep(0.30, 0.55, depth + 0.10 * n3);
            // Rock: steep faces and convex ridges above the grass line.
            let steep = smoothstep(rules.rock_slope_lo_deg, rules.rock_slope_hi_deg, slope);
            let ridge = smoothstep(0.25, 0.9, convex) * smoothstep(rules.grass_line * 0.6, rules.grass_line * 1.2, a);
            let above = smoothstep(rules.grass_line, rules.grass_line + 0.18, a);
            let rock = (steep + ridge * 0.8 + above * 0.55 * (1.0 - stick * 0.6)).clamp(0.0, 1.0);
            // Scree: the talus band under the cliffs, where thermal erosion put its mass.
            let band = smoothstep(24.0, 30.0, slope) * (1.0 - smoothstep(38.0, 47.0, slope));
            let scree = (dep * band * 0.6).clamp(0.0, 1.0) * (1.0 - steep);
            // Grass below the grass line on gentle ground, more where it is wet.
            let low = 1.0 - smoothstep(rules.grass_line - 0.04, rules.grass_line + 0.10, a + 0.05 * n1);
            let mild = 1.0 - smoothstep(22.0, 36.0, slope);
            let grass = low * mild * (0.55 + 0.45 * wet + 0.2 * n2).clamp(0.0, 1.0);
            let soil = (low * (1.0 - grass) * (1.0 - steep) * 0.8 + wet * 0.3).clamp(0.0, 1.0) + 0.02;
            // Composite: snow on top, then rock, scree, grass, soil.
            let mut w = [rock, scree, 0.0, grass, soil];
            let sum_rest = (w[0] + w[1] + w[3] + w[4]).max(1e-4);
            let rest = 1.0 - snow;
            w[2] = snow;
            for k in [0usize, 1, 3, 4] {
                w[k] = w[k] / sum_rest * rest;
            }
            let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            row[i] = [q(w[0]), q(w[1]), q(w[2]), q(w[3]), q(w[4]), q(wet), q(0.5 + 0.5 * (convex / 1.5)), q(depth)];
        }
    });
    for (k, px) in both.iter().enumerate() {
        layer0[k * 4..k * 4 + 4].copy_from_slice(&px[..4]);
        layer1[k * 4..k * 4 + 4].copy_from_slice(&px[4..]);
    }
    Materials { n, layer0, layer1 }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Horizon map

/// The terrain horizon (elevation angle of the highest terrain along each azimuth) per grid texel: four RGBA8 layers, channel `c` of
/// layer `l` holds azimuth `4 l + c`, in `[0, 2 pi)` counter-clockwise from +x (east), as `(deg - HORIZON_MIN_DEG) / HORIZON_SPAN_DEG`.
pub struct Horizon {
    pub n: usize,
    pub layers: [Vec<u8>; 4],
}

pub fn build_horizon(r: &Region, max_dist_m: f64) -> Horizon {
    let n = r.n;
    let half = EXTENT_M / 2.0;
    let mut cells: Vec<[u8; HORIZON_AZ]> = vec![[0; HORIZON_AZ]; n * n];
    par_rows(&mut cells, n, |j, row| {
        for i in 0..n {
            let (x, y) = ((i as f64 + 0.5) * r.cell - half, (j as f64 + 0.5) * r.cell - half);
            let h0 = r.at(i as i32, j as i32) + 1.5;
            for a in 0..HORIZON_AZ {
                let ang = a as f64 * std::f64::consts::TAU / HORIZON_AZ as f64;
                let (dx, dy) = (ang.cos(), ang.sin());
                let mut t = r.cell * 1.5;
                let mut best = (-10.0f64).to_radians().tan();
                while t < max_dist_m {
                    let (qx, qy) = (x + dx * t, y + dy * t);
                    if qx.abs() > half || qy.abs() > half {
                        break;
                    }
                    let q = r.height(qx, qy);
                    best = best.max((q - h0) / t);
                    t = t * 1.045 + 2.0;
                }
                let deg = best.atan().to_degrees();
                row[i][a] = (((deg - HORIZON_MIN_DEG) / HORIZON_SPAN_DEG).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
    });
    let mut layers = [vec![0u8; n * n * 4], vec![0u8; n * n * 4], vec![0u8; n * n * 4], vec![0u8; n * n * 4]];
    for (k, c) in cells.iter().enumerate() {
        for a in 0..HORIZON_AZ {
            layers[a / 4][k * 4 + a % 4] = c[a];
        }
    }
    Horizon { n, layers }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Persistence: a cache file so a render run does not synthesise again.

pub struct Baked {
    pub params: Params,
    pub region: Region,
    pub materials: Materials,
    pub horizon: Horizon,
    pub seconds: [f64; 4],
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn f32_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

impl Baked {
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut out = b"RGN1".to_vec();
        put(&mut out, &f32_bytes(&self.region.h));
        put(&mut out, &f32_bytes(&self.region.area));
        put(&mut out, &f32_bytes(&self.region.deposit));
        put(&mut out, &self.materials.layer0);
        put(&mut out, &self.materials.layer1);
        for l in &self.horizon.layers {
            put(&mut out, l);
        }
        out.extend_from_slice(&f32_bytes(&self.seconds.map(|s| s as f32)));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, out)
    }

    pub fn load(path: &std::path::Path, params: Params) -> Option<Baked> {
        let b = std::fs::read(path).ok()?;
        if b.get(..4)? != b"RGN1" {
            return None;
        }
        let mut at = 4usize;
        let mut take = || -> Option<&[u8]> {
            let len = u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?) as usize;
            let s = b.get(at + 8..at + 8 + len)?;
            at += 8 + len;
            Some(s)
        };
        let h = f32_vec(take()?);
        let area = f32_vec(take()?);
        let deposit = f32_vec(take()?);
        let layer0 = take()?.to_vec();
        let layer1 = take()?.to_vec();
        let hl = [take()?.to_vec(), take()?.to_vec(), take()?.to_vec(), take()?.to_vec()];
        let secs = f32_vec(b.get(at..)?);
        let max_h = h.iter().fold(0.0f32, |a, &b| a.max(b));
        Some(Baked {
            params,
            region: Region { n: GRID_N, cell: CELL_M, h, area, deposit, max_h },
            materials: Materials { n: MAT_N, layer0, layer1 },
            horizon: Horizon { n: GRID_N, layers: hl },
            seconds: [f64::from(secs[0]), f64::from(secs[1]), f64::from(secs[2]), f64::from(secs[3])],
        })
    }
}

/// Build everything (timed per stage) from the parameters.
pub fn bake(p: &Params, rules: &MaterialRules) -> Baked {
    let t0 = std::time::Instant::now();
    let region = synthesize(p);
    let t1 = t0.elapsed().as_secs_f64();
    let materials = build_materials(&region, rules, p.seed);
    let t2 = t0.elapsed().as_secs_f64();
    let horizon = build_horizon(&region, 5000.0);
    let t3 = t0.elapsed().as_secs_f64();
    Baked { params: *p, region, materials, horizon, seconds: [t1, t2 - t1, t3 - t2, t3] }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The height field stage

/// Where the region sits on the planet.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub centre: Vec3,
    pub east: Vec3,
    pub north: Vec3,
    pub radius_m: f64,
    /// The height of the valley floor above the reference surface.
    pub floor_m: f64,
}

impl Site {
    pub fn new(centre: Vec3, radius_m: f64, floor_m: f64) -> Site {
        let c = centre.normalized();
        let helper = if c.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = helper.cross(c).normalized();
        let north = c.cross(east).normalized();
        Site { centre: c, east, north, radius_m, floor_m }
    }

    /// Gnomonic coordinates of a direction in the region's plane, metres from the centre.
    pub fn local(&self, dir: Vec3) -> Option<(f64, f64)> {
        let c = dir.dot(self.centre);
        if c <= 0.1 {
            return None;
        }
        Some((self.radius_m * dir.dot(self.east) / c, self.radius_m * dir.dot(self.north) / c))
    }

    /// The point at `(x, y)` metres in the plane (planet-fixed, unit direction).
    pub fn dir_at(&self, x: f64, y: f64) -> Vec3 {
        (self.centre + self.east * (x / self.radius_m) + self.north * (y / self.radius_m)).normalized()
    }
}

/// Sub-grid detail: rock-like ridged octaves from 4 m down to 25 cm whose amplitude follows the slope.
pub fn fine_detail(seed: u32, x: f64, y: f64, slope: f64, octaves: u32) -> f64 {
    if octaves == 0 {
        return 0.0;
    }
    let rocky = 0.25 + 0.75 * smoothstep(0.25, 0.9, slope);
    let (mut sum, mut amp, mut f) = (0.0, 0.55, 1.0 / 5.0);
    for o in 0..octaves {
        let n = perlin(seed.wrapping_add(500 + o * 31), x * f + 9.1 * o as f64, y * f + 4.7 * o as f64);
        sum += amp * (1.0 - 1.6 * n.abs());
        amp *= 0.5;
        f *= 2.1;
    }
    sum * rocky
}

pub struct RegionStage<'a> {
    base: Box<dyn HeightField + 'a>,
    pub site: Site,
    pub baked: Arc<Baked>,
    /// 0 disables the sub-grid detail; otherwise the number of octaves.
    pub detail_octaves: u32,
    inputs: u64,
}

impl<'a> RegionStage<'a> {
    pub fn new(base: Box<dyn HeightField + 'a>, site: Site, baked: Arc<Baked>, detail_octaves: u32) -> RegionStage<'a> {
        let inputs = planet_core::hash::hash_u64s(0x7265_6733, &[base.inputs_hash(), u64::from(baked.params.seed), u64::from(detail_octaves)]);
        RegionStage { base, site, baked, detail_octaves, inputs }
    }

    fn blend(&self, dir: Vec3, base_h: f64) -> f64 {
        let Some((x, y)) = self.site.local(dir) else { return base_h };
        let half = EXTENT_M / 2.0;
        let out = (x.abs() - half).max(y.abs() - half);
        if out >= SKIRT_M {
            return base_h;
        }
        let w = 1.0 - smoothstep(0.0, SKIRT_M, out);
        let r = &self.baked.region;
        let rel = r.height(x, y);
        let detail = if self.detail_octaves > 0 && w > 0.0 {
            let (gx, gy) = r.gradient(x, y, 2.0);
            fine_detail(self.baked.params.seed, x, y, (gx * gx + gy * gy).sqrt(), self.detail_octaves)
        } else {
            0.0
        };
        lerp(base_h, self.site.floor_m + rel + detail, w)
    }

    /// Whether a cap touches the region's square plus the skirt.
    fn cap_touches(&self, centre: Vec3, angular_radius: f64) -> bool {
        let reach = (EXTENT_M / 2.0 + SKIRT_M) * std::f64::consts::SQRT_2 / self.site.radius_m;
        let ang = centre.dot(self.site.centre).clamp(-1.0, 1.0).acos();
        ang < reach + angular_radius
    }
}

impl HeightField for RegionStage<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        self.blend(dir, self.base.height(dir))
    }
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        let b = self.base.heights_until(dirs, stop);
        dirs.iter().zip(b).map(|(&d, h)| self.blend(d, h)).collect()
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        let b = self.base.heights_threaded_until(dirs, stop, threads);
        dirs.iter().zip(b).map(|(&d, h)| self.blend(d, h)).collect()
    }
    fn seed(&self) -> u64 {
        self.base.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "spike-realism-region"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m().max(self.site.floor_m.abs() + f64::from(self.baked.region.max_h) + 40.0)
    }
    fn inputs_hash(&self) -> u64 {
        self.inputs
    }
    fn read_records(&self) -> Vec<planet_world_def::ReadRecord> {
        self.base.read_records()
    }
    fn body(&self) -> planet_world_def::BodyId {
        self.base.body()
    }
    fn surface_id(&self) -> &'static str {
        self.base.surface_id()
    }
    fn climate(&self, map: &dyn planet_core::cube::FaceMapping, id: planet_core::TileId) -> crate::climate::ClimateTexture {
        self.base.climate(map, id)
    }
    fn climate_version_word(&self) -> u64 {
        self.base.climate_version_word()
    }
    fn heights_and_climate(
        &self,
        map: &dyn planet_core::cube::FaceMapping,
        id: planet_core::TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, crate::climate::ClimateTexture) {
        let (b, c) = self.base.heights_and_climate(map, id, dirs, stop);
        (dirs.iter().zip(b).map(|(&d, h)| self.blend(d, h)).collect(), c)
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.base.height_bounds(centre, angular_radius);
        if self.cap_touches(centre, angular_radius) {
            let top = self.site.floor_m + f64::from(self.baked.region.max_h) + 40.0;
            (lo.min(self.site.floor_m - 5.0), hi.max(top))
        } else {
            (lo, hi)
        }
    }
    fn bounds_monotone(&self) -> bool {
        false
    }
}
