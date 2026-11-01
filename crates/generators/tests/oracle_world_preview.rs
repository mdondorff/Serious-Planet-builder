//! Tier B, TEST-086: the preview for seed 1 runs and reports its class shares against the bands of TEST-082 (reported, not judged: the judgement belongs to
//! TEST-082 with the bake).

use planet_generators::classes::CLASS_COUNT;
use planet_generators::preview::{compute, report, shares};

// spec: TEST-086
#[test]
fn the_preview_of_seed_1_reports_the_class_shares_beside_the_bands() {
    let fields = compute(1, 720, 360, 128, 4);
    let s = shares(&fields);
    let text = report(&fields);
    println!("{text}");
    assert!((s.per_class.iter().sum::<f64>() - 1.0).abs() < 1e-6, "the class shares of land sum to 1: {:?}", s.per_class);
    assert!((0.2..0.4).contains(&s.land_fraction), "land fraction {}", s.land_fraction);
    assert_eq!((s.per_class[6], s.per_class[7]), (0.0, 0.0), "badlands and the sand sea are 0 in the default source");
    for needle in ["plains", "hills", "mountains", "land above 2 km", "land above 4 km", "highest land", "largest step"] {
        assert!(text.contains(needle), "the report names '{needle}'");
    }
    assert_eq!(s.per_class.len(), CLASS_COUNT);
}
