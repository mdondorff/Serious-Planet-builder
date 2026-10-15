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

// spec: TEST-019, TEST-020
#[test]
fn the_six_scenes_run_end_to_end_on_a_stored_bake_in_smoke_mode_and_log_their_source() {
    let store = tmp("store");
    run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", "256", "--threads", "2"]).unwrap();
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
        "--out",
        logs.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    assert!(out.contains(&format!("source: store {}, 256 cells", store.display())), "the source is printed: {out}");
    for scene in ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km"] {
        assert!(out.contains(&format!("{scene}: blank tiles: none in any of the frames ok")), "{scene}: {out}");
        assert!(out.contains(&format!("{scene}: columns (median / p99 / max ms): plan ")), "{scene}: {out}");
        let json = std::fs::read_to_string(logs.join(format!("{scene}.json"))).unwrap();
        assert!(json.contains("\"valid_for_budgets\":false") && json.contains("smoke run"), "{json}");
        assert!(json.contains(&format!("\"path\":\"stream/{scene}\"")), "{json}");
        assert!(json.contains("\"source\":\"store ") && json.contains("256 cells\""), "{json}");
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
