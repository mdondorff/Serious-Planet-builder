//! COORD-001, COORD-002, COORD-010: frames, camera-relative conversion, tile-local quantisation.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Geo, PlanetFixed, ReferenceSurface, Sphere, TileId, Vec3};

fn close(a: Vec3, b: Vec3, tol: f64) -> bool {
    a.distance(b) <= tol
}

// spec: COORD-001
#[test]
fn frame_axes_follow_the_convention() {
    let s = Sphere::EARTH;
    let r = s.radius_m;
    for (g, expect) in [
        (Geo::new(0.0, 0.0, 0.0), Vec3::new(r, 0.0, 0.0)),
        (Geo::new(0.0, 90.0, 0.0), Vec3::new(0.0, r, 0.0)),
        (Geo::new(90.0, 0.0, 0.0), Vec3::new(0.0, 0.0, r)),
        (Geo::new(0.0, 180.0, 0.0), Vec3::new(-r, 0.0, 0.0)),
        (Geo::new(-90.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -(r + 100.0))),
    ] {
        let p = s.geo_to_planet_fixed(g).0;
        assert!(close(p, expect, 1e-6), "{g:?}: got {p:?}, expected {expect:?}");
    }
}

// spec: COORD-002
#[test]
fn camera_relative_offsets_are_accurate_far_from_the_origin() {
    let s = Sphere::EARTH;
    let mut rng = SplitMix64(7);
    let mut worst = 0.0f64;
    for i in 0..2000 {
        // Camera anywhere on the planet; point within 10 km (every 20th: within 1 m).
        let cam = s.geo_to_planet_fixed(Geo::new(rng.range(-90.0, 90.0), rng.range(-180.0, 180.0), rng.range(0.0, 1000.0)));
        let reach = if i % 20 == 0 { 1.0 } else { 10_000.0 };
        let d = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized() * (reach * rng.next_f64());
        let p = PlanetFixed(cam.0 + d);
        let rel = p.camera_relative(cam).to_vec3();
        let err = rel.distance(p.0 - cam.0);
        worst = worst.max(err);
    }
    assert!(worst < 1e-3, "worst camera-relative error {worst} m, budget 1 mm");
}

// spec: COORD-002
#[test]
fn one_metre_apart_at_6371_km_keeps_millimetres() {
    let cam = PlanetFixed(Vec3::new(6_371_000.0, 0.0, 0.0));
    let p = PlanetFixed(Vec3::new(6_371_001.0, 0.25, -0.5));
    let rel = p.camera_relative(cam).to_vec3();
    assert!(rel.distance(Vec3::new(1.0, 0.25, -0.5)) < 1e-6, "{rel:?}");
    // Absolute f32 positions would lose this: spacing at planet radius is 0.5 m.
    let spacing = f64::from(f32::from_bits((6_371_000.0f32).to_bits() + 1)) - 6_371_000.0;
    assert!(spacing >= 0.5, "f32 spacing at planet radius is {spacing} m");
}

// spec: COORD-010
#[test]
fn finest_level_vertices_quantise_below_a_millimetre() {
    let map = TangentWarp;
    let mut rng = SplitMix64(11);
    let mut worst = 0.0f64;
    let r = Sphere::EARTH.radius_m;
    for _ in 0..500 {
        let dir = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized();
        let tile = TileId::from_direction(&map, dir, 28);
        let origin = PlanetFixed(tile.center_direction(&map) * r);
        // 33x33 vertices over the tile, as a finest-level patch would carry.
        for k in [0u32, 7, 16, 25, 32] {
            for l in [0u32, 9, 16, 31, 32] {
                let v = PlanetFixed(tile.sample_direction(&map, 32, k, l) * r);
                let back = v.tile_local(origin).to_planet_fixed(origin);
                worst = worst.max(back.0.distance(v.0));
            }
        }
    }
    assert!(worst <= 1e-3, "level-28 quantisation error {worst} m, budget 1 mm");
}

// spec: COORD-010
#[test]
fn coarse_tiles_need_more_than_f32_tile_local_offsets() {
    // Documents where f32 tile-local offsets stop meeting 1 mm, so later storage formats know the limit.
    let map = TangentWarp;
    let r = Sphere::EARTH.radius_m;
    let dir = Vec3::new(0.3, -0.5, 0.8).normalized();
    let mut coarsest_ok = None;
    for level in (0..=28u8).rev() {
        let tile = TileId::from_direction(&map, dir, level);
        let origin = PlanetFixed(tile.center_direction(&map) * r);
        let mut worst = 0.0f64;
        for (k, l) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
            let v = PlanetFixed(tile.sample_direction(&map, 32, k, l) * r);
            worst = worst.max(v.tile_local(origin).to_planet_fixed(origin).0.distance(v.0));
        }
        if worst > 1e-3 {
            break;
        }
        coarsest_ok = Some(level);
    }
    let level = coarsest_ok.expect("the finest level must pass");
    eprintln!("f32 tile-local offsets meet 1 mm down to level {level}");
    assert!((8..=12).contains(&level), "expected the 1 mm limit between levels 8 and 12, got {level}");
}
