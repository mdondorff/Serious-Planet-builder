//! GEN-018: the world names its height field by implementation id; the eroded field is the default.

use planet_core::cube::TangentWarp;
use planet_generators::erosion::EROSION_IMPLEMENTATION_ID;
use planet_generators::global::{bake, GlobalGrid, GridSource};
use planet_generators::height::{HeightField, IMPLEMENTATION_ID as REFERENCE_ID};
use planet_generators::height_fields::{build_field, known_ids, resolve, HeightFieldKind, DEFAULT_HEIGHT_FIELD};
use planet_generators::terrain::TERRAIN_IMPLEMENTATION_ID;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;

fn world(id: Option<&str>) -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(R, SEED);
    if let Some(id) = id {
        d.set_height_field(id);
    }
    d
}

fn grid() -> GlobalGrid {
    bake(SEED, &TangentWarp, 16, 2).unwrap()
}

// spec: GEN-018
#[test]
fn the_default_world_resolves_to_the_eroded_field() {
    let kind = resolve(&world(None)).unwrap();
    assert_eq!(kind, HeightFieldKind::Eroded);
    assert_eq!(kind, DEFAULT_HEIGHT_FIELD);
    assert_eq!(kind.id(), EROSION_IMPLEMENTATION_ID);
    let g = grid();
    let src = GridSource(&g);
    let def = world(None);
    let f = build_field(kind, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
    assert_eq!(f.implementation_id(), EROSION_IMPLEMENTATION_ID);
}

// spec: GEN-018
#[test]
fn a_definition_naming_the_terrain_field_resolves_to_it() {
    let def = world(Some("cpu-terrain-v1"));
    let kind = resolve(&def).unwrap();
    assert_eq!(kind, HeightFieldKind::Terrain);
    assert_eq!(TERRAIN_IMPLEMENTATION_ID, "cpu-terrain-v1");
    let g = grid();
    let src = GridSource(&g);
    let f = build_field(kind, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
    assert_eq!(f.implementation_id(), TERRAIN_IMPLEMENTATION_ID);
}

// spec: GEN-018
#[test]
fn every_known_id_resolves_to_a_field_that_reports_it() {
    let g = grid();
    let src = GridSource(&g);
    let ids = known_ids();
    assert_eq!(ids, [EROSION_IMPLEMENTATION_ID, TERRAIN_IMPLEMENTATION_ID, REFERENCE_ID], "the default comes first");
    for id in ids {
        let def = world(Some(id));
        let kind = resolve(&def).unwrap();
        assert_eq!(kind.id(), id);
        let f = build_field(kind, &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
        assert_eq!(f.implementation_id(), id);
        assert_eq!(f.seed(), SEED);
    }
}

// spec: GEN-018
#[test]
fn an_unknown_id_is_refused_with_the_list_of_known_ids() {
    let e = resolve(&world(Some("cpu-erosion-v9"))).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("cpu-erosion-v9"), "{text}");
    for id in known_ids() {
        assert!(text.contains(id), "{text} must list {id}");
    }
    assert_eq!(e.given, "cpu-erosion-v9");
}

// spec: GEN-018
#[test]
fn the_reference_field_needs_no_bake_and_matches_the_direct_construction() {
    let def = world(Some(REFERENCE_ID));
    let g = grid();
    let src = GridSource(&g);
    let f = build_field(resolve(&def).unwrap(), &GenContext::new(&def), BodyId::PLANET, &src, [1; 16]).unwrap();
    let direct = planet_generators::height::Reference(SEED);
    let d = planet_core::Vec3::new(0.3, 0.5, 0.8).normalized();
    assert_eq!(f.height(d).to_bits(), direct.height(d).to_bits());
    assert_eq!(f.inputs_hash(), direct.inputs_hash(), "keys like the seed-built field the goldens use");
}
