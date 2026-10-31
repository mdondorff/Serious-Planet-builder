//! The stored coarse horizon of a world (STRM-045) and the coupled views' tile keys (GEN-043): the world-side tests of a store-backed `WorldTerrain`.

use std::sync::Arc;

use planet::coupledworld::Standing;
use planet::worldfield::WorldTerrain;
use planet_core::{Face, TileId};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 32;

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

// spec: GEN-043
#[test]
fn the_coupled_views_key_every_tile_of_a_store_backed_world_with_and_without_the_lens() {
    use planet_core::cube::TangentWarp;
    use planet_generators::coupled::CoupledLens;
    // The owner's session of 2026-10-27: the K views (the run's heights and drainage area) panicked on the pool workers at the mesh key
    // ("a field that reads cells must report its cell queries in read_records_for"): every tile the view re-keys must have a key.
    let store = std::env::temp_dir().join(format!("planet-lens-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let world = open(&store);
    wait_for_run(&world);
    let world = open(&store);
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
    keys(&world, "no lens");
    for lens in [CoupledLens::Height, CoupledLens::Area] {
        world.set_coupled_lens(Some(lens));
        keys(&world, &format!("{lens:?}"));
    }
    let _ = std::fs::remove_dir_all(&store);
}
