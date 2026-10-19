//! STRM-027: the plan cache keys the tiles of a stamped field by their reads, so a persistent cache gives the plan of a fresh one after
//! any edit of the items, however it was made (put, change, delete, undo, a reload into another definition).

use std::sync::{Arc, RwLock};

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::StampStage;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn request() -> TerrainRequest {
    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[6];
    TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    }
}

fn hill(id: u128, at: Vec3, reach_m: f64, amount: f64) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: at, content_hash: 0, reach_m, max_delta_m: 60.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

fn world() -> WorldDefinition {
    WorldDefinition::single_planet(R, 1)
}

fn stage(def: &Arc<RwLock<WorldDefinition>>) -> StampStage {
    let base: Arc<dyn HeightField> = Arc::new(Reference(1));
    StampStage::new(base, def.clone(), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).unwrap()
}

fn fresh(req: &TerrainRequest, field: &dyn HeightField) -> TerrainPlan {
    plan_terrain_cached(req, field, &mut SelectionCache::new()).unwrap()
}

// spec: STRM-027
#[test]
fn a_persistent_cache_plans_what_a_fresh_one_plans_after_put_change_delete_undo_and_a_reload() {
    let req = request();
    let shared = Arc::new(RwLock::new(world()));
    let field = stage(&shared);
    let mut cache = SelectionCache::new();
    let empty = plan_terrain_cached(&req, &field, &mut cache).unwrap();
    assert_eq!(empty, fresh(&req, &field), "empty");
    let under: TileId = empty.nodes[empty.nodes.len() / 2].tile;
    let at = under.center_direction(&TangentWarp);
    let check = |cache: &mut SelectionCache, what: &str| {
        let plan = plan_terrain_cached(&req, &field, cache).unwrap();
        assert_eq!(plan, fresh(&req, &field), "{what}: the persistent cache plans what a fresh one plans");
        plan
    };
    // Put.
    shared.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, at, 3_000.0, 40.0)).unwrap();
    let put = check(&mut cache, "put");
    assert_ne!(put, empty, "the stamp is in the plan");
    // Change (reach, amount).
    shared.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, at, 2_000.0, 25.0)).unwrap();
    let changed = check(&mut cache, "change");
    assert_ne!(changed, put);
    // Delete.
    shared.write().unwrap().remove_item(BodyId::PLANET, 1);
    assert_eq!(check(&mut cache, "delete"), empty, "deleting restores the plan of before");
    // Undo of the delete: the same item again.
    shared.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, at, 2_000.0, 25.0)).unwrap();
    assert_eq!(check(&mut cache, "undo"), changed, "the undone edit restores the plan");
    // A warm plan of unchanged items checks no tile again.
    let before = cache.hashes_computed();
    check(&mut cache, "warm");
    assert_eq!(cache.hashes_computed() - before, 0, "an unchanged revision costs no footprint query");
}

// spec: STRM-027
#[test]
fn a_reload_into_another_definition_with_the_same_number_of_edits_reuses_nothing_stale() {
    let req = request();
    let a = Arc::new(RwLock::new(world()));
    let field_a = stage(&a);
    let mut cache = SelectionCache::new();
    let empty = plan_terrain_cached(&req, &field_a, &mut cache).unwrap();
    let under = empty.nodes[empty.nodes.len() / 2].tile;
    let at = under.center_direction(&TangentWarp);
    a.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, at, 3_000.0, 40.0)).unwrap();
    let plan_a = plan_terrain_cached(&req, &field_a, &mut cache).unwrap();
    // Another definition made by the same number of edits, with a different item: its counter equals the first one's.
    let b = Arc::new(RwLock::new(world()));
    b.write().unwrap().put_item_spec(BodyId::PLANET, hill(1, at, 3_000.0, -40.0)).unwrap();
    assert_ne!(a.read().unwrap().revision(), b.read().unwrap().revision(), "the revisions of two definitions never meet");
    let field_b = stage(&b);
    let plan_b = plan_terrain_cached(&req, &field_b, &mut cache).unwrap();
    assert_eq!(plan_b, fresh(&req, &field_b), "the reloaded definition is planned from its own items");
    assert_ne!(plan_a, plan_b, "the two definitions differ under the camera");
    // A clone is another instance too, whatever its history.
    let cloned = a.read().unwrap().clone();
    assert_ne!(cloned.revision(), a.read().unwrap().revision());
}
