//! Tier C: the flight sequence for the owner's real-GPU jitter review (TEST-013).

use planet_testkit::read_png;
use std::path::{Path, PathBuf};

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("planet-flight-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn flight(out: &Path) -> String {
    let o = out.to_str().unwrap();
    let args = [
        "test-render",
        "--scene",
        "terrain",
        "--flight",
        "12",
        "--out-dir",
        o,
        "--size",
        "160x120",
        "--cells",
        "8",
        "--view",
        "level",
        "--adapter",
        "software",
    ];
    run(&args).unwrap()
}

// spec: TEST-013
#[test]
fn the_flight_writes_a_deterministic_frame_sequence_and_an_index() {
    let (a, b) = (dir("a"), dir("b"));
    let log = flight(&a);
    assert!(log.contains("flight: wrote 12 frames") && log.contains("160x120, cells 8, tau 6 px, view level"), "{log}");
    flight(&b);
    let index = std::fs::read_to_string(a.join("flight.txt")).unwrap();
    let rows: Vec<&str> = index.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(rows.len(), 12, "{index}");
    assert!(rows[0].starts_with("0000 20000000.000 "), "{index}");
    assert!(rows[11].starts_with("0011 1.000 "), "{index}");
    assert_eq!(index, std::fs::read_to_string(b.join("flight.txt")).unwrap(), "the index is deterministic");
    let mut frames = Vec::new();
    for i in 0..12 {
        let name = format!("frame_{i:04}.png");
        let img = read_png(&a.join(&name)).unwrap();
        assert_eq!((img.width, img.height), (160, 120), "{name}");
        assert_eq!(img, read_png(&b.join(&name)).unwrap(), "{name} differs between two runs on one adapter");
        frames.push(img);
    }
    assert_ne!(frames[0], frames[11], "orbit and 1 m frames must differ");
    for d in [a, b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

// spec: TEST-013
#[test]
fn bad_flight_arguments_are_actionable() {
    let o = dir("bad");
    let p = o.to_str().unwrap();
    let e = run(&["test-render", "--scene", "terrain", "--flight", "5", "--out-dir", p, "--adapter", "software"]).unwrap_err();
    assert!(e.contains("at least 10 frames"), "{e}");
    let e = run(&["test-render", "--scene", "terrain", "--flight", "12", "--adapter", "software"]).unwrap_err();
    assert!(e.contains("needs --out-dir"), "{e}");
    let e = run(&["test-render", "--scene", "tiles", "--flight", "12", "--out-dir", p, "--adapter", "software"]).unwrap_err();
    assert!(e.contains("only available for --scene terrain"), "{e}");
    let e = run(&["test-render", "--scene", "terrain", "--flight", "12", "--out-dir", p, "--size", "160x90", "--adapter", "software"])
        .unwrap_err();
    assert!(e.contains("morph interval"), "{e}");
    assert!(run(&["test-render", "--size", "12x12"]).unwrap_err().contains("--size must be WxH"));
}
