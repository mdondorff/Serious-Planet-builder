//! Tier A, GEN-103 (the default class source, ADR 0089 decisions 1 to 3 and 10): the weights sum, the same bytes on any thread count, continuity across a cube
//! edge, the memberships of GEN-095 and the blended `k` of GEN-097.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::classes::*;

const SEED: u64 = 1;

fn lattice_bytes(l: &ClassLattice) -> Vec<u8> {
    let n = l.cells();
    let mut out = Vec::new();
    for f in 0..6u8 {
        for j in 0..=n {
            for i in 0..=n {
                out.extend_from_slice(&l.point(Face(f), i, j));
            }
        }
    }
    out
}

// spec: GEN-103
#[test]
fn every_lattice_point_sums_to_255_and_the_bytes_do_not_depend_on_the_thread_count() {
    let source = ClassSource::new(SEED);
    let one = ClassLattice::build_with(&source, 16, 1);
    let eight = ClassLattice::build_with(&source, 16, 8);
    let a = lattice_bytes(&one);
    assert_eq!(a, lattice_bytes(&eight), "the lattice differs between 1 and 8 threads");
    assert_eq!(a, lattice_bytes(&ClassLattice::build_with(&ClassSource::new(SEED), 16, 3)), "a second source of the same seed differs");
    for point in a.chunks(CLASS_COUNT) {
        assert_eq!(point.iter().map(|&v| u32::from(v)).sum::<u32>(), 255, "{point:?}");
    }
    assert_ne!(a, lattice_bytes(&ClassLattice::build_with(&ClassSource::new(2), 16, 1)), "two seeds give the same weights");
}

// spec: GEN-103
#[test]
fn the_quantised_weights_sum_to_255_and_ties_go_to_the_earlier_class() {
    let third = [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    assert_eq!(quantize_weights(&third), [85, 85, 85, 0, 0, 0, 0, 0]);
    let half = [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    assert_eq!(quantize_weights(&half), [128, 127, 0, 0, 0, 0, 0, 0], "the odd unit goes to the earlier class");
    let mut one = [0.0; CLASS_COUNT];
    one[HIGH_MOUNTAINS] = 1.0;
    assert_eq!(quantize_weights(&one)[HIGH_MOUNTAINS], 255);
}

// spec: GEN-103
#[test]
fn the_source_is_continuous_across_cube_edges_and_corners_and_reads_the_same_bits_twice() {
    let source = ClassSource::new(SEED);
    let eps = 1e-12;
    // Points straddling cube edges (two of |x|, |y|, |z| equal and largest), several along each of the edges, and the corners.
    let mut pairs: Vec<(Vec3, Vec3)> = Vec::new();
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for z in [-0.8, -0.2, 0.0, 0.45, 0.8] {
                let on_edge = |x: f64, y: f64, z: f64| Vec3::new(sx * x, sy * y, z);
                // The edge |x| = |y|: step across it along the in-plane normal.
                let p = on_edge(1.0, 1.0, z).normalized();
                let n = Vec3::new(sx, -sy, 0.0).normalized();
                pairs.push(((p + n * eps).normalized(), (p - n * eps).normalized()));
                // The edge |x| = |z| and the edge |y| = |z|, on the faces whose coordinates stay on the larger axis.
                let q = Vec3::new(sx * 1.0, z, 1.0).normalized();
                let m = Vec3::new(sx, 0.0, -1.0).normalized();
                pairs.push(((q + m * eps).normalized(), (q - m * eps).normalized()));
                let r = Vec3::new(z, sy * 1.0, 1.0).normalized();
                let k = Vec3::new(0.0, sy, -1.0).normalized();
                pairs.push(((r + k * eps).normalized(), (r - k * eps).normalized()));
            }
            let corner = Vec3::new(sx, sy, 1.0).normalized();
            let (e1, _) = corner.tangent_basis();
            pairs.push(((corner + e1 * eps).normalized(), (corner - e1 * eps).normalized()));
        }
    }
    for (a, b) in pairs {
        let (pa, pb) = (source.eval(a), source.eval(b));
        assert_eq!(pa, source.eval(a), "not pure");
        let (ta, tb) = (pa.terms, pb.terms);
        for (name, x, y) in [
            ("margin_km", ta.margin_km / 1000.0, tb.margin_km / 1000.0),
            ("convergence", ta.convergence, tb.convergence),
            ("core", ta.core, tb.core),
            ("flank", ta.flank, tb.flank),
            ("old margin", ta.old_margin, tb.old_margin),
            ("back-arc", ta.back_arc, tb.back_arc),
            ("plateau", ta.plateau_field, tb.plateau_field),
            ("e0", pa.e0 / 1000.0, pb.e0 / 1000.0),
        ] {
            assert!((x - y).abs() < 1e-9, "{name} jumps across a cube edge at {a:?}: {x} against {y}");
        }
        for c in 0..CLASS_COUNT {
            assert!((pa.weights[c] - pb.weights[c]).abs() < 1e-9, "class {c} jumps across a cube edge at {a:?}");
        }
    }
}

// spec: GEN-103
#[test]
fn a_core_over_high_ground_is_high_mountains_and_a_plateau_field_is_plateau() {
    let none = MaskTerms {
        margin_km: 500.0,
        convergence: 0.0,
        core: 0.0,
        flank: 0.0,
        old_margin: 0.0,
        back_arc: 0.0,
        plateau_field: 0.0,
        plate: 0,
    };
    let core = MaskTerms { core: 1.0, ..none };
    let w = quantize_weights(&class_weights(&core, 3500.0, 0.0));
    assert_eq!(w[HIGH_MOUNTAINS], 255, "{w:?}");
    let w = quantize_weights(&class_weights(&core, 1200.0, 0.0));
    assert_eq!(w[MOUNTAINS], 255, "a core below 2 km is plain mountains: {w:?}");
    let plateau = MaskTerms { plateau_field: 1.0, ..none };
    assert_eq!(quantize_weights(&class_weights(&plateau, 900.0, 0.0))[PLATEAU], 255);
    let w = quantize_weights(&class_weights(&none, 100.0, 1.0));
    assert_eq!(w[ROLLING], 255, "rolling noise 1 on empty ground is rolling plain: {w:?}");
    assert_eq!(quantize_weights(&class_weights(&none, 100.0, 0.0))[PLAIN], 255);
    // Every case sums to 1 before rounding, and badlands and the sand sea stay at zero.
    for terms in [none, core, plateau, MaskTerms { flank: 0.4, ..none }] {
        let w = class_weights(&terms, 2500.0, 0.3);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!((w[6], w[7]), (0.0, 0.0));
    }
}

// spec: GEN-103
#[test]
fn the_blended_k_is_the_weighted_mean_of_the_class_values() {
    let mut w = [0.0; CLASS_COUNT];
    w[HILLS] = 128.0 / 255.0;
    w[MOUNTAINS] = 127.0 / 255.0;
    assert!((blended_k(&w) - (128.0 * 5.0 + 127.0 * 6.0) / 255.0).abs() < 1e-12);
    assert!(
        (hill_amplitude_for_k(1.0) - 30.0).abs() < 1e-9
            && (hill_amplitude_for_k(5.0) - 175.0).abs() < 1e-9
            && (hill_amplitude_for_k(7.5) - 825.0).abs() < 1e-9
    );
}

// spec: GEN-103
#[test]
fn the_lattice_reads_bilinearly_between_points_and_is_the_same_at_a_shared_face_edge() {
    let source = ClassSource::new(SEED);
    let lattice = ClassLattice::build_with(&source, 8, 2);
    let map = TangentWarp;
    for t in [-0.5, 0.25] {
        let a = lattice.weights_at(face_to_direction(&map, Face(0), 1.0, t));
        let b = lattice.weights_at(face_to_direction(&map, Face(2), -1.0, t));
        for c in 0..CLASS_COUNT {
            assert!((a[c] - b[c]).abs() < 1e-9, "class {c} differs across the edge at t {t}: {} against {}", a[c], b[c]);
        }
    }
    let d = Vec3::new(0.3, -0.4, 0.5).normalized();
    let w = lattice.weights_at(d);
    assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-9);
}

// spec: GEN-103
#[test]
fn the_preview_images_are_the_same_bytes_for_the_same_seed_on_any_thread_count() {
    use planet_generators::preview::{compute, render};
    let a = render(&compute(SEED, 128, 64, 8, 1));
    let b = render(&compute(SEED, 128, 64, 8, 6));
    assert_eq!(
        a.len(),
        9,
        "land-sea, elevation, elevation-today, elevation-compare, plate-mask, plateau-backarc, classes-argmax, classes-blend, k"
    );
    for ((na, ia), (nb, ib)) in a.iter().zip(&b) {
        assert_eq!(na, nb);
        assert_eq!(ia, ib, "{na} differs between 1 and 6 threads");
    }
    let other = render(&compute(2, 128, 64, 8, 1));
    assert!(a.iter().zip(&other).any(|((_, x), (_, y))| x != y), "two seeds draw the same images");
}
