//! Terrain descriptors on a height raster (GEN-089..GEN-092, change `terrain-scorecard`).
//!
//! Pure f64 kernels on a row-major raster (`z[j * w + i]`, `i` along x, `j` along y, cell size `cell` in metres). NaN marks no-data. Every output holds NaN (or
//! [`UNCOUNTED`] for classes) where the cell is uncounted: its window touches the raster edge or a no-data cell.
//!
//! Thread independence (CON-14): the per-cell kernels split output rows over scoped workers that write disjoint rows and read only the input; every sum
//! (summed-area tables, drainage accumulation) runs on one thread in index order; the flood fill is single-threaded. Transcendentals use `libm`.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Class value of a cell the geomorphon classifier does not count.
pub const UNCOUNTED: u8 = 255;

/// Geomorphon classes (Jasiewicz and Stepinski 2013), the values of [`geomorphons`].
pub mod form {
    pub const FLAT: u8 = 0;
    pub const PEAK: u8 = 1;
    pub const RIDGE: u8 = 2;
    pub const SHOULDER: u8 = 3;
    pub const SPUR: u8 = 4;
    pub const SLOPE: u8 = 5;
    pub const HOLLOW: u8 = 6;
    pub const FOOTSLOPE: u8 = 7;
    pub const VALLEY: u8 = 8;
    pub const PIT: u8 = 9;
}

/// Epsilon of the priority flood in metres (GEN-092).
pub const FILL_EPSILON_M: f64 = 1e-3;
/// Channel threshold of the drainage density in m2 (GEN-092).
pub const CHANNEL_AREA_M2: f64 = 1e5;
/// Receiver value of a cell that drains nowhere (an outlet).
pub const NO_RECEIVER: u32 = u32::MAX;

/// A height raster borrowed for the kernels; NaN is no-data.
#[derive(Clone, Copy)]
pub struct Raster<'a> {
    pub z: &'a [f64],
    pub w: usize,
    pub h: usize,
    /// Cell size in metres.
    pub cell: f64,
}

impl<'a> Raster<'a> {
    pub fn new(z: &'a [f64], w: usize, h: usize, cell: f64) -> Self {
        assert_eq!(z.len(), w * h, "a {w} x {h} raster holds {} heights, not {}", w * h, z.len());
        assert!(cell.is_finite() && cell > 0.0, "the cell size must be finite and positive, got {cell}");
        Raster { z, w, h, cell }
    }
}

/// Runs `f(row, out_row)` for every row of `out` (`w` values per row, `rows` rows) on up to `threads` scoped workers over disjoint row bands.
fn par_rows<T: Send>(out: &mut [T], w: usize, rows: usize, threads: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    if rows == 0 || w == 0 {
        return;
    }
    debug_assert_eq!(out.len(), w * rows);
    let per = rows.div_ceil(threads.clamp(1, rows)).max(1);
    std::thread::scope(|s| {
        for (c, band) in out.chunks_mut(per * w).enumerate() {
            let f = &f;
            planet_core::thread::spawn_named(s, "descriptors", move || {
                for (r, row) in band.chunks_mut(w).enumerate() {
                    f(c * per + r, row);
                }
            });
        }
    });
}

/// The 3x3 window around `(i, j)` in row-major order (`z1 z2 z3 / z4 z5 z6 / z7 z8 z9`, rows with `j - 1` first); `None` at the edge or next to no-data.
fn window3(r: &Raster, i: usize, j: usize) -> Option<[f64; 9]> {
    if i == 0 || j == 0 || i + 1 >= r.w || j + 1 >= r.h {
        return None;
    }
    let mut a = [0.0; 9];
    for (k, v) in a.iter_mut().enumerate() {
        *v = r.z[(j + k / 3 - 1) * r.w + (i + k % 3 - 1)];
        if v.is_nan() {
            return None;
        }
    }
    Some(a)
}

/// Slope in degrees: `atan` of the central-difference gradient (GEN-089). NaN where uncounted.
pub fn slope_degrees(r: &Raster, threads: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; r.w * r.h];
    par_rows(&mut out, r.w, r.h, threads, |j, row| {
        for (i, o) in row.iter_mut().enumerate() {
            if let Some(a) = window3(r, i, j) {
                let gx = (a[5] - a[3]) / (2.0 * r.cell);
                let gy = (a[7] - a[1]) / (2.0 * r.cell);
                *o = libm::atan(libm::sqrt(gx * gx + gy * gy)).to_degrees();
            }
        }
    });
    out
}

/// Profile and contour curvature in 1/m (GEN-089), one value per cell each.
pub struct Curvature {
    pub profile: Vec<f64>,
    pub contour: Vec<f64>,
}

/// Evans-Young quadric over the 3x3 window. `p`, `q` are the slopes along `i` and `j`, `rr`, `t`, `s` the second derivatives `zii`, `zjj`, `zij`.
fn quadric(a: &[f64; 9], h: f64) -> (f64, f64, f64, f64, f64) {
    let p = (a[2] + a[5] + a[8] - a[0] - a[3] - a[6]) / (6.0 * h);
    let q = (a[6] + a[7] + a[8] - a[0] - a[1] - a[2]) / (6.0 * h);
    let rr = (a[0] + a[2] + a[3] + a[5] + a[6] + a[8] - 2.0 * (a[1] + a[4] + a[7])) / (3.0 * h * h);
    let t = (a[0] + a[1] + a[2] + a[6] + a[7] + a[8] - 2.0 * (a[3] + a[4] + a[5])) / (3.0 * h * h);
    let s = (a[8] + a[0] - a[2] - a[6]) / (4.0 * h * h);
    (p, q, rr, t, s)
}

/// Gradient under which the profile curvature is 0 (GEN-089).
pub const GRADIENT_FLOOR: f64 = 1e-6;
/// Gradient under which the contour curvature is uncounted: it grows as `1 / g` there and would only report noise (GEN-089).
pub const CONTOUR_GRADIENT_MIN: f64 = 1e-3;

/// Profile curvature (along the gradient) and contour curvature (across it), both positive where the surface curves upward (a bowl). The profile is 0 where the
/// gradient is under [`GRADIENT_FLOOR`]; the contour is NaN where it is under [`CONTOUR_GRADIENT_MIN`] (GEN-089). NaN where uncounted.
pub fn curvature(r: &Raster, threads: usize) -> Curvature {
    let mut out = vec![[f64::NAN; 2]; r.w * r.h];
    par_rows(&mut out, r.w, r.h, threads, |j, row| {
        for (i, o) in row.iter_mut().enumerate() {
            if let Some(a) = window3(r, i, j) {
                let (p, q, rr, t, s) = quadric(&a, r.cell);
                let g2 = p * p + q * q;
                let g = libm::sqrt(g2);
                let w = 1.0 + g2;
                let profile = if g < GRADIENT_FLOOR { 0.0 } else { (p * p * rr + 2.0 * p * q * s + q * q * t) / (g2 * w * libm::sqrt(w)) };
                let contour = if g < CONTOUR_GRADIENT_MIN { f64::NAN } else { (q * q * rr - 2.0 * p * q * s + p * p * t) / (g2 * g) };
                *o = [profile, contour];
            }
        }
    });
    Curvature { profile: out.iter().map(|v| v[0]).collect(), contour: out.iter().map(|v| v[1]).collect() }
}

/// Relief per block: `bw x bh` blocks of `side x side` cells from the raster's origin; a partial block at the east or south edge is dropped.
pub struct BlockRelief {
    pub bw: usize,
    pub bh: usize,
    /// Max minus min per block, row-major; NaN for a block holding a no-data cell.
    pub values: Vec<f64>,
}

/// Local relief over non-overlapping square blocks (GEN-090).
pub fn local_relief(r: &Raster, side: usize, threads: usize) -> BlockRelief {
    assert!(side > 0, "the block side must be positive");
    let (bw, bh) = (r.w / side, r.h / side);
    let mut values = vec![f64::NAN; bw * bh];
    par_rows(&mut values, bw, bh, threads, |bj, row| {
        for (bi, o) in row.iter_mut().enumerate() {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            let mut ok = true;
            for j in bj * side..(bj + 1) * side {
                for &v in &r.z[j * r.w + bi * side..j * r.w + (bi + 1) * side] {
                    ok &= !v.is_nan();
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            if ok {
                *o = hi - lo;
            }
        }
    });
    BlockRelief { bw, bh, values }
}

/// Summed-area table of `v` (`(w + 1) x (h + 1)`), accumulated in f64 in row order with additions only.
fn summed_area(w: usize, h: usize, v: impl Fn(usize) -> f64) -> Vec<f64> {
    let sw = w + 1;
    let mut s = vec![0.0; sw * (h + 1)];
    for j in 0..h {
        let mut run = 0.0;
        for i in 0..w {
            run += v(j * w + i);
            s[(j + 1) * sw + i + 1] = s[j * sw + i + 1] + run;
        }
    }
    s
}

/// Topographic position index: the height minus the mean over the square window of side `2 radius + 1` cells (GEN-090). Counted only at cells at least `radius`
/// from the edge whose window holds no no-data cell; NaN elsewhere.
pub fn tpi(r: &Raster, radius: usize, threads: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; r.w * r.h];
    if r.w < 2 * radius + 1 || r.h < 2 * radius + 1 {
        return out;
    }
    // The first data cell is the reference: sums of offsets keep the tables small when the heights are large.
    let reference = r.z.iter().copied().find(|v| !v.is_nan()).unwrap_or(0.0);
    let sums = summed_area(r.w, r.h, |k| if r.z[k].is_nan() { 0.0 } else { r.z[k] - reference });
    let counts = summed_area(r.w, r.h, |k| if r.z[k].is_nan() { 0.0 } else { 1.0 });
    let sw = r.w + 1;
    let side = 2 * radius + 1;
    let n = (side * side) as f64;
    let rect = |t: &[f64], i: usize, j: usize| {
        let (x0, y0, x1, y1) = (i - radius, j - radius, i + radius + 1, j + radius + 1);
        t[y1 * sw + x1] - t[y0 * sw + x1] - t[y1 * sw + x0] + t[y0 * sw + x0]
    };
    par_rows(&mut out, r.w, r.h, threads, |j, row| {
        if j < radius || j + radius >= r.h {
            return;
        }
        for (i, o) in row.iter_mut().enumerate().skip(radius).take(r.w - 2 * radius) {
            let z = r.z[j * r.w + i];
            if !z.is_nan() && rect(&counts, i, j) == n {
                *o = (z - reference) - rect(&sums, i, j) / n;
            }
        }
    });
    out
}

/// Class of a ternary pattern with `minus` lower and `plus` higher directions (GEN-091), `minus + plus <= 8`. The table is the matrix `forms[minus][plus]` of
/// the reference implementation (GRASS `r.geomorphon`, `geom.c` `determine_form`), read for its definition only.
pub fn geomorphon_class(minus: usize, plus: usize) -> u8 {
    use form::{
        FLAT as FL, FOOTSLOPE as FS, HOLLOW as HL, PEAK as PK, PIT as PT, RIDGE as RI, SHOULDER as SH, SLOPE as SL, SPUR as SP,
        VALLEY as VL,
    };
    assert!(minus + plus <= 8, "{minus} lower and {plus} higher directions are more than the 8 there are");
    const TABLE: [[u8; 9]; 9] = [
        [FL, FL, FL, FS, FS, VL, VL, VL, PT],
        [FL, FL, FS, FS, FS, VL, VL, VL, 0],
        [FL, SH, SL, SL, HL, HL, VL, 0, 0],
        [SH, SH, SL, SL, SL, HL, 0, 0, 0],
        [SH, SH, SP, SL, SL, 0, 0, 0, 0],
        [RI, RI, SP, SP, 0, 0, 0, 0, 0],
        [RI, RI, RI, 0, 0, 0, 0, 0, 0],
        [RI, RI, 0, 0, 0, 0, 0, 0, 0],
        [PK, 0, 0, 0, 0, 0, 0, 0, 0],
    ];
    TABLE[minus][plus]
}

/// Geomorphon class per cell (GEN-091): 8 directions out to `lookup` cells, flatness threshold `flat_deg`. [`UNCOUNTED`] within `lookup` cells of the edge or where
/// a looked-at cell is no-data.
pub fn geomorphons(r: &Raster, lookup: usize, flat_deg: f64, threads: usize) -> Vec<u8> {
    const DIRS: [(isize, isize); 8] = [(1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1)];
    let mut out = vec![UNCOUNTED; r.w * r.h];
    if lookup == 0 || r.w < 2 * lookup + 1 || r.h < 2 * lookup + 1 {
        return out;
    }
    // Elevation angles are compared as tangents (monotone), so no `atan` runs per step.
    let flat = libm::tan(flat_deg.to_radians());
    par_rows(&mut out, r.w, r.h, threads, |j, row| {
        if j < lookup || j + lookup >= r.h {
            return;
        }
        'cell: for (i, o) in row.iter_mut().enumerate().skip(lookup).take(r.w - 2 * lookup) {
            let z0 = r.z[j * r.w + i];
            if z0.is_nan() {
                continue;
            }
            let (mut minus, mut plus) = (0, 0);
            for &(dx, dy) in &DIRS {
                let step = if dx != 0 && dy != 0 { std::f64::consts::SQRT_2 * r.cell } else { r.cell };
                let (mut up, mut down) = (f64::NEG_INFINITY, f64::INFINITY);
                for k in 1..=lookup as isize {
                    let zk = r.z[(j as isize + dy * k) as usize * r.w + (i as isize + dx * k) as usize];
                    if zk.is_nan() {
                        continue 'cell;
                    }
                    let tan = (zk - z0) / (k as f64 * step);
                    up = up.max(tan);
                    down = down.min(tan);
                }
                // The rule of `compare_multi` in the reference: the zenith angle `up` and the nadir angle `down` count when their size exceeds the threshold; if only
                // one does it decides, if both do the larger size decides and an exact tie is higher.
                let (zenith, nadir) = (up.abs(), down.abs());
                match (zenith > flat, nadir > flat) {
                    (false, false) => {}
                    (true, false) => plus += 1,
                    (false, true) => minus += 1,
                    (true, true) if nadir > zenith => minus += 1,
                    (true, true) => plus += 1,
                }
            }
            *o = geomorphon_class(minus, plus);
        }
    });
    out
}

/// Priority flood key: height, then cell index (the tie rule of GEN-092).
#[derive(PartialEq)]
struct Key(f64, usize);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.total_cmp(&o.0).then(self.1.cmp(&o.1))
    }
}

/// Filled heights, D8 receivers and drainage area of a raster (GEN-092).
pub struct Drainage {
    /// Heights after the priority flood with epsilon; NaN at no-data.
    pub filled: Vec<f64>,
    /// Receiver cell index, [`NO_RECEIVER`] for an outlet and at no-data.
    pub receiver: Vec<u32>,
    /// Drainage area in m2 (the cell's own included); NaN at no-data.
    pub area: Vec<f64>,
}

/// D8 neighbours in the tie order N, NE, E, SE, S, SW, W, NW as `(dx, dy)` with `j` growing southward.
const D8: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// Fills depressions (priority flood with [`FILL_EPSILON_M`], ties by cell index), routes D8 on the filled surface (steepest descent, ties by the order N, NE, E, ...)
/// and accumulates the area in topological order (GEN-092). Raster edge cells and cells next to no-data are the outlets. Single-threaded.
pub fn drainage(r: &Raster) -> Drainage {
    let n = r.w * r.h;
    assert!(n < NO_RECEIVER as usize, "the raster is too large for u32 cell indices");
    let nb = |idx: usize, d: (isize, isize)| -> Option<usize> {
        let (i, j) = ((idx % r.w) as isize + d.0, (idx / r.w) as isize + d.1);
        (i >= 0 && j >= 0 && (i as usize) < r.w && (j as usize) < r.h).then(|| j as usize * r.w + i as usize)
    };
    let mut filled = vec![f64::NAN; n];
    let mut queued = vec![false; n];
    let mut heap = BinaryHeap::new();
    for idx in 0..n {
        if r.z[idx].is_nan() {
            continue;
        }
        let outlet = D8.iter().any(|&d| nb(idx, d).is_none_or(|m| r.z[m].is_nan()));
        if outlet {
            filled[idx] = r.z[idx];
            queued[idx] = true;
            heap.push(Reverse(Key(r.z[idx], idx)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse(Key(zc, c))) = heap.pop() {
        order.push(c);
        for &d in &D8 {
            if let Some(m) = nb(c, d) {
                if !queued[m] && !r.z[m].is_nan() {
                    queued[m] = true;
                    filled[m] = r.z[m].max(zc + FILL_EPSILON_M);
                    heap.push(Reverse(Key(filled[m], m)));
                }
            }
        }
    }
    let mut receiver = vec![NO_RECEIVER; n];
    for idx in 0..n {
        if filled[idx].is_nan() {
            continue;
        }
        let mut best = 0.0;
        for &d in &D8 {
            if let Some(m) = nb(idx, d) {
                if filled[m] < filled[idx] {
                    let dist = if d.0 != 0 && d.1 != 0 { std::f64::consts::SQRT_2 * r.cell } else { r.cell };
                    let slope = (filled[idx] - filled[m]) / dist;
                    if slope > best {
                        best = slope;
                        receiver[idx] = m as u32;
                    }
                }
            }
        }
    }
    let mut area: Vec<f64> = (0..n).map(|k| if filled[k].is_nan() { f64::NAN } else { r.cell * r.cell }).collect();
    // A receiver is strictly lower, so it was popped before its donor: the reverse pop order visits donors first.
    for &c in order.iter().rev() {
        let m = receiver[c];
        if m != NO_RECEIVER {
            area[m as usize] += area[c];
        }
    }
    Drainage { filled, receiver, area }
}

/// Drainage density in km per km2 (GEN-092): the length of the cells with area of at least `channel_area_m2` (`cell` or `cell * sqrt(2)` to the receiver) over the
/// area of all data cells. Summed in index order.
pub fn drainage_density(r: &Raster, d: &Drainage, channel_area_m2: f64) -> f64 {
    let (mut length, mut cells) = (0.0, 0usize);
    for idx in 0..r.w * r.h {
        if d.filled[idx].is_nan() {
            continue;
        }
        cells += 1;
        let m = d.receiver[idx];
        if d.area[idx] >= channel_area_m2 && m != NO_RECEIVER {
            let diagonal = (m as usize % r.w != idx % r.w) && (m as usize / r.w != idx / r.w);
            length += if diagonal { std::f64::consts::SQRT_2 * r.cell } else { r.cell };
        }
    }
    if cells == 0 {
        return f64::NAN;
    }
    (length / 1000.0) / (cells as f64 * r.cell * r.cell / 1e6)
}
