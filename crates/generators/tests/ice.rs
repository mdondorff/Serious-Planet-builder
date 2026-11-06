//! GEN-134: the ice history.

use planet_core::Vec3;
use planet_generators::classes::ClassSource;
use planet_generators::ice::{IceHistory, IceParams};
use planet_generators::orography::{Orography, SEASONS};

fn fib(i: u32, n: u32) -> Vec3 {
    let (lon, z) = (f64::from(i) * 2.399_963, 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n));
    let r = (1.0 - z * z).sqrt();
    Vec3::new(r * libm::cos(lon), r * libm::sin(lon), z)
}

// spec: GEN-134
#[test]
fn the_needed_temperature_rises_with_the_rain_and_is_held_to_its_range() {
    let p = IceParams::earth();
    assert_eq!(p.ela_temp_c(p.ela_dry_precip_mm), p.ela_dry_temp_c, "the dry value at the dry rain");
    assert!((p.ela_temp_c(3000.0) - 0.0).abs() < 1e-9, "a maritime line near 0 degrees at 3,000 mm: {}", p.ela_temp_c(3000.0));
    assert_eq!(p.ela_temp_c(1e9), p.ela_temp_range_c.1, "held to the upper bound");
    assert_eq!(p.ela_temp_c(0.0), p.ela_temp_range_c.0, "no rain: the lower bound");
    let mut prev = f64::NEG_INFINITY;
    for i in 0..400 {
        let t = p.ela_temp_c(f64::from(i) * 25.0);
        assert!(t >= prev, "monotone in the rain");
        prev = t;
    }
}

// spec: GEN-134
#[test]
fn the_line_stands_high_in_the_tropics_and_under_the_ground_where_the_sea_level_is_cold() {
    let p = IceParams::earth();
    let tropics = p.ela_m(30.0, 6.5, 3000.0);
    assert!((tropics - 24.0 / 6.5 * 1000.0).abs() < 1e-6, "the tropical line: {tropics}");
    assert!(p.ela_m(-20.0, 6.5, 500.0) < 0.0, "a cold sea level puts the line under sea level: a sheet");
    assert!(p.ela_m(10.0, 6.5, 1000.0) < p.ela_m(20.0, 6.5, 1000.0), "a colder sea level lowers the line");
    assert!(
        p.ela_m(10.0, 6.5, 2000.0) < p.ela_m(10.0, 6.5, 400.0),
        "more rain raises the needed temperature, which lowers the line (it needs less cold)"
    );
}

// spec: GEN-134
#[test]
fn the_cover_is_zero_below_the_line_and_at_sea_one_above_it_and_continuous() {
    let p = IceParams::earth();
    assert_eq!(p.cover(1000.0, 1500.0), 0.0);
    assert_eq!(p.cover(1500.0, 1500.0), 0.0);
    assert_eq!(p.cover(1500.0 + p.cover_ramp_m, 1500.0), 1.0);
    assert_eq!(p.cover(-100.0, -5000.0), 0.0, "no ice cover on the sea");
    assert_eq!(p.cover(0.0, -5000.0), 0.0, "nor at the coast");
    assert_eq!(p.cover(p.land_ramp_m, -5000.0), 1.0, "a sheet over land");
    let mut prev = 0.0;
    for i in 0..=400 {
        let c = p.cover(f64::from(i) * 5.0, 800.0);
        assert!((0.0..=1.0).contains(&c) && c >= prev && c - prev < 0.05, "monotone and continuous");
        prev = c;
    }
}

// spec: GEN-134
#[test]
fn the_parameters_are_validated() {
    for bad in [
        IceParams { lgm_cooling_c: f64::NAN, ..IceParams::earth() },
        IceParams { lgm_cooling_c: -1.0, ..IceParams::earth() },
        IceParams { ela_dry_precip_mm: 0.0, ..IceParams::earth() },
        IceParams { cover_ramp_m: 0.0, ..IceParams::earth() },
        IceParams { land_ramp_m: -5.0, ..IceParams::earth() },
        IceParams { ela_temp_range_c: (2.0, -12.0), ..IceParams::earth() },
        IceParams { ela_temp_per_decade_c: f64::INFINITY, ..IceParams::earth() },
        IceParams { ela_temp_per_decade_c: -1.0, ..IceParams::earth() },
    ] {
        assert!(std::panic::catch_unwind(|| IceHistory::new(bad)).is_err(), "{bad:?} is refused");
    }
    assert_eq!(IceHistory::new(IceParams::earth()).params, IceParams::earth());
}

// spec: GEN-134
#[test]
fn the_field_over_a_climate_is_polar_and_alpine_the_same_bits_on_any_thread_count() {
    let source = ClassSource::new(1);
    let (one, three) = (Orography::build(1, &source, 48, &SEASONS, 1), Orography::build(1, &source, 48, &SEASONS, 3));
    let h = IceHistory::earth();
    let n = 6000;
    let (mut land, mut ice, mut low_ela, mut low_n, mut high_ela, mut high_n) = (0usize, 0usize, 0.0, 0usize, 0.0, 0usize);
    for i in 0..n {
        let d = fib(i, n);
        let (a, b) = (h.at(&one, d), h.at(&three, d));
        assert_eq!((a.ela_m.to_bits(), a.cover.to_bits()), (b.ela_m.to_bits(), b.cover.to_bits()), "the same bits at 1 and 3 threads");
        assert!((0.0..=1.0).contains(&a.cover) && a.ela_m.is_finite());
        if a.ground_m > 0.0 {
            land += 1;
            ice += usize::from(a.cover >= 0.5);
        }
        if d.z.abs() > 0.85 {
            low_ela += a.ela_m;
            low_n += 1;
        } else if d.z.abs() < 0.3 {
            high_ela += a.ela_m;
            high_n += 1;
        }
    }
    assert!(low_ela / (low_n as f64) < high_ela / (high_n as f64) - 1000.0, "the line falls towards the poles");
    let share = ice as f64 / land.max(1) as f64;
    assert!(share > 0.03 && share < 0.6, "the land under ice is of the glacial maximum's order: {share}");
}
