//! Tier B, TEST-086: the preview for seed 1 runs and reports its class shares against the bands of TEST-082 (reported, not judged: the judgement belongs to
//! TEST-082 with the bake).

use planet_generators::classes::CLASS_COUNT;
use planet_generators::preview::{compute, relief, report, shares};

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
    for (needle, band) in [
        ("plains", "band 45-62 %"),
        ("hills", "band 15-30 %"),
        ("mountains", "band 20-32 %"),
        ("land above 2 km", "band 5-10 %"),
        ("land above 4 km", "band 1-3 %"),
        ("highest land", "band 7-9 km"),
        ("largest step", "limit 2.0"),
        ("S1 ", "Earth 5"),
        ("S2 ", "Earth about 57 %"),
        ("S3 ", "Earth 1.1-1.3"),
        ("S4 ", "target median about 310 km"),
        ("S5 ", "target the longest at most 7500 km"),
        ("S6 ", "Earth about 0-1"),
        ("S7 ", "Earth about 7 %"),
    ] {
        let line = text.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("the report names '{needle}'"));
        assert!(line.contains(band), "'{needle}' carries its band '{band}': {line}");
    }
    assert_eq!(s.per_class.len(), CLASS_COUNT);
}

// spec: TEST-082
#[test]
fn seeds_1_to_5_pass_the_relief_bands_of_spike_93_in_the_gate_form() {
    // (name, band low, band high, value per seed)
    let mut rows: Vec<(&str, f64, f64, Vec<f64>)> = [
        ("land under 500 m", 50.0, 65.0),
        ("median land elevation m", 300.0, 550.0),
        ("coastal land median m", 0.0, 200.0),
        ("land above 1 km", 17.0, 27.0),
        ("land above 2 km", 5.0, 10.0),
        ("land above 3 km", 2.0, 5.0),
        ("land above 4 km", 1.0, 3.0),
        ("highest land km", 7.0, 9.0),
        ("plains", 45.0, 62.0),
        ("hills", 15.0, 30.0),
        ("mountains", 20.0, 32.0),
    ]
    .into_iter()
    .map(|(n, lo, hi)| (n, lo, hi, Vec::new()))
    .collect();
    let mut belt = Vec::new();
    let mut largest_k = Vec::new();
    for seed in 1..=5u64 {
        let f = compute(seed, 1440, 720, 256, 8);
        let (s, r) = (shares(&f), relief(&f));
        let values = [
            100.0 * r.under_500m,
            r.median_m,
            r.coastal_median_m,
            100.0 * r.above_1km,
            100.0 * s.above_2km,
            100.0 * r.above_3km,
            100.0 * s.above_4km,
            s.highest_m / 1000.0,
            100.0 * s.plains,
            100.0 * s.hills,
            100.0 * s.mountains,
        ];
        for (row, v) in rows.iter_mut().zip(values) {
            row.3.push(v);
        }
        belt.push(100.0 * r.belt_land);
        largest_k.push(s.largest_k_step);
    }
    println!(
        "belt land % per seed (reported, gated with the convergent-only arc round): {belt:.1?}; largest k step per seed: {largest_k:.2?}"
    );
    let mut failures = Vec::new();
    for (name, lo, hi, v) in &rows {
        let w = hi - lo;
        let mut sorted = v.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[2];
        println!("{name}: {v:.1?}, median {median:.1}, band {lo}-{hi}");
        if median < lo - 0.1 * w || median > hi + 0.1 * w {
            failures.push(format!("{name}: median {median:.1} outside the band {lo}-{hi} widened by 10 %"));
        }
        for (i, x) in v.iter().enumerate() {
            if *x < lo - w || *x > hi + w {
                failures.push(format!("{name}: seed {} has {x:.1}, more than one band width outside {lo}-{hi}", i + 1));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}",
        failures.join(
            "
"
        )
    );
}
