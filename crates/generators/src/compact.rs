//! The compact tile format (ADR 0033 point 4, SPIKE-06, REND-016): one texel per grid vertex plus a 128-byte record instead of 44
//! bytes per vertex, for levels 3 and finer, with the exact morph target.
//!
//! No vertex buffer exists on the GPU for such a tile: the vertex shader derives `(i, j)` (or the skirt vertex) from the vertex
//! index, reads the texel of the vertex and of the neighbours the morph needs, and places the vertex on a biquadratic base
//! surface through nine tile-local control points plus the tile-relative height along the base normal. This module is the
//! GPU-free builder (CON-02/CON-03: decisions live here, the renderer executes them) and the CPU mirror of the shader.
//!
//! A texel is `Rgba16Uint`, 8 bytes: `[height, normal_x, normal_y, reserved]`. The height is 16 bits between the tile's own
//! minimum and maximum (`hr_min + q * hr_step`, relative to the tile origin's height); the normal is two snorm16 octahedral
//! components (`i16` bits in a `u16`), 0.0037 degrees at worst, measured (CON-14: 0.01 degrees). The spare 16 bits are zero and reserved.
//!
//! CON-11: everything the shader sees is tile-local (control points, heights relative to the tile origin) or a unit vector; the
//! control points are `shell * dir - origin` rounded once from f64.
//!
//! CON-13: the height step is `relief / 65,535`, so 1 mm holds while a finest tile has at most [`FALLBACK_RELIEF_M`] of relief;
//! a finest tile with more falls back to the vertex format ([`tile_data_field`]).

use crate::height::HeightField;
use crate::mesh::{
    finest_class_key, mesh_key_field, normal_from_stencil, normal_stencil, skirt_depth, tile_mesh_field, vertex_count, MeshVertex, TileMesh,
};
use planet_core::cube::FaceMapping;
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};

/// Tiles of level 0 to this level less one keep the vertex format: the compact format starts at level 3 (the biquadratic base
/// surface errs by about 0.018 theta^2 of the tile edge, too much for levels 0-2; ADR 0033 point 4).
pub const COMPACT_MIN_LEVEL: u8 = 3;

/// Bytes of one texel (`Rgba16Uint`).
pub const TEXEL_BYTES: u64 = 8;

/// Bytes of the per-slot record: nine control points (27 f32), `hr_min`, `hr_step`, `h_origin`, `skirt depth` and the flip flag.
pub const COMPACT_RECORD_BYTES: u64 = 9 * 12 + 4 * 4 + 4;

/// Quantisation levels of the height: 16 bits.
pub const HEIGHT_STEPS: f64 = 65_535.0;

/// CON-13: tile-vertex quantisation at the finest level at most this many metres.
pub const MAX_FINEST_STEP_M: f64 = 1.0e-3;

/// Relief (metres) above which a finest tile's height step exceeds [`MAX_FINEST_STEP_M`]: such a tile falls back to the vertex
/// format (ADR 0033 point 5).
pub const FALLBACK_RELIEF_M: f64 = MAX_FINEST_STEP_M * HEIGHT_STEPS;

/// Texels of a slot at `cells`: `(cells + 1)^2`.
pub fn texel_count(cells: u32) -> usize {
    ((cells + 1) * (cells + 1)) as usize
}

/// Bytes a compact tile occupies in its atlas slot at `cells` (texels only).
pub fn slot_texel_bytes(cells: u32) -> u64 {
    texel_count(cells) as u64 * TEXEL_BYTES
}

/// Texel bytes plus the record: what ADR 0033 calls the 8.8 KB tile at cells 32.
pub fn slot_bytes(cells: u32) -> u64 {
    slot_texel_bytes(cells) + COMPACT_RECORD_BYTES
}

/// One compact tile: control points, quantised heights and normals.
#[derive(Clone, Debug, PartialEq)]
pub struct CompactTile {
    pub id: TileId,
    pub cells: u32,
    /// The mesh key of the tile ([`mesh_key_field`]): a tile has one key whichever format it is drawn in, so a plan's key
    /// matches the resident tile.
    pub key: u64,
    /// Tile centre at its terrain height (identical to `TileMesh::origin`).
    pub origin: PlanetFixed,
    /// Control points `(a, b)` at `ctrl[3 * b + a]`: lattice `(a * cells / 2, b * cells / 2)`, tile-local, on the shell through the
    /// origin.
    pub ctrl: [[f32; 3]; 9],
    /// Height of the origin above the sphere (the field at the tile centre).
    pub h_origin: f32,
    /// Height (relative to `h_origin`) of quantisation level 0.
    pub hr_min: f32,
    /// Height step per quantisation level.
    pub hr_step: f32,
    pub skirt_depth: f32,
    /// True when the base surface's `du x dv` points into the planet.
    pub flip: bool,
    /// Relief of the tile in metres (maximum minus minimum height, f64): what the CON-13 guard looks at.
    pub relief_m: f64,
    /// `(cells + 1)^2` texels `[height, normal_x, normal_y, 0]`, row `j` major.
    pub texels: Vec<[u16; 4]>,
}

impl CompactTile {
    /// Bytes this tile occupies in the atlas.
    pub fn texel_bytes(&self) -> u64 {
        self.texels.len() as u64 * TEXEL_BYTES
    }

    /// Texel bytes plus the record.
    pub fn resident_bytes(&self) -> u64 {
        self.texel_bytes() + COMPACT_RECORD_BYTES
    }

    /// The texels as the atlas holds them: little-endian `u16` x 4 per texel, row `j` major.
    pub fn texel_data(&self) -> Vec<u8> {
        self.texels.iter().flat_map(|t| t.iter().flat_map(|c| c.to_le_bytes())).collect()
    }

    /// The record as the slot-record buffer holds it: 9 control points as f32 triples, `hr_min`, `hr_step`, `h_origin`,
    /// `skirt_depth`, then the flip flag as a `u32`; 128 bytes.
    pub fn record_bytes(&self) -> [u8; COMPACT_RECORD_BYTES as usize] {
        let mut out = [0u8; COMPACT_RECORD_BYTES as usize];
        let mut words = self.ctrl.iter().flatten().copied().collect::<Vec<f32>>();
        words.extend([self.hr_min, self.hr_step, self.h_origin, self.skirt_depth]);
        for (k, w) in words.iter().enumerate() {
            out[k * 4..k * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        out[124..128].copy_from_slice(&u32::from(self.flip).to_le_bytes());
        out
    }

    /// Height of a texel relative to the origin height, as the shader computes it.
    pub fn rel_height(&self, i: usize, j: usize) -> f32 {
        self.hr_min + f32::from(self.texels[j * (self.cells as usize + 1) + i][0]) * self.hr_step
    }

    /// The largest error the 16-bit height quantisation can cause: half a step.
    pub fn height_error_bound_m(&self) -> f64 {
        0.5 * f64::from(self.hr_step)
    }

    /// Whether the CON-13 step bound (1 mm) holds for this tile.
    pub fn step_within_con13(&self) -> bool {
        f64::from(self.hr_step) <= MAX_FINEST_STEP_M * (1.0 + 1.0e-6)
    }

    /// Hash of the exact texel, record and origin bits, for known-answer tests (CON-14).
    pub fn content_hash(&self) -> u64 {
        let mut v = vec![
            self.id.raw(),
            u64::from(self.cells),
            self.key,
            self.origin.0.x.to_bits(),
            self.origin.0.y.to_bits(),
            self.origin.0.z.to_bits(),
        ];
        v.extend(self.record_bytes().chunks(4).map(|c| u64::from(u32::from_le_bytes(c.try_into().expect("4 bytes")))));
        v.extend(self.texels.iter().map(|t| u64::from(t[0]) | u64::from(t[1]) << 16 | u64::from(t[2]) << 32 | u64::from(t[3]) << 48));
        hash_u64s(0x0063_706b, &v)
    }
}

/// A tile as the renderer takes it: the vertex format (levels 0-2, and fallback tiles) or the compact format.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderTile {
    Vertex(TileMesh),
    Compact(CompactTile),
}

impl RenderTile {
    pub fn id(&self) -> TileId {
        match self {
            RenderTile::Vertex(m) => m.id,
            RenderTile::Compact(c) => c.id,
        }
    }

    pub fn key(&self) -> u64 {
        match self {
            RenderTile::Vertex(m) => m.key,
            RenderTile::Compact(c) => c.key,
        }
    }

    pub fn cells(&self) -> u32 {
        match self {
            RenderTile::Vertex(m) => m.cells,
            RenderTile::Compact(c) => c.cells,
        }
    }

    /// Tile centre at its terrain height, planet-fixed f64 (the origin of the plan's camera-relative offsets).
    pub fn origin(&self) -> PlanetFixed {
        match self {
            RenderTile::Vertex(m) => m.origin,
            RenderTile::Compact(c) => c.origin,
        }
    }

    pub fn is_compact(&self) -> bool {
        matches!(self, RenderTile::Compact(_))
    }
}

/// Why a tile is in the vertex format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexReason {
    /// Levels 0-2: the base surface model error is too large.
    CoarseLevel,
    /// A finest tile whose relief makes the 16-bit step exceed 1 mm (CON-13, ADR 0033 point 5).
    Relief,
}

/// The format decision of a tile, from its level and relief alone (pure; CON-03). `relief_m` is only read at the finest level.
/// `finest_level` is the LOD parameter `max_level`.
pub fn vertex_reason(level: u8, finest_level: u8, relief_m: f64) -> Option<VertexReason> {
    if level < COMPACT_MIN_LEVEL {
        Some(VertexReason::CoarseLevel)
    } else if level >= finest_level && relief_m / HEIGHT_STEPS > MAX_FINEST_STEP_M {
        Some(VertexReason::Relief)
    } else {
        None
    }
}

// ---- encoding ----

/// Octahedral encoding of a unit vector into two snorm16 components (as `u16` bit patterns of `i16`).
pub fn oct_encode(n: Vec3) -> [u16; 2] {
    let l1 = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / l1, n.y / l1);
    if n.z < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sx, (1.0 - x.abs()) * sy);
    }
    let q = |v: f64| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16;
    [q(x), q(y)]
}

/// Decode as the shader does (`unpack2x16snorm`, then the octahedral fold, f32).
pub fn oct_decode(e: [u16; 2]) -> [f32; 3] {
    let s = |b: u16| (f32::from(b as i16) / 32767.0).max(-1.0);
    let (x, y) = (s(e[0]), s(e[1]));
    let mut n = [x, y, 1.0 - x.abs() - y.abs()];
    if n[2] < 0.0 {
        let (sx, sy) = (if n[0] >= 0.0 { 1.0 } else { -1.0 }, if n[1] >= 0.0 { 1.0 } else { -1.0 });
        n = [(1.0 - n[1].abs()) * sx, (1.0 - n[0].abs()) * sy, n[2]];
    }
    norm(n)
}

/// The largest f32 not above `x`.
fn f32_down(x: f64) -> f32 {
    let r = x as f32;
    if f64::from(r) > x {
        f32::from_bits(if r > 0.0 { r.to_bits() - 1 } else { r.to_bits() + 1 })
    } else {
        r
    }
}

/// The smallest f32 not below `x`.
fn f32_up(x: f64) -> f32 {
    let r = x as f32;
    if f64::from(r) < x {
        f32::from_bits(if r >= 0.0 { r.to_bits() + 1 } else { r.to_bits() - 1 })
    } else {
        r
    }
}

/// Build the compact tile from a height field (all geometry in f64, rounded once).
pub fn compact_tile_field(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> CompactTile {
    assert!(cells >= 2 && cells.is_power_of_two(), "cells must be a power of two >= 2");
    let w = (cells + 1) as usize;
    let centre = id.center_direction(map);
    // One batch call for every height the tile needs (GEN-020): the origin, the lattice points and the normal stencils.
    let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level()) / f64::from(cells);
    let count = w * w;
    let mut dirs = Vec::with_capacity(1 + count * 5);
    dirs.push(centre);
    for j in 0..=cells {
        for i in 0..=cells {
            dirs.push(id.sample_direction(map, cells, i, j));
        }
    }
    for k in 0..count {
        let st = normal_stencil(dirs[1 + k], eps);
        dirs.extend(st);
    }
    let all = field.heights(&dirs);
    let h_origin = all[0];
    let origin = PlanetFixed(centre * (radius_m + h_origin));
    // The shell through the origin: control points are small tile-local numbers (CON-11), not heights from the sea level.
    let shell = radius_m + h_origin;
    let half = cells / 2;
    let mut ctrl = [[0f32; 3]; 9];
    for b in 0..3 {
        for a in 0..3 {
            let p = id.sample_direction(map, cells, a * half, b * half) * shell - origin.0;
            ctrl[3 * b as usize + a as usize] = [p.x as f32, p.y as f32, p.z as f32];
        }
    }
    let mut hr = vec![0f64; count];
    let mut normals = vec![[0u16; 2]; count];
    for k in 0..count {
        hr[k] = all[1 + k] - h_origin;
        let s0 = 1 + count + 4 * k;
        let st = [dirs[s0], dirs[s0 + 1], dirs[s0 + 2], dirs[s0 + 3]];
        normals[k] = oct_encode(normal_from_stencil(&st, &[all[s0], all[s0 + 1], all[s0 + 2], all[s0 + 3]], radius_m));
    }
    let lo = hr.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = hr.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // Rounded so that level 0 is not above the lowest height and level 65,535 not below the highest: no clamping error.
    let hr_min = f32_down(lo);
    let hr_step = if hi > lo { f32_up((hi - f64::from(hr_min)) / HEIGHT_STEPS) } else { 1.0e-30 };
    let texels = hr
        .iter()
        .zip(&normals)
        .map(|(h, n)| {
            let q = ((h - f64::from(hr_min)) / f64::from(hr_step)).round().clamp(0.0, HEIGHT_STEPS) as u16;
            [q, n[0], n[1], 0]
        })
        .collect();
    let mut tile = CompactTile {
        id,
        cells,
        key: mesh_key_field(field, map, id, cells, radius_m),
        origin,
        ctrl,
        h_origin: h_origin as f32,
        hr_min,
        hr_step,
        skirt_depth: skirt_depth(id.level(), radius_m) as f32,
        flip: false,
        relief_m: hi - lo,
        texels,
    };
    let (du, dv) = tile.base_derivs(0.5, 0.5);
    let c = [centre.x as f32, centre.y as f32, centre.z as f32];
    let d = cross(du, dv);
    tile.flip = d[0] * c[0] + d[1] * c[1] + d[2] * c[2] < 0.0;
    tile
}

/// The vertex-format mesh of a tile for a view whose finest level is `finest_level`: [`tile_mesh_field`] with the key of its
/// class ([`finest_class_key`]), so a vertex tile and a compact tile of the same view carry the keys the plan expects.
pub fn vertex_tile_field(
    field: &dyn HeightField,
    map: &dyn FaceMapping,
    id: TileId,
    cells: u32,
    radius_m: f64,
    finest_level: u8,
) -> TileMesh {
    let mut mesh = tile_mesh_field(field, map, id, cells, radius_m);
    mesh.key = finest_class_key(mesh.key, id, finest_level);
    mesh
}

/// The tile as the renderer will hold it: the vertex format for levels 0-2 and for a finest tile whose relief breaks CON-13's
/// 1 mm (the fallback, ADR 0033 point 5), the compact format otherwise. `finest_level` is the LOD parameter `max_level`. The
/// tile carries the key of its tile and class ([`finest_class_key`]): the format decision is a function of that key (tile, field,
/// "at or deeper than the cap") and the tile's own relief, so a resident tile reused by its key was decided for the same class.
pub fn tile_data_field(
    field: &dyn HeightField,
    map: &dyn FaceMapping,
    id: TileId,
    cells: u32,
    radius_m: f64,
    finest_level: u8,
) -> RenderTile {
    let vertex = || RenderTile::Vertex(vertex_tile_field(field, map, id, cells, radius_m, finest_level));
    if vertex_reason(id.level(), finest_level, 0.0).is_some() {
        return vertex();
    }
    let mut tile = compact_tile_field(field, map, id, cells, radius_m);
    match vertex_reason(id.level(), finest_level, tile.relief_m) {
        Some(_) => vertex(),
        None => {
            tile.key = finest_class_key(tile.key, id, finest_level);
            RenderTile::Compact(tile)
        }
    }
}

// ---- CPU mirror of the vertex shader (f32; same operations, but the shader may fuse multiply-adds, so it is not bit-exact with the GPU) ----

type V = [f32; 3];
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V) -> V {
    scale(a, 1.0 / (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt())
}

fn basis(t: f32) -> [f32; 3] {
    [(1.0 - t) * (1.0 - 2.0 * t), 4.0 * t * (1.0 - t), t * (2.0 * t - 1.0)]
}

fn dbasis(t: f32) -> [f32; 3] {
    [4.0 * t - 3.0, 4.0 - 8.0 * t, 4.0 * t - 1.0]
}

#[allow(clippy::needless_range_loop)]
impl CompactTile {
    fn base_point(&self, u: f32, v: f32) -> V {
        let (bu, bv) = (basis(u), basis(v));
        let mut p = [0f32; 3];
        for b in 0..3 {
            for a in 0..3 {
                p = add(p, scale(self.ctrl[3 * b + a], bu[a] * bv[b]));
            }
        }
        p
    }

    fn base_derivs(&self, u: f32, v: f32) -> (V, V) {
        let (bu, bv, du, dv) = (basis(u), basis(v), dbasis(u), dbasis(v));
        let (mut pu, mut pv) = ([0f32; 3], [0f32; 3]);
        for b in 0..3 {
            for a in 0..3 {
                pu = add(pu, scale(self.ctrl[3 * b + a], du[a] * bv[b]));
                pv = add(pv, scale(self.ctrl[3 * b + a], bu[a] * dv[b]));
            }
        }
        (pu, pv)
    }

    fn base_normal(&self, u: f32, v: f32) -> V {
        let (pu, pv) = self.base_derivs(u, v);
        let n = norm(cross(pu, pv));
        if self.flip {
            scale(n, -1.0)
        } else {
            n
        }
    }

    /// Position (tile-local) and tile-relative height of lattice vertex `(i, j)`.
    fn place(&self, i: i64, j: i64) -> (V, f32) {
        let c = self.cells as f32;
        let (u, v) = (i as f32 / c, j as f32 / c);
        let hr = self.rel_height(i as usize, j as usize);
        (add(self.base_point(u, v), scale(self.base_normal(u, v), hr)), hr)
    }

    /// What the vertex shader computes for vertex `vi` of the shared grid (grid vertices, then the four skirt runs): position,
    /// morph target, normal and heights, in the vertex format's terms.
    pub fn shade_vertex(&self, vi: usize) -> MeshVertex {
        let cells = self.cells as i64;
        let w = cells + 1;
        let grid = (w * w) as usize;
        let (i, j, skirt) = if vi < grid {
            (vi as i64 % w, vi as i64 / w, false)
        } else {
            let r = (vi - grid) as i64;
            let (k, t) = (r / w, r % w);
            match k {
                0 => (t, 0, true),
                1 => (t, cells, true),
                2 => (0, t, true),
                _ => (cells, t, true),
            }
        };
        let c = self.cells as f32;
        let n_own = self.base_normal(i as f32 / c, j as f32 / c);
        let (own_p, own_h) = self.place(i, j);
        let avg = |a: (i64, i64), b: (i64, i64)| {
            let (pa, ha) = self.place(a.0, a.1);
            let (pb, hb) = self.place(b.0, b.1);
            (scale(add(pa, pb), 0.5), (ha + hb) * 0.5)
        };
        let (cp, ch) = match (i & 1, j & 1) {
            (0, 0) => (own_p, own_h),
            (1, 0) => avg((i - 1, j), (i + 1, j)),
            (0, 1) => avg((i, j - 1), (i, j + 1)),
            _ => avg((i - 1, j - 1), (i + 1, j + 1)),
        };
        let (mut pos, mut cpos) = (own_p, cp);
        if skirt {
            let d = scale(n_own, self.skirt_depth);
            pos = sub(pos, d);
            cpos = sub(cpos, d);
        }
        let t = self.texels[j as usize * w as usize + i as usize];
        let normal = oct_decode([t[1], t[2]]);
        MeshVertex { pos, coarse: cpos, normal, height: self.h_origin + own_h, coarse_height: self.h_origin + ch }
    }

    /// Every vertex as the shader computes it, in the vertex format's order.
    pub fn shade_all(&self) -> Vec<MeshVertex> {
        (0..vertex_count(self.cells)).map(|vi| self.shade_vertex(vi)).collect()
    }
}

impl CompactTile {
    /// Whether this tile may be drawn as a tile of a view whose finest level is `finest_level`: the format decision depends on that
    /// LOD parameter. Keys from [`tile_data_field`] carry the class ([`finest_class_key`]), so a resident tile is never reused across
    /// classes; this check is for a tile built with [`compact_tile_field`] (class-free key) that a consumer holds across views.
    pub fn allowed_at_finest_level(&self, finest_level: u8) -> bool {
        vertex_reason(self.id.level(), finest_level, self.relief_m).is_none()
    }
}
