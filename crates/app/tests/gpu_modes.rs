//! Tier C: the `test-render` mode end to end, and the performance harness refusing a software adapter.

use planet_testkit::read_png;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

// spec: BUILD-004, REND-007
#[test]
fn test_render_mode_writes_a_png() {
    let path = std::env::temp_dir().join(format!("planet-test-render-{}.png", std::process::id()));
    let out = run(&["test-render", "--adapter", "software", "--out", path.to_str().unwrap()]).unwrap();
    assert!(out.starts_with("adapter: "), "adapter line must come first: {out}");
    assert!(out.contains("Dx12") || out.contains("Vulkan"), "backend must be named: {out}");
    let img = read_png(&path).unwrap();
    assert_eq!((img.width, img.height), (256, 256));
    assert_eq!(img.pixel(128, 128), planet_render::HELLO_FILL);
}

// spec: TEST-005
#[test]
fn perf_harness_refuses_the_software_adapter() {
    let e = run(&["test-render", "--perf", "--adapter", "software", "--machine-ready"]).unwrap_err();
    assert!(e.contains("not a discrete GPU"), "{e}");
}

// spec: TEST-012
#[test]
fn the_terrain_perf_workload_refuses_unready_machines_and_the_smoke_run_is_labelled() {
    let e = run(&["test-render", "--perf", "--scene", "terrain", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("--machine-ready is required"), "{e}");
    let e = run(&["test-render", "--perf", "--scene", "terrain", "--adapter", "software", "--machine-ready"]).unwrap_err();
    assert!(e.contains("not a discrete GPU"), "{e}");
    let e = run(&["test-render", "--perf", "--perf-smoke", "--scene", "terrain", "--adapter", "software", "--machine-ready"]).unwrap_err();
    assert!(e.contains("exclude each other"), "{e}");
    let e = run(&["test-render", "--perf-smoke", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("only available for --scene terrain"), "{e}");
    // The default parameters (cells 32, tau 6) work in smoke mode, and the log, when kept, is marked invalid.
    let kept = std::env::temp_dir().join(format!("planet-smoke-{}.json", std::process::id()));
    let log =
        run(&["test-render", "--perf-smoke", "--scene", "terrain", "--adapter", "software", "--out", kept.to_str().unwrap()]).unwrap();
    let json = std::fs::read_to_string(&kept).unwrap();
    assert!(
        json.contains("\"valid_for_budgets\":false") && json.contains("smoke run") && json.contains("not the reference adapter"),
        "{json}"
    );
    assert!(log.contains("worst run-to-run noise within a view"), "{log}");
    assert!(log.contains("SMOKE RUN") && log.contains("not a measurement"), "{log}");
    assert!(log.contains("ground-1m") && log.contains("wrote"), "{log}");
    // Parameters the morph cannot support are rejected with advice before anything is measured.
    let e =
        run(&["test-render", "--perf-smoke", "--scene", "terrain", "--adapter", "software", "--cells", "256", "--tau", "6"]).unwrap_err();
    assert!(e.contains("morph interval"), "{e}");
}
