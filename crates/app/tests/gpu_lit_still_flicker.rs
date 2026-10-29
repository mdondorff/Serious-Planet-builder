//! Tier C, TEST-069: the lit path (atmosphere, water with the sun glint, adaptive exposure, the terrain debug overlay) draws the same pixels frame after
//! frame for a still camera. The coast view at 2 km of the lit scenes over the in-memory bake with a sea, 6 consecutive frames of one plan at one water
//! time, exposure adapted to its target (read back and checked, not hidden); the cases are the plain view, the sun ahead (the glint report) and the
//! overlay on the tiles. A change names the pixels, their box and the tiles and writes the frames and the diff to `target/review/still-flicker/`.
//! Adapter: the software adapter (WARP on Windows, lavapipe on Linux), the compact tile format (the only format; the Tracked WARP item is a
//! device removal that these frames did not show).

use super::gpu_still_flicker::changes;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::repo_root;

const FRAMES: usize = 6;
const SIZE: (u32, u32) = (256, 144);

// spec: TEST-069
#[test]
fn the_lit_coast_view_draws_the_same_pixels_frame_after_frame_with_the_exposure_adapted_the_sun_ahead_and_the_overlay_on() {
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
    let runs = planet::litperf::lit_still_frames(&o, &ctx, SIZE, FRAMES).expect("the lit stills");
    assert_eq!(runs.len(), 3);
    let dir = repo_root().join("target").join("review").join("still-flicker");
    let mut failures = Vec::new();
    for run in &runs {
        assert_eq!(run.frames.len(), FRAMES);
        // The test can only falsify if there is a sea to glint on, a picture, and an exposure that stands still.
        assert!(run.water_nodes > 0, "{}: no plan node carries water: the coast view proves nothing", run.name);
        assert!(run.frames[0].nodes.len() > 4, "{}: only {} tiles were drawn", run.name, run.frames[0].nodes.len());
        // A picture to look at, and the guards that equality means something: colours in it, light in the lit target (a night is all zeros and a black
        // picture, and an empty exposure histogram that holds the EV where it started).
        let first = planet_testkit::Image::new(run.frames[0].width, run.frames[0].height, run.frames[0].rgba.clone());
        planet_testkit::write_png(&dir.join(format!("{}-first.png", run.name)), &first).unwrap();
        let colours: std::collections::BTreeSet<[u8; 4]> = first.pixels().collect();
        assert!(colours.len() > 16, "{}: the lit frame has {} colours: nothing was drawn", run.name, colours.len());
        assert!(
            run.lit_share > 0.5,
            "{}: only {:.0} % of the lit target carries light: a night or nothing drawn",
            run.name,
            run.lit_share * 100.0
        );
        assert!(
            (run.exposure_ev100 - run.exposure_target_ev100).abs() < 1e-6,
            "{}: the exposure was still adapting ({} of {})",
            run.name,
            run.exposure_ev100,
            run.exposure_target_ev100
        );
        eprintln!(
            "{}: exposure {:.2} EV reached in {} one-second steps, {} water nodes, {} tiles",
            run.name,
            run.exposure_ev100,
            run.warm_frames,
            run.water_nodes,
            run.frames[0].nodes.len()
        );
        failures.extend(changes(&run.name, &run.frames, &dir));
    }
    // The plain view and the sun-ahead view are different pictures: the sun ahead is not a no-op.
    assert_ne!(runs[0].frames[0].rgba, runs[1].frames[0].rgba, "the sun ahead changed nothing");
    assert!(failures.is_empty(), "the lit picture of a still camera changed (images in {}): {}", dir.display(), failures.join(" | "));
}
