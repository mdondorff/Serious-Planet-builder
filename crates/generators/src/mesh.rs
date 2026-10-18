//! CPU terrain mesh for one tile (CON-11, ADR 0001): a regular grid of vertices stored as float32 offsets from the
//! tile origin, per-vertex normals, a "coarse" position for geomorphing to the parent grid, and skirts that hide
//! small cracks between tiles of different levels.
//!
//! Positions are computed in float64 on the planet-fixed frame and rounded once to f32 after subtracting the origin,
//! so vertex precision is bounded by tile size (plus the height range within the tile), never by planet size.

use crate::climate::ClimateTexture;
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
    /// Height after morphing to the parent grid: the parent-grid rule applied to heights, exactly as `coarse` applies it to
    /// positions (edge or diagonal midpoint of the parent cell's corner heights), so the height morphs with the position.
    pub coarse_height: f32,
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
    /// The climate field of the tile (ADR 0039 point 2): 9 x 9 `f16` texels of temperature and precipitation, 324 bytes.
    pub climate: ClimateTexture,
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
/// 3 = no `ref_pos` attribute: the morph distance is measured to the real vertex position (LOD-008); 4 = the 44-byte vertex (#65);
/// 5 = the compact tile format (GEN-019, ADR 0033) exists beside the vertex format: a tile's key now stands for either, and the
/// compact construction (control points, height and octahedral normals, record layout; 16-bit and snorm16 until version 8) is covered by this version.
/// 6 = the key names whether the tile is a finest-level tile of the view (level at or above the LOD cap, [`finest_class_key`]):
/// the compact format's CON-13 guard decides the format of such a tile from its relief, so the key must identify that class
/// (finest tiles get other keys; tiles below the cap keep the version-5 composition under the new version).
/// 7 = the climate field (ADR 0039 point 2, GEN-021): a tile carries 9 x 9 `f16` texels of temperature and precipitation (the spare
/// channel of the first 162 texels of a compact tile, a 324-byte block of a vertex tile) and the key names the climate version word
/// ([`HeightField::climate_version_word`]: the climate, reader and bake versions). Tiles stored under version 6 are unreachable (their
/// keys differ) and the coarse pre-fill of a pack must be written again.
/// 8 = the compact tile is the slot of GEN-025 (ADR 0062): a 32-bit height step count and two 15-bit octahedral normal components per
/// texel, the climate block and the header inside the slot; every compact key changes, vertex tiles are unchanged.
pub const MESH_VERSION: u32 = 8;

/// Key of a mesh: the height key of the tile (generator, seed, mapping, tile; resolution = `cells`), the grid size, the
/// radius bits, the skirt fraction bits and [`MESH_VERSION`].
pub fn mesh_key(map: &dyn FaceMapping, seed: u64, id: TileId, cells: u32, radius_m: f64) -> u64 {
    mesh_key_with(map, seed, id, cells, radius_m, MESH_VERSION, SKIRT_FRACTION)
}

/// [`mesh_key`] with the construction constants as arguments, so tests can show that each one changes the key.
pub fn mesh_key_with(map: &dyn FaceMapping, seed: u64, id: TileId, cells: u32, radius_m: f64, version: u32, skirt_fraction: f64) -> u64 {
    let field = Reference(seed);
    mesh_key_chain(
        cache_key_field(&field, map, id, cells),
        cells,
        body_params(&field, radius_m),
        version,
        skirt_fraction,
        field.climate_version_word(),
    )
}

/// The key class of a tile for a view whose finest level (the LOD cap, `max_level`) is `finest_level`: a tile at or deeper than the cap
/// is a finest tile, whose format depends on its relief (CON-13, ADR 0033 point 5), so its key is the base key rehashed with the
/// class (not the cap's value: the decision only depends on the class, so equal classes share keys); a tile below the cap keeps its base key. A resident tile is reused by its key, so the key must identify what the
/// format decision depends on: tile, field and "finest for this cap". Pure.
pub fn finest_class_key(base_key: u64, id: TileId, finest_level: u8) -> u64 {
    if id.level() >= finest_level {
        hash_u64s(0x0066_696e, &[base_key])
    } else {
        base_key
    }
}

/// The body parameter hash of a field (ADR 0001, 0007): body id, seed, radius and reference surface.
fn body_params(field: &dyn HeightField, radius_m: f64) -> u64 {
    planet_world_def::body_params_hash(field.body(), field.seed(), radius_m, field.surface_id())
}

fn mesh_key_chain(height_key: u64, cells: u32, body_params: u64, version: u32, skirt_fraction: f64, climate_word: u64) -> u64 {
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
            climate_word,
        ],
    )
}

/// [`mesh_key`] for a height field: it chains the field's height key ([`cache_key_field`]), so [`Reference`] gives
/// exactly [`mesh_key`] and any other field (other implementation id or version) gets other keys (ADR 0006).
pub fn mesh_key_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> u64 {
    mesh_key_chain(
        cache_key_field(field, map, id, cells),
        cells,
        body_params(field, radius_m),
        MESH_VERSION,
        SKIRT_FRACTION,
        field.climate_version_word(),
    )
}

/// [`mesh_key_field`] for many tiles of one field, mapping, grid size and radius: the string hashes of the field's
/// implementation id and the mapping id are computed once instead of per tile (planning a 5,000-node view calls
/// this 5,000 times). `key(tile)` equals `mesh_key_field(field, map, tile, cells, radius_m)` exactly (without `at_finest_level`; with it, the finest-level class of the tile is applied).
pub struct MeshKeyer<'a> {
    field: &'a dyn HeightField,
    map: &'a dyn FaceMapping,
    /// The field-wide inputs hash, computed once, when no tile reads cells.
    inputs: Option<u64>,
    version: u64,
    cells: u32,
    body_params: u64,
    body_words: [u64; 3],
    climate_word: u64,
    implementation_hash: u64,
    mapping_hash: u64,
    /// The LOD cap whose class the keys carry ([`finest_class_key`]); none = the class-free base keys.
    finest_level: Option<u8>,
}

impl<'a> MeshKeyer<'a> {
    pub fn new(field: &'a dyn HeightField, map: &'a dyn FaceMapping, cells: u32, radius_m: f64) -> MeshKeyer<'a> {
        MeshKeyer {
            field,
            map,
            inputs: (!field.reads_cells()).then(|| field.inputs_hash()),
            version: u64::from(field.version()),
            cells,
            body_params: body_params(field, radius_m),
            body_words: crate::height::body_key_words(field),
            climate_word: field.climate_version_word(),
            implementation_hash: hash_u64s(1, &crate::height::fold_str(field.implementation_id())),
            mapping_hash: hash_u64s(2, &crate::height::fold_str(map.id())),
            finest_level: None,
        }
    }

    /// Keys that carry the finest-level class of a view with this LOD cap: `key(tile)` equals [`finest_class_key`] of the base key.
    pub fn at_finest_level(mut self, finest_level: u8) -> MeshKeyer<'a> {
        self.finest_level = Some(finest_level);
        self
    }

    pub fn key(&self, id: TileId) -> u64 {
        let height_key = hash_u64s(
            0x006b_6579,
            &[
                self.version,
                u64::from(HASH_VERSION),
                self.inputs.unwrap_or_else(|| self.field.inputs_hash_for(id, self.map)),
                id.raw(),
                u64::from(self.cells),
                self.implementation_hash,
                self.mapping_hash,
                self.body_words[0],
                self.body_words[1],
                self.body_words[2],
            ],
        );
        let base = mesh_key_chain(height_key, self.cells, self.body_params, MESH_VERSION, SKIRT_FRACTION, self.climate_word);
        self.finest_level.map_or(base, |l| finest_class_key(base, id, l))
    }
}

/// The grid coordinates `(i, j)` of vertex `vi` of a tile of `cells` cells in the order of [`TileMesh::vertices`]: the grid row by row,
/// then the four skirt runs (south, north, west, east), each skirt vertex taking the coordinates of its border vertex.
pub fn vertex_grid_coords(cells: u32, vi: usize) -> (u32, u32) {
    let w = (cells + 1) as usize;
    if vi < w * w {
        return ((vi % w) as u32, (vi / w) as u32);
    }
    let r = vi - w * w;
    let (k, t) = ((r / w) as u32, (r % w) as u32);
    match k {
        0 => (t, 0),
        1 => (t, cells),
        2 => (0, t),
        _ => (cells, t),
    }
}

impl TileMesh {
    /// The lowest height (m) over the tile's own `(cells + 1)^2` lattice vertices (the skirt vertices repeat their edge vertex's
    /// height): the exact minimum of the drawn terrain, which is linear between vertices and morphs only to averages of own
    /// vertices (ATMO-020).
    pub fn min_height_m(&self) -> f32 {
        self.vertices[..grid_vertex_count(self.cells)].iter().map(|v| v.height).fold(f32::INFINITY, f32::min)
    }

    /// The climate of vertex `vi` as the GPU vertex buffer carries it: the two `f16` of [`ClimateTexture::vertex_value`] at the
    /// vertex's grid coordinates (presentation data derived at upload time).
    pub fn gpu_climate(&self, vi: usize) -> [u16; 2] {
        let (i, j) = vertex_grid_coords(self.cells, vi);
        self.climate.vertex_value(i, j, self.cells)
    }

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

/// Depth by which the skirts of a tile of `level` hang below its surface, metres: `SKIRT_FRACTION` of the edge length on the
/// sphere of `radius_m`, at least 1 mm. Shared with the frustum cut (LOD-007), which must keep the skirts' triangles in view.
pub fn skirt_depth(level: u8, radius_m: f64) -> f64 {
    let edge_len = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level);
    (edge_len * SKIRT_FRACTION).max(1e-3)
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
    // Every direction the tile needs, evaluated by one batch call (GEN-020): the lattice points, the four normal-stencil
    // points of each, and the tile centre. Each height is a pure function of its direction, so the order the field uses
    // inside the call cannot change a bit.
    let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level()) / f64::from(cells);
    let count = width * width;
    let mut dirs = Vec::with_capacity(count * 5 + 1);
    for j in 0..=n {
        for i in 0..=n {
            dirs.push(id.sample_direction(map, cells, i as u32, j as u32));
        }
    }
    for k in 0..count {
        let st = normal_stencil(dirs[k], eps);
        dirs.extend(st);
    }
    let centre_dir = id.center_direction(map);
    dirs.push(centre_dir);
    let (all, climate) = field.heights_and_climate(map, id, &dirs, &|| false);
    for idx in 0..count {
        world[idx] = dirs[idx] * (radius_m + all[idx]);
        heights[idx] = all[idx];
    }
    let at = |i: i64, j: i64| world[j as usize * width + i as usize];
    // The origin sits at the tile centre including its terrain height, so tile-local offsets and the camera-relative
    // origin stay small (a few tile sizes), never terrain-height sized.
    let origin = PlanetFixed(centre_dir * (radius_m + all[count * 5]));
    let local = |p: Vec3| {
        let d = p - origin.0;
        [d.x as f32, d.y as f32, d.z as f32]
    };

    let mut vertices = Vec::with_capacity(vertex_count(cells));
    for j in 0..=n {
        for i in 0..=n {
            let k = j as usize * width + i as usize;
            let (s0, h0) = (count + 4 * k, count + 4 * k);
            let st = [dirs[s0], dirs[s0 + 1], dirs[s0 + 2], dirs[s0 + 3]];
            let normal = normal_from_stencil(&st, &[all[h0], all[h0 + 1], all[h0 + 2], all[h0 + 3]], radius_m);
            // Geomorph target on the parent grid (diagonal (i-1,j-1)-(i+1,j+1) matches the triangulation).
            let coarse = match (i % 2, j % 2) {
                (0, 0) => at(i, j),
                (1, 0) => (at(i - 1, j) + at(i + 1, j)) * 0.5,
                (0, 1) => (at(i, j - 1) + at(i, j + 1)) * 0.5,
                _ => (at(i - 1, j - 1) + at(i + 1, j + 1)) * 0.5,
            };
            let h = |i: i64, j: i64| heights[j as usize * width + i as usize];
            let coarse_height = match (i % 2, j % 2) {
                (0, 0) => h(i, j),
                (1, 0) => (h(i - 1, j) + h(i + 1, j)) * 0.5,
                (0, 1) => (h(i, j - 1) + h(i, j + 1)) * 0.5,
                _ => (h(i - 1, j - 1) + h(i + 1, j + 1)) * 0.5,
            };
            vertices.push(MeshVertex {
                pos: local(at(i, j)),
                coarse: local(coarse),
                normal: [normal.x as f32, normal.y as f32, normal.z as f32],
                height: h(i, j) as f32,
                coarse_height: coarse_height as f32,
            });
        }
    }
    // Skirts: duplicates of the border vertices, pushed towards the planet centre (south, north, west, east).
    let depth = skirt_depth(id.level(), radius_m);
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
    TileMesh { id, cells, key: mesh_key_field(field, map, id, cells, radius_m), origin, vertices, climate }
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
    /// Number of f32 values per vertex in GPU order: `pos`, `coarse`, `normal`, `height`, `coarse_height`.
    pub const FLOATS: usize = 11;
    /// Byte stride of a vertex buffer entry.
    pub const STRIDE: u64 = (Self::FLOATS * 4) as u64;
    /// The attribute offsets (in floats) of `pos`, `coarse`, `normal`, `height` and `coarse_height`.
    pub const OFFSETS: [usize; 5] = [0, 3, 6, 9, 10];
    /// Byte stride of a vertex in a GPU vertex buffer: the 11 floats and the climate of the vertex (temperature and precipitation,
    /// two `f16`, [`TileMesh::gpu_climate`]) at byte offset [`MeshVertex::STRIDE`]. The stored form of a tile stays [`STRIDE`](MeshVertex::STRIDE)
    /// bytes per vertex plus the 324-byte climate field; the GPU form carries the field's value at each vertex (ADR 0039 point 2).
    pub const GPU_STRIDE: u64 = Self::STRIDE + 4;

    pub fn to_floats(&self) -> [f32; Self::FLOATS] {
        let (p, c, n) = (self.pos, self.coarse, self.normal);
        [p[0], p[1], p[2], c[0], c[1], c[2], n[0], n[1], n[2], self.height, self.coarse_height]
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
    let st = normal_stencil(dir, eps);
    let h = field.heights(&st);
    normal_from_stencil(&st, &[h[0], h[1], h[2], h[3]], radius_m)
}

/// The tangent frame of [`surface_normal`] at `dir`.
fn tangent_frame(dir: Vec3) -> (Vec3, Vec3) {
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
    (t1, dir.cross(t1))
}

/// The four directions whose heights give the normal at `dir`: `+t1`, `-t1`, `+t2`, `-t2` at the angular step `eps`. Batch
/// builders evaluate them together with the other samples of a tile (GEN-020) and combine them with [`normal_from_stencil`].
pub fn normal_stencil(dir: Vec3, eps: f64) -> [Vec3; 4] {
    let (t1, t2) = tangent_frame(dir);
    let p = |t: Vec3, sign: f64| (dir + t * (sign * eps)).normalized();
    [p(t1, 1.0), p(t1, -1.0), p(t2, 1.0), p(t2, -1.0)]
}

/// The outward normal from the heights `h` at the stencil directions `st` ([`normal_stencil`]): the same arithmetic as
/// [`surface_normal_field`].
pub fn normal_from_stencil(st: &[Vec3; 4], h: &[f64; 4], radius_m: f64) -> Vec3 {
    let q = |k: usize| st[k] * (radius_m + h[k]);
    (q(0) - q(1)).cross(q(2) - q(3)).normalized()
}
