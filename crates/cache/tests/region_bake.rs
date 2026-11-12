//! Tier A, GEN-177 and TEST-125: the region bake. A region of a small fixture face (analytic base: a ridge that falls to the sea, constant runoff) is baked to a commit with a channel graph; the commit names the graph,
//! the reader gives channels whose points lie on the regional cells' local centres inside the region's window, every tile the region meets is in the commit, two bakes and one and eight threads give the same bytes,
//! and a region near a face edge is refused.

use std::path::Path;
use std::sync::Arc;

use crate::l_writer::{world, Dir};
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{bake_region, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_read::RiverReader;
use planet_core::{Face, Vec3};

const R: f64 = 6_371_000.0;

fn spec(threads: usize) -> RegionSpec {
    RegionSpec {
        seed: 3,
        face: Face(2),
        radius_m: R,
        regional: 64,
        region: (16, 39, 16, 39),
        margin: 6,
        tile: 64,
        halo: 24,
        rounds: 2,
        threads,
        res: 64,
        base_id: 7,
        inputs_id: 11,
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: true,
    }
}

/// A ridge on the face's upper side that falls to the sea on the other, with a little relief across it.
fn base(d: Vec3) -> f64 {
    500.0 - 2400.0 * (d.x - 0.1) + 120.0 * libm::sin(40.0 * d.z)
}

fn climate(_: Vec3) -> (f64, f64, f64) {
    (1.0e-8, 0.0, 1.0)
}

/// Every file under `dir` with its bytes, by relative path.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(at).unwrap().map(|e| e.unwrap().path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

// spec: GEN-177, TEST-125
#[test]
fn a_fixture_region_bakes_to_a_commit_with_a_graph_whose_channels_lie_on_the_regional_cells() {
    let w = world();
    let d = Dir::new("region-a");
    let s = spec(2);
    let (commit, report) = bake_region(&w, &s, &base, &climate, d.path()).expect("the bake");
    println!(
        "region bake: window {:?}, {} channel cells in {} segments, {} lakes, {} tiles",
        report.window, report.channel_cells, report.segments, report.lakes, report.tiles
    );
    assert!(report.segments > 0 && report.channel_cells > 0, "the fixture has rivers");
    assert!(!commit.graph.is_empty() && commit.graph_h1_cells == 64, "the commit names the graph");
    assert!(
        !commit.tiles.is_empty() && commit.tiles.len() == report.tiles && report.tiles <= s.tiles().len(),
        "every land tile the region meets is in the commit"
    );
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let reader = RiverReader::new(store, commit.clone(), 64 << 20).expect("a reader of the commit");
    let (ri0, rj0, ri1, rj1) = (s.region.0 - s.margin, s.region.2 - s.margin, s.region.1 + s.margin, s.region.3 + s.margin);
    let l = LOCAL_PER_REGIONAL as f64;
    let (mut points, mut channels) = (0usize, 0usize);
    let tiles_per_edge = s.tiles_per_edge();
    // Every step of a channel the reader gives is a step of a channel that was burnt: a one-cell error in the graph's cell mapping or in the burn's would break the steps.
    let key = |a: (f64, f64), b: (f64, f64)| [a.0.to_bits(), a.1.to_bits(), b.0.to_bits(), b.1.to_bits()];
    let steps: std::collections::HashSet<[u64; 4]> =
        report.channels.iter().flat_map(|c| c.points.windows(2).map(|w| key(w[0], w[1]))).collect();
    assert!(!steps.is_empty());
    for tj in 0..tiles_per_edge {
        for ti in 0..tiles_per_edge {
            for c in reader.tile_channels(Face(2), ti, tj).expect("a tile's channels").iter() {
                channels += 1;
                for w in c.points.windows(2) {
                    assert!(steps.contains(&key(w[0], w[1])), "the step {:?} -> {:?} is not a step of a burnt channel", w[0], w[1]);
                }
                for &(x, y) in &c.points {
                    let (cx, cy) = ((x - (l - 1.0) / 2.0) / l, (y - (l - 1.0) / 2.0) / l);
                    points += 1;
                    assert!(
                        (cx - libm::round(cx)).abs() < 1e-9 && (cy - libm::round(cy)).abs() < 1e-9,
                        "a point at ({x}, {y}) is not a regional cell's local centre"
                    );
                    let (cx, cy) = (libm::round(cx) as usize, libm::round(cy) as usize);
                    assert!((ri0..=ri1).contains(&cx) && (rj0..=rj1).contains(&cy), "the cell ({cx}, {cy}) is outside the region's window");
                }
            }
        }
    }
    assert!(channels > 0 && points > 0, "the reader gives channels");
    println!("region bake: the reader gives {channels} channel pieces with {points} points over {} tiles", report.tiles);
}

// spec: GEN-177, TEST-125
#[test]
fn two_bakes_and_one_and_eight_threads_give_the_same_bytes() {
    let w = world();
    let (a, b, c) = (Dir::new("region-t1"), Dir::new("region-t1b"), Dir::new("region-t8"));
    bake_region(&w, &spec(1), &base, &climate, a.path()).unwrap();
    bake_region(&w, &spec(1), &base, &climate, b.path()).unwrap();
    bake_region(&w, &spec(8), &base, &climate, c.path()).unwrap();
    let (sa, sb, sc) = (snapshot(a.path()), snapshot(b.path()), snapshot(c.path()));
    assert!(!sa.is_empty());
    assert_eq!(sa, sb, "a second bake of the same region gives the same bytes");
    assert_eq!(sa, sc, "eight threads give the bytes of one");
}

// spec: GEN-177, TEST-125
#[test]
fn the_keys_name_the_region_and_its_margin() {
    let w = world();
    let (a, b) = (Dir::new("region-m6"), Dir::new("region-m7"));
    let (ca, _) = bake_region(&w, &spec(2), &base, &climate, a.path()).unwrap();
    let (cb, _) = bake_region(&w, &RegionSpec { margin: 7, ..spec(2) }, &base, &climate, b.path()).unwrap();
    assert_ne!(ca.id, cb.id, "two margins are two bakes");
    assert!(ca.tiles.iter().any(|(t, k)| cb.tiles.get(t).is_some_and(|o| o != k)), "some tile has another key");
}

// spec: GEN-177, TEST-125
#[test]
fn a_region_that_the_writer_or_the_reader_cannot_file_or_whose_tiles_leave_the_window_is_refused() {
    let w = world();
    let d = Dir::new("region-edge");
    let refuse = |s: RegionSpec, what: &str| {
        let e = bake_region(&w, &s, &base, &climate, d.path()).expect_err("refused");
        assert!(e.contains(what), "{e}");
    };
    refuse(RegionSpec { region: (2, 20, 16, 39), margin: 1, ..spec(1) }, "face edge");
    // 512 local cells in tiles of 128 are 4 a face edge, but 96 regional cells with tiles of 128 are 6: not a power of two.
    refuse(RegionSpec { regional: 96, region: (24, 59, 24, 59), margin: 8, tile: 128, ..spec(1) }, "power of two");
    refuse(RegionSpec { tile: 0, ..spec(1) }, "power of two");
    // A region that is not tile-aligned with a margin too small for the first tile's halo.
    refuse(RegionSpec { region: (17, 38, 17, 38), margin: 3, ..spec(1) }, "outside the solved window");
    assert!(planet_cache::l_writer::Commit::read(d.path()).unwrap().is_none(), "no commit after a refusal");
}

// spec: GEN-177, TEST-125
#[test]
fn the_production_constants_of_the_example_pass_the_checks_and_tiles_of_1024_do_not() {
    // The prototype region R3a of face 1 as the example's defaults: 98,304 local cells in 128 tiles of 768 (graph blocks of 96 H1 cells).
    let prod = RegionSpec {
        seed: 14,
        face: Face(1),
        radius_m: R,
        regional: 12_288,
        region: (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3),
        margin: 200,
        tile: 768,
        halo: 64,
        rounds: 4,
        threads: 4,
        res: 16_384,
        base_id: 1,
        inputs_id: 2,
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    };
    prod.check().expect("the example's defaults are a bakeable spec");
    assert_eq!(prod.tiles_per_edge(), 128);
    let e = RegionSpec { tile: 1024, ..prod }.check().expect_err("96 tiles a face edge cannot be filed");
    assert!(e.contains("power of two"), "{e}");
}

/// The fixture's base stretched to a 60 km span: a ridge of about 45 km over a deep sea.
fn tall_base(d: Vec3) -> f64 {
    30.0 * base(d)
}

// spec: GEN-177, GEN-175
#[test]
fn a_sixty_kilometre_region_takes_its_epsilon_from_the_world_and_passes_the_narrowing_guard_and_earth_keeps_the_profile() {
    use planet_cache::region_bake::solve_region;
    // Earth's span: the profile's epsilon and f32 rounds, as the literal 1e-3 was.
    let earth = spec(1).precision();
    assert!(!earth.f64_rounds && earth.epsilon_w == planet_generators::bake::carve::CarveParams::default().epsilon, "{earth:?}");
    // A world of 60 km: the epsilon exceeds the profile's (the f32 spacing at 60 km is 3.9 mm).
    let tall = RegionSpec { height_bounds: (-20_000.0, 60_000.0), ..spec(1) };
    let p = tall.precision();
    assert!(!p.f64_rounds && p.epsilon_w > planet_generators::bake::carve::CarveParams::default().epsilon, "{p:?}");
    // The regional solve of the 40 km ridge runs: the rounds' narrowing guard (an epsilon over the f32 spacing at the summit) holds under the world's epsilon, where the literal 1e-3 would have refused it.
    let solved = solve_region(&tall, &tall_base).expect("the 60 km region solves");
    let top = solved.zs.iter().fold(0.0f64, |m, h| m.max(h.abs()));
    assert!(top > 16_384.0 && top <= tall.height_bounds.1, "the surface reaches the tall heights: {top} m");
    // A world whose bounds do not hold the surface is refused with the cell named, before any commit.
    let small = RegionSpec { height_bounds: (-100.0, 100.0), ..spec(1) };
    let e = solve_region(&small, &base).expect_err("a surface above the bounds is refused");
    assert!(e.contains("outside the world's height bounds") && e.contains("GEN-175"), "{e}");
    // The commit of a world with another epsilon is refused up front, naming the question.
    let d = Dir::new("region-tall");
    let w = world();
    let e = bake_region(&w, &tall, &tall_base, &climate, d.path()).expect_err("not committable yet");
    // The message names R and both epsilons (the world's 3.9 mm and K7's 1 mm) and the question.
    assert!(
        e.contains("R = 60000 m") && e.contains("0.0039") && e.contains("K7 epsilon 0.001 m") && e.contains("records no flood epsilon"),
        "{e}"
    );
    assert!(planet_cache::l_writer::Commit::read(d.path()).unwrap().is_none());
}

// spec: GEN-177, GEN-175
#[test]
fn a_world_of_huge_bounds_runs_f64_rounds_and_commits_at_the_profile_epsilon_and_a_round_that_overshoots_is_an_error() {
    // R of 150 km: the f32 epsilon exceeds the named maximum, so the rounds run in f64 at the profile's epsilon: the commit spec describes it.
    let huge = RegionSpec { height_bounds: (-150_000.0, 150_000.0), ..spec(1) };
    let p = huge.precision();
    assert!(p.f64_rounds && p.epsilon_w == planet_generators::bake::carve::CarveParams::default().epsilon, "{p:?}");
    let (a, b) = (Dir::new("region-f64-a"), Dir::new("region-f64-b"));
    let w = world();
    let (ca, ra) = bake_region(&w, &huge, &base, &climate, a.path()).expect("an f64 world commits");
    assert!(ra.precision_mark.contains("rounds=f64"), "{}", ra.precision_mark);
    let (cb, _) = bake_region(&w, &RegionSpec { threads: 8, ..huge }, &base, &climate, b.path()).unwrap();
    assert_eq!(ca.id, cb.id, "the same bytes at one and eight threads");
    assert_ne!(ca.id, bake_region(&w, &spec(1), &base, &climate, Dir::new("region-f64-c").path()).unwrap().0.id, "the bounds are keyed");
    // Bounds that do not hold the base: refused at once, naming the stage (certain: the fixture base reaches about 1 km, above R = 300 m). A round that lifts a surface past bounds that held the base is returned the same way (the rounds' narrowing
    // assert is caught): which bounds that takes depends on the carve, so this test asserts only the certain case.
    let tight = RegionSpec { height_bounds: (-300.0, 300.0), ..spec(1) };
    let e = solve_region_for_test(&tight).expect_err("a base above the bounds is refused");
    assert!(e.contains("the base of the region") && e.contains("height bounds"), "{e}");
}

fn solve_region_for_test(s: &RegionSpec) -> Result<(), String> {
    planet_cache::region_bake::solve_region(s, &base).map(|_| ())
}
