//! STRM-049: a world bakes the horizon of its chunks as a job, from its own field (the base terrain and the stamps), and keeps the re-bakes few.

use std::sync::Arc;

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{ChunkKey, PYRAMID_TOP_LEVEL};
use planet_generators::height::IMPLEMENTATION_ID as REFERENCE_ID;
use planet_generators::horizon::HorizonParams;
use planet_streaming::horizonjobs::HorizonConfig;
use planet_streaming::{FakeClock, ManualSpawner};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;

struct Rig {
    world: Arc<WorldTerrain>,
    clock: Arc<FakeClock>,
    horizon_jobs: Arc<ManualSpawner>,
}

fn rig() -> Rig {
    // The default eroded world over a bake of 32 cells; the horizon lattice is coarse (16 cells a chunk edge) so that a map is a small raster.
    let world = Arc::new(WorldTerrain::open(WorldDefinition::single_planet(R, 5), 32).unwrap());
    world.set_horizon_cells(CELLS);
    Rig { world, clock: Arc::new(FakeClock::default()), horizon_jobs: Arc::new(ManualSpawner::default()) }
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

/// One frame of the world: the horizon jobs, run to the end of what they started.
fn frame(r: &Rig) {
    r.world.horizon_frame(&[tile()], camera());
    r.horizon_jobs.run(64);
}

// spec: STRM-049, STRM-064
#[test]
fn a_world_without_a_bake_has_no_horizon_jobs_and_does_nothing() {
    let mut def = WorldDefinition::single_planet(R, 5);
    def.set_height_field(REFERENCE_ID);
    let world = Arc::new(WorldTerrain::open(def, 16).unwrap());
    assert!(!world.has_bake(), "a seed-only world has no bake to cut chunks from");
    let err = world
        .enable_horizon(params(), Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()), HorizonConfig::default(), 2)
        .unwrap_err();
    assert!(err.contains("no chunk lattice"), "{err}");
    assert!(!world.horizon_enabled());
    assert_eq!(world.horizon_frame(&[tile()], camera()), Default::default());
    assert!(world.horizon_map((2, 1000, 1000)).is_none() && world.horizon_stats().is_none());
}

// spec: STRM-049
#[test]
fn a_horizon_reads_the_three_by_three_chunks_around_it_across_face_edges_too() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
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
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let spawn = |r: &Rig| -> (Arc<ManualSpawner>, Arc<FakeClock>) { (r.horizon_jobs.clone(), r.clock.clone()) };
    let (s, c) = spawn(&r);
    let lat = r.world.horizon_lattice().unwrap();
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
    // The default march on the 8 m lattice keeps the range inside a chunk edge.
    let default_lattice = planet_generators::chunkfield::ChunkLattice::new(614).unwrap();
    let p = chunk_horizon_params(&default_lattice, R);
    assert!(p.range_m < 5_000.0 && p.range_m > 4_700.0, "{} m", p.range_m);
    // A world that is gone gives a map of the right length, all zeros, not a short one.
    let gone = {
        let r2 = rig();
        ChunkHorizonBaker::new(&r2.world, r2.world.horizon_lattice().unwrap(), params(), 1)
    };
    let map = gone.bake((2, 1000, 1000), &[]).expect("a map");
    assert_eq!(map.len(), (CELLS * CELLS * 8) as usize);
    assert!(map.iter().all(|&b| b == 0));
}

// spec: STRM-049
#[test]
fn a_chunk_gets_its_horizon_map_from_the_worlds_own_field_and_deterministically() {
    let key: ChunkKey = (2, 1000, 1000);
    let maps: Vec<Vec<u8>> = (0..2)
        .map(|_| {
            let r = rig();
            r.world
                .enable_horizon(
                    params(),
                    r.horizon_jobs.clone(),
                    r.clock.clone(),
                    HorizonConfig { workers: 4, settle_ms: 1_000, keep: 8 },
                    2,
                )
                .unwrap();
            let up = r.world.horizon_frame(&[tile()], camera());
            assert!(up.started >= 1, "the chunks are ready from the base terrain: their horizon jobs start");
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
    r.world
        .enable_horizon(params(), r.horizon_jobs.clone(), r.clock.clone(), HorizonConfig { workers: 2, settle_ms: 1_000, keep: 8 }, 2)
        .unwrap();
    // 100 ms frames for 12 s: the horizon jobs bake the maps of the chunks the plan reads.
    for _ in 0..120 {
        frame(&r);
        r.clock.advance(100);
    }
    let s = r.world.horizon_stats().unwrap();
    let chunks = r.world.horizon_wanted(&[tile()], camera()).len() as u64;
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

fn hill_at(chunk: (u32, u32), id: u128) -> ItemSpec {
    let at = TileId::new(Face(2), CHUNK_GRID_LEVEL, chunk.0, chunk.1).unwrap().center_direction(&TangentWarp);
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: at, content_hash: 0, reach_m: 800.0, max_delta_m: 60.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]))
}

// spec: STRM-059
#[test]
fn a_bake_from_shared_height_tiles_equals_the_bake_that_evaluates_its_whole_raster_and_neighbours_share_the_tiles() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    let east: ChunkKey = (2, 1001, 1000);
    let reads = shared.reads(own);
    assert!(reads.len() == 9, "the rig's chunks around the tile are all ready: {reads:?}");
    let first = shared.bake(own, &[]).expect("a map");
    assert_eq!(first, plain.bake(own, &[]).expect("a map"), "the chunk");
    assert!(first.iter().any(|&b| b > 0), "the relief of the terrain shades something");
    assert_eq!(shared.tile_stats(), planet::horizonworld::TileStats { evaluated: 9, hits: 0, held: 9 });
    // The chunk to the east shares six of its nine tiles with the first: three are new.
    assert_eq!(shared.bake(east, &[]).expect("a map"), plain.bake(east, &[]).expect("a map"), "the neighbour");
    let s = shared.tile_stats();
    assert_eq!((s.evaluated, s.hits), (12, 6), "{s:?}");
    // The same chunk again evaluates nothing.
    assert_eq!(shared.bake(own, &[]).expect("a map"), first);
    assert_eq!(shared.tile_stats().evaluated, 12);
    // A cache that holds fewer tiles than a bake reads still gives the same map, and never holds more than its bound.
    let tight = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(4);
    assert_eq!(tight.bake(own, &[]).expect("a map"), first);
    assert_eq!(tight.tile_stats().held, 4);
}

// spec: STRM-059, STRM-060
#[test]
fn bakes_that_run_at_once_evaluate_a_shared_tile_once_and_give_the_same_maps_as_one_after_the_other() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let keys: Vec<ChunkKey> = (0..4).map(|dx| (2, 1000 + dx, 1000)).collect();
    let alone = ChunkHorizonBaker::new(&r.world, lat, params(), 1);
    let expected: Vec<_> = keys.iter().map(|k| alone.bake(*k, &[]).expect("a map")).collect();
    // Four chunks in a row read 6 x 3 distinct tiles; one after the other that is what is evaluated.
    assert_eq!(alone.tile_stats().evaluated, 18);
    let together = ChunkHorizonBaker::new(&r.world, lat, params(), 1);
    let maps: Vec<_> = std::thread::scope(|s| {
        let jobs: Vec<_> = keys.iter().map(|k| s.spawn(|| together.bake(*k, &[]).expect("a map"))).collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    });
    assert_eq!(maps, expected, "the result does not depend on how many bakes run at once (CON-14)");
    assert_eq!(together.tile_stats().evaluated, 18, "a tile two bakes need at once is evaluated by one of them");
}

// spec: STRM-059
#[test]
fn a_neighbours_edit_replaces_the_tile_it_touches_in_the_halo_and_no_other() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    let landed = shared.bake(own, &[]).expect("a map");
    assert_eq!(landed, plain.bake(own, &[]).expect("a map"));
    assert_eq!(shared.tile_stats().evaluated, 9);
    // An edit inside the chunk to the east, 800 m across and clear of every other chunk of the nine.
    let mut def = (*r.world.definition()).clone();
    def.put_item_spec(BodyId::PLANET, hill_at((1001, 1000), 11)).unwrap();
    r.world.set_definition(Arc::new(def)).unwrap();
    let edited = shared.bake(own, &[]).expect("a map");
    assert_eq!(edited, plain.bake(own, &[]).expect("a map"), "the edit is in the map, as if nothing had been kept");
    assert_ne!(edited, landed, "the hill beside the chunk shades its edge");
    let s = shared.tile_stats();
    // (A tile's key holds the items within the stamp stage's halo of it, so the hill, 2.5 km in, also re-keys the tiles beside the east one;
    // the three of the far west column, 7 km away, are read from the cache.)
    assert!(
        s.evaluated > 9 && s.hits >= 3 && s.evaluated - 9 + s.hits == 9,
        "only the tiles the hill reaches are made again, not the nine: {s:?}"
    );
    // Nothing changes: nothing is made again.
    let again = shared.bake(own, &[]).expect("a map");
    assert_eq!((again, shared.tile_stats().evaluated), (edited, s.evaluated));
}

// spec: STRM-059
#[test]
fn a_change_of_the_lens_makes_every_tile_again_and_the_map_still_equals_the_whole_raster() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let shared = ChunkHorizonBaker::new(&r.world, lat, params(), 2);
    let plain = ChunkHorizonBaker::new(&r.world, lat, params(), 2).with_tile_cap(0);
    let own: ChunkKey = (2, 1000, 1000);
    shared.bake(own, &[]).expect("a map");
    assert_eq!(shared.tile_stats().evaluated, 9);
    // The coupled lens (the field chain does not hash it into a tile key).
    r.world.set_coupled_lens(Some(planet_generators::coupled::CoupledLens::Height));
    assert_eq!(shared.bake(own, &[]).expect("a map"), plain.bake(own, &[]).expect("a map"));
    assert_eq!(shared.tile_stats().evaluated, 18, "a lens makes all nine again");
}

/// A flight of 120 frames of 100 ms east over the chunk of `tile()` through the horizon step, with the features given; what the step counted.
fn fly_through_the_step(features: planet::horizonstep::FrameFeatures) -> planet::horizonstep::FeatureCounters {
    use planet::horizonstep::{frame_step, FeatureCounters};
    let r = rig();
    r.world
        .enable_horizon(params(), r.horizon_jobs.clone(), r.clock.clone(), HorizonConfig { workers: 2, settle_ms: 1_000, keep: 8 }, 2)
        .unwrap();
    let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let (s0, s1, t0, t1) = chunk.bounds();
    let at = |s: f64| PlanetFixed(planet_core::cube::face_to_direction(&TangentWarp, Face(2), s, 0.5 * (t0 + t1)) * (R + 300.0));
    let mut counters = FeatureCounters::default();
    for frame in 0..120u64 {
        // 800 m a second east: the camera crosses a few of the 300 m cells of this rig's chunks.
        let pos = at(s0 + (s1 - s0) * 0.1 + 800.0 * (frame as f64 * 0.1) / (R * std::f64::consts::FRAC_PI_2) * 2.0);
        frame_step(&r.world, &[tile()], pos, &features, &mut counters);
        r.horizon_jobs.run(64);
        r.clock.advance(100);
    }
    counters
}

// spec: TEST-058
#[test]
fn the_horizon_step_counts_its_feature_and_a_feature_turned_off_counts_nothing() {
    use planet::horizonstep::FrameFeatures;
    let on = fly_through_the_step(FrameFeatures::default());
    assert_eq!(on.frames, 120);
    assert!(on.horizon_frames == 120 && on.horizon_landed > 0, "the horizon jobs ran and landed maps: {on:?}");
    // The same flight with the feature off: its counters stay 0.
    let off = fly_through_the_step(FrameFeatures { horizon: false });
    assert_eq!((off.frames, off.horizon_frames, off.horizon_landed), (120, 0, 0), "{off:?}");
}

// spec: STRM-057
#[test]
fn an_edit_changes_the_word_of_the_chunk_beside_it_whose_march_reads_it_and_of_no_chunk_two_edges_away() {
    use planet::horizonworld::ChunkHorizonBaker;
    use planet_streaming::horizonjobs::HorizonBaker;
    // The owner's session of 2026-10-27 asked whether an edit re-bakes the neighbours whose horizon reach it: the word of a chunk is the hash of the
    // items within its march's halo (2 km here, 9 cells), so a hill at the centre of one chunk is in the halo of the chunk beside it (its edge is
    // 2.4 km from the hill) and out of the halo of the chunk after that (7.3 km).
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&r.world, lat, params(), 1);
    let (own, beside, further) = ((2u8, 1000u32, 1000u32), (2u8, 1001u32, 1000u32), (2u8, 1002u32, 1000u32));
    assert!([own, beside, further].iter().all(|k| baker.edit_word(*k) == 0), "no items, no word");
    let centre = TileId::new(Face(2), CHUNK_GRID_LEVEL, own.1, own.2).unwrap().center_direction(&TangentWarp);
    let hill = ItemSpec {
        id: 1,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: centre,
        content_hash: 0,
        reach_m: 800.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
    let mut def = (*r.world.definition()).clone();
    def.put_item_spec(BodyId::PLANET, hill).unwrap();
    r.world.set_definition(Arc::new(def)).unwrap();
    assert!(baker.edit_word(own) != 0, "the chunk the hill stands in");
    assert!(baker.edit_word(beside) != 0, "the chunk beside it, whose march reads the hill: its map is baked again");
    assert_eq!(baker.edit_word(further), 0, "two chunks away: out of every halo, no new bake");
}

// spec: STRM-064
#[test]
fn a_world_with_a_bake_and_no_chunk_stage_wants_and_bakes_horizon_maps_from_the_base_terrain() {
    use planet::horizonworld::chunk_horizon_params;
    // The wiring of the default viewer since the owner's fallback of 2026-10-28: a stored (here in-memory) bake, no chunk stage, the horizon jobs
    // on the base terrain. The streaming tests cover `BaseTerrain` with a fake baker; this is the world's own lattice, wanted set and baker.
    let world = Arc::new(WorldTerrain::open(WorldDefinition::single_planet(R, 5), 32).unwrap());
    assert!(world.has_bake(), "a bake");
    let lat = world.horizon_lattice().expect("a world with a bake has the lattice its maps would be baked on");
    let (spawner, clock) = (Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()));
    world
        .enable_horizon(
            chunk_horizon_params(&lat, R),
            spawner.clone(),
            clock.clone(),
            HorizonConfig { workers: 1, settle_ms: 0, keep: 8 },
            2,
        )
        .expect("the horizon starts without a chunk stage");
    assert!(world.horizon_enabled());
    let chunk = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let camera = PlanetFixed(chunk.center_direction(&TangentWarp) * (R + 300.0));
    let wanted = world.horizon_wanted(&[chunk], camera);
    assert!(wanted.iter().any(|(k, _)| *k == (2, 1000, 1000)), "the chunk of a level-11 tile is wanted: {wanted:?}");
    assert!(wanted.windows(2).all(|w| w[0].1 <= w[1].1), "nearest first");
    world.horizon_frame(&[chunk], camera);
    spawner.run(64);
    let update = world.horizon_frame(&[chunk], camera);
    let stats = world.horizon_stats().expect("stats");
    assert!(stats.baked > 0, "a map was baked from the base terrain: {stats:?}");
    assert!(
        update.landed.len() as u64 + stats.landed > 0 && world.horizon_map((2, 1000, 1000)).is_some(),
        "the map of the tile's own chunk landed"
    );
}

/// Throughput of horizon bakes on the real store, `workers` at once with `threads` each over 12 chunks, cold (the height tiles are evaluated by the
/// bakes) and warm (the march alone). A measurement, not a test of a rule: `cargo test --release -p planet --test it horizon_bake_throughput --
/// --ignored --nocapture`. Numbers depend on what else runs on the machine.
#[test]
#[ignore]
fn horizon_bake_throughput() {
    use planet::horizonworld::{chunk_horizon_params, ChunkHorizonBaker};
    use planet_streaming::horizonjobs::HorizonBaker;
    let store = std::path::Path::new("C:/DEV/SeriousPlanetData/global16384");
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options").seed;
    let world = Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, seed), store, 16384).unwrap());
    let lat = world.horizon_lattice().unwrap();
    let params = chunk_horizon_params(&lat, R);
    let keys: Vec<(u8, u32, u32)> = (0..4u32).flat_map(|x| (0..3u32).map(move |y| (2u8, 1000 + x, 1000 + y))).collect();
    for (workers, threads) in [(1usize, 20usize), (1, 4), (4, 5), (8, 1), (12, 1)] {
        let baker = ChunkHorizonBaker::new(&world, lat, params, threads);
        for label in ["cold", "warm"] {
            let next = std::sync::atomic::AtomicUsize::new(0);
            let t = std::time::Instant::now();
            std::thread::scope(|s| {
                for _ in 0..workers {
                    s.spawn(|| loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(k) = keys.get(i) else { break };
                        baker.bake(*k, &[]).unwrap();
                    });
                }
            });
            println!("workers {workers} x threads {threads} {label}: {:.0} ms for {} maps", t.elapsed().as_secs_f64() * 1e3, keys.len());
        }
    }
}

// spec: STRM-064
#[test]
fn the_horizon_bakes_without_a_chunk_stage_run_on_half_the_generation_workers_at_most_eight() {
    use planet::horizonworld::{horizon_workers, HORIZON_WORKERS_MAX};
    use planet_streaming::GenWorkers;
    assert_eq!([1, 2, 5, 8, 16, 18, 40].map(|n| horizon_workers(GenWorkers::new(n))), [1, 1, 2, 4, 8, 8, HORIZON_WORKERS_MAX]);
}

/// The march of one map on the real store, one thread, tiles cached (`cargo test --release -p planet --test it horizon_march_time -- --ignored
/// --nocapture`): the seconds a map costs, and a hash of its bytes so a change of the march shows whether the bytes moved.
#[test]
#[ignore]
fn horizon_march_time() {
    use planet::horizonworld::{chunk_horizon_params, ChunkHorizonBaker};
    use planet_streaming::horizonjobs::HorizonBaker;
    let store = std::path::Path::new("C:/DEV/SeriousPlanetData/global16384");
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options").seed;
    let world = Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, seed), store, 16384).unwrap());
    let lat = world.horizon_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&world, lat, chunk_horizon_params(&lat, R), 1);
    for key in [(2u8, 1000u32, 1000u32), (2, 1500, 1200)] {
        baker.bake(key, &[]).unwrap();
        for rep in 0..3 {
            let t = std::time::Instant::now();
            let map = baker.bake(key, &[]).unwrap();
            let hash = planet_core::hash::hash_u64s(1, &map.iter().map(|&b| u64::from(b)).collect::<Vec<_>>());
            println!("{key:?} rep {rep}: {:.0} ms, hash {hash:016x}", t.elapsed().as_secs_f64() * 1e3);
        }
    }
}

/// Where the bakes spend their time after an edit, on the real store (`cargo test --release -p planet --test it horizon_bake_split -- --ignored
/// --nocapture`): 12 maps, 8 at once with one thread each, cold, warm, then after hill edits of growing reach in the middle of the 12 chunks.
/// Prints the wall time and the thread-summed milliseconds of the edit words, the field build, the height tiles (look-up, gate wait, evaluation) and
/// the march. A diagnosis: the edit-latency table's 500 m rows against the march's 2 s a map.
#[test]
#[ignore]
fn horizon_bake_split() {
    use planet::horizonworld::{chunk_horizon_params, ChunkHorizonBaker};
    use planet_streaming::horizonjobs::HorizonBaker;
    let store = std::path::Path::new("C:/DEV/SeriousPlanetData/global16384");
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options").seed;
    let world = Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, seed), store, 16384).unwrap());
    let lat = world.horizon_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&world, lat, chunk_horizon_params(&lat, R), 1);
    let keys: Vec<(u8, u32, u32)> = (0..4u32).flat_map(|x| (0..3u32).map(move |y| (2u8, 1000 + x, 1000 + y))).collect();
    let run = |label: &str| {
        let before = baker.bake_times();
        let stats = baker.tile_stats();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let t = std::time::Instant::now();
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(k) = keys.get(i) else { break };
                    baker.bake(*k, &[]).unwrap();
                });
            }
        });
        let (a, b, s) = (baker.bake_times(), before, baker.tile_stats());
        println!(
            "{label:>22}: wall {:6.0} ms | thread-summed ms: words {:7.0} field {:7.0} tiles {:7.0} march {:7.0} | tiles evaluated {} hits {}",
            t.elapsed().as_secs_f64() * 1e3,
            a.words_ms - b.words_ms,
            a.field_ms - b.field_ms,
            a.tiles_ms - b.tiles_ms,
            a.march_ms - b.march_ms,
            s.evaluated - stats.evaluated,
            s.hits - stats.hits,
        );
    };
    run("cold");
    run("warm");
    let at = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1001, 1001).unwrap().center_direction(&TangentWarp);
    let mut def = (*world.definition()).clone();
    for (n, reach) in [150.0f64, 500.0, 2000.0].into_iter().enumerate() {
        let item = ItemSpec {
            id: 100 + n as u128,
            layer: DEFAULT_ITEM_LAYER.into(),
            priority: 0,
            dir: at,
            content_hash: 0,
            reach_m: reach,
            max_delta_m: 60.0,
            stamp: None,
        }
        .with_stamp(Stamp::new(StampKind::Hill, [50.0, 0.0, 0.0]));
        def.put_item_spec(BodyId::PLANET, item).unwrap();
        world.set_definition(Arc::new(def.clone())).unwrap();
        run(&format!("edit reach {reach:.0} m"));
        run("  same, warm");
    }
}

// spec: STRM-059
#[test]
fn a_bake_times_its_steps_and_the_diagnosis_log_holds_it_when_started() {
    use planet::horizonworld::{bake_log_start, bake_log_take, ChunkHorizonBaker};
    use planet_streaming::horizonjobs::HorizonBaker;
    let r = rig();
    let lat = r.world.horizon_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&r.world, lat, params(), 1);
    let key: ChunkKey = (2, 1000, 1000);
    bake_log_start();
    baker.bake(key, &[]).expect("a map");
    let t = baker.bake_times();
    assert_eq!(t.bakes, 1);
    assert!(t.tiles_ms > 0.0 && t.march_ms > 0.0, "{t:?}");
    // Other bakes of the test binary may log too: this baker's is the one of its key with a march.
    let log = bake_log_take();
    let mine = log.iter().find(|b| b.key == key && b.made && b.step_ms[3] > 0.0).expect("the bake is in the log");
    assert!(mine.ended.expect("the bake ended") > mine.started);
    assert!(bake_log_take().is_empty() || bake_log_take().iter().all(|b| b.key != (9, 9, 9)), "the log goes on, emptied");
}

/// How a map's march slows when several run at once, on the real store (`cargo test --release -p planet --test it horizon_march_concurrency -- --ignored
/// --nocapture`): the height tiles of 12 chunks are warmed, then `c` threads of one thread per bake each make 2 maps of their own; the mean milliseconds a
/// map takes at 1, 2, 4 and 7 at once, with the threads at normal and at below-normal priority (the viewer's horizon pool runs below normal, ADR 0046).
/// A measurement, not a rule: it needs a quiet machine.
#[test]
#[ignore]
fn horizon_march_concurrency() {
    use planet::horizonworld::{chunk_horizon_params, ChunkHorizonBaker};
    use planet_streaming::horizonjobs::HorizonBaker;
    use planet_streaming::priority::{set_current, Priority};
    let store = std::path::Path::new("C:/DEV/SeriousPlanetData/global16384");
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options").seed;
    let world = Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, seed), store, 16384).unwrap());
    let lat = world.horizon_lattice().unwrap();
    let baker = ChunkHorizonBaker::new(&world, lat, chunk_horizon_params(&lat, R), 1);
    let keys: Vec<(u8, u32, u32)> = (0..4u32).flat_map(|x| (0..3u32).map(move |y| (2u8, 1000 + x, 1000 + y))).collect();
    for k in &keys {
        baker.bake(*k, &[]).unwrap();
    }
    for priority in [Priority::Normal, Priority::BelowNormal] {
        for c in [1usize, 2, 4, 7] {
            let before = baker.bake_times();
            let t = std::time::Instant::now();
            std::thread::scope(|s| {
                for i in 0..c {
                    let (baker, keys) = (&baker, &keys);
                    s.spawn(move || {
                        let applied = set_current(priority);
                        assert!(applied || cfg!(not(any(windows, target_os = "linux"))), "the priority was not applied");
                        for rep in 0..2 {
                            baker.bake(keys[(i * 2 + rep) % keys.len()], &[]).unwrap();
                        }
                    });
                }
            });
            let after = baker.bake_times();
            println!(
                "{:>12} x {c}: wall {:6.0} ms, march per map {:6.0} ms ({} maps)",
                priority.name(),
                t.elapsed().as_secs_f64() * 1e3,
                (after.march_ms - before.march_ms) / (after.bakes - before.bakes) as f64,
                after.bakes - before.bakes
            );
        }
    }
}
