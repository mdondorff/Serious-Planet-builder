//! Tier C, TEST-070: the lit path over streamed tiles draws the same pixels for a still camera, and the same pixels after a small turn and back. The coast
//! view at 2 km of the lit scenes over the in-memory bake, drawn by the streaming loop (the tiles arrive over frames, the rig of the streaming scenes with
//! the lit path), water time held, exposure at its target (read back). Once the loop is exact and idle, 6 consecutive frames must be pixel-identical; then
//! the camera turns 2 degrees and back, the loop settles again, the exposure returns to its target, and the frame must equal the one before the turn.
//! A change names the pixels, their box and the drawn tiles that differ (a tile that changed level or was dropped is in that list) and writes the frames
//! and the diff to `target/review/still-flicker/`. Adapter: the software adapter (WARP on Windows, lavapipe on Linux), the compact tile format.

use super::gpu_still_flicker::changes;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::repo_root;
use std::collections::BTreeSet;

const FRAMES: usize = 6;
const SIZE: (u32, u32) = (256, 144);
const TURN_DEG: f64 = 2.0;

// spec: TEST-070
#[test]
fn the_lit_coast_view_over_streamed_tiles_draws_the_same_pixels_still_and_after_a_small_turn_and_back() {
    let args: Vec<String> = [
        "test-render",
        "--perf-smoke",
        "--scene",
        "lit",
        "--adapter",
        "software",
        // The software rasteriser takes every core of a small runner; below-normal generation workers then never finish a job.
        "--gen-priority",
        "normal",
        "--global-res",
        "128",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let o = planet::parse_args(&args).expect("options");
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let run = planet::streamperf::lit_stream_stills(&o, &ctx, SIZE, FRAMES, TURN_DEG).expect("the streamed lit stills");
    assert_eq!(run.still.len(), FRAMES);
    // The test can only falsify if there is a sea, a picture, tiles and an exposure that stands still.
    assert!(run.water_nodes > 0, "no plan node carries water: the coast view proves nothing");
    assert!(run.still[0].nodes.len() > 4, "only {} tiles were drawn", run.still[0].nodes.len());
    let first = planet_testkit::Image::new(run.still[0].width, run.still[0].height, run.still[0].rgba.clone());
    let dir = repo_root().join("target").join("review").join("still-flicker");
    planet_testkit::write_png(&dir.join("lit-stream-first.png"), &first).unwrap();
    let colours: BTreeSet<[u8; 4]> = first.pixels().collect();
    assert!(colours.len() > 16, "the lit frame has {} colours: nothing was drawn", colours.len());
    assert!(run.lit_share > 0.5, "only {:.0} % of the lit target carries light: a night or nothing drawn", run.lit_share * 100.0);
    assert!(
        (run.exposure_ev100 - run.exposure_target_ev100).abs() < 1e-6,
        "the exposure was still adapting ({} of {})",
        run.exposure_ev100,
        run.exposure_target_ev100
    );
    eprintln!(
        "settled in {} ticks, exposure {:.2} EV reached in {} one-second steps, {} tiles, {} water nodes; turn {} deg and back settled in {} ticks",
        run.settle_ticks,
        run.exposure_ev100,
        run.exposure_steps,
        run.still[0].nodes.len(),
        run.water_nodes,
        run.turn_deg,
        run.turn_settle_ticks
    );
    assert_ne!(run.still[0].rgba, run.turned.rgba, "the turned view drew the same picture as the held one: the turn proved nothing");
    let mut failures = changes("lit-stream-still", &run.still, &dir);
    let before_turn = run.still.last().expect("frames").clone();
    failures.extend(changes("lit-stream-turn", &[before_turn, run.after_turn], &dir));
    assert!(failures.is_empty(), "the lit picture over streamed tiles changed (images in {}): {}", dir.display(), failures.join(" | "));
}
