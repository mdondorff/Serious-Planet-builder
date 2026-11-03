//! Tier C: the pose stills end to end on the software adapter over a small store (TEST-048).

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("planet-stills-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// Two poses on the ground of a small run (32 cells per face edge) keep the test quick.
const POSES: &str = r#"
[place]
anchor = "largest-river"

[[pose]]
name = "ground-sunset"
altitude_m = 1.8
back_m = 40
heading_deg = 90
pitch_deg = 0
sun_local_hours = 17.5

[[pose]]
name = "ground-midday"
altitude_m = 1.8
back_m = 40
heading_deg = 90
pitch_deg = 0
sun_local_hours = 12
"#;

fn png_size(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{}", path.display());
    (u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap()))
}

/// A small store of 32 cells per face edge, the poses file and the output directory of one test (each test makes its own: the two run in
/// one binary and a still takes the better part of a minute on the software adapter, so they are separate tests, TEST-048).
struct Fixture {
    store: std::path::PathBuf,
    dir: std::path::PathBuf,
    poses: std::path::PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Fixture {
        let f = Fixture { store: tmp(&format!("{tag}-store")), dir: tmp(&format!("{tag}-out")), poses: tmp(&format!("{tag}-poses.toml")) };
        std::fs::write(&f.poses, POSES).unwrap();
        run(&["generate", "--global-dir", f.store.to_str().unwrap(), "--global-res", "32", "--threads", "2"]).unwrap();
        f
    }

    /// A run with the clipmap shadow off: the subject of the tests that use it is the stills scene (poses, files, refusals), not the shadow, and the march
    /// more than doubles a software run (WARP CI). [`Fixture::call_with_march`] keeps the default.
    fn call(&self, extra: &[&str]) -> Result<String, String> {
        let mut extra = extra.to_vec();
        extra.push("--no-shadow-march");
        self.call_with_march(&extra)
    }

    fn call_with_march(&self, extra: &[&str]) -> Result<String, String> {
        let mut a = vec![
            "test-render",
            "--scene",
            "capture-stills",
            "--adapter",
            "software",
            "--store",
            self.store.to_str().unwrap(),
            "--global-res",
            "32",
            "--size",
            "160x90",
            // A tolerance this fine refines the smooth small bake to the pyramid's levels, where tiles read chunks.
            "--tau",
            "0.4",
            "--poses",
            self.poses.to_str().unwrap(),
            "--out-dir",
            self.dir.to_str().unwrap(),
        ];
        a.extend_from_slice(extra);
        run(&a)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.store);
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_file(&self.poses);
    }
}

// spec: TEST-048
#[test]
fn the_stills_scene_draws_each_pose_with_its_horizon_maps_baked() {
    let f = Fixture::new("all");
    let dir = &f.dir;
    let out = f.call(&[]).unwrap();
    assert!(out.contains("capture-stills: NOT a measurement (no gate, no timestamps, no log)"), "{out}");
    assert!(out.contains("anchor: latitude"), "{out}");
    assert!(out.contains("ground-sunset:") && out.contains("ground-midday:"), "{out}");
    let index = std::fs::read_to_string(dir.join("stills.txt")).unwrap();
    let names: Vec<&str> = index.lines().filter(|l| !l.starts_with('#')).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(names, ["ground-sunset", "ground-midday"], "{index}");
    for n in &names {
        assert_eq!(png_size(&dir.join(format!("{n}.png"))), (160, 90));
    }
    // Each line holds the plan's node count (the tenth word, index 9).
    for line in index.lines().filter(|l| !l.starts_with('#')) {
        let nodes: usize = line.split_whitespace().nth(9).unwrap().parse().unwrap();
        assert!(nodes >= 1, "{line}");
    }
    // The sun differs between the two poses: the images differ.
    assert_ne!(std::fs::read(dir.join("ground-sunset.png")).unwrap(), std::fs::read(dir.join("ground-midday.png")).unwrap());
    assert!(!dir.join("lit.json").exists() && !out.contains("budget"), "a tool: no perf log, no budget row: {out}");
}

// spec: TEST-048
#[test]
fn the_stills_scene_draws_one_pose_by_name_and_refuses_a_bad_call() {
    let f = Fixture::new("one");
    // One pose by name; an unknown name, a missing poses file or output directory, and a perf-only option are refused.
    let one = f.call(&["--views", "ground-midday"]).unwrap();
    assert!(one.contains("ground-midday:") && !one.contains("ground-sunset:"), "{one}");
    let e = f.call(&["--views", "nope"]).unwrap_err();
    assert!(e.contains("no pose 'nope'") && e.contains("ground-sunset"), "{e}");
    let e = run(&[
        "test-render",
        "--scene",
        "capture-stills",
        "--adapter",
        "software",
        "--store",
        f.store.to_str().unwrap(),
        "--out-dir",
        f.dir.to_str().unwrap(),
    ])
    .unwrap_err();
    assert!(e.contains("--poses"), "{e}");
    let e = f.call(&["--flight-frames", "3"]).unwrap_err();
    assert!(e.contains("--flight-frames"), "{e}");
    let e = run(&["test-render", "--scene", "lit", "--poses", f.poses.to_str().unwrap(), "--adapter", "software"]).unwrap_err();
    assert!(e.contains("--poses applies to the scene `capture-stills`"), "{e}");
}

/// A still shows what the viewer shows by default, the clipmap shadow (REND-051): every pose runs the shadow pass for its frames unless
/// `--no-shadow-march` turns it off. (The smoke world is smooth: that a low sun then casts the hills' shadows is the render goldens' check,
/// `gpu_ground_look`; here the wiring is what is checked.) The shadow only takes light away.
// spec: REND-051
#[test]
fn a_still_runs_the_clipmap_shadow_by_default_and_not_with_the_flag() {
    let f = Fixture::new("march");
    let png = |name: &str| -> Vec<u8> {
        planet_testkit::read_png(&f.dir.join(format!("{name}.png"))).unwrap_or_else(|e| panic!("{name}: {e}")).rgba
    };
    let off = f.call(&["--no-shadow-march"]).unwrap();
    assert!(off.contains("ground-sunset: ") && off.contains(" 0 shadow frames"), "{off}");
    let off_sunset = png("ground-sunset");
    // One pose with the march on: its six shadow frames are most of a software run (the test timed out at 302 s with both poses on WARP CI); the poses
    // share one code path, and the off run above names both.
    let on = f.call_with_march(&["--views", "ground-sunset"]).unwrap();
    assert!(on.contains("ground-sunset: ") && on.contains(" 6 shadow frames"), "the pose runs the shadow frames: {on}");
    let on_sunset = png("ground-sunset");
    assert_eq!(off_sunset.len(), on_sunset.len());
    let sum = |v: &[u8]| v.iter().map(|b| u64::from(*b)).sum::<u64>();
    assert!(sum(&on_sunset) <= sum(&off_sunset), "a shadow takes light away, it never adds any");
}
