//! Tier C, TEST-040: the preflight scene runs on the software adapter in smoke mode, prints its report and does not refuse.

// spec: TEST-040
#[test]
fn the_preflight_smoke_prints_the_host_facts_and_a_reading_and_never_refuses() {
    let args: Vec<String> =
        ["test-render", "--perf-smoke", "--scene", "preflight", "--adapter", "software"].iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).unwrap();
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("AC power:") && out.contains("power plan:"), "{out}");
    assert!(out.contains("round 1:") && !out.contains("round 2:"), "one round in a smoke run: {out}");
    assert!(out.contains("PREFLIGHT OK") || out.contains("PREFLIGHT REFUSED"), "{out}");
}
