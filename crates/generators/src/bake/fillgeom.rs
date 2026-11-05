//! Which cells touch a tile's border from other tiles (GEN-121, STRM-077): the geometry the spill graph is joined with.
//!
//! The layer grid has `cells` cells per face edge; a tile's core is `tile x tile` of them. A core cell on the tile's border touches up to 8 cells of other tiles: inside a face they are cells
//! of the same plane, past a face's edge they are the cells of the neighbouring face that hold the direction of the plane's continuation (a cube edge; the cell at a cube corner reaches the
//! cells of the other two faces). The same mapping the readers use ([`TileGrid::direction`] then `direction_to_face`) names them, so the graph and the layer agree on what adjoins what.

use super::tile::{TileGrid, TileId};
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::Face;

/// The cells of other tiles that touch core cell `(i, j)` of tile `id` (`i`, `j` in the tile's core), as `(tile, i, j)` with `i`, `j` in that tile's core, sorted and without repeats.
/// A neighbour past a face's edge is the nearest cell of the neighbouring face to the continued plane's point, so the relation can be one-sided at a cube edge: the caller joins every pair it
/// is given (a repeated pair does no harm).
pub fn border_neighbours(g: &TileGrid, id: TileId, i: usize, j: usize) -> Vec<(TileId, usize, usize)> {
    let (n, t) = (g.cells as i64, g.tile as i64);
    let (gi, gj) = (i64::from(id.ti) * t + i as i64, i64::from(id.tj) * t + j as i64);
    let mut out: Vec<(TileId, usize, usize)> = Vec::with_capacity(8);
    for dj in -1i64..=1 {
        for di in -1i64..=1 {
            if (di, dj) == (0, 0) {
                continue;
            }
            let (ni, nj) = (gi + di, gj + dj);
            let (face, ci, cj) = if (0..n).contains(&ni) && (0..n).contains(&nj) {
                (id.face, ni, nj)
            } else {
                let (f, s, u) = direction_to_face(&TangentWarp, g.direction(id.face, ni, nj));
                let cell = |c: f64| ((c + 1.0) * 0.5 * n as f64).floor().clamp(0.0, n as f64 - 1.0) as i64;
                (Face(f.0), cell(s), cell(u))
            };
            let other = TileId { face, ti: (ci / t) as u32, tj: (cj / t) as u32 };
            if other != id {
                out.push((other, (ci % t) as usize, (cj % t) as usize));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
