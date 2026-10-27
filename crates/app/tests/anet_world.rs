//! STRM-046: a world with a stored coupled channel has the A-net chunk stage when it is opened for viewing; one without has none.

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

// spec: STRM-046
#[test]
fn a_stored_channel_gives_the_world_a_chunk_stage_and_a_landed_chunk_is_read_from_disk_after_a_restart() {
    let store = std::env::temp_dir().join(format!("planet-anet-world-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let spawner = Arc::new(ManualSpawner::default());
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };

    // No channel stored yet: no chunk stage, every tile is the base.
    let world = open(&store);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(false));
    assert!(world.chunk_manager().is_none() && world.chunk_view().is_none());

    // The run is made and stored: now the stage comes with the world.
    wait_for_run(&world);
    let world = open(&store);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let manager = world.chunk_manager().expect("the world has chunks");
    let tile = TileId::new(Face(0), 15, 16000, 16000).unwrap();
    let key = manager.reads(tile)[0];
    let camera = planet_core::PlanetFixed(tile.center_direction(&planet_core::cube::TangentWarp) * R);
    world.chunk_frame(&[tile], camera, &[]);
    assert!(spawner.pending() > 0, "a tile that reads a chunk starts its job");
    while !manager.is_ready(key) {
        assert!(spawner.run_one() || manager.in_flight() == 0, "the jobs ran out before the chunk landed");
        world.chunk_frame(&[tile], camera, &[]);
    }
    let first = manager.result_hash(key).expect("landed");
    assert_eq!(manager.stats().disk_hits, 0);
    drop(world);

    // A restart: the chunk is read from the disk cache beside the store, the same result, nothing computed for it.
    let world = open(&store);
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner.clone(), Arc::new(FakeClock::default()), cfg), Ok(true));
    let manager = world.chunk_manager().expect("chunks again");
    while !manager.is_ready(key) {
        world.chunk_frame(&[tile], camera, &[]);
        if !manager.is_ready(key) {
            assert!(spawner.run_one() || manager.in_flight() == 0, "the jobs ran out before the chunk landed");
        }
    }
    assert_eq!(manager.result_hash(key), Some(first), "the stored chunk is the one computed");
    assert!(manager.stats().disk_hits >= 1, "{:?}", manager.stats());
    let _ = std::fs::remove_dir_all(&store);
}

// spec: STRM-046
#[test]
fn a_world_without_a_bake_asks_for_nothing() {
    let world = Arc::new(WorldTerrain::open(planet::worldfield::legacy_world(7, R), RES).unwrap());
    world.request_anet(Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()), ChunkConfig::default());
    assert!(world.chunk_manager().is_none());
}

// spec: STRM-046
#[test]
fn a_base_that_lost_its_world_says_so_and_a_lattice_that_does_not_divide_is_refused() {
    use planet_generators::anet::BaseDetail;
    use planet_generators::coupled::{CoupledMaps, Lattice};
    let world = Arc::new(WorldTerrain::open(planet::worldfield::legacy_world(7, R), RES).unwrap());
    let base = planet::anetworld::WorldBase::new(&world);
    assert!(base.healthy());
    let other = Arc::new(WorldTerrain::open(planet::worldfield::legacy_world(7, R), RES).unwrap());
    assert_eq!(planet::anetworld::WorldBase::new(&other).identity(), base.identity(), "the same world gives the same identity");
    // Another height field over the same seed and radius is another identity.
    let mut def = WorldDefinition::single_planet(R, 7);
    def.set_height_field("cpu-terrain-v1");
    let terrain = Arc::new(WorldTerrain::open(def, RES).unwrap());
    assert_ne!(planet::anetworld::WorldBase::new(&terrain).identity(), base.identity(), "another implementation re-keys the chunks");
    drop(terrain);
    let dir = planet_core::Vec3::new(0.0, 0.0, 1.0);
    drop(other);
    drop(world);
    assert_eq!(base.base_height(dir), 0.0);
    assert!(!base.healthy(), "a result made after the world went is not to be stored");

    // 608 cells per chunk make 1,245,184 per face edge: a run of 96 cells does not cut it evenly.
    let world = Arc::new(WorldTerrain::open(planet::worldfield::legacy_world(7, R), RES).unwrap());
    let lat = Lattice::new(96, R);
    let n = lat.len();
    let maps = Arc::new(CoupledMaps::new(lat, vec![1.0; n], vec![1.0; n], 1));
    let e = world.enable_anet(maps, 8.0, Arc::new(ManualSpawner::default()), Arc::new(FakeClock::default()), ChunkConfig::default());
    assert!(e.unwrap_err().contains("whole number"), "refused with the reason");
    assert!(world.chunk_manager().is_none());
}

// spec: STRM-046
#[test]
fn the_chunk_edge_of_every_cell_size_carries_the_pyramid_the_top_level_reads() {
    use planet::anetworld::ANET_TOP_LEVEL;
    use planet_generators::chunkfield::{pyramid_levels, ChunkLattice};
    for cell_m in [4.0, 8.0, 10.0, 60.0] {
        let per = planet::anetworld::cells_per_chunk_for(R, cell_m);
        let lat = ChunkLattice::new(per).unwrap();
        let k = pyramid_levels(&lat, R, ANET_TOP_LEVEL);
        assert_eq!(per as usize % (1 << k), 0, "{per} cells per chunk carry {k} pyramid levels at {cell_m} m");
        assert!((lat.cell_m(R) / cell_m - 1.0).abs() < 0.03, "the cell stays near {cell_m} m: {}", lat.cell_m(R));
    }
}

// spec: STRM-046
#[test]
fn the_plan_bound_taken_before_the_chunk_stage_refuses_after_it_and_the_stage_marks_the_change() {
    use planet_core::cube::{FaceMapping, TangentWarp};
    use planet_core::{PlanetFixed, Vec3};
    use planet_frame::{plan_terrain_cached, Camera, LodParams, SelectionCache, TerrainRequest, TerrainView};
    let store = std::env::temp_dir().join(format!("planet-anet-bound-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let world = open(&store);
    wait_for_run(&world);
    let world = open(&store);
    let before = world.field_stage();
    let (bound_before, bound_after) = {
        let field = world.field().unwrap();
        let bound = field.max_abs_height_m();
        assert_eq!(
            world.enable_anet_from_store(
                CELL_M,
                Arc::new(ManualSpawner::default()),
                Arc::new(FakeClock::default()),
                ChunkConfig::default()
            ),
            Ok(true)
        );
        // The window keeps `field` and the bound it took; the stage coming in changes what the world's field is built from.
        assert_ne!(world.field_stage(), before, "the stage's arrival is visible to a viewer that keeps a field");
        (bound, world.field().unwrap().max_abs_height_m())
    };
    assert!(bound_after > bound_before, "the stage lifts the field's bound: {bound_before} to {bound_after}");

    let field = world.field().unwrap();
    let fov = std::f64::consts::FRAC_PI_3;
    let camera = Camera::looking_at_centre(PlanetFixed(Vec3::new(0.0, 0.0, 2.0 * R)), Vec3::new(1.0, 0.0, 0.0), fov, 16.0 / 9.0, 100.0);
    let plan = |max_height_m: f64| {
        let lod = LodParams { max_height_m, ..LodParams::earth_1080p() };
        let req = TerrainRequest {
            camera,
            size: (1280, 720),
            view: TerrainView::Level,
            lod,
            face_mapping: FaceMapping::id(&TangentWarp).to_string(),
            seed: field.seed(),
        };
        plan_terrain_cached(&req, field.as_ref(), &mut SelectionCache::new())
    };
    let refused = plan(bound_before).unwrap_err().to_string();
    assert!(refused.contains("exceed max_height_m"), "{refused}");
    plan(bound_after).expect("the bound taken from the field it plans with plans");
    let _ = std::fs::remove_dir_all(&store);
}

// spec: STRM-045
#[test]
fn the_stored_coarse_horizon_is_read_on_a_thread_and_a_world_without_one_says_how_to_make_it() {
    use planet::horizonworld::CoarseStanding;
    let store = std::env::temp_dir().join(format!("planet-coarse-world-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let wait = |world: &Arc<WorldTerrain>| {
        world.request_coarse_horizon();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            match world.coarse_standing() {
                CoarseStanding::Idle | CoarseStanding::Working => {
                    assert!(std::time::Instant::now() < deadline, "the coarse horizon did not end");
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                s => return s,
            }
        }
    };
    // Nothing stored: absent, with the line that says how to make it, and the frame path was never asked to wait.
    let world = open(&store);
    match wait(&world) {
        CoarseStanding::Absent(text) => {
            assert!(text.contains("generate --coarse-horizon") && text.contains("chunk horizon alone"), "{text}")
        }
        s => panic!("expected Absent, got {s:?}"),
    }
    // Making it needs the stored coupled run: without one the step names it.
    let err = world.make_coarse_horizon(2).expect_err("no coupled run yet");
    assert!(err.contains("no stored coupled run"), "{err}");
    wait_for_run(&world);
    let records = world.make_coarse_horizon(2).expect("the horizon is made");
    assert!(records >= 6, "{records} records");
    // A fresh world over the same store reads it back: observer lattice of the run size, 16 azimuths, as many records as the layout says.
    let again = open(&store);
    match wait(&again) {
        CoarseStanding::Ready(h) => {
            assert_eq!((h.m, h.azimuths), (RES, 16));
            assert_eq!(
                h.bytes.len(),
                6 * (RES as usize) * (RES as usize) * planet_generators::coarsehorizon::record_len(16),
                "a record a cell: two horizons and the heights"
            );
        }
        s => panic!("expected Ready, got {s:?}"),
    }
}

// spec: STRM-045
#[test]
fn generate_coarse_horizon_makes_the_stored_horizon_from_the_stored_coupled_run_and_says_what_it_needs() {
    use planet::horizonworld::CoarseStanding;
    let store = std::env::temp_dir().join(format!("planet-coarse-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    let dir = store.to_str().unwrap();
    run(&["generate", "--global-dir", dir, "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    // Without a stored coupled run the command names it; without --global-res it says what is missing.
    let err = run(&["generate", "--coarse-horizon", dir, "--global-res", &RES.to_string(), "--seed", &seed().to_string()]).unwrap_err();
    assert!(err.contains("no stored coupled run"), "{err}");
    assert!(run(&["generate", "--coarse-horizon", dir]).unwrap_err().contains("--global-res"));
    assert!(run(&["generate", "--coarse-horizon", dir, "--global-res", "32", "--plan-check", dir]).unwrap_err().contains("--plan-check"));
    wait_for_run(&open(&store));
    let text =
        run(&["generate", "--coarse-horizon", dir, "--global-res", &RES.to_string(), "--seed", &seed().to_string(), "--threads", "2"])
            .unwrap();
    assert!(text.contains("marching 16 azimuths") && text.contains("records stored"), "{text}");
    let world = open(&store);
    world.request_coarse_horizon();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        match world.coarse_standing() {
            CoarseStanding::Ready(h) => {
                assert_eq!((h.m, h.azimuths), (RES, 16));
                break;
            }
            CoarseStanding::Idle | CoarseStanding::Working => {
                assert!(std::time::Instant::now() < deadline, "the coarse horizon did not end");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            s => panic!("{s:?}"),
        }
    }
}

// spec: GEN-043, STRM-046
#[test]
fn the_coupled_views_key_every_tile_of_a_store_backed_world_with_and_without_the_chunk_stage() {
    use planet_core::cube::TangentWarp;
    use planet_generators::coupled::CoupledLens;
    // The owner's session of 2026-10-27: the K views (the run's heights and drainage area) panicked on the pool workers at the mesh key
    // ("a field that reads cells must report its cell queries in read_records_for"): every tile the view re-keys must have a key.
    let store = std::env::temp_dir().join(format!("planet-anet-lens-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let world = open(&store);
    wait_for_run(&world);
    let world = open(&store);
    let spawner = Arc::new(ManualSpawner::default());
    let tiles: Vec<TileId> = [0u8, 3, 6, 9, 12, 15]
        .iter()
        .flat_map(|&level| {
            let n = 1u32 << level;
            [(0u8, n / 2, n / 2), (2, 0, 0), (4, n - 1, n / 3)].map(|(face, x, y)| TileId::new(Face(face), level, x, y).unwrap())
        })
        .collect();
    let keys = |world: &Arc<WorldTerrain>, what: &str| {
        let field = world.field().expect("the field");
        for tile in &tiles {
            let key = planet_cache::key::mesh_key(field.as_ref(), &TangentWarp, *tile, 16, R);
            assert!(key.0 != [0u8; 16], "{what}: a key for {tile:?}");
        }
    };
    keys(&world, "no lens, no chunk stage");
    for lens in [CoupledLens::Height, CoupledLens::Area] {
        world.set_coupled_lens(Some(lens));
        keys(&world, &format!("{lens:?}, no chunk stage"));
    }
    world.set_coupled_lens(None);
    let cfg = ChunkConfig { workers: 2, ..ChunkConfig::default() };
    assert_eq!(world.enable_anet_from_store(CELL_M, spawner, Arc::new(FakeClock::default()), cfg), Ok(true));
    keys(&world, "no lens, chunk stage");
    for lens in [CoupledLens::Height, CoupledLens::Area] {
        world.set_coupled_lens(Some(lens));
        keys(&world, &format!("{lens:?}, chunk stage"));
    }
    let _ = std::fs::remove_dir_all(&store);
}
