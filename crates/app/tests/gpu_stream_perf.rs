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

/// Every file of a directory tree with its size and modification time: what a read-only run must leave unchanged.
fn listing(dir: &std::path::Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut v = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let m = e.metadata().unwrap();
            if m.is_dir() {
                stack.push(e.path());
            } else {
                v.push((e.path().to_string_lossy().into_owned(), m.len(), m.modified().unwrap()));
            }
        }
    }
    v.sort();
    v
}

// spec: TEST-019, TEST-020, STRM-022, TEST-023, TEST-024
#[test]
fn the_six_scenes_run_end_to_end_on_a_stored_bake_in_smoke_mode_and_log_their_source() {
    let store = tmp("store");
    // Levels 0-1 pre-filled (30 tiles): the scenes check the same levels at their start (the gated run needs 0-4, the smoke run less).
    let built =
        run(&["generate", "--global-dir", store.to_str().unwrap(), "--global-res", "256", "--threads", "2", "--prefill-levels", "1"])
            .unwrap();
    assert!(built.contains("30 tiles, 30 written"), "{built}");
    let logs = tmp("logs");
    let before = listing(&store);
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
    // The store check (TEST-024): the ocean channel and the compact pack were read, and nothing was written.
    assert!(
        out.contains("store check (smoke, read-only, NOT a measurement)") && out.contains("ocean channel: present, 288 directions read"),
        "{out}"
    );
    assert!(out.contains("tile pack, levels 0-1: compact form 30 of 30 present") && out.contains("store check ok"), "{out}");
    assert_eq!(before, listing(&store), "the smoke run wrote into the store");
    assert!(out.contains("host load: not measured"), "a smoke run takes no host load: {out}");
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
    // A store whose pack lacks the levels asked for fails the smoke before any scene runs, naming the count and the command.
    let e = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "look-1m",
        "--adapter",
        "software",
        "--store",
        store.to_str().unwrap(),
        "--global-res",
        "256",
        "--prefill-levels",
        "2",
    ])
    .unwrap_err();
    assert!(
        e.contains("store check FAILED") && e.contains("of 126 compact tiles of levels 0-2") && e.contains("generate --prefill-dir"),
        "{e}"
    );
    assert_eq!(before, listing(&store), "the failed check wrote nothing either");
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

// spec: TEST-025, TEST-012, TEST-023, ATMO-020
#[test]
fn the_lit_scene_runs_every_view_and_both_flights_with_a_pass_row_each_and_logs_the_metrics_in_smoke_mode() {
    let log = tmp("lit.json");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "lit",
        "--adapter",
        "software",
        "--global-res",
        "128",
        "--out",
        log.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    for view in [
        "ground-1m",
        "aerial-20km",
        "coast-2km",
        "coast-3m",
        "orbit",
        "coast-glide",
        "coast-glide@filter0",
        "coast-descent",
        "coast-descent@filter0",
    ] {
        assert!(out.contains(&format!("lit/{view}: frame (waited) median")), "{view}: {out}");
        assert!(out.contains(&format!("  lit/{view}: GPU ms per pass (median / p99 / max): terrain ")), "{view}: {out}");
    }
    assert!(
        out.contains("lit path (atmosphere, water)") && out.contains("tile format vertex"),
        "the software smoke runs the vertex format: {out}"
    );
    // The held views plan water with the exact dry-tile minimum of every tile (ATMO-020): the cut counts are on the view lines.
    for view in ["coast-2km", "coast-3m"] {
        let line = out
            .lines()
            .find(|l| l.starts_with(&format!("lit/{view}: ")) && l.contains("with water (cut "))
            .unwrap_or_else(|| panic!("{view}: no cut line in {out}"));
        let exact: u64 = line
            .split("height (")
            .nth(1)
            .and_then(|t| t.split(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{line}"));
        assert!(exact > 0, "{view}: the exact minimum cut no node: {line}");
        assert!(line.contains(", 0 kept with no minimum)"), "{view}: tiles without a minimum: {line}");
    }
    assert!(!out.contains("terrain pass median") && !out.contains("budget 1 ms"), "a smoke run prints no budget rows: {out}");
    let json = std::fs::read_to_string(&log).unwrap();
    assert!(json.contains("\"valid_for_budgets\":false") && json.contains("smoke run"), "{json}");
    for path in ["lit/ground-1m", "lit/coast-glide", "lit/coast-glide@filter0", "lit/coast-descent@filter0"] {
        assert!(json.contains(&format!("\"path\":\"{path}\"")), "{path}: {json}");
    }
    assert!(json.contains("\"load\":{\"measured\":false}"), "{json}");
    if out.contains("timestamps true") {
        // The adapter times its passes: the per-pass metrics are in the log, the atmosphere and the water sums with them.
        for m in [
            "gpu_ms",
            "gpu_water_ms",
            "gpu_sky_view_ms",
            "gpu_aerial_perspective_ms",
            "gpu_sky_composite_ms",
            "gpu_exposure_histogram_ms",
            "gpu_exposure_adapt_ms",
            "gpu_tonemap_ms",
            "gpu_scene_copy_ms",
            "gpu_atmosphere_ms",
            "gpu_water_total_ms",
            "gpu_span_ms",
        ] {
            assert!(json.contains(&format!("\"{m}\":{{")), "{m}: {json}");
        }
    }
    for c in ["tile_format_compact", "water_nodes", "atmosphere"] {
        assert!(json.contains(&format!("\"{c}\":")), "{c}: {json}");
    }
    // A view that does not exist is refused with the list; the options belong to the lit scenes.
    let e = run(&["test-render", "--perf-smoke", "--scene", "lit", "--adapter", "software", "--global-res", "128", "--views", "moon"])
        .unwrap_err();
    assert!(e.contains("no view 'moon'") && e.contains("coast-glide"), "{e}");
    let e = run(&["test-render", "--perf-smoke", "--scene", "dive", "--adapter", "software", "--global-res", "8", "--views", "orbit"])
        .unwrap_err();
    assert!(e.contains("--views applies to the scenes"), "{e}");
    let e = run(&["test-render", "--perf-smoke", "--scene", "terrain", "--adapter", "software", "--global-res", "8", "--lit"]).unwrap_err();
    assert!(e.contains("--lit applies to the streaming perf scenes"), "{e}");
    let _ = std::fs::remove_file(&log);
}

// spec: TEST-025
#[test]
fn the_lit_formats_scene_runs_four_views_in_each_format_and_logs_a_path_per_format() {
    let log = tmp("litformats.json");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "lit-formats",
        "--adapter",
        "software",
        "--global-res",
        "128",
        "--out",
        log.to_str().unwrap(),
    ])
    .unwrap();
    // The software adapter runs the vertex format only (WARP removes its device with the compact format and the atmosphere).
    for view in ["ground-1m", "aerial-20km", "coast-2km", "orbit"] {
        assert!(out.contains(&format!("lit-formats/{view}/vertex: frame (waited) median")), "{view}: {out}");
    }
    assert!(!out.contains("/compact:"), "{out}");
    let json = std::fs::read_to_string(&log).unwrap();
    assert!(json.contains("\"path\":\"lit-formats/orbit/vertex\""), "{json}");
    let _ = std::fs::remove_file(&log);
}

// spec: TEST-025, TEST-019
#[test]
fn a_streaming_scene_on_the_lit_path_logs_beside_the_debug_path_under_its_own_name() {
    let log = tmp("look-1m-lit.json");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "look-1m",
        "--lit",
        "--adapter",
        "software",
        "--global-res",
        "128",
        "--tile-format",
        "vertex",
        "--out",
        log.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("lit path (atmosphere, water)") && out.contains("look-1m: blank tiles: none in any of the frames ok"), "{out}");
    let json = std::fs::read_to_string(&log).unwrap();
    assert!(json.contains("\"path\":\"stream-lit/look-1m\"") && json.contains("lit path (atmosphere, water)"), "{json}");
    assert!(json.contains("\"load\":{\"measured\":false}"), "{json}");
    let _ = std::fs::remove_file(&log);
}

// spec: TEST-027
#[test]
fn the_capture_adds_the_shore_flights_and_the_close_stills_and_says_the_stills_bypass_the_ground_clamp() {
    let dir = tmp("capture-shore");
    let views =
        "shore-run-30m,shore-approach-3m,still-sand-0p5m-along,still-water-0p5m-20m-beach,still-sand-0p2m-seaward,still-water-0p1m-along";
    let out = run(&[
        "test-render",
        "--scene",
        "capture-flights",
        "--adapter",
        "software",
        "--allow-memory-bake",
        "--global-res",
        "128",
        "--size",
        "320x180",
        "--flight-frames",
        "2",
        "--views",
        views,
        "--out-dir",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("1 m ground clamp (VIEW-004)"), "{out}");
    let index = std::fs::read_to_string(dir.join("capture.txt")).unwrap();
    assert!(index.contains("VIEW-004"), "{index}");
    let rows: Vec<(&str, usize)> = index
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            let mut c = l.split_whitespace();
            (c.next().unwrap(), c.next().unwrap().parse().unwrap())
        })
        .collect();
    // Four stills (one frame each) and two flights (the given frame count), every one at shore_filter 1 and 0.
    assert_eq!(rows.len(), 12, "{index}");
    for (name, frames) in rows {
        assert_eq!(frames, if name.starts_with("still-") { 1 } else { 2 }, "{name}");
        for k in 0..frames {
            assert!(dir.join(name).join(format!("frame_{k:04}.png")).exists(), "{name} frame {k}");
        }
    }
    let e = run(&[
        "test-render",
        "--scene",
        "capture-flights",
        "--adapter",
        "software",
        "--allow-memory-bake",
        "--global-res",
        "128",
        "--views",
        "nope",
        "--out-dir",
        dir.to_str().unwrap(),
    ])
    .unwrap_err();
    assert!(e.contains("no flight 'nope'"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: TEST-027
#[test]
fn the_flight_capture_writes_a_png_sequence_per_flight_and_shore_filter_and_is_no_measurement() {
    let dir = tmp("capture");
    let out = run(&[
        "test-render",
        "--scene",
        "capture-flights",
        "--adapter",
        "software",
        "--allow-memory-bake",
        "--global-res",
        "128",
        "--size",
        "320x180",
        "--flight-frames",
        "2",
        "--views",
        "coast-glide,coast-descent",
        "--out-dir",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("capture-flights: NOT a measurement (no gate, no timestamps, no log)"), "{out}");
    let index = std::fs::read_to_string(dir.join("capture.txt")).unwrap();
    let names: Vec<&str> = index.lines().filter(|l| !l.starts_with('#')).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(names, ["coast-glide-filter1", "coast-glide-filter0", "coast-descent-filter1", "coast-descent-filter0"], "{index}");
    for n in names {
        for k in 0..2 {
            let p = dir.join(n).join(format!("frame_{k:04}.png"));
            let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{}", p.display());
            // IHDR: width and height, big endian, at bytes 16..24.
            assert_eq!(
                (u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap())),
                (320, 180)
            );
        }
    }
    assert!(!dir.join("lit.json").exists() && !out.contains("budget"), "a tool: no perf log, no budget row: {out}");
    // It needs an output directory and a world source, and a perf option is refused.
    let e = run(&["test-render", "--scene", "capture-flights", "--adapter", "software", "--allow-memory-bake", "--global-res", "128"])
        .unwrap_err();
    assert!(e.contains("--out-dir"), "{e}");
    let e = run(&["test-render", "--scene", "capture-flights", "--adapter", "software", "--out-dir", dir.to_str().unwrap()]).unwrap_err();
    assert!(e.contains("--store DIR") && e.contains("--allow-memory-bake"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
