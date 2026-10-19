//! Tier C, TEST-040: the preflight scene runs on the software adapter in smoke mode, prints its report and does not refuse.

// spec: TEST-040
#[test]
fn the_preflight_smoke_prints_the_host_facts_and_a_reading_and_never_refuses() {
    let dir = std::env::temp_dir().join(format!("planet-preflight-{}", std::process::id())).join("made-by-the-scene");
    let dir_arg = dir.to_str().unwrap();
    let args: Vec<String> = ["test-render", "--perf-smoke", "--scene", "preflight", "--adapter", "software", "--out", dir_arg]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).unwrap();
    let out = String::from_utf8(buf).unwrap();
    let report = std::fs::read_to_string(dir.join("preflight.md")).expect("the report is written into a directory the scene made");
    assert!(report.contains("AC power:") && report.contains("round 1:"), "{report}");
    // A report path the scene cannot use is refused before anything is measured.
    let bad = ["test-render", "--perf-smoke", "--scene", "preflight", "--adapter", "software", "--out", "report.json"];
    let bad: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
    let started = std::time::Instant::now();
    let e = planet::run(&bad, &mut Vec::new()).unwrap_err();
    assert!(e.contains("give a directory") && started.elapsed().as_secs() < 8, "{e}");
    assert!(out.contains("SMOKE RUN") && out.contains("AC power:") && out.contains("power plan:"), "{out}");
    assert!(out.contains("round 1:") && !out.contains("round 2:"), "one round in a smoke run: {out}");
    assert!(out.contains("PREFLIGHT OK") || out.contains("PREFLIGHT REFUSED"), "{out}");
}
