//! SPIKE-06 (docs/archive/M4/spikes.md): an experimental compact tile format. NOT production code, behind the `spike06` feature, never
//! used by the product renderer.
//!
//! One shared grid (no vertex buffer at all: the vertex shader derives `(i, j)` or the skirt vertex from `vertex_index`, the
//! index buffer is today's `grid_indices`) plus, per tile, one `R32Uint` texel per grid vertex in a pooled atlas:
//! bits 16..31 the tile-relative height (16 bits between the tile's own min and max), bits 0..15 the surface normal as two
//! snorm8 octahedral components. The position is a biquadratic base surface through nine tile-local control points
//! (corners, edge midpoints and centre of the tile's sea-level shell at the origin's height, rounded from f64 once) plus the
//! tile-relative height along the base surface normal. The coarse (parent-grid) position and height of the geomorph are the
//! same average of neighbouring vertices that `tile_mesh_field` builds, computed in the shader from neighbouring texels, so
//! neither `coarse` nor `coarse_height` is stored. Skirts are the border vertices pushed down along the base normal.
//!
//! CON-11: every quantity the shader sees is tile-local (control points, heights relative to the tile origin) or a unit
//! vector; the planet radius never reaches it (the control points are `R * dir - origin` rounded in f64 on the CPU).

use crate::terrain::{grid_phases, GRID_SPACINGS_M, TERRAIN_SHADERS};
use crate::{read_rgba8, ExecError, FrameTiming, GpuContext, RgbaFrame, TerrainStats};
use planet_core::cube::FaceMapping;
use planet_core::hash::hash_u64s;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::TerrainPlan;
use planet_generators::height::HeightField;
use planet_generators::mesh::{grid_indices, mesh_key_field, surface_normal_field, vertex_count, MeshVertex, SKIRT_FRACTION};
use std::collections::HashMap;

/// Whether the cheaper morph target is selected (`SPIKE06_VARIANT=approx`): the height offsets of a vertex's neighbours use the
/// vertex's own base normal instead of their own. Changes the shader and its CPU mirror alike.
pub fn approx_neighbours() -> bool {
    std::env::var("SPIKE06_VARIANT").is_ok_and(|v| v.contains("approx"))
}

/// Version of the compact construction (the spike's own; a product format would fold it into MESH_VERSION).
pub const COMPACT_VERSION: u32 = 1;

/// Bytes of the per-node uniform of the compact pipeline: the product's node block (origin, colour, morph, nine decade phases:
/// 192 B), nine control points (144 B), `tp` (16 B) and `ti` (16 B).
pub const NODE_BYTES: u64 = 48 + 16 * GRID_SPACINGS_M.len() as u64 + 16 * 9 + 32;

/// Bytes of per-tile data that are not texels: the nine control points as f32 triples, hr_min, hr_step, h_origin, skirt depth
/// and the flip flag: what a resident slot has to remember besides its texels (the product node uniform stays per draw).
pub const TILE_RECORD_BYTES: u64 = 9 * 12 + 4 * 4 + 4;

/// One compact tile: control points, quantised heights and normals.
#[derive(Clone, Debug, PartialEq)]
pub struct CompactTile {
    pub id: TileId,
    pub cells: u32,
    pub key: u64,
    /// Tile centre at its terrain height (identical to `TileMesh::origin`).
    pub origin: PlanetFixed,
    /// Control points `(a, b)` at `ctrl[3 * b + a]`: lattice `(a * cells / 2, b * cells / 2)`, tile-local.
    pub ctrl: [[f32; 3]; 9],
    /// Height of the origin above the sphere (the field at the tile centre).
    pub h_origin: f32,
    pub hr_min: f32,
    pub hr_step: f32,
    pub skirt_depth: f32,
    /// True when the base surface's `du x dv` points into the planet.
    pub flip: bool,
    /// `(cells + 1)^2` texels, row `j` major.
    pub texels: Vec<u32>,
}

impl CompactTile {
    /// Bytes this tile occupies in the atlas.
    pub fn texel_bytes(&self) -> u64 {
        self.texels.len() as u64 * 4
    }

    /// Texel bytes plus the record.
    pub fn resident_bytes(&self) -> u64 {
        self.texel_bytes() + TILE_RECORD_BYTES
    }

    pub fn hr_of(&self, texel: u32) -> f32 {
        self.hr_min + (texel >> 16) as f32 * self.hr_step
    }
}

fn oct_encode(n: Vec3) -> u16 {
    let l1 = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / l1, n.y / l1);
    if n.z < 0.0 {
        let sx = if x >= 0.0 { 1.0 } else { -1.0 };
        let sy = if y >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sx, (1.0 - x.abs()) * sy);
    }
    let q = |v: f64| ((v.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8;
    u16::from(q(x)) | (u16::from(q(y)) << 8)
}

fn oct_decode(bits: u16) -> [f32; 3] {
    let s = |b: u8| f32::from(b as i8).max(-127.0) / 127.0;
    let (x, y) = (s((bits & 0xff) as u8), s((bits >> 8) as u8));
    let mut n = [x, y, 1.0 - x.abs() - y.abs()];
    if n[2] < 0.0 {
        let (sx, sy) = (if n[0] >= 0.0 { 1.0 } else { -1.0 }, if n[1] >= 0.0 { 1.0 } else { -1.0 });
        n = [(1.0 - n[1].abs()) * sx, (1.0 - n[0].abs()) * sy, n[2]];
    }
    norm(n)
}

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

/// Build the compact tile from a height field (all geometry in f64, rounded once).
pub fn compact_tile(field: &dyn HeightField, map: &dyn FaceMapping, id: TileId, cells: u32, radius_m: f64) -> CompactTile {
    assert!(cells >= 2 && cells.is_power_of_two(), "cells must be a power of two >= 2");
    let w = (cells + 1) as usize;
    let centre = id.center_direction(map);
    let h_origin = field.height(centre);
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
    let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level()) / f64::from(cells);
    let mut hr = vec![0f64; w * w];
    let mut normals = vec![0u16; w * w];
    for j in 0..=cells {
        for i in 0..=cells {
            let dir = id.sample_direction(map, cells, i, j);
            hr[j as usize * w + i as usize] = field.height(dir) - h_origin;
            normals[j as usize * w + i as usize] = oct_encode(surface_normal_field(field, dir, eps, radius_m));
        }
    }
    let lo = hr.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = hr.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let hr_min = lo as f32;
    let hr_step = if hi > lo { ((hi - lo) / 65535.0) as f32 } else { 1.0e-30 };
    let texels = hr
        .iter()
        .zip(&normals)
        .map(|(h, n)| {
            let q = ((h - f64::from(hr_min)) / f64::from(hr_step)).round().clamp(0.0, 65535.0) as u32;
            (q << 16) | u32::from(*n)
        })
        .collect();
    let edge_len = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
    let skirt_depth = (edge_len * SKIRT_FRACTION).max(1e-3) as f32;
    let mut tile =
        CompactTile { id, cells, key: 0, origin, ctrl, h_origin: h_origin as f32, hr_min, hr_step, skirt_depth, flip: false, texels };
    let (du, dv) = tile.base_derivs(0.5, 0.5);
    let c = [centre.x as f32, centre.y as f32, centre.z as f32];
    let d = cross(du, dv);
    tile.flip = d[0] * c[0] + d[1] * c[1] + d[2] * c[2] < 0.0;
    tile.key = hash_u64s(0x0063_6f6d, &[u64::from(COMPACT_VERSION), mesh_key_field(field, map, id, cells, radius_m)]);
    tile
}

// ---- CPU mirror of the vertex shader (f32; same operations, but the shader sums rows first and may fuse multiply-adds, so it is not bit-exact with the GPU) ----
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

    fn rel_height(&self, i: i64, j: i64) -> f32 {
        self.hr_of(self.texels[j as usize * (self.cells as usize + 1) + i as usize])
    }

    /// Position (tile-local) and tile-relative height of lattice vertex `(i, j)`.
    fn place(&self, i: i64, j: i64) -> (V, f32) {
        let c = self.cells as f32;
        let (u, v) = (i as f32 / c, j as f32 / c);
        let hr = self.rel_height(i, j);
        (add(self.base_point(u, v), scale(self.base_normal(u, v), hr)), hr)
    }

    /// What the vertex shader computes for vertex `vi` of the shared grid (grid vertices, then the four skirt runs).
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
        let approx = approx_neighbours();
        let c = self.cells as f32;
        let n_own = self.base_normal(i as f32 / c, j as f32 / c);
        let at = |a: i64, b: i64| {
            if approx {
                let hr = self.rel_height(a, b);
                (add(self.base_point(a as f32 / c, b as f32 / c), scale(n_own, hr)), hr)
            } else {
                self.place(a, b)
            }
        };
        let (own_p, own_h) = at(i, j);
        let avg = |a: (i64, i64), b: (i64, i64)| {
            let (pa, ha) = at(a.0, a.1);
            let (pb, hb) = at(b.0, b.1);
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
        let normal = oct_decode((self.texels[j as usize * w as usize + i as usize] & 0xffff) as u16);
        MeshVertex { pos, coarse: cpos, normal, height: self.h_origin + own_h, coarse_height: self.h_origin + ch }
    }

    pub fn shade_all(&self) -> Vec<MeshVertex> {
        (0..vertex_count(self.cells)).map(|vi| self.shade_vertex(vi)).collect()
    }
}

// ---- GPU ----

/// The compact pipeline's shader: the product shader's fragment stage, header and helper functions unchanged; the vertex stage
/// replaced by vertex pulling from the atlas, and the node block extended. Built by splicing the product source so that the
/// two pipelines cannot drift apart in the fragment stage.
pub fn compact_shader() -> String {
    let src = TERRAIN_SHADERS[0].1;
    let vs_at = src.find("@vertex").expect("product shader has a vertex stage");
    let tail_at = src.find("// Distance in pixels to the nearest integer").expect("product shader helper marker");
    let mut head = src[..vs_at].to_string();
    let node_old = "    phase: array<vec4<f32>, 9>,\n};";
    assert!(head.contains(node_old), "product node block changed");
    head = head.replace(
        node_old,
        "    phase: array<vec4<f32>, 9>,\n    // Spike: nine tile-local control points of the base surface; tp = (hr_min, hr_step, h_origin, skirt depth);\n    // ti = (atlas texel x, atlas texel y, cells, flip).\n    ctrl: array<vec4<f32>, 9>,\n    tp: vec4<f32>,\n    ti: vec4<u32>,\n};",
    );
    let bind_old = "@group(0) @binding(1) var<uniform> node: Node;";
    assert!(head.contains(bind_old), "product bindings changed");
    head = head.replace(bind_old, &format!("{bind_old}\n@group(0) @binding(2) var atlas: texture_2d<u32>;"));
    let mut vertex = VERTEX.to_string();
    // Spike diagnostics: SPIKE06_VARIANT=nomorph,nonormal,nofetch removes parts of the vertex stage to show where its time goes.
    let variant = std::env::var("SPIKE06_VARIANT").unwrap_or_default();
    if variant.contains("approx") {
        // Cheaper morph target: the neighbours' height offsets use this vertex's normal (see `approx_neighbours`).
        let reps = [
            (
                "let own_t = texel(i, j);
    let own = place(i, j, rel_height(own_t));",
                "let own_t = texel(i, j);
    let n = base_normal(f32(i) / f32(cells), f32(j) / f32(cells));
    let own = place_n(i, j, rel_height(own_t), n);",
            ),
            (
                "place(i - 1, j, rel_height(texel(i - 1, j))) + place(i + 1, j, rel_height(texel(i + 1, j)))",
                "place_n(i - 1, j, rel_height(texel(i - 1, j)), n) + place_n(i + 1, j, rel_height(texel(i + 1, j)), n)",
            ),
            (
                "place(i, j - 1, rel_height(texel(i, j - 1))) + place(i, j + 1, rel_height(texel(i, j + 1)))",
                "place_n(i, j - 1, rel_height(texel(i, j - 1)), n) + place_n(i, j + 1, rel_height(texel(i, j + 1)), n)",
            ),
            (
                "place(i - 1, j - 1, rel_height(texel(i - 1, j - 1))) + place(i + 1, j + 1, rel_height(texel(i + 1, j + 1)))",
                "place_n(i - 1, j - 1, rel_height(texel(i - 1, j - 1)), n) + place_n(i + 1, j + 1, rel_height(texel(i + 1, j + 1)), n)",
            ),
            ("let d = base_normal(f32(i) / f32(cells), f32(j) / f32(cells)) * node.tp.w;", "let d = n * node.tp.w;"),
        ];
        for (from, to) in reps {
            assert!(vertex.contains(from), "approx variant: pattern not found: {from}");
            vertex = vertex.replace(from, to);
        }
    }
    if variant.contains("nomorph") {
        for pat in ["odd_i && !odd_j", "!odd_i && odd_j", "odd_i && odd_j"] {
            vertex = vertex.replace(pat, "false");
        }
    }
    if variant.contains("nonormal") {
        vertex = vertex
            .replace("return normalize(cross(pu, pv)) * select(1.0, -1.0, node.ti.w == 1u);", "return normalize(node.origin_rel.xyz);");
    }
    if variant.contains("nofetch") {
        vertex = vertex.replace("return textureLoad(atlas, vec2<i32>(i32(node.ti.x) + i, i32(node.ti.y) + j), 0).x;", "return 0u;");
    }
    format!("{head}{vertex}{}", &src[tail_at..])
}

const VERTEX: &str = r#"
fn basis(t: f32) -> vec3<f32> {
    return vec3<f32>((1.0 - t) * (1.0 - 2.0 * t), 4.0 * t * (1.0 - t), t * (2.0 * t - 1.0));
}

fn dbasis(t: f32) -> vec3<f32> {
    return vec3<f32>(4.0 * t - 3.0, 4.0 - 8.0 * t, 4.0 * t - 1.0);
}

// Row blends of the nine control points along u (row b = control points 3b..3b+2); unrolled on purpose: a loop over a
// uniform array indexed at run time costs several times more than the arithmetic (SPIKE-06 measurement).
fn row_blend(w: vec3<f32>, b: i32) -> vec3<f32> {
    return node.ctrl[b * 3].xyz * w.x + node.ctrl[b * 3 + 1].xyz * w.y + node.ctrl[b * 3 + 2].xyz * w.z;
}

fn base_point(u: f32, v: f32) -> vec3<f32> {
    let bu = basis(u);
    let bv = basis(v);
    return row_blend(bu, 0) * bv.x + row_blend(bu, 1) * bv.y + row_blend(bu, 2) * bv.z;
}

// Unit normal of the base surface at (u, v), outward.
fn base_normal(u: f32, v: f32) -> vec3<f32> {
    let bu = basis(u);
    let bv = basis(v);
    let du = dbasis(u);
    let dv = dbasis(v);
    let pv = row_blend(bu, 0) * dv.x + row_blend(bu, 1) * dv.y + row_blend(bu, 2) * dv.z;
    let pu = row_blend(du, 0) * bv.x + row_blend(du, 1) * bv.y + row_blend(du, 2) * bv.z;
    return normalize(cross(pu, pv)) * select(1.0, -1.0, node.ti.w == 1u);
}

fn texel(i: i32, j: i32) -> u32 {
    return textureLoad(atlas, vec2<i32>(i32(node.ti.x) + i, i32(node.ti.y) + j), 0).x;
}

fn rel_height(t: u32) -> f32 {
    return node.tp.x + f32(t >> 16u) * node.tp.y;
}

// Position (tile-local) of lattice vertex (i, j) with its tile-relative height `hr`, and the height itself in w.
fn place(i: i32, j: i32, hr: f32) -> vec4<f32> {
    let c = f32(node.ti.z);
    let u = f32(i) / c;
    let v = f32(j) / c;
    return vec4<f32>(base_point(u, v) + base_normal(u, v) * hr, hr);
}

// The same with the normal given (the approximate morph target uses the vertex's own normal for its neighbours).
fn place_n(i: i32, j: i32, hr: f32, n: vec3<f32>) -> vec4<f32> {
    let c = f32(node.ti.z);
    return vec4<f32>(base_point(f32(i) / c, f32(j) / c) + n * hr, hr);
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    let cells = i32(node.ti.z);
    let w = cells + 1;
    let idx = i32(vi);
    var i = 0;
    var j = 0;
    var skirt = false;
    if (idx < w * w) {
        i = idx % w;
        j = idx / w;
    } else {
        let r = idx - w * w;
        let k = r / w;
        let t = r % w;
        skirt = true;
        if (k == 0) {
            i = t;
        } else if (k == 1) {
            i = t;
            j = cells;
        } else if (k == 2) {
            j = t;
        } else {
            i = cells;
            j = t;
        }
    }
    let own_t = texel(i, j);
    let own = place(i, j, rel_height(own_t));
    // The parent-grid target: the average of the neighbours' positions, as tile_mesh_field builds it.
    var coarse = own;
    let odd_i = (i & 1) == 1;
    let odd_j = (j & 1) == 1;
    if (odd_i && !odd_j) {
        coarse = 0.5 * (place(i - 1, j, rel_height(texel(i - 1, j))) + place(i + 1, j, rel_height(texel(i + 1, j))));
    } else if (!odd_i && odd_j) {
        coarse = 0.5 * (place(i, j - 1, rel_height(texel(i, j - 1))) + place(i, j + 1, rel_height(texel(i, j + 1))));
    } else if (odd_i && odd_j) {
        coarse = 0.5 * (place(i - 1, j - 1, rel_height(texel(i - 1, j - 1))) + place(i + 1, j + 1, rel_height(texel(i + 1, j + 1))));
    }
    var pos = own.xyz;
    var cpos = coarse.xyz;
    if (skirt) {
        let d = base_normal(f32(i) / f32(cells), f32(j) / f32(cells)) * node.tp.w;
        pos = pos - d;
        cpos = cpos - d;
    }
    let m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let local = mix(pos, cpos, m);
    let p = node.origin_rel.xyz + local;
    let h = node.tp.z + own.w;
    let ch = node.tp.z + coarse.w;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = oct_decode(unpack4x8snorm(own_t & 0xffffu).xy);
    out.height = mix(h, ch, m);
    out.height_fine = h;
    out.morph = m;
    out.local = local;
    return out;
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        let s = select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));
        n = vec3<f32>((1.0 - abs(n.y)) * s.x, (1.0 - abs(n.x)) * s.y, n.z);
    }
    return normalize(n);
}

"#;

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

struct Timing {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}

/// Renderer of compact tiles: the product renderer's structure (one indexed draw per planned node, one dynamic node uniform)
/// with the vertex buffers replaced by one atlas texture of fixed-size slots.
pub struct CompactRenderer {
    size: (u32, u32),
    cells: u32,
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    nodes: wgpu::Buffer,
    node_capacity: usize,
    bind: wgpu::BindGroup,
    stride: u64,
    indices: wgpu::Buffer,
    index_count: u32,
    atlas: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    slot: u32,
    cols: u32,
    capacity: u32,
    resident: HashMap<TileId, (u64, u32, CompactTile)>,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    timing: Option<Timing>,
    /// Texel bytes written to the atlas so far.
    pub uploaded_bytes: u64,
}

impl CompactRenderer {
    /// `capacity` slots of `(cells + 1)^2` texels in one atlas texture (at most 8192 texels per side).
    pub fn new(ctx: &GpuContext, size: (u32, u32), cells: u32, capacity: u32) -> Self {
        use wgpu::util::DeviceExt;
        let device = &ctx.device;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let target = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = target("compact-color", format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let depth = target("compact-depth", wgpu::TextureFormat::Depth32Float, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let side = cells + 1;
        let cols = (8192 / side).min(capacity.max(1));
        let rows = capacity.div_ceil(cols).max(1);
        assert!(rows * side <= 8192, "atlas of {capacity} slots of {side}^2 texels does not fit 8192 texels per side");
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("compact-atlas"),
            size: wgpu::Extent3d { width: cols * side, height: rows * side, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let stride = NODE_BYTES.next_multiple_of(u64::from(device.limits().min_uniform_buffer_offset_alignment));
        let node_capacity = 256;
        let nodes = Self::node_buffer(device, stride, node_capacity);
        let source = compact_shader();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-compact"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let uniform = |binding, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: if dynamic { wgpu::BufferSize::new(NODE_BYTES) } else { None },
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compact-bgl"),
            entries: &[
                uniform(0, false),
                uniform(1, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind = Self::bind_group(device, &bgl, &globals, &nodes, &atlas_view);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compact-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compact-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let index_data = grid_indices(cells);
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("compact-indices"),
            contents: &index_data.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let timing = ctx.timestamps_supported().then(|| Timing {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("compact-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-readback"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        });
        Self {
            size,
            cells,
            pipeline,
            bgl,
            globals,
            nodes,
            node_capacity,
            bind,
            stride,
            indices,
            index_count: index_data.len() as u32,
            atlas,
            atlas_view,
            slot: 0,
            cols,
            capacity,
            resident: HashMap::new(),
            color,
            color_view,
            depth_view,
            timing,
            uploaded_bytes: 0,
        }
    }

    fn node_buffer(device: &wgpu::Device, stride: u64, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compact-nodes"),
            size: stride * capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        nodes: &wgpu::Buffer,
        atlas: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compact-bind"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: nodes,
                        offset: 0,
                        size: wgpu::BufferSize::new(NODE_BYTES),
                    }),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(atlas) },
            ],
        })
    }

    /// Write a tile into the next free slot (or its existing slot when the key changed).
    pub fn upload(&mut self, ctx: &GpuContext, tile: &CompactTile) -> Result<(), ExecError> {
        if tile.cells != self.cells {
            return Err(ExecError::BadResolution { id: tile.id, res: tile.cells });
        }
        if self.resident.get(&tile.id).is_some_and(|(key, _, _)| *key == tile.key) {
            return Ok(());
        }
        let slot = match self.resident.get(&tile.id) {
            Some((_, s, _)) => *s,
            None => {
                assert!(self.slot < self.capacity, "atlas full: {} slots", self.capacity);
                self.slot += 1;
                self.slot - 1
            }
        };
        let side = self.cells + 1;
        let (x, y) = ((slot % self.cols) * side, (slot / self.cols) * side);
        let bytes: Vec<u8> = tile.texels.iter().flat_map(|t| t.to_le_bytes()).collect();
        self.uploaded_bytes += bytes.len() as u64;
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * side), rows_per_image: Some(side) },
            wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
        );
        self.resident.insert(tile.id, (tile.key, slot, tile.clone_without_texels()));
        Ok(())
    }

    pub fn resident_tiles(&self) -> usize {
        self.resident.len()
    }

    /// Draw the plan (every planned tile must have been uploaded). With `timed`, GPU time is measured when supported.
    pub fn render(&mut self, ctx: &GpuContext, plan: &TerrainPlan, timed: bool) -> Result<(FrameTiming, TerrainStats), ExecError> {
        let started = std::time::Instant::now();
        if plan.size != self.size {
            return Err(ExecError::SizeMismatch { plan: plan.size, renderer: self.size });
        }
        for d in &plan.nodes {
            if !self.resident.contains_key(&d.tile) {
                return Err(ExecError::MissingTile(d.tile));
            }
        }
        let [lo, hi] = plan.height_range;
        let device = &ctx.device;
        if plan.nodes.len() > self.node_capacity {
            self.node_capacity = plan.nodes.len().next_power_of_two();
            self.nodes = Self::node_buffer(device, self.stride, self.node_capacity);
            self.bind = Self::bind_group(device, &self.bgl, &self.globals, &self.nodes, &self.atlas_view);
        }
        let mut globals = f32_bytes(&plan.view_projection);
        globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
        let view = planet_frame::TerrainView::ALL.iter().position(|x| *x == plan.view).expect("known view") as u32;
        globals.extend([view, 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
        ctx.queue.write_buffer(&self.globals, 0, &globals);
        let side = self.cells + 1;
        let mut nodes = vec![0u8; self.stride as usize * plan.nodes.len().max(1)];
        for (i, d) in plan.nodes.iter().enumerate() {
            let (_, slot, t) = &self.resident[&d.tile];
            let r = d.camera_relative_origin;
            let c = d.color.map(|v| f32::from(v) / 255.0);
            let mut floats = vec![r[0], r[1], r[2], 0.0, c[0], c[1], c[2], c[3], d.morph_start_m, d.morph_end_m, 0.0, 0.0];
            for p in grid_phases([d.origin.0.x, d.origin.0.y, d.origin.0.z]) {
                floats.extend([p[0], p[1], p[2], 0.0]);
            }
            for p in t.ctrl {
                floats.extend([p[0], p[1], p[2], 0.0]);
            }
            floats.extend([t.hr_min, t.hr_step, t.h_origin, t.skirt_depth]);
            let mut b = f32_bytes(&floats);
            for v in [(slot % self.cols) * side, (slot / self.cols) * side, self.cells, u32::from(t.flip)] {
                b.extend(v.to_le_bytes());
            }
            nodes[i * self.stride as usize..i * self.stride as usize + NODE_BYTES as usize].copy_from_slice(&b);
        }
        ctx.queue.write_buffer(&self.nodes, 0, &nodes);

        let timing = if timed { self.timing.as_ref() } else { None };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("compact") });
        let mut stats = TerrainStats::default();
        {
            let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compact-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: timing.map(|t| wgpu::RenderPassTimestampWrites {
                    query_set: &t.queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (i, _) in plan.nodes.iter().enumerate() {
                pass.set_bind_group(0, &self.bind, &[(i as u64 * self.stride) as u32]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
                stats.draw_calls += 1;
                stats.triangles += u64::from(self.index_count) / 3;
            }
        }
        if let Some(t) = timing {
            encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, 16);
        }
        ctx.queue.submit([encoder.finish()]);
        let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let total_ms = started.elapsed().as_secs_f64() * 1000.0;
        let gpu_ms = timing.and_then(|t| {
            let slice = t.readback.slice(..);
            slice.map_async(wgpu::MapMode::Read, |r| r.expect("timestamp map failed"));
            ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
            let data = slice.get_mapped_range().expect("timestamp range");
            let ticks = |k: usize| u64::from_le_bytes(data[k * 8..k * 8 + 8].try_into().expect("8 bytes"));
            let delta = ticks(1).saturating_sub(ticks(0));
            drop(data);
            t.readback.unmap();
            (delta > 0).then(|| delta as f64 * f64::from(ctx.queue.get_timestamp_period()) / 1.0e6)
        });
        Ok((FrameTiming { cpu_ms, total_ms, gpu_ms, ..Default::default() }, stats))
    }

    pub fn read_color(&self, ctx: &GpuContext) -> RgbaFrame {
        read_rgba8(ctx, &self.color, self.size.0, self.size.1)
    }
}

impl CompactTile {
    /// The record without its texels (what the renderer keeps per slot for the node uniform).
    fn clone_without_texels(&self) -> CompactTile {
        CompactTile { texels: Vec::new(), ..self.clone() }
    }
}
