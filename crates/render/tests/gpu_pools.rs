//! STRM-006, REND-013: terrain tiles live in pooled fixed-size slots of a few large vertex buffers. Where a tile sits is
//! decided by the (GPU-free) residency manager or by the renderer's own free list; the image does not depend on it. The
//! allocation equals the pure model (`PoolConfig`) and stays under the tier's share. WARP and hardware alike.

use planet_core::cube::TangentWarp;
use planet_core::TileId;
use planet_frame::pool::{tier_pool, vertex_slot_bytes};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::{grid_indices, tile_mesh, TileMesh};
use planet_render::{execute_terrain, AdapterPolicy, ExecError, GpuContext, TerrainRenderer};
use planet_streaming::{
    FakeClock, ManualSpawner, PoolConfig, PoolKind, ResidencyManager, SlotRef, Spawner, StreamConfig, Streamer, Tier, TileSource, Wants,
};
use std::collections::HashSet;
use std::sync::Arc;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;

fn plan(index: usize) -> planet_frame::TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn meshes(p: &planet_frame::TerrainPlan) -> Vec<TileMesh> {
    p.nodes.iter().map(|n| tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).collect()
}

/// Small pools: 3 slots per block, the vertex pool for levels 0-2.
fn small_pool(vertex_slots: u32, retire: u64) -> PoolConfig {
    let slot = vertex_slot_bytes(CELLS);
    PoolConfig {
        slot_bytes: slot,
        block_slots: 3,
        main_blocks: 200,
        vertex_slot_bytes: slot,
        vertex_slots,
        index_bytes: grid_indices(CELLS).len() as u64 * 4,
        node_record_bytes: 4096 * 512,
        frames_in_flight: retire,
        slot_record_bytes: 0,
        fallback_slots: 0,
    }
}

// spec: STRM-006, REND-013
#[test]
fn the_image_does_not_depend_on_where_the_tiles_sit() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [3, 4, 5] {
        let p = plan(index);
        let ms = meshes(&p);
        let refs: Vec<&TileMesh> = ms.iter().collect();
        let (reference, _) = execute_terrain(&ctx, &p, &refs).unwrap();
        // The renderer's own free list in a pool of 3-slot blocks and a vertex pool for levels 0-2.
        let mut own = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, small_pool(8, 1));
        for m in &ms {
            own.upload(&ctx, m).unwrap();
        }
        own.render(&ctx, &p, false).unwrap();
        assert_eq!(own.read_color(&ctx), reference, "view {index}: free-list placement");
        // Placed by the caller, scattered over blocks in reverse order with gaps.
        let mut placed = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, small_pool(8, 1));
        let (mut main, mut vertex) = (599u32, 0u32);
        for m in ms.iter().rev() {
            let slot = if m.id.level() <= 2 {
                vertex += 1;
                SlotRef { pool: PoolKind::Vertex, index: (vertex - 1) % 8 }
            } else {
                main -= 2;
                SlotRef { pool: PoolKind::Main, index: main }
            };
            placed.upload_at(&ctx, m, slot).unwrap();
        }
        assert!(ms.iter().filter(|m| m.id.level() <= 2).count() <= 8, "the fixture's vertex pool is big enough");
        placed.render(&ctx, &p, false).unwrap();
        assert_eq!(placed.read_color(&ctx), reference, "view {index}: scattered placement");
        assert_eq!(placed.resident_tiles(), ms.len());
        // A slot outside the pool is refused with the numbers.
        let e = placed.upload_at(&ctx, &ms[0], SlotRef { pool: PoolKind::Main, index: 600 }).unwrap_err();
        assert!(matches!(e, ExecError::BadSlot { capacity: 600, .. }), "{e}");
    }
}

// spec: STRM-006
#[test]
fn the_allocation_equals_the_model_and_stays_under_the_share() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(4);
    let ms = meshes(&p);
    let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, small_pool(8, 1));
    let config = *r.pool_config();
    assert_eq!(r.pool_allocated_bytes(), config.index_bytes + r.node_buffer_bytes(), "nothing allocated before the first tile");
    for m in &ms {
        r.upload(&ctx, m).unwrap();
    }
    r.render(&ctx, &p, false).unwrap();
    let (blocks, vertex_live) = r.live_pools();
    let (in_use, retiring, free) = r.main_slot_counts();
    assert_eq!(in_use as usize, ms.iter().filter(|m| m.id.level() > 2).count());
    assert_eq!((retiring, in_use + free), (0, config.main_slots()));
    // Lowest slot first: the touched blocks are exactly the ones the slots in use need.
    assert_eq!(blocks, config.blocks_for(in_use), "{in_use} slots touch {blocks} blocks");
    assert_eq!(vertex_live, ms.iter().any(|m| m.id.level() <= 2));
    let modelled = config.allocated_bytes_with(blocks, vertex_live) - config.node_record_bytes + r.node_buffer_bytes();
    assert_eq!(r.pool_allocated_bytes(), modelled, "the real buffers equal the pure model");
    assert!(r.node_buffer_bytes() <= config.node_record_bytes);
    assert!(r.pool_allocated_bytes() <= config.allocated_bytes());
    // The default renderer: the mid tier's share is a ceiling on the model, at the renderer's own slot size.
    let default = TerrainRenderer::new(&ctx, SIZE, CELLS);
    assert!(default.pool_config().allocated_bytes() <= Tier::Mid.terrain_share_bytes());
    assert_eq!(default.pool_config(), &tier_pool(Tier::Mid, CELLS, 1).unwrap());
    assert!(default.pool_allocated_bytes() < default.pool_config().allocated_bytes(), "pools are allocated on demand");
}

// spec: STRM-006, STRM-007
#[test]
fn a_full_pool_is_refused_and_a_freed_slot_waits_for_the_frames_in_flight() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(4);
    let ms: Vec<TileMesh> = meshes(&p).into_iter().filter(|m| m.id.level() > 2).take(4).collect();
    assert_eq!(ms.len(), 4, "the fixture has four tiles of level 3 or finer");
    let slot = vertex_slot_bytes(CELLS);
    let config = PoolConfig { main_blocks: 1, block_slots: 3, ..small_pool(8, 1) };
    assert_eq!(config.slot_bytes, slot);
    let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, config);
    for m in &ms[..3] {
        r.upload(&ctx, m).unwrap();
    }
    let e = r.upload(&ctx, &ms[3]).unwrap_err();
    assert!(matches!(e, ExecError::PoolFull { pool: PoolKind::Main, tile } if tile == ms[3].id), "{e}");
    assert!(e.to_string().contains("no free slot"), "{e}");
    // Evict one: its slot is still in flight until a frame has been rendered.
    let freed = r.slot_of(ms[0].id).unwrap();
    assert!(r.evict(ms[0].id));
    assert!(!r.evict(ms[0].id), "evicting twice does nothing");
    assert!(matches!(r.upload(&ctx, &ms[3]), Err(ExecError::PoolFull { .. })), "the freed slot is in flight");
    let mut one = p.clone();
    one.nodes = vec![*p.nodes.iter().find(|n| n.tile == ms[1].id).unwrap()];
    r.render(&ctx, &one, false).unwrap();
    r.upload(&ctx, &ms[3]).unwrap();
    assert_eq!(r.slot_of(ms[3].id), Some(freed), "the retired slot is reused");
    assert_eq!(r.resident_tiles(), 3);
}

struct MeshSource;

impl TileSource<TileMesh> for MeshSource {
    fn load(&self, tile: TileId) -> Result<TileMesh, String> {
        Ok(tile_mesh(1, &TangentWarp, tile, CELLS, R))
    }
}

fn settle(m: &mut ResidencyManager<TileMesh>, sp: &ManualSpawner, w: &Wants) {
    for _ in 0..60 {
        m.begin_frame(w);
        sp.run(32);
    }
    m.begin_frame(w);
}

// spec: STRM-006, STRM-007
#[test]
fn the_residency_managers_slots_drive_the_renderer_and_the_images_are_unchanged() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (pa, pb) = (plan(4), plan(3));
    let closure = |p: &planet_frame::TerrainPlan| -> HashSet<TileId> {
        let mut s = HashSet::new();
        for n in &p.nodes {
            let mut x = Some(n.tile);
            while let Some(a) = x {
                s.insert(a);
                x = a.parent();
            }
        }
        s
    };
    // A share just big enough for either plan alone (plus the six roots), not for both: moving from one to the other
    // evicts and reuses slots.
    let main_levels = |s: &HashSet<TileId>| s.iter().filter(|t| t.level() > 2).count();
    let capacity = main_levels(&closure(&pa)).max(main_levels(&closure(&pb))) + 6;
    let config = StreamConfig { max_in_flight: 16, capacity, vertex_pool_slots: 512, retire_frames: 1, ..Default::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let mut streamer = Streamer::new(Arc::new(MeshSource), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    streamer.load_roots_blocking().unwrap();
    let mut manager = ResidencyManager::new(streamer);
    let mut renderer = TerrainRenderer::new(&ctx, SIZE, CELLS);
    let mut evictions_seen = 0;
    for (round, p) in [&pa, &pb, &pa].into_iter().enumerate() {
        let tiles: Vec<(TileId, f64)> = p.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect();
        let w = Wants { draw: tiles.clone(), residency: tiles, prefetch: vec![] };
        settle(&mut manager, &spawner, &w);
        let frame = manager.begin_frame(&w);
        assert!(frame.resolved.iter().all(|r| r.exact()), "round {round}: the plan is resident");
        assert!(!frame.report.budget_failure, "round {round}: {:?}", frame.report);
        // The renderer executes the manager's placement for every resident tile it does not hold in that slot.
        let s = manager.streamer();
        for n in &p.nodes {
            let slot = s.slot_of(n.tile).unwrap();
            if renderer.slot_of(n.tile) != Some(slot) || !renderer.has_mesh(n.tile, n.mesh_key) {
                renderer.upload_at(&ctx, &s.get(n.tile).unwrap(), slot).unwrap();
            }
        }
        renderer.render(&ctx, p, false).unwrap();
        let ms = meshes(p);
        let refs: Vec<&TileMesh> = ms.iter().collect();
        let (reference, _) = execute_terrain(&ctx, p, &refs).unwrap();
        assert_eq!(renderer.read_color(&ctx), reference, "round {round}: pooled slots draw the same image");
        let st = s.stats();
        assert!(st.main_in_use <= capacity, "round {round}: {st:?}");
        evictions_seen = st.evicted;
        s.pools().check().unwrap();
    }
    assert!(evictions_seen > 0, "the share forced the slots to be reused");
}
