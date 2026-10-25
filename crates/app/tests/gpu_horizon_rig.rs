//! Tier C: the viewer's horizon rig (REND-038, STRM-049): a world with a chunk stage and horizon jobs, a renderer, and the rig that moves the
//! landed maps into the renderer's slots one a frame. The slot holds what `pack_map` makes of the world's map; before the upload the node reads no map.

use std::sync::Arc;

use planet::horizonworld::HorizonRig;
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_frame::horizonview::pack_map;
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{ChunkKey, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::IMPLEMENTATION_ID as REFERENCE_ID;
use planet_generators::horizon::HorizonParams;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::chunks::{ChunkConfig, HorizonConfig};
use planet_streaming::{FakeClock, ManualSpawner};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;

fn tile() -> TileId {
    TileId::new(Face(2), PYRAMID_TOP_LEVEL, 8 * 1000 + 3, 8 * 1000 + 3).unwrap()
}

fn camera() -> PlanetFixed {
    let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    PlanetFixed(chunk.center_direction(&TangentWarp) * (R + 300.0))
}

// spec: REND-038, STRM-049
#[test]
fn the_rig_moves_a_landed_map_into_its_slot_one_a_frame_and_the_slot_holds_the_packed_map() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut def = WorldDefinition::single_planet(R, 5);
    def.set_height_field(REFERENCE_ID);
    let world = Arc::new(WorldTerrain::open(def, 16).unwrap());
    let (clock, chunk_jobs, horizon_jobs) =
        (Arc::new(FakeClock::default()), Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
    let stage = Arc::new(StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 3 });
    world
        .enable_chunks(stage, CELLS, chunk_jobs.clone(), clock.clone(), None, ChunkConfig { workers: 16, ..ChunkConfig::default() })
        .unwrap();
    // 16 azimuths: what the shader reads.
    let params = HorizonParams { azimuths: 16, range_m: 2_000.0, ..HorizonParams::chunk(R) };
    world.enable_horizon(params, horizon_jobs.clone(), clock.clone(), HorizonConfig { workers: 4, settle_ms: 1_000 }, 2).unwrap();
    let mut renderer = TerrainRenderer::new(&ctx, (64, 48), 16);
    let mut rig = HorizonRig::default();
    let key: ChunkKey = (2, 1000, 1000);
    let sun = [0.0f32, 0.6, 0.8];
    let mut uploaded = false;
    for _ in 0..200 {
        world.chunk_frame(&[tile()], camera(), &[]);
        chunk_jobs.run(64);
        rig.frame(&world, &ctx, &mut renderer, &[tile()], camera(), sun);
        horizon_jobs.run(64);
        clock.advance(100);
        if renderer.horizon_bytes() > 0 && world.horizon_map(key).is_some() {
            // Frames after the landing carry the upload.
            for _ in 0..4 {
                rig.frame(&world, &ctx, &mut renderer, &[tile()], camera(), sun);
            }
            uploaded = true;
            break;
        }
    }
    assert!(uploaded, "the map landed and the rig gave the renderer an array");
    let map = world.horizon_map(key).unwrap();
    let want = pack_map(&map, CELLS as usize, 16).expect("a 16-azimuth map packs");
    // Slot 0 is the first the chunk of the tile took.
    assert!(renderer.read_horizon_slot(&ctx, 0) == want, "the slot holds the packed map of the world");
}
