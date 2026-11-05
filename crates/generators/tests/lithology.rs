//! Tier A, GEN-116 and GEN-122 (ADR 0102): the lithology field is a pure function of the seed, the direction and the height, bit-identical for any thread count; its hardness is SPIKE-81's formula; its strata are
//! bands of 30 to 170 m whose erodibility is continuous in the height; and its preview writes the same bytes whatever the thread count. GPU-free.

use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::classes::ClassSource;
use planet_generators::lithology::{
    erodibility, Lithology, ProvinceLattice, Provinces, BAND_MAX_M, BAND_MIN_M, CAPROCK_HARDNESS, K_HARD, K_SOFT, LITHOLOGY_VERSION,
};
use planet_generators::lithology_preview::preview;

const SEED: u64 = 5;

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    loop {
        let v = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
        if v.length() > 0.1 && v.length() <= 1.0 {
            return v.normalized();
        }
    }
}

/// A lithology over a coarse province lattice (16 cells a face edge) of `threads` threads.
fn lithology(threads: usize) -> (ClassSource, Lithology) {
    let source = ClassSource::new(SEED);
    let provinces = ProvinceLattice::build_with(&source, 16, threads);
    (source, Lithology::with_provinces(SEED, provinces))
}

// spec: GEN-116
#[test]
fn the_province_lattice_and_the_field_are_bit_identical_for_any_thread_count_and_depend_on_the_seed() {
    let (_, one) = lithology(1);
    let (_, eight) = lithology(8);
    for f in 0..6 {
        for j in 0..=16 {
            for i in 0..=16 {
                assert_eq!(one.provinces().point(planet_core::Face(f), i, j), eight.provinces().point(planet_core::Face(f), i, j));
            }
        }
    }
    let mut rng = SplitMix64(11);
    for _ in 0..200 {
        let (d, z) = (random_dir(&mut rng), rng.next_f64() * 6000.0 - 500.0);
        let (a, b) = (one.strata(d, z), eight.strata(d, z));
        assert_eq!(
            (a.k.to_bits(), a.hardness.to_bits(), a.caprock, a.soluble.to_bits(), a.band),
            (b.k.to_bits(), b.hardness.to_bits(), b.caprock, b.soluble.to_bits(), b.band)
        );
        assert_eq!(one.hardness(d).to_bits(), eight.hardness(d).to_bits());
    }
    // Another seed is another rock.
    let other = Lithology::with_provinces(SEED + 1, ProvinceLattice::build_with(&ClassSource::new(SEED + 1), 16, 2));
    let d = random_dir(&mut rng);
    assert_ne!(one.hardness(d).to_bits(), other.hardness(d).to_bits());
    assert_eq!(LITHOLOGY_VERSION, 1, "a change of the field's output raises the version, which a consumer folds into its keys");
}

// spec: GEN-116
#[test]
fn the_hardness_is_spike_81s_formula_of_the_province_terms_and_stays_in_range_with_k_between_its_bounds() {
    let p = Provinces { craton: 0.8, core: 0.2, foreland: 0.1, basin: 0.3, rift: 0.0, old_margin: 0.0 };
    // 0.45 + 0.4 * 0.8 * 0.7 + 0.35 * 0.2 - 0.4 * 0.1 - 0.35 * 0.3 + 0.25 * 0.5
    let want = 0.45 + 0.4 * 0.8 * 0.7 + 0.35 * 0.2 - 0.4 * 0.1 - 0.35 * 0.3 + 0.25 * 0.5;
    assert!((Lithology::hardness_of(&p, 0.5) - want).abs() < 1e-12);
    assert_eq!(
        Lithology::hardness_of(&Provinces { craton: 1.0, core: 1.0, foreland: 0.0, basin: 0.0, rift: 0.0, old_margin: 0.0 }, 1.0),
        1.0
    );
    assert_eq!(
        Lithology::hardness_of(&Provinces { craton: 0.0, core: 0.0, foreland: 1.0, basin: 1.0, rift: 0.0, old_margin: 0.0 }, -1.0),
        0.0
    );
    // The erodibility falls with the hardness, from the soft bound to the hard one.
    assert!((erodibility(0.0) - K_SOFT).abs() < 1e-12 && (erodibility(1.0) - K_HARD).abs() < 1e-12);
    assert!(erodibility(0.3) > erodibility(0.6));
    let (_, lith) = lithology(2);
    let mut rng = SplitMix64(3);
    for _ in 0..500 {
        let (d, z) = (random_dir(&mut rng), rng.next_f64() * 8000.0 - 4000.0);
        let s = lith.strata(d, z);
        assert!((0.0..=1.0).contains(&lith.hardness(d)) && (0.0..=1.0).contains(&s.hardness));
        assert!(s.k.is_finite() && s.k >= K_HARD - 1e-12 && s.k <= K_SOFT + 1e-12, "K {} at hardness {}", s.k, s.hardness);
        assert!((0.0..=1.0).contains(&s.soluble));
        assert_eq!(s.caprock, s.hardness > CAPROCK_HARDNESS, "the caprock flag follows the band's hardness");
        assert_eq!(lith.k_at(d, z).to_bits(), s.k.to_bits());
    }
}

// spec: GEN-116
#[test]
fn the_strata_are_bands_of_30_to_170_metres_whose_erodibility_is_continuous_in_the_height_and_the_stack_is_tilted() {
    let (_, lith) = lithology(2);
    let mut rng = SplitMix64(8);
    let mut tilted = (f64::MAX, f64::MIN);
    for _ in 0..40 {
        let d = random_dir(&mut rng);
        let col = lith.column(d);
        tilted = (tilted.0.min(col.offset_m), tilted.1.max(col.offset_m));
        let (mut prev, mut band, mut start) = (col.at(0.0), col.at(0.0).band, 0.0);
        let (mut changes, mut thickness) = (0, Vec::new());
        let mut z = 0.25;
        while z < 1500.0 {
            let s = col.at(z);
            // No jump: the blend over a band's edge keeps the erodibility continuous (a step of a quarter metre moves it by under 0.4).
            assert!((s.k - prev.k).abs() < 0.4, "a jump of {} in K at z {z}", (s.k - prev.k).abs());
            if s.band != band {
                changes += 1;
                if changes > 1 {
                    thickness.push(z - start);
                }
                (band, start) = (s.band, z);
            }
            prev = s;
            z += 0.25;
        }
        assert!(changes >= 8, "the stack changes band over 1.5 km: {changes}");
        for t in thickness {
            assert!((BAND_MIN_M - 0.5..=BAND_MAX_M + 0.5).contains(&t), "a band {t} m thick");
        }
    }
    assert!(tilted.1 - tilted.0 > 100.0, "the stack's offset varies over the planet: {tilted:?}");
}

// spec: GEN-116
#[test]
fn the_province_lattice_reads_back_the_class_sources_terms_at_its_nodes_and_is_continuous_across_a_face_edge() {
    let (source, lith) = lithology(2);
    let n = lith.provinces().cells();
    for (f, i, j) in [(0u8, 3usize, 5usize), (2, 10, 1), (4, 7, 7), (5, 16, 16)] {
        let (s, t) = (2.0 * i as f64 / n as f64 - 1.0, 2.0 * j as f64 / n as f64 - 1.0);
        let d = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(f), s, t);
        let want = Provinces::of_source(&source, d);
        let got = lith.provinces().at(d);
        for (a, b) in [
            (got.craton, want.craton),
            (got.core, want.core),
            (got.foreland, want.foreland),
            (got.basin, want.basin),
            (got.rift, want.rift),
            (got.old_margin, want.old_margin),
        ] {
            assert!((a - b).abs() <= 0.5 / 255.0 + 1e-9, "{a} against {b} at node ({f}, {i}, {j})");
        }
    }
    // Two points a hair apart on either side of a cube edge read nearly the same terms.
    let edge = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::Face(0), 1.0, 0.2);
    let (a, b) = (edge + Vec3::new(0.0, 0.0, 1e-7), edge + Vec3::new(1e-7, 0.0, 0.0));
    let (pa, pb) = (lith.provinces().at(a.normalized()), lith.provinces().at(b.normalized()));
    assert!((pa.core - pb.core).abs() < 0.01 && (pa.basin - pb.basin).abs() < 0.01);
}

// spec: GEN-122
#[test]
fn the_preview_is_the_same_bytes_for_any_thread_count_and_labels_its_images_as_inputs_with_the_numbers() {
    // A province lattice of 64 cells a face edge, fine enough to hold a belt and a basin for the section.
    let source = ClassSource::new(SEED);
    let lith = Lithology::with_provinces(SEED, ProvinceLattice::build_with(&source, 64, 2));
    let a = preview(&source, &lith, 400.0, 1);
    let b = preview(&source, &lith, 400.0, 4);
    assert_eq!(
        (a.hardness.clone(), a.strata_surface.clone(), a.strata_below.clone(), a.soluble.clone(), a.section.clone(), a.text.clone()),
        (b.hardness, b.strata_surface, b.strata_below, b.soluble, b.section, b.text)
    );
    assert_eq!((a.hardness.width, a.hardness.height), (100, 50), "400 km a pixel: 2 pi R / 400 by half as many");
    assert_eq!((a.section.width, a.section.height), (900, 450));
    assert!(a.section.data.chunks(4).any(|p| p[..3] == [0, 0, 0]), "the surface is drawn on top");
    // The text says what the images are, and carries the shares and the band thickness against the carve's per-step change.
    for needle in [
        "lithology field (input), not terrain",
        "hardness H over land: hard",
        "caprock bands",
        "soluble",
        "cross-section:",
        "band thickness 30 to 170 m",
        "pass 1 height change a step",
    ] {
        assert!(a.text.contains(needle), "{needle}: {}", a.text);
    }
    // The files.
    let dir = std::env::temp_dir().join(format!("planet-lithology-preview-{}", std::process::id()));
    planet_generators::lithology_preview::write(&dir, &a).unwrap();
    for f in ["hardness.png", "strata_surface.png", "strata_200m_below.png", "soluble.png", "section.png", "lithology.txt"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
