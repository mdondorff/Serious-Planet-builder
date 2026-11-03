//! The bake's kernels (ADR 0088 point 3, GEN-100, GEN-101; m9-bake PR 3a): the routing and carve of pass 1 on a rectangular grid. A port of SPIKE-70's prototype
//! (`spikes/m9-global-routing/results/proto/s70carve.rs.txt`) to f64 with `libm`, the outlets an explicit input.
//!
//! Every kernel is single-threaded with a fixed visiting order (the flood's pop order with ties broken by the cell index, the donor stack, the topological order), so the
//! result is the same bits on any machine and thread count (CON-14); the jitter of the receivers is an integer hash of the cell and the direction.
//! The grids do not wrap: a cube face does not route across its edges (GEN-100), its edge cells are outlets like the sea.

pub mod carve;
pub mod flood;
pub mod route;

/// A `w x h` grid of cells `dx` metres apart, row-major (`p = j w + i`), 8-connected, not wrapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub dx: f64,
}

/// The 8 neighbours as `(di, dj)`: the four axis neighbours first, then the diagonals.
pub const NEIGHBOURS: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

impl Grid {
    pub fn new(w: usize, h: usize, dx: f64) -> Grid {
        assert!(w >= 3 && h >= 3, "a grid is at least 3 x 3 cells, not {w} x {h}");
        assert!(dx.is_finite() && dx > 0.0, "the cell size is finite and positive, not {dx}");
        assert!(w * h < u32::MAX as usize, "the cell index is a u32");
        Grid { w, h, dx }
    }

    pub fn len(&self) -> usize {
        self.w * self.h
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The neighbour `d` of cell `p`, if it is on the grid.
    pub fn neighbour(&self, p: usize, d: usize) -> Option<usize> {
        let (i, j) = ((p % self.w) as i64 + NEIGHBOURS[d].0, (p / self.w) as i64 + NEIGHBOURS[d].1);
        (i >= 0 && j >= 0 && i < self.w as i64 && j < self.h as i64).then(|| j as usize * self.w + i as usize)
    }

    /// The distance from a cell to its neighbour `d`.
    pub fn distance(&self, d: usize) -> f64 {
        if d < 4 {
            self.dx
        } else {
            self.dx * core::f64::consts::SQRT_2
        }
    }

    /// Whether `p` lies on the border of the grid.
    pub fn is_edge(&self, p: usize) -> bool {
        let (i, j) = (p % self.w, p / self.w);
        i == 0 || j == 0 || i == self.w - 1 || j == self.h - 1
    }

    /// The outlets of a face: its border cells and every cell at or below `sea_level` (water leaves through them).
    pub fn outlets(&self, z: &[f64], sea_level: f64) -> Vec<bool> {
        assert_eq!(z.len(), self.len(), "one height per cell");
        (0..self.len()).map(|p| self.is_edge(p) || z[p] <= sea_level).collect()
    }
}
