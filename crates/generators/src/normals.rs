//! The normal of a vertex from the heights of its lattice neighbours (ADR 0073, GEN-019): the canonical normal of the CPU mesh and of the
//! compact tile.
//!
//! A tile evaluates one batch: its `(cells + 1)^2` lattice points, then the one-sample ring around them without the four ring corners
//! (`4 (cells + 1)` points: south row, north row, west column, east column), then whatever else the builder needs. The normal of the
//! vertex `(k, l)` is the cross product of the centred differences of the positions of its neighbours along the lattice axes,
//! `n = (q(k+1, l) - q(k-1, l)) x (q(k, l+1) - q(k, l-1))` normalised, `q = d (R + h)` in f64. Every face has `U x V` outward, so `n` is
//! outward without a flip; a flip is an assertion failure.
//!
//! Seams: two tiles sharing a vertex read the same lattice directions ([`TileId::ring_direction`]: inside a face the same integers,
//! beyond a face edge the neighbour tile's own lattice point one cell inside it), and form the same cross product of the same two
//! difference vectors up to the sign of each (negating both operands, or swapping them, negates the product exactly), so the normals are
//! bit-identical. At the eight cube corners the three tiles read the same three edge points and form `(p1 - p0) x (p2 - p0)` in the
//! canonical order of [`TileId::corner_stencil`], then orient the result outward.

use planet_core::cube::FaceMapping;
use planet_core::{TileId, Vec3};

/// The directions a tile evaluates for its normals: the lattice (row-major, `k` fastest), then the ring (see [`ring_index`]).
pub fn lattice_directions(map: &dyn FaceMapping, id: TileId, cells: u32) -> Vec<Vec3> {
    let w = cells as usize + 1;
    let mut dirs = Vec::with_capacity(w * w + 4 * w);
    for l in 0..=cells {
        for k in 0..=cells {
            dirs.push(id.sample_direction(map, cells, k, l));
        }
    }
    let n = i64::from(cells);
    for t in 0..=n {
        dirs.push(id.ring_direction(map, cells, t, -1));
    }
    for t in 0..=n {
        dirs.push(id.ring_direction(map, cells, t, n + 1));
    }
    for t in 0..=n {
        dirs.push(id.ring_direction(map, cells, -1, t));
    }
    for t in 0..=n {
        dirs.push(id.ring_direction(map, cells, n + 1, t));
    }
    dirs
}

/// How many directions [`lattice_directions`] gives for a tile of `cells` cells.
pub fn lattice_direction_count(cells: u32) -> usize {
    let w = cells as usize + 1;
    w * w + 4 * w
}

/// The index in [`lattice_directions`] of the point `(k, l)`, for `k`, `l` from -1 to `cells + 1` with one at most outside `0..=cells`.
pub fn ring_index(cells: u32, k: i64, l: i64) -> usize {
    let n = i64::from(cells);
    let w = cells as usize + 1;
    let inside = |a: i64| (0..=n).contains(&a);
    let base = w * w;
    if inside(k) && inside(l) {
        l as usize * w + k as usize
    } else if l == -1 && inside(k) {
        base + k as usize
    } else if l == n + 1 && inside(k) {
        base + w + k as usize
    } else if k == -1 && inside(l) {
        base + 2 * w + l as usize
    } else if k == n + 1 && inside(l) {
        base + 3 * w + l as usize
    } else {
        panic!("({k}, {l}) is no lattice or ring point of a {cells}-cell tile (the ring has no corners)")
    }
}

/// The unit normals of the `(cells + 1)^2` vertices of tile `id` (row-major, `k` fastest) from the directions `dirs` of
/// [`lattice_directions`] (at least that many; the rest is ignored) and their heights `heights` in metres.
pub fn lattice_normals(id: TileId, cells: u32, radius_m: f64, dirs: &[Vec3], heights: &[f64]) -> Vec<Vec3> {
    let w = cells as usize + 1;
    assert!(dirs.len() >= lattice_direction_count(cells) && heights.len() >= lattice_direction_count(cells), "one height per direction");
    let q = |idx: usize| dirs[idx] * (radius_m + heights[idx]);
    let mut out = Vec::with_capacity(w * w);
    for l in 0..=i64::from(cells) {
        for k in 0..=i64::from(cells) {
            let slot = |s: u8| match s {
                0 => ring_index(cells, k + 1, l),
                1 => ring_index(cells, k - 1, l),
                2 => ring_index(cells, k, l + 1),
                _ => ring_index(cells, k, l - 1),
            };
            let here = dirs[l as usize * w + k as usize];
            let normal = match id.corner_stencil(cells, k as u32, l as u32) {
                Some([a, b, c]) => {
                    // A cube corner: the three tiles form the same cross product of the same three points in the same order, then orient
                    // it outward (the one place the orientation is decided by the sign of a dot product).
                    let (p0, p1, p2) = (q(slot(a)), q(slot(b)), q(slot(c)));
                    let n = (p1 - p0).cross(p2 - p0).normalized();
                    if n.dot(here) < 0.0 {
                        -n
                    } else {
                        n
                    }
                }
                None => {
                    let n = (q(slot(0)) - q(slot(1))).cross(q(slot(2)) - q(slot(3))).normalized();
                    assert!(
                        n.dot(here) > 0.0,
                        "the lattice normal of vertex ({k}, {l}) of {id:?} points inward: a face frame is not U x V = outward"
                    );
                    n
                }
            };
            out.push(normal);
        }
    }
    out
}
