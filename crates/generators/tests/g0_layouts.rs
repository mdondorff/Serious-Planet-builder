//! Tier A, TEST-151: TEST-139's partition layouts as data (`support/g0_layouts.txt`; lead #195): one, two and four regions, a shifted
//! seam and a seam on a face edge, regions of whole tiles of GEN-182's grid with halo 24, each tile a `CubePatch` (COORD-012).

use super::g0_fixture::fixture::*;
use planet_core::{CubePatch, Face, GlobalCell};
use planet_generators::bake::k7::{DISC_RADIUS_MAX, PROFILE_CARRY_MIN};
use std::collections::BTreeSet;

const N: i64 = L_N as i64;

/// The index of an L cell: `(face * N + j) * N + i`.
fn at(c: GlobalCell) -> usize {
    (c.face().0 as usize * L_N + c.j() as usize) * L_N + c.i() as usize
}

/// The tiles of a layout as patches, with their regions.
fn tiles(l: &PartitionLayout) -> Vec<(u8, CubePatch)> {
    let t = TILES_PER_FACE;
    let mut out = Vec::new();
    for f in 0..6 {
        for tj in 0..t {
            for ti in 0..t {
                let p = CubePatch::new(Face(f as u8), N, (ti * l.tile) as i64, (tj * l.tile) as i64, l.tile as i64, l.halo as i64)
                    .expect("a tile of the face with its halo");
                out.push((l.region(f, ti, tj), p));
            }
        }
    }
    out
}

/// The region of every L cell through the tiles' cores; asserts every cell lies in exactly one core.
fn region_of_cell(l: &PartitionLayout) -> Vec<u8> {
    let mut hits = vec![0u8; 6 * L_N * L_N];
    let mut region = vec![u8::MAX; 6 * L_N * L_N];
    for (r, p) in tiles(l) {
        for v in 0..p.size() {
            for u in 0..p.size() {
                let k = at(p.cell(u, v));
                hits[k] += 1;
                region[k] = r;
            }
        }
    }
    assert!(hits.iter().all(|&h| h == 1), "{}: every cell in exactly one tile's core (no gap, no overlap)", l.name);
    region
}

/// The pairs of axis neighbours on the cube that lie in two regions (each pair once, the smaller cell first).
fn seam_pairs(l: &PartitionLayout) -> Vec<(GlobalCell, GlobalCell)> {
    let region = region_of_cell(l);
    let mut out = Vec::new();
    for f in Face::ALL {
        for j in 0..N {
            for i in 0..N {
                let c = GlobalCell::new(f, N, i, j).unwrap();
                for b in c.neighbours4(N) {
                    if c < b && region[at(c)] != region[at(b)] {
                        out.push((c, b));
                    }
                }
            }
        }
    }
    out
}

// spec: TEST-151
#[test]
fn the_layouts_are_the_five_of_the_order_on_gen_182s_grid() {
    let ls = partition_layouts();
    let names: Vec<&str> = ls.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["one-region", "two-regions", "four-regions", "shifted-seam", "face-edge-seam"]);
    for l in &ls {
        // GEN-182: tile divides n and is a multiple of the base stride 4; halo >= DISC_RADIUS_MAX + PROFILE_CARRY_MIN (a tile with
        // channels) and at most n / 8; tile > 2 halo
        assert_eq!(l.tile, TILE_L, "{}", l.name);
        assert!(L_N.is_multiple_of(l.tile) && l.tile.is_multiple_of(4) && l.tile > 2 * l.halo, "{}", l.name);
        assert!(l.halo >= DISC_RADIUS_MAX as usize + PROFILE_CARRY_MIN && l.halo <= L_N / 8, "{}: halo {}", l.name, l.halo);
        let used: BTreeSet<u8> = l.region_of_tile.iter().copied().collect();
        assert_eq!(used, (0..l.regions as u8).collect(), "{}: regions 0 to {} each hold a tile", l.name, l.regions - 1);
    }
}

// spec: TEST-151
#[test]
fn every_layout_tiles_the_cube_with_no_gap_and_no_overlap() {
    for l in partition_layouts() {
        let region = region_of_cell(&l);
        assert!(region.iter().all(|&r| (r as usize) < l.regions), "{}", l.name);
        for (_, p) in tiles(&l) {
            assert_eq!(p.cells().len(), (l.tile + 2 * l.halo).pow(2), "{}: a whole patch", l.name);
        }
    }
}

// spec: TEST-151
#[test]
fn the_seams_lie_where_each_layout_says() {
    let ls = partition_layouts();
    let by = |name: &str| ls.iter().find(|l| l.name == name).unwrap();
    // two-regions is TEST-139's layout of whole tiles: each H1 cell's region is its tile's
    let two = by("two-regions");
    let old = &layouts()[1];
    let per = TILE_L / L_PER_H1;
    for (p, &r) in old.region_of_h1.iter().enumerate() {
        let (f, i, j) = (p / (H1_N * H1_N), p % H1_N, (p % (H1_N * H1_N)) / H1_N);
        assert_eq!(two.region(f, i / per, j / per), r, "H1 cell {p}");
    }
    // on face 0 the two-region seam is the tile line between columns 0 and 1; the shifted seam the one between columns 1 and 2
    let face0_lines = |l: &PartitionLayout| -> Vec<i64> {
        let mut v: Vec<i64> =
            seam_pairs(l).iter().filter(|(a, b)| a.face() == Face(0) && b.face() == Face(0)).map(|(a, _)| a.i()).collect();
        v.sort();
        v.dedup();
        v
    };
    assert_eq!(face0_lines(two), [TILE_L as i64 - 1]);
    assert_eq!(face0_lines(by("shifted-seam")), [2 * TILE_L as i64 - 1]);
    // the face-edge seam: every pair across it lies on two faces, and it holds face 0's whole border (4 edges of n cells)
    let edge = seam_pairs(by("face-edge-seam"));
    assert!(edge.iter().all(|(a, b)| a.face() != b.face()), "the seam runs on cube edges only");
    assert_eq!(edge.len() as i64, 4 * N);
    // one region has no seam; four regions have seams inside a face in both directions
    assert!(seam_pairs(by("one-region")).is_empty());
    let four = seam_pairs(by("four-regions"));
    assert!(four.iter().any(|(a, b)| a.face() == b.face() && a.j() == b.j()), "a seam across columns");
    assert!(four.iter().any(|(a, b)| a.face() == b.face() && a.i() == b.i()), "a seam across rows");
}
