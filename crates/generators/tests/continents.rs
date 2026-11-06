//! Tier A, GEN-104 and GEN-105 (the continent stage and its deterministic retries): the same bits on any thread count and on every call, a pinned fingerprint of
//! the continentalness (bump `GLOBAL_VERSION` when it changes), and the smooth value's relation to the full one.

use planet_core::Vec3;
use planet_generators::classes::*;
use planet_generators::continents::{continent, continent_smooth};
use planet_generators::global::GLOBAL_VERSION;

/// 2000 directions of a Fibonacci sphere, with the poles and the cube's face centres and corners added.
fn directions() -> Vec<Vec3> {
    let n = 2000usize;
    let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
    let mut v: Vec<Vec3> = (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt((1.0 - z * z).max(0.0));
            Vec3::new(r * libm::cos(golden * i as f64), r * libm::sin(golden * i as f64), z)
        })
        .collect();
    v.extend([Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0)]);
    for (x, y, z) in [(1.0, 1.0, 1.0), (-1.0, 1.0, -1.0), (1.0, -1.0, -1.0)] {
        v.push(Vec3::new(x, y, z).normalized());
    }
    v
}

fn bits(seed: u64, dirs: &[Vec3], threads: usize) -> Vec<u64> {
    let chunk = dirs.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> =
            dirs.chunks(chunk).map(|c| s.spawn(move || c.iter().map(|&d| continent(seed, d).to_bits()).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().expect("a thread of the continent stage")).collect()
    })
}

// spec: GEN-104
#[test]
fn the_continentalness_is_the_same_bits_on_1_and_8_threads_and_on_every_call() {
    let dirs = directions();
    // A seed that no other test uses, so that 8 threads build its plan concurrently (the plan is memoized for the process).
    let fresh = bits(9, &dirs, 8);
    assert_eq!(fresh, bits(9, &dirs, 1), "a plan built by 8 threads at once differs from a later single-thread read");
    let one = bits(3, &dirs, 1);
    assert_eq!(one, bits(3, &dirs, 8), "the continentalness differs between 1 and 8 threads");
    assert_eq!(one, bits(3, &dirs, 3), "the continentalness differs on a second call");
    assert_ne!(one, bits(4, &dirs, 1), "another seed gives the same field");
}

// spec: GEN-104
#[test]
fn the_continentalness_is_pinned_so_that_a_change_bumps_the_global_version() {
    // Bump GLOBAL_VERSION whenever this fingerprint changes (the stored fields carry the version).
    let dirs = directions();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for seed in [1u64, 14] {
        for b in bits(seed, &dirs, 4) {
            h = (h ^ b).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    println!("continent fingerprint {h:#018x} at GLOBAL_VERSION {GLOBAL_VERSION}");
    assert_eq!(
        (GLOBAL_VERSION, h),
        (8, PINNED),
        "the continent stage's output changed: bump GLOBAL_VERSION and record the new fingerprint"
    );
}

const PINNED: u64 = 3875448375184263834;

// spec: GEN-104
#[test]
fn the_smooth_value_has_the_same_plan_and_follows_the_full_value_at_the_scale_of_the_continents() {
    // Same offset: both put about 29 % of the sphere above the sea level, and they agree on the sign over most of it.
    let dirs = directions();
    let sea = planet_generators::global::SEA_LEVEL_C;
    let (mut land_full, mut land_smooth, mut agree) = (0usize, 0usize, 0usize);
    for &d in &dirs {
        let (f, s) = (continent(3, d) > sea, continent_smooth(3, d) > sea);
        land_full += usize::from(f);
        land_smooth += usize::from(s);
        agree += usize::from(f == s);
    }
    let n = dirs.len() as f64;
    assert!((land_full as f64 / n - 0.29).abs() < 0.03, "the full value's land share {}", land_full as f64 / n);
    assert!((land_smooth as f64 / n - 0.29).abs() < 0.06, "the smooth value's land share {}", land_smooth as f64 / n);
    assert!(agree as f64 / n > 0.93, "the smooth and full values agree on land and sea at {} of the directions only", agree as f64 / n);
}

// spec: GEN-105
#[test]
fn the_plate_draws_and_the_layout_retry_give_the_same_class_weights_on_every_build() {
    let (a, b) = (ClassSource::new(2), ClassSource::new(2));
    for &d in directions().iter().step_by(37) {
        assert_eq!(a.eval(d), b.eval(d), "two sources of seed 2 differ at {d:?}");
    }
    // The retry is effective: some seed keeps a draw other than its own, and every kept draw is one of the PLATE_DRAWS (64).
    let kept: Vec<u64> = (1..=8).map(|s| ClassSource::new(s).plate_draw()).collect();
    assert!(kept.iter().all(|&t| t < 64) && kept.iter().any(|&t| t > 0), "kept plate draws of seeds 1 to 8: {kept:?}");
    assert_eq!(ClassSource::new(2).plate_draw(), a.plate_draw());
    let c = ClassSource::new(5);
    assert!(directions().iter().step_by(37).any(|&d| a.eval(d) != c.eval(d)), "another seed gives the same source");
}

// spec: GEN-105
#[test]
fn the_kept_plate_draw_of_seeds_1_to_5_is_pinned_so_that_a_swapped_plate_set_needs_a_declared_re_pin() {
    let kept: Vec<u64> = (1..=5).map(|s| ClassSource::new(s).plate_draw()).collect();
    assert_eq!(kept, vec![36, 19, 48, 11, 48], "a seed's plate draw changed: declare the re-pin (round 3 start values)");
}
