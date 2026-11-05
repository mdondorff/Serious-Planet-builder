//! Tier A, GEN-117 and GEN-130 (ADR 0104): the orographic climate is a pure function of the seed and a base elevation, bit-identical for any thread count; precipitation is higher on the windward
//! side of a range than on its lee; an interior far from the sea is drier than a coast; more iterations barely change it; its preview writes the same bytes whatever the thread count. GPU-free.

use planet_core::Vec3;
use planet_generators::orography::{aridity_index, BaseElevation, Orography, ANNUAL_WIND, OROGRAPHY_VERSION, SEASONS};

/// A made-up planet: the sea west of longitude -40 degrees, land east of it, a ridge of 3,000 m along longitude 0 (1.5 degrees of standard deviation, about 120 km at 45 N).
struct Ridge;

fn lon_lat(d: Vec3) -> (f64, f64) {
    (libm::atan2(d.y, d.x).to_degrees(), libm::asin(d.z).to_degrees())
}

fn at(lon: f64, lat: f64) -> Vec3 {
    let (lo, la) = (lon.to_radians(), lat.to_radians());
    Vec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin())
}

impl BaseElevation for Ridge {
    fn e0(&self, d: Vec3) -> f64 {
        let (lon, _) = lon_lat(d);
        if lon < -40.0 {
            -3000.0
        } else {
            3000.0 * libm::exp(-(lon / 1.5) * (lon / 1.5))
        }
    }

    fn land_depth(&self, d: Vec3) -> f64 {
        lon_lat(d).0 + 40.0
    }
}

const CELLS: usize = 192;

// spec: GEN-117
#[test]
fn the_field_is_bit_identical_for_any_thread_count_and_depends_on_the_seed() {
    let (one, four) = (Orography::build(5, &Ridge, 24, &SEASONS, 1), Orography::build(5, &Ridge, 24, &SEASONS, 4));
    for (lon, lat) in [(0.0, 45.0), (-20.0, 10.0), (30.0, -50.0), (170.0, 70.0)] {
        let d = at(lon, lat);
        assert_eq!(one.precipitation_mm(d).to_bits(), four.precipitation_mm(d).to_bits());
        assert_eq!(one.seasonality(d).to_bits(), four.seasonality(d).to_bits());
        assert_eq!(one.wind(0, d).x.to_bits(), four.wind(0, d).x.to_bits());
    }
    let other = Orography::build(6, &Ridge, 24, &SEASONS, 2);
    assert_ne!(
        one.wind(0, at(0.0, 45.0)).x.to_bits(),
        other.wind(0, at(0.0, 45.0)).x.to_bits(),
        "the wind's noise turn depends on the seed"
    );
    assert_eq!(OROGRAPHY_VERSION, 1, "a change of the field's output raises the version, which a consumer folds into its keys");
}

// spec: GEN-117
#[test]
fn the_windward_flank_is_wetter_than_the_lee_and_an_interior_is_drier_than_a_coast() {
    let o = Orography::build(5, &Ridge, CELLS, &SEASONS, 4);
    // 45 N: the westerlies blow from the sea in the west over the ridge.
    let (windward, lee) = (o.precipitation_mm(at(-1.0, 45.0)), o.precipitation_mm(at(4.0, 45.0)));
    assert!(windward > 2.0 * lee, "windward {windward:.0} mm against lee {lee:.0} mm");
    assert!(windward > 1000.0 && windward < 6000.0, "a plausible windward total: {windward:.0} mm");
    // Far inland (the land runs east to longitude 180 on this planet, the coast is at -40): a coast point against an interior point at the same latitude.
    let (coast, interior) = (o.precipitation_mm(at(-38.0, 45.0)), o.precipitation_mm(at(60.0, 45.0)));
    assert!(coast > 1.1 * interior, "coast {coast:.0} mm against interior {interior:.0} mm");
    // Seasonality stays in [0, 1], the aridity is P / PET.
    for (lon, lat) in [(0.0, 45.0), (-20.0, 10.0), (30.0, -50.0), (100.0, 25.0)] {
        let d = at(lon, lat);
        let s = o.seasonality(d);
        assert!((0.0..=1.0).contains(&s), "seasonality {s}");
        assert!((o.aridity(d) - aridity_index(o.precipitation_mm(d), o.temperature_c(d))).abs() < 1e-12);
    }
}

// spec: GEN-117
#[test]
fn the_annual_precipitation_is_the_sum_of_the_seasons_and_a_single_annual_wind_gives_a_comparable_total() {
    let two = Orography::build(5, &Ridge, CELLS, &SEASONS, 4);
    let one = Orography::build(5, &Ridge, CELLS, &ANNUAL_WIND, 4);
    let d = at(-1.0, 45.0);
    assert!((two.season_precipitation_mm(0, d) + two.season_precipitation_mm(1, d) - two.precipitation_mm(d)).abs() < 1e-3);
    assert_eq!(one.seasonality(d), 0.0, "one season has no seasonality");
    let (a, b) = (two.precipitation_mm(d), one.precipitation_mm(d));
    assert!(a > 0.3 * b && a < 3.0 * b, "two seasons {a:.0} mm against the annual wind {b:.0} mm");
}

// spec: GEN-117
#[test]
fn the_field_is_continuous_across_a_face_edge() {
    let o = Orography::build(5, &Ridge, 48, &SEASONS, 4);
    let tw = planet_core::cube::TangentWarp;
    let mut worst = 0.0f64;
    let mut crossed = 0;
    for f in 0..6u8 {
        for k in 0..32 {
            let t = -0.9 + 1.8 * f64::from(k) / 31.0;
            let a = planet_core::cube::face_to_direction(&tw, planet_core::Face(f), 1.0 - 1e-7, t);
            let b = planet_core::cube::face_to_direction(&tw, planet_core::Face(f), 1.0 + 1e-7, t);
            if planet_core::cube::direction_to_face(&tw, a).0 != planet_core::cube::direction_to_face(&tw, b).0 {
                crossed += 1;
            }
            worst = worst.max((o.precipitation_mm(a) - o.precipitation_mm(b)).abs());
        }
    }
    assert!(crossed > 150, "the pairs lie on two faces: {crossed}");
    assert!(worst < 0.5, "a jump of {worst} mm across a face edge");
}

// spec: GEN-117
#[test]
fn more_iterations_change_the_annual_precipitation_by_under_two_percent() {
    let (a, b) =
        (Orography::build_iterations(5, &Ridge, 96, &SEASONS, 96, 4), Orography::build_iterations(5, &Ridge, 96, &SEASONS, 192, 4));
    let (mut sum_diff, mut sum) = (0.0, 0.0);
    for lat in [-60.0, -30.0, 0.0, 15.0, 30.0, 45.0, 60.0] {
        for lon in [-60.0, -30.0, -1.0, 4.0, 30.0, 90.0, 150.0] {
            let d = at(lon, lat);
            sum_diff += (a.precipitation_mm(d) - b.precipitation_mm(d)).abs();
            sum += b.precipitation_mm(d);
        }
    }
    assert!(sum_diff < 0.02 * sum, "the mean change of the annual precipitation is {:.2} %", 100.0 * sum_diff / sum);
}

// spec: GEN-130
#[test]
fn the_preview_is_the_same_bytes_for_any_thread_count_and_labels_its_images_as_inputs_with_the_numbers() {
    use planet_generators::classes::ClassSource;
    use planet_generators::lithology::ProvinceLattice;
    use planet_generators::orography_preview::{preview, write};
    let source = ClassSource::new(5);
    let provinces = ProvinceLattice::build_with(&source, 32, 2);
    let (a, b) = (
        preview(&source, 5, &provinces, &Orography::build(5, &source, 32, &SEASONS, 1), 800.0, None, 1),
        preview(&source, 5, &provinces, &Orography::build(5, &source, 32, &SEASONS, 4), 800.0, None, 4),
    );
    assert_eq!(a.text, b.text);
    assert_eq!(a.images.len(), b.images.len());
    for ((na, ia), (nb, ib)) in a.images.iter().zip(&b.images) {
        assert_eq!((na, ia), (nb, ib), "{na}");
    }
    for needle in [
        "orographic climate (input), not terrain",
        "land by aridity class",
        "land precipitation by latitude band",
        "windward over lee ratio at 32 cells",
        "regions (",
    ] {
        assert!(a.text.contains(needle), "{needle}: {}", a.text);
    }
    let dir = std::env::temp_dir().join(format!("planet-climate-preview-{}", std::process::id()));
    write(&dir, &a).unwrap();
    for (name, _) in &a.images {
        assert!(dir.join(name).is_file(), "{name}");
    }
    assert!(dir.join("orography.txt").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
