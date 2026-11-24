//! COORD-011: `GlobalCell` and the public `fold_cell` on the cube (lead #193).
use planet_core::global_cell::MAX_CELLS_PER_EDGE;
use planet_core::{Face, GlobalCell, TileId};
use std::collections::BTreeSet;

fn cells(n: i64) -> Vec<GlobalCell> {
    let mut out = Vec::new();
    for f in Face::ALL {
        for j in 0..n {
            for i in 0..n {
                out.push(GlobalCell::new(f, n, i, j).unwrap());
            }
        }
    }
    out
}

/// The 24 rotations of the cube: the signed permutation matrices of determinant +1.
fn rotations() -> Vec<[[i64; 3]; 3]> {
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut out = Vec::new();
    for p in perms {
        for s in 0..8 {
            let mut m = [[0i64; 3]; 3];
            for (r, &c) in p.iter().enumerate() {
                m[r][c] = if s >> r & 1 == 1 { -1 } else { 1 };
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if det == 1 {
                out.push(m);
            }
        }
    }
    out
}

fn rotate(m: &[[i64; 3]; 3], p: [i64; 3]) -> [i64; 3] {
    [0, 1, 2].map(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2])
}

// spec: COORD-011
#[test]
fn a_cell_packs_face_j_and_i_and_refuses_what_lies_outside_its_level() {
    let c = GlobalCell::new(Face(5), 1 << 20, 123_456, 654_321).unwrap();
    assert_eq!((c.face(), c.i(), c.j()), (Face(5), 123_456, 654_321));
    assert_eq!(c.0 >> 63, 0, "bit 63 stays reserved");
    let top = GlobalCell::new(Face(5), MAX_CELLS_PER_EDGE, MAX_CELLS_PER_EDGE - 1, MAX_CELLS_PER_EDGE - 1).unwrap();
    assert_eq!(
        (top.i(), top.j(), top.0 >> 63),
        (MAX_CELLS_PER_EDGE - 1, MAX_CELLS_PER_EDGE - 1, 0),
        "30 bits an axis, bit 63 free at the limit"
    );
    assert!(GlobalCell::new(Face(0), 4, 4, 0).is_none() && GlobalCell::new(Face(0), 4, 0, -1).is_none());
    assert!(GlobalCell::new(Face(6), 4, 0, 0).is_none() && GlobalCell::new(Face(0), MAX_CELLS_PER_EDGE + 1, 0, 0).is_none());
}

// spec: COORD-011
#[test]
fn the_id_order_is_face_then_row_then_column_the_seam_engines_tie_key() {
    // lead #193: the u64 order of GlobalCell is (face, j, i), the canonical tie key of the seam-pair spill engine (GEN-191)
    let n = 5;
    let mut by_id = cells(n);
    by_id.sort();
    let mut by_tuple = cells(n);
    by_tuple.sort_by_key(|c| (c.face().0, c.j(), c.i()));
    assert_eq!(by_id, by_tuple);
    assert!(GlobalCell::new(Face(0), n, 4, 0).unwrap() < GlobalCell::new(Face(0), n, 0, 1).unwrap(), "a row before the next");
    assert!(GlobalCell::new(Face(0), n, 4, 4).unwrap() < GlobalCell::new(Face(1), n, 0, 0).unwrap(), "a face before the next");
}

// spec: COORD-011
#[test]
fn neighbours_are_symmetric_and_no_diagonal_crosses_an_edge_or_a_corner() {
    for n in [1, 2, 4, 5] {
        for c in cells(n) {
            let n4 = c.neighbours4(n);
            for b in n4 {
                assert!(b.neighbours4(n).contains(&c), "n {n}: {c:?}'s axis neighbour {b:?} has it back");
            }
            let n8 = c.neighbours8(n);
            let set: BTreeSet<GlobalCell> = n8.iter().copied().collect();
            assert_eq!(set.len(), n8.len(), "no neighbour twice");
            // a neighbour on another face is an axis neighbour (GEN-139)
            assert!(n8.iter().filter(|b| b.face() != c.face()).all(|b| n4.contains(b)), "n {n}: {c:?}");
            if n >= 3 {
                let edge = |x: i64| x == 0 || x == n - 1;
                let want = match (edge(c.i()), edge(c.j())) {
                    (false, false) => 8,
                    (true, true) => 5,
                    _ => 6,
                };
                assert_eq!(n8.len(), want, "n {n}: {c:?}");
            }
            for b in &n8 {
                assert!(b.neighbours8(n).contains(&c), "n {n}: the 8-neighbourhood is symmetric ({c:?}, {b:?})");
            }
        }
    }
}

// spec: COORD-011
#[test]
fn every_cell_and_its_neighbours_commute_with_all_24_cube_rotations() {
    let rots = rotations();
    assert_eq!(rots.len(), 24);
    for n in [4, 5] {
        for m in &rots {
            let r = |c: GlobalCell| {
                GlobalCell::of_centre_point(rotate(m, c.centre_point(n)), n).expect("a rotation maps cell centres to cell centres")
            };
            for c in cells(n) {
                assert_eq!(GlobalCell::of_centre_point(c.centre_point(n), n), Some(c), "the centre round-trips");
                let rc = r(c);
                let a: BTreeSet<GlobalCell> = c.neighbours8(n).into_iter().map(r).collect();
                let b: BTreeSet<GlobalCell> = rc.neighbours8(n).into_iter().collect();
                assert_eq!(a, b, "n {n}: the neighbourhood of {c:?} rotates with it");
                let a4: BTreeSet<GlobalCell> = c.neighbours4(n).into_iter().map(r).collect();
                let b4: BTreeSet<GlobalCell> = rc.neighbours4(n).into_iter().collect();
                assert_eq!(a4, b4);
            }
        }
    }
}

// spec: COORD-011
#[test]
fn at_each_cube_corner_the_three_corner_cells_are_pairwise_axis_neighbours() {
    for n in [1, 3, 4] {
        let mut corners = 0;
        for c in cells(n).into_iter().filter(|c| (c.i() == 0 || c.i() == n - 1) && (c.j() == 0 || c.j() == n - 1)) {
            // the corner cell's two neighbours across its two edges, on the two other faces of that cube corner
            let across: Vec<GlobalCell> = c.neighbours4(n).into_iter().filter(|b| b.face() != c.face()).collect();
            if n > 1 {
                assert_eq!(across.len(), 2, "n {n}: {c:?}");
                assert_ne!(across[0].face(), across[1].face());
                assert!(across[0].neighbours4(n).contains(&across[1]), "n {n}: the three corner cells close a triangle ({c:?})");
            }
            corners += 1;
        }
        assert_eq!(corners, if n == 1 { 6 } else { 24 }, "4 corner cells a face");
    }
}

// spec: COORD-011
#[test]
fn global_cells_and_tile_neighbours_fold_alike() {
    // TileId::neighbors8 and GlobalCell use the one fold_cell: at a level, a tile's axis neighbours across an edge are the GlobalCell axis neighbours of its cell
    for level in 1..=3u8 {
        let n = 1i64 << level;
        for f in Face::ALL {
            for y in 0..n as u32 {
                for x in 0..n as u32 {
                    let t: BTreeSet<(u8, u32, u32)> =
                        TileId::new(f, level, x, y).unwrap().neighbors8().into_iter().map(|t| (t.face().0, t.x(), t.y())).collect();
                    let g = GlobalCell::new(f, n, i64::from(x), i64::from(y)).unwrap();
                    for b in g.neighbours8(n) {
                        assert!(t.contains(&(b.face().0, b.i() as u32, b.j() as u32)), "level {level}: {g:?}'s {b:?}");
                    }
                }
            }
        }
    }
}
