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
        strike: 1.0,
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

// spec: GEN-103
#[test]
fn the_mask_is_continuous_where_the_nearest_plate_changes() {
    let source = ClassSource::new(SEED);
    // Walk great-circle arcs, find where the nearest plate changes by bisection, and compare the two sides 1e-9 rad apart.
    let mut checked = 0;
    for k in 0..400u32 {
        let (a, b) = (
            Vec3::new(f64::from(k % 7) - 3.1, f64::from(k % 11) - 5.2, f64::from(k % 5) - 2.3).normalized(),
            Vec3::new(f64::from((k * 3) % 13) - 6.1, f64::from((k * 5) % 7) - 3.3, f64::from((k * 7) % 9) - 4.4).normalized(),
        );
        let at = |t: f64| (a * (1.0 - t) + b * t).normalized();
        let plate = |t: f64| {
            let p = source.eval(at(t));
            (p.terms.plate, p)
        };
        let n = 200;
        for s in 0..n {
            let (t0, t1) = (f64::from(s) / f64::from(n), f64::from(s + 1) / f64::from(n));
            let (p0, p1) = (plate(t0).0, plate(t1).0);
            if p0 == p1 {
                continue;
            }
            let (mut lo, mut hi) = (t0, t1);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if plate(mid).0 == p0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let (x, y) = (plate(lo).1, plate(hi).1);
            for (name, u, v) in [
                ("core", x.terms.core, y.terms.core),
                ("flank", x.terms.flank, y.terms.flank),
                ("old margin", x.terms.old_margin, y.terms.old_margin),
                ("back-arc", x.terms.back_arc, y.terms.back_arc),
                ("plateau", x.terms.plateau_field, y.terms.plateau_field),
                ("e0", x.e0 / 1000.0, y.e0 / 1000.0),
            ] {
                assert!((u - v).abs() < 1e-6, "{name} jumps where the nearest plate changes (arc {k}, t {lo}): {u} against {v}");
            }
            checked += 1;
        }
    }
    assert!(checked >= 20, "only {checked} plate changes were found on the arcs");
}

// spec: GEN-103
#[test]
fn the_shape_metrics_find_a_ring_a_long_band_and_two_land_masses_on_a_synthetic_grid() {
    use planet_generators::shape::{shape_metrics, Grid};
    let (w, h) = (360usize, 180usize);
    let mut land = vec![false; w * h];
    let mut high = vec![false; w * h];
    // Two land blocks (lon 40..100 and 200..260, lat rows 50..130) and a ring of high ground round a hole in the first.
    for y in 50..130 {
        for x in (40..100).chain(200..260) {
            land[y * w + x] = true;
        }
    }
    for y in 60..120 {
        for x in 50..90 {
            let edge = !(64..116).contains(&y) || !(54..86).contains(&x);
            high[y * w + x] = edge;
        }
    }
    let m = shape_metrics(&Grid { width: w, height: h, land: &land, high: &high });
    assert_eq!(m.land_masses.len(), 2, "{:?}", m.land_masses);
    assert!((m.largest_share - 0.5).abs() < 0.05, "{}", m.largest_share);
    assert!(m.enclosed_mkm2 > 0.0, "the ring encloses land the high ground does not cover: {}", m.enclosed_mkm2);
    assert!(!m.longest_spans_km.is_empty() && m.longest_spans_km[0] > 2000.0);
    assert!(m.width_median_km > 0.0 && m.width_p10_km <= m.width_median_km);
}

// spec: GEN-103
#[test]
fn the_lattice_reports_the_largest_k_step_and_where_and_the_share_of_steps_over_a_limit() {
    let source = ClassSource::new(SEED);
    let lattice = ClassLattice::build_with(&source, 16, 2);
    let (step, face, i, j) = lattice.largest_k_step_at();
    assert_eq!(step, lattice.largest_k_step());
    assert!(i <= 16 && j <= 16 && face.0 < 6);
    let over_half = lattice.k_steps_over(step / 2.0);
    assert!(over_half > 0.0 && over_half <= 1.0, "{over_half}");
    assert_eq!(lattice.k_steps_over(step + 1e-9), 0.0, "no step is over the largest step");
}
