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

/// The azimuth of `sun` (a unit direction) at a point with unit `up`, in the frame whose tangents at the point are `east` and `north`: radians
/// from north, clockwise, in `(-pi, pi]`. A sun straight overhead has no azimuth and gives 0.
///
/// The axes need not be square: a cube face's `i` and `j` are not orthogonal where the warp shears them (up to about 25 degrees at the corners of a face),
/// and the march of GEN-044 steps along `sin az` of `i` plus `cos az` of `j`. The sun's horizontal part is therefore split into its components
/// along the two axes (the dual basis), not projected on each; for orthonormal axes (the geographic frame) the two are the same.
pub fn azimuth_in_frame(sun: Vec3, east: Vec3, north: Vec3) -> f64 {
    let (pe, pn, g) = (sun.dot(east), sun.dot(north), east.dot(north));
    let det = 1.0 - g * g;
    libm::atan2((pe - g * pn) / det, (pn - g * pe) / det)
}

/// The elevation of `sun` over the horizontal plane at a point with unit `up`, in radians.
pub fn sun_elevation(sun: Vec3, up: Vec3) -> f64 {
    libm::asin(sun.dot(up).clamp(-1.0, 1.0))
}

/// The unit `(east, north)` tangents of a cube face's raster at the face coordinates `(s, t)`: east along increasing `i` (`s`), north along
/// increasing `j` (`t`), the frame GEN-044's azimuths are in. Taken from the mapping a quarter of a cell either side, so it is the face's own
/// frame whatever the mapping.
pub fn face_frame(map: &dyn FaceMapping, face: Face, s: f64, t: f64) -> (Vec3, Vec3) {
    const H: f64 = 1.0e-4;
    let up = face_to_direction(map, face, s, t);
    let along = |d: Vec3| d - up * d.dot(up);
    let east = along(face_to_direction(map, face, s + H, t) - face_to_direction(map, face, s - H, t)).normalized();
    let north = along(face_to_direction(map, face, s, t + H) - face_to_direction(map, face, s, t - H)).normalized();
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
            for (s, t) in [(0.0, 0.0), (0.7, -0.4), (-0.9, 0.9)] {
                let up = face_to_direction(&TangentWarp, Face(face), s, t);
                let (east, north) = face_frame(&TangentWarp, Face(face), s, t);
                assert!(east.dot(up).abs() < 1e-9 && north.dot(up).abs() < 1e-9, "tangent to the sphere");
                assert!(east.dot(north).abs() < 0.5, "the warp shears the axes by at most about 30 degrees: {}", east.dot(north));
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
}
