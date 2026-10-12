//! GEN-001 / WDEF-004 for the global-coarse generator: it reads only through the context, its read set is exactly what it
//! declared, and it agrees with the pointwise field built from the same seed.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_generators::global::{bake, grid_hash, sample, GlobalCoarseGenerator, GLOBAL_VERSION, IMPLEMENTATION_ID};
use planet_world_def::{BodyId, GenContext, MissingNode, NodeValue, ReadRecord, WorldDefinition};

const R: f64 = 6_371_000.0;

fn paths(records: &[ReadRecord]) -> Vec<String> {
    records.iter().map(ReadRecord::path).collect()
}

// spec: GEN-013
#[test]
fn the_global_field_reads_the_body_seed_through_the_context_and_nothing_else() {
    let def = WorldDefinition::single_planet(R, 7);
    let ctx = GenContext::new(&def);
    let field = GlobalCoarseGenerator.field(&ctx, BodyId::PLANET).unwrap();
    // Declared reads: the world seed and the body's derived seed, nothing about the radius or other bodies.
    assert_eq!(paths(&field.read_records()), ["derived.body.0.seed", "world.seed"]);
    assert_eq!(field.inputs_hash(), ctx.read_set_hash());
    assert_eq!((field.implementation_id(), field.version()), (IMPLEMENTATION_ID, GLOBAL_VERSION));

    // Generation reads nothing more: sampling and baking take no context, and the context's read set stays what it was.
    let before = ctx.read_set_hash();
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    let s = field.sample(d);
    let grid = field.bake(&TangentWarp, 8, 2).unwrap();
    assert_eq!(ctx.read_set_hash(), before);
    assert_eq!(field.read_records(), ctx.read_records());

    // It is the pointwise field of the body's seed.
    let seed = field.seed();
    assert_eq!(s, sample(seed, d));
    assert_eq!(grid_hash(&grid), grid_hash(&bake(seed, &TangentWarp, 8, 1).unwrap()));
}

// spec: GEN-013
#[test]
fn a_node_it_does_not_read_changes_nothing_and_a_node_it_reads_changes_everything() {
    let def = WorldDefinition::single_planet(R, 7);
    let build = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        let f = GlobalCoarseGenerator.field(&ctx, BodyId::PLANET).unwrap();
        (f.inputs_hash(), f.seed(), grid_hash(&f.bake(&TangentWarp, 4, 1).unwrap()))
    };
    let base = build(&def);
    let mut unrelated = def.clone();
    unrelated.set_body_radius(BodyId::PLANET, 1.0);
    unrelated.set("body.3.radius_m", NodeValue::F64(5.0));
    assert_eq!(build(&unrelated), base, "the radius and other bodies are not inputs");

    let mut seeded = def.clone();
    seeded.set(WorldDefinition::SEED_PATH, NodeValue::U64(8));
    let other = build(&seeded);
    assert!(other.0 != base.0 && other.1 != base.1 && other.2 != base.2, "another world seed is another read set, seed and world");

    // Another body of the same definition has its own derived seed, so its own world.
    let mut two = def.clone();
    two.set_body_radius(BodyId(1), R);
    let ctx = GenContext::new(&two);
    let moon = GlobalCoarseGenerator.field(&ctx, BodyId(1)).unwrap();
    assert_eq!(paths(&moon.read_records()), ["derived.body.1.seed", "world.seed"]);
    assert_ne!(moon.seed(), base.1);
}

// spec: GEN-013
#[test]
fn a_missing_node_is_an_error_naming_it_not_a_default() {
    let empty = WorldDefinition::empty();
    let e = GlobalCoarseGenerator.field(&GenContext::new(&empty), BodyId::PLANET).err();
    assert!(matches!(e, Some(MissingNode(p)) if p == "world.seed"));
}
