//! A tile plus its halo as one grid on the cube (COORD-012, lead #195; the G3 port order's C2 primitive). The patch is a square core of
//! `size x size` cells of one face of a level with `n` cells per face edge, at `(i0, j0)`, and a halo of `halo` cells on every side; every
//! patch cell names a [`GlobalCell`], so ties between cells of a patch break canonically (CON-30). Cells beyond a face edge are the neighbour
//! face's, folded over the edge by [`crate::tile::fold_cell`]'s rule (axis-only, GEN-139). Cells beyond a cube corner (both coordinates outside
//! the face) are stitched from the two other faces of that corner: the deeper coordinate is folded first, so each half of the missing quadrant
//! continues the strip it adjoins across the third edge; on the quadrant's diagonal the fold order is fixed by the corner's orientation
//! (`det(n, n_first, n_second) > 0`), which every rotation of the cube keeps. A corner patch has no hole; its stitched cells are cells the
//! patch already holds in its strips (a cube corner has 270 degrees, not 360), each once more. Additive: nothing calls it yet.

use crate::cube::Face;
use crate::global_cell::{GlobalCell, MAX_CELLS_PER_EDGE};
use crate::tile::{fold_cell_axis, int_axes};

/// A tile plus its halo on the cube: the cells `(u, v)`, `-halo <= u, v < size + halo`, of the core's face extended over its edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubePatch {
    face: Face,
    n: i64,
    i0: i64,
    j0: i64,
    size: i64,
    halo: i64,
    cells: Vec<GlobalCell>,
}

impl CubePatch {
    /// The patch of the core `[i0, i0 + size) x [j0, j0 + size)` of `face` (a level of `n` cells per face edge) with `halo` cells a side;
    /// refused (None) when the core leaves the face, `size` is not positive, `halo` is negative or over `n` (the fold reaches one face deep),
    /// or `n` is out of range.
    pub fn new(face: Face, n: i64, i0: i64, j0: i64, size: i64, halo: i64) -> Option<CubePatch> {
        let ok = face.0 < 6
            && n > 0
            && n <= MAX_CELLS_PER_EDGE
            && size > 0
            && (0..=n).contains(&halo)
            && i0 >= 0
            && j0 >= 0
            && i0 + size <= n
            && j0 + size <= n;
        if !ok {
            return None;
        }
        let edge = size + 2 * halo;
        let mut cells = Vec::with_capacity((edge * edge) as usize);
        for v in -halo..size + halo {
            for u in -halo..size + halo {
                cells.push(extended_cell(face, n, i0 + u, j0 + v));
            }
        }
        Some(CubePatch { face, n, i0, j0, size, halo, cells })
    }

    pub fn face(&self) -> Face {
        self.face
    }

    /// Cells per face edge of the level.
    pub fn n(&self) -> i64 {
        self.n
    }

    /// The core's first cell `(i0, j0)` on its face.
    pub fn origin(&self) -> (i64, i64) {
        (self.i0, self.j0)
    }

    pub fn size(&self) -> i64 {
        self.size
    }

    pub fn halo(&self) -> i64 {
        self.halo
    }

    /// Cells per patch edge: `size + 2 halo`.
    pub fn edge(&self) -> i64 {
        self.size + 2 * self.halo
    }

    /// Whether `(u, v)` is a cell of the patch.
    pub fn contains(&self, u: i64, v: i64) -> bool {
        let r = -self.halo..self.size + self.halo;
        r.contains(&u) && r.contains(&v)
    }

    /// Whether `(u, v)` is a core cell.
    pub fn is_core(&self, u: i64, v: i64) -> bool {
        (0..self.size).contains(&u) && (0..self.size).contains(&v)
    }

    /// The cell at `(u, v)`, or None outside the patch.
    pub fn get(&self, u: i64, v: i64) -> Option<GlobalCell> {
        self.contains(u, v).then(|| self.cells[self.index(u, v)])
    }

    /// The cell at `(u, v)`; panics outside the patch.
    pub fn cell(&self, u: i64, v: i64) -> GlobalCell {
        assert!(self.contains(u, v), "({u}, {v}) is outside the patch of {} halo cells around a core of {}", self.halo, self.size);
        self.cells[self.index(u, v)]
    }

    /// Every cell, rows of `v` from `-halo`, each of `u` from `-halo` (the index of `(u, v)` is `(v + halo) edge + u + halo`).
    pub fn cells(&self) -> &[GlobalCell] {
        &self.cells
    }

    /// The point of `(u, v)` on the core face's extended plane, in half-cell units (the face's normal coordinate `n`, in-face coordinates
    /// `2 i + 1 - n`, `2 j + 1 - n` with `i = i0 + u`, `j = j0 + v`): exact integers, the frame a rotation of the cube maps onto the rotated
    /// patch's.
    pub fn extended_point(&self, u: i64, v: i64) -> [i64; 3] {
        let (nrm, au, av) = int_axes(self.face);
        let (a, b) = (2 * (self.i0 + u) + 1 - self.n, 2 * (self.j0 + v) + 1 - self.n);
        [0, 1, 2].map(|k| self.n * nrm[k] + a * au[k] + b * av[k])
    }

    fn index(&self, u: i64, v: i64) -> usize {
        ((v + self.halo) * self.edge() + u + self.halo) as usize
    }
}

/// The cell of `face`'s extended plane at `(i, j)` (each coordinate at most `n` outside `0..n`): the cell itself inside the face; one fold
/// across an edge; past a cube corner, two folds, the deeper coordinate first, the diagonal by the corner's orientation.
fn extended_cell(face: Face, n: i64, i: i64, j: i64) -> GlobalCell {
    let depth = |c: i64| if c < 0 { -c } else { c - (n - 1) };
    let (in_i, in_j) = ((0..n).contains(&i), (0..n).contains(&j));
    let (f, fi, fj) = match (in_i, in_j) {
        (true, true) => (face, i, j),
        (false, true) => fold_cell_axis(face, n, i, j, true),
        (true, false) => fold_cell_axis(face, n, i, j, false),
        (false, false) => {
            let (di, dj) = (depth(i), depth(j));
            let fold_i = match di.cmp(&dj) {
                core::cmp::Ordering::Greater => true,
                core::cmp::Ordering::Less => false,
                core::cmp::Ordering::Equal => {
                    // the faces across the i edge and the j edge: fold first towards the one that follows the core's face counter-clockwise
                    // around the corner's outward direction (det(n, n_i, n_j) > 0), a choice every rotation keeps
                    let (nrm, au, av) = int_axes(face);
                    let (si, sj) = (if i < 0 { -1 } else { 1 }, if j < 0 { -1 } else { 1 });
                    let (ni, nj) = (au.map(|x| si * x), av.map(|x| sj * x));
                    let det = nrm[0] * (ni[1] * nj[2] - ni[2] * nj[1]) - nrm[1] * (ni[0] * nj[2] - ni[2] * nj[0])
                        + nrm[2] * (ni[0] * nj[1] - ni[1] * nj[0]);
                    det > 0
                }
            };
            let (g, gi, gj) = fold_cell_axis(face, n, i, j, fold_i);
            // one coordinate is now inside the neighbour face (the folded depth), the other still outside by the other depth
            let second_i = !(0..n).contains(&gi);
            fold_cell_axis(g, n, gi, gj, second_i)
        }
    };
    GlobalCell::new(f, n, fi, fj).expect("a folded cell lies on a face of the level")
}
