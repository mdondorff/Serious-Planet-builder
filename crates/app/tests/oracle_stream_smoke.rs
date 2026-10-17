//! Tier B: the real streamer and pool on the software adapter, read for order-of-magnitude findings (TEST-035). A short scripted dive
//! (48 frames) from an empty cache over a 256-cell in-memory bake with 6 generation workers; not a performance verdict, a net for a
//! configuration cap (the M4 kind) between perf windows. Each run takes a few seconds.

use planet_perf::smoke::streaming_smoke_findings;

const WORKERS: usize = 6;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

/// The dive's log of a smoke run with `workers` generation workers.
fn dive_log(name: &str, workers: usize) -> String {
    let dir = std::env::temp_dir().join(format!("planet-stream-smoke-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let workers = workers.to_string();
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
        "--frames",
        "48",
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

// spec: TEST-035
#[test]
fn the_streamer_and_the_pool_of_a_short_dive_show_no_configuration_cap() {
    let log = dive_log("default", WORKERS);
    let findings = streaming_smoke_findings(&log, WORKERS);
    assert!(findings.is_empty(), "{findings:#?}\n{log}");
}

// spec: TEST-035
#[test]
fn a_streamer_capped_to_one_worker_is_found() {
    // The injected cap: the same dive with one worker where six are expected. The findings name the concurrency.
    let log = dive_log("capped", 1);
    let findings = streaming_smoke_findings(&log, WORKERS);
    assert!(findings.iter().any(|f| f.contains("load_running_max") && f.contains("6 workers")), "{findings:#?}\n{log}");
}
