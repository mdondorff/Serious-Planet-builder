//! WDEF-004 / GEN-001: contract every height generator passes: it reads only through the context, its key covers
//! exactly what it read, and it agrees with a field built directly from the same seed.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::height::{cache_key_field, HeightField, HeightGenerator, Reference, ReferenceGenerator};
use planet_generators::mesh::mesh_key_field;
use planet_generators::stress::{Steep, SteepGenerator};
use planet_world_def::{BodyId, GenContext, MissingNode, NodeValue, WorldDefinition};

const R: f64 = 6_371_000.0;

fn tile() -> TileId {
    TileId::new(Face(2), 5, 3, 7).unwrap()
}

fn build(g: &dyn HeightGenerator, def: &WorldDefinition) -> Box<dyn HeightField> {
    g.field(&GenContext::new(def), BodyId::PLANET).unwrap()
}

fn key(f: &dyn HeightField) -> u64 {
    cache_key_field(f, &TangentWarp, tile(), 16)
}

fn contract(g: &dyn HeightGenerator, direct: &dyn HeightField) {
    let def = WorldDefinition::single_planet(R, direct.seed());
    let f = build(g, &def);
    // Same definition, same field, same keys, same heights.
    let again = build(g, &def);
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert_eq!(f.height(d).to_bits(), again.height(d).to_bits());
    assert_eq!(key(f.as_ref()), key(again.as_ref()));
    // It is the field built directly from the seed, and keys like it.
    assert_eq!(f.height(d).to_bits(), direct.height(d).to_bits());
    assert_eq!(f.implementation_id(), g.implementation_id());
    assert_eq!(f.version(), g.version());
    assert_eq!(key(f.as_ref()), key(direct));
    assert_eq!(mesh_key_field(f.as_ref(), &TangentWarp, tile(), 16, R), mesh_key_field(direct, &TangentWarp, tile(), 16, R));
    // A node it does not read changes nothing: not another body, not the radius.
    let mut other = def.clone();
    other.set_body_radius(BodyId::PLANET, 1.0);
    other.set("body.3.radius_m", NodeValue::F64(5.0));
    assert_eq!(key(f.as_ref()), key(build(g, &other).as_ref()));
    // A node it reads changes the key and the field.
    let mut seeded = def.clone();
    seeded.set(WorldDefinition::SEED_PATH, NodeValue::U64(direct.seed() + 1));
    let g3 = build(g, &seeded);
    assert_ne!(key(f.as_ref()), key(g3.as_ref()));
    assert_ne!(f.height(d).to_bits(), g3.height(d).to_bits());
    // A missing node is an error naming it, not a default.
    let empty = WorldDefinition::empty();
    assert!(matches!(g.field(&GenContext::new(&empty), BodyId::PLANET), Err(MissingNode(p)) if p == "world.seed"));
}

// spec: WDEF-004
#[test]
fn the_reference_generator_meets_the_contract() {
    contract(&ReferenceGenerator, &Reference(7));
}

// spec: WDEF-004
// spec: TEST-015
#[test]
fn the_steep_generator_meets_the_contract() {
    contract(&SteepGenerator, &Steep(7));
}

// spec: WDEF-004
#[test]
fn bodies_get_other_seeds_and_so_other_keys() {
    let mut def = WorldDefinition::single_planet(R, 7);
    def.set_body_radius(BodyId(1), 1_737_400.0);
    let planet = ReferenceGenerator.field(&GenContext::new(&def), BodyId::PLANET).unwrap();
    let moon = ReferenceGenerator.field(&GenContext::new(&def), BodyId(1)).unwrap();
    assert_ne!(planet.seed(), moon.seed());
    assert_ne!(key(planet.as_ref()), key(moon.as_ref()));
}
