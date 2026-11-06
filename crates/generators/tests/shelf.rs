//! GEN-118: the continental shelf of the default class source.

use planet_core::Vec3;
use planet_generators::classes::ClassSource;
use planet_generators::shelf::ShelfProfile;

fn fib(i: u32, n: u32) -> Vec3 {
    let (lon, z) = (f64::from(i) * 2.399_963, 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n));
    let r = (1.0 - z * z).sqrt();
    Vec3::new(r * libm::cos(lon), r * libm::sin(lon), z)
}

// spec: GEN-118
#[test]
fn the_land_of_the_field_is_the_land_of_before_to_the_bit() {
    let s = ClassSource::new(7);
    let pins: [(u32, u64); 8] = [
        (154, 0x406a_3541_54e1_3b94),
        (413, 0x4073_93d5_2809_43a2),
        (434, 0x4083_721f_fef8_4693),
        (455, 0x3f9f_b0a5_b28a_6e3e),
        (476, 0x4040_d918_efe5_adcb),
        (497, 0x4083_6ab4_5546_db50),
        (574, 0x4003_a8df_0311_2a54),
        (581, 0x3fff_1514_e4c0_cf31),
    ];
    for (i, bits) in pins {
        let p = s.eval(fib(i, 4000));
        assert!(p.land_depth > 0.0, "point {i} is land");
        assert_eq!(p.e0.to_bits(), bits, "land point {i}: the shelf changes the sea only");
    }
}

// spec: GEN-118
#[test]
fn the_profile_runs_from_the_coast_over_the_shelf_to_the_break_and_down_the_slope_to_the_abyss() {
    let p = ShelfProfile::earth();
    let w = 120.0;
    assert_eq!(p.floor_m(0.0, w), 0.0, "sea level at the coast");
    assert_eq!(p.floor_m(w, w), -p.break_depth_m, "the break depth at the shelf width");
    assert_eq!(p.floor_m(w + p.slope_width_km, w), -p.abyssal_depth_m, "the abyssal depth at the foot of the slope");
    assert_eq!(p.floor_m(w + 10.0 * p.slope_width_km, w), -p.abyssal_depth_m, "flat beyond");
    let mut prev = 0.0;
    let mut worst_step = 0.0f64;
    for i in 0..=2000 {
        let x = f64::from(i) * 0.25;
        let f = p.floor_m(x, w);
        assert!(f <= prev + 1e-9, "the floor never rises seaward (x {x} km)");
        worst_step = worst_step.max(prev - f);
        prev = f;
    }
    // 0.25 km steps: the steepest part of the slope (smoothstep: 1.5 x the mean) is about 1.5 * 4370 / 90 * 0.25 m.
    assert!(worst_step < 20.0, "continuous: no step over {worst_step} m in a quarter km");
    assert_eq!(p.floor_m(-5.0, w), 0.0, "a point at the coast side is the coast");
}

// spec: GEN-118
#[test]
fn the_width_follows_the_margin_type_smoothly() {
    let p = ShelfProfile::earth();
    assert_eq!(p.width_km(0.0), p.passive_width_km);
    assert_eq!(p.width_km(1.0), p.active_width_km);
    assert!(p.width_km(0.0) > p.width_km(1.0), "passive margins are wider than active ones");
    assert_eq!(p.width_km(7.0), p.active_width_km, "the weight is clamped");
    let mut prev = p.width_km(0.0);
    for i in 1..=100 {
        let w = p.width_km(f64::from(i) / 100.0);
        assert!(w <= prev + 1e-12 && prev - w < 3.0, "a smooth blend, no switch");
        prev = w;
    }
    assert_eq!(p.margin_term(5000.0, 1.0), 0.0, "far from any margin the coast is passive");
    assert_eq!(p.margin_term(0.0, 0.0), 0.0, "a margin that does not converge is passive");
    assert_eq!(p.margin_term(0.0, 1.0), 1.0, "a converging margin at the coast is active");
    let mid = p.margin_term(300.0, 0.2);
    assert!(mid > 0.0 && mid < 1.0, "in between: {mid}");
}

// spec: GEN-118
#[test]
fn the_parameters_are_validated() {
    for bad in [
        ShelfProfile { break_depth_m: 0.0, ..ShelfProfile::earth() },
        ShelfProfile { break_depth_m: 5000.0, ..ShelfProfile::earth() },
        ShelfProfile { passive_width_km: f64::NAN, ..ShelfProfile::earth() },
        ShelfProfile { slope_width_km: -1.0, ..ShelfProfile::earth() },
        ShelfProfile { active_within_km: (300.0, 300.0), ..ShelfProfile::earth() },
        ShelfProfile { active_convergence: (0.4, 0.1), ..ShelfProfile::earth() },
        ShelfProfile { abyssal_depth_m: f64::INFINITY, ..ShelfProfile::earth() },
        ShelfProfile { break_depth_m: f64::NAN, ..ShelfProfile::earth() },
        ShelfProfile { coast_ramp: -0.1, ..ShelfProfile::earth() },
        ShelfProfile { min_sea_ramp: 0.0, ..ShelfProfile::earth() },
        ShelfProfile { coast_lattice_cells: 100, ..ShelfProfile::earth() },
        ShelfProfile { coast_lattice_cells: 8192, ..ShelfProfile::earth() },
    ] {
        assert!(std::panic::catch_unwind(|| ShelfProfile::new(bad)).is_err(), "{bad:?} is refused");
    }
}

// spec: GEN-118
#[test]
fn the_sea_has_a_shelf_and_no_step_at_the_coast() {
    let s = ClassSource::new(7);
    let n = 6000;
    let (mut sea, mut shelf, mut deepest) = (0usize, 0usize, 0.0f64);
    for i in 0..n {
        let p = s.eval(fib(i, n));
        if p.land_depth < 0.0 {
            sea += 1;
            assert!(p.e0 <= 0.0, "the sea floor is below sea level: {}", p.e0);
            assert!(p.e0 >= -4500.0, "never deeper than the abyssal depth: {}", p.e0);
            if p.e0 > -200.0 {
                shelf += 1;
            }
            deepest = deepest.min(p.e0);
        }
    }
    let share = shelf as f64 / sea as f64;
    assert!(share > 0.03 && share < 0.25, "a shelf share of the ocean of Earth's order: {share}");
    assert!(deepest < -4000.0, "the abyss is reached: {deepest}");
}

// spec: GEN-118
#[test]
fn the_field_is_the_same_bits_twice() {
    let (a, b) = (ClassSource::new(7), ClassSource::new(7));
    for i in 0..300 {
        let d = fib(i, 300);
        assert_eq!(a.eval(d).e0.to_bits(), b.eval(d).e0.to_bits());
    }
}

// spec: GEN-118
#[test]
fn the_margin_activity_and_the_ocean_floor_have_no_jump_where_the_nearest_plate_changes() {
    let s = ClassSource::new(7);
    let n = 40_000;
    // A continuous function's step shrinks with the offset; a jump keeps its size: compare the worst steps at 0.6 km and at 64 m.
    let steps = |offset: f64| {
        let (mut worst_activity, mut worst_floor, mut active) = (0.0f64, 0.0f64, 0usize);
        for i in 0..n {
            let a = fib(i, n);
            let b = Vec3::new(a.x + offset, a.y, a.z).normalized();
            let (ma, mb) = (s.margin_activity(a), s.margin_activity(b));
            assert!((0.0..=1.0).contains(&ma), "the activity is in [0, 1]: {ma}");
            worst_activity = worst_activity.max((ma - mb).abs());
            active += usize::from(ma > 0.5);
            let (pa, pb) = (s.eval(a), s.eval(b));
            if pa.land_depth < 0.0 && pb.land_depth < 0.0 {
                worst_floor = worst_floor.max((pa.e0 - pb.e0).abs());
            }
        }
        (worst_activity, worst_floor, active)
    };
    let (act_far, floor_far, active) = steps(1e-4);
    let (act_near, floor_near, _) = steps(1e-5);
    assert!(act_far < 0.02 && act_near < 0.2 * act_far.max(1e-9) + 1e-6, "the activity is continuous: {act_far} then {act_near}");
    assert!(floor_far < 400.0, "the sea floor steps by {floor_far} m over 0.6 km");
    assert!(floor_near < 0.3 * floor_far, "the sea floor is continuous: a step of {floor_far} m over 0.6 km is {floor_near} m over 64 m");
    assert!(active > 0 && active < n as usize / 2, "some margins are active and most are not: {active} of {n}");
}

// spec: GEN-118
#[test]
fn a_point_far_from_every_margin_or_on_a_non_converging_one_is_passive_and_the_field_has_both_kinds() {
    let s = ClassSource::new(7);
    let n = 20_000;
    let acts: Vec<f64> = (0..n).map(|i| s.margin_activity(fib(i, n))).collect();
    let (zero, one) = (acts.iter().filter(|a| **a == 0.0).count(), acts.iter().filter(|a| **a > 0.99).count());
    assert!(zero > n as usize / 2, "most of the sphere is passive: {zero} of {n}");
    assert!(one > 0, "some of it is an active margin");
}

// spec: GEN-118
#[test]
fn the_floor_meets_the_land_at_every_coast_of_the_field() {
    let s = ClassSource::new(7);
    let n = 30_000;
    let (mut coasts, mut worst) = (0usize, 0.0f64);
    let mut prev = fib(0, n);
    for i in 1..n {
        let cur = fib(i, n);
        let (pa, pb) = (s.eval(prev), s.eval(cur));
        if (pa.land_depth < 0.0) != (pb.land_depth < 0.0) {
            // Bisect along the arc to the coast of the field, then compare the two sides a hair apart.
            let (mut lo, mut hi) = (prev, cur);
            let lo_is_sea = pa.land_depth < 0.0;
            for _ in 0..60 {
                let mid = Vec3::new(lo.x + hi.x, lo.y + hi.y, lo.z + hi.z).normalized();
                if (s.eval(mid).land_depth < 0.0) == lo_is_sea {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let (sea, land) = if lo_is_sea { (s.eval(lo), s.eval(hi)) } else { (s.eval(hi), s.eval(lo)) };
            coasts += 1;
            worst = worst.max((sea.e0 - land.e0).abs());
        }
        prev = cur;
    }
    println!("SHELF coasts {coasts}, worst step {worst:.2} m");
    assert!(coasts > 100, "the walk found {coasts} coasts");
    assert!(worst < 5.0, "the floor steps by {worst} m at a coast");
}

// spec: GEN-118
#[test]
fn the_near_shore_sea_is_not_an_abyss_wall() {
    // The sea within 0.01 of land depth of a coast (where the fade has not finished) must not sit in the abyss: an islet under a cell of the coast lattice would put it there.
    let s = ClassSource::new(7);
    let n = 40_000;
    let (mut near, mut deep, mut worst) = (0usize, 0usize, 0.0f64);
    for i in 0..n {
        let d = fib(i, n);
        let p = s.eval(d);
        if p.land_depth < 0.0 && p.land_depth > -0.01 {
            near += 1;
            if p.e0 < -1000.0 {
                deep += 1;
            }
            worst = worst.min(p.e0);
        }
    }
    println!("SHELF near-shore sea {near}, deeper than 1,000 m {deep}, deepest {worst:.0} m");
    assert!(near > 100, "{near} near-shore points");
    assert!(deep * 20 < near, "{deep} of {near} near-shore sea points lie deeper than 1,000 m");
}
