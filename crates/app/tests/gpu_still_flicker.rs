//! Tier C, TEST-068: with the camera still and streaming settled, consecutive frames are pixel-identical (no pixel may change: `diff` with `Tolerance::Exact`). The scene's first view at 200 m on the
//! software adapter (a small memory bake), settled as the edit-latency scene settles, then 6 frames of the same request, each read back, in the level,
//! relief and normals debug views. A change writes the two frames and the diff to `target/review/still-flicker/` and fails naming the changed pixels,
//! their box and the drawn tiles that differ.

use planet_frame::TerrainView;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::{diff, diff_image, repo_root, write_png, Image, Tolerance};
use std::collections::BTreeSet;

const FRAMES: usize = 6;
const VIEWS: [TerrainView; 3] = [TerrainView::Level, TerrainView::Relief, TerrainView::Normals];

// spec: TEST-068
#[test]
fn a_still_camera_after_streaming_settles_draws_the_same_pixels_frame_after_frame() {
    let args: Vec<String> = [
        "test-render",
        "--perf-smoke",
        "--scene",
        "edit-latency",
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
    let dir = repo_root().join("target").join("review").join("still-flicker");
    let mut failures = Vec::new();
    let sessions = planet::editperf::still_frames(&o, &ctx, &VIEWS, FRAMES).expect("the still frames");
    for (view, frames) in sessions {
        let name = format!("{view:?}").to_lowercase();
        assert_eq!(frames.len(), FRAMES);
        let images: Vec<Image> = frames.iter().map(|f| Image::new(f.width, f.height, f.rgba.clone())).collect();
        // The test can only falsify if there is something to see: more than one colour in the picture, and tiles drawn.
        let colours: BTreeSet<[u8; 4]> = images[0].pixels().collect();
        assert!(colours.len() > 2, "{name}: the still frame has {} colours: nothing was drawn", colours.len());
        assert!(frames[0].nodes.len() > 8, "{name}: only {} tiles were drawn", frames[0].nodes.len());
        for i in 1..FRAMES {
            let Some(d) = diff(&images[i - 1], &images[i], Tolerance::Exact) else { continue };
            // The box of the changed pixels and the drawn tiles that differ between the two frames.
            let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
            for (k, (a, b)) in images[i - 1].pixels().zip(images[i].pixels()).enumerate() {
                if a != b {
                    let (x, y) = ((k as u32) % images[i].width, (k as u32) / images[i].width);
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
            let (before, after): (BTreeSet<_>, BTreeSet<_>) = (frames[i - 1].nodes.iter().collect(), frames[i].nodes.iter().collect());
            let tiles: Vec<String> = before.symmetric_difference(&after).take(8).map(|(t, l, s)| format!("{t:?} L{l} {s}")).collect();
            write_png(&dir.join(format!("{name}-frame-{}.png", i - 1)), &images[i - 1]).unwrap();
            write_png(&dir.join(format!("{name}-frame-{i}.png")), &images[i]).unwrap();
            write_png(&dir.join(format!("{name}-diff-{}-{i}.png", i - 1)), &diff_image(&images[i - 1], &images[i], Tolerance::Exact))
                .unwrap();
            failures.push(format!(
                "{name} frames {} -> {i}: {} pixels changed (max channel delta {}), box ({x0},{y0})-({x1},{y1}), first {:?}; drawn tiles that differ ({}): {}",
                i - 1,
                d.differing_pixels,
                d.max_channel_delta,
                d.first_differing,
                before.symmetric_difference(&after).count(),
                tiles.join("; ")
            ));
        }
    }
    assert!(failures.is_empty(), "the picture of a still camera changed (images in {}): {}", dir.display(), failures.join(" | "));
}
