//! CPU terrain mesh for one tile (CON-11, ADR 0001): a regular grid of vertices stored as float32 offsets from the
//! tile origin, per-vertex normals, a "coarse" position for geomorphing to the parent grid, and skirts that hide
//! small cracks between tiles of different levels.
//!
//! Positions are computed in float64 on the planet-fixed frame and rounded once to f32 after subtracting the origin,
//! so vertex precision is bounded by tile size (plus the height range within the tile), never by planet size.

use crate::height::height_at;
use planet_core::cube::FaceMapping;
use planet_core::{PlanetFixed, TileId, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct MeshVertex {
    /// Offset from the tile origin.
    pub pos: [f32; 3],
    /// Position after morphing to the parent grid (same space as `pos`).
    pub coarse: [f32; 3],
    pub normal: [f32; 3],
    /// Terrain height above the sphere, metres.
    pub height: f32,
    /// Position on the reference sphere (height 0) below this vertex, same space as `pos`: the height-free point the
    /// morph distance is measured to, matching how node selection measures distance.
    pub ref_pos: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileMesh {
    pub id: TileId,
    pub cells: u32,
    /// Tile centre at its terrain height, planet-fixed f64 (identical to the origin in `TerrainPlan`).
    pub origin: PlanetFixed,
    /// `(cells + 1)²` grid vertices (row `j` major), then the skirt vertices.
    pub vertices: Vec<MeshVertex>,
}

pub fn grid_vertex_count(cells: u32) -> usize {
    ((cells + 1) * (cells + 1)) as usize
}

pub fn vertex_count(cells: u32) -> usize {
    grid_vertex_count(cells) + 4 * (cells as usize + 1)
}

/// Fraction of the tile edge length by which skirts hang below the surface.
pub const SKIRT_FRACTION: f64 = 0.08;

/// Build the mesh. `cells` must be a power of two of at least 2.
pub fn tile_mesh(seed: u64, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> TileMesh {
    assert!(cells >= 2 && cells.is_power_of_two(), "cells must be a power of two >= 2");
    let n = i64::from(cells);
    let width = (cells + 3) as usize;
    // World position of lattice point (i, j), -1 <= i, j <= cells + 1.
    let mut world = vec![Vec3::ZERO; width * width];
    let mut heights = vec![0.0f64; width * width];
    for j in -1..=n + 1 {
        for i in -1..=n + 1 {
            let dir = id.sample_direction_ext(map, cells, i, j);
            let h = height_at(seed, dir);
            let idx = ((j + 1) as usize) * width + (i + 1) as usize;
            world[idx] = dir * (radius_m + h);
            heights[idx] = h;
        }
    }
    let at = |i: i64, j: i64| world[((j + 1) as usize) * width + (i + 1) as usize];
    // The origin sits at the tile centre including its terrain height, so tile-local offsets and the camera-relative
    // origin stay small (a few tile sizes), never terrain-height sized.
    let centre_dir = id.center_direction(map);
    let origin = PlanetFixed(centre_dir * (radius_m + height_at(seed, centre_dir)));
    let local = |p: Vec3| {
        let d = p - origin.0;
        [d.x as f32, d.y as f32, d.z as f32]
    };

    let mut vertices = Vec::with_capacity(vertex_count(cells));
    for j in 0..=n {
        for i in 0..=n {
            let normal = (at(i + 1, j) - at(i - 1, j)).cross(at(i, j + 1) - at(i, j - 1)).normalized();
            // Geomorph target on the parent grid (diagonal (i-1,j-1)-(i+1,j+1) matches the triangulation).
            let coarse = match (i % 2, j % 2) {
                (0, 0) => at(i, j),
                (1, 0) => (at(i - 1, j) + at(i + 1, j)) * 0.5,
                (0, 1) => (at(i, j - 1) + at(i, j + 1)) * 0.5,
                _ => (at(i - 1, j - 1) + at(i + 1, j + 1)) * 0.5,
            };
            vertices.push(MeshVertex {
                pos: local(at(i, j)),
                coarse: local(coarse),
                normal: [normal.x as f32, normal.y as f32, normal.z as f32],
                height: heights[((j + 1) as usize) * width + (i + 1) as usize] as f32,
                ref_pos: local(at(i, j).normalized() * radius_m),
            });
        }
    }
    // Skirts: duplicates of the border vertices, pushed towards the planet centre (south, north, west, east).
    let edge_len = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
    let depth = (edge_len * SKIRT_FRACTION).max(1e-3);
    let border: Vec<usize> = (0..=cells as usize)
        .chain((0..=cells as usize).map(|i| cells as usize * (cells as usize + 1) + i)) // north
        .chain((0..=cells as usize).map(|j| j * (cells as usize + 1))) // west
        .chain((0..=cells as usize).map(|j| j * (cells as usize + 1) + cells as usize)) // east
        .collect();
    for &b in &border {
        let v = vertices[b];
        let radial = (origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))).normalized() * depth;
        let down =
            |p: [f32; 3]| [(f64::from(p[0]) - radial.x) as f32, (f64::from(p[1]) - radial.y) as f32, (f64::from(p[2]) - radial.z) as f32];
        vertices.push(MeshVertex { pos: down(v.pos), coarse: down(v.coarse), ref_pos: down(v.ref_pos), ..v });
    }
    TileMesh { id, cells, origin, vertices }
}

/// Triangle indices for the grid and its skirts; identical for every tile of a given `cells`.
pub fn grid_indices(cells: u32) -> Vec<u32> {
    let w = cells + 1;
    let mut idx = Vec::new();
    for j in 0..cells {
        for i in 0..cells {
            let (a, b, c, d) = (j * w + i, j * w + i + 1, (j + 1) * w + i + 1, (j + 1) * w + i);
            idx.extend([a, b, c, a, c, d]);
        }
    }
    // Skirt strips: four runs of `cells + 1` skirt vertices, each paired with its border vertex.
    let base = (w * w) as usize;
    let run = w as usize;
    let borders: [Box<dyn Fn(u32) -> u32>; 4] =
        [Box::new(|i| i), Box::new(move |i| cells * w + i), Box::new(move |j| j * w), Box::new(move |j| j * w + cells)];
    for (k, border) in borders.iter().enumerate() {
        for i in 0..cells {
            let (b0, b1) = (border(i), border(i + 1));
            let (s0, s1) = ((base + k * run) as u32 + i, (base + k * run) as u32 + i + 1);
            idx.extend([b0, b1, s1, b0, s1, s0]);
        }
    }
    idx
}

impl MeshVertex {
    /// Number of f32 values per vertex in GPU order: `pos`, `coarse`, `normal`, `height`, `ref_pos`.
    pub const FLOATS: usize = 13;
    /// Byte stride of a vertex buffer entry.
    pub const STRIDE: u64 = (Self::FLOATS * 4) as u64;
    /// The attribute offsets (in floats) of `pos`, `coarse`, `normal`, `height` and `ref_pos`.
    pub const OFFSETS: [usize; 5] = [0, 3, 6, 9, 10];

    pub fn to_floats(&self) -> [f32; Self::FLOATS] {
        let (p, c, n, r) = (self.pos, self.coarse, self.normal, self.ref_pos);
        [p[0], p[1], p[2], c[0], c[1], c[2], n[0], n[1], n[2], self.height, r[0], r[1], r[2]]
    }
}
