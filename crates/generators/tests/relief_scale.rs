//! GEN-033: the relief scale of a body (WDEF-017) multiplies the elevation the height fields read, and the fields' bounds, keys and heights
//! agree with it.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::global::{bake, GridSource};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::terrain::TerrainGenerator;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;

fn world(scale: Option<f64>) -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(R, SEED);
    if let Some(s) = scale {
        d.set_relief_scale(BodyId::PLANET, s);
    }
    d
}

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized()
}

// spec: GEN-033
#[test]
fn the_terrain_fields_base_elevation_is_the_bakes_times_the_scale_and_climate_is_not_touched() {
    let g = bake(SEED, &TangentWarp, 32, 2).unwrap();
    let src = GridSource(&g);
    let (d1, d25) = (world(None), world(Some(2.5)));
    let f1 = TerrainGenerator.field(&GenContext::new(&d1), BodyId::PLANET, &src, [1; 16]).unwrap();
    let f25 = TerrainGenerator.field(&GenContext::new(&d25), BodyId::PLANET, &src, [1; 16]).unwrap();
    let mut rng = SplitMix64(3);
    let mut checked = 0;
    for _ in 0..300 {
        let d = random_dir(&mut rng);
        let (a, b) = (f1.base_elevation(d), f25.base_elevation(d));
        assert!((b - 2.5 * a).abs() <= 5.0e-3, "at {d:?}: {a} m scaled to {b} m, not 2.5 times");
        if a.abs() > 1.0 {
            checked += 1;
            assert_eq!(a.is_sign_negative(), b.is_sign_negative(), "the scale keeps the sea level: {a} then {b}");
        }
    }
    assert!(checked > 100, "the sample had ground above and below the sea ({checked})");
}

// spec: GEN-033
#[test]
fn the_bounds_of_the_scaled_fields_hold_every_height_and_the_keys_follow_the_scale() {
    let g = bake(SEED, &TangentWarp, 32, 2).unwrap();
    let src = GridSource(&g);
    for kind in [HeightFieldKind::Terrain, HeightFieldKind::Eroded] {
        let mut keys = Vec::new();
        for scale in [None, Some(1.5), Some(4.0)] {
            let def = world(scale);
            let f = build_field(kind, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
            keys.push(f.inputs_hash());
            let global = f.max_abs_height_m();
            let mut rng = SplitMix64(11);
            for _ in 0..60 {
                let c = random_dir(&mut rng);
                let r = rng.range(1.0e-4, 0.3);
                let (lo, hi) = f.height_bounds(c, r);
                assert!(
                    lo >= -global - 1e-6 && hi <= global + 1e-6,
                    "{kind:?} at {scale:?}: bounds {lo} .. {hi} leave the global {global}"
                );
                for _ in 0..8 {
                    let p = (c + random_dir(&mut rng) * libm::tan(r * 0.5)).normalized();
                    if p.dot(c) < libm::cos(r) {
                        continue;
                    }
                    let h = f.height(p);
                    assert!(
                        h >= lo - 1e-6 && h <= hi + 1e-6,
                        "{kind:?} at {scale:?}: height {h} outside the bounds {lo} .. {hi} of its cap"
                    );
                    assert!(h.abs() <= global + 1e-6, "{kind:?} at {scale:?}: height {h} beyond the field's largest {global}");
                }
            }
            if scale == Some(4.0) {
                assert!(global > 36_000.0, "{kind:?}: the largest height follows the scale ({global})");
            }
        }
        assert!(keys[0] != keys[1] && keys[0] != keys[2] && keys[1] != keys[2], "{kind:?}: each scale has its own key: {keys:?}");
        let again = build_field(kind, &GenContext::new(&world(None)), BodyId::PLANET, &src, [1; 16]).unwrap().inputs_hash();
        assert_eq!(again, keys[0], "{kind:?}: a world that sets no scale keeps its key");
    }
}
