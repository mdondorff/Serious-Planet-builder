//! STRM-034: a plan cache told where the items changed keeps what the changes cannot reach, and plans what a fresh one plans, node for node,
//! after any sequence of edits; a change nobody announced starts the checks again from all tiles.

use std::sync::{Arc, RwLock};

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::lod::{changes_may_alter_tile, ItemChange};
use planet_frame::{plan_terrain_cached, scripted_views_field, LodParams, SelectionCache, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::StampStage;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn request(view: usize) -> TerrainRequest {
    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[view];
    TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, max_height_m: 20_000.0, ..LodParams::earth_1080p() },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    }
}

fn hill(id: u128, at: Vec3, reach_m: f64, amount: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: (id % 3) as i32,
        dir: at,
        content_hash: 0,
        reach_m,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

fn stage(def: &Arc<RwLock<WorldDefinition>>) -> StampStage<'static> {
    let base: Arc<dyn HeightField> = Arc::new(Reference(1));
    StampStage::new(base, def.clone(), BodyId::PLANET, DEFAULT_ITEM_LAYER, 0, 1).unwrap()
}

fn fresh(req: &TerrainRequest, field: &dyn HeightField) -> (TerrainPlan, u64) {
    let mut cache = SelectionCache::new();
    let plan = plan_terrain_cached(req, field, &mut cache).unwrap();
    (plan, cache.hashes_computed())
}

fn changes_of(new: &WorldDefinition, old: &WorldDefinition) -> Vec<ItemChange> {
    new.item_changes_from(old).into_iter().map(|(dir, reach_m)| ItemChange { dir, reach_m }).collect()
}

/// A random edit of the shared definition: a hill put, changed (another place and size) or removed.
fn edit(shared: &Arc<RwLock<WorldDefinition>>, rng: &mut Rng, ids: &mut Vec<u128>, next: &mut u128, near: Vec3) {
    let at = |rng: &mut Rng| -> Vec3 {
        let unit = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
        };
        match rng.next_u64() % 4 {
            // Near the view.
            0 => (near + Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5) * 0.002).normalized(),
            // At a cube corner and at a face edge, where the cells and the tiles meet across faces.
            1 => unit(35.264 + 0.2 * (rng.f() - 0.5), 45.0 + 0.2 * (rng.f() - 0.5)),
            2 => unit(0.2 * (rng.f() - 0.5), 45.0 + 0.2 * (rng.f() - 0.5)),
            _ => Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5).normalized(),
        }
    };
    let reach = |rng: &mut Rng| 100.0 * libm::pow(400.0, rng.f());
    let mut def = shared.write().unwrap();
    match rng.next_u64() % 3 {
        0 if !ids.is_empty() => {
            let id = ids[(rng.next_u64() as usize) % ids.len()];
            def.put_item_spec(BodyId::PLANET, hill(id, at(rng), reach(rng), 20.0 + 30.0 * rng.f())).unwrap();
        }
        1 if !ids.is_empty() => {
            let id = ids.remove((rng.next_u64() as usize) % ids.len());
            def.remove_item(BodyId::PLANET, id);
        }
        _ => {
            ids.push(*next);
            def.put_item_spec(BodyId::PLANET, hill(*next, at(rng), reach(rng), 20.0 + 30.0 * rng.f())).unwrap();
            *next += 1;
        }
    }
}

// spec: STRM-034, STRM-037
#[test]
fn a_cache_told_where_the_items_changed_plans_what_a_fresh_one_plans_after_every_random_edit() {
    let shared = Arc::new(RwLock::new(WorldDefinition::single_planet(R, 1)));
    let field = stage(&shared);
    let mut rng = Rng(7);
    let (mut ids, mut next) = (Vec::new(), 1u128);
    let reqs = [request(6), request(4)];
    let near = reqs[0].camera.position.0.normalized();
    let mut caches: Vec<SelectionCache> = reqs.iter().map(|_| SelectionCache::new()).collect();
    for (cache, req) in caches.iter_mut().zip(&reqs) {
        plan_terrain_cached(req, &field, cache).unwrap();
    }
    let mut kept_cheaper = 0;
    for step in 0..10 {
        let old = shared.read().unwrap().clone();
        let before = field.item_revision();
        edit(&shared, &mut rng, &mut ids, &mut next, near);
        let after = field.item_revision();
        assert_ne!(before, after);
        let changes = changes_of(&shared.read().unwrap(), &old);
        for (cache, req) in caches.iter_mut().zip(&reqs) {
            cache.note_item_change(before, after, changes.clone());
            let hashed = cache.hashes_computed();
            let plan = plan_terrain_cached(req, &field, cache).unwrap();
            let (cold, cold_hashes) = fresh(req, &field);
            assert_eq!(plan, cold, "step {step}: the persistent cache plans what a fresh one plans");
            if cache.hashes_computed() - hashed < cold_hashes {
                kept_cheaper += 1;
            }
        }
    }
    assert!(kept_cheaper >= 10, "the changes spared some of the footprint hashes in most plans: {kept_cheaper} of 20");
}

// spec: STRM-034
#[test]
fn two_edits_announced_one_after_the_other_and_planned_once_follow_the_chain_and_a_broken_chain_checks_everything() {
    let shared = Arc::new(RwLock::new(WorldDefinition::single_planet(R, 1)));
    let field = stage(&shared);
    let req = request(6);
    let near = req.camera.position.0.normalized();
    let mut cache = SelectionCache::new();
    plan_terrain_cached(&req, &field, &mut cache).unwrap();
    let mut rng = Rng(3);
    let (mut ids, mut next) = (Vec::new(), 1u128);
    // Two announced edits, one plan.
    for _ in 0..2 {
        let old = shared.read().unwrap().clone();
        let before = field.item_revision();
        edit(&shared, &mut rng, &mut ids, &mut next, near);
        cache.note_item_change(before, field.item_revision(), changes_of(&shared.read().unwrap(), &old));
    }
    assert_eq!(plan_terrain_cached(&req, &field, &mut cache).unwrap(), fresh(&req, &field).0, "a chain of two");
    // An edit that was not announced (the cache is not told): everything is checked again, and the plan is still right.
    edit(&shared, &mut rng, &mut ids, &mut next, near);
    let before = cache.hashes_computed();
    assert_eq!(plan_terrain_cached(&req, &field, &mut cache).unwrap(), fresh(&req, &field).0, "an edit nobody announced");
    let (_, cold_hashes) = fresh(&req, &field);
    assert!(cache.hashes_computed() - before >= cold_hashes / 2, "all the tiles are checked again");
    // An announcement that does not lead to the revision the field has is ignored.
    let old = shared.read().unwrap().clone();
    let before_rev = field.item_revision();
    edit(&shared, &mut rng, &mut ids, &mut next, near);
    cache.note_item_change(before_rev, before_rev ^ 0xFFFF, changes_of(&shared.read().unwrap(), &old));
    assert_eq!(plan_terrain_cached(&req, &field, &mut cache).unwrap(), fresh(&req, &field).0, "a broken chain");
}

// spec: STRM-034
#[test]
fn a_tile_the_changes_cannot_reach_keeps_the_hash_of_its_reads() {
    let shared = Arc::new(RwLock::new(WorldDefinition::single_planet(R, 1)));
    let field = stage(&shared);
    let req = request(6);
    let map = &TangentWarp;
    let mut rng = Rng(21);
    let (mut ids, mut next) = (Vec::new(), 1u128);
    let near = req.camera.position.0.normalized();
    let mut sampled = 0usize;
    let mut spared = 0usize;
    for _ in 0..16 {
        let old = shared.read().unwrap().clone();
        edit(&shared, &mut rng, &mut ids, &mut next, near);
        let changes = changes_of(&shared.read().unwrap(), &old);
        // Tiles at every level, round the changes and elsewhere.
        let mut places: Vec<Vec3> = changes.iter().map(|c| c.dir).collect();
        places.push(near);
        places.push(-near);
        for _ in 0..6 {
            places.push(Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5).normalized());
        }
        for dir in places {
            for level in 0..=17u8 {
                let tile = TileId::from_direction(map, dir, level);
                let reads_before = StampStageHash::of(&old, tile);
                let reads_after = StampStageHash::of(&shared.read().unwrap().clone(), tile);
                sampled += 1;
                if !changes_may_alter_tile(map, &req.lod, tile, &changes) {
                    spared += 1;
                    assert_eq!(reads_before, reads_after, "level {level} tile {tile:?} was spared but its reads changed");
                }
            }
        }
        let _ = &field;
    }
    assert!(spared * 3 >= sampled, "the test spares a good share of the tiles: {spared} of {sampled}");
}

// spec: STRM-035
#[test]
fn an_item_just_beyond_the_reach_of_a_tile_leaves_the_hash_of_its_reads_alone() {
    use planet_frame::lod::tile_reads_reach_rad;
    let req = request(6);
    let mut rng = Rng(99);
    let base = {
        let mut d = WorldDefinition::single_planet(R, 1);
        // Some items in the world, so that the covers of the coarse tiles hold something.
        for id in 1..=30u128 {
            let at = Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5).normalized();
            d.put_item_spec(BodyId::PLANET, hill(id, at, 200.0 + 3_000.0 * rng.f(), 30.0)).unwrap();
        }
        d
    };
    let (mut tested, mut whole_body) = (0usize, 0usize);
    for _ in 0..60 {
        let dir = Vec3::new(rng.f() - 0.5, rng.f() - 0.5, rng.f() - 0.5).normalized();
        for level in [0u8, 2, 3, 4, 5, 6, 7, 8, 10, 13, 17] {
            let tile = TileId::from_direction(&TangentWarp, dir, level);
            let region = tile_reads_reach_rad(&TangentWarp, &req.lod, tile);
            if !region.is_finite() {
                whole_body += 1;
                continue;
            }
            let centre = tile.center_direction(&TangentWarp);
            // The tangent direction of a random bearing at the centre.
            let helper = if centre.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
            let east = centre.cross(helper).normalized();
            let north = centre.cross(east).normalized();
            let bearing = rng.f() * core::f64::consts::TAU;
            let tangent = east * libm::cos(bearing) + north * libm::sin(bearing);
            let before = StampStageHash::of(&base, tile);
            for (reach_m, beyond) in [(1.0, 1e-5), (900.0, 1e-5), (40_000.0, 1e-5), (40_000.0, 1e-3), (5_000.0, 0.02)] {
                let angle = region + reach_m / R + beyond;
                if angle >= 3.0 {
                    continue;
                }
                let at = (centre * libm::cos(angle) + tangent * libm::sin(angle)).normalized();
                let mut edited = base.clone();
                edited.put_item_spec(BodyId::PLANET, hill(1_000, at, reach_m, 30.0)).unwrap();
                let change = [ItemChange { dir: at, reach_m }];
                assert!(!changes_may_alter_tile(&TangentWarp, &req.lod, tile, &change), "the test spares it: level {level}");
                assert_eq!(
                    before,
                    StampStageHash::of(&edited, tile),
                    "level {level} tile {tile:?}: an item {beyond} rad beyond the reach (item reach {reach_m} m)"
                );
                tested += 1;
            }
        }
    }
    assert!(tested > 1_000 && whole_body < tested, "{tested} placements tested, {whole_body} tiles read the whole body");
}

/// The hash of what a tile reads from a definition, as the plan cache's check takes it.
struct StampStageHash;

impl StampStageHash {
    fn of(def: &WorldDefinition, tile: TileId) -> u64 {
        let shared = Arc::new(RwLock::new(def.clone()));
        stage(&shared).inputs_hash_for(tile, &TangentWarp)
    }
}
