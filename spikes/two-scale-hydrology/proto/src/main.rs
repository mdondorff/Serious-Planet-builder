//! SPIKE-30 (D1): driver-maps measurements. Throwaway prototype.
//!
//! Flow accumulation (priority-flood + multiple-flow-direction on the cube sphere, across face edges), soil depth, a
//! pine suitability rule, a regional refinement variant, all on the existing global-coarse world.
#![allow(clippy::too_many_arguments)]
mod twoscale;

use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{bake as global_bake, sample, GridSource};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, WorldDefinition};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

const R_M: f64 = 6_371_000.0;
const NONE: u32 = u32::MAX;
/// Orthogonal neighbours first (W, E, S, N), then the diagonals.
const OFFS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
static SEED_CELL: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
fn seed() -> u64 {
    *SEED_CELL.get_or_init(|| {
        let def = WorldDefinition::single_planet(R_M, 42);
        GenContext::new(&def).body_seed(BodyId::PLANET).expect("seed")
    })
}

// ---------------------------------------------------------------- platform

#[repr(C)]
struct ProcMem {
    cb: u32,
    faults: u32,
    peak_ws: usize,
    ws: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    pagefile: usize,
    peak_pagefile: usize,
}
extern "system" {
    fn GetCurrentProcess() -> isize;
    fn SetPriorityClass(h: isize, class: u32) -> i32;
    fn K32GetProcessMemoryInfo(h: isize, m: *mut ProcMem, cb: u32) -> i32;
}
fn below_normal() {
    unsafe {
        SetPriorityClass(GetCurrentProcess(), 0x4000);
    }
}
fn peak_ws_mb() -> f64 {
    let mut m = ProcMem { cb: std::mem::size_of::<ProcMem>() as u32, faults: 0, peak_ws: 0, ws: 0, a: 0, b: 0, c: 0, d: 0, pagefile: 0, peak_pagefile: 0 };
    unsafe {
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb);
    }
    m.peak_ws as f64 / 1048576.0
}

// ---------------------------------------------------------------- topology

trait Topo: Sync {
    fn len(&self) -> usize;
    fn nbrs(&self, c: usize) -> [u32; 8];
    /// Cell edge length in metres (square root of the area).
    fn h(&self, c: usize) -> f32;
    fn area_km2(&self, c: usize) -> f32;
    /// Distance in metres from `c` to its neighbour `n` (index `k` in [`OFFS`]); the default models it from the cell sizes.
    fn dist(&self, c: usize, k: usize, n: usize) -> f32 {
        0.5 * (self.h(c) + self.h(n)) * if k < 4 { 1.0 } else { core::f32::consts::SQRT_2 }
    }
}

fn solid_f(u: f64, v: f64) -> f64 {
    libm::atan2(u * v, libm::sqrt(1.0 + u * u + v * v))
}
fn plane_u(s: f64) -> f64 {
    libm::tan(s * core::f64::consts::FRAC_PI_4)
}

/// The cube sphere of `n` cells per face edge, cell-centred, tangent warp, neighbours across face edges through the direction.
struct Sphere {
    n: usize,
    area: Vec<f32>, // km2, one face (all faces are equal)
    exact: bool,    // neighbour distances from the directions (diagnostic)
}
impl Sphere {
    fn new(n: usize) -> Self {
        let u: Vec<f64> = (0..=n).map(|k| plane_u(-1.0 + 2.0 * k as f64 / n as f64)).collect();
        let r_km = R_M / 1000.0;
        let mut area = vec![0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                let o = solid_f(u[i + 1], u[j + 1]) - solid_f(u[i], u[j + 1]) - solid_f(u[i + 1], u[j]) + solid_f(u[i], u[j]);
                area[j * n + i] = (o * r_km * r_km) as f32;
            }
        }
        Sphere { n, area, exact: false }
    }
    fn split(&self, c: usize) -> (usize, usize, usize) {
        let n2 = self.n * self.n;
        let r = c % n2;
        (c / n2, r % self.n, r / self.n)
    }
    fn direction(&self, f: usize, i: usize, j: usize) -> Vec3 {
        let n = self.n as f64;
        face_to_direction(&TangentWarp, Face(f as u8), -1.0 + (2.0 * i as f64 + 1.0) / n, -1.0 + (2.0 * j as f64 + 1.0) / n)
    }
    fn cell_of(&self, d: Vec3) -> usize {
        let (f, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.n as i64;
        let i = (((s + 1.0) * 0.5 * n as f64) as i64).clamp(0, n - 1);
        let j = (((t + 1.0) * 0.5 * n as f64) as i64).clamp(0, n - 1);
        (f.0 as usize) * self.n * self.n + (j as usize) * self.n + i as usize
    }
}
impl Topo for Sphere {
    fn len(&self) -> usize {
        6 * self.n * self.n
    }
    fn nbrs(&self, c: usize) -> [u32; 8] {
        let (f, i, j) = self.split(c);
        let n = self.n as i64;
        let n2 = self.n * self.n;
        let mut out = [NONE; 8];
        for (k, &(di, dj)) in OFFS.iter().enumerate() {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            out[k] = if ni >= 0 && ni < n && nj >= 0 && nj < n {
                (f * n2 + (nj as usize) * self.n + ni as usize) as u32
            } else {
                let s = -1.0 + (2.0 * ni as f64 + 1.0) / n as f64;
                let t = -1.0 + (2.0 * nj as f64 + 1.0) / n as f64;
                let d = face_to_direction(&TangentWarp, Face(f as u8), s, t);
                self.cell_of(d) as u32
            };
        }
        out
    }
    fn h(&self, c: usize) -> f32 {
        (self.area_km2(c).sqrt()) * 1000.0
    }
    fn area_km2(&self, c: usize) -> f32 {
        self.area[c % (self.n * self.n)]
    }
    fn dist(&self, c: usize, k: usize, n: usize) -> f32 {
        if self.exact {
            let (f, i, j) = self.split(c);
            let (f2, i2, j2) = self.split(n);
            let a = self.direction(f, i, j);
            let b = self.direction(f2, i2, j2);
            let _ = k;
            return (a.distance(b) * R_M) as f32;
        }
        0.5 * (self.h(c) + self.h(n)) * if k < 4 { 1.0 } else { core::f32::consts::SQRT_2 }
    }
}

/// A window of a face of the (finer) cube sphere; its border cells are outlets.
struct Patch {
    n_fine: usize,
    face: usize,
    i0: usize,
    j0: usize,
    w: usize,
    hh: usize,
    area: Vec<f32>,
}
impl Patch {
    fn new(n_fine: usize, face: usize, i0: usize, j0: usize, w: usize, hh: usize) -> Self {
        let r_km = R_M / 1000.0;
        let u: Vec<f64> = (0..=w).map(|k| plane_u(-1.0 + 2.0 * (i0 + k) as f64 / n_fine as f64)).collect();
        let v: Vec<f64> = (0..=hh).map(|k| plane_u(-1.0 + 2.0 * (j0 + k) as f64 / n_fine as f64)).collect();
        let mut area = vec![0f32; w * hh];
        for j in 0..hh {
            for i in 0..w {
                let o = solid_f(u[i + 1], v[j + 1]) - solid_f(u[i], v[j + 1]) - solid_f(u[i + 1], v[j]) + solid_f(u[i], v[j]);
                area[j * w + i] = (o * r_km * r_km) as f32;
            }
        }
        Patch { n_fine, face, i0, j0, w, hh, area }
    }
    fn direction(&self, i: usize, j: usize) -> Vec3 {
        let n = self.n_fine as f64;
        face_to_direction(
            &TangentWarp,
            Face(self.face as u8),
            -1.0 + (2.0 * (self.i0 + i) as f64 + 1.0) / n,
            -1.0 + (2.0 * (self.j0 + j) as f64 + 1.0) / n,
        )
    }
}
impl Topo for Patch {
    fn len(&self) -> usize {
        self.w * self.hh
    }
    fn nbrs(&self, c: usize) -> [u32; 8] {
        let (i, j) = ((c % self.w) as i64, (c / self.w) as i64);
        let mut out = [NONE; 8];
        for (k, &(di, dj)) in OFFS.iter().enumerate() {
            let (ni, nj) = (i + di, j + dj);
            if ni >= 0 && ni < self.w as i64 && nj >= 0 && nj < self.hh as i64 {
                out[k] = (nj as usize * self.w + ni as usize) as u32;
            }
        }
        out
    }
    fn h(&self, c: usize) -> f32 {
        self.area[c].sqrt() * 1000.0
    }
    fn area_km2(&self, c: usize) -> f32 {
        self.area[c]
    }
}

// ---------------------------------------------------------------- noise (lithology stand-in)

fn h3(x: i64, y: i64, z: i64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9) ^ seed();
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}
fn vnoise(p: Vec3) -> f64 {
    let (fx, fy, fz) = (libm::floor(p.x), libm::floor(p.y), libm::floor(p.z));
    let (ix, iy, iz) = (fx as i64, fy as i64, fz as i64);
    let sm = |t: f64| t * t * (3.0 - 2.0 * t);
    let (tx, ty, tz) = (sm(p.x - fx), sm(p.y - fy), sm(p.z - fz));
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = (if dx == 1 { tx } else { 1.0 - tx }) * (if dy == 1 { ty } else { 1.0 - ty }) * (if dz == 1 { tz } else { 1.0 - tz });
                acc += w * h3(ix + dx, iy + dy, iz + dz);
            }
        }
    }
    acc
}
/// Rock hardness in 0..1 (noise bands).
fn lithology(d: Vec3) -> f32 {
    (0.6 * vnoise(d * 4.0) + 0.4 * vnoise(d * 11.0)) as f32
}

// ---------------------------------------------------------------- the pipeline

struct Fields {
    z: Vec<f32>,
    lith: Vec<u8>,
}

fn run_parallel<F: Fn(usize) -> (f32, u8) + Sync>(len: usize, threads: usize, f: F) -> Fields {
    let mut z = vec![0f32; len];
    let mut lith = vec![0u8; len];
    let chunk = len.div_ceil(threads.max(1));
    std::thread::scope(|sc| {
        for (ci, (zs, ls)) in z.chunks_mut(chunk).zip(lith.chunks_mut(chunk)).enumerate() {
            let f = &f;
            sc.spawn(move || {
                for (k, (zo, lo)) in zs.iter_mut().zip(ls.iter_mut()).enumerate() {
                    let (a, b) = f(ci * chunk + k);
                    *zo = a;
                    *lo = b;
                }
            });
        }
    });
    Fields { z, lith }
}

fn sample_sphere(s: &Sphere, threads: usize) -> Fields {
    run_parallel(s.len(), threads, |c| {
        let (f, i, j) = s.split(c);
        let d = s.direction(f, i, j);
        (sample(seed(), d).elevation_m, (lithology(d) * 255.0) as u8)
    })
}
fn sample_patch(p: &Patch, threads: usize) -> Fields {
    run_parallel(p.len(), threads, |c| {
        let d = p.direction(c % p.w, c / p.w);
        (sample(seed(), d).elevation_m, (lithology(d) * 255.0) as u8)
    })
}
/// The world's eroded height (ADR 0030 point 1) over `field`, at the patch's cells.
fn sample_patch_eroded(p: &Patch, field: &dyn HeightField, threads: usize) -> Fields {
    run_parallel(p.len(), threads, |c| {
        let d = p.direction(c % p.w, c / p.w);
        (field.height(d) as f32, (lithology(d) * 255.0) as u8)
    })
}

struct FloodStats {
    seeds: usize,
    raised: usize,
    unreached: usize,
    heap_peak: usize,
    land: usize,
}

/// Priority-flood with epsilon (Barnes 2014) from the coast (and the border of a patch). Returns the filled surface and
/// the pop order (ascending), the order accumulation walks backwards.
fn priority_flood<T: Topo>(t: &T, z: &[f32]) -> (Vec<f32>, Vec<u32>, FloodStats) {
    let len = t.len();
    let mut filled = z.to_vec();
    let mut visited = vec![0u64; len.div_ceil(64)];
    let get = |v: &Vec<u64>, c: usize| v[c >> 6] >> (c & 63) & 1 == 1;
    let set = |v: &mut Vec<u64>, c: usize| v[c >> 6] |= 1u64 << (c & 63);
    let mut heap: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
    let mut land = 0usize;
    let mut seeds = 0usize;
    for c in 0..len {
        if z[c] <= 0.0 {
            set(&mut visited, c);
        }
    }
    for c in 0..len {
        if z[c] <= 0.0 {
            continue;
        }
        land += 1;
        let nb = t.nbrs(c);
        if nb.iter().any(|&n| n == NONE || z[n as usize] <= 0.0) {
            set(&mut visited, c);
            heap.push(Reverse(((z[c].to_bits() as u64) << 32) | c as u64));
            seeds += 1;
        }
    }
    let mut order: Vec<u32> = Vec::with_capacity(land);
    let (mut raised, mut heap_peak) = (0usize, heap.len());
    while let Some(Reverse(key)) = heap.pop() {
        let c = (key & 0xffff_ffff) as usize;
        order.push(c as u32);
        let fc = filled[c];
        for &n in t.nbrs(c).iter() {
            if n == NONE {
                continue;
            }
            let n = n as usize;
            if get(&visited, n) {
                continue;
            }
            set(&mut visited, n);
            if z[n] <= fc {
                filled[n] = f32::from_bits(fc.to_bits() + 1);
                raised += 1;
            }
            heap.push(Reverse(((filled[n].to_bits() as u64) << 32) | n as u64));
        }
        heap_peak = heap_peak.max(heap.len());
    }
    // Land cells the flood never reached (a one-way neighbour relation across a face edge): kept at their height, processed first.
    let mut unreached = 0usize;
    if order.len() < land {
        let mut seen = vec![false; len];
        for &c in &order {
            seen[c as usize] = true;
        }
        let mut extra: Vec<u32> = (0..len).filter(|&c| z[c] > 0.0 && !seen[c]).map(|c| c as u32).collect();
        unreached = extra.len();
        extra.extend(order);
        order = extra;
    }
    (filled, order, FloodStats { seeds, raised, unreached, heap_peak, land })
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Mfd,
    D8,
}

/// Upstream area in km2 per cell by walking the pop order backwards.
fn accumulate<T: Topo>(t: &T, filled: &[f32], order: &[u32], route: Route, inflow: Option<&[f32]>) -> Vec<f32> {
    let len = t.len();
    let mut a = vec![0f32; len];
    for &c in order {
        a[c as usize] = t.area_km2(c as usize) + inflow.map_or(0.0, |v| v[c as usize]);
    }
    let sq2 = core::f32::consts::SQRT_2;
    for &c in order.iter().rev() {
        let c = c as usize;
        let nb = t.nbrs(c);
        let (fc, hc) = (filled[c], t.h(c));
        let mut w = [0f32; 8];
        let mut sum = 0f32;
        let mut best = (0f32, 8usize);
        for k in 0..8 {
            if nb[k] == NONE {
                continue;
            }
            let n = nb[k] as usize;
            let fnn = filled[n];
            if fnn < fc {
                let d = t.dist(c, k, n);
                let s = (fc - fnn) / d;
                match route {
                    Route::Mfd => {
                        w[k] = s.powf(1.1);
                        sum += w[k];
                    }
                    Route::D8 => {
                        if s > best.0 {
                            best = (s, k);
                        }
                    }
                }
            }
        }
        let ac = a[c];
        match route {
            Route::Mfd => {
                if sum > 0.0 {
                    for k in 0..8 {
                        if w[k] > 0.0 {
                            a[nb[k] as usize] += ac * w[k] / sum;
                        }
                    }
                }
            }
            Route::D8 => {
                if best.1 < 8 {
                    a[nb[best.1] as usize] += ac;
                }
            }
        }
    }
    a
}

struct Derived {
    soil: Vec<u8>,  // sqrt-coded 0..8 m
    slope: Vec<u8>, // 0..90 degrees
    twi: Vec<u8>,   // 0..16
}
fn soil_decode(q: u8) -> f32 {
    let t = q as f32 / 255.0;
    t * t * 8.0
}
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Slope, topographic wetness index and soil depth from the heights, the accumulation and the lithology.
fn derive<T: Topo>(t: &T, z: &[f32], a: &[f32], lith: &[u8]) -> Derived {
    let len = t.len();
    let mut d = Derived { soil: vec![0; len], slope: vec![0; len], twi: vec![0; len] };
    for c in 0..len {
        if z[c] <= 0.0 {
            continue;
        }
        let nb = t.nbrs(c);
        let g = |k: usize| if nb[k] == NONE { z[c] } else { z[nb[k] as usize] };
        let (zw, ze, zs, zn) = (g(0), g(1), g(2), g(3));
        let h = t.h(c).max(1.0);
        let (dx, dy) = ((ze - zw) / (2.0 * h), (zn - zs) / (2.0 * h));
        let slope = (dx * dx + dy * dy).sqrt().atan();
        let slope_deg = slope.to_degrees();
        let lap = (zw + ze + zs + zn - 4.0 * z[c]) / (4.0 * h);
        let tan_b = slope.tan().max(0.01);
        let twi = ((a[c] * 1.0e6 / h).max(1.0) / tan_b).ln();
        let f_slope = (-slope_deg / 18.0).exp();
        let f_curv = 1.0 + 0.5 * (5.0 * lap).tanh();
        let f_flow = 0.5 + 0.5 * ((1.0 + a[c] / 10.0).log10() / 3.0).clamp(0.0, 1.0);
        let f_lith = 0.5 + lith[c] as f32 / 255.0;
        let f_alt = 1.0 - smoothstep(2500.0, 4500.0, z[c]);
        let depth = (2.5 * f_slope * f_curv * f_flow * f_lith * f_alt).clamp(0.0, 8.0);
        d.soil[c] = ((depth / 8.0).sqrt() * 255.0 + 0.5) as u8;
        d.slope[c] = (slope_deg / 90.0 * 255.0).clamp(0.0, 255.0) as u8;
        d.twi[c] = (twi / 16.0 * 255.0).clamp(0.0, 255.0) as u8;
    }
    d
}

// ---------------------------------------------------------------- suitability

fn ss(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}
/// Trapezoid response: 0 at a, 1 from b to c, 0 at d; NaN edges leave that side open.
fn trap(x: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    let rise = if a.is_nan() || b.is_nan() { 1.0 } else if x <= a { 0.0 } else if x >= b { 1.0 } else { ss((x - a) / (b - a)) };
    let fall = if c.is_nan() || d.is_nan() { 1.0 } else if x >= d { 0.0 } else if x <= c { 1.0 } else { 1.0 - ss((x - c) / (d - c)) };
    rise * fall
}
/// Moisture index 0..1: precipitation and the wetness index (stand-in for the real driver).
fn moisture(precip_mm: f32, twi: f32) -> f32 {
    0.5 * (precip_mm / 2200.0).clamp(0.0, 1.0) + 0.5 * ((twi - 3.0) / 9.0).clamp(0.0, 1.0)
}
const N: f32 = f32::NAN;
/// The pine of the briefing: soil depth, moisture, temperature, slope, altitude above the tree line.
fn pine(soil_m: f32, moist: f32, temp_c: f32, slope_deg: f32, above_treeline_m: f32) -> f32 {
    trap(soil_m, 0.1, 0.4, 3.0, N) * trap(moist, 0.2, 0.35, 0.7, 0.85) * trap(temp_c, -12.0, -4.0, 12.0, 18.0) * trap(slope_deg, N, 0.0, 30.0, 45.0) * trap(above_treeline_m, N, N, -200.0, 0.0)
}
/// Altitude above the tree line, where the sea-level-reduced temperature minus 6.5 C/km of height reaches -4 C.
fn above_treeline(elev_m: f32, temp_c: f32) -> f32 {
    let t_sea = temp_c + 6.5 * elev_m.max(0.0) / 1000.0;
    elev_m - (t_sea + 4.0) / 6.5 * 1000.0
}

// ---------------------------------------------------------------- images

fn save_png(path: &str, w: u32, h: u32, rgb: &[u8]) {
    let f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut e = png::Encoder::new(f, w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.set_compression(png::Compression::High);
    e.write_header().unwrap().write_image_data(rgb).unwrap();
    let kb = std::fs::metadata(path).unwrap().len() / 1024;
    println!("wrote {path} ({w}x{h}, {kb} KB)");
}
fn ramp(t: f32) -> [u8; 3] {
    // dark blue, teal, green, yellow, white
    const S: [[f32; 3]; 5] = [[20.0, 30.0, 90.0], [20.0, 120.0, 130.0], [40.0, 160.0, 60.0], [230.0, 210.0, 50.0], [255.0, 250.0, 240.0]];
    let x = t.clamp(0.0, 1.0) * 4.0;
    let k = (x as usize).min(3);
    let f = x - k as f32;
    let m = |c: usize| (S[k][c] * (1.0 - f) + S[k + 1][c] * f) as u8;
    [m(0), m(1), m(2)]
}

// ---------------------------------------------------------------- main

fn arg<T: std::str::FromStr>(a: &[String], i: usize, d: T) -> T {
    a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d)
}

struct Bake {
    sph: Sphere,
    z: Vec<f32>,
    lith: Vec<u8>,
    a: Vec<f32>,
    dv: Derived,
}

fn bake(n: usize, threads: usize, tag: &str) -> Bake {
    let t0 = Instant::now();
    let sph = Sphere::new(n);
    let t_topo = t0.elapsed();
    let t1 = Instant::now();
    let Fields { z, lith } = sample_sphere(&sph, threads);
    let t_sample = t1.elapsed();
    let t2 = Instant::now();
    let (filled, order, fs) = priority_flood(&sph, &z);
    let t_flood = t2.elapsed();
    let t3 = Instant::now();
    let a = accumulate(&sph, &filled, &order, Route::Mfd, None);
    let t_acc = t3.elapsed();
    let t4 = Instant::now();
    let dv = derive(&sph, &z, &a, &lith);
    let t_derive = t4.elapsed();
    let cells = sph.len();
    println!(
        "BAKE {tag} n={n} cells={cells} threads={threads} | topo {:.2}s sample {:.2}s ({:.2} us/cell/thread) flood {:.2}s accumulate(MFD) {:.2}s derive(slope,twi,soil) {:.2}s | land {} ({:.1} %) seeds {} raised {} unreached {} heap_peak {} | peak working set {:.0} MB",
        t_topo.as_secs_f64(),
        t_sample.as_secs_f64(),
        t_sample.as_secs_f64() * threads as f64 / cells as f64 * 1e6,
        t_flood.as_secs_f64(),
        t_acc.as_secs_f64(),
        t_derive.as_secs_f64(),
        fs.land,
        100.0 * fs.land as f64 / cells as f64,
        fs.seeds,
        fs.raised,
        fs.unreached,
        fs.heap_peak,
        peak_ws_mb()
    );
    println!(
        "BAKE-SINGLE-THREAD-EQUIV {tag} n={n}: flood+accumulate+derive {:.2}s = {:.1} ns/cell (all sequential); with sampling {:.2}s",
        (t_flood + t_acc + t_derive).as_secs_f64(),
        (t_flood + t_acc + t_derive).as_secs_f64() / cells as f64 * 1e9,
        (t_flood + t_acc + t_derive).as_secs_f64() + t_sample.as_secs_f64() * threads as f64
    );
    Bake { sph, z, lith, a, dv }
}

/// Routing comparison: single direction against multiple flow direction, plus the across-edge gradient check of ADR 0030 point 2.
fn compare(n: usize, threads: usize, out: &str) {
    let sph = Sphere::new(n);
    let Fields { z, lith: _ } = sample_sphere(&sph, threads);
    let (filled, order, _) = priority_flood(&sph, &z);
    let t = Instant::now();
    let a_d8 = accumulate(&sph, &filled, &order, Route::D8, None);
    let t_d8 = t.elapsed();
    let t = Instant::now();
    let a_mfd = accumulate(&sph, &filled, &order, Route::Mfd, None);
    let t_mfd = t.elapsed();
    println!("ROUTING n={n}: D8 accumulate {:.2}s, MFD accumulate {:.2}s", t_d8.as_secs_f64(), t_mfd.as_secs_f64());
    for (name, a) in [("D8", &a_d8), ("MFD", &a_mfd)] {
        // mean |ln A_c - ln A_n| over orthogonal land pairs, inside a face and across a face edge; also for river cells only
        let mut s = [[0f64; 2]; 2];
        let mut cnt = [[0u64; 2]; 2];
        for c in 0..sph.len() {
            if z[c] <= 0.0 {
                continue;
            }
            let nb = sph.nbrs(c);
            let (f, _, _) = sph.split(c);
            for k in 0..4 {
                let m = nb[k] as usize;
                if z[m] <= 0.0 {
                    continue;
                }
                let (fm, _, _) = sph.split(m);
                let cross = (fm != f) as usize;
                let dl = ((a[c] as f64).ln() - (a[m] as f64).ln()).abs();
                for (cls, thr) in [(0usize, 0.0f32), (1, 1000.0)] {
                    if a[c] >= thr && a[m] >= thr {
                        s[cls][cross] += dl;
                        cnt[cls][cross] += 1;
                    }
                }
            }
        }
        for (cls, name2) in ["all land", "rivers (A >= 1000 km2)"].iter().enumerate() {
            let mi = s[cls][0] / cnt[cls][0].max(1) as f64;
            let mc = s[cls][1] / cnt[cls][1].max(1) as f64;
            println!("EDGE-CHECK {name} n={n} {name2}: mean |dlnA| inside a face {mi:.4} ({} pairs), across an edge {mc:.4} ({} pairs), ratio {:.3}", cnt[cls][0], cnt[cls][1], mc / mi.max(1e-12));
        }
    }
    // Crop: 384 x 384 window on face 4 (north) of ln A for D8 and MFD side by side, same stretch.
    let (f, i0, j0, w) = (4usize, n / 4, n / 3, 384.min(n / 2));
    let mut rgb = vec![0u8; (2 * w + 4) * w * 3];
    for (p, a) in [&a_d8, &a_mfd].iter().enumerate() {
        for y in 0..w {
            for x in 0..w {
                let c = f * n * n + (j0 + w - 1 - y) * n + (i0 + x);
                let v = if z[c] <= 0.0 { -1.0 } else { ((a[c].max(1.0)).ln() - 2.0) / 12.0 };
                let col = if v < 0.0 { [10, 20, 60] } else { ramp(v) };
                let o = (y * (2 * w + 4) + p * (w + 4) + x) * 3;
                rgb[o..o + 3].copy_from_slice(&col);
            }
        }
    }
    save_png(&format!("{out}/routing-d8-vs-mfd.png"), (2 * w + 4) as u32, w as u32, &rgb);
}

fn main() {
    below_normal();
    let a: Vec<String> = std::env::args().collect();
    let cmd = a.get(1).map(String::as_str).unwrap_or("");
    let out = std::env::var("SPIKE_OUT").unwrap_or_else(|_| "out".into());
    std::fs::create_dir_all(&out).ok();
    match cmd {
        "bake" => {
            let n = arg(&a, 2, 1024usize);
            let threads = arg(&a, 3, 1usize);
            bake(n, threads, "plain");
        }
        "compare" => compare(arg(&a, 2, 1024usize), arg(&a, 3, 1usize), &out),
        "images" => images(arg(&a, 2, 2048usize), arg(&a, 3, 2usize), &out),
        "twoscale" => twoscale::run(arg(&a, 2, 4096usize), arg(&a, 3, 4usize), &out),
        "edgediag" => twoscale::edgediag(arg(&a, 2, 1024usize), arg(&a, 3, 2usize)),
        "evalbench" => evalbench(),
        "refine" => refine_bench(arg(&a, 2, 2048usize), arg(&a, 3, 2usize), &out),
        _ => eprintln!("usage: bake N T | compare N T | images N T | refine N T"),
    }
}

fn with_field<R>(threads: usize, f: impl FnOnce(&dyn HeightField) -> R) -> R {
    let t = Instant::now();
    let grid = global_bake(seed(), &TangentWarp, 2048, threads).expect("bake");
    println!("GLOBAL-GRID in-memory bake 2048 (for the eroded field): {:.1}s, {threads} threads", t.elapsed().as_secs_f64());
    let src = GridSource(&grid);
    let def = WorldDefinition::single_planet(R_M, 42);
    let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [0; 16]).expect("field");
    f(&field)
}

// ---------------------------------------------------------------- images and the pine rule

/// Drivers at a direction from the coarse bake (nearest cell).
struct CoarseDrivers {
    soil_m: f32,
    slope_deg: f32,
    twi: f32,
    log_a: f32,
}
fn coarse_at(b: &Bake, d: Vec3) -> CoarseDrivers {
    let c = b.sph.cell_of(d);
    CoarseDrivers { soil_m: soil_decode(b.dv.soil[c]), slope_deg: b.dv.slope[c] as f32 / 255.0 * 90.0, twi: b.dv.twi[c] as f32 / 255.0 * 16.0, log_a: b.a[c].max(1.0).ln() }
}

fn images(n: usize, threads: usize, out: &str) {
    let b = bake(n, threads, "images");
    let (w, h) = (2048usize, 1024usize);
    let mut pine_rgb = vec![0u8; w * h * 3];
    let mut drv: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; (w / 2) * (h / 2) * 3]);
    let mut suit = vec![0f32; w * h];
    let t0 = Instant::now();
    for y in 0..h {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * core::f64::consts::PI;
        for x in 0..w {
            let lon = ((x as f64 + 0.5) / w as f64 - 0.5) * 2.0 * core::f64::consts::PI;
            let d = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
            let s = sample(seed(), d);
            let o = (y * w + x) * 3;
            if s.elevation_m <= 0.0 {
                pine_rgb[o..o + 3].copy_from_slice(&[24, 44, 84]);
                continue;
            }
            let dr = coarse_at(&b, d);
            let m = moisture(s.precipitation_mm, dr.twi);
            let v = pine(dr.soil_m, m, s.temperature_c, dr.slope_deg, above_treeline(s.elevation_m, s.temperature_c));
            suit[y * w + x] = v;
            let shade = 0.55 + 0.45 * (s.elevation_m / 6000.0).clamp(0.0, 1.0);
            let base = [150.0 * shade, 135.0 * shade, 110.0 * shade];
            let g = [14.0, 120.0, 36.0];
            for c in 0..3 {
                pine_rgb[o + c] = (base[c] * (1.0 - v) + g[c] * v) as u8;
            }
            if x % 2 == 0 && y % 2 == 0 {
                let od = ((y / 2) * (w / 2) + x / 2) * 3;
                let vals = [(dr.log_a - 2.0) / 12.0, (dr.soil_m / 3.0).sqrt(), m, dr.slope_deg / 45.0];
                for (k, v) in vals.iter().enumerate() {
                    drv[k][od..od + 3].copy_from_slice(&ramp(*v));
                }
            }
        }
        // ocean in the driver images
    }
    println!("PINE-IMAGE {w}x{h}: {:.2}s ({:.2} us/pixel incl. the global sample)", t0.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64() / (w * h) as f64 * 1e6);
    for k in 0..4 {
        for y in 0..h / 2 {
            for x in 0..w / 2 {
                let o = (y * (w / 2) + x) * 3;
                if pine_rgb[((y * 2) * w + x * 2) * 3..][..3] == [24, 44, 84] {
                    drv[k][o..o + 3].copy_from_slice(&[24, 44, 84]);
                }
            }
        }
    }
    save_png(&format!("{out}/pine-suitability-equirect.png"), w as u32, h as u32, &pine_rgb);
    for (k, name) in ["log-flow-accumulation", "soil-depth", "moisture-index", "slope"].iter().enumerate() {
        save_png(&format!("{out}/driver-{name}.png"), (w / 2) as u32, (h / 2) as u32, &drv[k]);
    }
    let land: Vec<f32> = suit.iter().copied().filter(|v| *v >= 0.0).collect();
    let n_land = pine_rgb.chunks(3).filter(|c| *c != [24, 44, 84]).count();
    let hi = suit.iter().filter(|v| **v > 0.5).count();
    let any = suit.iter().filter(|v| **v > 0.05).count();
    println!("PINE-STATS land px {n_land} of {}, suitability > 0.5: {hi} ({:.1} % of land), > 0.05: {any} ({:.1} %), total px {}", w * h, 100.0 * hi as f64 / n_land as f64, 100.0 * any as f64 / n_land as f64, land.len());

    // Two close views: pick the best-scoring, relief-rich 8x8 blocks on the equirect image that are >= 40 degrees apart.
    let mut cand: Vec<(f32, usize, usize)> = Vec::new();
    for by in 8..(h / 8 - 8) {
        for bx in 0..w / 8 {
            let (mut s, mut zmax, mut zmin, mut cnt) = (0f32, -1e9f32, 1e9f32, 0);
            let (mut smax, mut smin) = (0f32, 1f32);
            for y in 0..8 {
                for x in 0..8 {
                    let v = suit[(by * 8 + y) * w + bx * 8 + x];
                    s += v;
                    cnt += (v > 0.0) as u32;
                    smax = smax.max(v);
                    smin = smin.min(v);
                }
            }
            if cnt < 40 || smax < 0.5 {
                continue;
            }
            let s = s * (smax - smin);
            let lat = (0.5 - (by as f64 * 8.0 + 4.0) / h as f64) * core::f64::consts::PI;
            let lon = ((bx as f64 * 8.0 + 4.0) / w as f64 - 0.5) * 2.0 * core::f64::consts::PI;
            let d = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
            // relief from the global sample at four offsets
            for (dx, dy) in [(0.02, 0.0), (-0.02, 0.0), (0.0, 0.02), (0.0, -0.02)] {
                let q = (d + Vec3::new(dx, dy, 0.0)).normalized();
                let e = sample(seed(), q).elevation_m;
                zmax = zmax.max(e);
                zmin = zmin.min(e);
            }
            cand.push((s * (zmax - zmin).max(0.0).sqrt(), bx * 8 + 4, by * 8 + 4));
        }
    }
    cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut picks: Vec<(usize, usize)> = Vec::new();
    for &(_, x, y) in &cand {
        let dir = |x: usize, y: usize| {
            let lat = (0.5 - y as f64 / h as f64) * core::f64::consts::PI;
            let lon = (x as f64 / w as f64 - 0.5) * 2.0 * core::f64::consts::PI;
            Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
        };
        if picks.iter().all(|&(px, py)| dir(px, py).dot(dir(x, y)) < libm::cos(40f64.to_radians())) {
            picks.push((x, y));
        }
        if picks.len() == 2 {
            break;
        }
    }
    let nf = 16384usize;
    let finer = nf / n;
    for (vi, &(x, y)) in picks.iter().enumerate() {
        let lat = (0.5 - y as f64 / h as f64) * core::f64::consts::PI;
        let lon = (x as f64 / w as f64 - 0.5) * 2.0 * core::f64::consts::PI;
        let d = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
        println!("CLOSE-VIEW {vi}: lat {:.1} lon {:.1} (equirect px {x},{y})", lat.to_degrees(), lon.to_degrees());
        with_field(threads, |fld| close_view(&b, d, nf, finer, threads, out, vi, fld));
    }
}

/// A close view on the 16384-cell grid: refined drivers (a patch with a halo) against the coarse drivers looked up per fine cell.
fn close_view(b: &Bake, d: Vec3, nf: usize, finer: usize, threads: usize, out: &str, vi: usize, fld: &dyn HeightField) {
    let (f, s, t) = direction_to_face(&TangentWarp, d);
    let ci = (((s + 1.0) * 0.5 * nf as f64) as usize).clamp(0, nf - 1);
    let cj = (((t + 1.0) * 0.5 * nf as f64) as usize).clamp(0, nf - 1);
    let (view, halo) = (768usize, 256usize);
    let tot = view + 2 * halo;
    let i0 = ci.saturating_sub(tot / 2).min(nf - tot);
    let j0 = cj.saturating_sub(tot / 2).min(nf - tot);
    let t0 = Instant::now();
    let p = Patch::new(nf, f.0 as usize, i0, j0, tot, tot);
    let Fields { z, lith } = sample_patch_eroded(&p, fld, threads);
    let t_s = t0.elapsed();
    let t1 = Instant::now();
    let (filled, order, _) = priority_flood(&p, &z);
    let a = accumulate(&p, &filled, &order, Route::Mfd, None);
    let dv = derive(&p, &z, &a, &lith);
    let t_p = t1.elapsed();
    println!("REFINE-PATCH {tot}x{tot} cells ({} with halo {halo}) at n=16384: sample {:.2}s (threads {threads}), flood+accumulate+derive {:.2}s ({:.0} ns/cell)", tot * tot, t_s.as_secs_f64(), t_p.as_secs_f64(), t_p.as_secs_f64() / (tot * tot) as f64 * 1e9);
    let _ = finer;
    // Render the central view: left = refined drivers, right = coarse drivers per fine cell; hillshade from fine heights in both.
    let gap = 4usize;
    let mut rgb = vec![0u8; (2 * view + gap) * view * 3];
    let cell_m = p.h((tot / 2) * tot + tot / 2) as f32;
    let (mut sum_abs, mut cnt, mut sum_r, mut sum_c) = (0f64, 0u64, 0f64, 0f64);
    let mut corr = (0f64, 0f64, 0f64, 0f64, 0f64, 0u64); // sum x, y, xx, yy, xy, n over ln A refined vs coarse
    for y in 0..view {
        for x in 0..view {
            let (px, py) = (halo + x, halo + (view - 1 - y));
            let c = py * tot + px;
            let dir = p.direction(px, py);
            let s = sample(seed(), dir);
            let zc = z[c];
            let shade = {
                let g = |dx: i64, dy: i64| z[((py as i64 + dy) as usize) * tot + (px as i64 + dx) as usize];
                let (gx, gy) = ((g(1, 0) - g(-1, 0)) / (2.0 * cell_m), (g(0, 1) - g(0, -1)) / (2.0 * cell_m));
                let n = Vec3::new(-gx as f64, -gy as f64, 1.0).normalized();
                let l = Vec3::new(-0.5, 0.5, 0.7).normalized();
                (0.35 + 0.65 * n.dot(l).max(0.0)) as f32
            };
            for panel in 0..2 {
                let o = (y * (2 * view + gap) + panel * (view + gap) + x) * 3;
                if zc <= 0.0 {
                    rgb[o..o + 3].copy_from_slice(&[24, 44, 84]);
                    continue;
                }
                let (soil, slope, twi, la) = if panel == 0 {
                    (soil_decode(dv.soil[c]), dv.slope[c] as f32 / 255.0 * 90.0, dv.twi[c] as f32 / 255.0 * 16.0, a[c].max(1.0).ln())
                } else {
                    let cd = coarse_at(b, dir);
                    (cd.soil_m, cd.slope_deg, cd.twi, cd.log_a)
                };
                if panel == 1 && zc > 0.0 {
                    let (la_f, la_c) = (a[c].max(1.0).ln() as f64, la as f64);
                    corr.0 += la_f;
                    corr.1 += la_c;
                    corr.2 += la_f * la_f;
                    corr.3 += la_c * la_c;
                    corr.4 += la_f * la_c;
                    corr.5 += 1;
                }
                let m = moisture(s.precipitation_mm, twi);
                let v = pine(soil, m, s.temperature_c, slope, above_treeline(zc, s.temperature_c));
                if panel == 0 {
                    sum_r += v as f64;
                } else {
                    sum_c += v as f64;
                }
                let base = [150.0, 135.0, 110.0];
                let g = [14.0, 120.0, 36.0];
                for k in 0..3 {
                    rgb[o + k] = ((base[k] * (1.0 - v) + g[k] * v) * shade).min(255.0) as u8;
                }
                if panel == 0 {
                    // keep for the difference statistic below
                }
            }
            // difference statistic: |refined - coarse| suitability on land
            if zc > 0.0 {
                let cd = coarse_at(b, dir);
                let mc = moisture(s.precipitation_mm, cd.twi);
                let vc = pine(cd.soil_m, mc, s.temperature_c, cd.slope_deg, above_treeline(zc, s.temperature_c));
                let mr = moisture(s.precipitation_mm, dv.twi[c] as f32 / 255.0 * 16.0);
                let vr = pine(soil_decode(dv.soil[c]), mr, s.temperature_c, dv.slope[c] as f32 / 255.0 * 90.0, above_treeline(zc, s.temperature_c));
                sum_abs += (vr - vc).abs() as f64;
                cnt += 1;
            }
        }
    }
    let n = corr.5.max(1) as f64;
    let cov = corr.4 / n - corr.0 / n * corr.1 / n;
    let r = cov / ((corr.2 / n - (corr.0 / n).powi(2)).sqrt() * (corr.3 / n - (corr.1 / n).powi(2)).sqrt()).max(1e-12);
    println!(
        "CLOSE-STATS view {vi}: land cells {cnt} of {}, mean suitability refined {:.3} vs coarse {:.3}, mean |difference| {:.3}; Pearson r of ln(A) refined vs coarse-lookup {:.3}",
        view * view,
        sum_r / cnt.max(1) as f64,
        sum_c / cnt.max(1) as f64,
        sum_abs / cnt.max(1) as f64,
        r
    );
    save_png(&format!("{out}/pine-close-view-{vi}.png"), (2 * view + gap) as u32, view as u32, &rgb);
}

/// Regional refinement cost: patch sizes with and without halo, single thread.
fn refine_bench(n_unused: usize, threads: usize, _out: &str) {
    let _ = n_unused;
    with_field(threads, |fld| refine_bench_inner(threads, fld));
}
fn refine_bench_inner(threads: usize, fld: &dyn HeightField) {
    let nf = 16384usize;
    for tot in [512usize, 1024, 1536, 2048] {
        // centred on a land point (lat 31.6, lon -145.5), clamped into its face
        let envf = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
        let (lat, lon) = (envf("SPIKE_LAT", 31.6).to_radians(), envf("SPIKE_LON", -145.5).to_radians());
        let dd = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
        let (ff, ss_, tt) = direction_to_face(&TangentWarp, dd);
        let ci = (((ss_ + 1.0) * 0.5 * nf as f64) as usize).clamp(0, nf - 1);
        let cj = (((tt + 1.0) * 0.5 * nf as f64) as usize).clamp(0, nf - 1);
        let (f, i0, j0) = (ff.0 as usize, ci.saturating_sub(tot / 2).min(nf - tot), cj.saturating_sub(tot / 2).min(nf - tot));
        let t0 = Instant::now();
        let p = Patch::new(nf, f, i0, j0, tot, tot);
        let t_topo = t0.elapsed();
        let t1 = Instant::now();
        let Fields { z, lith } = sample_patch(&p, threads);
        let t_s = t1.elapsed();
        let t1e = Instant::now();
        let Fields { z: ze, lith: _ } = sample_patch_eroded(&p, fld, threads);
        let t_se = t1e.elapsed();
        println!("REFINE-BENCH-SAMPLE patch {tot}x{tot}: global-field height {:.2}s, eroded height {:.2}s ({:.2} us/cell, {threads} threads); mean |eroded - base| {:.1} m", t_s.as_secs_f64(), t_se.as_secs_f64(), t_se.as_secs_f64() * threads as f64 / (tot * tot) as f64 * 1e6, z.iter().zip(&ze).map(|(a, b)| (a - b).abs() as f64).sum::<f64>() / z.len() as f64);
        println!("DEBUG patch face {f} i0 {i0} j0 {j0} z base [{:.0},{:.0}] eroded [{:.0},{:.0}] centre dir {:?}", z.iter().cloned().fold(1e9f32, f32::min), z.iter().cloned().fold(-1e9f32, f32::max), ze.iter().cloned().fold(1e9f32, f32::min), ze.iter().cloned().fold(-1e9f32, f32::max), p.direction(tot/2, tot/2));
        let z = ze;
        let t2 = Instant::now();
        let (filled, order, fs) = priority_flood(&p, &z);
        let t_f = t2.elapsed();
        let t3 = Instant::now();
        let a = accumulate(&p, &filled, &order, Route::Mfd, None);
        let t_a = t3.elapsed();
        let t4 = Instant::now();
        let dv = derive(&p, &z, &a, &lith);
        let t_d = t4.elapsed();
        let cells = tot * tot;
        println!(
            "REFINE-BENCH patch {tot}x{tot} ({} cells, {:.0} km across) n=16384: topo {:.2}s sample {:.2}s ({threads} threads) flood {:.2}s accumulate {:.2}s derive {:.2}s | sequential part {:.0} ns/cell | land {:.0} % | checksum {}",
            cells,
            tot as f64 * p.h(cells / 2) as f64 / 1000.0,
            t_topo.as_secs_f64(),
            t_s.as_secs_f64(),
            t_f.as_secs_f64(),
            t_a.as_secs_f64(),
            t_d.as_secs_f64(),
            (t_f + t_a + t_d).as_secs_f64() / cells as f64 * 1e9,
            100.0 * fs.land as f64 / cells as f64,
            dv.soil.iter().map(|&v| v as u64).sum::<u64>()
        );
    }
}

/// Cost of evaluating the rule and of reading a driver vector, single thread (the CPU side of per-tile against per-pixel).
fn evalbench() {
    let b = bake(512, 1, "evalbench");
    // 1. the pine rule from five numbers
    let mut x = 0x1234_5678_9abc_def0u64;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 40) as f32 / (1u64 << 24) as f32
    };
    let inputs: Vec<[f32; 5]> = (0..4096).map(|_| [rnd() * 4.0, rnd(), rnd() * 40.0 - 15.0, rnd() * 50.0, rnd() * 1500.0 - 1000.0]).collect();
    let reps = 20_000usize;
    let t = Instant::now();
    let mut acc = 0f32;
    for r in 0..reps {
        for v in &inputs {
            acc += pine(v[0] + (r & 1) as f32 * 0.001, v[1], v[2], v[3], v[4]);
        }
    }
    let ns = t.elapsed().as_secs_f64() / (reps * inputs.len()) as f64 * 1e9;
    println!("EVAL pine rule (5 trapezoids, product): {ns:.1} ns per point, single thread (checksum {acc:.1})");
    // 2. a driver vector read at a direction: cell lookup + three channel reads
    let dirs: Vec<Vec3> = (0..1_000_000).map(|_| Vec3::new(rnd() as f64 - 0.5, rnd() as f64 - 0.5, rnd() as f64 - 0.5).normalized()).collect();
    let t = Instant::now();
    let mut acc = 0u64;
    for d in &dirs {
        let c = b.sph.cell_of(*d);
        acc += b.dv.soil[c] as u64 + b.dv.slope[c] as u64 + b.dv.twi[c] as u64;
    }
    let ns = t.elapsed().as_secs_f64() / dirs.len() as f64 * 1e9;
    println!("EVAL driver-vector read by direction (cube-face lookup, nearest, 3 u8 channels, 512 grid): {ns:.1} ns per point (checksum {acc})");
    // 3. one tile's 9 x 9 vector from the grid
    let t = Instant::now();
    let mut acc = 0u64;
    for k in 0..10_000usize {
        for j in 0..9 {
            for i in 0..9 {
                let d = dirs[(k * 81 + j * 9 + i) % dirs.len()];
                let c = b.sph.cell_of(d);
                acc += b.dv.soil[c] as u64 + b.dv.slope[c] as u64 + b.dv.twi[c] as u64 + (b.a[c] as u64 & 0xff);
            }
        }
    }
    println!("EVAL one tile's 9x9 driver vector (4 channels) from the grid: {:.2} us per tile (checksum {acc})", t.elapsed().as_secs_f64() / 10_000.0 * 1e6);
}
