//! GEN-082, STRM-056, tier B: an edit in a chunk's read set re-solves the chunk to the bits of a cold solve of the same definition, and an edit
//! outside it starts no job. Two stores of 32 cells with a coupled run each and two chunk solves: about 30 s, so it lives outside the fast tier.

use std::sync::Arc;

use planet::coupledworld::Standing;
use planet::worldfield::WorldTerrain;
use planet_core::{Face, TileId};
use planet_streaming::chunks::ChunkConfig;
use planet_streaming::{FakeClock, ManualSpawner};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 32;
/// Coarse chunks (about 80 cells an edge) keep the test quick; the default is 8 m.
const CELL_M: f64 = 60.0;

/// The reuse counters are process-wide: the tests that read them run one at a time.
static REUSE_COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn seed() -> u64 {
    planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).expect("options").seed
}

fn open(store: &std::path::Path) -> Arc<WorldTerrain> {
    Arc::new(WorldTerrain::open_store(WorldDefinition::single_planet(R, seed()), store, RES).expect("the store opens"))
}

fn wait_for_run(world: &Arc<WorldTerrain>) {
    world.request_coupled();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        match world.coupled_standing() {
            Standing::Ready(..) => return,
            Standing::Failed(e) => panic!("{e}"),
            _ => {
                assert!(std::time::Instant::now() < deadline, "the coupled run did not end");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
}

fn store_with_run(name: &str) -> std::path::PathBuf {
    let store = std::env::temp_dir().join(format!("planet-anet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    wait_for_run(&open(&store));
    store
}

fn hill_at(dir: planet_core::Vec3, id: u128) -> planet_world_def::ItemSpec {
    use planet_world_def::stamp::{Stamp, StampKind};
    planet_world_def::ItemSpec {
        id,
        layer: "stamps".into(),
        priority: 0,
        dir,
        content_hash: 0,
        reach_m: 3_000.0,
        max_delta_m: 60.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [40.0, 0.0, 0.0]))
}

/// Land the chunk `tile` reads first on `world` and return its key and result hash.
fn land(world: &Arc<WorldTerrain>, spawner: &ManualSpawner, tile: TileId) -> (planet_generators::chunkfield::ChunkKey, u64) {
    let manager = world.chunk_manager().expect("chunks");
    let key = manager.reads(tile)[0];
    let camera = planet_core::PlanetFixed(tile.center_direction(&planet_core::cube::TangentWarp) * R);
    world.chunk_frame(&[tile], camera, &[]);
    while manager.result_hash(key).is_none() || manager.in_flight() > 0 {
        spawner.run_one();
        world.chunk_frame(&[tile], camera, &[]);
    }
    (key, manager.result_hash(key).expect("landed"))
}

// spec: GEN-082, STRM-056
#[test]
fn an_edit_in_the_read_set_re_solves_the_chunk_to_the_bits_of_a_cold_solve_and_an_edit_outside_it_changes_nothing() {
    let _serial = REUSE_COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let (a, b) = (store_with_run("resolve-a"), store_with_run("resolve-b"));
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let tile = TileId::new(Face(0), 15, 16000, 16000).unwrap();
    let centre = tile.center_direction(&planet_core::cube::TangentWarp);

    // World A: the chunk is solved plain, then a hill is put in its halo.
    let world = open(&a);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let (key, plain) = land(&world, &spawner, tile);
    // A hill on the far side of the planet: outside every read set, so nothing is stale and nothing starts.
    let mut def = (*world.definition()).clone();
    def.put_item_spec(planet_world_def::BodyId::PLANET, hill_at(-centre, 1)).unwrap();
    world.set_definition(Arc::new(def.clone())).unwrap();
    let camera = planet_core::PlanetFixed(centre * R);
    world.chunk_frame(&[tile], camera, &[]);
    assert_eq!(spawner.pending(), 0, "an edit outside the read set starts no job");
    // The hill at the chunk: stale, re-solved when wanted, and the old result keeps drawing until the new lands.
    def.put_item_spec(planet_world_def::BodyId::PLANET, hill_at(centre, 2)).unwrap();
    world.set_definition(Arc::new(def.clone())).unwrap();
    let manager = world.chunk_manager().unwrap();
    world.chunk_frame(&[tile], camera, &[]);
    assert!(manager.in_flight() >= 1, "the chunks the hill reaches are re-solved (the tile reads its neighbours too)");
    assert_eq!(manager.result_hash(key), Some(plain), "the old result draws meanwhile");
    while manager.in_flight() > 0 {
        spawner.run_one();
        world.chunk_frame(&[tile], camera, &[]);
    }
    let resolved = manager.result_hash(key).expect("landed");
    assert_ne!(resolved, plain, "the hill changed the chunk");

    // World B: the same definition from the start, a cold solve of the same version: the same bits.
    let cold_world = Arc::new(WorldTerrain::open_store(def, &b, RES).expect("the store opens"));
    let spawner_b = Arc::new(ManualSpawner::default());
    assert_eq!(cold_world.enable_anet_from_store(CELL_M, spawner_b.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let (_, cold) = land(&cold_world, &spawner_b, tile);
    assert_eq!(cold, resolved, "a re-solve after an edit equals a cold solve of the new version");
    let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
}

/// Re-solve `tile` on `world` after its definition changed to `def`, until nothing is in flight, and return the result hash of the chunk.
fn resolve(world: &Arc<WorldTerrain>, spawner: &ManualSpawner, tile: TileId, def: &WorldDefinition) -> u64 {
    let manager = world.chunk_manager().expect("chunks");
    let key = manager.reads(tile)[0];
    let camera = planet_core::PlanetFixed(tile.center_direction(&planet_core::cube::TangentWarp) * R);
    world.set_definition(Arc::new(def.clone())).unwrap();
    world.chunk_frame(&[tile], camera, &[]);
    while manager.in_flight() > 0 {
        spawner.run_one();
        world.chunk_frame(&[tile], camera, &[]);
    }
    manager.result_hash(key).expect("landed")
}

// spec: GEN-087, GEN-082
#[test]
fn a_hill_put_moved_and_taken_away_re_solves_to_the_bits_of_a_cold_solve_while_the_heights_far_from_it_are_reused() {
    let _serial = REUSE_COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let (a, b) = (store_with_run("reuse-a"), store_with_run("reuse-b"));
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let tile = TileId::new(Face(0), 15, 16000, 16000).unwrap();
    let centre = tile.center_direction(&planet_core::cube::TangentWarp);
    // A second place a little off the first: the moved hill leaves the old place and takes the new one.
    let moved = (centre + planet_core::Vec3::new(0.0, 0.0, 1.0) * (1_500.0 / R)).normalized();

    let world = open(&a);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let (_, plain) = land(&world, &spawner, tile);
    let base_def = (*world.definition()).clone();
    let cold = |def: &WorldDefinition| -> u64 {
        let w = Arc::new(WorldTerrain::open_store(def.clone(), &b, RES).expect("the store opens"));
        let s = Arc::new(ManualSpawner::default());
        assert_eq!(w.enable_anet_from_store(CELL_M, s.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
        land(&w, &s, tile).1
    };
    let _ = planet::regionheights::reuse_stats();

    // Put.
    let mut put = base_def.clone();
    put.put_item_spec(planet_world_def::BodyId::PLANET, hill_at(centre, 7)).unwrap();
    let after_put = resolve(&world, &spawner, tile, &put);
    assert_ne!(after_put, plain, "the hill changed the chunk");
    assert_eq!(after_put, cold(&put), "put: a re-solve equals a cold solve");
    let (reused, asked) = planet::regionheights::reuse_stats();
    assert!(asked > 0 && reused > 0, "put: no height was reused ({reused} of {asked})");

    // Moved: the same item id at another place.
    let mut mv = put.clone();
    mv.put_item_spec(planet_world_def::BodyId::PLANET, hill_at(moved, 7)).unwrap();
    let after_move = resolve(&world, &spawner, tile, &mv);
    assert_eq!(after_move, cold(&mv), "moved: a re-solve equals a cold solve");
    let (reused, asked) = planet::regionheights::reuse_stats();
    assert!(asked > 0 && reused > 0, "moved: no height was reused ({reused} of {asked})");

    // Taken away: the first result again.
    let after_remove = resolve(&world, &spawner, tile, &base_def);
    assert_eq!(after_remove, plain, "removed: the chunk is what it was before the hill");
    let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
}

/// A brush item: one raise stroke of `points` points `spacing_m` apart running east from `dir`, the reach holding the stroke and its radius.
fn brush_at(dir: planet_core::Vec3, id: u128, points: usize, spacing_m: f64, radius_m: f64) -> planet_world_def::ItemSpec {
    use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
    let north = planet_core::Vec3::new(0.0, 0.0, 1.0);
    let east = north.cross(dir).normalized();
    let pts: Vec<planet_core::Vec3> = (0..points)
        .map(|k| {
            let along = k as f64 * spacing_m / R;
            (dir * libm::cos(along) + east * libm::sin(along)).normalized()
        })
        .collect();
    let reach_m = (points - 1) as f64 * spacing_m + radius_m + 2.0;
    planet_world_def::ItemSpec {
        id,
        layer: "brush".into(),
        priority: 0,
        dir: pts[0],
        content_hash: 0,
        reach_m,
        max_delta_m: 30.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&[Stroke {
        kind: BrushKind::Raise,
        radius_m,
        strength_m: 30.0,
        target_m: None,
        points: pts,
    }]))
}

// spec: GEN-087, GEN-082
#[test]
fn a_brush_put_extended_and_taken_away_re_solves_to_the_bits_of_a_cold_solve_while_the_heights_beside_its_stroke_are_reused() {
    let _serial = REUSE_COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let (a, b) = (store_with_run("brush-a"), store_with_run("brush-b"));
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let tile = TileId::new(Face(0), 15, 16000, 16000).unwrap();
    let centre = tile.center_direction(&planet_core::cube::TangentWarp);
    let start = (centre - planet_core::Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized() * (1_000.0 / R)).normalized();

    let world = open(&a);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let (_, plain) = land(&world, &spawner, tile);
    let base_def = (*world.definition()).clone();
    let cold = |def: &WorldDefinition| -> u64 {
        let w = Arc::new(WorldTerrain::open_store(def.clone(), &b, RES).expect("the store opens"));
        let s = Arc::new(ManualSpawner::default());
        assert_eq!(w.enable_anet_from_store(CELL_M, s.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
        land(&w, &s, tile).1
    };
    let _ = planet::regionheights::reuse_stats();

    // Put: a stroke of 5 points 400 m apart (1.6 km) with a radius of 300 m, through the chunk.
    let mut put = base_def.clone();
    put.put_item_spec(planet_world_def::BodyId::PLANET, brush_at(start, 9, 5, 400.0, 300.0)).unwrap();
    let after_put = resolve(&world, &spawner, tile, &put);
    assert_ne!(after_put, plain, "the brush changed the chunk");
    assert_eq!(after_put, cold(&put), "put: a re-solve equals a cold solve");
    let (reused, asked) = planet::regionheights::reuse_stats();
    assert!(
        asked > 0 && reused * 10 >= asked * 8,
        "put: the heights beside the stroke (inside its reach disc) are reused, at least 80 % ({reused} of {asked})"
    );

    // Extended: the same item id with more points (the item's record changes: both the old and the new stroke are stale).
    let mut ext = put.clone();
    ext.put_item_spec(planet_world_def::BodyId::PLANET, brush_at(start, 9, 8, 400.0, 300.0)).unwrap();
    let after_ext = resolve(&world, &spawner, tile, &ext);
    assert_eq!(after_ext, cold(&ext), "extended: a re-solve equals a cold solve");
    let (reused, asked) = planet::regionheights::reuse_stats();
    assert!(asked > 0 && reused > 0, "extended: no height was reused ({reused} of {asked})");

    // Taken away: the first result again.
    let after_remove = resolve(&world, &spawner, tile, &base_def);
    assert_eq!(after_remove, plain, "removed: the chunk is what it was before the brush");
    let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
}

/// A brush item of mixed strokes: a raise, a smooth and a flatten with a target, and a one-point dab, as one drag of the editor makes them.
fn mixed_brush_at(dir: planet_core::Vec3, id: u128, layer: &str, flip: bool) -> planet_world_def::ItemSpec {
    use planet_world_def::stamp::{BrushKind, Stamp, StampKind, Stroke};
    let north = planet_core::Vec3::new(0.0, 0.0, 1.0);
    let east = north.cross(dir).normalized();
    let pt = |m: f64| {
        let along = m / R;
        (dir * libm::cos(along) + east * libm::sin(along)).normalized()
    };
    let stroke =
        |kind, radius_m, strength_m, target_m, a: f64, b: f64| Stroke { kind, radius_m, strength_m, target_m, points: vec![pt(a), pt(b)] };
    let (k0, k1) = if flip { (BrushKind::Lower, BrushKind::Raise) } else { (BrushKind::Raise, BrushKind::Lower) };
    let strokes = [
        stroke(k0, 300.0, 25.0, None, 0.0, 600.0),
        stroke(BrushKind::Smooth, 350.0, 15.0, None, 500.0, 1_100.0),
        stroke(BrushKind::Flatten, 250.0, 20.0, Some(5.0), 1_000.0, 1_500.0),
        stroke(k1, 200.0, 10.0, None, 1_500.0, 1_500.0 + 400.0),
        Stroke { kind: BrushKind::Raise, radius_m: 150.0, strength_m: 10.0, target_m: None, points: vec![pt(1_800.0)] },
    ];
    let reach_m = 1_900.0 + 350.0 + 10.0;
    planet_world_def::ItemSpec {
        id,
        layer: layer.into(),
        priority: 0,
        dir: strokes[0].points[0],
        content_hash: 0,
        reach_m,
        max_delta_m: 90.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(&strokes))
}

// spec: GEN-087, GEN-082
#[test]
fn brushes_of_every_kind_and_a_second_smoothing_brush_beside_them_re_solve_to_the_bits_of_a_cold_solve() {
    let _serial = REUSE_COUNTERS.lock().unwrap_or_else(|e| e.into_inner());
    let (a, b) = (store_with_run("mixed-a"), store_with_run("mixed-b"));
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let tile = TileId::new(Face(0), 15, 16000, 16000).unwrap();
    let centre = tile.center_direction(&planet_core::cube::TangentWarp);
    let start = (centre - planet_core::Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized() * (1_000.0 / R)).normalized();
    let world = open(&a);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let (_, plain) = land(&world, &spawner, tile);
    let base_def = (*world.definition()).clone();
    let cold = |def: &WorldDefinition| -> u64 {
        let w = Arc::new(WorldTerrain::open_store(def.clone(), &b, RES).expect("the store opens"));
        let s = Arc::new(ManualSpawner::default());
        assert_eq!(w.enable_anet_from_store(CELL_M, s.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
        land(&w, &s, tile).1
    };
    // An unchanged smoothing brush of another layer overlaps the one that is edited: the smooth reads the base on a ring, and the running state
    // of the layers must not leak into the cells beside the edited strokes.
    let mut with_other = base_def.clone();
    with_other.put_item_spec(planet_world_def::BodyId::PLANET, mixed_brush_at(start, 20, "other", true)).unwrap();
    let after_other = resolve(&world, &spawner, tile, &with_other);
    assert_eq!(after_other, cold(&with_other), "the first brush: a re-solve equals a cold solve");
    let mut put = with_other.clone();
    put.put_item_spec(planet_world_def::BodyId::PLANET, mixed_brush_at(start, 21, "brush", false)).unwrap();
    let after_put = resolve(&world, &spawner, tile, &put);
    assert_ne!(after_put, after_other, "the second brush changed the chunk");
    assert_eq!(after_put, cold(&put), "put: a re-solve equals a cold solve");
    let mut flipped = put.clone();
    flipped.put_item_spec(planet_world_def::BodyId::PLANET, mixed_brush_at(start, 21, "brush", true)).unwrap();
    let after_flip = resolve(&world, &spawner, tile, &flipped);
    assert_eq!(after_flip, cold(&flipped), "changed strokes: a re-solve equals a cold solve");
    let after_remove = resolve(&world, &spawner, tile, &base_def);
    assert_eq!(after_remove, plain, "removed: the chunk is what it was");
    let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
}

// spec: STRM-056, VIEW-056
#[test]
fn the_chunk_step_over_new_chunks_with_an_item_in_a_stamp_layer_never_holds_the_calling_thread() {
    use planet_core::cube::TangentWarp;
    use planet_core::Side::{East, North, South, West};
    let a = store_with_run("main-thread");
    // The viewer's cells (8 m): the read set of a chunk is then a few hundred milliseconds of work, which the update must not do on its thread.
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    let spawner = Arc::new(ManualSpawner::default());
    let world = open(&a);
    assert_eq!(
        world.enable_anet_from_store(planet::anetworld::ANET_CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg),
        Ok(true)
    );
    let site = TileId::new(Face(0), 15, 16000, 16000).unwrap().center_direction(&TangentWarp);
    let mut def = (*world.definition()).clone();
    def.put_item_spec(planet_world_def::BodyId::PLANET, hill_at(site, 1)).unwrap();
    world.set_definition(Arc::new(def)).unwrap();
    // The jobs never run (a manual spawner nobody drives): what is timed is what the update itself does on the calling thread.
    let east = planet_core::Vec3::new(0.0, 0.0, 1.0).cross(site).normalized();
    let mut slowest = 0.0f64;
    for k in 0..6 {
        let along = f64::from(k) * 5_000.0 / R;
        let dir = (site * libm::cos(along) + east * libm::sin(along)).normalized();
        let c = TileId::from_direction(&TangentWarp, dir, 15);
        let mut tiles = vec![c];
        tiles.extend([East, West, North, South].map(|s| c.neighbor(&TangentWarp, s)));
        let camera = planet_core::PlanetFixed(dir * R);
        let started = std::time::Instant::now();
        world.chunk_frame(&tiles, camera, &[]);
        slowest = slowest.max(started.elapsed().as_secs_f64() * 1000.0);
    }
    assert!(
        slowest < 100.0,
        "a chunk frame over new chunks took {slowest:.0} ms on the calling thread (the limit of an editing frame is 100 ms)"
    );
    assert!(spawner.pending() > 0, "the new chunks were started");
    let _ = std::fs::remove_dir_all(&a);
}
