//! Tier A, GEN-097 and GEN-099 (the relief channels of the bake): the same bytes on any thread count, block ranges that hold every value a block's cells read, the
//! coast taper, a point shared by two faces, and the step limit as a parameter. Small lattices on synthetic surfaces; no store, no planet bake.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::classes::{blended_k, ClassLattice, ClassSource};
use planet_generators::relief_channels::*;

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
    let ch = ReliefChannels::compute(&c, &surface, R, CELLS, 2);
    // For every point on the +X face's edge s = 1, find the point of another face that is the same direction (within 1e-12) and compare the values.
    let (mut checked, mut worst) = (0, 0.0f64);
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
                        worst = worst.max(f64::from((a.0 - b.0).abs())).max(f64::from((a.1 - b.1).abs()) / 1000.0);
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 0 && worst < 1e-3, "{checked} edge points checked, worst difference {worst}");
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
