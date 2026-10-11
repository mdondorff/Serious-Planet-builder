//! CPU terrain mesh for one tile (CON-11, ADR 0001): a regular grid of vertices stored as float32 offsets from the
//! tile origin, per-vertex normals, a "coarse" position for geomorphing to the parent grid, and skirts that hide
//! small cracks between tiles of different levels.
//!
//! Positions are computed in float64 on the planet-fixed frame and rounded once to f32 after subtracting the origin,
//! so vertex precision is bounded by tile size (plus the height range within the tile), never by planet size.

use crate::height::{cache_key_field, HeightField, Reference};
use planet_core::cube::FaceMapping;
use planet_core::hash::{hash_u64s, HASH_VERSION};
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileMesh {
    pub id: TileId,
    pub cells: u32,
    /// Key of everything this mesh depends on ([`mesh_key`]); a renderer replaces a resident buffer whose key differs.
    pub key: u64,
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

/// Version of the mesh construction (ADR 0006): bump it whenever vertex output changes for the same inputs (skirt
/// depth, normal stencil, origin rule, vertex layout). History: 1 = first meshes; 2 = seam-consistent normals (#15);
/// 3 = no `ref_pos` attribute: the morph distance is measured to the real vertex position (LOD-008).
pub const MESH_VERSION: u32 = 3;

/// Key of a mesh: the height key of the tile (generator, seed, mapping, tile; resolution = `cells`), the grid size, the
/// radius bits, the skirt fraction bits and [`MESH_VERSION`].
pub fn mesh_key(map: &dyn FaceMapping, seed: u64, id: TileId, cells: u32, radius_m: f64) -> u64 {
    mesh_key_with(map, seed, id, cells, radius_m, MESH_VERSION, SKIRT_FRACTION)
}

/// [`mesh_key`] with the construction constants as arguments, so tests can show that each one changes the key.
pub fn mesh_key_with(map: &dyn FaceMapping, seed: u64, id: TileId, cells: u32, radius_m: f64, version: u32, skirt_fraction: f64) -> u64 {
    let field = Reference(seed);
    mesh_key_chain(cache_key_field(&field, map, id, cells), cells, body_params(&field, radius_m), version, skirt_fraction)
}

/// The body parameter hash of a field (ADR 0001, 0007): body id, seed, radius and reference surface.
fn body_params(field: &dyn HeightField, radius_m: f64) -> u64 {
    planet_world_def::body_params_hash(field.body(), field.seed(), radius_m, field.surface_id())
}

fn mesh_key_chain(height_key: u64, cells: u32, body_params: u64, version: u32, skirt_fraction: f64) -> u64 {
    // The key format version is named here too: it reaches the height key's words, but a mesh-only change of this
    // composition must have a constant to bump.
    hash_u64s(
        0x006d_6573,
        &[
            u64::from(version),
            u64::from(crate::height::KEY_FORMAT_VERSION),
            height_key,
            u64::from(cells),
            body_params,
            skirt_fraction.to_bits(),
        ],
    )
}

/// [`mesh_key`] for a height field: it chains the field's height key ([`cache_key_field`]), so [`Reference`] gives
/// exactly [`mesh_key`] and any other field (other implementation id or version) gets other keys (ADR 0006).
pub fn mesh_key_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> u64 {
    mesh_key_chain(cache_key_field(field, map, id, cells), cells, body_params(field, radius_m), MESH_VERSION, SKIRT_FRACTION)
}

/// [`mesh_key_field`] for many tiles of one field, mapping, grid size and radius: the string hashes of the field's
/// implementation id and the mapping id are computed once instead of per tile (planning a 5,000-node view calls
/// this 5,000 times). `key(tile)` equals `mesh_key_field(field, map, tile, cells, radius_m)` exactly.
pub struct MeshKeyer<'a> {
    field: &'a dyn HeightField,
    version: u64,
    cells: u32,
    body_params: u64,
    body_words: [u64; 3],
    implementation_hash: u64,
    mapping_hash: u64,
}

impl<'a> MeshKeyer<'a> {
    pub fn new(field: &'a dyn HeightField, map: &dyn FaceMapping, cells: u32, radius_m: f64) -> MeshKeyer<'a> {
        MeshKeyer {
            field,
            version: u64::from(field.version()),
            cells,
            body_params: body_params(field, radius_m),
            body_words: crate::height::body_key_words(field),
            implementation_hash: hash_u64s(1, &crate::height::fold_str(field.implementation_id())),
            mapping_hash: hash_u64s(2, &crate::height::fold_str(map.id())),
        }
    }

    pub fn key(&self, id: TileId) -> u64 {
        let height_key = hash_u64s(
            0x006b_6579,
            &[
                self.version,
                u64::from(HASH_VERSION),
                self.field.inputs_hash_for(id),
                id.raw(),
                u64::from(self.cells),
                self.implementation_hash,
                self.mapping_hash,
                self.body_words[0],
                self.body_words[1],
                self.body_words[2],
            ],
        );
        mesh_key_chain(height_key, self.cells, self.body_params, MESH_VERSION, SKIRT_FRACTION)
    }
}

impl TileMesh {
    /// Hash of the exact vertex bits and origin, for known-answer tests (CON-14).
    pub fn content_hash(&self) -> u64 {
        let mut v =
            vec![self.id.raw(), u64::from(self.cells), self.origin.0.x.to_bits(), self.origin.0.y.to_bits(), self.origin.0.z.to_bits()];
        for vert in &self.vertices {
            v.extend(vert.to_floats().iter().map(|f| u64::from(f.to_bits())));
        }
        hash_u64s(0x006d_6873, &v)
    }
}

/// Build the mesh. `cells` must be a power of two of at least 2.
pub fn tile_mesh(seed: u64, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> TileMesh {
    tile_mesh_field(&Reference(seed), map, id, cells, radius_m)
}

/// [`tile_mesh`] over any height field (stress tests use `stress::Steep`).
pub fn tile_mesh_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> TileMesh {
    assert!(cells >= 2 && cells.is_power_of_two(), "cells must be a power of two >= 2");
    let n = i64::from(cells);
    let width = (cells + 1) as usize;
    // World position and height of lattice point (i, j), 0 <= i, j <= cells.
    let mut world = vec![Vec3::ZERO; width * width];
    let mut heights = vec![0.0f64; width * width];
    let mut dirs = vec![Vec3::ZERO; width * width];
    for j in 0..=n {
        for i in 0..=n {
            let dir = id.sample_direction(map, cells, i as u32, j as u32);
            let h = field.height(dir);
            let idx = j as usize * width + i as usize;
            world[idx] = dir * (radius_m + h);
            heights[idx] = h;
            dirs[idx] = dir;
        }
    }
    let at = |i: i64, j: i64| world[j as usize * width + i as usize];
    // Angular step of the normal stencil: one cell of this level (identical for tiles of the same level).
    let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level()) / f64::from(cells);
    // The origin sits at the tile centre including its terrain height, so tile-local offsets and the camera-relative
    // origin stay small (a few tile sizes), never terrain-height sized.
    let centre_dir = id.center_direction(map);
    let origin = PlanetFixed(centre_dir * (radius_m + field.height(centre_dir)));
    let local = |p: Vec3| {
        let d = p - origin.0;
        [d.x as f32, d.y as f32, d.z as f32]
    };

    let mut vertices = Vec::with_capacity(vertex_count(cells));
    for j in 0..=n {
        for i in 0..=n {
            let normal = surface_normal_field(field, dirs[j as usize * width + i as usize], eps, radius_m);
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
                height: heights[j as usize * width + i as usize] as f32,
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
        vertices.push(MeshVertex { pos: down(v.pos), coarse: down(v.coarse), ..v });
    }
    TileMesh { id, cells, key: mesh_key_field(field, map, id, cells, radius_m), origin, vertices }
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
    /// Number of f32 values per vertex in GPU order: `pos`, `coarse`, `normal`, `height`.
    pub const FLOATS: usize = 10;
    /// Byte stride of a vertex buffer entry.
    pub const STRIDE: u64 = (Self::FLOATS * 4) as u64;
    /// The attribute offsets (in floats) of `pos`, `coarse`, `normal` and `height`.
    pub const OFFSETS: [usize; 4] = [0, 3, 6, 9];

    pub fn to_floats(&self) -> [f32; Self::FLOATS] {
        let (p, c, n) = (self.pos, self.coarse, self.normal);
        [p[0], p[1], p[2], c[0], c[1], c[2], n[0], n[1], n[2], self.height]
    }
}

/// Outward surface normal at the unit direction `dir`, from central differences of the height field in a tangent frame
/// that is a pure function of `dir` (not of any tile's lattice). Border vertices shared by tiles of the same level, also
/// across cube faces, have bit-identical directions and the same `eps`, so they get bit-identical normals (ADR 0003).
pub fn surface_normal(seed: u64, dir: Vec3, eps: f64, radius_m: f64) -> Vec3 {
    surface_normal_field(&Reference(seed), dir, eps, radius_m)
}

/// [`surface_normal`] over any height field.
pub fn surface_normal_field(field: &dyn HeightField, dir: Vec3, eps: f64, radius_m: f64) -> Vec3 {
    // Tangent frame from `dir` alone: the world axis least aligned with it seeds t1; t2 = dir x t1, so t1 x t2 = dir.
    let (ax, ay, az) = (dir.x.abs(), dir.y.abs(), dir.z.abs());
    let axis = if ax <= ay && ax <= az {
        Vec3::new(1.0, 0.0, 0.0)
    } else if ay <= az {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let t1 = (axis - dir * axis.dot(dir)).normalized();
    let t2 = dir.cross(t1);
    let p = |t: Vec3, sign: f64| {
        let d = (dir + t * (sign * eps)).normalized();
        d * (radius_m + field.height(d))
    };
    (p(t1, 1.0) - p(t1, -1.0)).cross(p(t2, 1.0) - p(t2, -1.0)).normalized()
}
