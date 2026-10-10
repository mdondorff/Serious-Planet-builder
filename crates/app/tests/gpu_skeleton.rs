//! Tier C: the walking skeleton end to end in all three run modes, and repro-bundle replay.

use planet_testkit::read_png;
use std::path::PathBuf;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("planet-skeleton-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

// spec: BUILD-007, REND-001
#[test]
fn the_skeleton_runs_in_all_three_modes_and_they_agree() {
    let (a, b) = (tmp("test-render.png"), tmp("editor.png"));
    let bundle = tmp("bundle.repro");
    let (pa, pb, pbundle) = (a.to_str().unwrap(), b.to_str().unwrap(), bundle.to_str().unwrap());
    let log =
        run(&["test-render", "--scene", "tiles", "--view", "height", "--adapter", "software", "--out", pa, "--bundle", pbundle]).unwrap();
    assert!(log.contains("2 draws, 2 draw calls"), "{log}");
    run(&["editor", "--offscreen", "--view", "height", "--adapter", "software", "--out", pb]).unwrap();
    assert_eq!(read_png(&a).unwrap(), read_png(&b).unwrap(), "editor --offscreen and test-render must produce the same frame");

    // generate: CPU only, prints the hashes and writes the height map and the face net.
    let (h, n) = (tmp("height.png"), tmp("net.png"));
    let log = run(&["generate", "--tile", "3,3,5,2", "--out", h.to_str().unwrap(), "--face-net", n.to_str().unwrap()]).unwrap();
    assert!(log.contains("content hash") && log.contains("cache key"), "{log}");
    assert_eq!(read_png(&h).unwrap().width, 17);
    assert_eq!(read_png(&n).unwrap().width, 512);
}

// spec: TEST-008
#[test]
fn a_repro_bundle_replays_to_the_same_pixels_and_refuses_a_version_mismatch() {
    let (png, bundle, again) = (tmp("r1.png"), tmp("r.repro"), tmp("r2.png"));
    run(&[
        "test-render",
        "--scene",
        "tiles",
        "--view",
        "tile-id",
        "--adapter",
        "software",
        "--out",
        png.to_str().unwrap(),
        "--bundle",
        bundle.to_str().unwrap(),
    ])
    .unwrap();
    let log =
        run(&["test-render", "--adapter", "software", "--repro", bundle.to_str().unwrap(), "--out", again.to_str().unwrap()]).unwrap();
    assert!(log.contains("plan identical"), "{log}");
    assert_eq!(read_png(&png).unwrap(), read_png(&again).unwrap());

    let text = std::fs::read_to_string(&bundle).unwrap();
    // Lower the recorded generator version; the plan hash line is unaffected, so only the version check can object.
    let old = text.replacen("generator_version 2", "generator_version 1", 1);
    let bad = tmp("old.repro");
    std::fs::write(&bad, old).unwrap();
    let e = run(&["test-render", "--adapter", "software", "--repro", bad.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("recorded with generator 1") && e.contains("this build has 2"), "{e}");
}

// spec: BUILD-004
#[test]
fn unknown_views_and_tiles_are_reported_with_the_expected_form() {
    let e = run(&["test-render", "--scene", "tiles", "--view", "sepia", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("face, tile-id or height"), "{e}");
    let e = run(&["test-render", "--scene", "tiles", "--tile", "9,9,9,9", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("not a valid tile"), "{e}");
    let e = run(&["test-render", "--scene", "tiles", "--tile", "1,2", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("FACE,LEVEL,X,Y"), "{e}");
}

// spec: TEST-008, BUILD-007
#[test]
fn editor_offscreen_replays_bundles_and_generate_rejects_frame_options() {
    let (png, bundle, again) = (tmp("e1.png"), tmp("e.repro"), tmp("e2.png"));
    run(&[
        "test-render",
        "--scene",
        "tiles",
        "--view",
        "face",
        "--adapter",
        "software",
        "--out",
        png.to_str().unwrap(),
        "--bundle",
        bundle.to_str().unwrap(),
    ])
    .unwrap();
    let log = run(&["editor", "--offscreen", "--repro", bundle.to_str().unwrap(), "--out", again.to_str().unwrap()]).unwrap();
    assert!(log.contains("plan identical"), "{log}");
    assert_eq!(read_png(&png).unwrap(), read_png(&again).unwrap());
    let e = run(&["generate", "--repro", bundle.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("generate has no frames"), "{e}");
    let e = run(&["generate", "--res", "3"]).unwrap_err();
    assert!(e.contains("power of two"), "{e}");
}

// spec: STRM-002, BUILD-007
#[test]
fn streamed_terrain_equals_synchronous_terrain() {
    let (a, b) = (tmp("terrain-sync.png"), tmp("terrain-stream.png"));
    let base = ["test-render", "--scene", "terrain", "--script", "aerial-200km", "--view", "tile-id", "--adapter", "software", "--out"];
    let log_a = run(&[&base[..], &[a.to_str().unwrap()]].concat()).unwrap();
    let log_b = run(&[&base[..], &[b.to_str().unwrap(), "--stream"]].concat()).unwrap();
    assert!(log_a.contains("draw calls") && log_b.contains("draw calls"));
    assert_eq!(read_png(&a).unwrap(), read_png(&b).unwrap(), "the streaming path must produce the same frame");
}
