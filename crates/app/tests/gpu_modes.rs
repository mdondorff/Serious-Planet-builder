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
