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

/// Coarse horizon chunks (about 80 cells an edge, over a run of 32 cells per face edge) keep the test quick; the poses are on the ground, where
/// a frame reads only a few chunks.
const POSES: &str = r#"
horizon_cell_m = 300
chunk_timeout_s = 300

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

    fn call(&self, extra: &[&str]) -> Result<String, String> {
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
    // spec: REND-038 — every pose waited for the horizon maps of the chunks near the eye and says so.
    assert!(out.contains("horizon maps in"), "{out}");
    let index = std::fs::read_to_string(dir.join("stills.txt")).unwrap();
    let names: Vec<&str> = index.lines().filter(|l| !l.starts_with('#')).map(|l| l.split_whitespace().next().unwrap()).collect();
    assert_eq!(names, ["ground-sunset", "ground-midday"], "{index}");
    for n in &names {
        assert_eq!(png_size(&dir.join(format!("{n}.png"))), (160, 90));
    }
    // Each pose waited for at least one horizon map.
    for line in index.lines().filter(|l| !l.starts_with('#')) {
        let maps: usize = line.split_whitespace().nth(9).unwrap().parse().unwrap();
        assert!(maps >= 1, "{line}");
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
