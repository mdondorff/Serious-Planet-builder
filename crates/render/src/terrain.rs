//! Executes a `TerrainPlan` (CON-03): one indexed draw per planned node, camera-relative f32 positions, reversed-Z
//! `Depth32Float` with an infinite far plane (ADR 0002). The renderer makes no decisions: nodes, matrices, morph
//! intervals and colours all come from the plan. Shaders see only tile-local and camera-relative quantities (CON-11).

use crate::compact::{compact_terrain_shader, CompactGpu, REF_BYTES};
use crate::lit::LitPath;
use crate::{read_rgba8, ExecError, GpuContext, RgbaFrame};
use planet_frame::draw::{buffer_of, build_draw_list_placed, DrawBuffer, DRAW_RECORD_BYTES};
use planet_frame::pool::tier_pool;
use planet_frame::{TerrainPlan, TerrainView};
use planet_generators::compact::{CompactTile, RenderTile, COMPACT_RECORD_BYTES};
use planet_generators::mesh::{grid_indices, MeshVertex, TileMesh};
use planet_streaming::{PoolConfig, PoolKind, SlotPool, SlotPools, SlotRef, Tier};
use std::collections::HashMap;

/// Grid line spacings of the relief view (metres): 1 cm to 1,000 km, one decade apart.
pub const GRID_SPACINGS_M: [f64; 9] = [0.01, 0.1, 1.0, 10.0, 100.0, 1.0e3, 1.0e4, 1.0e5, 1.0e6];

/// Bytes of one node record (origin, colour, morph interval, one grid phase per decade) in the storage buffer the shader
/// indexes with the instance index.
const NODE_BYTES: u64 = 48 + 16 * GRID_SPACINGS_M.len() as u64 + 16;
const _: () = assert!(NODE_BYTES == planet_frame::draw::NODE_RECORD_BYTES, "the frame crate counts the same record size");

/// The relief grid's anchors for a tile origin (planet-fixed, f64): for each decade the fractional part of
/// `origin / spacing`, per axis, in cells. The shader adds `local / spacing` to it, so the line coordinate stays a small
/// number (a tile-local offset in cells plus a fraction) at every decade: adding a planet-radius-sized or even a
/// 1,000 km phase in metres would round to centimetres in f32 and turn the coarse decades into wide plateau bands. Two
/// tiles agree on where a grid line is (CON-11: fractions, never planet-radius values).
pub fn grid_phases(origin: [f64; 3]) -> [[f32; 3]; 9] {
    GRID_SPACINGS_M.map(|s| {
        origin.map(|v| {
            // The f32 rounding of a fraction just under 1 can reach 1.0 (the same line, but a phase is a fraction in [0, 1)).
            let f = (v / s).rem_euclid(1.0) as f32;
            if f >= 1.0 {
                0.0
            } else {
                f
            }
        })
    })
}

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    range: vec4<f32>,
    mode: vec4<u32>,
    // Lit path only: x = the water level W in metres (ADR 0041); the other lanes are reserved.
    extra: vec4<f32>,
    // Lit path only: the albedo table of the look (ADR 0039 point 3): eight colours with a threshold in the fourth lane, then
    // (snow_c_full, snow_c_none, snow_cos_none, snow_cos_full) and (beach_height, beach_fade, shore_blend, 0).
    albedo: array<vec4<f32>, 10>,
};
struct Node {
    origin_rel: vec4<f32>,
    color: vec4<f32>,
    // x, y: vertex distances between which a vertex morphs to the parent grid; z, w: padding (the plan's node-level
    // `morph` is a diagnostic of the CPU selection and is not read by the shader).
    morph: vec4<f32>,
    // Per grid decade (1 cm .. 1,000 km) the fractional part of (planet-fixed tile origin / spacing), xyz, taken in f64 on
    // the CPU: they anchor the relief grid to the world instead of to this tile, so neighbouring tiles and LOD splits draw
    // the same lines, and keep the line coordinates small.
    phase: array<vec4<f32>, 9>,
    // The unit up vector of the tile origin (planet-fixed axes) and the inverse of the origin's distance from the planet centre:
    // up at a point `local` metres from the origin is normalize(xyz + local * w), formed from tile-local numbers only (CON-11).
    up: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
// One record per planned node, read with the draw's instance index (`first_instance` = the node's index in the plan, REND-015).
@group(0) @binding(1) var<storage, read> nodes: array<Node>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) height: f32,
    @location(2) morph: f32,
    // Morphed position relative to the tile origin: small, tile-local, and what the relief grid is anchored to.
    @location(3) local: vec3<f32>,
    @location(4) height_fine: f32,
    @location(5) @interpolate(flat) node_index: u32,
    // The vertex format: the climate (temperature, precipitation) of the vertex. The compact format: the tile coordinates (u, v) of
    // the vertex, from which the fragment stage reads the tile's climate texture.
    @location(6) aux: vec2<f32>,
};

@vertex
fn vs(@builtin(instance_index) inst: u32, @location(0) pos: vec3<f32>, @location(1) coarse: vec3<f32>, @location(2) normal: vec3<f32>, @location(3) height: f32, @location(4) coarse_height: f32, @location(5) climate: vec2<f32>) -> VsOut {
    // The morph distance is measured to the vertex's real position: node selection and the morph interval use bounding
    // volumes that include the node's height range.
    let node = nodes[inst];
    let m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let local = mix(pos, coarse, m);
    let p = node.origin_rel.xyz + local;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = normal;
    // The height morphs with the position, so contours, tint and the metric of every height-reading view agree with the
    // geometry and with the coarser neighbour at a border.
    out.height = mix(height, coarse_height, m);
    out.height_fine = height;
    out.morph = m;
    out.local = local;
    out.node_index = inst;
    out.aux = climate;
    return out;
}

// The climate at a fragment of a vertex-format tile: the value interpolated from the vertices (the compact format reads its texture).
fn tile_climate(in: VsOut) -> vec2<f32> {
    return in.aux;
}

// Distance in pixels to the nearest integer of `x`, so lines stay about one pixel wide at any scale.
fn line(x: f32, w: f32) -> f32 {
    let d = abs(fract(x + 0.5) - 0.5);
    return 1.0 - smoothstep(0.0, max(w, 1e-6) * 1.5, d);
}

// Lines at every multiple of `base` * 10^k (k < count) of x (metres). A decade fades out continuously once its lines would
// be closer than ~6 px, like the height contours (the width `wx` is taken by the caller in uniform control flow). `res` is
// the resolution of x itself (its f32 spacing): where x changes by less than that between pixels (`wx` under 4 `res`) it is a
// staircase of plateaus and every plateau at an integer crossing would draw as a wide band, so the lines fade out.
fn decades(x: f32, wx: f32, base: f32, count: i32, res: f32) -> f32 {
    var acc = 0.0;
    var s = base;
    for (var k = 0; k < count; k = k + 1) {
        let w = wx / s;
        let fade = clamp((log2(1.0 / max(w, 1e-6)) - 2.5) / 2.5, 0.0, 1.0) * smoothstep(res, 4.0 * res, wx);
        acc = max(acc, line(x / s, w) * fade);
        s = s * 10.0;
    }
    return acc;
}

// The same for the world-anchored grid along one axis: a line coordinate per decade is the tile-local offset in cells plus
// the node's f64-derived fractional phase of that decade, so it never carries a large magnitude. An f32 fraction is good
// to about 6e-8 cells, so a decade whose pixel is narrower than ~8e-7 cells (its line could be a fraction of a pixel off)
// fades out, fully off below 4e-7: distant 10 km .. 1,000 km lines vanish near the ground, and a decade at full strength
// keeps its place to under 0.15 px (up to ~0.3 px while it fades in).
fn grid_axis(lx: f32, wx: f32, axis: i32, ni: u32) -> f32 {
    var acc = 0.0;
    var s = 0.01;
    for (var k = 0; k < 9; k = k + 1) {
        let w = wx / s;
        let fade = clamp((log2(1.0 / max(w, 1e-6)) - 2.5) / 2.5, 0.0, 1.0) * smoothstep(4.0e-7, 8.0e-7, w);
        acc = max(acc, line(lx / s + nodes[ni].phase[k][axis], w) * fade);
        s = s * 10.0;
    }
    return acc;
}

// Relief: fixed-sun (wrapped) hillshade of the vertex normals on a height tint, a metric grid of the tile-local axes (each axis
// weighted by how square it cuts the surface) from 1 cm to 1,000 km, and height contours from 1 m to 10 km. Unlike the
// flat-colour views it keeps structure at any altitude, so a flight down to 1 m stays readable.
fn relief(in: VsOut) -> vec4<f32> {
    let n = normalize(in.normal);
    let t = clamp((in.height - g.range.x) / max(g.range.y - g.range.x, 1e-6), 0.0, 1.0);
    let low = mix(vec3<f32>(0.18, 0.36, 0.56), vec3<f32>(0.36, 0.56, 0.26), smoothstep(0.0, 0.35, t));
    let tint = mix(low, mix(vec3<f32>(0.66, 0.56, 0.38), vec3<f32>(0.95, 0.95, 0.95), smoothstep(0.7, 1.0, t)), smoothstep(0.35, 0.7, t));
    let sun = normalize(vec3<f32>(0.5, 0.6, 0.62));
    // Wrapped lighting: the night side keeps a gentle slope gradient instead of going flat.
    let wrap = 0.5 + 0.5 * dot(n, sun);
    let shade = 0.3 + 0.7 * wrap * wrap;
    let lx = in.local;
    let wgt = clamp(vec3<f32>(1.0) - abs(n) * 1.25, vec3<f32>(0.0), vec3<f32>(1.0));
    let grid = max(
        max(grid_axis(lx.x, fwidth(lx.x), 0, in.node_index) * wgt.x, grid_axis(lx.y, fwidth(lx.y), 1, in.node_index) * wgt.y),
        grid_axis(lx.z, fwidth(lx.z), 2, in.node_index) * wgt.z,
    );
    // Contours start at 1 m, and fade out wherever the f32 height (spacing about 2.4e-7 of its size) changes by less than
    // a few steps between pixels, which would otherwise draw plateaus of one height as wide bands.
    let contour = decades(in.height, fwidth(in.height), 1.0, 5, max(abs(in.height), 1.0) * 2.4e-7);
    var c = tint * shade;
    c = mix(c, vec3<f32>(0.36, 0.1, 0.05), 0.7 * contour);
    c = mix(c, vec3<f32>(0.04, 0.06, 0.14), 0.8 * grid);
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let mode = g.mode.x;
    // Derivatives are taken before any branch (uniform control flow).
    // Contours every 10 m, 100 m, 1 km and 10 km. Each decade fades out continuously once its lines would be closer
    // than ~6 px, over a window of 2.5 octaves of spacing (no discrete switch, and a slope change of tens of percent between triangles moves a fade only a little).
    let h10 = in.height / 10.0;
    let w10 = fwidth(h10);
    var contour = 0.0;
    var scale = 1.0;
    for (var d = 0; d < 4; d = d + 1) {
        let w = w10 / scale;
        let fade = clamp((log2(1.0 / max(w, 1e-6)) - 2.5) / 2.5, 0.0, 1.0);
        contour = max(contour, line(h10 / scale, w) * fade);
        scale = scale * 10.0;
    }
    let band_z = log2(max(in.pos.z, 1e-30));
    let w_z = fwidth(band_z);
    if (mode <= 2u) {
        return vec4<f32>(nodes[in.node_index].color.rgb, 1.0);
    }
    if (mode == 3u) {
        // Blue = full detail, red = fully morphed onto the coarser grid; red stays the morph value.
        return vec4<f32>(in.morph, 0.0, 1.0 - in.morph, 1.0);
    }
    if (mode == 4u) {
        let t = clamp((in.height - g.range.x) / (g.range.y - g.range.x), 0.0, 1.0);
        let v = floor(t * 255.0 + 0.5) / 255.0;
        let k = 1.0 - 0.8 * contour;
        return vec4<f32>(v * k, v * k, v * k, 1.0);
    }
    if (mode == 5u) {
        let n = normalize(in.normal) * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
        return vec4<f32>(n, 1.0);
    }
    if (mode == 7u) {
        return relief(in);
    }
    if (mode == 8u) {
        // The morphed minus the vertex's own height, over the plan's height range as the delta window.
        let t = clamp((in.height - in.height_fine - g.range.x) / max(g.range.y - g.range.x, 1e-6), 0.0, 1.0);
        let v = floor(t * 255.0 + 0.5) / 255.0;
        return vec4<f32>(v, v, v, 1.0);
    }
    let v = clamp(band_z / 40.0 + 1.0, 0.0, 1.0);
    let k = 1.0 - 0.8 * line(band_z, w_z);
    return vec4<f32>(v * k, v * k, v * k, 1.0);
}

// The albedo of the surface (ADR 0039 point 3, REND-026): ground from the climate (desert, grass, forest, tundra), bare rock on
// steep ground, snow by temperature and slope, a beach band above the water and the seabed below it. `t` is the temperature in
// degrees C, `p` the precipitation in mm, `cs` the cosine of the slope and `h` the height above the water level. The same function
// as `AlbedoParams::albedo` (the f64 reference in `planet_frame::look::shading`).
fn albedo_of(tab: array<vec4<f32>, 10>, t: f32, p: f32, cs: f32, h: f32) -> vec3<f32> {
    let wet = smoothstep(tab[0].w, tab[1].w, p);
    let forest = smoothstep(tab[2].w, tab[3].w, p);
    let vegetation = mix(tab[2].xyz, tab[3].xyz, forest);
    var ground = mix(tab[4].xyz, vegetation, wet);
    let warm = smoothstep(tab[4].w, tab[5].w, t);
    ground = mix(tab[5].xyz, ground, warm);
    let rock_w = 1.0 - smoothstep(tab[6].w, tab[7].w, cs);
    let base = mix(ground, tab[0].xyz, rock_w);
    let cold = 1.0 - smoothstep(tab[8].x, tab[8].y, t);
    let sticks = smoothstep(tab[8].z, tab[8].w, cs);
    let snow_w = cold * sticks;
    let land = mix(base, tab[1].xyz, snow_w);
    let beach_w = (1.0 - smoothstep(tab[9].x, tab[9].x + tab[9].y, h)) * (1.0 - snow_w);
    let shore = mix(land, tab[6].xyz, beach_w);
    let land_w = smoothstep(-tab[9].z, 0.0, h);
    return mix(tab[7].xyz, shore, land_w);
}

// The false colour of a climate channel (the debug views): temperature blue to red over -30..40 C, precipitation sand to blue over
// 0..3000 mm.
fn climate_colour(view: u32, t: f32, p: f32) -> vec3<f32> {
    if (view == 3u) {
        let x = clamp((t + 30.0) / 70.0, 0.0, 1.0);
        return mix(mix(vec3<f32>(0.05, 0.15, 0.85), vec3<f32>(0.95, 0.95, 0.85), smoothstep(0.0, 0.5, x)), vec3<f32>(0.90, 0.10, 0.05), smoothstep(0.5, 1.0, x));
    }
    let y = clamp(p / 3000.0, 0.0, 1.0);
    return mix(vec3<f32>(0.95, 0.92, 0.80), vec3<f32>(0.05, 0.15, 0.55), y);
}

// The lit terrain's radiance before the pre-exposure (ADR 0039 point 4, REND-026): Lambert diffuse `albedo * max(n . sun, 0) / pi`
// for the sun's illuminance of one unit, in the view `view` (0 lit, 1 albedo, 2 lighting, 3 temperature, 4 precipitation). `up` is
// the unit up vector at the point; the climate comes from the tile's field. No ambient, no shadows, no specular.
fn terrain_radiance(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, sun: vec3<f32>) -> vec3<f32> {
    let n = normalize(normal);
    let cs = clamp(dot(n, up), 0.0, 1.0);
    let ndl = max(dot(n, sun), 0.0);
    let inv_pi = 0.31830988618;
    if (view >= 3u) {
        return climate_colour(view, t, p) * inv_pi;
    }
    if (view == 2u) {
        return vec3<f32>(ndl * inv_pi);
    }
    let a = albedo_of(tab, t, p, cs, h);
    if (view == 1u) {
        return a * inv_pi;
    }
    return a * (ndl * inv_pi);
}

// Up at a fragment: the unit up vector of the tile origin plus the fragment's tile-local position times the inverse of the origin's
// distance from the planet centre, normalised (`node_up` is the node record's `up`). Tile-local arithmetic only (CON-11).
fn up_at(node_up: vec4<f32>, local: vec3<f32>) -> vec3<f32> {
    return normalize(node_up.xyz + local * node_up.w);
}

// What the lit path writes for a fragment (ADR 0038 point 1, REND-025/026): the radiance times the pre-exposure factor of the plan,
// clamped to 60,000 so the scene target does not hold inf in f16.
fn shade_lit(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, sun: vec3<f32>, factor: f32) -> vec3<f32> {
    return clamp(terrain_radiance(view, tab, normal, up, t, p, h, sun) * factor, vec3<f32>(0.0), vec3<f32>(60000.0));
}

// The sun direction and the pre-exposure factor travel in the otherwise unused words of the globals (mode.yzw as bits, range.z), the
// view in mode.x and the water level in extra.x. The up vector is formed from the node's tile-local numbers and the fragment's
// tile-local position: no planet-radius value reaches this stage.
@fragment
fn fs_lit(in: VsOut) -> @location(0) vec4<f32> {
    let sun = vec3<f32>(bitcast<f32>(g.mode.y), bitcast<f32>(g.mode.z), bitcast<f32>(g.mode.w));
    let up = up_at(nodes[in.node_index].up, in.local);
    let climate = tile_climate(in);
    return vec4<f32>(shade_lit(g.mode.x, g.albedo, in.normal, up, climate.x, climate.y, in.height - g.extra.x, sun, g.range.z), 1.0);
}
"#;

pub const TERRAIN_SHADERS: &[(&str, &str)] = &[("terrain", SHADER)];

/// Depth target format and test: reversed-Z `Depth32Float` with the clear value 0 (CON-12, ADR 0002), for the debug and lit paths alike.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub(crate) const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::Greater;

fn view_index(v: TerrainView) -> u32 {
    TerrainView::ALL.iter().position(|x| *x == v).expect("known view") as u32
}

/// Counts that are deterministic on every adapter (CON-20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainStats {
    /// Draws of the frame: one per planned node on either path.
    pub draw_calls: u32,
    pub triangles: u64,
    pub vertex_bytes: u64,
    /// Draw commands encoded: one per node on the per-draw path, one per run of consecutive nodes in one buffer on the indirect
    /// path (REND-015).
    pub encoded_draws: u32,
    /// Bytes of node records written this frame (both paths).
    pub node_record_bytes: u64,
    /// Bytes of draw argument records written this frame (indirect path only).
    pub argument_bytes: u64,
    /// Bytes of tile references written this frame (compact renderer only: 16 per node).
    pub tile_ref_bytes: u64,
}

/// How the renderer submits the planned draws (REND-015). Both draw the same plan in the same order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawMode {
    /// One `draw_indexed` per node with its own vertex-buffer binding: the reference path and the fallback.
    PerDraw,
    /// Argument records built on the CPU from the plan, one `multi_draw_indexed_indirect` per run of nodes in one buffer.
    Indirect,
}

impl DrawMode {
    pub fn name(self) -> &'static str {
        match self {
            DrawMode::PerDraw => "per-draw",
            DrawMode::Indirect => "indirect",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "per-draw" => Ok(DrawMode::PerDraw),
            "indirect" => Ok(DrawMode::Indirect),
            other => Err(format!("unknown draw mode '{other}' (per-draw or indirect)")),
        }
    }
}

/// Per-frame buffers in a ring of `MAX_FRAMES_IN_FLIGHT` sets (the pipelining rule of STRM-016). `queue.write_buffer` is already
/// ordered on the queue timeline, so the ring is a deliberate choice that makes the rule explicit, not a requirement.
const RING: usize = 2;

/// Bytes of the globals uniform (view-projection, height range, view mode, the shading extras and the albedo table).
const GLOBALS_BYTES: u64 = 96 + 16 + 16 * planet_frame::look::shading::ALBEDO_VEC4S as u64;

struct FrameSet {
    globals: wgpu::Buffer,
    nodes: wgpu::Buffer,
    args: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Compact format only: one tile reference per node, and the group-1 bind group (slot records plus these).
    refs: Option<wgpu::Buffer>,
    frame_bind: Option<wgpu::BindGroup>,
}

/// Where a resident tile's data sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// Vertex format in a slot of the main pool (the vertex-format renderer) or of the small vertex pool (levels 0-2).
    Vertex,
    /// Compact format in the atlas slot `Resident::slot`.
    Atlas,
    /// A tile of the compact renderer that is in the vertex format although it is not level 0-2 (CON-13 fallback): in the
    /// fallback region of the vertex pool at this absolute slot index. `Resident::slot` is the manager's (empty) main slot, or the
    /// vertex-pool slot itself when the renderer placed the tile.
    Fallback(u32),
}

#[derive(Clone, Copy, Debug)]
struct Resident {
    key: u64,
    slot: SlotRef,
    place: Place,
}

/// The format a resident tile is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidentFormat {
    /// Vertex format by level (0-2) or because the renderer has no compact pool.
    Vertex,
    /// Compact texels in an atlas slot.
    Compact,
    /// A tile of level 3 or finer in the vertex format (CON-13 fallback).
    Fallback,
}

/// A tile to place, in either format.
#[derive(Clone, Copy)]
enum TileRef<'a> {
    Vertex(&'a TileMesh),
    Compact(&'a CompactTile),
}

impl<'a> TileRef<'a> {
    fn of(tile: &'a RenderTile) -> Self {
        match tile {
            RenderTile::Vertex(m) => TileRef::Vertex(m),
            RenderTile::Compact(c) => TileRef::Compact(c),
        }
    }

    fn parts(&self) -> (planet_core::TileId, u64, u32) {
        match self {
            TileRef::Vertex(m) => (m.id, m.key, m.cells),
            TileRef::Compact(c) => (c.id, c.key, c.cells),
        }
    }
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A vertex tile's bytes as the pool holds them: the floats of each vertex, little-endian. (Written with `extend_from_slice`: the
/// byte-wise `flat_map` it replaces cost about 8x as much CPU per tile, REND-016; the bytes are the same.)
fn vertex_bytes_into(out: &mut Vec<u8>, mesh: &TileMesh) {
    out.reserve(mesh.vertices.len() * MeshVertex::GPU_STRIDE as usize);
    for (vi, v) in mesh.vertices.iter().enumerate() {
        for x in v.to_floats() {
            out.extend_from_slice(&x.to_le_bytes());
        }
        for c in mesh.gpu_climate(vi) {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

/// Keep the last of each run of entries with equal `key` in the sorted `v` (the greatest write index of a place, which sorts
/// last) and return how many are kept; `v` is truncated to them.
fn dedup_places<T: Copy, K: PartialEq>(v: &mut Vec<T>, key: impl Fn(&T) -> K) -> usize {
    let mut kept = 0;
    for k in 0..v.len() {
        if k + 1 < v.len() && key(&v[k + 1]) == key(&v[k]) {
            continue;
        }
        v[kept] = v[k];
        kept += 1;
    }
    v.truncate(kept);
    kept
}

/// A compact tile's texels as the atlas holds them (`CompactTile::texel_data`'s bytes, converted without the byte-wise iterator).
fn texel_bytes(texels: &[[u16; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(texels.len() * 8);
    texel_bytes_into(&mut out, texels);
    out
}

/// [`texel_bytes`] appended to `out`.
fn texel_bytes_into(out: &mut Vec<u8>, texels: &[[u16; 4]]) {
    out.reserve(texels.len() * 8);
    for t in texels {
        for c in t {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
}

/// How a frame's tile writes reach the queue (REND-016, STRM-006): `PerSlot` is the reference (one `write_buffer` or
/// `write_texture` call per compact slot, plus one for its record), `Batched` collects the frame's compact tiles and writes each
/// run of adjacent slots with one call. Both leave identical pool contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UploadMode {
    PerSlot,
    #[default]
    Batched,
}

impl UploadMode {
    pub fn name(self) -> &'static str {
        match self {
            UploadMode::PerSlot => "per-slot",
            UploadMode::Batched => "batched",
        }
    }

    pub fn parse(s: &str) -> Option<UploadMode> {
        match s {
            "per-slot" => Some(UploadMode::PerSlot),
            "batched" => Some(UploadMode::Batched),
            _ => None,
        }
    }
}

/// What the tile writes cost so far (cumulative; REND-016): queue calls (`write_buffer` and `write_texture`; each is one staging
/// allocation and one copy command) and the bytes handed to them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadCounters {
    pub write_calls: u64,
    pub staging_bytes: u64,
}

/// The frame thread's time in the tile writes so far (cumulative, recorded only: no decision reads it): `total` in
/// [`TerrainRenderer::upload_tiles_at`], of which `queue` in the queue calls (`write_buffer`, `write_texture`) and the rest staging
/// (conversion, sorting, packing). Where a frame's upload time goes, once the bytes are known not to be it (REND-018).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UploadTimes {
    pub total: std::time::Duration,
    pub queue: std::time::Duration,
}

/// A buffer the tile data lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BufTarget {
    VertexPool,
    Block(usize),
}

/// The buffers a frame's tile writes are staged in (REND-018): kept by the renderer and reused, so that after the first frames
/// of a size a batch allocates nothing on the frame thread (a heap shared with the generation threads is where a frame's time
/// would vary with their load). Every vector is cleared, never freed, between batches; [`TerrainRenderer::batch_scratch_capacity`]
/// is what tests watch.
#[derive(Default)]
struct BatchScratch {
    /// Whether a batch is being collected (`upload_tiles_at`).
    open: bool,
    /// The converted texels of the batch's compact tiles, one after the other, `tile_bytes` each.
    texels: Vec<u8>,
    /// Per compact tile in collection order: its atlas block, its slot within the block and the offset of its texels in `texels`.
    tiles: Vec<(usize, u32, usize)>,
    /// Per compact tile in collection order: its slot index and its record.
    records: Vec<(u32, [u8; COMPACT_RECORD_BYTES as usize])>,
    /// The texel writes sorted by place (the index of the write in `tiles` last, so that the last write of a place is the last
    /// of its equals), and the record writes likewise.
    texel_order: Vec<(usize, u32, u32)>,
    record_order: Vec<(u32, u32)>,
    /// The rectangle of one run of texels, and the bytes of one run of records.
    run: Vec<u8>,
    run_records: Vec<u8>,
    /// The bytes of a vertex-format tile (converted, then written at once).
    vertex: Vec<u8>,
}

impl BatchScratch {
    fn capacity(&self) -> usize {
        self.texels.capacity()
            + self.tiles.capacity()
            + self.records.capacity()
            + self.texel_order.capacity()
            + self.record_order.capacity()
            + self.run.capacity()
            + self.run_records.capacity()
            + self.vertex.capacity()
    }
}

/// Per-frame timings of [`TerrainRenderer::render`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTiming {
    /// CPU time to update uniforms, record and submit, milliseconds.
    pub cpu_ms: f64,
    /// Wall time from the start of the call until the GPU finished the frame, milliseconds.
    pub total_ms: f64,
    /// GPU time of the render pass from timestamp queries, when the device supports them.
    pub gpu_ms: Option<f64>,
}

/// Terrain renderer with persistent resources: pipeline, uniform buffers, per-tile vertex buffers and render targets are
/// created once, so a frame only writes two small buffers and records the draws. `execute_terrain` is the one-shot wrapper.
pub struct TerrainRenderer {
    size: (u32, u32),
    cells: u32,
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    /// The per-frame buffers, used round robin: `ring[frame % RING]`.
    ring: Vec<FrameSet>,
    node_capacity: usize,
    draw_mode: DrawMode,
    indices: wgpu::Buffer,
    index_count: u32,
    /// Terrain tiles live in pooled fixed-size slots (STRM-006, ADR 0033 point 3): a few large vertex buffers, one slot per
    /// tile. `pool` is the configuration whose `allocated_bytes` bound what this renderer allocates.
    pool: PoolConfig,
    /// Placement of `upload`: slots handed out lowest first, reused after the frames in flight retire.
    pools: SlotPools,
    /// Blocks of the main pool, allocated on first use.
    blocks: Vec<Option<wgpu::Buffer>>,
    /// The small vertex pool (levels 0-2), allocated on first use.
    vertex_pool: Option<wgpu::Buffer>,
    /// Resident tiles: mesh key, slot and where the data sits.
    tiles: HashMap<planet_core::TileId, Resident>,
    /// The compact format's pipeline, atlases and slot records (None for the vertex-format renderer).
    compact: Option<CompactGpu>,
    /// The tile each occupied slot holds.
    owners: HashMap<SlotRef, planet_core::TileId>,
    /// Bytes of one slot.
    slot_bytes: u64,
    /// Most nodes the node record buffer may hold.
    max_nodes: usize,
    /// Frames rendered; the clock of the slot retirement.
    frame: u64,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    timing: Option<TimingResources>,
    /// Bytes of vertex data uploaded so far.
    pub uploaded_bytes: u64,
    upload_mode: UploadMode,
    upload_counters: UploadCounters,
    upload_times: UploadTimes,
    /// The staging buffers of the tile writes, reused from frame to frame (REND-018).
    scratch: BatchScratch,
    /// Frames the GPU has finished, counted by the queue's completion callbacks (they run when the device is polled).
    retired: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Submissions of the frames not yet seen finished, oldest first.
    in_flight: std::collections::VecDeque<wgpu::SubmissionIndex>,
    /// Long-lived GPU objects created so far (REND-013).
    init: InitCounts,
    /// Which parts of the renderer a submission has used so far (REND-017).
    used: UseMasks,
    /// The lit path (ADR 0042): None until [`enable_lit`](TerrainRenderer::enable_lit).
    lit: Option<LitGpu>,
    /// Pipelines and targets made by `enable_lit` (zero before it): fixed once enabled, so no lit frame creates one (REND-017).
    lit_objects: u32,
}

/// The lit path's terrain pipelines (HDR target, `fs_lit`) and its post chain.
struct LitGpu {
    path: LitPath,
    pipeline: wgpu::RenderPipeline,
    compact_pipeline: Option<wgpu::RenderPipeline>,
}

fn make_pool_buffer(device: &wgpu::Device, label: &'static str, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// How often the renderer created its long-lived GPU objects (REND-013, REND-016): after construction and
/// [`warm_up`](TerrainRenderer::warm_up) a script whose first frames fit the warmed blocks and ring leaves every counter where it
/// was, so no frame pays for a one-time initialisation. Demand growth (a second block, a plan beyond the ring) counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InitCounts {
    /// Render pipelines created (1 for the vertex format, 2 with the compact format).
    pub pipelines: u32,
    /// Pool allocations: the vertex pool, blocks of the vertex-format main pool, atlases.
    pub pool_allocations: u32,
    /// Builds of the per-frame ring (the first one counts).
    pub ring_builds: u32,
}

/// Sets in the per-frame ring: the number of distinct sets of buffers and bind groups the first frames use (REND-017).
pub const RING_SETS: u32 = RING as u32;

/// Which parts of the renderer a submission has already drawn with, as masks.
#[derive(Clone, Copy, Debug, Default)]
struct UseMasks {
    ring: u8,
    pipelines: u8,
    timestamps: bool,
    modes: u8,
    waited: u32,
}

/// How many distinct parts of the renderer have been used by a submission so far (REND-017): the lazy work of the driver and of
/// wgpu for each happens at its first use, so a part used for the first time inside a measured frame makes that frame wait on the
/// GPU. [`warm_up`](TerrainRenderer::warm_up) uses every part a scene's first frames use, in one waited submission, and later
/// frames add nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FirstUse {
    /// Sets of the per-frame ring a frame has been recorded with (of [`RING_SETS`]).
    pub ring_sets: u32,
    /// Pipelines that have drawn (1 for the vertex format, 2 with the compact format once both drew).
    pub pipelines: u32,
    /// 1 once a pass with timestamp queries has run (adapters that support them), else 0.
    pub timestamp_passes: u32,
    /// Draw modes used: per-draw and indirect each count once.
    pub draw_modes: u32,
    /// Submissions the renderer waited for to completion by itself before returning (the warm-up's).
    pub waited_submissions: u32,
}

/// Nodes the ring holds after [`warm_up`](TerrainRenderer::warm_up) (capped by the share's node-record bytes): above the node
/// count of the dive's plans, about 1 MiB of buffers.
pub const WARM_NODES: usize = 2048;

struct TimingResources {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}

impl TerrainRenderer {
    /// A renderer with the mid tier's terrain share (ADR 0033: 2 GiB, a ceiling counted in slots). Slots are reusable one
    /// frame after their release (the renderer finishes a frame before it returns).
    pub fn new(ctx: &GpuContext, size: (u32, u32), cells: u32) -> Self {
        let pool = tier_pool(Tier::Mid, cells, 1).unwrap_or_else(|e| panic!("terrain pools for cells {cells}: {e}"));
        Self::with_pools(ctx, size, cells, pool)
    }

    /// [`new`](TerrainRenderer::new) for a loop that keeps up to `frames_in_flight` frames in flight (STRM-016): a released slot is
    /// reusable that many frames later, so no frame that may still read it is running.
    pub fn with_frames_in_flight(ctx: &GpuContext, size: (u32, u32), cells: u32, frames_in_flight: u64) -> Self {
        let pool = tier_pool(Tier::Mid, cells, frames_in_flight).unwrap_or_else(|e| panic!("terrain pools for cells {cells}: {e}"));
        Self::with_pools(ctx, size, cells, pool)
    }

    /// A renderer whose pools follow `pool` (a tier's share, or a small one for tests). The slot size must be the
    /// vertex-format tile of `cells`.
    pub fn with_pools(ctx: &GpuContext, size: (u32, u32), cells: u32, pool: PoolConfig) -> Self {
        assert_eq!(
            pool.slot_bytes,
            planet_frame::pool::vertex_slot_bytes(cells),
            "the pool slot is not the vertex-format tile of {cells} cells"
        );
        Self::build(ctx, size, cells, pool, false)
    }

    /// A renderer whose main pool is the compact format (REND-016, ADR 0033): atlases of `Rgba16Uint` texels with a record per slot,
    /// the small vertex pool for levels 0-2 and for the CON-13 fallback tiles. `pool` comes from `planet_frame::pool::compact_format_pool`
    /// (or a small one for tests): its slot is the texels of one tile and its block is one atlas texture.
    pub fn with_compact_pools(ctx: &GpuContext, size: (u32, u32), cells: u32, pool: PoolConfig) -> Self {
        assert_eq!(
            pool.slot_bytes,
            planet_generators::compact::slot_texel_bytes(cells),
            "the pool slot is not the compact tile of {cells} cells"
        );
        assert_eq!(
            pool.vertex_slot_bytes,
            planet_frame::pool::vertex_slot_bytes(cells),
            "the vertex pool slot is not the vertex-format tile"
        );
        assert_eq!(pool.slot_record_bytes, COMPACT_RECORD_BYTES, "the pool slot record is not the compact record");
        assert!(
            pool.fallback_slots > 0 && pool.fallback_slots < pool.vertex_slots,
            "the compact renderer needs a fallback region in its vertex pool"
        );
        Self::build(ctx, size, cells, pool, true)
    }

    /// A renderer in the context's tile format (`GpuContext::default_tile_format`) with the mid tier's terrain share, for a loop
    /// that keeps up to `frames_in_flight` frames in flight: what the product paths use.
    pub fn for_context(ctx: &GpuContext, size: (u32, u32), cells: u32, frames_in_flight: u64) -> Self {
        match ctx.default_tile_format() {
            crate::TileFormat::Compact => Self::with_frames_in_flight_compact(ctx, size, cells, frames_in_flight),
            crate::TileFormat::Vertex => Self::with_frames_in_flight(ctx, size, cells, frames_in_flight),
        }
    }

    /// A compact-format renderer with the mid tier terrain share (15 atlases, 230,640 slots at cells 32).
    pub fn new_compact(ctx: &GpuContext, size: (u32, u32), cells: u32) -> Self {
        Self::with_frames_in_flight_compact(ctx, size, cells, 1)
    }

    /// [`new_compact`](TerrainRenderer::new_compact) for a loop that keeps up to `frames_in_flight` frames in flight.
    pub fn with_frames_in_flight_compact(ctx: &GpuContext, size: (u32, u32), cells: u32, frames_in_flight: u64) -> Self {
        let pool = planet_frame::pool::tier_compact_pool(Tier::Mid, cells, frames_in_flight)
            .unwrap_or_else(|e| panic!("compact terrain pools for cells {cells}: {e}"));
        Self::with_compact_pools(ctx, size, cells, pool)
    }

    fn build(ctx: &GpuContext, size: (u32, u32), cells: u32, pool: PoolConfig, compact: bool) -> Self {
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
        let color = target(
            "terrain-color",
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = target("terrain-depth", DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));
        // The share's node-record bytes bound the ring: per node a record and an argument record in each of the RING sets.
        let per_node = NODE_BYTES + DRAW_RECORD_BYTES + if compact { REF_BYTES } else { 0 };
        let max_nodes = (pool.node_record_bytes.saturating_sub(RING as u64 * GLOBALS_BYTES) / (RING as u64 * per_node)) as usize;
        let node_capacity = 256.min(max_nodes.max(1));
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("terrain"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain-bgl"),
            entries: &[entry(0, wgpu::BufferBindingType::Uniform), entry(1, wgpu::BufferBindingType::Storage { read_only: true })],
        });
        let compact_gpu = compact.then(|| Self::make_compact(ctx, &bgl, format, cells, &pool));
        let ring = Self::make_ring(device, &bgl, node_capacity, compact_gpu.as_ref());
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attr =
            |format, k: usize, loc| wgpu::VertexAttribute { format, offset: 4 * MeshVertex::OFFSETS[k] as u64, shader_location: loc };
        let attrs = [
            attr(wgpu::VertexFormat::Float32x3, 0, 0),
            attr(wgpu::VertexFormat::Float32x3, 1, 1),
            attr(wgpu::VertexFormat::Float32x3, 2, 2),
            attr(wgpu::VertexFormat::Float32, 3, 3),
            attr(wgpu::VertexFormat::Float32, 4, 4),
            // The climate of the vertex after the 11 floats: two f16 (temperature, precipitation), see `MeshVertex::GPU_STRIDE`.
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float16x2, offset: MeshVertex::STRIDE, shader_location: 5 },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: MeshVertex::GPU_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            // Reversed-Z (ADR 0002): near = 1, far = 0, so the test is Greater and the clear value 0.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let index_data = grid_indices(cells);
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain-indices"),
            contents: &index_data.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let timing = ctx.timestamps_supported().then(|| TimingResources {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("terrain-timestamps"),
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
            ring,
            node_capacity,
            draw_mode: ctx.default_draw_mode(),
            indices,
            index_count: index_data.len() as u32,
            pools: pool.slot_pools(),
            blocks: Vec::new(),
            vertex_pool: None,
            tiles: HashMap::new(),
            compact: compact_gpu,
            owners: HashMap::new(),
            slot_bytes: pool.slot_bytes,
            max_nodes,
            frame: 0,
            pool,
            color,
            color_view,
            depth_view,
            timing,
            uploaded_bytes: 0,
            upload_mode: ctx.default_upload_mode(),
            upload_counters: UploadCounters::default(),
            upload_times: UploadTimes::default(),
            scratch: BatchScratch::default(),
            retired: Default::default(),
            in_flight: Default::default(),
            init: InitCounts { pipelines: if compact { 2 } else { 1 }, pool_allocations: 0, ring_builds: 1 },
            used: UseMasks::default(),
            lit: None,
            lit_objects: 0,
        }
    }

    /// The compact format pipeline, record buffer and (empty) atlas table.
    fn make_compact(
        ctx: &GpuContext,
        bgl0: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        cells: u32,
        pool: &PoolConfig,
    ) -> CompactGpu {
        let device = &ctx.device;
        let side = ((pool.block_alloc_bytes / planet_generators::compact::TEXEL_BYTES) as f64).sqrt().round() as u32;
        assert_eq!(
            u64::from(side) * u64::from(side) * planet_generators::compact::TEXEL_BYTES,
            pool.block_alloc_bytes,
            "a block is a square atlas"
        );
        let cols = side / (cells + 1);
        assert_eq!(cols * cols, pool.block_slots, "block_slots must fill an atlas of {side} texels with tiles of {} texels", cells + 1);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-compact"),
            source: wgpu::ShaderSource::Wgsl(compact_terrain_shader(cells).into()),
        });
        // The tile references (binding 1) and the atlas are read by the fragment stage too (the climate of the tile, ADR 0039).
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: if binding == 1 { wgpu::ShaderStages::VERTEX_FRAGMENT } else { wgpu::ShaderStages::VERTEX },
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl_frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compact-frame-bgl"),
            entries: &[storage(0), storage(1)],
        });
        let bgl_atlas = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compact-atlas-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compact-layout"),
            bind_group_layouts: &[Some(bgl0), Some(&bgl_frame), Some(&bgl_atlas)],
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
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let records = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compact-slot-records"),
            size: pool.slot_record_total_bytes().max(COMPACT_RECORD_BYTES),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        CompactGpu {
            pipeline,
            bgl_frame,
            bgl_atlas,
            records,
            atlases: Vec::new(),
            cols,
            side,
            fallback: SlotPool::new(pool.fallback_slots, pool.frames_in_flight),
            fallback_base: pool.managed_vertex_slots(),
        }
    }

    /// The ring of per-frame buffers for plans of up to `capacity` nodes.
    fn make_ring(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, capacity: usize, compact: Option<&CompactGpu>) -> Vec<FrameSet> {
        (0..RING)
            .map(|_| {
                let buffer = |label, size, usage| {
                    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
                };
                let globals = buffer("globals", GLOBALS_BYTES, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
                let nodes = buffer("nodes", NODE_BYTES * capacity as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
                let args =
                    buffer("draw-args", DRAW_RECORD_BYTES * capacity as u64, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST);
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("terrain-bind"),
                    layout: bgl,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: nodes.as_entire_binding() },
                    ],
                });
                let refs = compact
                    .map(|_| buffer("tile-refs", REF_BYTES * capacity as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST));
                let frame_bind = compact.zip(refs.as_ref()).map(|(c, refs)| {
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("compact-frame-bind"),
                        layout: &c.bgl_frame,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: c.records.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: refs.as_entire_binding() },
                        ],
                    })
                });
                FrameSet { globals, nodes, args, bind, refs, frame_bind }
            })
            .collect()
    }

    /// Enable the lit path (ADR 0042 point 1): create the `Rgba16Float` scene target, the exposure buffers and compute pipelines,
    /// the tone-map pipeline and the terrain pipelines that write the scene target. Call it before the frames that are measured
    /// (REND-017); a plan with a [`LitPlan`](planet_frame::look::LitPlan) then draws through it, and the debug path is untouched.
    pub fn enable_lit(&mut self, ctx: &GpuContext) {
        if self.lit.is_some() {
            return;
        }
        let device = &ctx.device;
        let hdr = crate::lit::HDR_FORMAT;
        let depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(DEPTH_COMPARE),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let targets = [Some(wgpu::ColorTargetState { format: hdr, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-lit"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain-lit-layout"),
            bind_group_layouts: &[Some(&self.bgl)],
            immediate_size: 0,
        });
        let attr =
            |format, k: usize, loc| wgpu::VertexAttribute { format, offset: 4 * MeshVertex::OFFSETS[k] as u64, shader_location: loc };
        let attrs = [
            attr(wgpu::VertexFormat::Float32x3, 0, 0),
            attr(wgpu::VertexFormat::Float32x3, 1, 1),
            attr(wgpu::VertexFormat::Float32x3, 2, 2),
            attr(wgpu::VertexFormat::Float32, 3, 3),
            attr(wgpu::VertexFormat::Float32, 4, 4),
            // The climate of the vertex after the 11 floats: two f16 (temperature, precipitation), see `MeshVertex::GPU_STRIDE`.
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float16x2, offset: MeshVertex::STRIDE, shader_location: 5 },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain-lit-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: MeshVertex::GPU_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_lit"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let compact_pipeline = self.compact.as_ref().map(|c| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("terrain-compact-lit"),
                source: wgpu::ShaderSource::Wgsl(compact_terrain_shader(self.cells).into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compact-lit-layout"),
                bind_group_layouts: &[Some(&self.bgl), Some(&c.bgl_frame), Some(&c.bgl_atlas)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("compact-lit-pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_lit"),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        let path = LitPath::new(ctx, self.size);
        // The scene target, the exposure buffers, three post pipelines and the terrain pipelines.
        self.lit_objects = 1 + 3 + 1 + u32::from(compact_pipeline.is_some());
        self.lit = Some(LitGpu { path, pipeline, compact_pipeline });
    }

    /// Long-lived GPU objects the lit path made (scene target, exposure buffers, the post pipelines and the terrain pipelines).
    pub fn lit_objects(&self) -> u32 {
        self.lit_objects
    }

    /// Whether the lit path is enabled.
    pub fn lit_enabled(&self) -> bool {
        self.lit.is_some()
    }

    /// The lit path's post chain (scene target, exposure state), for tests and tools.
    pub fn lit_path(&self) -> Option<&LitPath> {
        self.lit.as_ref().map(|l| &l.path)
    }

    /// Bytes of all frame targets the renderer holds for the lit path: the scene target, the depth target, the histogram and the
    /// exposure history. The frame-target share counts exactly these (REND-020).
    pub fn lit_frame_target_bytes(&self) -> Option<u64> {
        self.lit
            .as_ref()
            .map(|l| l.path.allocated_bytes() + u64::from(self.size.0) * u64::from(self.size.1) * crate::graph::DEPTH_BYTES_PER_PIXEL)
    }

    /// Long-lived GPU objects created so far (REND-013): the counters a test compares before and after a frame.
    pub fn init_counts(&self) -> InitCounts {
        self.init
    }

    /// Do the one-time work of the first frames now, outside any measured frame (REND-013, TEST-018): allocate the vertex pool and
    /// the first block of the main pool (block 0, or atlas 0 with its bind group), grow the per-frame ring to [`WARM_NODES`]
    /// (within the share's node-record bytes), and submit one throwaway frame (every ring set, the timed pass where supported, a
    /// two-record multi-draw) that draws a zeroed tile through every pipeline
    /// the format has in the active draw mode (call `set_draw_mode` first), polled to completion, so the driver's lazy work (first-use initialisation of the
    /// buffers and the atlas, pipeline state, shader JIT) is done before frame 0. No tile becomes resident and no counter of the
    /// frame clock moves. The bytes allocated are the pure model's for one block plus the vertex pool (STRM-006); the rest of the
    /// share stays on demand.
    pub fn warm_up(&mut self, ctx: &GpuContext) {
        let device = &ctx.device;
        let warm_nodes = WARM_NODES.min(self.max_nodes);
        if warm_nodes > self.node_capacity {
            self.node_capacity = warm_nodes;
            self.ring = Self::make_ring(device, &self.bgl, self.node_capacity, self.compact.as_ref());
            self.init.ring_builds += 1;
        }
        self.vertex_pool_buffer(device);
        if let Some(c) = self.compact.as_mut() {
            c.atlas(device, 0, &mut self.init);
        } else {
            self.block_buffer(device, 0);
        }
        let indirect = self.draw_mode == DrawMode::Indirect;
        // Two records in the indirect path so the multi-draw takes its count above one; one instance each, the rest zero.
        let records = if indirect { 2u32 } else { 1 };
        let mut record = [0u8; 2 * DRAW_RECORD_BYTES as usize];
        for k in 0..records as usize {
            let at = k * DRAW_RECORD_BYTES as usize;
            record[at..at + 4].copy_from_slice(&self.index_count.to_le_bytes());
            record[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
        }
        // Every set of the ring carries its own buffers and bind groups; each is written, bound and drawn from once.
        for set in &self.ring {
            ctx.queue.write_buffer(&set.globals, 0, &[0u8; GLOBALS_BYTES as usize]);
            ctx.queue.write_buffer(&set.nodes, 0, &[0u8; 2 * NODE_BYTES as usize]);
            ctx.queue.write_buffer(&set.args, 0, &record[..records as usize * DRAW_RECORD_BYTES as usize]);
            if let Some(refs) = &set.refs {
                ctx.queue.write_buffer(refs, 0, &[0u8; 2 * REF_BYTES as usize]);
            }
        }
        let timing = self.timing.as_ref();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain-warm-up") });
        // Mark the pools initialised on the GPU (wgpu zeroes a buffer lazily before its first use).
        if let Some(pool) = &self.vertex_pool {
            encoder.clear_buffer(pool, 0, None);
        }
        if let Some(Some(block)) = self.blocks.first() {
            encoder.clear_buffer(block, 0, None);
        }
        for (k, set) in self.ring.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-warm-up-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                // The timed pass of `render(.., true)` runs once here too.
                timestamp_writes: timing.filter(|_| k == 0).map(|t| wgpu::RenderPassTimestampWrites {
                    query_set: &t.queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let draw = |pass: &mut wgpu::RenderPass| {
                if indirect {
                    pass.multi_draw_indexed_indirect(&set.args, 0, records);
                } else {
                    pass.draw_indexed(0..self.index_count, 0, 0..1);
                }
            };
            pass.set_pipeline(&self.pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(0, &set.bind, &[]);
            let pool = self.vertex_pool.as_ref().expect("vertex pool");
            pass.set_vertex_buffer(0, pool.slice(..self.pool.vertex_slot_bytes));
            draw(&mut pass);
            if let (Some(c), Some(frame_bind)) = (&self.compact, &set.frame_bind) {
                pass.set_pipeline(&c.pipeline);
                pass.set_bind_group(0, &set.bind, &[]);
                pass.set_bind_group(1, frame_bind, &[]);
                pass.set_bind_group(2, &c.atlases[0].as_ref().expect("atlas 0").bind, &[]);
                draw(&mut pass);
            }
            self.used.ring |= 1 << k;
        }
        self.used.pipelines = if self.compact.is_some() { 0b11 } else { 0b01 };
        self.used.modes |= if indirect { 0b10 } else { 0b01 };
        if let Some(t) = timing {
            encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, 16);
        }
        ctx.queue.submit([encoder.finish()]);
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        if let Some(t) = timing {
            // The readback buffer's first map, as a timed frame does it.
            let slice = t.readback.slice(..);
            slice.map_async(wgpu::MapMode::Read, |r| r.expect("timestamp map failed"));
            ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
            drop(slice.get_mapped_range().expect("timestamp range"));
            t.readback.unmap();
            self.used.timestamps = true;
        }
        self.used.waited += 1;
    }

    /// What the submissions so far have used (REND-017): the counters a test compares after the warm-up and after frames.
    pub fn first_use(&self) -> FirstUse {
        FirstUse {
            ring_sets: self.used.ring.count_ones(),
            pipelines: self.used.pipelines.count_ones(),
            timestamp_passes: u32::from(self.used.timestamps),
            draw_modes: self.used.modes.count_ones(),
            waited_submissions: self.used.waited,
        }
    }

    /// Choose how draws are submitted. Indirect needs adapter support (`GpuContext::indirect_draw_supported`); where it is
    /// missing the per-draw loop stays and the mode in effect is returned.
    pub fn set_draw_mode(&mut self, ctx: &GpuContext, mode: DrawMode) -> DrawMode {
        self.draw_mode = if mode == DrawMode::Indirect && !ctx.indirect { DrawMode::PerDraw } else { mode };
        self.draw_mode
    }

    /// The draw mode in effect.
    pub fn draw_mode(&self) -> DrawMode {
        self.draw_mode
    }

    /// Upload a vertex-format tile once; later calls for the same tile do nothing. The renderer places the tile itself: the
    /// lowest free slot of the pool for its level (levels 0-2 in the small vertex pool). A tile that comes back with another
    /// mesh key takes a new slot and its old one is released (reusable after the frames in flight retire). In a compact renderer
    /// a mesh of level 3 or finer is a fallback tile (CON-13, ADR 0033 point 5) and takes a slot of the fallback region of the
    /// vertex pool.
    pub fn upload(&mut self, ctx: &GpuContext, mesh: &TileMesh) -> Result<(), ExecError> {
        self.place_tile(ctx, TileRef::Vertex(mesh), None)
    }

    /// Upload a tile in whichever format the builder chose (`planet_generators::compact::tile_data_field`), placed by the
    /// renderer like [`upload`](TerrainRenderer::upload).
    pub fn upload_tile(&mut self, ctx: &GpuContext, tile: &RenderTile) -> Result<(), ExecError> {
        self.place_tile(ctx, TileRef::of(tile), None)
    }

    /// Upload a tile's vertices into the slot the residency manager chose (`Streamer::slot_of`): the renderer executes the
    /// placement and decides nothing. Whatever tile the slot held before is forgotten, and so is the old slot of this tile.
    /// Do not mix with [`upload`](TerrainRenderer::upload) on one renderer: that one hands out slots itself.
    pub fn upload_at(&mut self, ctx: &GpuContext, mesh: &TileMesh, slot: SlotRef) -> Result<(), ExecError> {
        self.place_tile(ctx, TileRef::Vertex(mesh), Some(slot))
    }

    /// [`upload_at`](TerrainRenderer::upload_at) for a tile in either format. A compact tile goes to a slot of the main pool; a
    /// vertex-format tile of level 3 or finer (a fallback) is given the main slot the manager reserved for it, which stays empty,
    /// and the renderer keeps it in its own fallback region of the vertex pool.
    pub fn upload_tile_at(&mut self, ctx: &GpuContext, tile: &RenderTile, slot: SlotRef) -> Result<(), ExecError> {
        self.place_tile(ctx, TileRef::of(tile), Some(slot))
    }

    fn place_tile(&mut self, ctx: &GpuContext, tile: TileRef, at: Option<SlotRef>) -> Result<(), ExecError> {
        let (id, key, cells) = tile.parts();
        if cells != self.cells {
            return Err(ExecError::BadResolution { id, res: cells });
        }
        // Resident with the same key: nothing to do for a tile the renderer places itself. A different key (other seed, radius,
        // mesh version) replaces it.
        if at.is_none() && self.tiles.get(&id).is_some_and(|r| r.key == key) {
            return Ok(());
        }
        let compact_mode = self.compact.is_some();
        if let TileRef::Compact(_) = tile {
            if id.level() < planet_generators::compact::COMPACT_MIN_LEVEL {
                return Err(ExecError::CompactUnsupported { tile: id });
            }
        }
        let fallback = compact_mode && matches!(tile, TileRef::Vertex(_)) && self.pools.kind_of(id) == PoolKind::Main;
        let (slot, place) = if let TileRef::Compact(_) = tile {
            if !compact_mode {
                return Err(ExecError::CompactUnsupported { tile: id });
            }
            let slot = match at {
                Some(slot) => slot,
                None => self.pools.acquire(PoolKind::Main, self.frame).ok_or(ExecError::PoolFull { tile: id, pool: PoolKind::Main })?,
            };
            if slot.pool != PoolKind::Main {
                return Err(ExecError::CompactUnsupported { tile: id });
            }
            (slot, Place::Atlas)
        } else if fallback {
            let c = self.compact.as_mut().expect("compact mode");
            let v = c.fallback.acquire(self.frame).ok_or(ExecError::PoolFull { tile: id, pool: PoolKind::Vertex })? + c.fallback_base;
            let slot = match at {
                Some(slot) => slot,
                None => SlotRef { pool: PoolKind::Vertex, index: v },
            };
            (slot, Place::Fallback(v))
        } else {
            if compact_mode && at.is_some_and(|slot| slot.pool == PoolKind::Main) {
                // Cannot happen for a fallback (handled above); a level 0-2 mesh does not belong in the atlas pool.
                return Err(ExecError::CompactUnsupported { tile: id });
            }
            let slot = match at {
                Some(slot) => slot,
                None => {
                    let kind = self.pools.kind_of(id);
                    self.pools.acquire(kind, self.frame).ok_or(ExecError::PoolFull { tile: id, pool: kind })?
                }
            };
            (slot, Place::Vertex)
        };
        if at.is_some() && place != Place::Atlas && !fallback {
            let capacity = match slot.pool {
                PoolKind::Main => self.pool.main_slots(),
                PoolKind::Vertex => self.pool.managed_vertex_slots(),
            };
            if slot.index >= capacity {
                return Err(ExecError::BadSlot { tile: id, slot, capacity });
            }
        } else if at.is_some() {
            let capacity = self.pool.main_slots();
            if slot.pool == PoolKind::Main && slot.index >= capacity {
                if let Place::Fallback(v) = place {
                    self.compact.as_mut().expect("compact mode").fallback.release(v - self.pool.managed_vertex_slots(), self.frame);
                }
                return Err(ExecError::BadSlot { tile: id, slot, capacity });
            }
        }
        // The old residency of this tile: a slot the renderer handed out is released (reusable after the frames in flight retire),
        // a fallback region slot always is; a slot the manager chose is the manager's.
        let old = self.tiles.get(&id).copied();
        match tile {
            TileRef::Vertex(m) => self.write_vertex_tile(ctx, m, slot, place),
            TileRef::Compact(c) => self.write_compact_tile(ctx, c, slot),
        }
        if let Some(old) = old {
            self.release_resident(&old, at.is_none());
        }
        if let Some(previous) = self.owners.insert(slot, id) {
            if previous != id {
                if let Some(r) = self.tiles.remove(&previous) {
                    self.release_fallback(&r);
                }
            }
        }
        if let Some(old) = self.tiles.insert(id, Resident { key, slot, place }) {
            if old.slot != slot {
                self.owners.remove(&old.slot);
            }
        }
        Ok(())
    }

    /// Give back what a replaced or forgotten resident held: its fallback slot, and (when the renderer placed it) its pool slot.
    fn release_resident(&mut self, r: &Resident, renderer_placed: bool) {
        self.release_fallback(r);
        let own_fallback_slot = matches!(r.place, Place::Fallback(_)) && r.slot.pool == PoolKind::Vertex;
        if renderer_placed && !own_fallback_slot {
            self.pools.release(r.slot, self.frame);
        }
    }

    fn release_fallback(&mut self, r: &Resident) {
        if let (Place::Fallback(v), Some(c)) = (r.place, self.compact.as_mut()) {
            c.fallback.release(v - c.fallback_base, self.frame);
        }
    }

    fn write_vertex_tile(&mut self, ctx: &GpuContext, mesh: &TileMesh, slot: SlotRef, place: Place) {
        let mut bytes = std::mem::take(&mut self.scratch.vertex);
        bytes.clear();
        vertex_bytes_into(&mut bytes, mesh);
        debug_assert_eq!(bytes.len() as u64, self.pool.vertex_slot_bytes, "a vertex-format tile fills its slot exactly");
        let slot = match place {
            Place::Fallback(v) => SlotRef { pool: PoolKind::Vertex, index: v },
            _ => slot,
        };
        let (target, offset) = self.slot_buffer_for_write(ctx, slot);
        self.uploaded_bytes += bytes.len() as u64;
        // Vertex tiles are written at once even in a batch: a call carries 53,724 bytes, so the call is not the cost and
        // concatenating a run would add a copy of the megabyte (measured, `examples/upload_bench.rs`).
        self.write_buf_now(ctx, target, offset, &bytes);
        self.scratch.vertex = bytes;
    }

    fn write_compact_tile(&mut self, ctx: &GpuContext, tile: &CompactTile, slot: SlotRef) {
        let block_slots = self.pool.block_slots;
        let c = self.compact.as_mut().expect("compact mode");
        let (block, local) = ((slot.index / block_slots) as usize, slot.index % block_slots);
        let cols = c.cols;
        let atlas = c.atlas(&ctx.device, block, &mut self.init);
        let w = self.cells + 1;
        let (x, y) = ((local % cols) * w, (local / cols) * w);
        let n = tile.texels.len() as u64 * 8;
        self.uploaded_bytes += n + COMPACT_RECORD_BYTES;
        if self.scratch.open {
            // Held back: converted straight into the batch's texel buffer, written by the flush.
            let at = self.scratch.texels.len();
            texel_bytes_into(&mut self.scratch.texels, &tile.texels);
            debug_assert_eq!(&self.scratch.texels[at..], &tile.texel_data()[..]);
            self.scratch.tiles.push((block, local, at));
            self.scratch.records.push((slot.index, tile.record_bytes()));
            return;
        }
        let bytes = texel_bytes(&tile.texels);
        debug_assert_eq!(bytes, tile.texel_data());
        self.upload_counters.write_calls += 2;
        self.upload_counters.staging_bytes += bytes.len() as u64 + COMPACT_RECORD_BYTES;
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(8 * w), rows_per_image: Some(w) },
            wgpu::Extent3d { width: w, height: w, depth_or_array_layers: 1 },
        );
        let records = &self.compact.as_ref().expect("compact mode").records;
        ctx.queue.write_buffer(records, u64::from(slot.index) * COMPACT_RECORD_BYTES, &tile.record_bytes());
    }

    fn write_buf_now(&mut self, ctx: &GpuContext, target: BufTarget, offset: u64, bytes: &[u8]) {
        let buffer = match target {
            BufTarget::VertexPool => self.vertex_pool.as_ref().expect("vertex pool allocated"),
            BufTarget::Block(b) => self.blocks[b].as_ref().expect("block allocated"),
        };
        let trw = std::time::Instant::now();
        ctx.queue.write_buffer(buffer, offset, bytes);
        self.upload_times.queue += trw.elapsed();
        self.upload_counters.write_calls += 1;
        self.upload_counters.staging_bytes += bytes.len() as u64;
    }

    /// How tile writes reach the queue (default [`UploadMode::Batched`]); the results are the same in both modes.
    pub fn set_upload_mode(&mut self, mode: UploadMode) {
        self.upload_mode = mode;
    }

    pub fn upload_mode(&self) -> UploadMode {
        self.upload_mode
    }

    /// The bytes of the compact pools as the GPU holds them (REND-016): every allocated atlas (`side x side` texels of 8 bytes, row
    /// major, `None` for a block not yet allocated) and the slot records. A test aid: it submits copies and waits. Empty for a
    /// renderer without the compact format.
    pub fn read_compact_pools(&self, ctx: &GpuContext) -> (Vec<Option<Vec<u8>>>, Vec<u8>) {
        let Some(c) = &self.compact else { return (Vec::new(), Vec::new()) };
        let read_buffer = |buffer: &wgpu::Buffer, size: u64| -> Vec<u8> {
            let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pool-readback"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = ctx.device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, size);
            ctx.queue.submit([encoder.finish()]);
            readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
            ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
            let data = readback.slice(..).get_mapped_range().expect("readback range").to_vec();
            readback.unmap();
            data
        };
        let side = c.side;
        let row = 8 * side;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let atlases = c
            .atlases
            .iter()
            .map(|a| {
                let a = a.as_ref()?;
                let size = u64::from(padded) * u64::from(side);
                let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("atlas-readback"),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = ctx.device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &a.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(side) },
                    },
                    wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
                );
                ctx.queue.submit([encoder.finish()]);
                readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
                ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
                let data = readback.slice(..).get_mapped_range().expect("readback range");
                let mut out = Vec::with_capacity((row * side) as usize);
                for r in data.chunks_exact(padded as usize) {
                    out.extend_from_slice(&r[..row as usize]);
                }
                drop(data);
                readback.unmap();
                Some(out)
            })
            .collect();
        let records = read_buffer(&c.records, c.records.size());
        (atlases, records)
    }

    /// Queue calls and staging bytes of the tile writes so far.
    pub fn upload_counters(&self) -> UploadCounters {
        self.upload_counters
    }

    /// The time of the tile writes so far, whole and in the queue calls.
    pub fn upload_times(&self) -> UploadTimes {
        self.upload_times
    }

    /// Place a frame's tiles, in order, each in the slot the manager chose ([`upload_tile_at`](TerrainRenderer::upload_tile_at) for
    /// each). In [`UploadMode::Batched`] the writes of compact tiles are held back and issued after the last tile: a slot written
    /// twice keeps the last tile, and each run of adjacent slots is one call (a run in one row of an atlas for texels, a run of
    /// slot indices for records), so a frame makes one call per run instead of two per slot. Vertex-format tiles are written at
    /// once, one call each (their 53,724 bytes dominate the call). The bookkeeping of every tile is as with single uploads. On an
    /// error the tiles placed before it are written and the error returned.
    pub fn upload_tiles_at(&mut self, ctx: &GpuContext, tiles: &[(&RenderTile, SlotRef)]) -> Result<(), ExecError> {
        let started = std::time::Instant::now();
        if self.upload_mode == UploadMode::PerSlot {
            for (tile, slot) in tiles {
                self.upload_tile_at(ctx, tile, *slot)?;
            }
            self.upload_times.total += started.elapsed();
            return Ok(());
        }
        self.scratch.open = true;
        let mut result = Ok(());
        for (tile, slot) in tiles {
            result = self.upload_tile_at(ctx, tile, *slot);
            if result.is_err() {
                break;
            }
        }
        self.flush_batch(ctx);
        self.upload_times.total += started.elapsed();
        result
    }

    /// The capacity, in elements, of the staging buffers of the batched tile writes: constant once the buffers have grown to the
    /// largest batch, which is what "no allocation per frame in the batched path" means to a test (REND-018).
    pub fn batch_scratch_capacity(&self) -> usize {
        self.scratch.capacity()
    }

    /// Issue the held-back writes: sorted by target and offset, a later write to the same place replacing an earlier one, each
    /// run of adjacent places one call. Everything is staged in the renderer's scratch buffers (cleared here, kept).
    fn flush_batch(&mut self, ctx: &GpuContext) {
        if !self.scratch.open {
            return;
        }
        let sc = &mut self.scratch;
        sc.open = false;
        let w = self.cells + 1;
        let row_bytes = 8 * w as usize;
        let tile_bytes = row_bytes * w as usize;
        sc.texel_order.clear();
        sc.texel_order.extend(sc.tiles.iter().enumerate().map(|(i, &(block, local, _))| (block, local, i as u32)));
        sc.texel_order.sort_unstable();
        sc.record_order.clear();
        sc.record_order.extend(sc.records.iter().enumerate().map(|(i, &(index, _))| (index, i as u32)));
        sc.record_order.sort_unstable();
        // Of several writes to one place the last (the greatest index, sorted last) is kept.
        let places = dedup_places(&mut sc.texel_order, |o| (o.0, o.1));
        // Compact texels: runs of adjacent slots in one row of one atlas, interleaved row by row into one rectangle.
        if places > 0 {
            let cols = self.compact.as_ref().expect("compact mode").cols;
            let mut k = 0;
            while k < places {
                let (block, first, _) = sc.texel_order[k];
                let mut n = 1;
                while k + n < places
                    && (sc.texel_order[k + n].0, sc.texel_order[k + n].1) == (block, first + n as u32)
                    && !(first + n as u32).is_multiple_of(cols)
                {
                    n += 1;
                }
                sc.run.clear();
                sc.run.resize(n * tile_bytes, 0);
                for t in 0..n {
                    let at = sc.tiles[sc.texel_order[k + t].2 as usize].2;
                    let src = &sc.texels[at..at + tile_bytes];
                    for row in 0..w as usize {
                        let dst = (row * n + t) * row_bytes;
                        sc.run[dst..dst + row_bytes].copy_from_slice(&src[row * row_bytes..(row + 1) * row_bytes]);
                    }
                }
                let atlas = self.compact.as_ref().expect("compact mode").atlases[block].as_ref().expect("atlas allocated");
                let trw = std::time::Instant::now();
                ctx.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &atlas.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: (first % cols) * w, y: (first / cols) * w, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &sc.run,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some((n * row_bytes) as u32), rows_per_image: Some(w) },
                    wgpu::Extent3d { width: n as u32 * w, height: w, depth_or_array_layers: 1 },
                );
                self.upload_times.queue += trw.elapsed();
                self.upload_counters.write_calls += 1;
                self.upload_counters.staging_bytes += sc.run.len() as u64;
                k += n;
            }
        }
        // Slot records: runs of adjacent slot indices (across blocks: one buffer).
        let places = dedup_places(&mut sc.record_order, |o| o.0);
        let mut k = 0;
        while k < places {
            let mut n = 1;
            while k + n < places && sc.record_order[k + n].0 == sc.record_order[k].0 + n as u32 {
                n += 1;
            }
            sc.run_records.clear();
            for t in 0..n {
                sc.run_records.extend_from_slice(&sc.records[sc.record_order[k + t].1 as usize].1);
            }
            let buffer = &self.compact.as_ref().expect("compact mode").records;
            let trw = std::time::Instant::now();
            ctx.queue.write_buffer(buffer, u64::from(sc.record_order[k].0) * COMPACT_RECORD_BYTES, &sc.run_records);
            self.upload_times.queue += trw.elapsed();
            self.upload_counters.write_calls += 1;
            self.upload_counters.staging_bytes += sc.run_records.len() as u64;
            k += n;
        }
        sc.texels.clear();
        sc.tiles.clear();
        sc.records.clear();
    }

    /// The buffer and byte offset of `slot`, allocating its block (or the vertex pool) on first use.
    fn slot_buffer_for_write(&mut self, ctx: &GpuContext, slot: SlotRef) -> (BufTarget, u64) {
        match slot.pool {
            PoolKind::Vertex => {
                let offset = u64::from(slot.index) * self.pool.vertex_slot_bytes;
                self.vertex_pool_buffer(&ctx.device);
                (BufTarget::VertexPool, offset)
            }
            PoolKind::Main => {
                let (block, within) = (slot.index / self.pool.block_slots, slot.index % self.pool.block_slots);
                let offset = u64::from(within) * self.slot_bytes;
                self.block_buffer(&ctx.device, block as usize);
                (BufTarget::Block(block as usize), offset)
            }
        }
    }

    /// The small vertex pool, allocated whole on first use.
    fn vertex_pool_buffer(&mut self, device: &wgpu::Device) -> &wgpu::Buffer {
        let bytes = self.pool.vertex_slot_bytes * u64::from(self.pool.vertex_slots);
        let init = &mut self.init;
        self.vertex_pool.get_or_insert_with(|| {
            init.pool_allocations += 1;
            make_pool_buffer(device, "terrain-vertex-pool", bytes)
        })
    }

    /// Block `block` of the vertex-format main pool, allocated on first use.
    fn block_buffer(&mut self, device: &wgpu::Device, block: usize) -> &wgpu::Buffer {
        if self.blocks.len() <= block {
            self.blocks.resize_with(block + 1, || None);
        }
        let bytes = self.pool.block_bytes();
        let init = &mut self.init;
        self.blocks[block].get_or_insert_with(|| {
            init.pool_allocations += 1;
            make_pool_buffer(device, "terrain-block", bytes)
        })
    }

    /// Release the slot `upload` gave to `tile` (reusable after the frames in flight retire) and forget the tile.
    pub fn evict(&mut self, tile: planet_core::TileId) -> bool {
        let Some(r) = self.tiles.remove(&tile) else { return false };
        self.owners.remove(&r.slot);
        self.release_resident(&r, true);
        true
    }

    /// Forget `tile` without touching the pools: for a caller that places tiles with [`upload_at`](TerrainRenderer::upload_at).
    /// (A fallback slot of the compact renderer is the renderer's own and is released.)
    pub fn forget(&mut self, tile: planet_core::TileId) -> bool {
        let Some(r) = self.tiles.remove(&tile) else { return false };
        self.owners.remove(&r.slot);
        self.release_fallback(&r);
        true
    }

    /// The slot a resident tile occupies.
    pub fn slot_of(&self, tile: planet_core::TileId) -> Option<SlotRef> {
        self.tiles.get(&tile).map(|r| r.slot)
    }

    /// Whether a resident tile is in the compact format, and whether it is a fallback tile of the compact renderer.
    pub fn format_of(&self, tile: planet_core::TileId) -> Option<ResidentFormat> {
        self.tiles.get(&tile).map(|r| match r.place {
            Place::Vertex => ResidentFormat::Vertex,
            Place::Atlas => ResidentFormat::Compact,
            Place::Fallback(_) => ResidentFormat::Fallback,
        })
    }

    /// Resident tiles by format: `(vertex, compact, fallback)`.
    pub fn format_counts(&self) -> (usize, usize, usize) {
        let mut n = (0, 0, 0);
        for r in self.tiles.values() {
            match r.place {
                Place::Vertex => n.0 += 1,
                Place::Atlas => n.1 += 1,
                Place::Fallback(_) => n.2 += 1,
            }
        }
        n
    }

    /// The pool configuration whose `allocated_bytes` bound this renderer's allocations.
    pub fn pool_config(&self) -> &PoolConfig {
        &self.pool
    }

    /// Slots handed out by `upload`, retiring and free in the main pool: `(in use, retiring, free)`.
    pub fn main_slot_counts(&self) -> (u32, u32, u32) {
        (self.pools.main.in_use(), self.pools.main.retiring(), self.pools.main.free())
    }

    /// Slots of the compact renderer's fallback region: `(in use, retiring, free)`; zeros for the vertex-format renderer.
    pub fn fallback_slot_counts(&self) -> (u32, u32, u32) {
        self.compact.as_ref().map_or((0, 0, 0), |c| (c.fallback.in_use(), c.fallback.retiring(), c.fallback.free()))
    }

    /// Bytes of the tile pools (vertex blocks, atlases, the vertex pool), the slot records, the index buffer and the node record
    /// buffers as allocated now (summed from the buffers' and textures' real sizes). The pools, records and the index buffer equal
    /// what `PoolConfig::allocated_bytes_with` says for the live blocks; the node records are as large as the largest plan so
    /// far, at most the configured buffer; the whole stays under the terrain share (STRM-006).
    pub fn pool_allocated_bytes(&self) -> u64 {
        let blocks: u64 = self.blocks.iter().flatten().map(wgpu::Buffer::size).sum();
        let atlases: u64 = self.compact.as_ref().map_or(0, |c| {
            c.atlases.iter().flatten().count() as u64 * u64::from(c.side) * u64::from(c.side) * planet_generators::compact::TEXEL_BYTES
        });
        let records = self.compact.as_ref().map_or(0, |c| c.records.size());
        blocks
            + atlases
            + records
            + self.vertex_pool.as_ref().map_or(0, wgpu::Buffer::size)
            + self.indices.size()
            + self.node_buffer_bytes()
    }

    /// Bytes of the per-frame buffers (the ring of globals, node records, draw arguments and tile references) as allocated now (it grows with the largest plan, up to the configured maximum).
    pub fn node_buffer_bytes(&self) -> u64 {
        self.ring.iter().map(|s| s.globals.size() + s.nodes.size() + s.args.size() + s.refs.as_ref().map_or(0, wgpu::Buffer::size)).sum()
    }

    /// Blocks of the main pool (vertex blocks or atlases) allocated so far, and whether the vertex pool is.
    pub fn live_pools(&self) -> (u32, bool) {
        let atlases = self.compact.as_ref().map_or(0, |c| c.atlases.iter().flatten().count());
        ((self.blocks.iter().flatten().count() + atlases) as u32, self.vertex_pool.is_some())
    }

    /// Whether `tile` is resident with exactly the mesh `key` (what a plan draw expects): the test for "needs an upload".
    pub fn has_mesh(&self, tile: planet_core::TileId, key: u64) -> bool {
        self.tile_key(tile) == Some(key)
    }

    /// Key of the resident mesh of `tile`, if any.
    pub fn tile_key(&self, tile: planet_core::TileId) -> Option<u64> {
        self.tiles.get(&tile).map(|r| r.key)
    }

    pub fn resident_tiles(&self) -> usize {
        self.tiles.len()
    }

    /// Draw the plan and wait until the GPU has finished it. Every planned tile must have been uploaded. With `timed`, GPU time is
    /// measured when supported.
    pub fn render(&mut self, ctx: &GpuContext, plan: &TerrainPlan, timed: bool) -> Result<(FrameTiming, TerrainStats), ExecError> {
        self.render_inner(ctx, plan, timed, true)
    }

    /// Draw the plan and return once it is submitted, without waiting for the GPU (STRM-016). The timing carries the CPU time of
    /// the call (`total_ms` equals it, there is no GPU time). Count finished frames with [`retired_frames`](TerrainRenderer::retired_frames).
    pub fn submit(&mut self, ctx: &GpuContext, plan: &TerrainPlan) -> Result<(FrameTiming, TerrainStats), ExecError> {
        self.render_inner(ctx, plan, false, false)
    }

    /// Frames the GPU has finished so far (polls the device without blocking).
    pub fn retired_frames(&mut self, ctx: &GpuContext) -> u64 {
        let _ = ctx.device.poll(wgpu::PollType::Poll);
        self.reap()
    }

    /// Block until the oldest submitted frame has finished; the one wait of a pipelined loop, taken only when the bound on frames
    /// in flight is reached. Returns the frames finished in total.
    pub fn wait_oldest(&mut self, ctx: &GpuContext) -> u64 {
        if let Some(idx) = self.in_flight.front().cloned() {
            ctx.device.poll(wgpu::PollType::Wait { submission_index: Some(idx), timeout: None }).expect("device poll failed");
        }
        self.reap()
    }

    fn reap(&mut self) -> u64 {
        let retired = self.retired.load(std::sync::atomic::Ordering::SeqCst);
        while self.in_flight.len() as u64 > self.frame.saturating_sub(retired) {
            self.in_flight.pop_front();
        }
        retired
    }

    fn render_inner(
        &mut self,
        ctx: &GpuContext,
        plan: &TerrainPlan,
        timed: bool,
        wait: bool,
    ) -> Result<(FrameTiming, TerrainStats), ExecError> {
        let started = std::time::Instant::now();
        if plan.size != self.size {
            return Err(ExecError::SizeMismatch { plan: plan.size, renderer: self.size });
        }
        if plan.cells != self.cells {
            return Err(ExecError::BadResolution { id: planet_core::TileId::from_raw(0).expect("root"), res: plan.cells });
        }
        if plan.nodes.len() > self.max_nodes {
            return Err(ExecError::TooManyNodes { planned: plan.nodes.len(), max: self.max_nodes });
        }
        for d in &plan.nodes {
            match self.tiles.get(&d.tile) {
                None => return Err(ExecError::MissingTile(d.tile)),
                Some(r) if r.key != d.mesh_key => return Err(ExecError::StaleMesh { tile: d.tile, resident: r.key, planned: d.mesh_key }),
                Some(_) => {}
            }
        }
        let [lo, hi] = plan.height_range;
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(ExecError::BadHeightRange { lo, hi });
        }
        if let Some(l) = &plan.lit {
            l.validate().map_err(ExecError::BadLitPlan)?;
        }
        if plan.lit.is_some() && self.lit.is_none() {
            return Err(ExecError::LitNotEnabled);
        }
        self.frame += 1;
        let device = &ctx.device;
        if plan.nodes.len() > self.node_capacity {
            self.node_capacity = plan.nodes.len().next_power_of_two().min(self.max_nodes);
            // Frames in flight keep the buffers they use alive; the new ring serves the frames from here on.
            self.ring = Self::make_ring(device, &self.bgl, self.node_capacity, self.compact.as_ref());
            self.init.ring_builds += 1;
            self.used.ring = 0;
        }
        let mut globals = f32_bytes(&plan.view_projection);
        match &plan.lit {
            // The lit path: the pre-exposure factor in range.z, the view in mode.x and the sun direction in mode.yzw (as bits), the
            // water level in extra.x and the albedo table of the look, all decided by the plan.
            Some(lit) => {
                let factor = planet_frame::look::pre_exposure_factor_with(&lit.units, f64::from(lit.exposure.pre_ev100())) as f32;
                globals.extend(f32_bytes(&[lo, hi, factor, 0.0]));
                globals.extend(
                    [lit.view.index(), lit.sun_dir[0].to_bits(), lit.sun_dir[1].to_bits(), lit.sun_dir[2].to_bits()]
                        .iter()
                        .flat_map(|v: &u32| v.to_le_bytes()),
                );
                globals.extend(f32_bytes(&[lit.water_level_m, 0.0, 0.0, 0.0]));
                globals.extend(f32_bytes(&lit.albedo.to_words()));
            }
            None => {
                globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
                globals.extend([view_index(plan.view), 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
                globals.extend(vec![0u8; GLOBALS_BYTES as usize - globals.len()]);
            }
        }
        debug_assert_eq!(globals.len() as u64, GLOBALS_BYTES);
        // Each frame writes its own set of the ring, so a frame still in flight keeps the records it reads.
        let ring_index = (self.frame % RING as u64) as usize;
        self.used.ring |= 1 << ring_index;
        self.used.modes |= if self.draw_mode == DrawMode::Indirect { 0b10 } else { 0b01 };
        for d in &plan.nodes {
            self.used.pipelines |= if self.tiles[&d.tile].place == Place::Atlas { 0b10 } else { 0b01 };
        }
        let set = &self.ring[ring_index];
        ctx.queue.write_buffer(&set.globals, 0, &globals);
        let mut nodes = vec![0u8; NODE_BYTES as usize * plan.nodes.len().max(1)];
        for (i, d) in plan.nodes.iter().enumerate() {
            let r = d.camera_relative_origin;
            let c = d.color.map(|v| f32::from(v) / 255.0);
            let mut floats = vec![r[0], r[1], r[2], 0.0, c[0], c[1], c[2], c[3], d.morph_start_m, d.morph_end_m, 0.0, 0.0];
            for p in grid_phases([d.origin.0.x, d.origin.0.y, d.origin.0.z]) {
                floats.extend([p[0], p[1], p[2], 0.0]);
            }
            // The unit up vector of the tile origin and the inverse of its distance from the planet centre (decided in f64, rounded
            // once): the shader forms up at a fragment as normalize(up + local * inv_r), which is tile-local arithmetic (CON-11).
            let o = d.origin.0;
            let len = (o.x * o.x + o.y * o.y + o.z * o.z).sqrt();
            floats.extend([(o.x / len) as f32, (o.y / len) as f32, (o.z / len) as f32, (1.0 / len) as f32]);
            let b = f32_bytes(&floats);
            nodes[i * NODE_BYTES as usize..(i + 1) * NODE_BYTES as usize].copy_from_slice(&b);
        }
        ctx.queue.write_buffer(&set.nodes, 0, &nodes);
        let mut stats = TerrainStats { node_record_bytes: nodes.len() as u64, ..Default::default() };
        // The argument records come from the plan and the slots alone (CON-03: the GPU culls and decides nothing).
        let vertices_per_slot = self.pool.vertex_slot_bytes / MeshVertex::GPU_STRIDE;
        let placed = |r: &Resident| -> (DrawBuffer, u64) {
            match r.place {
                Place::Vertex => {
                    let (buffer, within) = buffer_of(r.slot, self.pool.block_slots);
                    (buffer, u64::from(within) * vertices_per_slot)
                }
                Place::Atlas => (DrawBuffer::Atlas(r.slot.index / self.pool.block_slots), 0),
                Place::Fallback(v) => (DrawBuffer::VertexPool, u64::from(v) * vertices_per_slot),
            }
        };
        if let (Some(c), Some(refs)) = (&self.compact, &set.refs) {
            // One tile reference per node (the texel origin of its slot and the slot's record); unused entries stay zero.
            let w = self.cells + 1;
            let mut bytes = vec![0u8; REF_BYTES as usize * plan.nodes.len().max(1)];
            for (i, d) in plan.nodes.iter().enumerate() {
                let r = &self.tiles[&d.tile];
                if r.place == Place::Atlas {
                    let local = r.slot.index % self.pool.block_slots;
                    let words = [(local % c.cols) * w, (local / c.cols) * w, r.slot.index, 0u32];
                    for (k, v) in words.iter().enumerate() {
                        bytes[i * REF_BYTES as usize + 4 * k..i * REF_BYTES as usize + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
                    }
                }
            }
            ctx.queue.write_buffer(refs, 0, &bytes);
            stats.tile_ref_bytes = bytes.len() as u64;
        }
        let indirect = (self.draw_mode == DrawMode::Indirect)
            .then(|| build_draw_list_placed(plan.nodes.iter().map(|d| placed(&self.tiles[&d.tile])), self.index_count));
        if let Some(list) = &indirect {
            ctx.queue.write_buffer(&set.args, 0, &list.record_bytes());
            stats.argument_bytes = list.argument_bytes();
        }

        let timing = if timed { self.timing.as_ref() } else { None };
        self.used.timestamps |= timing.is_some() && wait;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain") });
        // The lit path draws into its scene target with its own pipelines and clears to black without coverage; the debug path
        // is the plan's colour target as before.
        let lit_gpu = plan.lit.as_ref().and(self.lit.as_ref());
        let (color_target, vertex_pipeline, compact_pipeline) = match lit_gpu {
            Some(l) => (l.path.hdr_view(), &l.pipeline, l.compact_pipeline.as_ref()),
            None => (&self.color_view, &self.pipeline, self.compact.as_ref().map(|c| &c.pipeline)),
        };
        {
            let [r, g, b, a] = if lit_gpu.is_some() { [0.0; 4] } else { plan.clear_color.map(|v| f64::from(v) / 255.0) };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
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
            pass.set_pipeline(vertex_pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(0, &set.bind, &[]);
            // Which pipeline is bound: a run of an atlas needs the compact one with its two bind groups.
            let mut compact_bound: Option<u32> = None;
            let mut vertex_bound = true;
            match &indirect {
                Some(list) => {
                    // One multi-draw per run of consecutive nodes in one buffer or atlas; the node index is the first instance.
                    for run in &list.runs {
                        match run.buffer {
                            DrawBuffer::Atlas(b) => {
                                if compact_bound != Some(b) {
                                    let c = self.compact.as_ref().expect("compact renderer");
                                    if vertex_bound || compact_bound.is_none() {
                                        pass.set_pipeline(compact_pipeline.expect("compact renderer"));
                                        pass.set_bind_group(0, &set.bind, &[]);
                                        pass.set_bind_group(1, set.frame_bind.as_ref(), &[]);
                                    }
                                    pass.set_bind_group(2, &c.atlases[b as usize].as_ref().expect("atlas of a resident tile").bind, &[]);
                                    compact_bound = Some(b);
                                    vertex_bound = false;
                                }
                            }
                            other => {
                                if !vertex_bound {
                                    pass.set_pipeline(vertex_pipeline);
                                    pass.set_bind_group(0, &set.bind, &[]);
                                    vertex_bound = true;
                                    compact_bound = None;
                                }
                                let buffer = match other {
                                    DrawBuffer::VertexPool => self.vertex_pool.as_ref().expect("vertex pool of a resident tile"),
                                    DrawBuffer::Block(b) => self.blocks[b as usize].as_ref().expect("block of a resident tile"),
                                    DrawBuffer::Atlas(_) => unreachable!(),
                                };
                                pass.set_vertex_buffer(0, buffer.slice(..));
                            }
                        }
                        pass.multi_draw_indexed_indirect(&set.args, u64::from(run.first) * DRAW_RECORD_BYTES, run.count);
                        stats.encoded_draws += 1;
                    }
                }
                None => {
                    for (i, d) in plan.nodes.iter().enumerate() {
                        let r = self.tiles[&d.tile];
                        if r.place == Place::Atlas {
                            let c = self.compact.as_ref().expect("compact renderer");
                            let b = r.slot.index / self.pool.block_slots;
                            if compact_bound != Some(b) {
                                if vertex_bound || compact_bound.is_none() {
                                    pass.set_pipeline(compact_pipeline.expect("compact renderer"));
                                    pass.set_bind_group(0, &set.bind, &[]);
                                    pass.set_bind_group(1, set.frame_bind.as_ref(), &[]);
                                }
                                pass.set_bind_group(2, &c.atlases[b as usize].as_ref().expect("atlas of a resident tile").bind, &[]);
                                compact_bound = Some(b);
                                vertex_bound = false;
                            }
                        } else {
                            if !vertex_bound {
                                pass.set_pipeline(vertex_pipeline);
                                pass.set_bind_group(0, &set.bind, &[]);
                                vertex_bound = true;
                                compact_bound = None;
                            }
                            let (buffer, offset) = self.vertex_slot_buffer(&r);
                            pass.set_vertex_buffer(0, buffer.slice(offset..offset + self.pool.vertex_slot_bytes));
                        }
                        pass.draw_indexed(0..self.index_count, 0, i as u32..i as u32 + 1);
                        stats.encoded_draws += 1;
                    }
                }
            }
            stats.draw_calls = plan.nodes.len() as u32;
            stats.triangles = plan.nodes.len() as u64 * u64::from(self.index_count) / 3;
        }
        if let (Some(l), Some(lit)) = (self.lit.as_mut(), plan.lit.as_ref()) {
            l.path.record_post(ctx, &mut encoder, &self.color_view, lit);
        }
        if let Some(t) = timing {
            encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, 16);
        }
        let command_buffer = encoder.finish();
        let index = ctx.queue.submit([command_buffer]);
        let retired = std::sync::Arc::clone(&self.retired);
        ctx.queue.on_submitted_work_done(move || {
            retired.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        self.in_flight.push_back(index);
        let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
        if !wait {
            return Ok((FrameTiming { cpu_ms, total_ms: cpu_ms, gpu_ms: None }, stats));
        }
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
            // A zero or negative interval is a broken query, not a free frame: report nothing instead of 0 ms.
            (delta > 0).then(|| delta as f64 * f64::from(ctx.queue.get_timestamp_period()) / 1.0e6)
        });
        self.reap();
        Ok((FrameTiming { cpu_ms, total_ms, gpu_ms }, stats))
    }

    /// The vertex buffer and byte offset of a resident tile held in the vertex format.
    fn vertex_slot_buffer(&self, r: &Resident) -> (&wgpu::Buffer, u64) {
        let slot = match r.place {
            Place::Fallback(v) => SlotRef { pool: PoolKind::Vertex, index: v },
            _ => r.slot,
        };
        match slot.pool {
            PoolKind::Vertex => {
                (self.vertex_pool.as_ref().expect("vertex pool of a resident tile"), u64::from(slot.index) * self.pool.vertex_slot_bytes)
            }
            PoolKind::Main => (
                self.blocks[(slot.index / self.pool.block_slots) as usize].as_ref().expect("block of a resident tile"),
                u64::from(slot.index % self.pool.block_slots) * self.slot_bytes,
            ),
        }
    }

    /// The colour target the terrain pass draws into, for the window's blit (`window.rs`).
    pub(crate) fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    /// The size of the colour target in pixels.
    pub fn target_size(&self) -> (u32, u32) {
        self.size
    }

    /// Read the last rendered frame back.
    pub fn read_color(&self, ctx: &GpuContext) -> RgbaFrame {
        read_rgba8(ctx, &self.color, self.size.0, self.size.1)
    }
}

/// Execute the plan on the GPU and read the frame back (one-shot wrapper over [`TerrainRenderer`]).
pub fn execute_terrain(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[&TileMesh]) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    let lookup: HashMap<_, _> = meshes.iter().map(|m| (m.id, *m)).collect();
    let mut renderer = TerrainRenderer::new(ctx, plan.size, plan.cells);
    for d in &plan.nodes {
        let m = lookup.get(&d.tile).ok_or(ExecError::MissingTile(d.tile))?;
        renderer.upload(ctx, m)?;
    }
    let (_, mut stats) = renderer.render(ctx, plan, false)?;
    stats.vertex_bytes = renderer.uploaded_bytes;
    Ok((renderer.read_color(ctx), stats))
}

/// [`execute_terrain`] for tiles in either format (`planet_generators::compact::tile_data_field`): the product's tile path. The
/// renderer's pools are sized to the plan (one atlas just large enough for its tiles); a plan of only vertex-format tiles uses the
/// vertex-format renderer, as before.
pub fn execute_terrain_tiles(ctx: &GpuContext, plan: &TerrainPlan, tiles: &[&RenderTile]) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    let lookup: HashMap<_, _> = tiles.iter().map(|t| (t.id(), *t)).collect();
    let mut renderer = if tiles.iter().any(|t| t.is_compact()) {
        let cells = plan.cells;
        let cols = (plan.nodes.len().max(1) as f64).sqrt().ceil() as u32;
        let side = cols * (cells + 1);
        if side > crate::compact::MAX_ONE_SHOT_SIDE {
            return Err(ExecError::BadPool(format!(
                "a one-shot atlas for {} nodes of {cells} cells is {side} texels wide, over the {} of the default texture limit; use a persistent renderer",
                plan.nodes.len(),
                crate::compact::MAX_ONE_SHOT_SIDE
            )));
        }
        let block = u64::from(side) * u64::from(side) * planet_generators::compact::TEXEL_BYTES;
        let share = planet_streaming::budget::FIXED_RESERVE_BYTES + block;
        let pool = planet_frame::pool::compact_format_pool_with(share, cells, side, 1).map_err(ExecError::BadPool)?;
        TerrainRenderer::with_compact_pools(ctx, plan.size, cells, pool)
    } else {
        TerrainRenderer::new(ctx, plan.size, plan.cells)
    };
    for d in &plan.nodes {
        let t = lookup.get(&d.tile).ok_or(ExecError::MissingTile(d.tile))?;
        renderer.upload_tile(ctx, t)?;
    }
    let (_, mut stats) = renderer.render(ctx, plan, false)?;
    stats.vertex_bytes = renderer.uploaded_bytes;
    Ok((renderer.read_color(ctx), stats))
}
