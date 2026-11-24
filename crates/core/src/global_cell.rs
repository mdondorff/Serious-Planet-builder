//! A cell of one level of the cube planet, by face and index (COORD-011, lead #193; the G3 port's C1 primitive). A level has `n` cells per face edge; the cell is
//! `(face, i, j)` with `0 <= i, j < n`, packed in a `u64` (face in bits 60-62, `j` in bits 30-59, `i` in bits 0-29), so the id orders by face, then row, then column.
//! Neighbours cross the twelve cube edges by [`crate::tile::fold_cell`] (exact integers on the cube); a step across an edge is an axis step, and no diagonal crosses an
//! edge or a corner (GEN-139). Additive: nothing calls it yet, and `TileId::neighbors8` keeps its own call of the same fold.

use crate::cube::Face;
use crate::tile::{fold_cell, int_axes};

/// The largest `n` a [`GlobalCell`] holds: 2^30 cells per face edge.
pub const MAX_CELLS_PER_EDGE: i64 = 1 << 30;

/// A cell `(face, i, j)` of a level with `n` cells per face edge (`n` is not stored: every call that needs it takes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalCell(pub u64);

impl GlobalCell {
    /// The cell `(face, i, j)`; refused (None) outside `0..n` or for `n` over [`MAX_CELLS_PER_EDGE`].
    pub fn new(face: Face, n: i64, i: i64, j: i64) -> Option<GlobalCell> {
        let ok = face.0 < 6 && n > 0 && n <= MAX_CELLS_PER_EDGE && (0..n).contains(&i) && (0..n).contains(&j);
        ok.then(|| GlobalCell((u64::from(face.0) << 60) | ((j as u64) << 30) | i as u64))
    }

    pub fn face(self) -> Face {
        Face((self.0 >> 60) as u8)
    }

    pub fn i(self) -> i64 {
        (self.0 & ((1 << 30) - 1)) as i64
    }

    pub fn j(self) -> i64 {
        ((self.0 >> 30) & ((1 << 30) - 1)) as i64
    }

    /// The cell `(i + di, j + dj)` of the same face, or across one cube edge by [`fold_cell`] when exactly one coordinate leaves `0..n` by one step; None when both
    /// leave (a diagonal across an edge or a corner, GEN-139) or a step is larger than one.
    pub fn step(self, n: i64, di: i64, dj: i64) -> Option<GlobalCell> {
        if di.abs() > 1 || dj.abs() > 1 {
            return None;
        }
        let (i, j) = (self.i() + di, self.j() + dj);
        let (in_i, in_j) = ((0..n).contains(&i), (0..n).contains(&j));
        match (in_i, in_j) {
            (true, true) => GlobalCell::new(self.face(), n, i, j),
            (false, false) => None,
            // across an edge only by an axis step: a diagonal one cell across the edge has no cube-adjacent cell (GEN-139)
            _ if di != 0 && dj != 0 => None,
            _ => {
                let (f, fi, fj) = fold_cell(self.face(), n, i, j);
                GlobalCell::new(f, n, fi, fj)
            }
        }
    }

    /// The four axis neighbours (east, west, north, south: `+i`, `-i`, `+j`, `-j`), across cube edges where they leave the face.
    pub fn neighbours4(self, n: i64) -> [GlobalCell; 4] {
        [(1, 0), (-1, 0), (0, 1), (0, -1)].map(|(di, dj)| self.step(n, di, dj).expect("an axis step always has a cube-adjacent cell"))
    }

    /// The 8-neighbourhood on the cube (GEN-139): inside a face all eight; a diagonal that would leave the face is dropped, so a cell on an edge has 6 (its in-face
    /// diagonals and the axis neighbour across) and a face's corner cell 5. In the order east, west, north, south, then the diagonals NE, NW, SE, SW where they exist.
    pub fn neighbours8(self, n: i64) -> Vec<GlobalCell> {
        let mut out: Vec<GlobalCell> = self.neighbours4(n).to_vec();
        out.extend([(1, 1), (-1, 1), (1, -1), (-1, -1)].iter().filter_map(|&(di, dj)| self.step(n, di, dj)));
        out
    }

    /// The exact cube point of the cell's centre, in half-cell units (a face's normal coordinate is `n`; its in-face coordinates are `2 i + 1 - n` and `2 j + 1 - n`).
    pub fn centre_point(self, n: i64) -> [i64; 3] {
        let (nrm, u, v) = int_axes(self.face());
        let (a, b) = (2 * self.i() + 1 - n, 2 * self.j() + 1 - n);
        [0, 1, 2].map(|k| n * nrm[k] + a * u[k] + b * v[k])
    }

    /// The cell whose centre is the cube point `p` (half-cell units, [`GlobalCell::centre_point`]), or None when `p` is no cell centre of a level with `n` cells.
    pub fn of_centre_point(p: [i64; 3], n: i64) -> Option<GlobalCell> {
        for g in Face::ALL {
            let (gn, gu, gv) = int_axes(g);
            let dot = |w: [i64; 3]| p[0] * w[0] + p[1] * w[1] + p[2] * w[2];
            if dot(gn) == n {
                let (a, b) = (dot(gu), dot(gv));
                if (a + n - 1) % 2 != 0 || (b + n - 1) % 2 != 0 {
                    return None;
                }
                return GlobalCell::new(g, n, (a + n - 1) / 2, (b + n - 1) / 2);
            }
        }
        None
    }
}
