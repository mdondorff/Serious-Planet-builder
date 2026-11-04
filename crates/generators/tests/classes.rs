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
        hilly: 0.0,
        strike: 1.0,
        plate: 0,
    };
    let core = MaskTerms { core: 1.0, ..none };
    let w = quantize_weights(&class_weights(&core, 5500.0, 0.0));
    assert_eq!(w[HIGH_MOUNTAINS], 255, "{w:?}");
    let w = quantize_weights(&class_weights(&core, 800.0, 0.0));
    assert_eq!(w[MOUNTAINS], 255, "a core below 1 km is plain mountains: {w:?}");
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
fn the_mask_has_no_jump_where_the_nearest_or_the_second_plate_changes() {
    let source = ClassSource::new(SEED);
    // Walk great-circle arcs in steps of about 13 km; where a term moves by more than 0.08 between two samples, bisect on the larger difference and compare
    // the two sides about 1e-12 rad apart. A smooth change shrinks with the step, a jump does not.
    let terms = |d: Vec3| {
        let p = source.eval(d);
        [p.terms.core, p.terms.flank, p.terms.old_margin, p.terms.back_arc, p.terms.plateau_field, p.e0 / 1000.0]
    };
    let (mut candidates, mut plate_changes) = (0, 0);
    for k in 0..90u32 {
        let (a, b) = (
            Vec3::new(f64::from(k % 7) - 3.1, f64::from(k % 11) - 5.2, f64::from(k % 5) - 2.3).normalized(),
            Vec3::new(f64::from((k * 3) % 13) - 6.1, f64::from((k * 5) % 7) - 3.3, f64::from((k * 7) % 9) - 4.4).normalized(),
        );
        let at = |t: f64| (a * (1.0 - t) + b * t).normalized();
        let n = 1500;
        let mut prev = (terms(at(0.0)), source.eval(at(0.0)).terms.plate);
        for s in 1..=n {
            let t1 = f64::from(s) / f64::from(n);
            let cur = (terms(at(t1)), source.eval(at(t1)).terms.plate);
            let diff = |x: &[f64; 6], y: &[f64; 6]| x.iter().zip(y).map(|(u, v)| (u - v).abs()).fold(0.0, f64::max);
            if cur.1 != prev.1 {
                plate_changes += 1;
            }
            if diff(&prev.0, &cur.0) > 0.08 {
                candidates += 1;
                let (mut lo, mut hi) = (f64::from(s - 1) / f64::from(n), t1);
                for _ in 0..50 {
                    let mid = 0.5 * (lo + hi);
                    let (left, right) = (diff(&terms(at(lo)), &terms(at(mid))), diff(&terms(at(mid)), &terms(at(hi))));
                    // A smooth change has halved below the jump limit's tenth within a few steps (a jump of 1e-4 would show in one half): stop there.
                    if left.max(right) < 1e-5 {
                        break;
                    }
                    if left > right {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let jump = diff(&terms(at(lo)), &terms(at(hi)));
                assert!(jump < 1e-4, "a term jumps by {jump} at t {lo} of arc {k}: {:?} against {:?}", terms(at(lo)), terms(at(hi)));
            }
            prev = cur;
        }
    }
    assert!(plate_changes >= 8, "only {plate_changes} changes of the nearest plate on the arcs");
    println!("{candidates} steep steps examined, {plate_changes} changes of the nearest plate");
}

// spec: GEN-103
#[test]
fn the_class_source_version_goes_with_its_output() {
    // Bump CLASS_SOURCE_VERSION whenever this fingerprint changes (the preview images and the stored fields carry the version).
    let source = ClassSource::new(SEED);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for k in 0..24u32 {
        let d = Vec3::new(f64::from(k % 5) - 2.2, f64::from(k % 7) - 3.1, f64::from(k % 3) - 1.3).normalized();
        let p = source.eval(d);
        let k = planet_generators::classes::blended_k(&p.weights);
        for v in [
            p.terms.core,
            p.terms.flank,
            p.terms.old_margin,
            p.terms.back_arc,
            p.terms.plateau_field,
            p.terms.hilly,
            p.terms.strike,
            p.e0,
            source.elevation_m(d, &p, k),
        ] {
            for byte in v.to_bits().to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    assert_eq!(
        (CLASS_SOURCE_VERSION, hash),
        (6, FINGERPRINT),
        "the default source's output changed: bump CLASS_SOURCE_VERSION and record the new fingerprint"
    );
}

const FINGERPRINT: u64 = 17130647235075386674;

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

// spec: GEN-103
#[test]
fn a_world_of_one_small_island_has_all_its_land_in_the_largest_mass() {
    use planet_generators::shape::{shape_metrics, Grid};
    let (w, h) = (360usize, 180usize);
    let mut land = vec![false; w * h];
    for y in 88..92 {
        for x in 100..104 {
            land[y * w + x] = true;
        }
    }
    let m = shape_metrics(&Grid { width: w, height: h, land: &land, high: &vec![false; w * h] });
    assert!(m.land_masses.is_empty(), "under 1 Mkm2");
    assert!((m.largest_share - 1.0).abs() < 1e-9, "{}", m.largest_share);
}

// spec: GEN-103
#[test]
fn the_mask_has_no_step_at_the_latitudes_where_a_tangent_basis_would_change() {
    let source = ClassSource::new(SEED);
    // The tangent basis of Vec3 switches its helper axis at |z| = 0.9; the estimate of the coast distance must not.
    let at = |phi: f64, z: f64| {
        let r = libm::sqrt(1.0 - z * z);
        Vec3::new(r * libm::cos(phi), r * libm::sin(phi), z).normalized()
    };
    for sign in [-1.0, 1.0] {
        for k in 0..400u32 {
            let phi = f64::from(k) * 0.01571;
            let (a, b) = (source.eval(at(phi, sign * (0.9 - 1e-13))), source.eval(at(phi, sign * (0.9 + 1e-13))));
            for (name, x, y) in [
                ("core", a.terms.core, b.terms.core),
                ("flank", a.terms.flank, b.terms.flank),
                ("old margin", a.terms.old_margin, b.terms.old_margin),
                ("plateau", a.terms.plateau_field, b.terms.plateau_field),
                ("e0", a.e0 / 1000.0, b.e0 / 1000.0),
            ] {
                assert!((x - y).abs() < 1e-6, "{name} steps at z = {} and longitude {phi}: {x} against {y}", sign * 0.9);
            }
        }
    }
}

// spec: GEN-094
#[test]
fn no_belt_stands_in_the_sea_and_the_plates_of_a_retry_are_the_same_on_every_build() {
    let source = ClassSource::new(3);
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let n = 6000;
    let (mut sea, mut belts_on_land) = (0, 0);
    for i in 0..n {
        let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n);
        let r = libm::sqrt((1.0 - z * z).max(0.0));
        let d = Vec3::new(r * libm::cos(golden * f64::from(i)), r * libm::sin(golden * f64::from(i)), z);
        let p = source.eval(d);
        if p.land_depth <= 0.0 {
            sea += 1;
            assert_eq!((p.terms.core, p.terms.flank, p.terms.old_margin), (0.0, 0.0, 0.0), "a belt term over the sea at {d:?}");
        } else if p.terms.core > 0.0 {
            belts_on_land += 1;
        }
    }
    assert!(sea > 3000, "most of the sphere is sea: {sea}");
    assert!(belts_on_land > 50, "the retried plates put belts on the land: {belts_on_land} of the land points");
}

// spec: GEN-095
#[test]
fn the_flank_of_a_belt_is_mountains_before_its_core_is_high_enough_to_count() {
    let none = MaskTerms {
        margin_km: 500.0,
        convergence: 0.0,
        core: 0.0,
        flank: 0.0,
        old_margin: 0.0,
        back_arc: 0.0,
        plateau_field: 0.0,
        hilly: 0.0,
        strike: 1.0,
        plate: 0,
    };
    let on_flank = MaskTerms { flank: 0.2, ..none };
    let w = class_weights(&on_flank, 400.0, 0.0);
    assert!(w[MOUNTAINS] > 0.3 && w[MOUNTAINS] < 1.0, "a flank of 0.2 is partly mountains: {w:?}");
    let faint = class_weights(&MaskTerms { flank: 0.05, ..none }, 400.0, 0.0);
    assert!(faint[MOUNTAINS] < w[MOUNTAINS] && faint[MOUNTAINS] < 0.1, "the membership rises smoothly from the belt's edge: {faint:?}");
}

// spec: GEN-097
#[test]
fn the_k_step_quantile_runs_from_the_smallest_to_the_largest_step_and_is_monotone() {
    let lattice = ClassLattice::build_with(&ClassSource::new(SEED), 16, 2);
    let largest = lattice.largest_k_step();
    assert_eq!(lattice.k_step_quantile(1.0), largest, "the 1.0 quantile is the largest step");
    let qs: Vec<f64> = [0.0, 0.5, 0.9, 0.99, 1.0].iter().map(|&q| lattice.k_step_quantile(q)).collect();
    assert!(qs.windows(2).all(|w| w[0] <= w[1]), "the quantiles of the steps are monotone: {qs:?}");
    // A hand-made lattice of one cell per face: plain everywhere except one point of mountains, so the steps are 0 and 5 only.
    let mut bytes = vec![0u8; 6 * 4 * CLASS_COUNT];
    for p in bytes.as_chunks_mut::<CLASS_COUNT>().0 {
        p[PLAIN] = 255;
    }
    bytes[MOUNTAINS..=MOUNTAINS].copy_from_slice(&[255]);
    bytes[PLAIN] = 0;
    let tiny = ClassLattice::from_bytes(1, &bytes).expect("a lattice of one cell per face");
    assert_eq!(tiny.k_step_quantile(1.0), 5.0, "plain k 1, mountains k 6");
    assert_eq!(tiny.k_step_quantile(0.0), 0.0);
}

// spec: GEN-098
#[test]
fn the_mountain_band_is_ridged_four_and_a_half_times_as_high_and_centred() {
    let source = ClassSource::new(SEED);
    let (mut sum, mut max, mut min, mut amp_sum, mut count) = (0.0f64, f64::MIN, f64::MAX, 0.0f64, 0u32);
    for i in 0..60_000u32 {
        // A golden-angle spiral over the sphere: deterministic, evenly spread.
        let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / 60_000.0;
        let r = libm::sqrt(1.0 - z * z);
        let phi = f64::from(i) * 2.399_963_229_728_653;
        let d = Vec3::new(r * libm::cos(phi), r * libm::sin(phi), z);
        let p = source.eval(d);
        let wm = p.weights[MOUNTAINS] + p.weights[HIGH_MOUNTAINS];
        if wm < 0.99 || p.land_depth < 0.2 || p.e0 > 8000.0 {
            continue;
        }
        let k = blended_k(&p.weights);
        let amp = hill_amplitude_for_k(k) * MOUNTAIN_BAND_SCALE;
        let band = source.elevation_m(d, &p, k) - p.e0;
        sum += band / amp;
        amp_sum += amp;
        max = max.max(band / amp);
        min = min.min(band / amp);
        count += 1;
    }
    assert!(count > 500, "only {count} mountain points");
    let mean = sum / f64::from(count);
    assert!(amp_sum / f64::from(count) > 2000.0, "the mountain band is x4.5: mean amplitude {} m", amp_sum / f64::from(count));
    assert!(mean.abs() < 0.12, "the band is centred: mean {mean} of the amplitude");
    assert!(
        max <= 1.0 - MOUNTAIN_BAND_CENTRE + 1e-9 && min >= -MOUNTAIN_BAND_CENTRE - 1e-9,
        "the band lies in [-0.45, 0.55] of its amplitude: {min}..{max}"
    );
}
