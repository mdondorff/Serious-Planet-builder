//! Tier C: the streaming perf scenes end to end on the software adapter (smoke), and their refusals (TEST-019, TEST-012).

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("planet-stream-perf-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

// spec: TEST-019, TEST-020, STRM-022
#[test]
fn the_six_scenes_run_end_to_end_on_a_stored_bake_in_smoke_mode_and_log_their_source() {
    let store = tmp("store");
    // Levels 0-1 pre-filled (30 tiles): the scenes check the same levels at their start (the gated run needs 0-4, the smoke run less).
    let built =
        run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", "256", "--threads", "2", "--prefill-levels", "1"])
            .unwrap();
    assert!(built.contains("30 tiles, 30 written"), "{built}");
    let logs = tmp("logs");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "streaming",
        "--adapter",
        "software",
        "--store",
        store.to_str().unwrap(),
        "--global-res",
        "256",
        "--prefill-levels",
        "1",
        "--out",
        logs.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    assert!(out.contains(&format!("source: store {}, 256 cells", store.display())), "the source is printed: {out}");
    for scene in ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"] {
        assert!(out.contains(&format!("{scene}: blank tiles: none in any of the frames ok")), "{scene}: {out}");
        assert!(out.contains(&format!("{scene}: columns (median / p99 / max ms): plan ")), "{scene}: {out}");
        let json = std::fs::read_to_string(logs.join(format!("{scene}.json"))).unwrap();
        assert!(json.contains("\"valid_for_budgets\":false") && json.contains("smoke run"), "{json}");
        assert!(json.contains(&format!("\"path\":\"stream/{scene}\"")), "{json}");
        // The source names where the bake was read from and what the tile pack held at the start (STRM-021, TEST-021).
        let marker = if scene == "dive-cold" {
            "cold: the pack's coarse tiles are ignored"
        } else {
            "prefilled coarse pack (levels 0-1 present at start)"
        };
        assert!(json.contains("\"source\":\"store ") && json.contains(&format!("256 cells; {marker}")), "{scene}: {json}");
        assert!(out.contains(&format!("{scene}: tile pack at the start: {marker}")), "{scene}: {out}");
        let used =
            json.split("\"prefill_tiles_used\":").nth(1).and_then(|t| t.split([',', '}']).next()).and_then(|n| n.parse::<u64>().ok());
        assert!(
            if scene == "dive-cold" { used == Some(0) } else { used.is_some_and(|n| n >= 6) },
            "{scene}: the pack is read except by the cold reader: {used:?}"
        );
        // The tile loads of the measured part are recorded (STRM-022), under the `load_` prefix; a smoke run takes no calibration.
        let count = |name: &str| {
            json.split(&format!("\"{name}\":")).nth(1).and_then(|t| t.split([',', '}']).next()).and_then(|n| n.parse::<u64>().ok())
        };
        for name in [
            "load_jobs",
            "load_run_us_median",
            "load_wait_us_p99",
            "load_block_decodes",
            "load_block_evictions",
            "load_pack_gets",
            "load_prefill_lookups",
            "load_field_builds",
        ] {
            assert!(count(name).is_some(), "{scene}: no count {name}: {json}");
        }
        assert!(count("load_pack_gets") >= count("load_block_decodes"), "{scene}: every block decode asked the pack: {json}");
        assert!(!json.contains("load_calib_"), "{scene}: a smoke run measures no calibration: {json}");
        assert!(json.contains("\"field\":\"height field cpu-erosion-v1 over a 256-cell bake\""), "{json}");
        for column in ["plan_ms", "scheduler_ms", "upload_ms", "record_ms"] {
            assert!(json.contains(&format!("\"{column}\"")), "{scene} {column}: {json}");
        }
        assert!(!json.contains("\"gpu_ms\""), "{scene}: no GPU time is claimed: {json}");
        let frames = std::fs::read_to_string(logs.join(format!("{scene}.frames.txt"))).unwrap();
        assert!(frames.starts_with("# frame plan_ms scheduler_ms upload_ms record_ms gpu_ms wall_ms lag uncovered blank"), "{frames}");
        assert!(frames.lines().count() > 20, "{scene}: {frames}");
        // Every frame has a lag from the post-pass (TEST-020): the lag column (index 7) is a number, never `-`.
        for line in frames.lines().skip(1) {
            let lag = line.split_whitespace().nth(7).unwrap_or("");
            assert!(lag.parse::<f64>().is_ok(), "{scene}: a frame without a lag: {line}");
        }
        assert!(out.contains(&format!("{scene}: detail lag post-pass over 24 frames")), "{scene}: {out}");
        // A gated lag rule is judged, never "not measured": the look scenes' rotation rule and the dive's descent rule have a verdict.
        let gated = if scene.starts_with("look") { "rotation" } else { "detail lag: descent" };
        if scene.starts_with("look") || scene.starts_with("dive") {
            let line = out
                .lines()
                .find(|l| l.starts_with(&format!("  {scene}: {gated}")))
                .unwrap_or_else(|| panic!("{scene}: no {gated} line: {out}"));
            assert!(!line.ends_with("not measured"), "{line}");
        }
    }
    assert!(out.contains("GPU time: not measured"), "{out}");
    let _ = std::fs::remove_dir_all(&store);
    let _ = std::fs::remove_dir_all(&logs);
}

// spec: TEST-019
#[test]
fn an_in_memory_run_is_labelled_not_the_production_source_and_real_runs_refuse_without_a_store_and_on_software() {
    let log = tmp("mem.json");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "look-2km",
        "--adapter",
        "software",
        "--global-res",
        "8",
        "--out",
        log.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("source: in-memory 8 cells (NOT the production source)"), "{out}");
    let json = std::fs::read_to_string(&log).unwrap();
    assert!(json.contains("\"source\":\"in-memory 8 cells (NOT the production source)\""), "{json}");
    let _ = std::fs::remove_file(&log);
    // A real run without a store is refused with the remedy; with the memory bake allowed it still refuses the software adapter.
    let e = run(&["test-render", "--perf", "--scene", "dive", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("--store DIR") && e.contains("--allow-memory-bake"), "{e}");
    let e = run(&["test-render", "--perf", "--scene", "dive", "--adapter", "software", "--allow-memory-bake", "--global-res", "8"])
        .unwrap_err();
    assert!(e.contains("not a discrete GPU"), "{e}");
    let e = run(&["test-render", "--perf", "--scene", "dive", "--adapter", "software", "--store", "no-such-dir-here"]).unwrap_err();
    assert!(e.contains("no such store directory"), "{e}");
}

// spec: TEST-019, STRM-021
#[test]
fn the_gated_dives_refuse_a_store_without_the_pre_filled_levels_before_anything_runs_and_the_cold_dive_does_not_care() {
    let store = tmp("refuse-store");
    let s = store.to_str().unwrap();
    run(&["generate", "--global-dir", s, "--global-res", "8", "--threads", "2", "--prefill-levels", "0"]).unwrap();
    for scene in ["dive", "dive-fast"] {
        let e = run(&["test-render", "--perf", "--scene", scene, "--adapter", "software", "--store", s, "--global-res", "8"]).unwrap_err();
        assert!(
            e.contains(&format!("{scene} starts with levels 0-4 pre-filled"))
                && e.contains("holds 0 of the 2046 tiles")
                && e.contains("generate --prefill-dir"),
            "{scene}: {e}"
        );
    }
    // The levels required are a parameter (the smoke tests use fewer; `accept M4` needs the 4 of the owner's decision).
    let e = run(&[
        "test-render",
        "--perf",
        "--scene",
        "dive",
        "--adapter",
        "software",
        "--store",
        s,
        "--global-res",
        "8",
        "--prefill-levels",
        "1",
    ])
    .unwrap_err();
    assert!(e.contains("levels 0-1 pre-filled") && e.contains("holds 0 of the 30 tiles"), "{e}");
    // The scenes that only use the pack, and the cold dive, are not refused for lacking it: they go on to the next check (the adapter).
    for scene in ["dive-cold", "teleport-1m", "look-2km"] {
        let e = run(&["test-render", "--perf", "--scene", scene, "--adapter", "software", "--store", s, "--global-res", "8"]).unwrap_err();
        assert!(e.contains("not a discrete GPU"), "{scene}: {e}");
    }
    // After the pre-fill (idempotent, into the existing bake) the gated dive passes the check and meets the adapter check.
    let done = run(&["generate", "--prefill-dir", s, "--global-res", "8", "--prefill-levels", "1", "--threads", "2"]).unwrap();
    assert!(done.contains("30 tiles, 30 written"), "{done}");
    let e = run(&[
        "test-render",
        "--perf",
        "--scene",
        "dive",
        "--adapter",
        "software",
        "--store",
        s,
        "--global-res",
        "8",
        "--prefill-levels",
        "1",
    ])
    .unwrap_err();
    assert!(e.contains("not a discrete GPU"), "{e}");
    let _ = std::fs::remove_dir_all(&store);
}
