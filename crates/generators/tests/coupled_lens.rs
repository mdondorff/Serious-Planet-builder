//! GEN-043: the coupled run seen through the climate views (the viewer's debug lens).

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::chain::CoupledLensField;
use planet_generators::climate::{CLIMATE_EDGE, CLIMATE_TEXELS};
use planet_generators::coupled::{lens_climate, lens_version_word, run, CoupledInput, CoupledLens, CoupledMaps, CoupledParams, Lattice};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::mesh_key_field;

const R: f64 = 6_371_000.0;
const N: usize = 32;

fn tile(face: u8, level: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(face), level, x, y).unwrap()
}

fn maps(id: u64) -> Arc<CoupledMaps> {
    let lat = Lattice::new(N, R);
    let z: Vec<f32> = (0..lat.len())
        .map(|c| {
            let d = lat.direction(c);
            (2500.0 * (d.x * d.y + 0.5 * d.z * d.x) + 300.0) as f32
        })
        .collect();
    let input = CoupledInput { uplift: vec![1.0e-3; z.len()], erodibility: vec![2.0e-5; z.len()], z };
    let state = run(&lat, &input, &CoupledParams::default(), 8);
    Arc::new(CoupledMaps::new(lat, state.z, state.area_m2, id))
}

fn temperatures(t: &planet_generators::climate::ClimateTexture) -> Vec<f32> {
    (0..CLIMATE_TEXELS).map(|k| t.texel(k % CLIMATE_EDGE, k / CLIMATE_EDGE).0).collect()
}

fn precipitations(t: &planet_generators::climate::ClimateTexture) -> Vec<f32> {
    (0..CLIMATE_TEXELS).map(|k| t.texel(k % CLIMATE_EDGE, k / CLIMATE_EDGE).1).collect()
}

// spec: GEN-043
#[test]
fn the_height_lens_puts_sea_at_the_cold_end_and_land_by_height() {
    let m = maps(1);
    let mut sea = 0;
    let mut land = Vec::new();
    for face in 0..6u8 {
        let t = lens_climate(&m, CoupledLens::Height, &TangentWarp, tile(face, 0, 0, 0));
        assert!(precipitations(&t).iter().all(|&p| p == 0.0), "the lens leaves the other channel flat");
        for v in temperatures(&t) {
            assert!((-30.0..=40.0).contains(&v), "{v} is inside the temperature view's range");
            if v == -30.0 {
                sea += 1;
            } else {
                land.push(v);
            }
        }
    }
    assert!(sea > 0 && !land.is_empty(), "the world has sea and land");
    assert!(land.iter().any(|&v| v > -20.0), "land is lifted off the cold end by its height");
}

// spec: GEN-043
#[test]
fn the_area_lens_shows_a_river_a_coarse_tile_cannot_resolve() {
    let m = maps(1);
    // A whole face in one tile: a texel spans several cells, so its largest area, not the area under its centre, is shown.
    let coarse = precipitations(&lens_climate(&m, CoupledLens::Area, &TangentWarp, tile(0, 0, 0, 0)));
    assert!(coarse.iter().all(|&p| (0.0..=3000.0).contains(&p)));
    assert!(coarse.iter().filter(|&&p| p > 0.0).count() > 40, "most texels cover land and so a drainage above one cell's");
    let max = coarse.iter().cloned().fold(0.0f32, f32::max);
    assert!(max > 1500.0, "a texel on the main stem is on the wet half of the scale, got {max}");
    // The same lens at a fine level reads cells nearly one to one: the same river is a narrow set of texels there.
    let fine = precipitations(&lens_climate(&m, CoupledLens::Area, &TangentWarp, tile(0, 4, 5, 5)));
    assert!(fine.iter().any(|&p| p < max), "{fine:?}");
}

// spec: GEN-043
#[test]
fn the_lens_is_a_pure_function_of_the_run_the_lens_and_the_tile() {
    let (a, b) = (maps(1), maps(1));
    let t = tile(2, 1, 1, 0);
    for lens in [CoupledLens::Height, CoupledLens::Area] {
        assert_eq!(lens_climate(&a, lens, &TangentWarp, t), lens_climate(&b, lens, &TangentWarp, t));
    }
    assert_ne!(lens_version_word(&a, CoupledLens::Height), lens_version_word(&a, CoupledLens::Area));
    assert_ne!(lens_version_word(&a, CoupledLens::Height), lens_version_word(&maps(2), CoupledLens::Height));
}

// spec: GEN-043
#[test]
fn the_lens_field_changes_only_the_climate_and_the_mesh_key() {
    let m = maps(7);
    let t = tile(1, 2, 1, 3);
    let plain = Reference(5);
    let lensed = CoupledLensField::new(Box::new(Reference(5)), m.clone(), CoupledLens::Area);
    let dirs: Vec<Vec3> = (0..9).map(|k| t.sample_direction(&TangentWarp, 8, k, 3)).collect();
    let bits = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<_>>();
    assert_eq!(bits(plain.heights_until(&dirs, &|| false)), bits(lensed.heights_until(&dirs, &|| false)), "heights are the base's");
    let (h, c) = lensed.heights_and_climate(&TangentWarp, t, &dirs, &|| false);
    assert_eq!(bits(h), bits(plain.heights_until(&dirs, &|| false)));
    assert_eq!(c, lens_climate(&m, CoupledLens::Area, &TangentWarp, t));
    assert_ne!(c, plain.climate(&TangentWarp, t));
    assert_eq!(lensed.inputs_hash(), plain.inputs_hash());
    assert_ne!(
        mesh_key_field(&lensed, &TangentWarp, t, 8, R),
        mesh_key_field(&plain, &TangentWarp, t, 8, R),
        "a tile drawn through the lens has its own mesh key, so switching the lens refreshes it"
    );
    let other = CoupledLensField::new(Box::new(Reference(5)), m, CoupledLens::Height);
    assert_ne!(mesh_key_field(&lensed, &TangentWarp, t, 8, R), mesh_key_field(&other, &TangentWarp, t, 8, R));
}

// spec: GEN-043
#[test]
fn a_stamp_stage_that_passes_a_tile_through_does_not_hide_the_lens_from_the_mesh_key() {
    use planet_generators::chain::with_stamps;
    use planet_world_def::stamp::{Stamp, StampKind};
    use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};
    let mut def = WorldDefinition::single_planet(R, 1);
    let at = Vec3::new(0.0, 0.0, 1.0);
    let item = ItemSpec {
        id: 1,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: at,
        content_hash: 0,
        reach_m: 800.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
    def.put_item_spec(BodyId::PLANET, item).unwrap();
    let def = Arc::new(def);
    let base = || with_stamps(Box::new(Reference(5)), &def).unwrap();
    let t = tile(1, 3, 2, 5);
    assert!(base().passes_through(t, &TangentWarp), "the stamp is far from the tile: the stage is the identity there");
    let m = maps(3);
    let plain = base();
    let height = CoupledLensField::new(base(), m.clone(), CoupledLens::Height);
    let area = CoupledLensField::new(base(), m, CoupledLens::Area);
    let key = |f: &dyn HeightField| mesh_key_field(f, &TangentWarp, t, 8, R);
    assert_ne!(key(plain.as_ref()), key(&height));
    assert_ne!(key(&height), key(&area));
}
