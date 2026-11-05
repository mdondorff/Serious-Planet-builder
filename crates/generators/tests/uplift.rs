//! Tier A, GEN-123 and GEN-125 (planner inbox #70): the uplift field is a pure function of the seed and the direction, bit-identical for any thread count; it is highest in a belt, lowest on a craton
//! with the back-arc between, continuous across a face edge, finite and not negative; its preview writes the same bytes whatever the thread count. GPU-free.

use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::classes::ClassSource;
use planet_generators::lithology::{Lithology, ProvinceLattice, Provinces};
use planet_generators::uplift::{Uplift, NOISE_AMPLITUDE, UPLIFT_VERSION, U_BACKARC, U_BELT, U_CRATON, U_FLAT};
use planet_generators::uplift_preview::preview;

const SEED: u64 = 5;

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
        if v.length() > 0.1 && v.length() <= 1.0 {
            return v.normalized();
        }
    }
}

fn terms(craton: f64, core: f64, back_arc: f64) -> Provinces {
    Provinces { craton, core, foreland: 0.0, basin: 0.0, rift: 0.0, old_margin: 0.0, back_arc }
}

// spec: GEN-123
#[test]
fn the_field_is_bit_identical_for_any_thread_count_and_depends_on_the_seed_and_stays_finite_and_not_negative() {
    let source = ClassSource::new(SEED);
    let (one, eight) = (ProvinceLattice::build_with(&source, 16, 1), ProvinceLattice::build_with(&source, 16, 8));
    let u = Uplift::new(SEED);
    let mut rng = SplitMix64(21);
    for _ in 0..300 {
        let d = random_dir(&mut rng);
        let (a, b) = (u.at(&one, d), u.at(&eight, d));
        assert_eq!(a.to_bits(), b.to_bits());
        assert!(a.is_finite() && a >= 0.0, "{a}");
    }
    let d = random_dir(&mut rng);
    assert_ne!(u.at(&one, d).to_bits(), Uplift::new(SEED + 1).at(&one, d).to_bits());
    assert_eq!(UPLIFT_VERSION, 1, "a change of the field's output raises the version, which a consumer folds into its keys");
}

// spec: GEN-123
#[test]
fn the_uplift_is_highest_in_a_belt_lowest_on_a_craton_with_the_back_arc_between() {
    let (belt, arc, craton, flat) = (terms(0.0, 1.0, 0.0), terms(0.0, 0.0, 1.0), terms(1.0, 0.0, 0.0), terms(0.0, 0.0, 0.0));
    let at = |p: &Provinces| Uplift::of_terms(p, 0.0);
    assert_eq!((at(&belt), at(&arc), at(&craton), at(&flat)), (U_BELT, U_BACKARC, U_CRATON, U_FLAT));
    assert!(at(&belt) > at(&arc) && at(&arc) > at(&craton), "belt over back-arc over craton");
    // A belt on a craton edge keeps the belt value; the noise scales by at most its amplitude.
    assert_eq!(at(&terms(1.0, 1.0, 0.0)), U_BELT);
    let (hi, lo) = (Uplift::of_terms(&belt, 1.0), Uplift::of_terms(&belt, -1.0));
    assert!((hi - U_BELT * (1.0 + NOISE_AMPLITUDE)).abs() < 1e-12 && (lo - U_BELT * (1.0 - NOISE_AMPLITUDE)).abs() < 1e-12);
    // The terms of the class source: a belt point reads high, a craton point low.
    let source = ClassSource::new(SEED);
    let lith = Lithology::with_provinces(SEED, ProvinceLattice::build_with(&source, 64, 2));
    let u = Uplift::new(SEED);
    let (mut max, mut min) = (0.0f64, f64::MAX);
    let n = lith.provinces().cells();
    for f in 0..6u8 {
        for j in (0..=n).step_by(2) {
            for i in (0..=n).step_by(2) {
                let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
                let d = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(f), s, t);
                let v = u.at(lith.provinces(), d);
                max = max.max(v);
                min = min.min(v);
            }
        }
    }
    assert!(max > 0.9 * U_BELT && min < 2.0 * U_CRATON, "the planet holds belts and cratons: {min}..{max}");
}

// spec: GEN-123
#[test]
fn the_uplift_is_continuous_across_a_face_edge() {
    let source = ClassSource::new(SEED);
    let lith = Lithology::with_provinces(SEED, ProvinceLattice::build_with(&source, 16, 2));
    let u = Uplift::new(SEED);
    let edge = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(0), 1.0, 0.2);
    let (a, b) = ((edge + Vec3::new(1e-7, 0.0, 0.0)).normalized(), (edge + Vec3::new(0.0, 1e-7, 0.0)).normalized());
    assert_ne!(
        planet_core::cube::direction_to_face(&planet_core::cube::TangentWarp, a).0,
        planet_core::cube::direction_to_face(&planet_core::cube::TangentWarp, b).0,
        "the points are on two faces"
    );
    assert!((u.at(lith.provinces(), a) - u.at(lith.provinces(), b)).abs() < 1e-3);
}

// spec: GEN-125
#[test]
fn the_preview_is_the_same_bytes_for_any_thread_count_and_labels_its_images_as_inputs_with_the_numbers() {
    let source = ClassSource::new(SEED);
    let lith = Lithology::with_provinces(SEED, ProvinceLattice::build_with(&source, 64, 2));
    let u = Uplift::new(SEED);
    let a = preview(&source, &lith, &u, 400.0, 1);
    let b = preview(&source, &lith, &u, 400.0, 4);
    assert_eq!((a.uplift.clone(), a.steepness.clone(), a.section.clone(), a.text.clone()), (b.uplift, b.steepness, b.section, b.text));
    assert_eq!((a.uplift.width, a.uplift.height), (100, 50));
    for needle in
        ["uplift field (input), not terrain", "belt or arc coast", "back-arc", "section: from an active belt", "U / K at the surface"]
    {
        assert!(a.text.contains(needle), "{needle}: {}", a.text);
    }
    let dir = std::env::temp_dir().join(format!("planet-uplift-preview-{}", std::process::id()));
    planet_generators::uplift_preview::write(&dir, &a).unwrap();
    for f in ["uplift.png", "steepness.png", "section.png", "uplift.txt"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
