//! Tier C, TEST-068: with the camera still and streaming settled, consecutive frames are pixel-identical (no pixel may change: `diff` with `Tolerance::Exact`). The scene's first view at 200 m on the
//! software adapter (a small memory bake), settled as the edit-latency scene settles, then 6 frames of the same request, each read back, in the level,
//! relief and normals debug views. A change writes the two frames and the diff to `target/review/still-flicker/` and fails naming the changed pixels,
//! their box and the drawn tiles that differ.

use planet::editperf::StillFrame;
use planet_frame::TerrainView;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::{diff, diff_image, repo_root, write_png, Image, Tolerance};
use std::collections::BTreeSet;
use std::path::Path;

const FRAMES: usize = 6;
const VIEWS: [TerrainView; 3] = [TerrainView::Level, TerrainView::Relief, TerrainView::Normals];

/// The changes between consecutive frames of a still camera: one message per changed pair naming the pixels, their box and the drawn tiles that differ;
/// both frames and the diff image of each go to `dir`. Empty when every pair is identical.
pub(crate) fn changes(name: &str, frames: &[StillFrame], dir: &Path) -> Vec<String> {
    let images: Vec<Image> = frames.iter().map(|f| Image::new(f.width, f.height, f.rgba.clone())).collect();
    let mut failures = Vec::new();
    for i in 1..frames.len() {
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
        write_png(&dir.join(format!("{name}-diff-{}-{i}.png", i - 1)), &diff_image(&images[i - 1], &images[i], Tolerance::Exact)).unwrap();
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
    failures
}

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
        // The test can only falsify if there is something to see: more than one colour in the picture, and tiles drawn.
        let first = Image::new(frames[0].width, frames[0].height, frames[0].rgba.clone());
        let colours: BTreeSet<[u8; 4]> = first.pixels().collect();
        // A picture to look at: the first frame of each view is kept beside the failures (CI uploads `target/review/`).
        write_png(&dir.join(format!("{name}-first.png")), &first).unwrap();
        eprintln!("{name}: {} colours, {} tiles in the first frame", colours.len(), frames[0].nodes.len());
        assert!(colours.len() > 2, "{name}: the still frame has {} colours: nothing was drawn", colours.len());
        assert!(frames[0].nodes.len() > 8, "{name}: only {} tiles were drawn", frames[0].nodes.len());
        failures.extend(changes(&name, &frames, &dir));
    }
    assert!(failures.is_empty(), "the picture of a still camera changed (images in {}): {}", dir.display(), failures.join(" | "));
}

fn frame(rgba: Vec<u8>, tiles: &[(u32, u8)]) -> StillFrame {
    let nodes =
        tiles.iter().map(|&(x, l)| (planet_core::TileId::new(planet_core::Face(0), 4, x, 0).unwrap(), l, "Exact".to_string())).collect();
    StillFrame { width: 4, height: 2, rgba, nodes }
}

// spec: TEST-068
#[test]
fn a_changed_pixel_between_two_still_frames_is_named_with_its_box_and_tiles_and_its_images_are_written() {
    let dir = std::env::temp_dir().join(format!("planet-still-flicker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let same = vec![10u8; 4 * 2 * 4];
    let mut changed = same.clone();
    changed[(3 + 4) * 4] = 200; // pixel (3, 1) of a 4 x 2 picture
    assert!(changes("t", &[frame(same.clone(), &[(0, 3)]), frame(same.clone(), &[(0, 3)])], &dir).is_empty());
    let got = changes("t", &[frame(same.clone(), &[(0, 3)]), frame(changed, &[(0, 3), (1, 4)])], &dir);
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(
        got[0].contains("1 pixels changed") && got[0].contains("box (3,1)-(3,1)") && got[0].contains("drawn tiles that differ (1)"),
        "{got:?}"
    );
    for f in ["t-frame-0.png", "t-frame-1.png", "t-diff-0-1.png"] {
        assert!(dir.join(f).is_file(), "{f} was not written");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
