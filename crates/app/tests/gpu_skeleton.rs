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

// spec: STRM-002, BUILD-007, STRM-017
#[test]
fn streamed_terrain_equals_synchronous_terrain() {
    let (a, b) = (tmp("terrain-sync.png"), tmp("terrain-stream.png"));
    let base = ["test-render", "--scene", "terrain", "--script", "aerial-200km", "--view", "tile-id", "--adapter", "software", "--out"];
    let log_a = run(&[&base[..], &[a.to_str().unwrap()]].concat()).unwrap();
    let log_b = run(&[&base[..], &[b.to_str().unwrap(), "--stream"]].concat()).unwrap();
    assert!(log_a.contains("draw calls") && log_b.contains("draw calls"));
    assert_eq!(read_png(&a).unwrap(), read_png(&b).unwrap(), "the streaming path must produce the same frame");
}

// spec: STRM-016, STRM-002, STRM-017
#[test]
fn the_pipelined_loop_settles_on_the_synchronous_frame_from_a_cold_start_and_after_a_jump() {
    let sync = |script: &str, name: &str| {
        let png = tmp(name);
        let args = ["test-render", "--scene", "terrain", "--script", script, "--view", "tile-id", "--adapter", "software", "--out"];
        run(&[&args[..], &[png.to_str().unwrap()]].concat()).unwrap();
        png
    };
    let piped = |script: &str, then: Option<&str>, name: &str, dir: Option<&PathBuf>| {
        let png = tmp(name);
        let args =
            ["test-render", "--scene", "terrain", "--script", script, "--view", "tile-id", "--adapter", "software", "--pipelined", "--out"];
        let mut all: Vec<&str> = [&args[..], &[png.to_str().unwrap()]].concat();
        if let Some(t) = then {
            all.extend(["--then", t]);
        }
        if let Some(d) = dir {
            all.extend(["--out-dir", d.to_str().unwrap()]);
        }
        let log = run(&all).unwrap();
        (png, log)
    };
    // Cold start: the first frame is a cover (never blank), the loop ends on the frame the synchronous path draws.
    let dir = tmp("pipelined-log");
    let (p1, log1) = piped("aerial-200km", None, "pipe-cold.png", Some(&dir));
    assert!(log1.contains("cover") && log1.contains("exact 1") && log1.contains("pipelined"), "{log1}");
    assert_eq!(read_png(&sync("aerial-200km", "sync-a.png")).unwrap(), read_png(&p1).unwrap(), "cold start");
    let text = std::fs::read_to_string(dir.join("pipelined.txt")).unwrap();
    assert!(
        text.starts_with(
            "# iteration kind nodes substituted in_flight base_seq plan_ms scheduler_ms upload_ms record_ms gpu_ms wall_ms \
             resident in_flight queued dispatched cancelled blocked evicted uploaded degraded upload_bytes deferred node_bytes"
        ),
        "{text}"
    );
    // The manager's per-frame counts: tiles went into slots, nothing was blocked or degraded (the share holds everything).
    let last: Vec<&str> = text.lines().last().unwrap().split_whitespace().collect();
    let col = |name: &str| -> u64 {
        let names: Vec<&str> = text.lines().next().unwrap().trim_start_matches("# ").split_whitespace().collect();
        let at = names.iter().rposition(|n| *n == name).unwrap();
        last[at].parse().unwrap()
    };
    assert!(col("resident") > 6, "the loop places tiles in slots: {text}");
    assert_eq!((col("blocked"), col("degraded")), (0, 0), "{text}");
    assert!(log1.contains("slots") && log1.contains("resident of"), "{log1}");
    assert!(text.lines().nth(1).unwrap().contains(" cover "), "frame 1 is a cover plan: {text}");
    assert!(text.lines().last().unwrap().contains(" exact "), "{text}");
    // A jump: settle on one view, then another; the frame the loop ends on is the synchronous frame of the second.
    let (p2, log2) = piped("orbit-20000km", Some("aerial-20km"), "pipe-jump.png", None);
    assert!(log2.contains("cover") && log2.contains("pipelined"), "{log2}");
    assert_eq!(read_png(&sync("aerial-20km", "sync-b.png")).unwrap(), read_png(&p2).unwrap(), "after a jump");
    // Options that do not combine are refused before any GPU work.
    let e = run(&["test-render", "--scene", "terrain", "--pipelined", "--stream"]).unwrap_err();
    assert!(e.contains("excludes --stream"), "{e}");
    let e = run(&["test-render", "--scene", "terrain", "--then", "3"]).unwrap_err();
    assert!(e.contains("--then only applies to --pipelined"), "{e}");
}

// spec: TEST-008
#[test]
fn a_terrain_bundle_replays_to_the_same_pixels_and_refuses_changed_inputs() {
    let (png, bundle, again) = (tmp("t1.png"), tmp("t.repro"), tmp("t2.png"));
    run(&[
        "test-render",
        "--scene",
        "terrain",
        "--script",
        "4",
        "--view",
        "level",
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
    assert!(log.contains("terrain plan identical"), "{log}");
    assert_eq!(read_png(&png).unwrap(), read_png(&again).unwrap(), "the replay must draw the recorded pixels");

    let text = std::fs::read_to_string(&bundle).unwrap();
    // A version 1 bundle (written before bundles could record a world) is the seed-only reference field and still replays.
    let v1 = tmp("tv1.repro");
    assert!(text.starts_with("planet-terrain-repro 2\n") && !text.contains("world_toml"), "a seed-only run records no world");
    std::fs::write(&v1, text.replacen("planet-terrain-repro 2", "planet-terrain-repro 1", 1)).unwrap();
    let log = run(&["test-render", "--adapter", "software", "--repro", v1.to_str().unwrap()]).unwrap();
    assert!(log.contains("terrain plan identical"), "{log}");
    // A version mismatch is refused before anything is replanned.
    let bad = tmp("told.repro");
    let (current, old) = (planet_generators::mesh::MESH_VERSION, planet_generators::mesh::MESH_VERSION - 1);
    std::fs::write(&bad, text.replacen(&format!("mesh_version {current}"), &format!("mesh_version {old}"), 1)).unwrap();
    let e = run(&["test-render", "--adapter", "software", "--repro", bad.to_str().unwrap()]).unwrap_err();
    assert!(e.contains(&format!("mesh version {old}")) && e.contains("this build has"), "{e}");
    // The bundle records the definition it was made from; a hash this build cannot rebuild is refused (CON-19).
    let line = text.lines().find(|l| l.starts_with("definition_hash ")).expect("definition_hash line").to_string();
    assert_ne!(line, "definition_hash 0000000000000000", "a new bundle records the real definition hash");
    for (replacement, needle) in
        [("definition_hash 0000000000000000", "predates the WorldDefinition"), ("definition_hash 0000000000000001", "can only rebuild")]
    {
        let bad = tmp("tdef.repro");
        std::fs::write(&bad, text.replacen(&line, replacement, 1)).unwrap();
        let e = run(&["test-render", "--adapter", "software", "--repro", bad.to_str().unwrap()]).unwrap_err();
        assert!(e.contains(needle), "{e}");
    }
    // Another seed leaves the recorded plan text intact but replans to other mesh keys: the replay reports the difference.
    let other = tmp("tseed.repro");
    assert!(text.contains("\nseed 1\n"));
    let seed2 = text.replacen("\nseed 1\n", "\nseed 2\n", 1);
    // With the old definition hash the replay refuses before replanning; with the hash of seed 2 it replans and reports.
    std::fs::write(&other, &seed2).unwrap();
    let e = run(&["test-render", "--adapter", "software", "--repro", other.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("can only rebuild"), "{e}");
    let hash2 = planet_world_def::WorldDefinition::single_planet(6_371_000.0, 2).hash();
    std::fs::write(&other, seed2.replacen(&line, &format!("definition_hash {hash2:016x}"), 1)).unwrap();
    let e = run(&["test-render", "--adapter", "software", "--repro", other.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("replayed terrain plan differs") && e.contains("mesh_key"), "{e}");
}

// spec: REND-015, STRM-016, STRM-017
#[test]
fn the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either() {
    let base = ["test-render", "--scene", "terrain", "--script", "aerial-200km", "--view", "relief", "--adapter", "software"];
    let render = |extra: &[&str], name: &str| {
        let png = tmp(name);
        let log = run(&[&base[..], extra, &["--out", png.to_str().unwrap()]].concat()).unwrap();
        (read_png(&png).unwrap(), log)
    };
    let (reference, log) = render(&["--draw", "per-draw"], "draw-per.png");
    assert!(log.contains("draw: per-draw requested, per-draw in effect"), "{log}");
    let (indirect, log) = render(&["--draw", "indirect"], "draw-ind.png");
    assert!(log.contains("draw: indirect requested,"), "{log}");
    assert_eq!(reference, indirect, "the synchronous path");
    // The streaming and the pipelined loops (frames in flight, managed slots) on the indirect path.
    assert_eq!(reference, render(&["--draw", "indirect", "--stream"], "draw-stream.png").0, "streamed");
    assert_eq!(reference, render(&["--draw", "indirect", "--pipelined"], "draw-pipe.png").0, "pipelined");
    assert_eq!(reference, render(&["--draw", "per-draw", "--pipelined"], "draw-pipe-per.png").0, "pipelined, per-draw");
    assert!(run(&[&base[..], &["--draw", "bogus"]].concat()).unwrap_err().contains("unknown draw mode"));
}
