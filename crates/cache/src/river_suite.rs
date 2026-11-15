//! The river gate's measurement suite (BUILD-057): the shape metrics every river candidate of P3 is judged by beside the gate's counts. Every function is pure: the same input gives the same bits.
//! One definition per metric, in the BUILD-057 delta; the Earth benchmark that gives the bars is the spikes' (#121).
use planet_generators::bake::route::NONE;
use planet_generators::bake::Grid;

/// A river step uphill by more than this is counted (metres).
pub const UPHILL_M: f64 = 0.25;
/// The valley width is measured at this depth above the river cell (metres) and at half the ring 2-4 depth.
pub const WIDTH_DEPTH_M: f64 = 10.0;
/// The cross-section is walked at most this many cells to each side; a side that reaches it is capped.
pub const WIDTH_MAX_CELLS: usize = 64;
/// The asymmetry reads the surface 2 to 4 cells out on each side.
pub const ASYM_CELLS: (usize, usize) = (2, 4);
/// A ratio is counted only above this denominator (metres).
pub const MIN_DEPTH_M: f64 = 1.0;
/// Slope-area bins: half decades of the area in cells, from 4 cells; a bin is fitted from this many cells on.
pub const SA_MIN_AREA_CELLS: f64 = 4.0;
pub const SA_BIN_MIN_CELLS: usize = 30;
/// The relief spectrum's square is at most this many cells a side (a power of two).
pub const SPECTRUM_MAX_N: usize = 1024;

/// The quantile `f` of `v` (0 for none).
pub fn quantile(v: &mut [f64], f: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 * f) as usize).min(v.len() - 1)]
}

/// p50 and p90 of a set (and its size).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Spread {
    pub n: usize,
    pub p50: f64,
    pub p90: f64,
}

pub fn spread(mut v: Vec<f64>) -> Spread {
    Spread { n: v.len(), p50: quantile(&mut v, 0.5), p90: quantile(&mut v, 0.9) }
}

/// One slope-area bin: the cells whose area lies in the half decade, and the p50 of their slope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlopeBin {
    pub area_lo_cells: f64,
    pub cells: usize,
    pub slope_p50: f64,
}

/// A relief power spectrum: the mean power per octave of the wavenumber (cycles per square), from 1 on, and the log-log slope fitted over the octaves from the second on.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Spectrum {
    pub n: usize,
    pub power: Vec<f64>,
    pub beta: f64,
}

/// The suite's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct SuiteReport {
    /// River steps inside (a river cell to its river receiver), those rising by more than [`UPHILL_M`], and the largest rise.
    pub steps: usize,
    pub uphill_steps: usize,
    pub uphill_max_m: f64,
    /// The valley width (metres, both sides, across the flow) at [`WIDTH_DEPTH_M`] above the river cell and at half the ring 2-4 depth; sides that reached [`WIDTH_MAX_CELLS`]; cells with a flat ring (no half depth).
    pub width_fixed: Spread,
    pub width_fixed_capped: usize,
    pub width_half: Spread,
    pub width_half_capped: usize,
    pub width_half_flat: usize,
    /// The asymmetry |L - R| / (L + R) of the rise 2 to 4 cells out on the two sides, over cells with L + R at least [`MIN_DEPTH_M`].
    pub asymmetry: Spread,
    /// The drainage density (river length inside per land area inside, km/km2), with the lengths.
    pub density_km_per_km2: f64,
    pub river_km: f64,
    pub land_km2: f64,
    /// H1 crossings matched by an L crossing; the median implied runoff (L area per H1 discharge, m2 per m3/s) and the p10 and p90 of the ratio to it (conservation: 1 when the area is the H1's).
    pub conservation_pairs: usize,
    pub implied_runoff: f64,
    pub conservation_p10: f64,
    pub conservation_p90: f64,
    /// Neighbour pairs across the region's edge (count) and their height step |dz|, against the pairs inside the region.
    pub seam_pairs: usize,
    pub seam_p50_m: f64,
    pub seam_p99_m: f64,
    pub seam_max_m: f64,
    pub interior_p50_m: f64,
    pub interior_p99_m: f64,
    /// The narrowness ratio: valley depth (ring 1-2) over the ring 2-4 depth, per river cell with the latter at least [`MIN_DEPTH_M`], and the ratio of the two p50s.
    pub narrowness: Spread,
    pub narrowness_of_p50: f64,
    /// Slope against area, log-binned, and the concavity (the slope falls as area^-theta over the fitted bins; 0 with fewer than two).
    pub slope_area: Vec<SlopeBin>,
    pub theta: f64,
    /// The relief spectrum of the surface and of the start surface.
    pub spectrum: Spectrum,
    pub spectrum_start: Spectrum,
}

/// The surface at the fractional cell (x, y), bilinear (None outside the grid).
fn bilinear(z: &[f64], w: usize, h: usize, x: f64, y: f64) -> Option<f64> {
    if x < 0.0 || y < 0.0 || x > (w - 1) as f64 || y > (h - 1) as f64 {
        return None;
    }
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let at = |i: usize, j: usize| z[j * w + i];
    Some(at(x0, y0) * (1.0 - fx) * (1.0 - fy) + at(x1, y0) * fx * (1.0 - fy) + at(x0, y1) * (1.0 - fx) * fy + at(x1, y1) * fx * fy)
}

/// Cells from `c` to where the cross-section, walked along (nx, ny) in steps of a cell, first stands more than `depth` above `base` (the crossing interpolated); the flag says the walk ended at the
/// limit or the grid's edge.
fn reach(z: &[f64], w: usize, h: usize, c: usize, (nx, ny): (f64, f64), base: f64, depth: f64) -> (f64, bool) {
    let (cx, cy) = ((c % w) as f64, (c / w) as f64);
    let mut last = base;
    for s in 1..=WIDTH_MAX_CELLS {
        let Some(v) = bilinear(z, w, h, cx + nx * s as f64, cy + ny * s as f64) else {
            return (s as f64 - 1.0, true);
        };
        if v > base + depth {
            let f = if v > last { (base + depth - last) / (v - last) } else { 1.0 };
            return (s as f64 - 1.0 + f.clamp(0.0, 1.0), false);
        }
        last = v;
    }
    (WIDTH_MAX_CELLS as f64, true)
}

/// The cross-section direction of river cell `c` (unit, across the flow towards its left), None without a receiver.
fn across(w: usize, c: usize, r: u32) -> Option<(f64, f64)> {
    if r == NONE {
        return None;
    }
    let (dx, dy) = ((r as usize % w) as f64 - (c % w) as f64, (r as usize / w) as f64 - (c / w) as f64);
    let len = libm::hypot(dx, dy);
    (len > 0.0).then(|| (-dy / len, dx / len))
}

/// The in-place radix-2 FFT of `re + i im` (length a power of two).
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (libm::sin(ang * k as f64), libm::cos(ang * k as f64));
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// The relief power spectrum of the largest power-of-two square (at most [`SPECTRUM_MAX_N`]) inside the box of the `inside` cells, centred in it: the mean removed, a Hann window, the 2-D FFT, the power
/// (m2) averaged over octaves of the radial wavenumber, and the slope of log power against log wavenumber fitted over the octaves from the second.
pub fn spectrum(grid: &Grid, z: &[f64], inside: &[bool]) -> Spectrum {
    let (w, h) = (grid.w(), grid.h());
    let (mut x0, mut x1, mut y0, mut y1) = (w, 0, h, 0);
    for c in 0..w * h {
        if inside[c] {
            x0 = x0.min(c % w);
            x1 = x1.max(c % w + 1);
            y0 = y0.min(c / w);
            y1 = y1.max(c / w + 1);
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return Spectrum::default();
    }
    let side = (x1 - x0).min(y1 - y0).min(SPECTRUM_MAX_N);
    if side < 8 {
        return Spectrum::default();
    }
    let n = 1usize << (usize::BITS - 1 - side.leading_zeros());
    let (ox, oy) = (x0 + (x1 - x0 - n) / 2, y0 + (y1 - y0 - n) / 2);
    let mean = (0..n * n).map(|k| z[(oy + k / n) * w + ox + k % n]).sum::<f64>() / (n * n) as f64;
    let hann: Vec<f64> = (0..n).map(|i| 0.5 - 0.5 * libm::cos(2.0 * std::f64::consts::PI * (i as f64 + 0.5) / n as f64)).collect();
    let win2 = (0..n * n)
        .map(|k| {
            let v = hann[k % n] * hann[k / n];
            v * v
        })
        .sum::<f64>();
    let mut re: Vec<f64> = (0..n * n).map(|k| (z[(oy + k / n) * w + ox + k % n] - mean) * hann[k % n] * hann[k / n]).collect();
    let mut im = vec![0.0; n * n];
    for row in 0..n {
        fft(&mut re[row * n..(row + 1) * n], &mut im[row * n..(row + 1) * n]);
    }
    let (mut cr, mut ci) = (vec![0.0; n], vec![0.0; n]);
    for col in 0..n {
        for row in 0..n {
            cr[row] = re[row * n + col];
            ci[row] = im[row * n + col];
        }
        fft(&mut cr, &mut ci);
        for row in 0..n {
            re[row * n + col] = cr[row];
            im[row * n + col] = ci[row];
        }
    }
    let octaves = (n / 2).ilog2() as usize + 1;
    let (mut sum, mut cnt) = (vec![0.0; octaves], vec![0usize; octaves]);
    for ky in 0..n {
        for kx in 0..n {
            let (fx, fy) = (if kx <= n / 2 { kx } else { n - kx } as f64, if ky <= n / 2 { ky } else { n - ky } as f64);
            let k = libm::hypot(fx, fy);
            if k < 1.0 || k >= n as f64 {
                continue;
            }
            let o = (libm::log2(k).floor() as usize).min(octaves - 1);
            let (a, b) = (re[ky * n + kx], im[ky * n + kx]);
            sum[o] += (a * a + b * b) / win2;
            cnt[o] += 1;
        }
    }
    let power: Vec<f64> = sum.iter().zip(&cnt).map(|(&s, &c)| if c > 0 { s / c as f64 } else { 0.0 }).collect();
    // Octave o stands at the wavenumber 1.5 * 2^o; log-log least squares from octave 1 on.
    let pts: Vec<(f64, f64)> = power
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, &p)| p > 0.0)
        .map(|(o, &p)| (libm::log10(1.5 * (1u64 << o) as f64), libm::log10(p)))
        .collect();
    Spectrum { n, power, beta: fit(&pts) }
}

/// The least-squares slope of y on x (0 with fewer than two points).
fn fit(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    let m = pts.len() as f64;
    let (sx, sy) = (pts.iter().map(|p| p.0).sum::<f64>(), pts.iter().map(|p| p.1).sum::<f64>());
    let (sxx, sxy) = (pts.iter().map(|p| p.0 * p.0).sum::<f64>(), pts.iter().map(|p| p.0 * p.1).sum::<f64>());
    let d = m * sxx - sx * sx;
    if d.abs() < 1e-12 {
        0.0
    } else {
        (m * sxy - sx * sy) / d
    }
}

/// What the suite reads beyond the grid: the routing's receivers and areas, the river flags, the cell sets, the H1 matches and the per-river-cell ring depths the gate already computed.
pub struct SuiteInput<'a> {
    pub grid: &'a Grid,
    pub start: &'a [f64],
    pub surface: &'a [f64],
    pub receiver: &'a [u32],
    /// Drainage area per cell (m2).
    pub area: &'a [f64],
    pub river: &'a [bool],
    pub inside: &'a [bool],
    pub region: &'a [bool],
    pub sea_level_m: f64,
    /// Per river cell inside (in cell order): the valley depth (ring 1-2 above the cell) and the ring 2-4 depth above the cell.
    pub near_depth: &'a [f64],
    pub ring_depth: &'a [f64],
    /// Matched H1 crossings: (L area m2 at the matching L crossing, H1 discharge m3/s).
    pub pairs: &'a [(f64, f64)],
}

pub fn measure_suite(i: &SuiteInput) -> SuiteReport {
    let (g, z) = (i.grid, i.surface);
    let (w, h) = (g.w(), g.h());
    let (mut steps, mut uphill, mut uphill_max) = (0usize, 0usize, 0f64);
    let (mut wf, mut wh, mut asym) = (Vec::new(), Vec::new(), Vec::new());
    let (mut capped_f, mut capped_h, mut flat_h) = (0usize, 0usize, 0usize);
    let (mut river_len, mut rank) = (0.0, 0usize);
    for c in 0..w * h {
        if !i.river[c] || !i.inside[c] {
            continue;
        }
        let ring = i.ring_depth.get(rank).copied().unwrap_or(0.0);
        rank += 1;
        let r = i.receiver[c];
        if r == NONE {
            continue;
        }
        let (rx, ry) = ((r as usize % w) as f64 - (c % w) as f64, (r as usize / w) as f64 - (c / w) as f64);
        if i.river[r as usize] {
            steps += 1;
            river_len += libm::hypot(rx, ry) * g.dx();
            let rise = z[r as usize] - z[c];
            if rise > UPHILL_M {
                uphill += 1;
                uphill_max = uphill_max.max(rise);
            }
        }
        let Some((nx, ny)) = across(w, c, r) else { continue };
        let base = z[c];
        let width = |depth: f64| {
            let (a, ca) = reach(z, w, h, c, (nx, ny), base, depth);
            let (b, cb) = reach(z, w, h, c, (-nx, -ny), base, depth);
            ((a + b) * g.dx(), usize::from(ca) + usize::from(cb))
        };
        let (m, k) = width(WIDTH_DEPTH_M);
        wf.push(m);
        capped_f += k;
        if ring >= 2.0 * MIN_DEPTH_M {
            let (m, k) = width(0.5 * ring);
            wh.push(m);
            capped_h += k;
        } else {
            flat_h += 1;
        }
        let side = |sign: f64| {
            let vals: Option<Vec<f64>> = (ASYM_CELLS.0..=ASYM_CELLS.1)
                .map(|s| bilinear(z, w, h, (c % w) as f64 + sign * nx * s as f64, (c / w) as f64 + sign * ny * s as f64))
                .collect();
            vals.map(|v| (v.iter().sum::<f64>() / v.len() as f64 - base).max(0.0))
        };
        if let (Some(l), Some(rr)) = (side(1.0), side(-1.0)) {
            if l + rr >= MIN_DEPTH_M {
                asym.push((l - rr).abs() / (l + rr));
            }
        }
    }
    let land_cells = (0..w * h).filter(|&c| i.inside[c] && z[c] > i.sea_level_m).count();
    let (river_km, land_km2) = (river_len / 1000.0, land_cells as f64 * g.dx() * g.dx() / 1.0e6);
    // catchment conservation: the L area per H1 discharge at the matched crossings, against its median
    let implied: Vec<f64> = i.pairs.iter().filter(|p| p.1 > 0.0).map(|p| p.0 / p.1).collect();
    let conservation_pairs = implied.len();
    let implied_runoff = quantile(&mut implied.clone(), 0.5);
    let mut ratios: Vec<f64> = if implied_runoff > 0.0 { implied.iter().map(|v| v / implied_runoff).collect() } else { Vec::new() };
    let (cons_p10, cons_p90) = (quantile(&mut ratios, 0.1), quantile(&mut ratios, 0.9));
    // the seam: height steps between 4-neighbours across the region's edge against those inside it
    let (mut seam, mut inner) = (Vec::new(), Vec::new());
    for c in 0..w * h {
        for n in [(c % w + 1 < w).then(|| c + 1), (c / w + 1 < h).then(|| c + w)].into_iter().flatten() {
            let d = (z[c] - z[n]).abs();
            if i.region[c] != i.region[n] {
                seam.push(d);
            } else if i.region[c] {
                inner.push(d);
            }
        }
    }
    let seam_max = seam.iter().copied().fold(0.0, f64::max);
    let seam_pairs = seam.len();
    let (seam_p50, seam_p99) = (quantile(&mut seam.clone(), 0.5), quantile(&mut seam, 0.99));
    let (in_p50, in_p99) = (quantile(&mut inner.clone(), 0.5), quantile(&mut inner, 0.99));
    // the narrowness
    let ratio: Vec<f64> = i.near_depth.iter().zip(i.ring_depth).filter(|(_, &b)| b >= MIN_DEPTH_M).map(|(&a, &b)| a / b).collect();
    let (np, rp) = (quantile(&mut i.near_depth.to_vec(), 0.5), quantile(&mut i.ring_depth.to_vec(), 0.5));
    // slope against area
    let cell = g.dx() * g.dx();
    let mut bins: Vec<Vec<f64>> = Vec::new();
    for c in 0..w * h {
        let r = i.receiver[c];
        if r == NONE || !i.inside[c] || z[c] <= i.sea_level_m {
            continue;
        }
        let a = i.area[c] / cell;
        if a < SA_MIN_AREA_CELLS {
            continue;
        }
        let (rx, ry) = ((r as usize % w) as f64 - (c % w) as f64, (r as usize / w) as f64 - (c / w) as f64);
        let slope = (z[c] - z[r as usize]) / (libm::hypot(rx, ry) * g.dx());
        if slope <= 0.0 {
            continue;
        }
        let b = (libm::log10(a / SA_MIN_AREA_CELLS) * 2.0) as usize;
        if bins.len() <= b {
            bins.resize(b + 1, Vec::new());
        }
        bins[b].push(slope);
    }
    let slope_area: Vec<SlopeBin> = bins
        .iter_mut()
        .enumerate()
        .map(|(b, v)| SlopeBin {
            area_lo_cells: SA_MIN_AREA_CELLS * libm::pow(10.0, b as f64 / 2.0),
            cells: v.len(),
            slope_p50: quantile(v, 0.5),
        })
        .collect();
    let pts: Vec<(f64, f64)> = slope_area
        .iter()
        .filter(|b| b.cells >= SA_BIN_MIN_CELLS && b.slope_p50 > 0.0)
        .map(|b| (libm::log10(b.area_lo_cells * libm::pow(10.0, 0.25)), libm::log10(b.slope_p50)))
        .collect();
    let theta = -fit(&pts);
    SuiteReport {
        steps,
        uphill_steps: uphill,
        uphill_max_m: uphill_max,
        width_fixed: spread(wf),
        width_fixed_capped: capped_f,
        width_half: spread(wh),
        width_half_capped: capped_h,
        width_half_flat: flat_h,
        asymmetry: spread(asym),
        density_km_per_km2: if land_km2 > 0.0 { river_km / land_km2 } else { 0.0 },
        river_km,
        land_km2,
        conservation_pairs,
        implied_runoff,
        conservation_p10: cons_p10,
        conservation_p90: cons_p90,
        seam_pairs,
        seam_p50_m: seam_p50,
        seam_p99_m: seam_p99,
        seam_max_m: seam_max,
        interior_p50_m: in_p50,
        interior_p99_m: in_p99,
        narrowness: spread(ratio),
        narrowness_of_p50: if rp > 0.0 { np / rp } else { 0.0 },
        slope_area,
        theta,
        spectrum: spectrum(g, z, i.inside),
        spectrum_start: spectrum(g, i.start, i.inside),
    }
}

impl SuiteReport {
    /// The printed lines, one per metric.
    pub fn lines(&self) -> Vec<String> {
        let sa: Vec<String> = self
            .slope_area
            .iter()
            .filter(|b| b.cells > 0)
            .map(|b| format!("{:.0}:{:.4}({})", b.area_lo_cells, b.slope_p50, b.cells))
            .collect();
        let sp = |s: &Spectrum| s.power.iter().map(|p| format!("{p:.3e}")).collect::<Vec<_>>().join(" ");
        vec![
            format!(
                "RIVER-GATE gradient: {} of {} river steps rise by more than {UPHILL_M} m ({:.2} %; largest {:.2} m)",
                self.uphill_steps,
                self.steps,
                100.0 * self.uphill_steps as f64 / self.steps.max(1) as f64,
                self.uphill_max_m
            ),
            format!(
                "RIVER-GATE valley width: at {WIDTH_DEPTH_M} m above the river cell p50 {:.0} m p90 {:.0} m ({} cells, {} sides capped at {WIDTH_MAX_CELLS} cells); at half the ring 2-4 depth p50 {:.0} m p90 {:.0} m ({} cells, {} sides capped, {} cells with a flat ring)",
                self.width_fixed.p50,
                self.width_fixed.p90,
                self.width_fixed.n,
                self.width_fixed_capped,
                self.width_half.p50,
                self.width_half.p90,
                self.width_half.n,
                self.width_half_capped,
                self.width_half_flat
            ),
            format!("RIVER-GATE asymmetry: |L - R| / (L + R) of the rise 2 to 4 cells out: p50 {:.2} p90 {:.2} ({} cells)", self.asymmetry.p50, self.asymmetry.p90, self.asymmetry.n),
            format!("RIVER-GATE drainage density: {:.3} km/km2 ({:.1} km of river over {:.1} km2 of land)", self.density_km_per_km2, self.river_km, self.land_km2),
            format!(
                "RIVER-GATE catchment: {} matched H1 crossings; implied runoff (L area per H1 discharge) {:.3e} m2 per m3/s; the ratio to it p10 {:.2} p90 {:.2}",
                self.conservation_pairs, self.implied_runoff, self.conservation_p10, self.conservation_p90
            ),
            format!(
                "RIVER-GATE seam: {} neighbour pairs across the region's edge, |dz| p50 {:.2} m p99 {:.2} m max {:.2} m; inside the region p50 {:.2} m p99 {:.2} m",
                self.seam_pairs, self.seam_p50_m, self.seam_p99_m, self.seam_max_m, self.interior_p50_m, self.interior_p99_m
            ),
            format!(
                "RIVER-GATE narrowness: valley depth (ring 1-2) over ring 2-4 depth p50 {:.2} p90 {:.2} ({} cells); of the two p50s {:.2}",
                self.narrowness.p50, self.narrowness.p90, self.narrowness.n, self.narrowness_of_p50
            ),
            format!("RIVER-GATE slope-area: theta {:.2} over bins (area in cells: slope p50 (cells)) {}", self.theta, sa.join(" ")),
            format!(
                "RIVER-GATE spectrum: {} x {} cells, power per octave from wavenumber 1 (m2) {}; slope beta {:.2}; the start's {} x {}: {}; beta {:.2}",
                self.spectrum.n,
                self.spectrum.n,
                sp(&self.spectrum),
                self.spectrum.beta,
                self.spectrum_start.n,
                self.spectrum_start.n,
                sp(&self.spectrum_start),
                self.spectrum_start.beta
            ),
        ]
    }
}
