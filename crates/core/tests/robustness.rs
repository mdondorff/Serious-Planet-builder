//! Float known answers (libm bits), tile-index correctness against bounds, and misuse panics.
//! Review fixes for the numerics-reviewer and verifier reports on m1-coordinates.

use planet_core::cube::{direction_to_face, face_to_direction, FaceMapping, TangentWarp};
use planet_core::hash::SplitMix64;
use planet_core::{Face, Geo, PlanetFixed, ReferenceSurface, Sphere, TileId, Vec3};

fn bits(v: Vec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

// spec: COORD-008, COORD-005
#[test]
fn float_known_answers_pin_libm_results_bit_for_bit() {
    let m = TangentWarp;
    assert_eq!(bits(face_to_direction(&m, Face(2), 0.3, -0.7)), [0xbfc9ab5ec866861c, 0x3feabaee86b6764f, 0xbfe0615a9d78cfdc]);
    assert_eq!(
        bits(Sphere::EARTH.geo_to_planet_fixed(Geo::new(37.5, -122.25, 123.0)).0),
        [0xc14493ef15372db6, 0xc1504e95c0a8f18b, 0x414d972ef88f34f7]
    );
    assert_eq!((m.plane_to_face(0.3).to_bits(), m.face_to_plane(0.3).to_bits()), (0x3fd7c00260125fbb, 0x3fcebae6995b4f6d));
    let g = Sphere::EARTH.planet_fixed_to_geo(PlanetFixed(Vec3::new(1.0e6, 2.0e6, 3.0e6)));
    assert_eq!(
        [g.lat_deg.to_bits(), g.lon_deg.to_bits(), g.height_m.to_bits()],
        [0x404aa67fc9e3adb0, 0x404fb7ac672cf11e, 0xc1440f6f4e7e3105]
    );
}

// spec: COORD-006
#[test]
fn containing_tile_agrees_with_its_bounds_and_centre() {
    let m = TangentWarp;
    let mut rng = SplitMix64(5);
    for level in [0u8, 1, 2, 5, 12, 20, 28] {
        for _ in 0..3000 {
            let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
            let t = TileId::from_direction(&m, d, level);
            let (face, s, tt) = direction_to_face(&m, d);
            let (s0, s1, t0, t1) = t.bounds();
            assert_eq!(t.face(), face);
            assert!(s >= s0 && (s < s1 || s1 == 1.0), "s={s} outside [{s0}, {s1}) at level {level}");
            assert!(tt >= t0 && (tt < t1 || t1 == 1.0), "t={tt} outside [{t0}, {t1}) at level {level}");
            assert_eq!(TileId::from_direction(&m, t.center_direction(&m), level), t, "centre of {t:?}");
        }
    }
    // Tiny negative coordinates belong to the tile below zero, not the one above (exact indexing).
    let d = face_to_direction(&m, Face(0), -1e-17, 0.3);
    assert_eq!(TileId::from_direction(&m, d, 1).x(), 0);
}

// spec: COORD-007
#[test]
fn border_directions_compare_by_bits_at_deep_levels() {
    let m = TangentWarp;
    let mut rng = SplitMix64(9);
    for _ in 0..300 {
        let level = 20 + (rng.next_u64() % 9) as u8;
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        // Slide to a face edge so the tile has a cross-face neighbour.
        let (face, _, _) = direction_to_face(&m, d);
        let n = 1u32 << level;
        let t = TileId::new(face, level, n - 1, (rng.next_u64() % u64::from(n)) as u32).unwrap();
        let nb = t.neighbor(&m, planet_core::Side::East);
        let res = 32;
        for k in 0..=res {
            let mine = bits(t.sample_direction(&m, res, res, k));
            let found = (0..=res).any(|j| {
                [
                    nb.sample_direction(&m, res, 0, j),
                    nb.sample_direction(&m, res, res, j),
                    nb.sample_direction(&m, res, j, 0),
                    nb.sample_direction(&m, res, j, res),
                ]
                .iter()
                .any(|c| bits(*c) == mine)
            });
            assert!(found, "{t:?} sample {k} has no bit-identical partner in {nb:?}");
        }
    }
}

// spec: COORD-005
#[test]
fn edge_ties_follow_the_documented_rule() {
    let m = TangentWarp;
    assert_eq!(direction_to_face(&m, Vec3::new(1.0, 1.0, 0.0).normalized()).0, Face(0), "X wins ties over Y");
    assert_eq!(direction_to_face(&m, Vec3::new(-1.0, 1.0, 0.0).normalized()).0, Face(1), "negative X face");
    assert_eq!(direction_to_face(&m, Vec3::new(0.0, 1.0, 1.0).normalized()).0, Face(2), "Y wins ties over Z");
    assert_eq!(direction_to_face(&m, Vec3::new(0.0, -1.0, -1.0).normalized()).0, Face(3));
    assert_eq!(direction_to_face(&m, Vec3::new(1.0, 1.0, 1.0).normalized()).0, Face(0));
}

// spec: COORD-004
#[test]
fn sphere_normal_is_perpendicular_to_the_surface() {
    let s = Sphere::EARTH;
    let g = Geo::new(40.0, 20.0, 500.0);
    let p = s.geo_to_planet_fixed(g);
    let n = s.normal(p);
    let east = s.geo_to_planet_fixed(Geo::new(40.0, 20.0001, 500.0)).0 - p.0;
    let north = s.geo_to_planet_fixed(Geo::new(40.0001, 20.0, 500.0)).0 - p.0;
    assert!(n.dot(east).abs() < 1e-3 * east.length() && n.dot(north).abs() < 1e-3 * north.length());
}

// spec: COORD-003
#[test]
fn round_trip_error_stays_near_the_measured_nanometre_level() {
    let s = Sphere::EARTH;
    let mut rng = SplitMix64(13);
    let mut worst = 0.0f64;
    for _ in 0..20_000 {
        let p = s.geo_to_planet_fixed(Geo::new(rng.range(-90.0, 90.0), rng.range(-180.0, 180.0), rng.range(-1e4, 1e5)));
        worst = worst.max(p.0.distance(s.geo_to_planet_fixed(s.planet_fixed_to_geo(p)).0));
    }
    assert!(worst < 1e-6, "regression: round-trip error {worst} m (measured about 1e-8 m)");
}

// spec: COORD-006
#[test]
#[should_panic(expected = "exceeds the maximum")]
fn from_direction_rejects_levels_above_the_maximum() {
    TileId::from_direction(&TangentWarp, Vec3::new(1.0, 0.0, 0.0), 29);
}

// spec: COORD-006
#[test]
#[should_panic(expected = "zero or not finite")]
fn from_direction_rejects_the_zero_direction() {
    TileId::from_direction(&TangentWarp, Vec3::ZERO, 5);
}

// spec: COORD-007
#[test]
#[should_panic(expected = "stay exact")]
fn sample_grid_too_fine_for_exact_lattice_indices_is_rejected() {
    TileId::new(Face(0), 28, 0, 0).unwrap().sample_direction(&TangentWarp, 1 << 26, 0, 0);
}

// spec: COORD-008
#[test]
fn range_never_returns_its_upper_bound() {
    let mut rng = SplitMix64(0);
    for _ in 0..10_000 {
        let v = rng.range(3.647254699579916, 3.846430900724949);
        assert!((3.647254699579916..3.846430900724949).contains(&v));
    }
}
