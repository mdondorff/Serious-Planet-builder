//! VIEW-059, VIEW-061, CON-22, tier B: a block of the bake is decoded by the warm-up thread and the hover pick, never by the calling thread.

use std::sync::Arc;

use planet::worldfield::WorldTerrain;
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

// spec: VIEW-059, VIEW-056, CON-22
#[test]
fn a_block_of_the_bake_is_decoded_by_the_warm_up_thread_so_the_first_read_on_the_main_thread_decodes_none() {
    use planet_cache::global_store::thread_block_decodes;
    let store = std::env::temp_dir().join(format!("planet-warm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let world = open(&store);
    let field = world.field().expect("a field");
    // One block per face at 32 cells: a direction on each of two other faces is a block nobody has read.
    let (cold, warmed) = (planet_core::Vec3::new(0.0, 1.0, 0.0), planet_core::Vec3::new(0.0, 0.0, 1.0));
    // Without the warm-up the main thread decodes the block itself.
    let before = thread_block_decodes();
    std::hint::black_box(field.height(cold));
    assert!(thread_block_decodes() > before, "a cold block is decoded by the thread that reads it");
    // With it, the warm-up thread decodes it and the main thread decodes none.
    world.warm_heights(vec![warmed]);
    let started = std::time::Instant::now();
    while world.warmed_batches() < 1 {
        assert!(started.elapsed().as_secs() < 60, "the warm-up did not finish");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let before = thread_block_decodes();
    std::hint::black_box(field.height(warmed));
    assert_eq!(
        thread_block_decodes(),
        before,
        "the block was decoded by the warm-up thread: the main thread read it from the shared cache"
    );
    let _ = std::fs::remove_dir_all(&store);
}

// spec: VIEW-061, CON-22
#[test]
fn the_hover_pick_decodes_its_cold_block_on_its_own_thread_and_the_stall_probe_names_a_phase_that_decodes() {
    use planet::hoverpick::{HoverPicker, PickSource};
    use planet_cache::global_store::thread_block_decodes;
    use planet_frame::pick::{PickLimits, Ray};
    let store = std::env::temp_dir().join(format!("planet-hover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", &RES.to_string(), "--threads", "2"]).unwrap();
    let world = open(&store);
    let radius = world.radius_m();
    // A ray from above a block nobody has read, straight down at it.
    let (cold, other) = (planet_core::Vec3::new(0.0, 1.0, 0.0), planet_core::Vec3::new(0.0, 0.0, -1.0));
    let ray = |dir: planet_core::Vec3| Ray { origin: dir * (radius + 100_000.0), dir: dir * -1.0 };
    let limits = PickLimits::new(radius, 9_000.0);
    let source: PickSource = {
        let world = world.clone();
        std::sync::Arc::new(move |ray, limits, level| {
            let field = world.field().ok()?;
            planet_frame::pick::pick_surface(&ray, &limits, &|d| field.height_at_level(d, level))
        })
    };
    let mut picker = HoverPicker::new(source);
    // The frame polls: the pick is another thread's, so this thread decodes no block, and the pick finds the ground once it has finished.
    let before = thread_block_decodes();
    let mut hit = picker.poll((ray(cold), limits, 5));
    assert_eq!(thread_block_decodes(), before, "the main thread decoded a block for the hover pick");
    let started = std::time::Instant::now();
    while hit.is_none() {
        assert!(started.elapsed().as_secs() < 60, "the hover pick never finished");
        std::thread::sleep(std::time::Duration::from_millis(5));
        hit = picker.poll((ray(cold), limits, 5));
    }
    assert_eq!(thread_block_decodes(), before, "polling the finished pick decoded nothing on this thread");
    // A phase that reads a cold block on the main thread is named with its count by the probe.
    let mut probe = planet::stall::StallProbe::default();
    probe.begin();
    probe.mark("a phase that reads nothing new");
    let field = world.field().expect("a field");
    std::hint::black_box(field.height(other));
    probe.mark("a phase that reads a cold block");
    assert_eq!(probe.main_decodes(), vec![("a phase that reads a cold block", 1)]);
    let _ = std::fs::remove_dir_all(&store);
}
