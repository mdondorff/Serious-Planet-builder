//! The horizon map of a chunk (M8-5, GEN-044, SPIKE-32 step 2): per texel and per azimuth the elevation angle of the highest terrain along that
//! azimuth, out to a range, from a raster of heights that includes the halo. The sun's elevation against it decides the shadow; the map does not
//! depend on the sun, so any time of day uses the same map.
//!
//! A pure function of the raster and the parameters (CON-14): one texel's value reads only the raster, in a fixed order, with `libm` for the
//! one transcendental (the final angle); rows are independent, so the result does not depend on the thread count. Positions are relative to the
//! texel (the sample offset is a multiple of the texel size from the texel's integer index), so a texel shared by two chunks gets the same
//! value from either raster whenever both hold the terrain it sees (CON-11: no absolute coordinate is formed).

/// How a horizon is marched and stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizonParams {
    /// Evenly spaced azimuths from north (0), clockwise through east (a quarter of the way round).
    pub azimuths: u32,
    /// How far the march goes, metres.
    pub range_m: f64,
    /// A step is `step_min_m + step_growth * distance`, metres.
    pub step_min_m: f64,
    pub step_growth: f64,
    /// Radius of the body, metres: terrain at a distance `d` appears `d^2 / (2 R)` lower.
    pub radius_m: f64,
}

impl HorizonParams {
    /// SPIKE-32's values: 16 azimuths, 5 km, steps growing 4.5 % plus 2 m.
    pub fn chunk(radius_m: f64) -> HorizonParams {
        HorizonParams { azimuths: 16, range_m: 5_000.0, step_min_m: 2.0, step_growth: 0.045, radius_m }
    }
}

/// A raster of heights in metres, row-major: index `j * n + i` with `i` growing east and `j` growing north, texels `texel_m` apart.
#[derive(Clone, Copy, Debug)]
pub struct HeightRaster<'a> {
    pub n: usize,
    pub texel_m: f64,
    pub heights: &'a [f32],
}

/// A horizon map of a `w x h` block of texels: per texel `azimuths` bytes, the elevation angle quantised by [`quantize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HorizonMap {
    pub w: usize,
    pub h: usize,
    pub azimuths: u32,
    /// Texel-major (row `j` outer, `i`, then azimuth).
    pub data: Vec<u8>,
}

impl HorizonMap {
    /// The stored byte of texel `(i, j)` at azimuth `k`.
    pub fn at(&self, i: usize, j: usize, k: usize) -> u8 {
        self.data[(j * self.w + i) * self.azimuths as usize + k]
    }
}

/// Degrees one byte step stands for: 90 degrees over 255 steps (0.353 degrees, about the shadow's soft edge).
pub const DEGREES_PER_STEP: f64 = 90.0 / 255.0;

/// The byte for an elevation angle in radians: rounded to nearest, 0 for an angle at or below the horizon, 255 at 90 degrees.
pub fn quantize(angle_rad: f64) -> u8 {
    let q = (angle_rad.to_degrees() / DEGREES_PER_STEP + 0.5).floor();
    q.clamp(0.0, 255.0) as u8
}

/// Distance from the surface (metres) up to which the coarse horizon shadows are full (REND-043).
pub const COARSE_FADE_NEAR_M: f64 = 1.0e6;
/// Distance from which they are gone (REND-043): a coarse cell of 19.6 km is under a few pixels there, and its shadow is a spot, not a shape.
pub const COARSE_FADE_FAR_M: f64 = 3.0e6;

// spec: REND-043
/// The strength of the coarse horizon at `distance_m` from the surface point: 1 up to [`COARSE_FADE_NEAR_M`], a smoothstep down to 0 at
/// [`COARSE_FADE_FAR_M`]. The shader's `horizon_coarse_fade` is this function; the chunk horizon is not faded.
pub fn coarse_fade(distance_m: f64) -> f64 {
    let x = ((distance_m - COARSE_FADE_NEAR_M) / (COARSE_FADE_FAR_M - COARSE_FADE_NEAR_M)).clamp(0.0, 1.0);
    1.0 - x * x * (3.0 - 2.0 * x)
}

/// The elevation angle in radians a byte stands for.
pub fn dequantize(q: u8) -> f64 {
    (f64::from(q) * DEGREES_PER_STEP).to_radians()
}

/// Bilinear height at `(i + fx, j + fy)` texels; `None` outside the raster.
fn sample(r: &HeightRaster<'_>, i: i64, j: i64, fx: f64, fy: f64) -> Option<f64> {
    let n = r.n as i64;
    if i < 0 || j < 0 || i + 1 >= n || j + 1 >= n {
        return None;
    }
    let at = |a: i64, b: i64| f64::from(r.heights[(b * n + a) as usize]);
    let (h00, h10, h01, h11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    Some((h00 * (1.0 - fx) + h10 * fx) * (1.0 - fy) + (h01 * (1.0 - fx) + h11 * fx) * fy)
}

/// The distance of every step of the march and the curvature drop at it, `d^2 / (2 R)`: the same for every texel and azimuth, so made once per bake
/// (GEN-085). The steps are the recurrence `d += step_min + growth * d` from one texel, evaluated in the same order as a march that adds them as
/// it goes, so every `d` and every drop is the same double.
fn march_steps(p: &HorizonParams, texel_m: f64) -> Vec<(f64, f64)> {
    let mut steps = Vec::new();
    let mut d = texel_m;
    while d <= p.range_m {
        steps.push((d, d * d / (2.0 * p.radius_m)));
        d += p.step_min_m + p.step_growth * d;
    }
    steps
}

/// The horizon byte of texel `(x, y)` of the raster at azimuth index `k`: the steepest rise (tangent) over the samples along the azimuth
/// out to the range or the raster's edge, as an angle.
fn horizon_at(r: &HeightRaster<'_>, steps: &[(f64, f64)], x: usize, y: usize, dir: (f64, f64)) -> u8 {
    let h0 = f64::from(r.heights[y * r.n + x]);
    let mut best = f64::NEG_INFINITY;
    for &(d, drop) in steps {
        let (sx, sy) = (d * dir.0 / r.texel_m, d * dir.1 / r.texel_m);
        let (ix, iy) = (floor_i64(sx), floor_i64(sy));
        let Some(h) = sample(r, x as i64 + ix, y as i64 + iy, sx - ix as f64, sy - iy as f64) else { break };
        let rise = h - h0 - drop;
        best = best.max(rise / d);
    }
    if best.is_finite() {
        quantize(libm::atan(best))
    } else {
        0
    }
}

/// `floor` as an integer: exact (the cast truncates toward zero, one step down fixes a negative fraction), and the arguments are at most a
/// million texels, far inside `i64`.
fn floor_i64(v: f64) -> i64 {
    let t = v as i64;
    if (t as f64) > v {
        t - 1
    } else {
        t
    }
}

/// How the march of a texel runs: one azimuth at a time, or four azimuths at once on AVX2 (GEN-086). Both do the same IEEE operations in the
/// same order on each azimuth, so they give the same bytes; the scalar one is the fallback of a machine without AVX2 and what the other is tested against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    Scalar,
    #[cfg(target_arch = "x86_64")]
    Avx2,
}

impl Kernel {
    /// The fastest kernel this machine runs.
    pub fn detect() -> Kernel {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            return Kernel::Avx2;
        }
        Kernel::Scalar
    }
}

/// Four azimuths of one texel on AVX2: the lanes of [`horizon_at`], operation for operation. A lane whose sample leaves the raster stops, as the
/// scalar march breaks. `idx` of a stopped lane is 0, so the gather stays inside the raster (`n >= 2` and `(n + 1)^2 <= i32::MAX`, checked by the caller).
///
/// # Safety
/// The CPU must support AVX2 and `r.heights` must hold `r.n * r.n` values (`bake_horizon_with` checks both before any texel).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn horizon_at4(r: &HeightRaster<'_>, steps: &[(f64, f64)], x: usize, y: usize, dirs: &[(f64, f64); 4]) -> [u8; 4] {
    use std::arch::x86_64::*;
    let n = r.n as i32;
    let h0 = _mm256_set1_pd(f64::from(r.heights[y * r.n + x]));
    let dx = _mm256_set_pd(dirs[3].0, dirs[2].0, dirs[1].0, dirs[0].0);
    let dy = _mm256_set_pd(dirs[3].1, dirs[2].1, dirs[1].1, dirs[0].1);
    let (texel, one) = (_mm256_set1_pd(r.texel_m), _mm256_set1_pd(1.0));
    let mut best = _mm256_set1_pd(f64::NEG_INFINITY);
    let mut alive = _mm256_castsi256_pd(_mm256_set1_epi64x(-1));
    let (xi, yi, nn) = (_mm_set1_epi32(x as i32), _mm_set1_epi32(y as i32), _mm_set1_epi32(n));
    for &(d, drop) in steps {
        let dd = _mm256_set1_pd(d);
        let sx = _mm256_div_pd(_mm256_mul_pd(dd, dx), texel);
        let sy = _mm256_div_pd(_mm256_mul_pd(dd, dy), texel);
        let (fx_, fy_) = (_mm256_floor_pd(sx), _mm256_floor_pd(sy));
        let ix = _mm_add_epi32(xi, _mm256_cvttpd_epi32(fx_));
        let iy = _mm_add_epi32(yi, _mm256_cvttpd_epi32(fy_));
        // Inside the raster as `sample` asks: 0 <= i and i + 1 < n on both axes.
        let low = _mm_and_si128(_mm_cmpgt_epi32(ix, _mm_set1_epi32(-1)), _mm_cmpgt_epi32(iy, _mm_set1_epi32(-1)));
        let high = _mm_and_si128(
            _mm_cmpgt_epi32(nn, _mm_add_epi32(ix, _mm_set1_epi32(1))),
            _mm_cmpgt_epi32(nn, _mm_add_epi32(iy, _mm_set1_epi32(1))),
        );
        let inside = _mm_and_si128(low, high);
        alive = _mm256_and_pd(alive, _mm256_castsi256_pd(_mm256_cvtepi32_epi64(inside)));
        if _mm256_movemask_pd(alive) == 0 {
            break;
        }
        let idx = _mm_add_epi32(_mm_mullo_epi32(_mm_and_si128(iy, inside), nn), _mm_and_si128(ix, inside));
        let at = |o: i32| _mm256_cvtps_pd(_mm_i32gather_ps::<4>(r.heights.as_ptr(), _mm_add_epi32(idx, _mm_set1_epi32(o))));
        let (h00, h10, h01, h11) = (at(0), at(1), at(n), at(n + 1));
        let (fx, fy) = (_mm256_sub_pd(sx, fx_), _mm256_sub_pd(sy, fy_));
        let (ofx, ofy) = (_mm256_sub_pd(one, fx), _mm256_sub_pd(one, fy));
        let a = _mm256_add_pd(_mm256_mul_pd(h00, ofx), _mm256_mul_pd(h10, fx));
        let b = _mm256_add_pd(_mm256_mul_pd(h01, ofx), _mm256_mul_pd(h11, fx));
        let h = _mm256_add_pd(_mm256_mul_pd(a, ofy), _mm256_mul_pd(b, fy));
        let rise = _mm256_sub_pd(_mm256_sub_pd(h, h0), _mm256_set1_pd(drop));
        // `f64::max` ignores a NaN operand: `_mm256_max_pd(a, b)` returns `b` when either is NaN, so the candidate goes first.
        let next = _mm256_max_pd(_mm256_div_pd(rise, dd), best);
        best = _mm256_blendv_pd(best, next, alive);
    }
    let mut lanes = [0f64; 4];
    _mm256_storeu_pd(lanes.as_mut_ptr(), best);
    lanes.map(|b| if b.is_finite() { quantize(libm::atan(b)) } else { 0 })
}

/// The horizon bytes of one texel at every azimuth into `out`.
fn texel_bytes(r: &HeightRaster<'_>, steps: &[(f64, f64)], x: usize, y: usize, dirs: &[(f64, f64)], kernel: Kernel, out: &mut [u8]) {
    debug_assert_eq!(r.heights.len(), r.n * r.n, "the gather reads inside n x n heights");
    let mut k = 0;
    #[cfg(target_arch = "x86_64")]
    if kernel == Kernel::Avx2 && r.n >= 2 && (r.n + 1).saturating_mul(r.n + 1) <= i32::MAX as usize {
        while k + 4 <= dirs.len() {
            let four = [dirs[k], dirs[k + 1], dirs[k + 2], dirs[k + 3]];
            // SAFETY: `bake_horizon_with` lets `Kernel::Avx2` through only where `Kernel::detect` says the CPU has AVX2; `texel_bytes` is private.
            out[k..k + 4].copy_from_slice(&unsafe { horizon_at4(r, steps, x, y, &four) });
            k += 4;
        }
    }
    let _ = kernel;
    for (j, d) in dirs.iter().enumerate().skip(k) {
        out[j] = horizon_at(r, steps, x, y, *d);
    }
}

/// The unit direction of azimuth `k` of `a` as `(east, north)` in the raster's frame (`i` east, `j` north). Components that are only the
/// rounding of an exact zero (the cosine of a quarter turn is 6e-17) are snapped to 0, so a cardinal azimuth marches along a row or a column
/// exactly and reads no neighbouring row with a weight of 1e-16.
fn azimuth_dir(k: u32, a: u32) -> (f64, f64) {
    let az = core::f64::consts::TAU * f64::from(k) / f64::from(a);
    let snap = |c: f64| if c.abs() < 1e-12 { 0.0 } else { c };
    (snap(libm::sin(az)), snap(libm::cos(az)))
}

/// Texels of raster the march of `p` can reach beyond a texel on every side: the last sample is at most `range / texel` texels away, the
/// bilinear sample reads one texel further, and a sample on the far edge needs its neighbour (one more). A block closer to the raster's edge
/// than this has a horizon cut short.
pub fn halo_texels(p: &HorizonParams, texel_m: f64) -> usize {
    ((p.range_m / texel_m).ceil() as usize).saturating_add(2)
}

/// The horizon map of the block `[x0, x0 + w) x [y0, y0 + h)` of `raster` (the texels whose value the caller keeps; the rest of the raster is
/// the halo), on `threads` threads (rows are independent: the result does not depend on the count, CON-14). Refused with the cause named: a
/// raster whose size does not match, a block outside it, a block with less than [`halo_texels`] of raster on any side (the horizon of its
/// edge texels would be cut short, and two chunks would disagree at their border), and parameters that are not finite, positive and sensible.
pub fn bake_horizon(
    raster: &HeightRaster<'_>,
    block: (usize, usize, usize, usize),
    p: &HorizonParams,
    threads: usize,
) -> Result<HorizonMap, String> {
    bake_horizon_with(raster, block, p, threads, Kernel::detect())
}

/// [`bake_horizon`] on a given kernel (the tests run both; the bytes are the same).
pub fn bake_horizon_with(
    raster: &HeightRaster<'_>,
    block: (usize, usize, usize, usize),
    p: &HorizonParams,
    threads: usize,
    kernel: Kernel,
) -> Result<HorizonMap, String> {
    // A kernel the CPU cannot run is the scalar one (the enum is public: the feature test is made here, once, not trusted from the caller).
    let kernel = if kernel == Kernel::detect() || kernel == Kernel::Scalar { kernel } else { Kernel::Scalar };
    let (x0, y0, w, h) = block;
    let n = raster.n;
    if n == 0 || n.checked_mul(n) != Some(raster.heights.len()) {
        return Err(format!("the raster holds {} heights, not {n} x {n}", raster.heights.len()));
    }
    let inside = |o: usize, len: usize| o.checked_add(len).is_some_and(|e| e <= n);
    if w == 0 || h == 0 || !inside(x0, w) || !inside(y0, h) {
        return Err(format!("the block {w} x {h} at ({x0}, {y0}) is not inside the {n} x {n} raster"));
    }
    let finite_positive = |v: f64| v.is_finite() && v > 0.0;
    if p.azimuths == 0
        || !finite_positive(p.range_m)
        || !finite_positive(p.step_min_m)
        || !finite_positive(p.radius_m)
        || !(p.step_growth.is_finite() && p.step_growth >= 0.0)
        || !finite_positive(raster.texel_m)
        || p.step_min_m < 1e-3 * raster.texel_m
        || p.range_m > 1.0e7
        || p.range_m / raster.texel_m > 1.0e6
    {
        return Err(format!(
            "bad horizon parameters {p:?} / texel {} m: finite and positive, a step of at least a thousandth of a texel, a range of at most 10,000 km and a million texels",
            raster.texel_m
        ));
    }
    let halo = halo_texels(p, raster.texel_m);
    let (west, south, east, north) = (x0, y0, n - x0 - w, n - y0 - h);
    if west.min(south).min(east).min(north) < halo {
        return Err(format!(
            "the block {w} x {h} at ({x0}, {y0}) has {west} / {south} / {east} / {north} texels of halo (west / south / east / north) in the {n} x {n} raster, but a range of {} m at {} m texels needs {halo}",
            p.range_m, raster.texel_m
        ));
    }
    let a = p.azimuths as usize;
    let steps = march_steps(p, raster.texel_m);
    let dirs: Vec<(f64, f64)> = (0..p.azimuths).map(|k| azimuth_dir(k, p.azimuths)).collect();
    let mut data = vec![0u8; w * h * a];
    let row_len = w * a;
    let rows_per = h.div_ceil(threads.clamp(1, h)).max(1);
    let parts: Vec<std::sync::Mutex<&mut [u8]>> = data.chunks_mut(rows_per * row_len).map(std::sync::Mutex::new).collect();
    planet_core::par::Par::run(&threads, parts.len(), &|c| {
        let mut part = parts[c].lock().expect("a horizon chunk");
        for (r, row) in part.chunks_mut(row_len).enumerate() {
            let y = y0 + c * rows_per + r;
            for i in 0..w {
                texel_bytes(raster, &steps, x0 + i, y, &dirs, kernel, &mut row[i * a..(i + 1) * a]);
            }
        }
    });
    drop(parts);
    Ok(HorizonMap { w, h, azimuths: p.azimuths, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;
    const TEXEL: f64 = 4.0;

    fn short(range_m: f64) -> HorizonParams {
        HorizonParams { range_m, ..HorizonParams::chunk(R) }
    }

    /// A flat raster with a band of `height` metres: columns `[at, at + width)` when `columns`, else rows.
    fn band(n: usize, at: usize, width: usize, height: f32, columns: bool) -> Vec<f32> {
        (0..n * n)
            .map(|k| {
                let c = if columns { k % n } else { k / n };
                if (at..at + width).contains(&c) {
                    height
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The bounds the byte at distance `d` from the face of a ridge of `height` must lie in: the first sample on the ridge is at most one step
    /// and one texel (the bilinear ramp) past its face.
    fn bounds(p: &HorizonParams, height: f32, d: f64) -> (u8, u8) {
        let step = p.step_min_m + p.step_growth * d;
        let curvature = d * d / (2.0 * p.radius_m);
        let lo = quantize(libm::atan((f64::from(height) - curvature) / (d + step + TEXEL)));
        let hi = quantize(libm::atan(f64::from(height) / (d - TEXEL)));
        (lo.saturating_sub(1), hi.saturating_add(1))
    }

    // spec: GEN-044
    #[test]
    fn a_ridge_casts_the_angle_of_its_height_over_its_distance_toward_it_and_none_away_from_it() {
        let (n, at, width, height) = (300usize, 150usize, 8usize, 100.0f32);
        let p = short(200.0);
        let (east, west, north, south) = (4usize, 12usize, 0usize, 8usize);
        // A ridge to the east (along columns): texels west of it see it toward the east and nothing toward the west.
        let heights = band(n, at, width, height, true);
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let map = bake_horizon(&raster, (60, 100, 100, 8), &p, 2).unwrap();
        for x in [104usize, 130, 146] {
            let d = (at - x) as f64 * TEXEL;
            let (lo, hi) = bounds(&p, height, d);
            let q = map.at(x - 60, 3, east);
            assert!((lo..=hi).contains(&q), "x {x}: {q} not in [{lo}, {hi}]");
            assert_eq!(map.at(x - 60, 3, west), 0, "x {x}: flat plain to the west");
            assert_eq!((map.at(x - 60, 3, north), map.at(x - 60, 3, south)), (0, 0), "x {x}: nothing north or south of a column ridge");
        }
        // A texel just east of the ridge looks away from it: nothing eastward, a steep wall westward.
        let after = at + width + 1 - 60;
        assert_eq!(map.at(after, 3, east), 0);
        assert!(map.at(after, 3, west) > 200, "the wall beside it is steeper than 70 degrees: {}", map.at(after, 3, west));
        // A ridge to the north (along rows) is seen toward k = 0 and not toward k = 8: j grows north.
        let heights = band(n, at, width, height, false);
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let map = bake_horizon(&raster, (100, 60, 8, 100), &p, 2).unwrap();
        for y in [104usize, 130, 146] {
            let d = (at - y) as f64 * TEXEL;
            let (lo, hi) = bounds(&p, height, d);
            let q = map.at(3, y - 60, north);
            assert!((lo..=hi).contains(&q), "y {y}: {q} not in [{lo}, {hi}]");
            assert_eq!(map.at(3, y - 60, south), 0, "y {y}: nothing to the south");
            assert_eq!((map.at(3, y - 60, east), map.at(3, y - 60, west)), (0, 0));
        }
    }

    /// The march as it was before the step table (GEN-085): the distance and its curvature drop made as it goes, `libm::floor` for the sample.
    fn reference_bake(r: &HeightRaster<'_>, block: (usize, usize, usize, usize), p: &HorizonParams) -> Vec<u8> {
        let (x0, y0, w, h) = block;
        let mut out = Vec::new();
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                for k in 0..p.azimuths {
                    let dir = azimuth_dir(k, p.azimuths);
                    let h0 = f64::from(r.heights[y * r.n + x]);
                    let (mut best, mut d) = (f64::NEG_INFINITY, r.texel_m);
                    while d <= p.range_m {
                        let (sx, sy) = (d * dir.0 / r.texel_m, d * dir.1 / r.texel_m);
                        let (ix, iy) = (libm::floor(sx), libm::floor(sy));
                        let Some(hh) = sample(r, x as i64 + ix as i64, y as i64 + iy as i64, sx - ix, sy - iy) else { break };
                        let rise = hh - h0 - d * d / (2.0 * p.radius_m);
                        best = best.max(rise / d);
                        d += p.step_min_m + p.step_growth * d;
                    }
                    out.push(if best.is_finite() { quantize(libm::atan(best)) } else { 0 });
                }
            }
        }
        out
    }

    // spec: GEN-085
    #[test]
    fn the_step_table_march_equals_the_march_that_adds_its_steps_as_it_goes_byte_for_byte() {
        // Rough terrain with ridges and a plain, a texel that is not a power of two, a body small enough for the curvature to matter, and the
        // default 16 azimuths (the cardinal ones snap to a row or a column), at three ranges and two growths.
        let n = 330usize;
        let heights: Vec<f32> = (0..n * n)
            .map(|k| {
                let (i, j) = ((k % n) as f64, (k / n) as f64);
                (60.0 * libm::sin(0.11 * i) * libm::cos(0.07 * j) + 25.0 * libm::sin(0.31 * i + 0.17 * j) + 0.013 * i * j.sqrt()) as f32
            })
            .collect();
        let texel = 8.037;
        let raster = HeightRaster { n, texel_m: texel, heights: &heights };
        for (range, growth, radius) in [(300.0, 0.045, 20_000.0), (1_200.0, 0.045, R), (1_200.0, 0.0, 50_000.0)] {
            let p = HorizonParams { range_m: range, step_growth: growth, radius_m: radius, ..HorizonParams::chunk(R) };
            let halo = halo_texels(&p, texel);
            let block = (halo, halo, n - 2 * halo, n - 2 * halo);
            let got = bake_horizon(&raster, block, &p, 3).unwrap();
            assert_eq!(got.data, reference_bake(&raster, block, &p), "range {range} growth {growth} radius {radius}");
            assert!(got.data.iter().any(|&q| q > 0) && got.data.contains(&0), "the fixture shades some texels and not others");
        }
    }

    // spec: GEN-086
    #[test]
    fn the_avx2_kernel_gives_the_bytes_of_the_scalar_one_including_rays_that_leave_the_raster() {
        let n = 330usize;
        let heights: Vec<f32> = (0..n * n)
            .map(|k| {
                let (i, j) = ((k % n) as f64, (k / n) as f64);
                (60.0 * libm::sin(0.11 * i) * libm::cos(0.07 * j) + 25.0 * libm::sin(0.31 * i + 0.17 * j)) as f32
            })
            .collect();
        let raster = HeightRaster { n, texel_m: 8.037, heights: &heights };
        #[allow(unused_mut)]
        let mut kernels = vec![Kernel::Scalar];
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            kernels.push(Kernel::Avx2);
        }
        eprintln!("kernels under test: {kernels:?}");
        // The default 16 azimuths and counts that are not a multiple of four.
        for azimuths in [16u32, 7, 4, 1] {
            let p = HorizonParams { azimuths, range_m: 1_200.0, ..HorizonParams::chunk(R) };
            let halo = halo_texels(&p, 8.037);
            let block = (halo, halo, n - 2 * halo, n - 2 * halo);
            let want = bake_horizon_with(&raster, block, &p, 2, Kernel::Scalar).unwrap();
            for &k in &kernels {
                assert_eq!(bake_horizon_with(&raster, block, &p, 3, k).unwrap(), want, "{k:?}, {azimuths} azimuths");
            }
        }
        // Rays that run off the raster: texels near the edge, which `bake_horizon` refuses for want of halo, through the texel kernel.
        let p = HorizonParams { range_m: 2_000.0, ..HorizonParams::chunk(R) };
        let steps = march_steps(&p, 8.037);
        let dirs: Vec<(f64, f64)> = (0..p.azimuths).map(|k| azimuth_dir(k, p.azimuths)).collect();
        for &(x, y) in &[(0usize, 0usize), (3, 200), (n - 2, 5), (n - 1, n - 1), (160, 160), (n - 40, 17)] {
            let mut want = vec![0u8; dirs.len()];
            texel_bytes(&raster, &steps, x, y, &dirs, Kernel::Scalar, &mut want);
            for &k in &kernels {
                let mut got = vec![0u8; dirs.len()];
                texel_bytes(&raster, &steps, x, y, &dirs, k, &mut got);
                assert_eq!(got, want, "{k:?} at ({x}, {y})");
            }
        }
        // A raster of NaN heights: no candidate, the same byte on every kernel.
        let nan = vec![f32::NAN; 64 * 64];
        let r = HeightRaster { n: 64, texel_m: 8.0, heights: &nan };
        let mut want = vec![9u8; dirs.len()];
        texel_bytes(&r, &steps, 30, 30, &dirs, Kernel::Scalar, &mut want);
        for &k in &kernels {
            let mut got = vec![9u8; dirs.len()];
            texel_bytes(&r, &steps, 30, 30, &dirs, k, &mut got);
            assert_eq!(got, want, "{k:?} on NaN");
        }
    }

    // spec: GEN-044
    #[test]
    fn the_result_does_not_depend_on_the_thread_count_and_is_a_pure_function() {
        let n = 200;
        let heights: Vec<f32> =
            (0..n * n).map(|k| 30.0 * libm::sin(0.13 * (k % n) as f64) as f32 * libm::cos(0.07 * (k / n) as f64) as f32 + 5.0).collect();
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = short(100.0);
        let one = bake_horizon(&raster, (40, 40, 60, 50), &p, 1).unwrap();
        assert!(one.data.iter().any(|&q| q > 0), "the terrain shades something");
        for threads in [2, 3, 7, 64] {
            assert_eq!(bake_horizon(&raster, (40, 40, 60, 50), &p, threads).unwrap(), one, "{threads} threads");
        }
        assert_eq!(bake_horizon(&raster, (40, 40, 60, 50), &p, 1).unwrap(), one);
    }

    // spec: GEN-044
    #[test]
    fn texels_shared_by_two_chunks_agree_when_both_rasters_hold_the_halo_and_a_short_halo_is_refused() {
        let n = 260usize;
        let heights: Vec<f32> =
            (0..n * n).map(|k| 40.0 * (libm::sin(0.09 * (k % n) as f64) + libm::cos(0.11 * (k / n) as f64)) as f32).collect();
        let big = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = short(100.0);
        let halo = halo_texels(&p, TEXEL);
        assert_eq!(halo, 27, "25 texels of range, one for the bilinear sample, one for the sample on the edge");
        let want = bake_horizon(&big, (80, 80, 40, 40), &p, 2).unwrap();
        // The sub-raster that holds exactly the halo around the block gives the same texels bit for bit...
        let m = 40 + 2 * halo;
        let origin = 80 - halo;
        let sub: Vec<f32> = (0..m * m).map(|k| heights[(origin + k / m) * n + origin + k % m]).collect();
        let small = HeightRaster { n: m, texel_m: TEXEL, heights: &sub };
        assert_eq!(bake_horizon(&small, (halo, halo, 40, 40), &p, 3).unwrap(), want);
        // ...and one texel less of it on any side is refused, naming the shortfall.
        for (x0, y0, w, h) in [(halo - 1, halo, 40, 40), (halo, halo - 1, 40, 40), (halo, halo, 41, 40), (halo, halo, 40, 41)] {
            let e = bake_horizon(&small, (x0, y0, w, h), &p, 1).unwrap_err();
            assert!(e.contains("halo") && e.contains(&format!("needs {halo}")), "{e}");
        }
    }

    // spec: GEN-044
    #[test]
    fn the_curvature_of_the_body_lowers_distant_terrain_by_d_squared_over_two_r() {
        // A body of radius 20 km: terrain 1 km away is 25 m lower than the plane through the observer.
        let small = 20_000.0f64;
        let p = HorizonParams { range_m: 1_500.0, ..HorizonParams::chunk(small) };
        let halo = halo_texels(&p, TEXEL);
        let n = 2 * halo + 1 + 300;
        let (x, y) = (halo, halo);
        let ridge_at = x + 250; // 1 km east
        let drop = 1_000.0f64 * 1_000.0 / (2.0 * small);
        for factor in [0.8f64, 2.0] {
            let hr = (drop * factor) as f32;
            let heights = band(n, ridge_at, 40, hr, true);
            let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
            let got = bake_horizon(&raster, (x, y, 1, 1), &p, 1).unwrap().at(0, 0, 4);
            // The same march by hand over the ridge's profile along the row (a one-texel ramp up and down).
            let profile = |sx: f64| -> f64 {
                let c = sx + x as f64 - ridge_at as f64;
                let w = if c < 0.0 {
                    0.0
                } else if c < 1.0 {
                    c
                } else if c < 39.0 {
                    1.0
                } else if c < 40.0 {
                    40.0 - c
                } else {
                    0.0
                };
                f64::from(hr) * w
            };
            let (mut best, mut d) = (f64::NEG_INFINITY, TEXEL);
            while d <= p.range_m {
                best = best.max((profile(d / TEXEL) - d * d / (2.0 * small)) / d);
                d += p.step_min_m + p.step_growth * d;
            }
            let want = quantize(libm::atan(best));
            assert!(got.abs_diff(want) <= 1, "ridge at {factor} x the drop: {got} against {want}");
            if factor < 1.0 {
                assert_eq!(got, 0, "a ridge lower than the curvature drop is below the horizon");
            } else {
                assert!(got >= 3, "twice the drop rises 1.4 degrees: {got}");
            }
        }
    }

    // spec: GEN-044
    #[test]
    fn bad_input_is_refused_with_the_cause_and_the_byte_conversion_is_exact_at_its_edges() {
        let p = short(100.0);
        let heights = [0.0f32; 16];
        let e = bake_horizon(&HeightRaster { n: 4, texel_m: TEXEL, heights: &heights[..3] }, (0, 0, 1, 1), &p, 1).unwrap_err();
        assert!(e.contains("3 heights, not 4 x 4"), "{e}");
        let n = 80;
        let flat = vec![0.0f32; n * n];
        let r = HeightRaster { n, texel_m: TEXEL, heights: &flat };
        let block = (30, 30, 20, 20);
        assert!(bake_horizon(&r, (usize::MAX, 0, 1, 1), &p, 1).unwrap_err().contains("not inside"));
        assert!(bake_horizon(&r, (0, 0, 0, 1), &p, 1).unwrap_err().contains("not inside"));
        for bad in [
            HorizonParams { azimuths: 0, ..p },
            HorizonParams { range_m: f64::INFINITY, ..p },
            HorizonParams { range_m: f64::NAN, ..p },
            HorizonParams { step_min_m: 1e-20, step_growth: 0.0, ..p },
            HorizonParams { step_growth: f64::NAN, ..p },
            HorizonParams { radius_m: 0.0, ..p },
            HorizonParams { radius_m: -1.0, ..p },
            HorizonParams { radius_m: f64::NAN, ..p },
        ] {
            assert!(bake_horizon(&r, block, &bad, 1).unwrap_err().contains("bad horizon parameters"), "{bad:?}");
        }
        for texel in [f64::INFINITY, 0.0, f64::NAN, 1e-30] {
            let r = HeightRaster { n, texel_m: texel, heights: &flat };
            assert!(bake_horizon(&r, block, &p, 1).is_err(), "texel {texel}");
        }
        assert_eq!(bake_horizon(&r, block, &p, 1).unwrap().data.len(), 20 * 20 * 16);
        // The byte: 0 at and below the horizon (and for a missing angle), 255 at the zenith, a round trip on every byte, rounding at the half step.
        assert_eq!((quantize(-0.3), quantize(0.0), quantize(f64::NAN), quantize(core::f64::consts::FRAC_PI_2)), (0, 0, 0, 255));
        assert!((0..=255u8).all(|q| quantize(dequantize(q)) == q));
        let half = (DEGREES_PER_STEP * 0.5).to_radians();
        assert_eq!((quantize(half * 0.999), quantize(half * 1.001)), (0, 1));
    }

    // spec: REND-043
    #[test]
    fn the_coarse_strength_is_one_near_zero_far_and_a_monotone_smoothstep_between() {
        assert_eq!(
            (coarse_fade(0.0), coarse_fade(COARSE_FADE_NEAR_M), coarse_fade(COARSE_FADE_FAR_M), coarse_fade(1.0e9)),
            (1.0, 1.0, 0.0, 0.0)
        );
        let mid = 0.5 * (COARSE_FADE_NEAR_M + COARSE_FADE_FAR_M);
        assert!((coarse_fade(mid) - 0.5).abs() < 1e-12, "the middle is one half");
        let d: Vec<f64> = (0..=40).map(|k| COARSE_FADE_NEAR_M + (COARSE_FADE_FAR_M - COARSE_FADE_NEAR_M) * f64::from(k) / 40.0).collect();
        assert!(d.windows(2).all(|w| coarse_fade(w[1]) <= coarse_fade(w[0])), "monotone");
        // The owner's frame was taken at 3,904 km: the coarse shadows are gone there.
        assert_eq!(coarse_fade(3.904e6), 0.0);
    }
}
