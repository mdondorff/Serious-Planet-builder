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
        blocks_per_edge: 8,
        rounds: 2,
        threads,
        res: 64,
        base_id: 7,
        inputs_id: 11,
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
    assert_eq!(commit.tiles.len(), s.tiles().len(), "every tile the region meets is in the commit");
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let reader = RiverReader::new(store, commit.clone(), 64 << 20).expect("a reader of the commit");
    let (ri0, rj0, ri1, rj1) = (s.region.0 - s.margin, s.region.2 - s.margin, s.region.1 + s.margin, s.region.3 + s.margin);
    let l = LOCAL_PER_REGIONAL as f64;
    let (mut points, mut channels) = (0usize, 0usize);
    let tiles_per_edge = (s.local() / s.tile) as u32;
    for tj in 0..tiles_per_edge {
        for ti in 0..tiles_per_edge {
            for c in reader.tile_channels(Face(2), ti, tj).expect("a tile's channels").iter() {
                channels += 1;
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
    println!("region bake: the reader gives {channels} channel pieces with {points} points over {} tiles", s.tiles().len());
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
fn a_region_near_a_face_edge_or_with_a_bad_block_count_is_refused() {
    let w = world();
    let d = Dir::new("region-edge");
    let near = RegionSpec { region: (2, 20, 16, 39), margin: 1, ..spec(1) };
    let e = bake_region(&w, &near, &base, &climate, d.path()).expect_err("refused");
    assert!(e.contains("face edge"), "{e}");
    let blocks = RegionSpec { blocks_per_edge: 6, ..spec(1) };
    let e = bake_region(&w, &blocks, &base, &climate, d.path()).expect_err("refused");
    assert!(e.contains("power of two"), "{e}");
    assert!(planet_cache::l_writer::Commit::read(d.path()).unwrap().is_none(), "no commit after a refusal");
}
