//! STRM-027: the keys of an authored stage chain the stage below; the stamp stage over real fields.

use std::sync::Arc;

use planet_cache::key::*;
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::StampStage;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn hill(id: u128, layer: &str, at: Vec3, reach_m: f64) -> ItemSpec {
    ItemSpec { id, layer: layer.into(), priority: 0, dir: at, content_hash: 0, reach_m, max_delta_m: 40.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [20.0, 0.0, 0.0]))
}

fn world(items: &[ItemSpec]) -> Arc<WorldDefinition> {
    let mut d = WorldDefinition::single_planet(R, 7);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i.clone()).unwrap();
    }
    Arc::new(d)
}

fn stage_over(base: Arc<dyn HeightField>, def: &Arc<WorldDefinition>, layer: &str, index: u32, version: u32) -> StampStage<'static> {
    StampStage::new(base, def.clone(), BodyId::PLANET, layer, index, version).unwrap()
}

fn keys(f: &dyn HeightField, tile: TileId) -> (CacheKey, CacheKey) {
    (height_key(f, &TangentWarp, tile, 32, R), mesh_key(f, &TangentWarp, tile, 32, R))
}

fn near_tile(at: Vec3) -> TileId {
    TileId::from_direction(&TangentWarp, at, 12)
}

// spec: STRM-027
#[test]
fn a_stage_tile_key_chains_the_base_and_the_stage_version_the_layer_and_the_index_and_ignores_a_far_stamp() {
    let at = dir(1.0, 0.2, 0.1);
    let tile = near_tile(at);
    let def = world(&[hill(1, DEFAULT_ITEM_LAYER, at, 3_000.0)]);
    let base: Arc<dyn HeightField> = Arc::new(Reference(7));
    let reference = keys(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1), tile);
    assert_eq!(reference, keys(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1), tile), "deterministic");
    let changed = |what: &str, k: (CacheKey, CacheKey)| {
        assert_ne!(k.0, reference.0, "{what}: the height key follows");
        assert_ne!(k.1, reference.1, "{what}: the mesh key follows");
    };
    changed("another base seed", keys(&stage_over(Arc::new(Reference(8)), &def, DEFAULT_ITEM_LAYER, 0, 1), tile));
    changed("another stage version", keys(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 2), tile));
    changed("another stack index", keys(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 1, 1), tile));
    let other_layer = world(&[hill(1, "other", at, 3_000.0)]);
    changed("another layer name", keys(&stage_over(base.clone(), &other_layer, "other", 0, 1), tile));
    // A stamp on the other side of the planet changes no key of this tile.
    let with_far = world(&[hill(1, DEFAULT_ITEM_LAYER, at, 3_000.0), hill(2, DEFAULT_ITEM_LAYER, dir(-1.0, -0.3, -0.2), 3_000.0)]);
    assert_eq!(keys(&stage_over(base.clone(), &with_far, DEFAULT_ITEM_LAYER, 0, 1), tile), reference);
    // The stage's key is not the base's: the chain is a source, not a copy.
    assert_ne!(height_key(&Reference(7), &TangentWarp, tile, 32, R), reference.0);
    // The source of the stage's height key is the base's height key of the same tile at the same resolution.
    let other_res = height_key(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1), &TangentWarp, tile, 16, R);
    assert_ne!(other_res, reference.0, "another resolution is another key");
    // The source is the base's key of the same tile at the same resolution: a base seen at another resolution re-keys the stage too.
    let stage_16 = stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1);
    assert_eq!(height_key(&stage_16, &TangentWarp, tile, 16, R), other_res, "deterministic at the other resolution");
}

// spec: STRM-027
#[test]
fn an_edit_re_keys_the_tiles_the_footprint_finds_and_restoring_it_restores_every_key() {
    let at = dir(1.0, 0.2, 0.1);
    let base: Arc<dyn HeightField> = Arc::new(Reference(7));
    let tiles = [near_tile(at), near_tile(dir(1.0, 0.2 + 30_000.0 / R, 0.1)), near_tile(dir(-1.0, 0.3, 0.2))];
    let all = |def: &Arc<WorldDefinition>| tiles.map(|t| keys(&stage_over(base.clone(), def, DEFAULT_ITEM_LAYER, 0, 1), t));
    let plain = world(&[]);
    let edited = world(&[hill(1, DEFAULT_ITEM_LAYER, at, 600.0)]);
    let (before, after) = (all(&plain), all(&edited));
    assert_ne!(before[0], after[0], "the tile under the stamp is re-keyed");
    assert_eq!(before[1], after[1], "a tile 30 km away, out of reach, is not");
    assert_eq!(before[2], after[2], "nor one on the other side");
    assert_eq!(all(&world(&[])), before, "restoring the definition restores every key");
    // Edited (another reach) and moved (another place): the stamp's tile follows, the far tiles never do.
    let widened = all(&world(&[hill(1, DEFAULT_ITEM_LAYER, at, 900.0)]));
    assert_ne!(widened[0], after[0], "an edited reach re-keys the tile under it");
    assert_eq!((widened[1], widened[2]), (before[1], before[2]));
    let moved = all(&world(&[hill(1, DEFAULT_ITEM_LAYER, dir(-1.0, 0.3, 0.2), 600.0)]));
    assert_eq!(moved[0], before[0], "moved away, the first tile is as without it");
    assert_ne!(moved[2], before[2], "moved here, the third tile is re-keyed");
    assert_eq!(moved[1], before[1]);
}

// spec: STRM-027
#[test]
fn in_a_two_layer_stack_a_base_change_re_keys_both_layers_and_an_edit_below_re_keys_the_layer_above_at_that_place_only() {
    let at = dir(1.0, 0.2, 0.1);
    let tile = near_tile(at);
    let elsewhere = near_tile(dir(-1.0, 0.3, 0.2));
    let two = |seed: u64, lower: &[ItemSpec]| {
        let mut items = lower.to_vec();
        items.push(hill(100, "upper", at, 1_000.0));
        let def = world(&items);
        let base: Arc<dyn HeightField> = Arc::new(Reference(seed));
        let lower_stage = Arc::new(stage_over(base, &def, DEFAULT_ITEM_LAYER, 0, 1));
        let upper = StampStage::new(lower_stage.clone(), def.clone(), BodyId::PLANET, "upper", 1, 1).unwrap();
        let k = |f: &dyn HeightField, t: TileId| keys(f, t);
        ((k(&*lower_stage, tile), k(&upper, tile)), (k(&*lower_stage, elsewhere), k(&upper, elsewhere)))
    };
    let reference = two(7, &[hill(1, DEFAULT_ITEM_LAYER, at, 800.0)]);
    let other_base = two(8, &[hill(1, DEFAULT_ITEM_LAYER, at, 800.0)]);
    assert_ne!(reference.0 .0, other_base.0 .0, "a base change re-keys the lower layer");
    assert_ne!(reference.0 .1, other_base.0 .1, "and the upper layer");
    // An edit in the lower layer at this place: the lower layer's tile and the upper layer's tile over it change; nothing elsewhere.
    let edited = two(7, &[hill(1, DEFAULT_ITEM_LAYER, at, 800.0), hill(2, DEFAULT_ITEM_LAYER, at, 500.0)]);
    assert_ne!(reference.0 .0, edited.0 .0, "the lower layer's tile");
    assert_ne!(reference.0 .1, edited.0 .1, "the upper layer's tile over it chains the lower layer's key");
    assert_eq!(reference.1, edited.1, "tiles elsewhere keep every key in both layers");
}

// spec: STRM-027
#[test]
fn the_in_memory_key_of_a_stage_follows_its_index_its_version_and_its_base_like_the_persistent_one() {
    use planet_generators::height::cache_key_field;
    let at = dir(1.0, 0.2, 0.1);
    let tile = near_tile(at);
    let def = world(&[hill(1, DEFAULT_ITEM_LAYER, at, 3_000.0)]);
    let base: Arc<dyn HeightField> = Arc::new(Reference(7));
    let key = |f: &dyn HeightField| cache_key_field(f, &TangentWarp, tile, 32);
    let reference = key(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1));
    assert_eq!(reference, key(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 1)));
    assert_ne!(reference, key(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 1, 1)), "the stack index");
    assert_ne!(reference, key(&stage_over(base.clone(), &def, DEFAULT_ITEM_LAYER, 0, 2)), "the stage version");
    assert_ne!(reference, key(&stage_over(Arc::new(Reference(8)), &def, DEFAULT_ITEM_LAYER, 0, 1)), "the base");
}

// spec: GEN-026
#[test]
fn a_stage_over_a_shared_definition_reads_the_bound_of_the_moment() {
    use std::sync::RwLock;
    let at = dir(1.0, 0.2, 0.1);
    let shared = Arc::new(RwLock::new(WorldDefinition::single_planet(R, 7)));
    let base: Arc<dyn HeightField> = Arc::new(Reference(7));
    let stage = StampStage::new(base.clone(), shared.clone(), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).unwrap();
    let empty_bound = stage.max_abs_height_m();
    assert_eq!(empty_bound, base.max_abs_height_m());
    shared.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, DEFAULT_ITEM_LAYER, at, 3_000.0)).unwrap();
    assert!(stage.max_abs_height_m() >= empty_bound + 40.0, "the bound grows with the item added through the lock");
}

// spec: STRM-027
#[test]
fn a_key_without_a_stage_is_what_it_was_and_every_wrapper_that_forwards_the_source_key_forwards_the_chain() {
    let tile = TileId::new(Face(2), 5, 3, 7).unwrap();
    assert_eq!(
        height_key(&Reference(7), &TangentWarp, tile, 32, R).to_hex(),
        "340480159ae2ca791db08d1cae1232ce",
        "the pinned unstaged answer"
    );
    // Every source file that forwards `source_key()` forwards `upstream()` and `stage_params()` too (a wrapper that drops them loses the chain).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut files = Vec::new();
    let mut stack = vec![root.join("crates"), root.join("xtask")];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "target") {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let mut forwarding = 0;
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        if text.contains("fn source_key(&self) -> Option<[u8; 16]> {") && text.contains(".source_key()") && !f.ends_with("stamp_keys.rs") {
            // A forwarding override: `self.<inner>.source_key()` inside `fn source_key`.
            let forwards =
                text.lines().zip(text.lines().skip(1)).any(|(a, b)| a.contains("fn source_key(&self)") && b.contains(".source_key()"));
            if forwards {
                forwarding += 1;
                assert!(
                    text.contains("fn upstream(&self)")
                        && text.contains("fn stage_params(&self)")
                        && text.contains("fn item_revision(&self)"),
                    "{} forwards source_key but not upstream, stage_params and item_revision",
                    f.display()
                );
            }
        }
    }
    assert!(forwarding >= 5, "the scan found the wrappers (Bound, water, skeleton, plan_ablation, origin_batch), {forwarding}");
}
