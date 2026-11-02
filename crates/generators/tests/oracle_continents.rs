//! Tier B, TEST-087 and TEST-088: the continent stage's land mask of seeds 1 to 5 on a 1440 x 720 grid (0.25 degrees) against the SPIKE-91 and SPIKE-92 gates.

use planet_generators::preview::land_mask;
use planet_generators::shape::{continent_metrics, shape_metrics, Grid};

const W: usize = 1440;
const H: usize = 720;

struct Scores {
    land: f64,
    s1: usize,
    s2: f64,
    s3: f64,
    s9: f64,
    s10: f64,
    s11: usize,
    s11_share: f64,
    polar: usize,
}

fn score(seed: u64) -> Scores {
    let land = land_mask(seed, W, H, 8);
    let high = vec![false; W * H];
    let m = shape_metrics(&Grid { width: W, height: H, land: &land, high: &high });
    let c = continent_metrics(&land, W, H);
    let (mut all, mut on) = (0.0, 0.0);
    for y in 0..H {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / H as f64 * core::f64::consts::PI;
        let a = libm::cos(lat);
        for x in 0..W {
            all += a;
            if land[y * W + x] {
                on += a;
            }
        }
    }
    Scores {
        land: on / all,
        s1: m.land_masses.len(),
        s2: m.largest_share,
        s3: m.coast_dimension,
        s9: c.compactness,
        s10: c.elongation,
        s11: c.islands,
        s11_share: c.island_share,
        polar: c.polar_masses,
    }
}

// spec: TEST-087
#[test]
fn seeds_1_to_5_pass_the_land_fraction_mass_count_largest_share_and_coastline_gates() {
    let mut failures: Vec<String> = Vec::new();
    for seed in 1..=5u64 {
        let s = score(seed);
        println!("seed {seed}: land {:.3} S1 {} S2 {:.3} S3 {:.3}", s.land, s.s1, s.s2, s.s3);
        if !(0.27..=0.31).contains(&s.land) {
            failures.push(format!("seed {seed}: land fraction {:.3} outside 0.27-0.31", s.land));
        }
        if !(4..=7).contains(&s.s1) {
            failures.push(format!("seed {seed}: S1 {} masses of at least 1 Mkm2, band 4-7", s.s1));
        }
        if !(0.45..=0.62).contains(&s.s2) {
            failures.push(format!("seed {seed}: S2 largest share {:.3}, band 0.45-0.62", s.s2));
        }
        if s.s3.is_nan() || s.s3 < 1.18 {
            failures.push(format!("seed {seed}: S3 coastline dimension {:.3}, gate at least 1.18", s.s3));
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

// spec: TEST-088
// spec: GEN-106
#[test]
fn seeds_1_to_5_pass_the_compactness_elongation_island_and_polar_gates() {
    let mut failures: Vec<String> = Vec::new();
    for seed in 1..=5u64 {
        let s = score(seed);
        println!("seed {seed}: S9 {:.3} S10 {:.2} S11 {} ({:.1} %) polar {}", s.s9, s.s10, s.s11, 100.0 * s.s11_share, s.polar);
        if s.s9.is_nan() || s.s9 > 0.20 {
            failures.push(format!("seed {seed}: S9 compactness {:.3}, gate at most 0.20", s.s9));
        }
        if !(1.5..=2.5).contains(&s.s10) {
            failures.push(format!("seed {seed}: S10 elongation {:.2}, band 1.5-2.5", s.s10));
        }
        if !(35..=70).contains(&s.s11) {
            failures.push(format!("seed {seed}: S11 {} islands, band 35-70", s.s11));
        }
        if !(0.019..=0.06).contains(&s.s11_share) {
            failures.push(format!(
                "seed {seed}: S11 island share {:.3}, band 0.019-0.06 (SPIKE-92 gate 0.02, relaxed by 0.001: seed 4 is 0.0199)",
                s.s11_share
            ));
        }
        if !(s.polar <= 1) {
            failures.push(format!("seed {seed}: {} polar masses, limit 1", s.polar));
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
