//! STRM-049: a world with a chunk stage bakes the horizon of its chunks as a second job, from its own field, and keeps the re-bakes few.

use std::sync::Arc;

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{ChunkKey, StandIn, PYRAMID_TOP_LEVEL};
use planet_generators::height::IMPLEMENTATION_ID as REFERENCE_ID;
use planet_generators::horizon::HorizonParams;
use planet_streaming::chunks::{ChunkConfig, HorizonConfig};
use planet_streaming::{FakeClock, ManualSpawner};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;

struct Rig {
    world: Arc<WorldTerrain>,
    clock: Arc<FakeClock>,
    chunk_jobs: Arc<ManualSpawner>,
    horizon_jobs: Arc<ManualSpawner>,
}

fn rig() -> Rig {
    let mut def = WorldDefinition::single_planet(R, 5);
    def.set_height_field(REFERENCE_ID);
    let world = Arc::new(WorldTerrain::open(def, 16).unwrap());
    let (clock, chunk_jobs, horizon_jobs) =
        (Arc::new(FakeClock::default()), Arc::new(ManualSpawner::default()), Arc::new(ManualSpawner::default()));
    let stage = Arc::new(StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 3 });
    world
        .enable_chunks(stage, CELLS, chunk_jobs.clone(), clock.clone(), None, ChunkConfig { workers: 16, ..ChunkConfig::default() })
        .unwrap();
    Rig { world, clock, chunk_jobs, horizon_jobs }
}

fn params() -> HorizonParams {
    // A cell of the 16-cell lattice is about 300 m: a range of 2 km needs a halo of 9 cells.
    HorizonParams { azimuths: 8, range_m: 2_000.0, ..HorizonParams::chunk(R) }
}

fn tile() -> TileId {
    TileId::new(Face(2), PYRAMID_TOP_LEVEL, 8 * 1000 + 3, 8 * 1000 + 3).unwrap()
}

fn camera() -> PlanetFixed {
    let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    PlanetFixed(chunk.center_direction(&TangentWarp) * (R + 300.0))
}

/// One frame of the world: the chunk jobs, then the horizon jobs, both run to the end of what they started.
fn frame(r: &Rig) {
    r.world.chunk_frame(&[tile()], camera(), &[]);
    r.chunk_jobs.run(64);
    r.world.horizon_frame(&[tile()], camera());
    r.horizon_jobs.run(64);
}

// spec: STRM-049
#[test]
fn the_horizon_jobs_need_the_chunk_stage_and_a_world_without_them_does_nothing() {
    let mut def = WorldDefinition::single_planet(R, 5);
    def.set_height_field(REFERENCE_ID);
    let world = Arc::new(WorldTerrain::open(def, 16).unwrap());
    let err = world
        .enable_horizon(params(), Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()), HorizonConfig::default(), 2)
        .unwrap_err();
    assert!(err.contains("enable_chunks"), "{err}");
    assert_eq!(world.horizon_frame(&[tile()], camera()), Default::default());
    assert!(world.horizon_map((2, 1000, 1000)).is_none() && world.horizon_stats().is_none());
}

// spec: STRM-049
#[test]
fn a_chunk_gets_its_horizon_map_after_its_heights_from_the_worlds_own_field_and_deterministically() {
    let key: ChunkKey = (2, 1000, 1000);
    let maps: Vec<Vec<u8>> = (0..2)
        .map(|_| {
            let r = rig();
            r.world
                .enable_horizon(params(), r.horizon_jobs.clone(), r.clock.clone(), HorizonConfig { workers: 4, settle_ms: 1_000 }, 2)
                .unwrap();
            // Before the heights: the horizon jobs start nothing.
            assert_eq!(r.world.horizon_frame(&[tile()], camera()).started, 0);
            r.world.chunk_frame(&[tile()], camera(), &[]);
            assert_eq!(r.horizon_jobs.pending(), 0);
            r.chunk_jobs.run(64);
            r.world.chunk_frame(&[tile()], camera(), &[]);
            let up = r.world.horizon_frame(&[tile()], camera());
            assert!(up.started >= 1, "the chunks are ready: their horizon jobs start");
            assert!(r.world.horizon_map(key).is_none(), "until it lands the coarse horizon alone shades the tile");
            r.horizon_jobs.run(64);
            r.world.horizon_frame(&[tile()], camera());
            let map = r.world.horizon_map(key).expect("the map landed");
            assert_eq!(map.len(), (CELLS * CELLS * 8) as usize, "one byte per azimuth for each cell of the chunk");
            (*map).clone()
        })
        .collect();
    assert_eq!(maps[0], maps[1], "the same world gives the same map");
}

// spec: STRM-049
#[test]
fn a_flight_over_the_chunks_lands_every_map_and_the_rebakes_stay_few_and_an_edit_bakes_again() {
    let r = rig();
    r.world.enable_horizon(params(), r.horizon_jobs.clone(), r.clock.clone(), HorizonConfig { workers: 2, settle_ms: 1_000 }, 2).unwrap();
    // 100 ms frames for 12 s: the chunks land, then their neighbours, and the horizon jobs follow.
    for _ in 0..120 {
        frame(&r);
        r.clock.advance(100);
    }
    let s = r.world.horizon_stats().unwrap();
    let chunks = r.world.chunk_manager().unwrap().stats().landed;
    eprintln!("flight: {s:?} for {chunks} chunks");
    assert!(chunks >= 1 && s.landed >= chunks, "every ready chunk has a map: {s:?} for {chunks} chunks");
    assert!(s.rebaked <= 2 * chunks, "the re-bakes stay at one or two per chunk: {s:?} for {chunks} chunks");
    let before = s.baked;
    // Nothing changes: no further bake.
    for _ in 0..30 {
        frame(&r);
        r.clock.advance(100);
    }
    assert_eq!(r.world.horizon_stats().unwrap().baked, before, "an unchanged world bakes nothing more");
    // An edit changes the definition's revision and so every map's state word: they are baked again once the edit has settled.
    let mut def = (*r.world.definition()).clone();
    let item = ItemSpec {
        id: 1,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: tile().center_direction(&TangentWarp),
        content_hash: 0,
        reach_m: 800.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
    def.put_item_spec(BodyId::PLANET, item).unwrap();
    r.world.set_definition(Arc::new(def)).unwrap();
    for _ in 0..40 {
        frame(&r);
        r.clock.advance(100);
    }
    let after = r.world.horizon_stats().unwrap();
    assert!(after.baked > before, "the edit bakes the maps again: {after:?}");
    assert!(after.rebaked > s.rebaked);
}
