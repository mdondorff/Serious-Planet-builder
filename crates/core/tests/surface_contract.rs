//! COORD-003, COORD-004: the `ReferenceSurface` contract suite, run against every implementation.

use planet_core::hash::SplitMix64;
use planet_core::{Geo, PlanetFixed, ReferenceSurface, Sphere, Vec3};

/// Every implementation must pass this.
fn contract(surface: &dyn ReferenceSurface, tol_m: f64, heights: (f64, f64)) {
    let mut rng = SplitMix64(0xC0FFEE);
    let mut cases: Vec<Geo> = vec![
        Geo::new(90.0, 0.0, 0.0),
        Geo::new(-90.0, 123.0, 0.0),
        Geo::new(0.0, 180.0, 0.0),
        Geo::new(0.0, -180.0, 0.0),
        Geo::new(89.999_999_9, 45.0, 10.0),
        Geo::new(0.0, 0.0, -500.0),
        Geo::new(35.0, 179.999_999_9, 8_848.0),
    ];
    // The eight cube corners (directions (+-1, +-1, +-1)).
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let g = surface.planet_fixed_to_geo(PlanetFixed(Vec3::new(sx, sy, sz).normalized() * 1.0e6));
                cases.push(Geo::new(g.lat_deg, g.lon_deg, 1_234.5));
            }
        }
    }
    for _ in 0..20_000 {
        // Plain uniform latitude covers the poles more densely than sine-uniform, which is the harder case.
        cases.push(Geo::new(rng.range(-90.0, 90.0), rng.range(-180.0, 180.0), rng.range(heights.0, heights.1)));
    }
    let mut worst = 0.0f64;
    for g in cases {
        let p = surface.geo_to_planet_fixed(g);
        let g2 = surface.planet_fixed_to_geo(p);
        let p2 = surface.geo_to_planet_fixed(g2);
        let err = p.0.distance(p2.0);
        worst = worst.max(err);
        assert!(err <= tol_m, "{g:?} -> {p:?} -> {g2:?}: position error {err} m exceeds {tol_m} m");
        // Height is monotone and the normal is a unit vector pointing away from the centre.
        let up = surface.geo_to_planet_fixed(Geo::new(g.lat_deg, g.lon_deg, g.height_m + 10.0));
        assert!(up.0.length() > p.0.length());
        let n = surface.normal(p);
        assert!((n.length() - 1.0).abs() < 1e-12 && n.dot(p.0) > 0.0);
    }
    eprintln!("{}: worst round-trip error {worst:e} m", surface.id());
}

// spec: COORD-003, COORD-004
#[test]
fn sphere_satisfies_the_contract_with_millimetre_round_trips() {
    contract(&Sphere::EARTH, 1e-3, (-10_000.0, 100_000.0));
    contract(&Sphere { radius_m: 3_389_500.0 }, 1e-3, (-10_000.0, 100_000.0));
    contract(&Sphere { radius_m: 1_000.0 }, 1e-3, (-500.0, 5_000.0));
}

// spec: COORD-004
#[test]
fn surface_id_names_the_parameters() {
    assert_eq!(Sphere::EARTH.id(), "sphere:6371000");
}
