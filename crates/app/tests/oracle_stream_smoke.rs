//! Tier B: the real streamer and pool on the software adapter, read for order-of-magnitude findings (TEST-035). A short scripted dive
//! (48 frames) from an empty cache over a 256-cell in-memory bake with 6 generation workers; not a performance verdict, a net for a
//! configuration cap (the M4 kind) between perf windows. Each run takes a few seconds.
//!
//! The dive runs at render floor 0 (`--floor-level 0`, the six face tiles are the roots): the net judges the streamer's and the pool's
//! concurrency, which the floor does not change, and at floor 3 the 384 floor tiles are loaded before the first frame (a start-up
//! cost every run pays again) and the first frames of the dive from orbit need no job beyond them, so a short dive would not ask the
//! streamer for work. The start at floor 3 has its own tests (LOD-011, STRM-001).

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

/// The dive's log of a smoke run of `frames` frames with `workers` generation workers.
fn dive_log(name: &str, workers: usize, frames: usize) -> String {
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
        "0",
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
