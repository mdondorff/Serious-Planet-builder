//! Tier C, M8-V / TEST-047: the speed-envelope scene (`test-render --perf-smoke --scene envelope`) runs end to end on the software adapter
//! over an in-memory bake: one short level flight per altitude from an empty cache, the coarse share judged against the fresh plan of every
//! frame, and the log written with the four altitudes. The numbers are not a measurement (a smoke run never is).

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

// spec: TEST-047
#[test]
fn the_envelope_scene_runs_end_to_end_in_smoke_mode_and_writes_its_log_with_every_altitude() {
    let dir = std::env::temp_dir().join(format!("planet-envelope-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("envelope.json");
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "envelope",
        "--adapter",
        "software",
        // The software rasteriser takes every core of a small runner; below-normal generation workers then never finish a job.
        "--gen-priority",
        "normal",
        "--global-res",
        "128",
        "--out",
        log.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    let text = std::fs::read_to_string(&log).unwrap();
    for a in ["30", "100", "300", "1000"] {
        assert!(text.contains(&format!("\"altitude_m\": {a},")), "altitude {a} in {text}");
    }
    assert!(text.contains("\"smoke\": true") && text.contains("\"bound\": 0.01") && text.contains("\"coarse_share\""), "{text}");
    let bad = run(&["test-render", "--perf-smoke", "--scene", "envelope", "--adapter", "software", "--share-bound", "2"]);
    assert!(bad.unwrap_err().contains("--share-bound"), "a bound outside (0, 1) is refused");
    let _ = std::fs::remove_dir_all(&dir);
}
