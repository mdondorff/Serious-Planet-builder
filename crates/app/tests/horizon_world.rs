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
fn a_horizon_reads_the_three_by_three_chunks_around_it_across_face_edges_too() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    let lat = r.world.chunk_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&r.world, lat, params(), 1);
    let last = (1u32 << CHUNK_GRID_LEVEL) - 1;
    for key in [(2u8, 1000u32, 1000u32), (2, 0, 0), (2, last, 3), (4, 0, last)] {
        let reads = baker.reads(key);
        assert_eq!(reads[0], key, "its own chunk first");
        let mut sorted = reads.clone();
        sorted.sort_unstable();
        sorted.dedup();
        // A chunk at a corner of the cube has eight neighbours (three faces meet there), every other chunk nine in all.
        let corner = (key.1 == 0 || key.1 == last) && (key.2 == 0 || key.2 == last);
        assert_eq!(sorted.len(), if corner { 8 } else { 9 }, "{key:?}: the chunk and its neighbours, no two the same: {reads:?}");
    }
}

// spec: STRM-049
#[test]
fn a_march_the_chunks_cannot_hold_or_the_bake_refuses_is_turned_down_at_once_and_a_gone_world_gives_a_map_of_zeros() {
    use planet::horizonworld::{chunk_horizon_params, max_range_m, ChunkHorizonBaker};
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    let spawn = |r: &Rig| -> (Arc<ManualSpawner>, Arc<FakeClock>) { (r.horizon_jobs.clone(), r.clock.clone()) };
    let (s, c) = spawn(&r);
    let lat = r.world.chunk_lattice().unwrap();
    // A range whose halo does not fit in a chunk edge: refused, with the longest range that does.
    let long = HorizonParams { range_m: 3.0 * max_range_m(&lat, R), ..params() };
    let e = r.world.enable_horizon(long, s.clone(), c.clone(), HorizonConfig::default(), 1).unwrap_err();
    assert!(e.contains("halo") && e.contains("chunk edge") && e.contains("at most"), "{e}");
    // Parameters the bake would refuse never reach a worker.
    for bad in [
        HorizonParams { azimuths: 0, ..params() },
        HorizonParams { step_min_m: 1e-20, ..params() },
        HorizonParams { radius_m: f64::NAN, ..params() },
    ] {
        let e = r.world.enable_horizon(bad, s.clone(), c.clone(), HorizonConfig::default(), 1);
        assert!(e.is_err(), "{bad:?}");
    }
    assert!(
        r.world.enable_horizon(chunk_horizon_params(&lat, R), s, c, HorizonConfig::default(), 1).is_ok(),
        "the world's own march is accepted"
    );
    assert!(chunk_horizon_params(&lat, R).range_m <= max_range_m(&lat, R));
    // The default march on the 8 m lattice of the A-net default keeps the range inside a chunk edge.
    let a_net = planet_generators::chunkfield::ChunkLattice::new(614).unwrap();
    let p = chunk_horizon_params(&a_net, R);
    assert!(p.range_m < 5_000.0 && p.range_m > 4_700.0, "{} m", p.range_m);
    // A world that is gone gives a map of the right length, all zeros, not a short one.
    let gone = {
        let r2 = rig();
        ChunkHorizonBaker::new(&r2.world, r2.world.chunk_lattice().unwrap(), params(), 1)
    };
    let map = gone.bake((2, 1000, 1000), &[]);
    assert_eq!(map.len(), (CELLS * CELLS * 8) as usize);
    assert!(map.iter().all(|&b| b == 0));
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

// spec: STRM-049, STRM-057
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
    // An edit far from every chunk of the flight changes no map's word: nothing is baked again.
    let far = ItemSpec {
        id: 7,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: -tile().center_direction(&TangentWarp),
        content_hash: 0,
        reach_m: 800.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
    let mut def = (*r.world.definition()).clone();
    def.put_item_spec(BodyId::PLANET, far).unwrap();
    r.world.set_definition(Arc::new(def.clone())).unwrap();
    for _ in 0..40 {
        frame(&r);
        r.clock.advance(100);
    }
    assert_eq!(r.world.horizon_stats().unwrap().baked, before, "an edit outside every halo bakes nothing again");
    // An edit at the chunks changes the words of the chunks it touches: they are baked again once the edit has settled, and only they.
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
    assert!(after.baked > before, "the edit bakes the maps it touches again: {after:?}");
    // (The chunks of this rig are a few hundred metres across, so the hill reaches all nine; the selectivity is the far edit above.)
    assert!(after.baked - before <= chunks, "at most one bake per chunk: {} for {chunks} chunks", after.baked - before);
    assert!(after.rebaked > s.rebaked);
}

// spec: STRM-047
#[test]
fn the_worlds_field_chain_hands_the_chunk_change_to_the_tile_builder() {
    use planet_generators::height::HeightField;
    let r = rig();
    let dirs = [tile().center_direction(&TangentWarp)];
    // No chunk ready: no change, so the tiles of a world without chunks are built as they were.
    assert_eq!(r.world.field().unwrap().level_change(&dirs, 17), None);
    for _ in 0..40 {
        r.world.chunk_frame(&[tile()], camera(), &[]);
        r.chunk_jobs.run(64);
        r.clock.advance(100);
    }
    let field = r.world.field().unwrap();
    let got = field.level_change(&dirs, 17).expect("a chunk landed, the chain carries its change");
    assert!(got[0] != 0.0, "the stand-in changes the ground at the tile: {got:?}");
    let by_difference = field.height_at_level(dirs[0], 17) - field.height_at_level(dirs[0], 5);
    assert!((got[0] - by_difference).abs() < 1e-6, "{} against {by_difference}", got[0]);
}

// spec: STRM-047
#[test]
fn a_plateau_that_absorbs_the_chunk_change_leaves_none_of_it_in_the_plane() {
    use planet_generators::height::HeightField;
    let r = rig();
    let d = tile().center_direction(&TangentWarp);
    for _ in 0..40 {
        r.world.chunk_frame(&[tile()], camera(), &[]);
        r.chunk_jobs.run(64);
        r.clock.advance(100);
    }
    let bare = r.world.field().unwrap();
    let plain = bare.level_change(&[d], 17).expect("a chunk landed")[0];
    assert!(plain.abs() > 1.0, "the stand-in changes the ground by metres here: {plain}");
    // A plateau at the height the ground has before the chunk, strong enough to take the whole change: the stamp stage holds the final height at
    // the target, so the change that reaches the texel is the difference of the stamped heights, not the chunk's change.
    let target = bare.height_at_level(d, 5);
    let mut def = (*r.world.definition()).clone();
    let item = ItemSpec {
        id: 2,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: d,
        content_hash: 0,
        reach_m: 20_000.0,
        max_delta_m: 500.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Plateau, [target, 0.0, 0.0]));
    def.put_item_spec(BodyId::PLANET, item).unwrap();
    // The plateau sits above the chunk stage (`on-top`): the other position, `under`, lets the change through the stamp.
    def.set_chunk_stage(BodyId::PLANET, DEFAULT_ITEM_LAYER, planet_world_def::ChunkStagePosition::OnTop);
    r.world.set_definition(Arc::new(def)).unwrap();
    let field = r.world.field().unwrap();
    let got = field.level_change(&[d], 17).expect("a chunk landed")[0];
    let by_difference = field.height_at_level(d, 17) - field.height_at_level(d, 5);
    assert!((got - by_difference).abs() < 1e-6, "plane {got} against the stamped difference {by_difference}");
    assert!(
        got.abs() < 1e-6 && by_difference.abs() < 1e-6,
        "the plateau absorbs the change: {got}, {by_difference} (the chunk's own {plain})"
    );
}

fn land_chunks(r: &Rig) {
    for _ in 0..40 {
        r.world.chunk_frame(&[tile()], camera(), &[]);
        r.chunk_jobs.run(64);
        r.clock.advance(100);
    }
}

fn hill_at(chunk: (u32, u32), id: u128) -> ItemSpec {
    let at = TileId::new(Face(2), CHUNK_GRID_LEVEL, chunk.0, chunk.1).unwrap().center_direction(&TangentWarp);
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: at, content_hash: 0, reach_m: 800.0, max_delta_m: 60.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]))
}

// spec: STRM-059
#[test]
fn a_bake_from_shared_height_tiles_equals_the_bake_that_evaluates_its_whole_raster_and_neighbours_share_the_tiles() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    land_chunks(&r);
    let lat = r.world.chunk_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    let east: ChunkKey = (2, 1001, 1000);
    let reads = shared.reads(own);
    assert!(reads.len() == 9, "the rig's chunks around the tile are all ready: {reads:?}");
    let first = shared.bake(own, &[]);
    assert_eq!(first, plain.bake(own, &[]), "the chunk");
    assert!(first.iter().any(|&b| b > 0), "the stand-in's relief shades something");
    assert_eq!(shared.tile_stats(), planet::horizonworld::TileStats { evaluated: 9, hits: 0, held: 9 });
    // The chunk to the east shares six of its nine tiles with the first: three are new.
    assert_eq!(shared.bake(east, &[]), plain.bake(east, &[]), "the neighbour");
    let s = shared.tile_stats();
    assert_eq!((s.evaluated, s.hits), (12, 6), "{s:?}");
    // The same chunk again evaluates nothing.
    assert_eq!(shared.bake(own, &[]), first);
    assert_eq!(shared.tile_stats().evaluated, 12);
    // A cache that holds fewer tiles than a bake reads still gives the same map, and never holds more than its bound.
    let tight = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(4);
    assert_eq!(tight.bake(own, &[]), first);
    assert_eq!(tight.tile_stats().held, 4);
}

// spec: STRM-059
#[test]
fn a_neighbours_edit_replaces_the_tile_it_touches_in_the_halo_and_no_other_and_a_chunk_landing_replaces_its_own() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    let lat = r.world.chunk_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    // Nothing landed yet: the tiles are the base alone. Then the chunks land: every tile is made again (their key holds the result).
    let before_landing = shared.bake(own, &[]);
    assert_eq!(shared.tile_stats().evaluated, 9);
    land_chunks(&r);
    let landed = shared.bake(own, &[]);
    assert_eq!(landed, plain.bake(own, &[]));
    assert_ne!(landed, before_landing, "the stand-in's change is in the heights");
    assert_eq!(shared.tile_stats().evaluated, 18, "nine tiles held a chunk that was not ready: all nine are made again");
    // An edit inside the chunk to the east, 800 m across and clear of every other chunk of the nine.
    let mut def = (*r.world.definition()).clone();
    def.put_item_spec(BodyId::PLANET, hill_at((1001, 1000), 11)).unwrap();
    r.world.set_definition(Arc::new(def)).unwrap();
    let edited = shared.bake(own, &[]);
    assert_eq!(edited, plain.bake(own, &[]), "the edit is in the map, as if nothing had been kept");
    assert_ne!(edited, landed, "the hill beside the chunk shades its edge");
    let s = shared.tile_stats();
    // (A tile's key holds the items within the stamp stage's halo of it, so the hill, 2.5 km in, also re-keys the tiles beside the east one;
    // the three of the far west column, 7 km away, are read from the cache.)
    assert!(
        s.evaluated > 18 && s.hits >= 3 && s.evaluated - 18 + s.hits == 9,
        "only the tiles the hill reaches are made again, not the nine: {s:?}"
    );
    // Nothing changes: nothing is made again.
    let again = shared.bake(own, &[]);
    assert_eq!((again, shared.tile_stats().evaluated), (edited, s.evaluated));
}

// spec: STRM-059
#[test]
fn a_change_of_the_lens_makes_every_tile_again_and_the_map_still_equals_the_whole_raster() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    land_chunks(&r);
    let lat = r.world.chunk_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    shared.bake(own, &[]);
    assert_eq!(shared.tile_stats().evaluated, 9);
    // The coupled lens (the field chain does not hash it into a tile key).
    r.world.set_coupled_lens(Some(planet_generators::coupled::CoupledLens::Height));
    assert_eq!(shared.bake(own, &[]), plain.bake(own, &[]));
    assert_eq!(shared.tile_stats().evaluated, 18, "a lens makes all nine again");
}

// spec: STRM-059
#[test]
fn a_chunk_that_lands_after_a_neighbours_tile_was_kept_replaces_the_tile_because_the_spline_reads_across_the_edge() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::chunks::HorizonBaker;
    let r = rig();
    let lat = r.world.chunk_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    // Only the chunk under the tile lands first (one frame of the chunk jobs for the tile alone), then the rest.
    r.world.chunk_frame(&[tile()], camera(), &[]);
    r.chunk_jobs.run(1);
    let first = shared.bake(own, &[]);
    assert_eq!(first, plain.bake(own, &[]));
    let made = shared.tile_stats().evaluated;
    land_chunks(&r);
    assert_eq!(shared.bake(own, &[]), plain.bake(own, &[]), "the map equals the whole raster after the neighbours landed");
    assert!(shared.tile_stats().evaluated > made, "the tiles were made again");
}
