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

/// The horizon byte of texel `(x, y)` of the raster at azimuth index `k`: the steepest rise (tangent) over the samples along the azimuth
/// out to the range or the raster's edge, as an angle.
fn horizon_at(r: &HeightRaster<'_>, p: &HorizonParams, x: usize, y: usize, dir: (f64, f64)) -> u8 {
    let h0 = f64::from(r.heights[y * r.n + x]);
    let (mut best, mut d) = (f64::NEG_INFINITY, r.texel_m);
    while d <= p.range_m {
        let (sx, sy) = (d * dir.0 / r.texel_m, d * dir.1 / r.texel_m);
        let (ix, iy) = (libm::floor(sx), libm::floor(sy));
        let Some(h) = sample(r, x as i64 + ix as i64, y as i64 + iy as i64, sx - ix, sy - iy) else { break };
        let rise = h - h0 - d * d / (2.0 * p.radius_m);
        best = best.max(rise / d);
        d += p.step_min_m + p.step_growth * d;
    }
    if best.is_finite() {
        quantize(libm::atan(best))
    } else {
        0
    }
}

/// The unit direction of azimuth `k` of `a` as `(east, north)`.
fn azimuth_dir(k: u32, a: u32) -> (f64, f64) {
    let az = core::f64::consts::TAU * f64::from(k) / f64::from(a);
    (libm::sin(az), libm::cos(az))
}

/// The horizon map of the block `[x0, x0 + w) x [y0, y0 + h)` of `raster` (the texels whose value the caller keeps; the rest of the raster is
/// the halo), on `threads` threads (rows are independent: the result does not depend on the count, CON-14). A block outside the raster is an
/// error naming it.
pub fn bake_horizon(
    raster: &HeightRaster<'_>,
    block: (usize, usize, usize, usize),
    p: &HorizonParams,
    threads: usize,
) -> Result<HorizonMap, String> {
    let (x0, y0, w, h) = block;
    if raster.heights.len() != raster.n * raster.n || raster.n == 0 {
        return Err(format!("the raster holds {} heights, not {0} x {0}", raster.heights.len()).replace("{0}", &raster.n.to_string()));
    }
    if w == 0 || h == 0 || x0 + w > raster.n || y0 + h > raster.n {
        return Err(format!("the block {w} x {h} at ({x0}, {y0}) is not inside the {} x {} raster", raster.n, raster.n));
    }
    if p.azimuths == 0 || !(p.range_m > 0.0) || !(p.step_min_m > 0.0) || !(p.step_growth >= 0.0) || !(raster.texel_m > 0.0) {
        return Err(format!("bad horizon parameters {p:?} / texel {} m", raster.texel_m));
    }
    let a = p.azimuths as usize;
    let dirs: Vec<(f64, f64)> = (0..p.azimuths).map(|k| azimuth_dir(k, p.azimuths)).collect();
    let mut data = vec![0u8; w * h * a];
    let row_len = w * a;
    let rows_per = h.div_ceil(threads.clamp(1, h)).max(1);
    std::thread::scope(|s| {
        for (c, part) in data.chunks_mut(rows_per * row_len).enumerate() {
            let dirs = &dirs;
            s.spawn(move || {
                for (r, row) in part.chunks_mut(row_len).enumerate() {
                    let y = y0 + c * rows_per + r;
                    for i in 0..w {
                        for (k, d) in dirs.iter().enumerate() {
                            row[i * a + k] = horizon_at(raster, p, x0 + i, y, *d);
                        }
                    }
                }
            });
        }
    });
    Ok(HorizonMap { w, h, azimuths: p.azimuths, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;
    const TEXEL: f64 = 4.0;

    /// A flat raster of `n x n` texels with a ridge of `height` metres, `width` texels wide, from column `col`.
    fn ridge(n: usize, col: usize, width: usize, height: f32) -> Vec<f32> {
        (0..n * n).map(|k| if (col..col + width).contains(&(k % n)) { height } else { 0.0 }).collect()
    }

    fn params(range_m: f64) -> HorizonParams {
        HorizonParams { range_m, ..HorizonParams::chunk(R) }
    }

    // spec: GEN-044
    #[test]
    fn a_ridge_casts_the_angle_of_its_height_over_its_distance_toward_it_and_none_away_from_it() {
        let (n, col, width, height) = (128usize, 80usize, 4usize, 100.0f32);
        let heights = ridge(n, col, width, height);
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = params(2_000.0);
        let map = bake_horizon(&raster, (0, 60, 100, 8), &p, 2).unwrap();
        let east = (p.azimuths / 4) as usize;
        let west = (3 * p.azimuths / 4) as usize;
        for x in [0usize, 20, 50, 70] {
            let d = (col - x) as f64 * TEXEL;
            let step = p.step_min_m + p.step_growth * d;
            let toward = f64::from(map.at(x, 3, east));
            // The sample that first meets the ridge lies at most a step (and the ramp of one texel) beyond its face.
            let lo = quantize(libm::atan(f64::from(height) / (d + step + TEXEL) - d * d / (2.0 * R) / d));
            let hi = quantize(libm::atan(f64::from(height) / (d - TEXEL)));
            assert!(toward >= f64::from(lo) - 1.0 && toward <= f64::from(hi) + 1.0, "x {x}: {toward} not in [{lo}, {hi}]");
            assert_eq!(map.at(x, 3, west), 0, "x {x}: nothing rises to the west of a flat plain");
        }
        // Texels east of the ridge look away from it: no horizon toward the east, the ridge to the west.
        assert_eq!(map.at(99, 3, east), 0);
        assert!(map.at(99, 3, west) > 0 || col + width + 99 > n, "the ridge is to the west of a texel east of it");
    }

    // spec: GEN-044
    #[test]
    fn the_result_does_not_depend_on_the_thread_count_and_is_a_pure_function() {
        let n = 96;
        let heights: Vec<f32> =
            (0..n * n).map(|k| 30.0 * libm::sin(0.13 * (k % n) as f64) as f32 * libm::cos(0.07 * (k / n) as f64) as f32 + 5.0).collect();
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = params(300.0);
        let one = bake_horizon(&raster, (10, 10, 40, 37), &p, 1).unwrap();
        for threads in [2, 3, 7, 64] {
            assert_eq!(bake_horizon(&raster, (10, 10, 40, 37), &p, threads).unwrap(), one, "{threads} threads");
        }
        assert_eq!(bake_horizon(&raster, (10, 10, 40, 37), &p, 1).unwrap(), one);
    }

    // spec: GEN-044
    #[test]
    fn texels_shared_by_two_chunks_agree_when_both_rasters_hold_the_halo_and_the_halo_is_needed() {
        let n = 160usize;
        let heights: Vec<f32> =
            (0..n * n).map(|k| 40.0 * (libm::sin(0.09 * (k % n) as f64) + libm::cos(0.11 * (k / n) as f64)) as f32).collect();
        let big = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = params(200.0); // 50 texels
        let halo = 50usize;
        // The block [60, 100) x [60, 100) from the whole raster, and from the sub-raster [10, 150)^2 that holds exactly its halo.
        let want = bake_horizon(&big, (60, 60, 40, 40), &p, 2).unwrap();
        let m = 140usize;
        let sub: Vec<f32> = (0..m * m).map(|k| heights[(10 + k / m) * n + 10 + k % m]).collect();
        let small = HeightRaster { n: m, texel_m: TEXEL, heights: &sub };
        let got = bake_horizon(&small, (60 - 10, 60 - 10, 40, 40), &p, 3).unwrap();
        assert_eq!(got, want, "a chunk with its halo gives the values of the whole raster");
        // Without the halo the march stops at the raster's edge and the edge texels lose what lay beyond it.
        let tight: Vec<f32> = (0..40 * 40).map(|k| heights[(60 + k / 40) * n + 60 + k % 40]).collect();
        let bare = bake_horizon(&HeightRaster { n: 40, texel_m: TEXEL, heights: &tight }, (0, 0, 39, 39), &p, 1).unwrap();
        let differs = (0..39).any(|i| (0..16).any(|k| bare.at(i, 0, k) != want.at(i, 0, k)));
        assert!(differs, "the halo changes the horizon of the edge texels (halo {halo} texels)");
    }

    // spec: GEN-044
    #[test]
    fn the_curvature_of_the_body_lowers_distant_terrain_and_bad_input_is_refused() {
        // On a small body (radius 20 km) the terrain 1 km away is 25 m lower than the plane: a ridge of 12.5 m there is below the horizon.
        let (n, col) = (400usize, 300usize);
        let small = 20_000.0f64;
        let drop = (1_000.0f64 * 1_000.0 / (2.0 * small)) as f32;
        let heights = ridge(n, col, 40, drop * 0.5);
        let raster = HeightRaster { n, texel_m: TEXEL, heights: &heights };
        let p = HorizonParams { range_m: 1_500.0, ..HorizonParams::chunk(small) };
        let east = (p.azimuths / 4) as usize;
        let map = bake_horizon(&raster, (50, 200, 1, 1), &p, 1).unwrap();
        assert_eq!(map.at(0, 0, east), 0, "a ridge lower than the curvature drop is below the horizon");
        let flat_body = HorizonParams { radius_m: 1.0e18, ..p };
        assert!(bake_horizon(&raster, (50, 200, 1, 1), &flat_body, 1).unwrap().at(0, 0, east) > 0, "on a flat body it rises");
        assert!(bake_horizon(&raster, (390, 0, 20, 1), &p, 1).unwrap_err().contains("not inside"));
        assert!(bake_horizon(&raster, (0, 0, 1, 1), &HorizonParams { azimuths: 0, ..p }, 1).unwrap_err().contains("bad horizon"));
        assert!(bake_horizon(&HeightRaster { n: 4, texel_m: TEXEL, heights: &[0.0; 3] }, (0, 0, 1, 1), &p, 1).is_err());
        assert_eq!(quantize(-0.3), 0);
        assert_eq!(quantize(core::f64::consts::FRAC_PI_2), 255);
        assert!((dequantize(quantize(0.2)) - 0.2).abs() <= DEGREES_PER_STEP.to_radians() / 2.0 + 1e-12);
    }
}
