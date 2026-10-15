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

/// Structure of a frame: the share of its terrain pixels (those whose right and lower neighbours are not the sky too) that
/// sit next to a clearly different pixel (a channel differs by at least 12). A solid colour or a few big flat tiles score
/// near 0; a grid, contours or shading gradients score higher.
fn structure(img: &Image) -> f64 {
    let clear = planet_frame::plan::CLEAR_COLOR;
    let differs = |a: [u8; 4], b: [u8; 4]| (0..3).any(|c| a[c].abs_diff(b[c]) >= 12);
    let (mut terrain, mut edges) = (0usize, 0usize);
    for y in 0..img.height - 1 {
        for x in 0..img.width - 1 {
            let (p, r, d) = (img.pixel(x, y), img.pixel(x + 1, y), img.pixel(x, y + 1));
            if p == clear || r == clear || d == clear {
                continue;
            }
            terrain += 1;
            if differs(p, r) || differs(p, d) {
                edges += 1;
            }
        }
    }
    if terrain == 0 {
        0.0
    } else {
        edges as f64 / terrain as f64
    }
}

fn flight_structure(name: &str, extra: &[&str]) -> Vec<f64> {
    let d = dir(name);
    let mut args =
        vec!["test-render", "--scene", "terrain", "--flight", "30", "--out-dir", d.to_str().unwrap(), "--size", "768x432", "--cells", "32"];
    args.extend_from_slice(extra);
    run(&args).unwrap();
    let s: Vec<f64> = (0..30).map(|i| structure(&read_png(&d.join(format!("frame_{i:04}.png"))).unwrap())).collect();
    let _ = std::fs::remove_dir_all(d);
    s
}

// spec: TEST-013
#[test]
fn the_default_relief_flight_is_deterministic_including_the_contact_sheet() {
    let (a, b) = (dir("rel-a"), dir("rel-b"));
    for d in [&a, &b] {
        run(&[
            "test-render",
            "--scene",
            "terrain",
            "--flight",
            "12",
            "--out-dir",
            d.to_str().unwrap(),
            "--size",
            "160x120",
            "--cells",
            "8",
        ])
        .unwrap();
    }
    for name in ["contact_sheet.png", "frame_0000.png", "frame_0006.png", "frame_0011.png", "flight.txt"] {
        assert_eq!(std::fs::read(a.join(name)).unwrap(), std::fs::read(b.join(name)).unwrap(), "{name} differs between two runs");
    }
    for d in [a, b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A frame at least this structured is legible; the flat-colour views fall below it near the ground.
const LEGIBLE: f64 = 0.10;

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

// spec: TEST-013
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

// spec: REND-016, TEST-013
#[test]
fn the_flight_from_compact_tiles_matches_the_vertex_reference_frame_by_frame() {
    let (compact, vertex) = (dir("compact"), dir("vertex"));
    for (out, format) in [(&compact, "compact"), (&vertex, "vertex")] {
        let log = run(&[
            "test-render",
            "--scene",
            "terrain",
            "--flight",
            "12",
            "--out-dir",
            out.to_str().unwrap(),
            "--size",
            "160x120",
            "--cells",
            "8",
            "--view",
            "relief",
            "--tile-format",
            format,
        ])
        .unwrap();
        assert!(log.contains(&format!("tile format: {format}")), "{log}");
    }
    assert_eq!(
        std::fs::read_to_string(compact.join("flight.txt")).unwrap(),
        std::fs::read_to_string(vertex.join("flight.txt")).unwrap(),
        "the plans do not depend on the tile format"
    );
    let (mut worst, mut worst_shaded) = (0f64, 0f64);
    for i in 0..12 {
        let name = format!("frame_{i:04}.png");
        let (a, b) = (read_png(&compact.join(&name)).unwrap(), read_png(&vertex.join(&name)).unwrap());
        let (mut wrong, mut shaded) = (0usize, 0usize);
        for y in 0..a.height {
            for x in 0..a.width {
                let (p, q) = (a.pixel(x, y), b.pixel(x, y));
                if (0..3).any(|c| p[c].abs_diff(q[c]) > 12) {
                    wrong += 1;
                }
                // A normal decode error of a degree shifts the shading by a few levels over whole slopes; 16-bit normals stay at about one.
                if (0..3).any(|c| p[c].abs_diff(q[c]) > 3) {
                    shaded += 1;
                }
            }
        }
        worst = worst.max(wrong as f64 / f64::from(a.width * a.height));
        worst_shaded = worst_shaded.max(shaded as f64 / f64::from(a.width * a.height));
    }
    println!("flight, compact against vertex: worst fraction of pixels over 12 levels {worst:.5}");
    println!("flight, compact against vertex: worst fraction over 3 levels {worst_shaded:.5}");
    assert!(worst <= 0.001, "{worst} of a flight frame's pixels differ between the compact and the vertex format");
    assert!(worst_shaded <= 0.002, "{worst_shaded} of a flight frame's pixels differ by more than 3 levels: a normal decode error?");
    for d in [compact, vertex] {
        let _ = std::fs::remove_dir_all(d);
    }
}
