//! The chunk lattice (ADR 0077, GEN-034, GEN-035): the cells of a cube-sphere lattice of `cells_per_chunk x 2^11` per face edge, so a chunk (the area of
//! a level-11 tile) is `cells_per_chunk` cells wide, and the key of a chunk on the grid. The horizon maps are baked per chunk on this lattice.

use crate::chunk::CHUNK_GRID_LEVEL;
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{TileId, Vec3};

/// The coarsest tile level whose tiles read the chunk grid (GEN-035): the chunk grid level, so a tile reads one chunk at most.
pub const PYRAMID_TOP_LEVEL: u8 = 14;

/// A cell of the lattice: the face and the position on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub face: u8,
    pub i: u32,
    pub j: u32,
}

/// The cube-sphere lattice the chunks are cut from (see the module comment).
#[derive(Clone, Copy, Debug)]
pub struct ChunkLattice {
    per_chunk: u32,
    n: i64,
}

impl ChunkLattice {
    /// `cells_per_chunk` cells per chunk edge; `None` for zero or for a lattice whose indices would not fit the cell key.
    pub fn new(cells_per_chunk: u32) -> Option<ChunkLattice> {
        let n = i64::from(cells_per_chunk) << CHUNK_GRID_LEVEL;
        (cells_per_chunk > 0 && n < (1 << 27)).then_some(ChunkLattice { per_chunk: cells_per_chunk, n })
    }

    pub fn cells_per_chunk(&self) -> u32 {
        self.per_chunk
    }

    /// Cells per face edge.
    pub fn n(&self) -> i64 {
        self.n
    }

    /// The cell size in metres on a body of radius `radius_m`, at the face centre.
    pub fn cell_m(&self, radius_m: f64) -> f64 {
        radius_m * core::f64::consts::FRAC_PI_2 / self.n as f64
    }

    /// The unit direction of the centre of the cell at (`i`, `j`) of `face`; a position beyond the face edge is read on the extended cube plane.
    fn direction_at(&self, face: u8, i: i64, j: i64) -> Vec3 {
        direction_at_n(self.n, face, i, j)
    }

    /// [`ChunkLattice::direction`] for a position given as signed indices, beyond the face edge read on the extended cube plane: the cell
    /// directions of a raster that reaches over a face edge (the horizon bake's halo, M8-5).
    pub fn direction_ext(&self, face: u8, i: i64, j: i64) -> Vec3 {
        self.direction_at(face, i, j)
    }

    /// The unit direction of the centre of `c`.
    pub fn direction(&self, c: Cell) -> Vec3 {
        self.direction_at(c.face, i64::from(c.i), i64::from(c.j))
    }

    /// The cell holding the direction `d`.
    pub fn cell_at(&self, d: Vec3) -> Cell {
        cell_at_n(self.n, d)
    }
}

/// The direction of the centre of cell (`i`, `j`) of `face` on a lattice of `n` cells per face edge (beyond the edge: the extended cube plane).
fn direction_at_n(n: i64, face: u8, i: i64, j: i64) -> Vec3 {
    let n = n as f64;
    face_to_direction(&TangentWarp, planet_core::Face(face), -1.0 + (2.0 * i as f64 + 1.0) / n, -1.0 + (2.0 * j as f64 + 1.0) / n)
}

/// How far past a cell boundary a position must lie to count as inside the next cell, in cells. Across a cube-face edge near a corner the centre
/// of the cell beyond the edge lies within about 1e-9 cells of a boundary of the other face's lattice (8 m cells: 1e-12), where the rounding of
/// the direction maths would pick the neighbour; the bias picks the upper cell whatever the rounding.
const TIE_BIAS_CELLS: f64 = 1e-6;

/// The cell of a lattice of `n` cells per face edge holding the direction `d` (a position within [`TIE_BIAS_CELLS`] below a boundary is read as
/// above it).
fn cell_at_n(n: i64, d: Vec3) -> Cell {
    let (f, s, t) = direction_to_face(&TangentWarp, d);
    let i = (((s + 1.0) * 0.5 * n as f64 + TIE_BIAS_CELLS) as i64).clamp(0, n - 1);
    let j = (((t + 1.0) * 0.5 * n as f64 + TIE_BIAS_CELLS) as i64).clamp(0, n - 1);
    Cell { face: f.0, i: i as u32, j: j as u32 }
}

/// The cell at (`i`, `j`) of `face` on a lattice of `n` cells per face edge, a position beyond the face edge being the cell holding the
/// direction beyond it.
fn canon_n(n: i64, face: u8, i: i64, j: i64) -> Cell {
    if (0..n).contains(&i) && (0..n).contains(&j) {
        Cell { face, i: i as u32, j: j as u32 }
    } else {
        cell_at_n(n, direction_at_n(n, face, i, j))
    }
}

/// A chunk's place on the grid: face and tile position at the chunk grid level.
pub type ChunkKey = (u8, u32, u32);

/// The chunks a tile reads, own chunk first and the neighbours that its reads can reach after it, in a fixed order; empty for a tile above `top`.
/// A tile reads cells of the neighbouring chunk within `reach_cells` of its edge; a neighbour out of that reach is not listed.
pub fn chunks_read_by(lat: &ChunkLattice, top: u8, reach_cells: i64, tile: TileId) -> Vec<ChunkKey> {
    if tile.level() < top || tile.level() < CHUNK_GRID_LEVEL {
        return Vec::new();
    }
    let (n, per) = (lat.n(), i64::from(lat.cells_per_chunk()));
    let shift = u32::from(tile.level());
    let range = |index: u32| -> (i64, i64) {
        let lo = ((u64::from(index) * n as u64) >> shift) as i64;
        let hi = (((u64::from(index) + 1) * n as u64 + (1u64 << shift) - 1) >> shift) as i64 - 1;
        (lo, hi)
    };
    let ((i0, i1), (j0, j1)) = (range(tile.x()), range(tile.y()));
    let own = (tile.face().0, (i0 / per) as u32, (j0 / per) as u32);
    let mut out = vec![own];
    let mut others = std::collections::BTreeSet::new();
    for a in [i0 - reach_cells, (i0 + i1) / 2, i1 + reach_cells] {
        for b in [j0 - reach_cells, (j0 + j1) / 2, j1 + reach_cells] {
            let c = canon_n(n, tile.face().0, a, b);
            let key = (c.face, (i64::from(c.i) / per) as u32, (i64::from(c.j) / per) as u32);
            if key != own {
                others.insert(key);
            }
        }
    }
    out.extend(others);
    out
}
