//! Tier C: the flight sequence for the owner's real-GPU jitter review (TEST-013). Runs on `PLANET_ADAPTER` (software by
//! default, the real GPU under `cargo xtask test gpu --real`).

use planet_testkit::{read_png, Image};
use std::path::{Path, PathBuf};

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

/// Output under `target/review/flight-test/` so a failing run leaves its frames where the review tooling looks.
fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/flight-test").join(format!("{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn flight(out: &Path) -> String {
    let o = out.to_str().unwrap();
    run(&["test-render", "--scene", "terrain", "--flight", "12", "--out-dir", o, "--size", "160x120", "--cells", "8", "--view", "level"])
        .unwrap()
}

/// Number of differing pixels, the first one and the largest channel delta.
fn delta(a: &Image, b: &Image) -> (usize, Option<(u32, u32)>, u8) {
    let (mut n, mut first, mut worst) = (0, None, 0u8);
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.pixel(x, y), b.pixel(x, y));
            if p != q {
                n += 1;
                first.get_or_insert((x, y));
                worst = worst.max((0..4).map(|c| p[c].abs_diff(q[c])).max().unwrap_or(0));
            }
        }
    }
    (n, first, worst)
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
        let (img, again) = (read_png(&a.join(&name)).unwrap(), read_png(&b.join(&name)).unwrap());
        assert_eq!((img.width, img.height), (160, 120), "{name}");
        let (n, first, worst) = delta(&img, &again);
        assert_eq!(
            n,
            0,
            "{name}: {n} pixels differ between two runs on one adapter, first at {first:?}, max delta {worst}; see {} and {}",
            a.display(),
            b.display()
        );
        frames.push(img);
    }
    assert_ne!(frames[0], frames[11], "orbit and 1 m frames must differ");
    for d in [a, b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

// spec: TEST-013
#[test]
fn a_size_that_fails_lod_validation_writes_no_frames() {
    let o = dir("bad-size");
    let e =
        run(&["test-render", "--scene", "terrain", "--flight", "12", "--out-dir", o.to_str().unwrap(), "--size", "160x90"]).unwrap_err();
    assert!(e.contains("morph interval"), "{e}");
    assert!(!o.exists(), "a rejected flight must not create {}", o.display());
}

/// Share of the terrain pixels (everything but the sky) that differ from the most common terrain colour: 0 for a solid
/// frame, towards 1 for a frame full of structure.
fn structure(img: &Image) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for p in img.pixels().filter(|p| *p != planet_frame::plan::CLEAR_COLOR) {
        *counts.entry(p).or_insert(0usize) += 1;
    }
    let total: usize = counts.values().sum();
    if total == 0 {
        return 0.0;
    }
    1.0 - *counts.values().max().unwrap() as f64 / total as f64
}

fn flight_structure(name: &str, extra: &[&str]) -> Vec<f64> {
    let d = dir(name);
    let mut args =
        vec!["test-render", "--scene", "terrain", "--flight", "30", "--out-dir", d.to_str().unwrap(), "--size", "160x120", "--cells", "8"];
    args.extend_from_slice(extra);
    run(&args).unwrap();
    let s: Vec<f64> = (0..30).map(|i| structure(&read_png(&d.join(format!("frame_{i:04}.png"))).unwrap())).collect();
    let _ = std::fs::remove_dir_all(d);
    s
}

/// A frame whose terrain is at least this structured is legible; the flat-colour views fall below it near the ground.
const LEGIBLE: f64 = 0.4;

// spec: TEST-017
#[test]
fn no_flight_frame_is_a_solid_colour_in_the_default_view() {
    let s = flight_structure("legible", &[]);
    let weakest = s.iter().copied().enumerate().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
    assert!(
        weakest.1 >= LEGIBLE,
        "frame {} of the default (relief) flight has structure {:.2} < {LEGIBLE}: it is nearly one colour, so it cannot show jitter, swimming or cracks; all frames {s:.2?}",
        weakest.0,
        weakest.1
    );
}

// spec: TEST-017
#[test]
fn the_check_has_teeth_the_tile_id_view_is_solid_at_one_metre() {
    // Negative control: if the measure stopped telling a flat-colour glide from a legible one, the test above would pass vacuously.
    let s = flight_structure("control", &["--view", "tile-id"]);
    let last = *s.last().unwrap();
    assert!(last < LEGIBLE, "the tile-id glide frame should be nearly solid, got structure {last:.2}; all frames {s:.2?}");
}

// spec: TEST-013, TEST-017
#[test]
fn a_flight_writes_a_labelled_contact_sheet_and_the_finest_level_per_frame() {
    let d = dir("sheet");
    run(&["test-render", "--scene", "terrain", "--flight", "30", "--out-dir", d.to_str().unwrap(), "--size", "160x120", "--cells", "8"])
        .unwrap();
    let sheet = read_png(&d.join("contact_sheet.png")).unwrap();
    // 12 thumbnails in 4 columns, 160 px wide (factor 1), 4 px gutters.
    assert_eq!((sheet.width, sheet.height), (4 * 160 + 5 * 4, 3 * 120 + 4 * 4));
    let index = std::fs::read_to_string(d.join("flight.txt")).unwrap();
    assert!(index.starts_with("# frame altitude_above_terrain_m nodes plan_hash finest_level"), "{index}");
    let rows: Vec<Vec<&str>> = index.lines().filter(|l| !l.starts_with('#')).map(|l| l.split(' ').collect()).collect();
    assert!(rows.iter().all(|r| r.len() == 5), "{index}");
    let level = |i: usize| rows[i][4].parse::<u8>().unwrap();
    assert!(level(29) > level(0), "the finest level must grow from orbit to the ground: {} -> {}", level(0), level(29));
    let _ = std::fs::remove_dir_all(d);
}
