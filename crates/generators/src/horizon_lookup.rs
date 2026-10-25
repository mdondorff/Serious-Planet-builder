//! The horizon of a point and the shadow it casts (M8-5, GEN-046): the CPU reference of what the lit terrain shader does with the two horizon
//! maps (GEN-044 per chunk, GEN-045 for the whole planet). The horizon at an azimuth is read by linear interpolation between the two nearest
//! stored azimuths, the point's horizon is the larger of the chunk's and the coarse one, and the sun is in shadow when its elevation is below
//! it, with a soft edge. The shader must give the same factor within the tolerance of its tests (CON-07: this is the reference).
//!
//! The two maps use different azimuth frames (the chunk raster's `i` east and `j` north per cube face, the coarse map's geographic one), so the
//! sun's azimuth is formed in each: [`azimuth_in_frame`] with the frame of the face ([`face_frame`]) or of the globe
//! ([`crate::coarsehorizon::geographic_frame`]).

use crate::horizon::dequantize;
use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{Face, Vec3};

/// The soft edge of a shadow in degrees of sun elevation above the horizon: the sun is fully in shadow `SOFT_BELOW_DEG` below it and fully lit
/// `SOFT_ABOVE_DEG` above it (SPIKE-32: -0.35 to +0.9).
pub const SOFT_BELOW_DEG: f64 = 0.35;
pub const SOFT_ABOVE_DEG: f64 = 0.9;

/// The horizon elevation in radians at azimuth `az` (radians from north, clockwise; any real number) from the `azimuths` bytes of a texel:
/// linear between the two nearest stored azimuths, wrapping from the last to the first. A map of one azimuth gives its byte everywhere.
pub fn interpolate(bytes: &[u8], az: f64) -> f64 {
    let a = bytes.len();
    assert!(a >= 1, "a horizon texel holds at least one azimuth");
    let x = az.rem_euclid(core::f64::consts::TAU) / core::f64::consts::TAU * a as f64;
    let (k, f) = (libm::floor(x), x - libm::floor(x));
    let (k0, k1) = (k as usize % a, (k as usize + 1) % a);
    dequantize(bytes[k0]) * (1.0 - f) + dequantize(bytes[k1]) * f
}

/// A horizon texel and the azimuth of the sun in its frame.
#[derive(Clone, Copy, Debug)]
pub struct HorizonSample<'a> {
    pub bytes: &'a [u8],
    pub sun_azimuth: f64,
}

/// The horizon of a point: the larger of the chunk's and the coarse map's at the sun's azimuth in each one's frame; a map that is not there
/// (`None`: the chunk's horizon has not landed) leaves the other alone, and neither gives 0 (the sky's own horizon).
pub fn point_horizon(chunk: Option<HorizonSample<'_>>, coarse: Option<HorizonSample<'_>>) -> f64 {
    let a = chunk.map_or(0.0, |s| interpolate(s.bytes, s.sun_azimuth));
    let b = coarse.map_or(0.0, |s| interpolate(s.bytes, s.sun_azimuth));
    a.max(b)
}

/// The lit factor of the sun at elevation `sun_elevation` over a horizon at `horizon` (both radians): 0 in shadow, 1 lit, smooth between
/// ([`SOFT_BELOW_DEG`] below the horizon to [`SOFT_ABOVE_DEG`] above it).
pub fn lit_factor(sun_elevation: f64, horizon: f64) -> f64 {
    let (lo, hi) = (-SOFT_BELOW_DEG.to_radians(), SOFT_ABOVE_DEG.to_radians());
    let x = ((sun_elevation - horizon - lo) / (hi - lo)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The azimuth of `sun` (a unit direction) in the frame whose tangents at the point are `east` and `north` (they lie in the tangent plane, so the
/// point's `up` does not enter): radians from north, clockwise, in `(-pi, pi]`. A sun straight overhead has no horizontal direction: its azimuth is
/// noise and its lit factor does not depend on it unless the horizon is at the zenith. A NaN input gives NaN (out of contract).
///
/// The axes need not be unit or square. The march of GEN-044 steps `sin az` along the raster's `i` and `cos az` along its `j` in index space, so
/// the direction it looks along on the ground is `sin az * a_i + cos az * a_j` with `a_i = dP/ds` and `a_j = dP/dt` at their own lengths: a cube
/// face's `i` and `j` are sheared by up to 30 degrees at a corner and, under the tangent warp, of lengths in a ratio up to 1.41 near an edge. The
/// sun's horizontal part `x a_i + y a_j` is therefore solved for `(x, y)` with the full 2 x 2 overlap matrix, and the azimuth is `atan2(x, y)`;
/// for orthonormal axes (the geographic frame) that is the plain projection on each.
pub fn azimuth_in_frame(sun: Vec3, east: Vec3, north: Vec3) -> f64 {
    let (pe, pn) = (sun.dot(east), sun.dot(north));
    let (ee, nn, g) = (east.dot(east), north.dot(north), east.dot(north));
    let det = ee * nn - g * g;
    libm::atan2((pe * nn - g * pn) / det, (pn * ee - g * pe) / det)
}

/// The elevation of `sun` over the horizontal plane at a point with unit `up`, in radians.
pub fn sun_elevation(sun: Vec3, up: Vec3) -> f64 {
    libm::asin(sun.dot(up).clamp(-1.0, 1.0))
}

/// The `(east, north)` tangents of a cube face's raster at the face coordinates `(s, t)`: `dP/ds` along increasing `i` and `dP/dt` along
/// increasing `j`, the frame GEN-044's azimuths are in, at their true (unequal) lengths for [`azimuth_in_frame`]. A central difference of
/// `1e-4` face units either side (about a tenth of a level-11 chunk; the error is second order, about 1e-8 relative). The mapping must continue
/// smoothly past a face edge, as the tangent warp does (`face_to_plane` extends onto the cube plane); a mapping that clamps would give a one-sided
/// difference at the edge, half the step but still pointing the right way.
pub fn face_frame(map: &dyn FaceMapping, face: Face, s: f64, t: f64) -> (Vec3, Vec3) {
    const H: f64 = 1.0e-4;
    let up = face_to_direction(map, face, s, t);
    let along = |d: Vec3| (d - up * d.dot(up)) * (0.5 / H);
    let east = along(face_to_direction(map, face, s + H, t) - face_to_direction(map, face, s - H, t));
    let north = along(face_to_direction(map, face, s, t + H) - face_to_direction(map, face, s, t - H));
    (east, north)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsehorizon::geographic_frame;
    use crate::horizon::{quantize, DEGREES_PER_STEP};
    use planet_core::cube::TangentWarp;

    const STEP: f64 = DEGREES_PER_STEP;

    fn bytes_of(deg: &[f64]) -> Vec<u8> {
        deg.iter().map(|d| quantize(d.to_radians())).collect()
    }

    // spec: GEN-046
    #[test]
    fn the_horizon_is_linear_between_the_two_nearest_azimuths_and_wraps() {
        let b = bytes_of(&[0.0, 10.0, 20.0, 30.0]);
        let q = |i: usize| dequantize(b[i]);
        let quarter = core::f64::consts::FRAC_PI_2;
        for i in 0..4 {
            assert!((interpolate(&b, quarter * i as f64) - q(i)).abs() < 1e-12, "node {i}");
        }
        assert!((interpolate(&b, quarter * 1.5) - 0.5 * (q(1) + q(2))).abs() < 1e-12);
        assert!((interpolate(&b, quarter * 3.25) - (0.75 * q(3) + 0.25 * q(0))).abs() < 1e-12, "the last node blends into the first");
        assert!((interpolate(&b, quarter * 1.5 + core::f64::consts::TAU) - interpolate(&b, quarter * 1.5)).abs() < 1e-12);
        assert!((interpolate(&b, -quarter * 0.5) - interpolate(&b, quarter * 3.5)).abs() < 1e-12, "negative azimuths wrap");
        assert!(
            (interpolate(&bytes_of(&[7.0]), 2.0) - dequantize(quantize(7.0f64.to_radians()))).abs() < 1e-12,
            "one azimuth is everywhere"
        );
    }

    // spec: GEN-046
    #[test]
    fn the_point_is_in_shadow_below_the_larger_of_the_two_horizons_and_the_chunk_alone_decides_when_the_coarse_one_is_absent() {
        // 3 degrees from the chunk, 5 from the coarse map, at the sun's azimuth (one azimuth per map: the same value all round).
        let chunk = bytes_of(&[3.0]);
        let coarse = bytes_of(&[5.0]);
        let sample = |b: &'static [u8]| HorizonSample { bytes: b, sun_azimuth: 1.0 };
        let (c, k) = (Box::leak(chunk.into_boxed_slice()) as &'static [u8], Box::leak(coarse.into_boxed_slice()) as &'static [u8]);
        let both = point_horizon(Some(sample(c)), Some(sample(k)));
        assert!((both.to_degrees() - 5.0).abs() <= STEP, "{}", both.to_degrees());
        assert!(lit_factor(4.0f64.to_radians(), both) < 0.01, "a 4 degree sun is below the 5 degree horizon");
        assert!(lit_factor(6.0f64.to_radians(), both) > 0.99, "a 6 degree sun is above it");
        let chunk_only = point_horizon(Some(sample(c)), None);
        assert!((chunk_only.to_degrees() - 3.0).abs() <= STEP);
        assert!(lit_factor(4.5f64.to_radians(), chunk_only) > 0.99, "with the coarse horizon absent the chunk map decides alone");
        let coarse_only = point_horizon(None, Some(sample(k)));
        assert!((coarse_only.to_degrees() - 5.0).abs() <= STEP, "the stand-in before the chunk's map lands");
        assert_eq!(point_horizon(None, None), 0.0);
        // Each map is read at the sun's azimuth in its own frame.
        let ramp = bytes_of(&[0.0, 10.0, 20.0, 30.0]);
        let there = point_horizon(Some(HorizonSample { bytes: &ramp, sun_azimuth: core::f64::consts::FRAC_PI_2 }), None);
        let elsewhere = point_horizon(None, Some(HorizonSample { bytes: &ramp, sun_azimuth: core::f64::consts::PI }));
        assert!(elsewhere > there);
    }

    // spec: GEN-046
    #[test]
    fn the_soft_edge_is_a_smooth_ramp_from_0_35_below_to_0_9_above_the_horizon() {
        let h = 5.0f64.to_radians();
        let at = |d: f64| lit_factor(h + d.to_radians(), h);
        assert!(at(-SOFT_BELOW_DEG).abs() < 1e-12);
        assert!((at(SOFT_ABOVE_DEG) - 1.0).abs() < 1e-12);
        assert_eq!((at(-10.0), at(10.0)), (0.0, 1.0));
        let mut last = -1.0;
        for k in 0..=125 {
            let f = at(-SOFT_BELOW_DEG + 0.01 * k as f64);
            assert!(f >= last - 1e-15 && (0.0..=1.0).contains(&f), "monotone in [0, 1] at step {k}");
            last = f;
        }
        let mid = at(0.5 * (SOFT_ABOVE_DEG - SOFT_BELOW_DEG));
        assert!((mid - 0.5).abs() < 1e-12, "half way through the ramp it is half lit: {mid}");
    }

    // spec: GEN-046
    #[test]
    fn the_suns_azimuth_is_formed_in_each_maps_own_frame() {
        // Geographic: at +x on the equator, the sun toward +z is north (0), toward +y east (a quarter turn), toward -y west.
        let d = Vec3::new(1.0, 0.0, 0.0);
        let (e, n) = geographic_frame(d);
        let az = |sun: Vec3| azimuth_in_frame(sun, e, n);
        assert!(az(Vec3::new(0.0, 0.0, 1.0)).abs() < 1e-12);
        assert!((az(Vec3::new(0.0, 1.0, 0.0)) - core::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((az(Vec3::new(0.0, -1.0, 0.0)) + core::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((az(Vec3::new(-1.0, 0.0, 1.0).normalized()) - 0.0).abs() < 1e-12, "the part along the up vector does not matter");
        // The elevation: 30 degrees up from the horizontal toward north.
        let sun = Vec3::new(libm::sin(30f64.to_radians()), 0.0, libm::cos(30f64.to_radians()));
        assert!((sun_elevation(sun, d) - 30f64.to_radians()).abs() < 1e-12);
        assert!((sun_elevation(d * -1.0, d) + core::f64::consts::FRAC_PI_2).abs() < 1e-12, "a sun behind the planet is -90 degrees");
        // A cube face's raster frame: east along increasing s, north along increasing t, for every face and a few places on it.
        for face in 0..6u8 {
            for (s, t) in [(0.0, 0.0), (0.7, -0.4), (-0.9, 0.9), (1.0, 1.0), (-1.0, 1.0), (0.95, 0.0)] {
                let up = face_to_direction(&TangentWarp, Face(face), s, t);
                let (east, north) = face_frame(&TangentWarp, Face(face), s, t);
                assert!(east.dot(up).abs() < 1e-9 && north.dot(up).abs() < 1e-9, "tangent to the sphere");
                let cos_shear = east.normalized().dot(north.normalized()).abs();
                assert!(cos_shear <= 0.5 + 1e-6, "the warp shears the axes by at most 30 degrees (0.5 at a corner): {cos_shear}");
                let ahead = face_to_direction(&TangentWarp, Face(face), s + 1.0e-3, t);
                assert!(
                    (azimuth_in_frame(ahead, east, north) - core::f64::consts::FRAC_PI_2).abs() < 1e-6,
                    "increasing s is east in the raster frame"
                );
                let aloft = face_to_direction(&TangentWarp, Face(face), s, t + 1.0e-3);
                assert!(azimuth_in_frame(aloft, east, north).abs() < 1e-6, "increasing t is north in the raster frame");
            }
        }
        // Where the two frames differ the same sun has two azimuths: on a face whose raster is turned against north.
        let diff = (0..6u8)
            .filter(|&f| {
                let up = face_to_direction(&TangentWarp, Face(f), 0.3, 0.2);
                let sun = Vec3::new(0.3, -0.5, 0.8).normalized();
                let (ge, gn) = geographic_frame(up);
                let (fe, fn_) = face_frame(&TangentWarp, Face(f), 0.3, 0.2);
                (azimuth_in_frame(sun, ge, gn) - azimuth_in_frame(sun, fe, fn_)).abs() > 0.1
            })
            .count();
        assert!(diff >= 1, "at least one face is turned against north, so the two maps need their own azimuths");
    }

    // spec: GEN-046
    #[test]
    fn a_sun_half_way_between_the_raster_axes_is_at_45_degrees_in_index_space_whatever_their_lengths() {
        // Near the middle of a face edge the tangent warp makes `i` longer than `j` by a factor 1.35 to 1.41: the ground direction that the march
        // looks along at index azimuth 45 degrees is a_i + a_j, and the sun there must read 45 degrees, not the angle between unit axes.
        for (s, t) in [(0.95, 0.0), (1.0, 0.0), (0.0, 0.95), (-0.9, 0.2)] {
            let up = face_to_direction(&TangentWarp, Face(2), s, t);
            let (a_i, a_j) = face_frame(&TangentWarp, Face(2), s, t);
            let horizontal = (a_i + a_j).normalized();
            let sun = (horizontal * libm::cos(0.3) + up * libm::sin(0.3)).normalized();
            let az = azimuth_in_frame(sun, a_i, a_j);
            assert!((az - core::f64::consts::FRAC_PI_4).abs() < 1e-9, "({s}, {t}): {:.3} degrees", az.to_degrees());
            let unit = azimuth_in_frame(sun, a_i.normalized(), a_j.normalized());
            if (a_i.length() / a_j.length() - 1.0).abs() > 0.1 {
                assert!((unit - core::f64::consts::FRAC_PI_4).abs() > 0.05, "with unit axes the answer would be off at ({s}, {t})");
            }
        }
    }

    // spec: GEN-046
    #[test]
    fn the_stored_maps_cast_their_shadows_in_the_direction_the_sun_is_in() {
        use crate::coarsehorizon::{coarse_horizon, CoarseParams};
        use crate::coupled::Lattice;
        use crate::horizon::{bake_horizon, halo_texels, HeightRaster, HorizonParams};
        const R: f64 = 6_371_000.0;
        // The chunk map: a ridge of 100 m to the raster's east, 520 m from the texel; the sun is placed on the ground toward the ridge's
        // direction on the face (the physical direction of increasing i), a little above and below the ridge's angle.
        let (n, texel) = (400usize, 4.0);
        let p = HorizonParams { range_m: 600.0, ..HorizonParams::chunk(R) };
        let heights: Vec<f32> = (0..n * n).map(|k| if (300..320).contains(&(k % n)) { 100.0 } else { 0.0 }).collect();
        let raster = HeightRaster { n, texel_m: texel, heights: &heights };
        assert!(halo_texels(&p, texel) < 170);
        let block = (170usize, 170usize, 1usize, 1usize);
        let map = bake_horizon(&raster, block, &p, 1).unwrap();
        let bytes: Vec<u8> = (0..16).map(|k| map.at(0, 0, k)).collect();
        let angle = libm::atan(100.0 / 520.0);
        for (face, s, t) in [(0u8, 0.0, 0.0), (2, 0.95, 0.0), (4, -0.6, 0.5)] {
            let up = face_to_direction(&TangentWarp, Face(face), s, t);
            let (a_i, a_j) = face_frame(&TangentWarp, Face(face), s, t);
            let ground = |toward: Vec3, el: f64| (toward.normalized() * libm::cos(el) + up * libm::sin(el)).normalized();
            let lit = |sun: Vec3| {
                let sample = HorizonSample { bytes: &bytes, sun_azimuth: azimuth_in_frame(sun, a_i, a_j) };
                lit_factor(sun_elevation(sun, up), point_horizon(Some(sample), None))
            };
            assert!(
                lit(ground(a_i, angle - 4.0f64.to_radians())) < 0.01,
                "face {face}: a sun below the ridge's angle, toward it, is in shadow"
            );
            assert!(lit(ground(a_i, angle + 4.0f64.to_radians())) > 0.99, "face {face}: a sun above it is lit");
            assert!(lit(ground(a_i * -1.0, angle - 4.0f64.to_radians())) > 0.99, "face {face}: a sun on the other side of the sky is lit");
            assert!(lit(ground(a_j, angle - 4.0f64.to_radians())) > 0.99, "face {face}: nor does the ridge shade the north");
        }
        // The coarse map: a plateau toward the geographic north, seen by an observer at +x on a small body.
        let small = 100_000.0;
        let run = Lattice::new(64, small);
        let c = Vec3::new(libm::cos(0.1), 0.0, libm::sin(0.1)); // 10 km north of +x
        let z: Vec<f32> = (0..run.len())
            .map(|k| if small * libm::acos(run.direction(k).dot(c).clamp(-1.0, 1.0)) <= 4_000.0 { 3_000.0 } else { 0.0 })
            .collect();
        let cp = CoarseParams { azimuths: 16, range_m: 20_000.0, step_m: 2_000.0, radius_m: small };
        let coarse = coarse_horizon(&run, &z, 64, &cp, 2).unwrap();
        let obs = run.cell_at(Vec3::new(1.0, 0.0, 0.0));
        let d = run.direction(obs);
        let (e, nrth) = geographic_frame(d);
        let cbytes = &coarse[obs * 16..(obs + 1) * 16];
        let elev = crate::coarsehorizon::elevation(small, 0.0, 3_000.0, 10_000.0);
        let lit = |toward: Vec3, el: f64| {
            let sun = (toward * libm::cos(el) + d * libm::sin(el)).normalized();
            lit_factor(
                sun_elevation(sun, d),
                point_horizon(None, Some(HorizonSample { bytes: cbytes, sun_azimuth: azimuth_in_frame(sun, e, nrth) })),
            )
        };
        assert!(lit(nrth, elev - 6.0f64.to_radians()) < 0.01, "a low sun toward the plateau is in shadow");
        assert!(lit(nrth, elev + 6.0f64.to_radians()) > 0.99);
        assert!(lit(e, elev - 6.0f64.to_radians()) > 0.99, "and a sun to the east is lit");
        assert!(lit(nrth * -1.0, elev - 6.0f64.to_radians()) > 0.99);
    }

    // spec: GEN-046
    #[test]
    fn an_azimuth_just_below_zero_reads_the_first_node_and_the_sun_overhead_is_lit() {
        let b = bytes_of(&[5.0, 9.0, 12.0, 7.0]);
        assert_eq!(interpolate(&b, -1e-17), interpolate(&b, 0.0));
        assert_eq!(interpolate(&b, core::f64::consts::TAU - 1e-17), interpolate(&b, 0.0));
        // Straight overhead the azimuth is whatever the rounding gives; the lit factor does not depend on it.
        let up = Vec3::new(1.0, 0.0, 0.0);
        let (e, n) = geographic_frame(up);
        let az = azimuth_in_frame(up, e, n);
        assert!(az.is_finite());
        let ramp = bytes_of(&[20.0, 30.0, 40.0, 25.0]);
        let horizon = point_horizon(Some(HorizonSample { bytes: &ramp, sun_azimuth: az }), None);
        assert!(lit_factor(sun_elevation(up, up), horizon) > 0.99);
    }
}
