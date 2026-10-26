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
