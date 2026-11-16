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
    for (c, &is_in) in inside.iter().enumerate().take(w * h) {
        if is_in {
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

/// The H1 trunk reaches against the surface (BUILD-057, the E4 note's "KR does not cut"): each reach a polyline in the grid's cells, upstream first, sampled every cell.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TrunkReport {
    /// Reaches with at least one vertex inside, and their vertices inside.
    pub reaches: usize,
    pub vertices: usize,
    /// Those whose surface stands more than the bar above the reach's running minimum (the bed a non-increasing profile needs: the burn KR would have to cut).
    pub reaches_over: usize,
    pub vertices_over: usize,
    /// The deepest such vertex: its depth (m), its reach (index in the input) and its position (cells).
    pub deepest_m: f64,
    pub deepest_reach: usize,
    pub deepest_at: (f64, f64),
}

/// Samples each reach every cell along its polyline (bilinear surface), takes the running minimum downstream and counts the vertices inside whose surface stands more than `bar_m` above it.
pub fn trunk_burn(grid: &Grid, z: &[f64], inside: &[bool], reaches: &[Vec<(f64, f64)>], bar_m: f64) -> TrunkReport {
    let (w, h) = (grid.w(), grid.h());
    let mut r = TrunkReport::default();
    for (k, line) in reaches.iter().enumerate() {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for pair in line.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let n = libm::hypot(bx - ax, by - ay).ceil().max(1.0) as usize;
            for s in 0..n {
                let t = s as f64 / n as f64;
                pts.push((ax + t * (bx - ax), ay + t * (by - ay)));
            }
        }
        if let Some(&last) = line.last() {
            pts.push(last);
        }
        let (mut bed, mut seen, mut over) = (f64::INFINITY, 0usize, 0usize);
        for &(x, y) in &pts {
            let Some(v) = bilinear(z, w, h, x, y) else { continue };
            bed = bed.min(v);
            let c = (y.round() as usize).min(h - 1) * w + (x.round() as usize).min(w - 1);
            if !inside[c] {
                continue;
            }
            seen += 1;
            let depth = v - bed;
            if depth > bar_m {
                over += 1;
                if depth > r.deepest_m {
                    (r.deepest_m, r.deepest_reach, r.deepest_at) = (depth, k, (x, y));
                }
            }
        }
        if seen > 0 {
            r.reaches += 1;
            r.vertices += seen;
            r.reaches_over += usize::from(over > 0);
            r.vertices_over += over;
        }
    }
    r
}

/// The corridors of the trunk column (BUILD-057, lead #158): the radius of a reach's corridor in H1 cells (`k_c`; the E4 note §3.3).
pub const CORRIDOR_K: [f64; 3] = [1.0, 2.0, 4.0];

/// A key for the corridor search's heap: the label (higher first), then the cell id (lower first).
#[derive(PartialEq)]
struct Key(f64, usize);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // `BinaryHeap` pops the largest: the higher label, then the lower cell id
        self.0.total_cmp(&o.0).then(o.1.cmp(&self.1))
    }
}

/// The cells of a reach's corridor: the counted cells (`inside`: a store's tiles; the region's box in window mode) within `radius` cells of its polyline (Euclidean,
/// to the segments), as (cell, id in the corridor) and the corridor's size; `None` when an end lies outside the grid. Outside the counted cells a store holds the
/// start surface, not the variant's (lead, DEC explorer-4).
fn corridor(
    w: usize,
    h: usize,
    line: &[(f64, f64)],
    radius: f64,
    inside: &[bool],
) -> Option<(std::collections::HashMap<usize, usize>, Vec<usize>)> {
    let inside_grid = |p: (f64, f64)| p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= (w - 1) as f64 && p.1 <= (h - 1) as f64;
    if line.len() < 2 || !inside_grid(line[0]) || !inside_grid(line[line.len() - 1]) {
        return None;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in line {
        (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
    }
    let clamp = |v: f64, n: usize| (v.max(0.0) as usize).min(n - 1);
    let (bx0, bx1, by0, by1) =
        (clamp((x0 - radius).floor(), w), clamp((x1 + radius).ceil(), w), clamp((y0 - radius).floor(), h), clamp((y1 + radius).ceil(), h));
    let seg_dist = |px: f64, py: f64| {
        line.windows(2)
            .map(|s| {
                let ((ax, ay), (bx, by)) = (s[0], s[1]);
                let (dx, dy) = (bx - ax, by - ay);
                let l2 = dx * dx + dy * dy;
                let t = if l2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                libm::hypot(px - ax - t * dx, py - ay - t * dy)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let (mut index, mut cells) = (std::collections::HashMap::new(), Vec::new());
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            if inside[y * w + x] && seg_dist(x as f64, y as f64) <= radius {
                index.insert(y * w + x, cells.len());
                cells.push(y * w + x);
            }
        }
    }
    Some((index, cells))
}

/// A route inside the corridor from `from` to `to` with every cell at most `d` above the running minimum of the route so far, the minimum starting at
/// `start_label` (`+inf` for a reach's first leg); on success the route (cells, upstream first) and its final running minimum. A higher running minimum leaves
/// every later step at least as much room (a step is allowed when the cell stands at most `d` above it, and the minimum only falls), so a search that keeps each
/// cell's highest reachable minimum, highest first and ties by cell id, is exact (the maximin form of Dijkstra's search). Steps route over land only (`z > sea_m`,
/// #163): a sea cell is never stepped from, and a route that reaches one ends there (the third value is then `true`; the leg's `to` is not reached).
#[allow(clippy::too_many_arguments)]
fn corridor_route(
    w: usize,
    z: &[f64],
    index: &std::collections::HashMap<usize, usize>,
    cells: &[usize],
    from: usize,
    to: usize,
    d: f64,
    start_label: f64,
    sea_m: f64,
) -> Option<(Vec<usize>, f64, bool)> {
    if z[from] - start_label > d {
        return None;
    }
    let mut label = vec![f64::NEG_INFINITY; cells.len()];
    let mut parent = vec![usize::MAX; cells.len()];
    let mut done = vec![false; cells.len()];
    let mut heap = std::collections::BinaryHeap::new();
    let l0 = start_label.min(z[from]);
    label[index[&from]] = l0;
    heap.push(Key(l0, from));
    while let Some(Key(l, c)) = heap.pop() {
        let ci = index[&c];
        if done[ci] || l < label[ci] {
            continue;
        }
        done[ci] = true;
        let sea = z[c] <= sea_m;
        if c == to || sea {
            let mut route = vec![c];
            let mut k = ci;
            while parent[k] != usize::MAX {
                k = parent[k];
                route.push(cells[k]);
            }
            route.reverse();
            return Some((route, l, c != to && sea));
        }
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        for (dx, dy) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= (z.len() / w) as i64 {
                continue;
            }
            let n = ny as usize * w + nx as usize;
            let Some(&ni) = index.get(&n) else { continue };
            if done[ni] || z[n] - l > d {
                continue;
            }
            let nl = l.min(z[n]);
            if nl > label[ni] {
                label[ni] = nl;
                parent[ni] = ci;
                heap.push(Key(nl, n));
            }
        }
    }
    None
}

/// The route through the waypoints (leg after leg, the running minimum carried over), or `None` when a leg has no route within `d`. A leg that meets the sea
/// ends the route there (the later waypoints are dropped).
#[allow(clippy::too_many_arguments)]
fn waypoint_route(
    w: usize,
    z: &[f64],
    index: &std::collections::HashMap<usize, usize>,
    cells: &[usize],
    points: &[usize],
    d: f64,
    sea_m: f64,
) -> Option<Vec<usize>> {
    let (mut route, mut label) = (vec![points[0]], f64::INFINITY);
    for leg in points.windows(2) {
        let (part, l, at_sea) = corridor_route(w, z, index, cells, leg[0], leg[1], d, label, sea_m)?;
        route.extend_from_slice(&part[1..]);
        label = l;
        if at_sea {
            break;
        }
    }
    Some(route)
}

/// The lowest cell (ties by cell id) among the grid cells within `radius` cells of `p` that `keep` admits; `None` when there is none.
fn lowest_in_disc(grid: &Grid, z: &[f64], p: (f64, f64), radius: f64, keep: impl Fn(usize) -> bool) -> Option<usize> {
    let (w, h) = (grid.w() as i64, grid.h() as i64);
    let (x0, x1) = (((p.0 - radius).floor() as i64).max(0), ((p.0 + radius).ceil() as i64).min(w - 1));
    let (y0, y1) = (((p.1 - radius).floor() as i64).max(0), ((p.1 + radius).ceil() as i64).min(h - 1));
    let mut best: Option<usize> = None;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let c = (y * w + x) as usize;
            if libm::hypot(x as f64 - p.0, y as f64 - p.1) > radius || !keep(c) {
                continue;
            }
            if best.is_none_or(|b| z[c] < z[b] || (z[c] == z[b] && c < b)) {
                best = Some(c);
            }
        }
    }
    best
}

/// The snapped form of the corridor measure (#159): a reach's ends (H1's nodes) snap to the lowest L cell within `node_radius` cells, ties by cell id (all reaches
/// meeting at a node share it: the snap depends on the node's position only), and a reach that crosses the region's edge passes through the lowest edge cell
/// (a cell of the region with a 4-neighbour outside it) within `node_radius` cells of the midpoint of H1's crossing step, ties by cell id. Nodes and crossings
/// snap to land cells only (#163); only a mouth (a reach's last point where no reach starts) may snap to the sea. A node with no land cell in its disc snaps
/// to the lowest cell (the route then ends at the sea); a crossing with no land edge cell adds no waypoint.
pub struct Snap<'a> {
    pub node_radius: f64,
    pub region: &'a [bool],
}

/// The corridor form of [`trunk_burn`] (leads #158 and #159): each reach routed inside its corridor of `radius` cells from its first to its last point (the nearest
/// cells, or with `snap` the snapped nodes and the crossing cells) by the route that minimises the largest rise above its running minimum. A reach counts over
/// when no route keeps within `bar_m`; its minimax rise is found to 0.1 m by bisection, and the vertices over the bar along that route are counted (vertices
/// inside only). `vertices` counts the cells of the routes inside. A reach whose end lies outside the grid or its corridor, whose corridor does not join its
/// waypoints, or that has no point inside is skipped. Routes step over land only (`z > sea_level_m`) and end at the first sea cell they meet (#163);
/// `f64::NEG_INFINITY` makes every cell land.
#[allow(clippy::too_many_arguments)]
pub fn trunk_corridor(
    grid: &Grid,
    z: &[f64],
    inside: &[bool],
    reaches: &[Vec<(f64, f64)>],
    radius: f64,
    bar_m: f64,
    sea_level_m: f64,
    snap: Option<&Snap>,
) -> TrunkReport {
    let (w, h) = (grid.w(), grid.h());
    let mut r = TrunkReport::default();
    let cell_of = |p: (f64, f64)| (p.1.round() as usize).min(h - 1) * w + (p.0.round() as usize).min(w - 1);
    let key = |p: (f64, f64)| (p.0.to_bits(), p.1.to_bits());
    let starts: std::collections::HashSet<(u64, u64)> = reaches.iter().filter(|l| !l.is_empty()).map(|l| key(l[0])).collect();
    let land = |c: usize| z[c] > sea_level_m;
    for (k, line) in reaches.iter().enumerate() {
        let Some((index, cells)) = corridor(w, h, line, radius, inside) else { continue };
        if !line.iter().any(|&p| inside[cell_of(p)]) {
            continue;
        }
        let (first, last) = (line[0], line[line.len() - 1]);
        let mut points: Vec<usize> = Vec::new();
        match snap {
            None => points.extend([cell_of(first), cell_of(last)]),
            Some(s) => {
                let at_node = |p: (f64, f64), sea_ok: bool| {
                    let any = lowest_in_disc(grid, z, p, s.node_radius, |_| true);
                    if sea_ok {
                        any
                    } else {
                        lowest_in_disc(grid, z, p, s.node_radius, land).or(any)
                    }
                };
                let Some(a) = at_node(first, false) else { continue };
                points.push(a);
                let region = |c: usize| s.region[c];
                let edge = |c: usize| {
                    let (x, y) = (c % w, c / w);
                    region(c)
                        && land(c)
                        && [(x > 0).then(|| c - 1), (x + 1 < w).then(|| c + 1), (y > 0).then(|| c - w), (y + 1 < h).then(|| c + w)]
                            .into_iter()
                            .flatten()
                            .any(|n| !region(n))
                };
                for pair in line.windows(2) {
                    if region(cell_of(pair[0])) != region(cell_of(pair[1])) {
                        let mid = (0.5 * (pair[0].0 + pair[1].0), 0.5 * (pair[0].1 + pair[1].1));
                        if let Some(x) = lowest_in_disc(grid, z, mid, s.node_radius, edge) {
                            points.push(x);
                        }
                    }
                }
                let Some(b) = at_node(last, !starts.contains(&key(last))) else { continue };
                points.push(b);
                points.dedup();
            }
        }
        if points.len() < 2 || points.iter().any(|p| !index.contains_key(p)) {
            continue;
        }
        // No route rises more than the corridor's height range above its running minimum (the bed may fall below the start first).
        let top = cells.iter().map(|&c| z[c]).fold(f64::NEG_INFINITY, f64::max) - cells.iter().map(|&c| z[c]).fold(f64::INFINITY, f64::min);
        // A reach is over exactly when no route keeps within the bar (every cell of the route counts; the corridor holds counted cells only); its depth is then the
        // minimax rise. The vertex counts are of the route's cells inside.
        let mut failed = false;
        let route = match waypoint_route(w, z, &index, &cells, &points, bar_m, sea_level_m) {
            Some(route) => route,
            None => {
                failed = true;
                // over the bar: bisect the minimax rise in (bar, top]
                let (mut lo, mut hi) = (bar_m, top.max(bar_m));
                // At the height range every step is allowed: no route means the corridor does not join the waypoints (8-neighbour), and the reach is skipped.
                let Some(mut best) = waypoint_route(w, z, &index, &cells, &points, hi, sea_level_m) else { continue };
                while hi - lo > 0.1 {
                    let mid = 0.5 * (lo + hi);
                    match waypoint_route(w, z, &index, &cells, &points, mid, sea_level_m) {
                        Some(route) => (hi, best) = (mid, route),
                        None => lo = mid,
                    }
                }
                best
            }
        };
        let (mut bed, mut seen, mut over) = (f64::INFINITY, 0usize, 0usize);
        let mut deepest = (0.0f64, (0.0, 0.0));
        for &c in &route {
            bed = bed.min(z[c]);
            let depth = z[c] - bed;
            if failed && depth > deepest.0 {
                deepest = (depth, ((c % w) as f64, (c / w) as f64));
            }
            if inside[c] {
                seen += 1;
                over += usize::from(depth > bar_m);
            }
        }
        r.reaches += 1;
        r.vertices += seen;
        r.reaches_over += usize::from(failed);
        r.vertices_over += over;
        if deepest.0 > r.deepest_m {
            (r.deepest_m, r.deepest_reach, r.deepest_at) = (deepest.0, k, deepest.1);
        }
    }
    r
}

/// The corridor rise bar of a window (#159): `min(50, max(10, 0.5 x` the start's p50 relief over 10 x 10 cells`))`, metres.
pub fn corridor_bar_m(relief_10_start_m: f64) -> f64 {
    (0.5 * relief_10_start_m).clamp(10.0, 50.0)
}
