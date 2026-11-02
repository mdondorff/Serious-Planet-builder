//! Tier A, GEN-097 and GEN-099 (the relief channels of the bake): the same bytes on any thread count, block ranges that hold every value a block's cells read, the
//! coast taper, a point shared by two faces, and the step limit as a parameter. Small lattices on synthetic surfaces; no store, no planet bake.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::classes::{blended_k, ClassLattice, ClassSource};
use planet_generators::relief_channels::*;

/// A rough surface with relief at a wavelength of about 7 to 9 km: the ring mean depends on how the ring is oriented.
fn rough(d: Vec3) -> f64 {
    1000.0 * libm::sin(d.x * R / 1500.0 + 0.3) * libm::cos(d.y * R / 1100.0 + 0.7) + 400.0 * libm::sin(d.z * R / 900.0)
}

const R: f64 = 6_371_000.0;
const CELLS: usize = 64;

/// A smooth synthetic surface of the direction: a continent of 2 km mountains and a sea (from -3000 to 2500 m).
fn surface(d: Vec3) -> f64 {
    2800.0 * libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y) + 900.0 * libm::sin(7.0 * d.z) - 300.0
}

fn classes() -> ClassLattice {
    ClassLattice::build_with(&ClassSource::new(3), 16, 2)
}

// spec: GEN-099
#[test]
fn the_channels_are_the_same_bytes_on_1_and_8_threads() {
    let c = classes();
    let (one, eight) = (ReliefChannels::compute(&c, &surface, R, CELLS, 1), ReliefChannels::compute(&c, &surface, R, CELLS, 8));
    assert!(one.to_bytes() == eight.to_bytes(), "the channels differ between 1 and 8 threads");
    assert_eq!(one.cells(), CELLS);
}

// spec: GEN-099
#[test]
fn every_value_a_block_reads_lies_in_its_block_range() {
    let c = classes();
    let ch = ReliefChannels::compute(&c, &surface, R, CELLS, 4);
    let b = CELLS / BLOCK_CELLS;
    let mut blocks = 0;
    for f in 0..6u8 {
        for bj in 0..b {
            for bi in 0..b {
                let r = ch.block_range(Face(f), bi, bj);
                for j in bj * BLOCK_CELLS..=(bj + 1) * BLOCK_CELLS {
                    for i in bi * BLOCK_CELLS..=(bi + 1) * BLOCK_CELLS {
                        let (k, m) = ch.at(Face(f), i, j);
                        assert!(
                            r[0] <= k && k <= r[1],
                            "k {k} outside [{}, {}] at face {f} block ({bi}, {bj}) point ({i}, {j})",
                            r[0],
                            r[1]
                        );
                        assert!(
                            r[2] <= m && m <= r[3],
                            "m {m} outside [{}, {}] at face {f} block ({bi}, {bj}) point ({i}, {j})",
                            r[2],
                            r[3]
                        );
                    }
                }
                blocks += 1;
            }
        }
    }
    assert_eq!(blocks, 6 * b * b);
}

// spec: GEN-097
#[test]
fn the_taper_is_1_over_the_sea_and_the_class_driver_from_400_m_up() {
    let c = classes();
    let sea = |_: Vec3| -500.0;
    let high = |_: Vec3| 1500.0;
    let (low, up) = (ReliefChannels::compute(&c, &sea, R, CELLS, 2), ReliefChannels::compute(&c, &high, R, CELLS, 2));
    let mut tapered_somewhere = false;
    for f in 0..6u8 {
        for j in (0..=CELLS).step_by(3) {
            for i in (0..=CELLS).step_by(3) {
                assert_eq!(low.at(Face(f), i, j).0, 1.0, "k is 1 where the ring mean is under 0 m");
                assert_eq!(low.at(Face(f), i, j).1, -500.0);
                let d = face_to_direction(&TangentWarp, Face(f), 2.0 * i as f64 / CELLS as f64 - 1.0, 2.0 * j as f64 / CELLS as f64 - 1.0);
                let k = blended_k(&c.weights_at(d));
                assert!(
                    (f64::from(up.at(Face(f), i, j).0) - k).abs() < 1e-5,
                    "k is the class driver {k} above 400 m, not {}",
                    up.at(Face(f), i, j).0
                );
                tapered_somewhere |= k > 1.0;
            }
        }
    }
    assert!(tapered_somewhere, "the test surface has a class with k over 1");
}

// spec: GEN-099
#[test]
fn the_ring_mean_of_a_constant_surface_is_that_constant_and_of_a_ramp_is_its_centre_value() {
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert!((ring_mean(&|_| 123.0, R, d) - 123.0).abs() < 1e-9);
    // A linear function of the position: the 8 compass points of a ring are symmetric about the centre, so the mean is the centre's value to second order (the ring is 1.6e-3 rad).
    let ramp = |p: Vec3| 1000.0 * (p.x + 2.0 * p.y - p.z);
    assert!((ring_mean(&ramp, R, d) - ramp(d)).abs() < 1e-2, "{} against {}", ring_mean(&ramp, R, d), ramp(d));
}

// spec: GEN-099
#[test]
fn a_point_on_the_edge_of_two_faces_has_the_same_values_on_both() {
    let c = classes();
    let ch = ReliefChannels::compute(&c, &rough, R, CELLS, 2);
    // For every point on the +X face's edge s = 1, find the point of another face that is the same direction (within 1e-12) and compare the values.
    let mut checked = 0;
    for j in 0..=CELLS {
        let d = face_to_direction(&TangentWarp, Face(0), 1.0, 2.0 * j as f64 / CELLS as f64 - 1.0);
        for f in 1..6u8 {
            for e in 0..=CELLS {
                for (i2, j2) in [(0, e), (CELLS, e), (e, 0), (e, CELLS)] {
                    let d2 = face_to_direction(
                        &TangentWarp,
                        Face(f),
                        2.0 * i2 as f64 / CELLS as f64 - 1.0,
                        2.0 * j2 as f64 / CELLS as f64 - 1.0,
                    );
                    if (d2 - d).length() < 1e-12 {
                        let (a, b) = (ch.at(Face(0), CELLS, j), ch.at(Face(f), i2, j2));
                        assert!(
                            (a.0.to_bits(), a.1.to_bits()) == (b.0.to_bits(), b.1.to_bits()),
                            "face 0 point ({CELLS}, {j}) and face {f} point ({i2}, {j2}) differ: {a:?} against {b:?}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 0, "no edge point was found");
}

// spec: GEN-097
#[test]
fn the_step_limit_is_a_parameter_of_the_feathering_and_the_check() {
    assert_eq!(DEFAULT_MAX_STEP, 2.0);
    assert_eq!((feather_cells(5.0, 2.0), feather_cells(-6.5, 2.0), feather_cells(0.0, 2.0), feather_cells(2.0, 2.0)), (3, 4, 0, 1));
    assert_eq!(feather_cells(5.0, 1.0), 5, "a tighter limit feathers over more cells");
    let c = classes();
    let ch = ReliefChannels::compute(&c, &surface, R, CELLS, 2);
    let (loose, tight) = (ch.k_steps_over(100.0), ch.k_steps_over(0.01));
    assert_eq!(loose, 0.0, "no step is over 100");
    assert!(tight > loose && tight <= 1.0, "a tighter limit has more steps over it: {tight}");
}

// spec: GEN-099
#[test]
fn the_ring_mean_is_continuous_across_the_face_diagonals_of_a_rough_surface() {
    // Points 1e-12 either side of the diagonal |x| = |y| of the +Z face: a frame that rotates there would jump by hundreds of metres on this surface.
    let ring = Ring::new(R);
    let mut worst = 0.0f64;
    for t in [0.05, 0.2, 0.4, 0.6, 0.8] {
        let u = libm::tan(t * core::f64::consts::FRAC_PI_4);
        let (a, b) = (Vec3::new(u * (1.0 + 1e-12), u, 1.0).normalized(), Vec3::new(u, u * (1.0 + 1e-12), 1.0).normalized());
        worst = worst.max((ring_mean_with(&rough, &ring, a) - ring_mean_with(&rough, &ring, b)).abs());
    }
    assert!(worst < 1.0, "the ring mean jumps by {worst} m across a face diagonal");
}

// spec: GEN-099
#[test]
fn a_block_range_is_tight_and_the_shared_row_is_in_both_neighbouring_blocks() {
    let c = classes();
    let ch = ReliefChannels::compute(&c, &rough, R, CELLS, 2);
    for f in 0..6u8 {
        for (bi, bj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let r = ch.block_range(Face(f), bi, bj);
            let mut pts = Vec::new();
            for j in bj * BLOCK_CELLS..=(bj + 1) * BLOCK_CELLS {
                for i in bi * BLOCK_CELLS..=(bi + 1) * BLOCK_CELLS {
                    pts.push(ch.at(Face(f), i, j));
                }
            }
            assert!(pts.iter().any(|p| p.0 == r[0]) && pts.iter().any(|p| p.0 == r[1]), "k's range is not met by a point of its block");
            assert!(pts.iter().any(|p| p.1 == r[2]) && pts.iter().any(|p| p.1 == r[3]), "m's range is not met by a point of its block");
        }
    }
    // The points of the shared row (j = 32) of blocks (0, 0) and (0, 1) lie in both ranges.
    let (lo, hi) = (ch.block_range(Face(0), 0, 0), ch.block_range(Face(0), 0, 1));
    for i in 0..=BLOCK_CELLS {
        let (k, m) = ch.at(Face(0), i, BLOCK_CELLS);
        assert!(lo[0] <= k && k <= lo[1] && hi[0] <= k && k <= hi[1] && lo[2] <= m && m <= lo[3] && hi[2] <= m && m <= hi[3]);
    }
}

// spec: GEN-099
#[test]
#[should_panic(expected = "the body's radius must exceed the ring's")]
fn a_body_smaller_than_the_ring_is_refused_with_a_message() {
    let _ = Ring::new(0.0);
}
