//! Tier A, GEN-128 (ADR 0104 point 4): the arid class rules give badlands weight only on soft, semi-arid, moderately high, broken ground and sand-sea weight only on arid, low, sediment-fed ground, by smooth
//! ramps, summing to 1 when blended over the base weights, with the Earth preset as named parameters. GPU-free.

use planet_generators::arid::{AridInputs, AridParams, AridRules, ARID_RULES_VERSION, BADLANDS, SAND_SEA};
use planet_generators::classes::{CLASS_COUNT, CLASS_K};
use planet_generators::lithology::Provinces;

fn provinces(foreland: f64, old_margin: f64, basin: f64, rift: f64, core: f64) -> Provinces {
    Provinces { craton: 0.0, core, foreland, basin, rift, old_margin, back_arc: 0.0 }
}

fn inputs(aridity: f64, hardness: f64, e0: f64, p: Provinces) -> AridInputs {
    AridInputs { aridity, hardness, e0, provinces: p }
}

fn base() -> [f64; CLASS_COUNT] {
    [0.3, 0.2, 0.2, 0.1, 0.05, 0.15, 0.0, 0.0]
}

// spec: GEN-128
#[test]
fn badlands_gain_weight_only_on_soft_semi_arid_moderately_high_broken_ground() {
    let rules = AridRules::earth();
    let broken = provinces(0.8, 0.0, 0.0, 0.0, 0.0);
    let good = rules.weights(&inputs(0.3, 0.2, 800.0, broken)).0;
    assert!(good > 0.3, "the badlands' ground: {good}");
    // Each condition alone removes it.
    assert_eq!(rules.weights(&inputs(0.3, 0.8, 800.0, broken)).0, 0.0, "hard rock");
    assert_eq!(rules.weights(&inputs(1.2, 0.2, 800.0, broken)).0, 0.0, "humid");
    assert_eq!(rules.weights(&inputs(0.01, 0.2, 800.0, broken)).0, 0.0, "hyper-arid");
    assert_eq!(rules.weights(&inputs(0.3, 0.2, 50.0, broken)).0, 0.0, "a lowland");
    assert_eq!(rules.weights(&inputs(0.3, 0.2, 4000.0, broken)).0, 0.0, "a high mountain");
    assert_eq!(rules.weights(&inputs(0.3, 0.2, 800.0, provinces(0.0, 0.0, 0.0, 0.0, 0.0))).0, 0.0, "relief the water cannot cut");
}

// spec: GEN-128
#[test]
fn sand_sea_gains_weight_only_on_arid_low_sediment_fed_ground() {
    let rules = AridRules::earth();
    let basin = provinces(0.0, 0.0, 0.9, 0.0, 0.0);
    let good = rules.weights(&inputs(0.03, 0.5, 200.0, basin)).1;
    assert!(good > 0.5, "the ergs' ground: {good}");
    assert_eq!(rules.weights(&inputs(0.6, 0.5, 200.0, basin)).1, 0.0, "humid");
    assert_eq!(rules.weights(&inputs(0.03, 0.5, 1500.0, basin)).1, 0.0, "high");
    assert_eq!(rules.weights(&inputs(0.03, 0.5, 200.0, provinces(0.0, 0.0, 0.0, 0.0, 0.0))).1, 0.0, "no sediment to blow");
}

// spec: GEN-128
#[test]
fn the_blend_sums_to_the_base_sum_keeps_the_caps_and_replaces_the_base_arid_shares() {
    let rules = AridRules::earth();
    let mut worst = 0.0f64;
    for a in 0..=20 {
        for h in 0..=10 {
            for e in 0..=12 {
                let i = inputs(f64::from(a) * 0.05, f64::from(h) * 0.1, -200.0 + f64::from(e) * 300.0, provinces(0.9, 0.6, 0.9, 0.5, 0.4));
                let (bad, sand) = rules.weights(&i);
                assert!((0.0..=rules.params.cap + 1e-12).contains(&bad) && (0.0..=rules.params.cap + 1e-12).contains(&sand));
                assert!(bad + sand <= rules.params.cap_both + 1e-12, "both capped: {}", bad + sand);
                let w = rules.blend(&base(), &i);
                worst = worst.max((w.iter().sum::<f64>() - 1.0).abs());
                assert_eq!((w[BADLANDS], w[SAND_SEA]), (bad, sand));
                assert!(w.iter().all(|v| *v >= 0.0));
            }
        }
    }
    assert!(worst < 1e-12, "the weights sum to 1: {worst}");
    // A base that already holds badlands and sand sea has them replaced by the rules'.
    let mut b = base();
    b[BADLANDS] = 0.3;
    b[SAND_SEA] = 0.2;
    let w = rules.blend(&b, &inputs(1.5, 0.9, 5000.0, provinces(0.0, 0.0, 0.0, 0.0, 0.0)));
    assert_eq!((w[BADLANDS], w[SAND_SEA]), (0.0, 0.0));
    assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}

// spec: GEN-128
#[test]
fn the_weights_are_smooth_in_every_input_so_that_the_blended_k_steps_stay_small() {
    let rules = AridRules::earth();
    let p = provinces(0.8, 0.5, 0.9, 0.0, 0.2);
    // Per input step of 0.01 of aridity or 10 m (a small part of the narrowest ramp), the weights and the blended `k` move by a bounded part.
    let k_of = |w: &[f64; CLASS_COUNT]| w.iter().zip(CLASS_K).map(|(w, k)| w * k).sum::<f64>();
    let mut worst_k = 0.0f64;
    let mut worst_w = 0.0f64;
    let mut prev: Option<[f64; CLASS_COUNT]> = None;
    let mut aridity = 0.0;
    while aridity < 1.2 {
        let w = rules.blend(&base(), &inputs(aridity, 0.3, 700.0, p));
        if let Some(q) = prev {
            worst_k = worst_k.max((k_of(&w) - k_of(&q)).abs());
            worst_w = worst_w.max(w.iter().zip(&q).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max));
        }
        prev = Some(w);
        aridity += 0.01;
    }
    let mut e0 = -100.0;
    prev = None;
    while e0 < 3000.0 {
        let w = rules.blend(&base(), &inputs(0.3, 0.3, e0, p));
        if let Some(q) = prev {
            worst_k = worst_k.max((k_of(&w) - k_of(&q)).abs());
            worst_w = worst_w.max(w.iter().zip(&q).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max));
        }
        prev = Some(w);
        e0 += 10.0;
    }
    assert!(worst_w < 0.08, "a weight moves by {worst_w} for 0.01 of aridity or 10 m");
    assert!(worst_k < 0.25, "k moves by {worst_k} for 0.01 of aridity or 10 m (k ranges from 0.35 to 7.5)");
}

// spec: GEN-128
#[test]
fn the_earth_preset_is_the_default_parameters_and_another_planet_sets_its_own() {
    assert_eq!(AridRules::earth().params, AridParams::default());
    assert_eq!(ARID_RULES_VERSION, 1, "a change of the rules' output raises the version, which a consumer folds into its keys");
    let wet = AridParams { semi_from: (0.6, 0.9), semi_to: (1.5, 2.0), ..AridParams::default() };
    let (earth, other) = (AridRules::earth(), AridRules::new(wet));
    let i = inputs(1.0, 0.2, 800.0, provinces(0.8, 0.0, 0.0, 0.0, 0.0));
    assert_eq!(earth.weights(&i).0, 0.0, "a humid country is no badlands on Earth");
    assert!(other.weights(&i).0 > 0.1, "a planet whose semi-arid band lies higher gives it weight");
}

// spec: GEN-128
#[test]
fn a_bake_input_blends_the_rules_over_the_class_lattice() {
    use planet_core::hash::SplitMix64;
    use planet_core::Vec3;
    use planet_generators::bake::inputs::BakeInputs;
    let inputs = BakeInputs::new(5, 2);
    let mut rng = SplitMix64(9);
    let mut changed = 0;
    for _ in 0..200 {
        let d = loop {
            let v = Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0);
            if v.length() > 0.1 && v.length() <= 1.0 {
                break v.normalized();
            }
        };
        let (base, w) = (inputs.base_weights_at(d), inputs.weights_at(d));
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-9 && w.iter().all(|v| *v >= 0.0));
        assert!(base[BADLANDS] == 0.0 && base[SAND_SEA] == 0.0, "the class lattice holds no arid weight");
        changed += usize::from(w[BADLANDS] > 0.0 || w[SAND_SEA] > 0.0);
    }
    println!("{changed} of 200 points carry an arid weight");
}
