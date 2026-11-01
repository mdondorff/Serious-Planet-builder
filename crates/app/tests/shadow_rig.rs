//! REND-050, STRM-015, tier A: the clipmap planner runs on the terrain share and never on the frame thread. The first plan of a view
//! (about 400 ms at full size) must not hitch a frame: `ShadowRig::step` only hands inputs in and takes a landed plan out.

use std::sync::Arc;

use planet::shadowrig::{ShadowInputs, ShadowRig};
use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::clipmap::ClipmapParams;
use planet_frame::{residency_set, LodParams, SelectionCache};
use planet_streaming::ManualSpawner;

const R: f64 = 6_371_000.0;

fn resident() -> (Vec<TileId>, PlanetFixed) {
    let lod = LodParams { cells: 16, viewport_h_px: 360.0, tau_px: 4.0, max_level: 12, ..LodParams::earth_1080p() };
    let cam = PlanetFixed(Vec3::new(0.3, 0.5, 0.8).normalized() * (R + 300.0));
    let r = residency_set(&TangentWarp, cam, &lod, &mut SelectionCache::new());
    (r.tiles.iter().map(|e| e.tile).collect(), cam)
}

fn inputs(resident: &[TileId], camera: PlanetFixed, placed: &[TileId]) -> ShadowInputs {
    ShadowInputs { camera, resident: resident.to_vec(), placed: placed.to_vec(), stamps: Vec::new() }
}

// spec: REND-050
#[test]
fn the_planner_never_runs_on_the_frame_thread_and_a_plan_lands_a_frame_after_its_job() {
    let spawner = Arc::new(ManualSpawner::default());
    let mut rig = ShadowRig::new(spawner.clone(), ClipmapParams { texels: 256, ..ClipmapParams::new(R, 16) });
    let (tiles, cam) = resident();
    let frame_thread = std::thread::current().id();

    // Frame 1: the job is queued, nothing is planned here, nothing lands.
    assert!(rig.step(inputs(&tiles, cam, &[])).is_none());
    assert_eq!(spawner.pending(), 1);
    assert!(rig.planning());
    assert!(rig.job_threads().is_empty(), "no plan was made by the frame's call");

    // Frames while the job waits or runs: no second job, no plan.
    for _ in 0..3 {
        assert!(rig.step(inputs(&tiles, cam, &[])).is_none());
    }
    assert_eq!(spawner.pending(), 1, "one job at a time");

    // The terrain share runs the job on another thread.
    let s = spawner.clone();
    std::thread::spawn(move || assert!(s.run_one())).join().unwrap();
    assert_eq!(rig.job_threads().len(), 1);
    assert_ne!(rig.job_threads()[0], frame_thread, "the plan ran off the frame thread");

    // The next frame takes the plan, with the floor tiles of the first plan, and queues the next job.
    let landed = rig.step(inputs(&tiles, cam, &[])).expect("the plan landed");
    assert!(!landed.plan.levels.is_empty());
    assert!(!landed.floor_tiles.is_empty(), "the first plan carries the floor tiles");
    assert_eq!(spawner.pending(), 1);

    // A steady second plan carries no floor tiles (the tile placed during its frame is refreshed by the job after).
    let s = spawner.clone();
    std::thread::spawn(move || assert!(s.run_one())).join().unwrap();
    let steady = rig.step(inputs(&tiles, cam, &[tiles[0]])).expect("the second plan landed");
    assert!(steady.floor_tiles.is_empty(), "the floor did not change");
}
