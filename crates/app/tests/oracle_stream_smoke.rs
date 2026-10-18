//! Tier B: the real streamer and pool on the software adapter, read for order-of-magnitude findings (TEST-035). A short scripted dive
//! (48 frames) from an empty cache over a 256-cell in-memory bake with 6 generation workers; not a performance verdict, a net for a
//! configuration cap (the M4 kind) between perf windows. Each run takes a few seconds.
//!
//! The dive runs at the production render floor 3 (the 384 floor tiles are loaded before the first frame; the compact format, the only one,
//! starts at level 3, so a floor below draws nothing, ADR 0062): the net judges the streamer's and the pool's concurrency over the dive's
//! own tile jobs. Before `remove-vertex-render-path` it ran at floor 0 because the roots loaded at start-up cost every run again and the
//! first frames from orbit asked the streamer for no work; the liveness condition below is unchanged and held at floor 3 in the first run.
//! Tiles have 16 cells (the roots cost a quarter of cells 32 at start-up; on the CI runner 384 roots at cells 32 took 58 s and used the whole liveness cap).
//! The start at floor 3 has its own test (LOD-011, STRM-001).

use planet_perf::smoke::{liveness_met, streaming_smoke_findings};
use std::time::{Duration, Instant};

const WORKERS: usize = 6;
/// The dive starts at this many frames and doubles while the streamer has not shown it is alive.
const FRAMES: usize = 48;
/// The wall-time cap of the liveness run (a generous 60 s, 20 times a quiet run): its expiry is the failure.
const CAP: Duration = Duration::from_secs(60);

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

/// The dive's log of a smoke run of `frames` frames with `workers` generation workers, at the production render floor 3 (a floor below the compact
/// format's first level draws nothing, ADR 0062).
fn dive_log(name: &str, workers: usize, frames: usize) -> String {
    dive_log_at(name, workers, frames, "3")
}

/// The same at render floor `floor` (`--floor-level`).
fn dive_log_at(name: &str, workers: usize, frames: usize, floor: &str) -> String {
    let dir = std::env::temp_dir().join(format!("planet-stream-smoke-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (workers, frames) = (workers.to_string(), frames.to_string());
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "dive",
        "--adapter",
        "software",
        "--allow-memory-bake",
        "--global-res",
        "256",
        "--floor-level",
        floor,
        "--cells",
        "16",
        "--frames",
        &frames,
        "--gen-workers",
        &workers,
        "--out",
        dir.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    let log = std::fs::read_to_string(dir.join("dive.json")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    log
}

/// The dive run until the streamer is alive ([`liveness_met`]): 48 frames, then twice as many while it has finished fewer than 3 jobs or
/// made fewer than 3 slots resident, until [`CAP`] runs out. The expiry is not hidden: the last log goes to the findings, which name the
/// missing jobs. The software renderer paces the dive, so a loaded runner can finish 2 jobs in 48 frames; the test judges liveness,
/// not speed (a determinism fix: the condition is the same, only the stopping rule changed).
fn dive_until_alive(name: &str, workers: usize) -> String {
    let started = Instant::now();
    let mut frames = FRAMES;
    loop {
        let log = dive_log(name, workers, frames);
        if liveness_met(&log) || started.elapsed() > CAP {
            return log;
        }
        frames *= 2;
    }
}

// spec: TEST-035
#[test]
fn the_streamer_and_the_pool_of_a_short_dive_show_no_configuration_cap() {
    let log = dive_until_alive("default", WORKERS);
    let findings = streaming_smoke_findings(&log, WORKERS);
    assert!(findings.is_empty(), "{findings:#?}\n{log}");
}

// spec: TEST-035
#[test]
fn a_streamer_capped_to_one_worker_is_found() {
    // The injected cap: the same dive with one worker where six are expected. The findings name the concurrency.
    let log = dive_log("capped", 1, FRAMES);
    let findings = streaming_smoke_findings(&log, WORKERS);
    assert!(findings.iter().any(|f| f.contains("load_running_max") && f.contains("6 workers")), "{findings:#?}\n{log}");
}

/// A count of the first run of a smoke log.
fn count(log: &str, name: &str) -> Option<u64> {
    log.split(&format!("\"{name}\":")).nth(1).and_then(|t| t.split([',', '}']).next()).and_then(|n| n.parse().ok())
}

// spec: TEST-035, STRM-001, LOD-011
#[test]
fn the_floor_3_start_loads_the_384_roots_on_the_pool_before_the_first_frame_and_draws_no_blank() {
    // The production start: the real pool loads the 384 level-3 tiles before frame 0 (the dive's own tile jobs begin later); a few frames from orbit draw them and nothing is blank.
    let log = dive_log_at("floor3", WORKERS, 24, "3");
    assert_eq!(count(&log, "load_startup_roots"), Some(384), "{log}");
    assert!(count(&log, "max_resident_slots").is_some_and(|n| n >= 384), "the roots are resident: {log}");
    assert_eq!(count(&log, "blank_frames"), Some(0), "no blank frame while the roots are resident: {log}");
}
