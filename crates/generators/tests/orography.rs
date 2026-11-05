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
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
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
            // The +s edge and the -t edge of the face: a point just inside and one just outside, which the next face holds.
            for (inside, outside) in [((1.0 - 1e-7, t), (1.0 + 1e-7, t)), ((t, -1.0 + 1e-7), (t, -1.0 - 1e-7))] {
                let a = planet_core::cube::face_to_direction(&tw, planet_core::Face(f), inside.0, inside.1);
                let b = planet_core::cube::face_to_direction(&tw, planet_core::Face(f), outside.0, outside.1);
                if planet_core::cube::direction_to_face(&tw, a).0 != planet_core::cube::direction_to_face(&tw, b).0 {
                    crossed += 1;
                }
                worst = worst.max((o.precipitation_mm(a) - o.precipitation_mm(b)).abs());
            }
        }
    }
    assert!(crossed > 300, "the pairs lie on two faces: {crossed}");
    assert!(worst < 0.5, "a jump of {worst} mm across a face edge");
}

// spec: GEN-117
#[test]
fn more_iterations_change_the_annual_precipitation_by_under_two_percent_on_average_and_five_at_most_points() {
    let (a, b) =
        (Orography::build_iterations(5, &Ridge, 192, &SEASONS, 96, 4), Orography::build_iterations(5, &Ridge, 192, &SEASONS, 192, 4));
    let (mut sum_diff, mut sum, mut changes) = (0.0, 0.0, Vec::new());
    for lat in [-60.0, -30.0, 0.0, 15.0, 30.0, 45.0, 60.0] {
        for lon in [-60.0, -30.0, -1.0, 4.0, 30.0, 90.0, 150.0] {
            let d = at(lon, lat);
            let (pa, pb) = (a.precipitation_mm(d), b.precipitation_mm(d));
            sum_diff += (pa - pb).abs();
            sum += pb;
            changes.push((pa - pb).abs() / pb.max(50.0));
        }
    }
    changes.sort_by(f64::total_cmp);
    assert!(sum_diff < 0.02 * sum, "the mean change of the annual precipitation is {:.2} %", 100.0 * sum_diff / sum);
    assert!(
        changes[changes.len() * 19 / 20] < 0.05,
        "the 95th percentile of the point changes is {:.1} %",
        100.0 * changes[changes.len() * 19 / 20]
    );
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

/// A flat world with the sea everywhere (no relief, no land): the rain is the zonal bands alone.
struct Ocean;

impl BaseElevation for Ocean {
    fn e0(&self, _d: Vec3) -> f64 {
        -3000.0
    }

    fn land_depth(&self, _d: Vec3) -> f64 {
        -1.0
    }
}

/// A flat land that begins 5 degrees north of the equator (land where the latitude is above 5 degrees), no relief: the land-sea gradient points north at the equator.
struct NorthernLand;

impl BaseElevation for NorthernLand {
    fn e0(&self, d: Vec3) -> f64 {
        if lon_lat(d).1 > 5.0 {
            0.0
        } else {
            -3000.0
        }
    }

    fn land_depth(&self, d: Vec3) -> f64 {
        lon_lat(d).1 - 5.0
    }
}

// spec: GEN-117
#[test]
fn a_flat_sea_world_rains_the_zonal_bands_and_the_equator_and_the_pole_show_no_artefact() {
    use planet_generators::global::rain_bands_mm;
    let sea = Orography::build(5, &Ocean, 96, &ANNUAL_WIND, 4);
    for lat in [-60.0, -30.0, 0.0, 15.0, 45.0, 70.0] {
        let (p, want) = (sea.precipitation_mm(at(20.0, lat)), rain_bands_mm(lat));
        // The vapour that cold air can carry limits the rain at high latitudes (about 0.7 of the bands at 60 degrees) and the convergence of the trades adds at the equator (about 1.2).
        assert!(p > 0.65 * want && p < 1.3 * want, "lat {lat}: {p:.0} mm against the bands' {want:.0} mm");
    }
    // The monsoon term changes sign at the equator smoothly: on a coast 5 degrees north, neither season rains a line at the equator.
    let coast = Orography::build(5, &NorthernLand, 96, &SEASONS, 4);
    for k in 0..2 {
        let line = coast.season_precipitation_mm(k, at(30.0, 0.0));
        let around = [-4.0, -2.0, 2.0, 4.0].iter().map(|lat| coast.season_precipitation_mm(k, at(30.0, *lat))).fold(0.0, f64::max);
        assert!(line < 1.5 * around + 50.0, "season {k}: {line:.0} mm on the equator against {around:.0} mm around it");
    }
    // The wind fades at the pole: the rain at the pole is that of 0.1 degrees away.
    for lon in [0.0, 90.0, 200.0] {
        let (pole, near) = (sea.precipitation_mm(at(lon, 90.0)), sea.precipitation_mm(at(lon, 89.9)));
        assert!((pole - near).abs() < 0.05 * near + 5.0, "lon {lon}: pole {pole:.0} mm, near it {near:.0} mm");
    }
}

const RECORDED_OROGRAPHY_VERSION: u32 = 1;
const KNOWN_PRECIPITATION: [u32; 5] = [1143160140, 1143253288, 1142953300, 1141971919, 1151450676];

// spec: GEN-117
#[test]
fn the_precipitation_is_pinned_so_that_a_change_of_the_model_must_bump_its_version() {
    assert_eq!(OROGRAPHY_VERSION, RECORDED_OROGRAPHY_VERSION, "the known answers belong to this version: re-pin both together");
    let o = Orography::build(5, &Ridge, 24, &SEASONS, 2);
    let got: Vec<u32> = [(-1.0, 45.0), (4.0, 45.0), (30.0, 15.0), (100.0, -40.0), (-20.0, 0.0)]
        .iter()
        .map(|&(lon, lat)| (o.precipitation_mm(at(lon, lat)) as f32).to_bits())
        .collect();
    assert_eq!(got, KNOWN_PRECIPITATION, "the climate changed: bump OROGRAPHY_VERSION and re-pin");
}
