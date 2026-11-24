//! COORD-012: `CubePatch`, a tile plus its halo as one grid on the cube over `GlobalCell` and the fold (lead #195).
use planet_core::{CubePatch, Face, GlobalCell};
use std::collections::{BTreeMap, BTreeSet};

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

/// Patches of a level of `n` cells: every core position of `size` on every face.
fn patches(n: i64, size: i64, halo: i64) -> Vec<CubePatch> {
    let mut out = Vec::new();
    for f in Face::ALL {
        for j0 in 0..=n - size {
            for i0 in 0..=n - size {
                out.push(CubePatch::new(f, n, i0, j0, size, halo).unwrap());
            }
        }
    }
    out
}

/// One step straight on from `cur`, having come from its axis neighbour `prev`: the axis neighbour of `cur` opposite `prev` (across a cube
/// edge the step folds; an oracle by single axis steps, independent of the patch's multi-cell fold).
fn straight(n: i64, prev: GlobalCell, cur: GlobalCell) -> GlobalCell {
    let n4 = cur.neighbours4(n);
    let k = n4.iter().position(|&c| c == prev).expect("prev is an axis neighbour of cur");
    n4[k ^ 1]
}

/// Walks `steps` cells straight on from the line `prev -> cur`; returns the last cell (`cur` for no step).
fn walk(n: i64, mut prev: GlobalCell, mut cur: GlobalCell, steps: i64) -> GlobalCell {
    for _ in 0..steps {
        let next = straight(n, prev, cur);
        (prev, cur) = (cur, next);
    }
    cur
}

/// How far `c` lies outside `0..n` (0 inside).
fn out(n: i64, c: i64) -> i64 {
    if c < 0 {
        -c
    } else if c >= n {
        c - n + 1
    } else {
        0
    }
}

// spec: COORD-012
#[test]
fn a_patch_is_refused_outside_its_face_or_level() {
    assert!(CubePatch::new(Face(0), 8, 6, 0, 3, 2).is_none(), "a core over the face's edge");
    assert!(CubePatch::new(Face(0), 8, 0, 0, 0, 2).is_none() && CubePatch::new(Face(0), 8, -1, 0, 2, 2).is_none());
    assert!(CubePatch::new(Face(0), 8, 0, 0, 2, 9).is_none() && CubePatch::new(Face(0), 8, 0, 0, 2, -1).is_none(), "a halo over n");
    assert!(CubePatch::new(Face(6), 8, 0, 0, 2, 2).is_none());
    let p = CubePatch::new(Face(2), 8, 2, 3, 4, 2).unwrap();
    assert_eq!((p.edge(), p.cells().len(), p.get(-3, 0), p.get(5, 5).is_some()), (8, 64, None, true));
}

// spec: COORD-012
#[test]
fn the_interior_is_the_faces_cells_and_every_halo_cell_is_its_fold() {
    for (n, size, halo) in [(8i64, 3i64, 2i64), (9, 4, 4), (6, 2, 3), (12, 4, 5)] {
        for p in patches(n, size, halo) {
            let (f, (i0, j0)) = (p.face(), p.origin());
            let at = |u: i64, v: i64| p.cell(u, v);
            for v in -halo..size + halo {
                for u in -halo..size + halo {
                    let (i, j) = (i0 + u, j0 + v);
                    let (oi, oj) = (out(n, i), out(n, j));
                    let want = match (oi, oj) {
                        // inside the face (the core and the in-face halo): the face's own cell
                        (0, 0) => GlobalCell::new(f, n, i, j).unwrap(),
                        // across one edge: straight on from the face's border cell, one axis step at a time
                        (_, 0) => {
                            let (b, inner) = if i < 0 { (0, 1) } else { (n - 1, n - 2) };
                            walk(n, GlobalCell::new(f, n, inner, j).unwrap(), GlobalCell::new(f, n, b, j).unwrap(), oi)
                        }
                        (0, _) => {
                            let (b, inner) = if j < 0 { (0, 1) } else { (n - 1, n - 2) };
                            walk(n, GlobalCell::new(f, n, i, inner).unwrap(), GlobalCell::new(f, n, i, b).unwrap(), oj)
                        }
                        // past a cube corner: the half nearer the i strip continues that strip's column across the third edge, the other half the
                        // j strip's row; the diagonal takes one of the two
                        _ => {
                            let bj = if j < 0 { 0 } else { n - 1 };
                            let bi = if i < 0 { 0 } else { n - 1 };
                            let (vi, vj) = (v - (j - bj), u - (i - bi));
                            let step_j = if j < 0 { 1 } else { -1 };
                            let step_i = if i < 0 { 1 } else { -1 };
                            let from_i = walk(n, at(u, vi + step_j), at(u, vi), oj);
                            let from_j = walk(n, at(vj + step_i, v), at(vj, v), oi);
                            match oi.cmp(&oj) {
                                std::cmp::Ordering::Greater => from_i,
                                std::cmp::Ordering::Less => from_j,
                                std::cmp::Ordering::Equal => {
                                    assert!(at(u, v) == from_i || at(u, v) == from_j, "n {n}: a diagonal cell is one of its two folds");
                                    at(u, v)
                                }
                            }
                        }
                    };
                    assert_eq!(at(u, v), want, "n {n}, size {size}, halo {halo}: {f:?} core ({i0}, {j0}), patch cell ({u}, {v})");
                    if p.is_core(u, v) {
                        assert_eq!((want.face(), want.i(), want.j()), (f, i, j), "a core cell is the face's");
                    }
                }
            }
        }
    }
}

// spec: COORD-012
#[test]
fn a_corner_patch_has_no_hole_and_its_rows_stay_connected() {
    // a 3-face corner: every patch cell is a cell; the stitched quadrant repeats cells of the strips (270 degrees around a cube corner), and
    // within each half of the quadrant, and from the strips into it, neighbouring patch cells are axis neighbours on the cube
    let (n, size, halo) = (10i64, 4i64, 4i64);
    for f in Face::ALL {
        for (i0, j0) in [(0, 0), (n - size, 0), (0, n - size), (n - size, n - size)] {
            let p = CubePatch::new(f, n, i0, j0, size, halo).unwrap();
            // the half of the quadrant a cell lies in: 0 outside it, 1 nearer the i strip, 2 nearer the j strip, 3 the diagonal
            let half = |u: i64, v: i64| {
                let (oi, oj) = (out(n, i0 + u), out(n, j0 + v));
                match (oi, oj) {
                    (0, _) | (_, 0) => 0,
                    _ if oi > oj => 1,
                    _ if oi < oj => 2,
                    _ => 3,
                }
            };
            let mut quadrant = 0;
            let mut strips = BTreeSet::new();
            for v in -halo..size + halo {
                for u in -halo..size + halo {
                    if half(u, v) == 0 {
                        strips.insert(p.cell(u, v));
                    } else {
                        quadrant += 1;
                    }
                    for (du, dv) in [(1, 0), (0, 1)] {
                        let (hu, hv) = (half(u, v), half(u + du, v + dv));
                        if !p.contains(u + du, v + dv) || hu == 3 || hv == 3 || (hu != 0 && hv != 0 && hu != hv) {
                            continue;
                        }
                        let (a, b) = (p.cell(u, v), p.cell(u + du, v + dv));
                        assert!(a.neighbours4(n).contains(&b), "{f:?} ({i0}, {j0}): ({u}, {v}) {a:?} and its neighbour {b:?}");
                    }
                }
            }
            assert_eq!(quadrant, halo * halo, "one quadrant past the corner");
            assert_eq!(strips.len() as i64, p.edge() * p.edge() - quadrant, "the core and the strips hold each cell once");
            assert!(
                p.cells().iter().all(|c| strips.contains(c)),
                "{f:?} ({i0}, {j0}): every stitched cell is a strip's cell: no hole, nothing foreign"
            );
            // the three faces of the corner: the core's and the two across its edges
            let faces: BTreeSet<u8> = p.cells().iter().map(|c| c.face().0).collect();
            assert_eq!(faces.len(), 3, "{f:?} ({i0}, {j0}): a corner patch spans three faces");
        }
    }
}

// spec: COORD-012
#[test]
fn every_patch_commutes_with_all_24_cube_rotations() {
    let rots = rotations();
    assert_eq!(rots.len(), 24);
    let rot_cell = |m: &[[i64; 3]; 3], c: GlobalCell, n: i64| GlobalCell::of_centre_point(rotate(m, c.centre_point(n)), n).unwrap();
    for (n, size, halo) in [(6i64, 2i64, 3i64), (7, 3, 2)] {
        for p in patches(n, size, halo) {
            for m in &rots {
                // the rotated core: its cells' face and bounding box
                let core: Vec<GlobalCell> =
                    (0..size).flat_map(|v| (0..size).map(move |u| (u, v))).map(|(u, v)| rot_cell(m, p.cell(u, v), n)).collect();
                let face = core[0].face();
                assert!(core.iter().all(|c| c.face() == face));
                let (i0, j0) = (core.iter().map(|c| c.i()).min().unwrap(), core.iter().map(|c| c.j()).min().unwrap());
                let q = CubePatch::new(face, n, i0, j0, size, halo).unwrap();
                let by_point: BTreeMap<[i64; 3], GlobalCell> = (-halo..size + halo)
                    .flat_map(|v| (-halo..size + halo).map(move |u| (u, v)))
                    .map(|(u, v)| (q.extended_point(u, v), q.cell(u, v)))
                    .collect();
                for v in -halo..size + halo {
                    for u in -halo..size + halo {
                        let rp = rotate(m, p.extended_point(u, v));
                        let got = by_point.get(&rp).expect("the rotation maps the extended plane onto the rotated patch's");
                        assert_eq!(*got, rot_cell(m, p.cell(u, v), n), "n {n}: {:?} ({u}, {v}) under {m:?}", p.face());
                    }
                }
            }
        }
    }
}
