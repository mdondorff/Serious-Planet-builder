//! Tier C, TEST-076: the edit shown as the REND-033 preview and the same edit at rest (the pair of the owner's drag report). The software adapter and a small
//! memory bake (the scene's smoke world): a 50 m hill at 200 m, the stale frame, the first frame after the edit and the exact frame once the refresh settled,
//! in the relief and normals debug views, judged by `planet_testkit::pair` (TEST-077). The verdict of each view is reported, not asserted: the pair is a
//! measurement of how far the preview is from the exact frame (REND-033), and what it finds is a finding for the planner. The test asserts what makes the
//! pair trustworthy: the control (the world after taking the edit back) is the stale frame bit for bit, and the edit is seen. The pictures go to
//! `target/review/edit-pair/`. The real-store run with the lit frame and the horizon is the scene `edit-pair`.

use planet::editperf::pair_frames;
use planet_frame::TerrainView;
use planet_render::{AdapterPolicy, GpuContext};
use planet_testkit::pair::{compare_pair, PairVerdict};
use planet_testkit::{repo_root, write_png, Image};

const VIEWS: [TerrainView; 2] = [TerrainView::Relief, TerrainView::Normals];
const GREY: u8 = 3;

// spec: TEST-076
#[test]
fn the_pair_of_an_edit_is_trustworthy_and_its_verdict_per_view_is_reported() {
    let args: Vec<String> = [
        "test-render",
        "--perf-smoke",
        "--scene",
        "edit-latency",
        "--adapter",
        "software",
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
    let dir = repo_root().join("target").join("review").join("edit-pair");
    std::fs::create_dir_all(&dir).unwrap();
    let pairs = pair_frames(&o, &ctx, &VIEWS, 200.0, 50.0).expect("the pair frames");
    assert_eq!(pairs.len(), VIEWS.len());
    let mut seen = 0;
    for p in &pairs {
        let img = |f: &planet::editperf::StillFrame| Image::new(f.width, f.height, f.rgba.clone());
        let (stale, preview, exact) = (img(&p.stale), img(&p.preview), img(&p.exact));
        let name = format!("{:?}", p.view).to_lowercase();
        for (tag, i) in [("stale", &stale), ("preview", &preview), ("exact", &exact)] {
            write_png(&dir.join(format!("{name}-{tag}.png")), i).unwrap();
        }
        let differing = p.stale.rgba.iter().zip(&p.restored.rgba).filter(|(a, b)| a != b).count();
        println!("{name}: control, the world after taking the edit back against the stale frame: {differing} bytes differ");
        assert_eq!(
            p.stale.rgba, p.restored.rgba,
            "{name}: the world after taking the edit back differs from the stale frame ({differing} bytes)"
        );
        let r = compare_pair(&stale, &preview, &exact, GREY).expect("frames of one size");
        println!("{name}: {}", r.line());
        seen += usize::from(r.verdict != PairVerdict::TooSmall);
    }
    assert!(seen > 0, "no view shows the edit (every pair is TooSmall): the pair proves nothing");
}
