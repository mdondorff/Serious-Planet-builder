//! Executes a `TerrainPlan` (CON-03): one indexed draw per planned node, camera-relative f32 positions, reversed-Z
//! `Depth32Float` with an infinite far plane (ADR 0002). The renderer makes no decisions: nodes, matrices, morph
//! intervals and colours all come from the plan. Shaders see only tile-local and camera-relative quantities (CON-11).

use crate::compact::{compact_terrain_shader, CompactGpu, REF_BYTES};
use crate::lit::LitPath;
use crate::water_gpu::{CompactLayouts, WaterGpu};
use crate::{read_rgba8, ExecError, GpuContext, RgbaFrame};
use planet_frame::draw::{buffer_of, build_draw_list_placed, DRAW_RECORD_BYTES};
use planet_frame::{TerrainPlan, TerrainView};
use planet_generators::compact::CompactTile;
use planet_generators::mesh::grid_indices;
use planet_streaming::{PoolConfig, PoolKind, SlotPools, SlotRef, Tier};
use std::collections::HashMap;

/// Grid line spacings of the relief view (metres): 1 cm to 1,000 km, one decade apart.
pub const GRID_SPACINGS_M: [f64; 9] = [0.01, 0.1, 1.0, 10.0, 100.0, 1.0e3, 1.0e4, 1.0e5, 1.0e6];

/// Bytes of one node record (origin, colour, morph interval, one grid phase per decade) in the storage buffer the shader
/// indexes with the instance index.
pub(crate) const NODE_BYTES: u64 = 48 + 16 * GRID_SPACINGS_M.len() as u64 + 16;
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

/// The product shader up to the vertex stage: the globals, the node record, the vertex output and the declarations the stages share.
/// The vertex stage itself is the compact tile's (`compact_terrain_shader`, REND-016) or the reference renderer's.
pub(crate) const SHADER_HEAD: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    range: vec4<f32>,
    mode: vec4<u32>,
    // Lit path only: x = the water level W in metres (ADR 0041); y = the debug overlay (VIEW-014: 0 none, 1 tiles, 2 mesh); z = the growth of
    // the shore ramp with the distance from the camera (REND-030, the plan's `shore_ramp_per_m`); w is reserved.
    extra: vec4<f32>,
    // Lit path only: the albedo table of the look (ADR 0039 point 3): eight colours with a threshold in the fourth lane, then
    // (snow_c_full, snow_c_none, snow_cos_none, snow_cos_full) and (beach_height, beach_fade, shore_blend, 0).
    albedo: array<vec4<f32>, 10>,
    // The edit preview (REND-033), read only for a previewed node and only when extra.w (the count in the low 6 bits, at most 32, and 64 times the number of stamps of the delta above them) is above 0: per stamp two
    // entries, (anchor relative to the camera, reach in metres) and (amount in metres, kind 1 hill / 2 crater / 3 plateau, sign, bound).
    preview: array<vec4<f32>, 64>,
    // The region overlay (VIEW-030), read only when extra.y is 3: the count in the first entry's x, then per polygon 37 entries: five of
    // header (anchor relative to the camera and the point count; e1 and the red of the tint; e2 and the green; the unit up and the blue;
    // the square of the reach and the band), then 32 of points, two per entry (x, y metres in the anchor's tangent plane).
    regions: array<vec4<f32>, 297>,
};
struct Node {
    origin_rel: vec4<f32>,
    color: vec4<f32>,
    // x, y: vertex distances between which a vertex morphs to the parent grid; z: 1 for a previewed node (REND-033), else 0 (the shore ramp
    // follows the fragment's distance, REND-030, and is in the globals); w: the tile's level of detail (the wireframe overlay's colour, VIEW-014; the plan's node-level
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
    // The tile coordinates (u, v) of the vertex, from which the fragment stage reads the tile's climate block (the reference renderer
    // writes zeros: it draws no lit view).
    @location(6) aux: vec2<f32>,
};
"#;

/// The product shader after the vertex stage (the helpers, the debug fragment stage and the lit fragment stages), shared by the compact
/// vertex stage (`compact_terrain_shader`) and the test-only reference renderer.
pub(crate) const SHADER_TAIL: &str = r#"
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

// The fragment entries take the quadrant cut only in the pipelines made for nodes drawn over some quadrants (`CUT_QUADRANTS` is set to 1
// at their creation); the pipelines of whole nodes have it false, so their fragment stages hold no `discard` and keep early depth writes.
override CUT_QUADRANTS: bool = false;

// A node drawn over some quadrants of its tile only (STRM-016, SPIKE-18): its record's `origin_rel.w` holds the quadrants it leaves out as
// bits, bit k for child k of the tile's `children()` = 2 * (x half) + (y half), the halves of the tile coordinates (`aux` is the lattice
// position over the tile, i along x and j along y). A node drawn whole has 0 there and never takes the branch. The caller discards after
// its derivatives, in uniform control flow.
fn quadrant_cut_at(node_index: u32, aux: vec2<f32>) -> bool {
    let left_out = u32(nodes[node_index].origin_rel.w + 0.5);
    if (left_out == 0u) {
        return false;
    }
    let q = select(0u, 2u, aux.x >= 0.5) + select(0u, 1u, aux.y >= 0.5);
    return ((left_out >> q) & 1u) != 0u;
}

fn quadrant_cut(in: VsOut) -> bool {
    return quadrant_cut_at(in.node_index, in.aux);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // The derivatives of the inner function come first; a node drawn over some quadrants discards the rest after them (STRM-016).
    let c = fs_view(in);
    if (CUT_QUADRANTS && quadrant_cut(in)) {
        discard;
    }
    return c;
}

fn fs_view(in: VsOut) -> vec4<f32> {
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
// as `AlbedoParams::albedo_with_ramp` (the f64 reference in `planet_frame::look::shading`). `ramp_min` is the node's least ramp height
// (REND-030): the seabed-to-shore ramp is `max(shore_blend, ramp_min)` tall.
fn albedo_of(tab: array<vec4<f32>, 10>, t: f32, p: f32, cs: f32, h: f32, ramp_min: f32) -> vec3<f32> {
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
    let land_w = smoothstep(-max(tab[9].z, ramp_min), 0.0, h);
    return mix(tab[7].xyz, shore, land_w);
}

// The ramp height at a fragment `d` metres from the camera (REND-030, `planet_frame::lod::shore_ramp_at`): `per_m * d` capped at 8 m. It
// depends on the distance only, so a ground point has one ramp whichever level of tile is drawn there.
fn shore_ramp_at(per_m: f32, d: f32) -> f32 {
    return min(per_m * d, 8.0);
}

// The fragment's distance from the camera: the node's camera-relative origin plus the fragment's tile-local position (camera-relative f32,
// no planet-radius value, CON-11).
fn camera_distance(node_origin_rel: vec4<f32>, local: vec3<f32>) -> f32 {
    return length(node_origin_rel.xyz + local);
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
fn terrain_radiance(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, ramp_min: f32, sun: vec3<f32>) -> vec3<f32> {
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
    let a = albedo_of(tab, t, p, cs, h, ramp_min);
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
fn shade_lit(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, ramp_min: f32, sun: vec3<f32>, factor: f32) -> vec3<f32> {
    return clamp(terrain_radiance(view, tab, normal, up, t, p, h, ramp_min, sun) * factor, vec3<f32>(0.0), vec3<f32>(60000.0));
}

// The sun direction and the pre-exposure factor travel in the otherwise unused words of the globals (mode.yzw as bits, range.z), the
// view in mode.x and the water level in extra.x. The up vector is formed from the node's tile-local numbers and the fragment's
// tile-local position: no planet-radius value reaches this stage.
@fragment
fn fs_lit(in: VsOut) -> @location(0) vec4<f32> {
    let c = fs_lit_view(in);
    if (CUT_QUADRANTS && quadrant_cut(in)) {
        discard;
    }
    return c;
}

fn fs_lit_view(in: VsOut) -> vec4<f32> {
    let sun = vec3<f32>(bitcast<f32>(g.mode.y), bitcast<f32>(g.mode.z), bitcast<f32>(g.mode.w));
    let up = up_at(nodes[in.node_index].up, in.local);
    let climate = tile_climate(in);
    var rgb = shade_lit(g.mode.x, g.albedo, in.normal, up, climate.x, climate.y, in.height - g.extra.x, shore_ramp_at(g.extra.z, camera_distance(nodes[in.node_index].origin_rel, in.local)), sun, g.range.z);
    if (g.extra.y > 2.5) {
        rgb = region_tint(nodes[in.node_index].origin_rel.xyz + in.local, rgb, g.range.z);
    } else if (g.extra.y > 0.5) {
        rgb = overlay_wire(in, rgb, g.range.z, g.extra.y);
    }
    return vec4<f32>(rgb, 1.0);
}

// The region tint (VIEW-030): the lit colour mixed with the tint of the nearest region polygon the fragment lies in. Plain arithmetic on
// the fragment's position relative to the camera; no derivative, no planet-radius value (CON-11).
// The even-odd test of the point `q` (metres in the anchor's plane) against the `n` points of the polygon whose points start at entry
// `first` of the globals' region table: a point on the polygon's boundary is decided by the crossing rule alone (a debug tint).
fn region_inside(first: u32, n: u32, q: vec2<f32>) -> bool {
    var inside = false;
    var j = n - 1u;
    for (var i = 0u; i < n; i = i + 1u) {
        let vi = g.regions[first + i / 2u];
        let vj = g.regions[first + j / 2u];
        let pi = select(vi.xy, vi.zw, (i & 1u) == 1u);
        let pj = select(vj.xy, vj.zw, (j & 1u) == 1u);
        if ((pi.y > q.y) != (pj.y > q.y)) && (q.x < (pj.x - pi.x) * (q.y - pi.y) / (pj.y - pi.y) + pi.x) {
            inside = !inside;
        }
        j = i;
    }
    return inside;
}

// `rel` is the fragment's position relative to the camera (the node's origin plus the tile-local position), `factor` the pre-exposure.
fn region_tint(rel: vec3<f32>, rgb: vec3<f32>, factor: f32) -> vec3<f32> {
    let count = min(u32(g.regions[0].x + 0.5), 8u);
    for (var k = 0u; k < count; k = k + 1u) {
        let base = 1u + k * 37u;
        let h0 = g.regions[base];
        let h1 = g.regions[base + 1u];
        let h2 = g.regions[base + 2u];
        let h3 = g.regions[base + 3u];
        let h4 = g.regions[base + 4u];
        let d = rel - h0.xyz;
        if (abs(dot(d, h3.xyz)) > h4.y) {
            continue;
        }
        let q = vec2<f32>(dot(d, h1.xyz), dot(d, h2.xyz));
        if (dot(q, q) > h4.x) {
            continue;
        }
        if (region_inside(base + 5u, min(u32(h0.w + 0.5), 64u), q)) {
            return mix(rgb, vec3<f32>(h1.w, h2.w, h3.w) * (0.8 * factor), 0.45);
        }
    }
    return rgb;
}
"#;

/// Depth target format and test: reversed-Z `Depth32Float` with the clear value 0 (CON-12, ADR 0002), for the debug and lit paths alike.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub(crate) const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::Greater;

pub(crate) fn view_index(v: TerrainView) -> u32 {
    TerrainView::ALL.iter().position(|x| *x == v).expect("known view") as u32
}

/// The node records of a plan, `NODE_BYTES` each (at least one record's worth, zero-filled when the plan has no nodes), as the shader
/// indexes them with the instance index.
pub(crate) fn node_record_bytes(plan: &TerrainPlan) -> Vec<u8> {
    let mut nodes = vec![0u8; NODE_BYTES as usize * plan.nodes.len().max(1)];
    for (i, d) in plan.nodes.iter().enumerate() {
        let r = d.camera_relative_origin;
        let c = d.color.map(|v| f32::from(v) / 255.0);
        let mut floats = vec![
            r[0],
            r[1],
            r[2],
            // The quadrants of the tile the node leaves out, as bits (STRM-016): 0 for a node drawn whole, so its record is what it was.
            f32::from(!d.quadrants & 0b1111),
            c[0],
            c[1],
            c[2],
            c[3],
            d.morph_start_m,
            d.morph_end_m,
            if d.preview && d.transient_only {
                2.0
            } else if d.preview {
                1.0
            } else {
                0.0
            },
            f32::from(d.level),
        ];
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
    nodes
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
    /// Draws of the water pass (one per water node, per-draw submission).
    pub water_draws: u32,
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
pub(crate) const GLOBALS_BYTES: u64 = 96 + 16 + 16 * planet_frame::look::shading::ALBEDO_VEC4S as u64 + PREVIEW_BYTES + REGION_BYTES;

/// Entries (vec4s) of one polygon of the region table (VIEW-030): five of header, then [`planet_frame::regions::MAX_REGION_POINTS`] points
/// two to an entry.
const REGION_ENTRIES: usize = 5 + planet_frame::regions::MAX_REGION_POINTS / 2;

/// Bytes of the region table at the very end of the globals: a count entry, then [`planet_frame::regions::MAX_REGIONS`] polygons.
pub(crate) const REGION_BYTES: u64 = 16 * (1 + (REGION_ENTRIES * planet_frame::regions::MAX_REGIONS) as u64);

/// The region table of the globals (VIEW-030) for `list`: zeros (count 0) when the list is empty. The layout is the one `Globals::regions`
/// of the shader documents.
pub(crate) fn region_table(list: &planet_frame::regions::RegionList) -> Vec<u8> {
    let mut rows: Vec<u8> = Vec::with_capacity(REGION_BYTES as usize);
    let polys = &list.polys[..list.polys.len().min(planet_frame::regions::MAX_REGIONS)];
    rows.extend(f32_bytes(&[polys.len() as f32, 0.0, 0.0, 0.0]));
    for p in polys {
        let n = p.points.len().min(planet_frame::regions::MAX_REGION_POINTS);
        rows.extend(f32_bytes(&[p.anchor[0], p.anchor[1], p.anchor[2], n as f32]));
        rows.extend(f32_bytes(&[p.e1[0], p.e1[1], p.e1[2], p.tint[0]]));
        rows.extend(f32_bytes(&[p.e2[0], p.e2[1], p.e2[2], p.tint[1]]));
        rows.extend(f32_bytes(&[p.up[0], p.up[1], p.up[2], p.tint[2]]));
        rows.extend(f32_bytes(&[p.reach_sq, p.band_m, 0.0, 0.0]));
        let start = rows.len();
        for q in &p.points[..n] {
            rows.extend(f32_bytes(q));
        }
        rows.resize(start + 16 * (REGION_ENTRIES - 5), 0);
    }
    rows.resize(REGION_BYTES as usize, 0);
    rows
}

/// Bytes of the preview table at the end of the globals: [`planet_frame::preview::MAX_PREVIEW_STAMPS`] stamps of two vec4s.
pub(crate) const PREVIEW_BYTES: u64 = 32 * planet_frame::preview::MAX_PREVIEW_STAMPS as u64;

/// The preview table of the globals (REND-033) for `stamps` seen from `camera`, `radius_m` the body's: per stamp its anchor on the
/// reference sphere relative to the camera (f64 on the CPU, rounded once), its reach, amount, kind, sign and bound. The count is the
/// caller's to put in `extra.w`.
pub(crate) fn preview_table(stamps: &[planet_frame::preview::PreviewStamp], camera: planet_core::Vec3, radius_m: f64) -> Vec<u8> {
    let mut rows = Vec::with_capacity(PREVIEW_BYTES as usize);
    for s in stamps.iter().take(planet_frame::preview::MAX_PREVIEW_STAMPS) {
        let c = s.dir * radius_m - camera;
        let kind = s.kind_code();
        rows.extend(f32_bytes(&[c.x as f32, c.y as f32, c.z as f32, s.reach_m as f32]));
        rows.extend(f32_bytes(&[s.amount_m as f32, kind, f32::from(s.sign), s.max_delta_m as f32]));
    }
    rows.resize(PREVIEW_BYTES as usize, 0);
    rows
}

struct FrameSet {
    globals: wgpu::Buffer,
    nodes: wgpu::Buffer,
    args: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// One tile reference per node, and the group-1 bind group that holds them.
    refs: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    /// The water pass's per-node records and globals (bindings 2 and 3 of the group `bind`; written only by frames with water).
    water_nodes: wgpu::Buffer,
    water_globals: wgpu::Buffer,
}

/// A resident tile: its mesh key and the slot of the pool it sits in.
#[derive(Clone, Copy, Debug)]
struct Resident {
    key: u64,
    slot: SlotRef,
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
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

/// How a frame's tile writes reach the queue (REND-016, STRM-006): `PerSlot` is the reference (one `write_buffer` or
/// `write_texture` call per compact slot), `Batched` collects the frame's compact tiles and writes each
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

/// The buffers a frame's tile writes are staged in (REND-018): kept by the renderer and reused, so that after the first frames
/// of a size a batch allocates nothing on the frame thread (a heap shared with the generation threads is where a frame's time
/// would vary with their load). Every vector is cleared, never freed, between batches; [`TerrainRenderer::batch_scratch_capacity`]
/// is what tests watch.
#[derive(Default)]
struct BatchScratch {
    /// Whether a batch is being collected (`upload_tiles_at`).
    open: bool,
    /// The slots of the batch's compact tiles (GEN-025), one after the other, `tile_bytes` each.
    texels: Vec<u8>,
    /// Per compact tile in collection order: its block, its slot within the block and the offset of its slot bytes in `texels`.
    tiles: Vec<(usize, u32, usize)>,
    /// The slot writes sorted by place (the index of the write in `tiles` last, so that the last write of a place is the last
    /// of its equals).
    texel_order: Vec<(usize, u32, u32)>,
    /// The bytes of one run of adjacent slots.
    run: Vec<u8>,
}

impl BatchScratch {
    fn capacity(&self) -> usize {
        self.texels.capacity() + self.tiles.capacity() + self.texel_order.capacity() + self.run.capacity()
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
    /// GPU time of the water pass (the scene copy is outside it), when the frame drew water and timestamps are supported.
    pub water_gpu_ms: Option<f64>,
    /// The GPU time of every pass the frame ran (REND-028): terrain, water, the atmosphere's products and composite, the exposure
    /// passes and the tone map, the scene copy, and the frame's GPU span.
    pub passes: crate::gpu_timing::PassTimes,
}
/// Terrain renderer with persistent resources: pipelines, uniform buffers, the pool of compact blocks and render targets are
/// created once, so a frame only writes a few small buffers and records the draws. `execute_terrain_tiles` is the one-shot wrapper.
pub struct TerrainRenderer {
    /// The preview stamps of the edit in flight and the body's radius (REND-033), written into the globals of each frame.
    preview: (Vec<planet_frame::preview::PreviewStamp>, f64),
    /// How many of the first stamps of `preview` are the delta of an edit in flight; the rest are the transient preview (VIEW-036).
    preview_delta_len: usize,
    /// The region polygons of the overlay (VIEW-030), written into the globals of the frames that show it.
    regions: planet_frame::regions::RegionList,
    size: (u32, u32),
    cells: u32,
    bgl: wgpu::BindGroupLayout,
    /// The per-frame buffers, used round robin: `ring[frame % RING]`.
    ring: Vec<FrameSet>,
    node_capacity: usize,
    draw_mode: DrawMode,
    indices: wgpu::Buffer,
    index_count: u32,
    /// Terrain tiles live in pooled fixed-size slots (STRM-006, ADR 0033 point 3): a few large storage-buffer blocks, one slot per
    /// tile. `pool` is the configuration whose `allocated_bytes` bound what this renderer allocates.
    pool: PoolConfig,
    /// Placement of `upload_tile`: slots handed out lowest first, reused after the frames in flight retire.
    pools: SlotPools,
    /// Resident tiles: mesh key and slot.
    tiles: HashMap<planet_core::TileId, Resident>,
    /// The compact pipeline, the blocks of the pool (allocated on first use) and their bind group layouts.
    compact: CompactGpu,
    /// The tile each occupied slot holds.
    owners: HashMap<SlotRef, planet_core::TileId>,
    /// Most nodes the node record buffer may hold.
    max_nodes: usize,
    /// Frames rendered; the clock of the slot retirement.
    frame: u64,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    timing: Option<TimingResources>,
    /// Bytes of tile slots uploaded so far.
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
    /// The water pass (ADR 0041, ADR 0042 point 2): None until [`enable_water`](TerrainRenderer::enable_water).
    water: Option<WaterGpu>,
}

/// The lit path's terrain pipelines (HDR target, `fs_lit`) and its post chain.
struct LitGpu {
    path: LitPath,
    pipeline: wgpu::RenderPipeline,
    /// The same for the nodes drawn over some quadrants (STRM-016): the cut is on.
    pipeline_cut: wgpu::RenderPipeline,
    /// The terrain pipeline of the frames that have an atmosphere (`fs_lit_atmo`, the tables in group 3).
    atmosphere_pipeline: Option<wgpu::RenderPipeline>,
    atmosphere_pipeline_cut: Option<wgpu::RenderPipeline>,
}

/// The pipeline-overridable constants of the fragment stage of the pipelines made for nodes drawn over some quadrants.
const CUT_CONSTANTS: [(&str, f64); 1] = [("CUT_QUADRANTS", 1.0)];

fn fragment_options(cut: bool) -> wgpu::PipelineCompilationOptions<'static> {
    wgpu::PipelineCompilationOptions { constants: if cut { &CUT_CONSTANTS } else { &[] }, ..Default::default() }
}

/// How often the renderer created its long-lived GPU objects (REND-013, REND-016): after construction and
/// [`warm_up`](TerrainRenderer::warm_up) a script whose first frames fit the warmed blocks and ring leaves every counter where it
/// was, so no frame pays for a one-time initialisation. Demand growth (a second block, a plan beyond the ring) counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InitCounts {
    /// Render pipelines created (1 for the vertex format, 2 with the compact format).
    pub pipelines: u32,
    /// Pool allocations: the vertex pool, blocks of the vertex-format main pool, compact blocks.
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
    /// Bit 0: a lit frame was recorded; bit 1: a lit frame with the atmosphere was.
    lit: u8,
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
    /// Lit variants recorded: a lit frame and a lit frame with the atmosphere each count once ([`warm_up_lit`](TerrainRenderer::warm_up_lit)).
    pub lit_variants: u32,
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
    /// [`TerrainRenderer::set_preview`] for the preview of a frame's draw plan (REND-033): call before submitting it.
    pub fn set_preview_of(&mut self, draw: &planet_frame::pipeline::DrawPlan) {
        self.set_preview(&draw.preview, draw.preview_radius_m);
        self.preview_delta_len = draw.preview_delta_len.min(self.preview.0.len());
    }

    /// Set the preview stamps drawn on the nodes flagged `preview` (REND-033; `DrawPlan::preview`) and the radius of the body they lie on. An
    /// empty list ends the preview: the vertex stage is then not entered and the frame is the frame of a build without it.
    pub fn set_preview(&mut self, stamps: &[planet_frame::preview::PreviewStamp], radius_m: f64) {
        self.preview = (stamps.iter().copied().take(planet_frame::preview::MAX_PREVIEW_STAMPS).collect(), radius_m);
        // All of them are the delta's (no node is transient-only without a draw plan that says so).
        self.preview_delta_len = self.preview.0.len();
    }

    /// Set the region polygons the regions overlay tints the terrain with (VIEW-030; `planet_frame::regions::region_list`, decided by the
    /// app for this frame's camera). They are read only by a frame whose lit plan has the overlay `Regions`.
    pub fn set_regions(&mut self, regions: &planet_frame::regions::RegionList) {
        self.regions = regions.clone();
    }

    /// A renderer with the mid tier's terrain share (ADR 0033: 2 GiB, a ceiling counted in slots: 15 blocks, 219,585 slots at cells
    /// 32). Slots are reusable one frame after their release (the renderer finishes a frame before it returns).
    pub fn new(ctx: &GpuContext, size: (u32, u32), cells: u32) -> Self {
        Self::with_frames_in_flight(ctx, size, cells, 1)
    }

    /// [`new`](TerrainRenderer::new) for a loop that keeps up to `frames_in_flight` frames in flight (STRM-016): a released slot is
    /// reusable that many frames later, so no frame that may still read it is running.
    pub fn with_frames_in_flight(ctx: &GpuContext, size: (u32, u32), cells: u32, frames_in_flight: u64) -> Self {
        let pool = planet_frame::pool::tier_compact_pool(Tier::Mid, cells, frames_in_flight)
            .unwrap_or_else(|e| panic!("terrain pools for cells {cells}: {e}"));
        Self::with_pools(ctx, size, cells, pool)
    }

    /// A renderer whose pool follows `pool` (a tier's share, or a small one for tests): storage-buffer blocks of compact slots (header,
    /// climate block and texels, GEN-025; REND-016, ADR 0033). `pool` comes from `planet_frame::pool::compact_format_pool` (or a
    /// small one for tests): its slot is the slot of one tile and its block is one storage buffer.
    pub fn with_pools(ctx: &GpuContext, size: (u32, u32), cells: u32, pool: PoolConfig) -> Self {
        assert_eq!(
            pool.slot_bytes,
            planet_generators::compact::slot_bytes(cells),
            "the pool slot is not the compact slot of {cells} cells"
        );
        Self::build(ctx, size, cells, pool)
    }

    fn build(ctx: &GpuContext, size: (u32, u32), cells: u32, pool: PoolConfig) -> Self {
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
        // The atmosphere's composite reads the depth (TEXTURE_BINDING); the debug path still discards it.
        let depth = target("terrain-depth", DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));
        // The share's node-record bytes bound the ring: per node a record, an argument record and a tile reference in each of the
        // RING sets.
        let per_node = NODE_BYTES + DRAW_RECORD_BYTES + REF_BYTES;
        let max_nodes = (pool.node_record_bytes.saturating_sub(RING as u64 * GLOBALS_BYTES) / (RING as u64 * per_node)) as usize;
        let node_capacity = 256.min(max_nodes.max(1));
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain-bgl"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(3, wgpu::BufferBindingType::Uniform),
            ],
        });
        let compact_gpu = Self::make_compact(ctx, &bgl, format, cells, &pool);
        let ring = Self::make_ring(device, &bgl, node_capacity, &compact_gpu);
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
                count: crate::gpu_timing::PASS_QUERIES,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-resolve"),
                size: crate::gpu_timing::PASS_QUERY_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-readback"),
                size: crate::gpu_timing::PASS_QUERY_BYTES,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        });
        Self {
            preview: (Vec::new(), 0.0),
            preview_delta_len: 0,
            regions: planet_frame::regions::RegionList::default(),
            size,
            cells,
            bgl,
            ring,
            node_capacity,
            draw_mode: ctx.default_draw_mode(),
            indices,
            index_count: index_data.len() as u32,
            pools: pool.slot_pools(),
            tiles: HashMap::new(),
            compact: compact_gpu,
            owners: HashMap::new(),
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
            init: InitCounts { pipelines: 1, pool_allocations: 0, ring_builds: 1 },
            used: UseMasks::default(),
            lit: None,
            lit_objects: 0,
            water: None,
        }
    }

    /// The compact pipeline and (empty) block table.
    fn make_compact(
        ctx: &GpuContext,
        bgl0: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        cells: u32,
        pool: &PoolConfig,
    ) -> CompactGpu {
        let device = &ctx.device;
        assert!(
            pool.block_slots > 0 && pool.slot_bytes * u64::from(pool.block_slots) <= planet_frame::pool::COMPACT_BLOCK_BYTES,
            "a block of {} slots of {} B is over the {} B a storage binding holds",
            pool.block_slots,
            pool.slot_bytes,
            planet_frame::pool::COMPACT_BLOCK_BYTES
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-compact"),
            source: wgpu::ShaderSource::Wgsl(compact_terrain_shader(cells).into()),
        });
        // The tile references and the block are read by the fragment stage too (the climate block of the slot, ADR 0039, GEN-025).
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl_frame =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("compact-frame-bgl"), entries: &[storage(0)] });
        let bgl_block =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("compact-block-bgl"), entries: &[storage(0)] });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compact-layout"),
            bind_group_layouts: &[Some(bgl0), Some(&bgl_frame), Some(&bgl_block)],
            immediate_size: 0,
        });
        // The pipeline of the whole nodes, and the one of the nodes drawn over some quadrants (STRM-016): the same shader with the cut on.
        let make = |cut: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if cut { "compact-pipeline-cut" } else { "compact-pipeline" }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: fragment_options(cut),
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
            })
        };
        let (pipeline, pipeline_cut) = (make(false), make(true));
        CompactGpu {
            pipeline,
            pipeline_cut,
            bgl_frame,
            bgl_block,
            blocks: Vec::new(),
            block_bytes: pool.slot_bytes * u64::from(pool.block_slots),
        }
    }

    /// The ring of per-frame buffers for plans of up to `capacity` nodes.
    fn make_ring(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, capacity: usize, compact: &CompactGpu) -> Vec<FrameSet> {
        (0..RING)
            .map(|_| {
                let buffer = |label, size, usage| {
                    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
                };
                let globals = buffer("globals", GLOBALS_BYTES, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
                let nodes = buffer("nodes", NODE_BYTES * capacity as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
                let args =
                    buffer("draw-args", DRAW_RECORD_BYTES * capacity as u64, wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST);
                let water_nodes = buffer(
                    "water-nodes",
                    planet_frame::look::water::NODE_RECORD_BYTES * capacity as u64,
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                );
                let water_globals = buffer(
                    "water-globals",
                    planet_frame::look::water::GLOBALS_BYTES,
                    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                );
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("terrain-bind"),
                    layout: bgl,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: nodes.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: water_nodes.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 3, resource: water_globals.as_entire_binding() },
                    ],
                });
                let refs = buffer("tile-refs", REF_BYTES * capacity as u64, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
                let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("compact-frame-bind"),
                    layout: &compact.bgl_frame,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: refs.as_entire_binding() }],
                });
                FrameSet { globals, nodes, args, bind, refs, frame_bind, water_nodes, water_globals }
            })
            .collect()
    }

    /// Enable the lit path (ADR 0042 point 1): create the `Rgba16Float` scene target, the exposure buffers and compute pipelines,
    /// the tone-map pipeline and the terrain pipeline that writes the scene target. Call it before the frames that are measured
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
        let c = &self.compact;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-compact-lit"),
            source: wgpu::ShaderSource::Wgsl(compact_terrain_shader(self.cells).into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compact-lit-layout"),
            bind_group_layouts: &[Some(&self.bgl), Some(&c.bgl_frame), Some(&c.bgl_block)],
            immediate_size: 0,
        });
        let make = |cut: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if cut { "compact-lit-pipeline-cut" } else { "compact-lit-pipeline" }),
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
                    compilation_options: fragment_options(cut),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let (pipeline, pipeline_cut) = (make(false), make(true));
        let path = LitPath::new(ctx, self.size);
        // The scene target, the exposure buffers, three post pipelines and the terrain pipelines (whole nodes, nodes over some quadrants).
        self.lit_objects = 1 + 3 + 2;
        self.lit = Some(LitGpu { path, pipeline, pipeline_cut, atmosphere_pipeline: None, atmosphere_pipeline_cut: None });
    }

    /// Enable the atmosphere (ADR 0040, ADR 0042 point 2) for the body `atmosphere`: its static tables are computed on the CPU in f64
    /// and uploaded, the products, the scene target, the composite and the terrain's atmosphere pipeline are created. Needs
    /// [`enable_lit`](TerrainRenderer::enable_lit); call it before the measured frames (REND-017). A plan whose atmosphere differs
    /// from this body's uploads the new body's tables on its first frame.
    pub fn enable_atmosphere(&mut self, ctx: &GpuContext, atmosphere: &planet_frame::look::atmosphere::Atmosphere) {
        let lit = self.lit.as_mut().expect("enable_lit before enable_atmosphere");
        if lit.path.atmosphere().is_some() {
            return;
        }
        lit.path.enable_atmosphere(ctx, &self.depth_view, atmosphere);
        let device = &ctx.device;
        let atmo_bgl = lit.path.atmosphere().expect("just enabled").terrain_layout();
        let depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(DEPTH_COMPARE),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let targets = [Some(wgpu::ColorTargetState { format: crate::lit::HDR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        // The atmosphere's group is 3 (groups 1 and 2 are the tile references and the block), so one bind group set once per pass stays
        // bound for the whole pass.
        let c = &self.compact;
        let source = format!("{}{}", compact_terrain_shader(self.cells), crate::atmosphere::terrain_atmosphere_source());
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-compact-lit-atmosphere"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compact-lit-atmosphere-layout"),
            bind_group_layouts: &[Some(&self.bgl), Some(&c.bgl_frame), Some(&c.bgl_block), Some(atmo_bgl)],
            immediate_size: 0,
        });
        let make = |cut: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if cut { "compact-lit-atmosphere-pipeline-cut" } else { "compact-lit-atmosphere-pipeline" }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_lit_atmo"),
                    compilation_options: fragment_options(cut),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth.clone()),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let (pipeline, pipeline_cut) = (make(false), make(true));
        // Counted by declaration: the atmosphere path's scene target, two tables, two products, the uniform buffer and three
        // pipelines (sky view, aerial perspective, composite) beside the two terrain pipelines made here.
        const ATMOSPHERE_PATH_OBJECTS: u32 = 9;
        self.lit_objects += 2 + ATMOSPHERE_PATH_OBJECTS - 3;
        lit.atmosphere_pipeline = Some(pipeline);
        lit.atmosphere_pipeline_cut = Some(pipeline_cut);
        if let Some(w) = self.water.as_mut() {
            let layouts = CompactLayouts { frame: &self.compact.bgl_frame, block: &self.compact.bgl_block, cells: self.cells };
            let before = w.objects;
            w.add_atmosphere(device, &self.bgl, &layouts, lit.path.atmosphere().expect("just enabled"));
            self.lit_objects += w.objects - before;
        }
    }

    /// Enable the water pass (ADR 0041 points 4 to 6, ADR 0042 point 2): the copy of the scene target the water reads the seabed
    /// from, the group-3 bind group and the water pipeline (and, when the atmosphere is enabled, the one of its lighting). Needs
    /// [`enable_lit`](TerrainRenderer::enable_lit); call it before the measured frames (REND-017). A plan with a water part on a
    /// renderer without it is refused naming this call.
    pub fn enable_water(&mut self, ctx: &GpuContext) {
        assert!(self.lit.is_some(), "enable_lit before enable_water");
        if self.water.is_some() {
            return;
        }
        let device = &ctx.device;
        let layouts = CompactLayouts { frame: &self.compact.bgl_frame, block: &self.compact.bgl_block, cells: self.cells };
        let mut water = WaterGpu::new(device, self.size, &self.bgl, &layouts);
        if let Some(a) = self.lit.as_ref().and_then(|l| l.path.atmosphere()) {
            water.add_atmosphere(device, &self.bgl, &layouts, a);
        }
        self.lit_objects += water.objects;
        self.water = Some(water);
    }

    /// Whether the water pass is enabled.
    pub fn water_enabled(&self) -> bool {
        self.water.is_some()
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
        self.lit.as_ref().map(|l| {
            l.path.allocated_bytes()
                + u64::from(self.size.0) * u64::from(self.size.1) * crate::graph::DEPTH_BYTES_PER_PIXEL
                + self.water.as_ref().map_or(0, |w| w.copy_bytes)
        })
    }

    /// Long-lived GPU objects created so far (REND-013): the counters a test compares before and after a frame.
    pub fn init_counts(&self) -> InitCounts {
        self.init
    }

    /// Do the one-time work of the first frames now, outside any measured frame (REND-013, TEST-018): allocate the vertex pool and
    /// the first block of the main pool (block 0 of either format, with its bind group), grow the per-frame ring to [`WARM_NODES`]
    /// (within the share's node-record bytes), and submit one throwaway frame (every ring set, the timed pass where supported, a
    /// two-record multi-draw) that draws a zeroed tile through every pipeline
    /// the format has in the active draw mode (call `set_draw_mode` first), polled to completion, so the driver's lazy work (first-use initialisation of the
    /// buffers, pipeline state, shader JIT) is done before frame 0. No tile becomes resident and no counter of the
    /// frame clock moves. The bytes allocated are the pure model's for one block plus the vertex pool (STRM-006); the rest of the
    /// share stays on demand.
    pub fn warm_up(&mut self, ctx: &GpuContext) {
        let device = &ctx.device;
        let warm_nodes = WARM_NODES.min(self.max_nodes);
        if warm_nodes > self.node_capacity {
            self.node_capacity = warm_nodes;
            self.ring = Self::make_ring(device, &self.bgl, self.node_capacity, &self.compact);
            self.init.ring_builds += 1;
        }
        self.compact.block(device, 0, &mut self.init);
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
            ctx.queue.write_buffer(&set.refs, 0, &[0u8; 2 * REF_BYTES as usize]);
        }
        let timing = self.timing.as_ref();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain-warm-up") });
        // Mark the pool initialised on the GPU (wgpu zeroes a buffer lazily before its first use).
        if let Some(Some(block)) = self.compact.blocks.first() {
            encoder.clear_buffer(&block.buffer, 0, None);
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
            pass.set_pipeline(&self.compact.pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(0, &set.bind, &[]);
            pass.set_bind_group(1, &set.frame_bind, &[]);
            pass.set_bind_group(2, &self.compact.blocks[0].as_ref().expect("block 0").bind, &[]);
            draw(&mut pass);
            self.used.ring |= 1 << k;
        }
        self.used.pipelines = 0b01;
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

    /// Do the one-time work of the first lit frames now, outside any measured frame (REND-017): enable the lit path and, when the
    /// plan carries an atmosphere, the atmosphere (`enable_lit` is not part of [`warm_up`](TerrainRenderer::warm_up)), then draw
    /// `plan` (the lit frame the caller will draw first, with no node) once for every set of the per-frame ring, each waited to
    /// completion, so the compute and post pipelines, the graph compile, the table upload and the bind groups have been used
    /// before frame 0. No tile becomes resident. Call [`warm_up`](TerrainRenderer::warm_up) first.
    pub fn warm_up_lit(&mut self, ctx: &GpuContext, plan: &TerrainPlan) -> Result<(), ExecError> {
        let lit = plan.lit.as_ref().ok_or(ExecError::LitNotEnabled)?;
        self.enable_lit(ctx);
        if let Some(a) = &lit.atmosphere {
            self.enable_atmosphere(ctx, &a.atmosphere);
        }
        if plan.water.is_some() {
            self.enable_water(ctx);
        }
        // Like the debug warm-up, these frames do not count: the frame clock of the slot retirement and the retired total (which a
        // pipelined loop compares with its own submissions) are put back.
        let (frame, retired) = (self.frame, self.retired.load(std::sync::atomic::Ordering::SeqCst));
        for _ in 0..RING_SETS {
            self.render(ctx, plan, false)?;
            self.used.waited += 1;
        }
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
        self.frame = frame;
        self.retired.store(retired, std::sync::atomic::Ordering::SeqCst);
        self.in_flight.clear();
        Ok(())
    }

    /// What the submissions so far have used (REND-017): the counters a test compares after the warm-up and after frames.
    pub fn first_use(&self) -> FirstUse {
        FirstUse {
            ring_sets: self.used.ring.count_ones(),
            pipelines: self.used.pipelines.count_ones(),
            timestamp_passes: u32::from(self.used.timestamps),
            draw_modes: self.used.modes.count_ones(),
            lit_variants: self.used.lit.count_ones(),
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

    /// Upload a compact tile once; later calls for the same tile do nothing. The renderer places the tile itself: the lowest free
    /// slot of the pool. A tile that comes back with another mesh key takes a new slot and its old one is released (reusable after
    /// the frames in flight retire).
    pub fn upload_tile(&mut self, ctx: &GpuContext, tile: &CompactTile) -> Result<(), ExecError> {
        self.place_tile(ctx, tile, None)
    }

    /// Upload a tile into the slot the residency manager chose (`Streamer::slot_of`): the renderer executes the placement and decides
    /// nothing. Whatever tile the slot held before is forgotten, and so is the old slot of this tile. Do not mix with
    /// [`upload_tile`](TerrainRenderer::upload_tile) on one renderer: that one hands out slots itself.
    pub fn upload_tile_at(&mut self, ctx: &GpuContext, tile: &CompactTile, slot: SlotRef) -> Result<(), ExecError> {
        self.place_tile(ctx, tile, Some(slot))
    }

    fn place_tile(&mut self, ctx: &GpuContext, tile: &CompactTile, at: Option<SlotRef>) -> Result<(), ExecError> {
        let (id, key, cells) = (tile.id, tile.key, tile.cells);
        if cells != self.cells {
            return Err(ExecError::BadResolution { id, res: cells });
        }
        // Resident with the same key: nothing to do for a tile the renderer places itself. A different key (other seed, radius,
        // mesh version) replaces it.
        if at.is_none() && self.tiles.get(&id).is_some_and(|r| r.key == key) {
            return Ok(());
        }
        if id.level() < planet_generators::compact::COMPACT_MIN_LEVEL {
            return Err(ExecError::CompactUnsupported { tile: id });
        }
        let slot = match at {
            Some(slot) => slot,
            None => self.pools.acquire(PoolKind::Main, self.frame).ok_or(ExecError::PoolFull { tile: id, pool: PoolKind::Main })?,
        };
        if at.is_some() {
            let capacity = self.pool.main_slots();
            if slot.index >= capacity {
                return Err(ExecError::BadSlot { tile: id, slot, capacity });
            }
        }
        // The old residency of this tile: a slot the renderer handed out is released (reusable after the frames in flight retire),
        // a slot the manager chose is the manager's.
        let old = self.tiles.get(&id).copied();
        self.write_compact_tile(ctx, tile, slot);
        if let (Some(old), true) = (old, at.is_none()) {
            self.pools.release(old.slot, self.frame);
        }
        if let Some(previous) = self.owners.insert(slot, id) {
            if previous != id {
                self.tiles.remove(&previous);
            }
        }
        if let Some(old) = self.tiles.insert(id, Resident { key, slot }) {
            if old.slot != slot {
                self.owners.remove(&old.slot);
            }
        }
        Ok(())
    }

    fn write_compact_tile(&mut self, ctx: &GpuContext, tile: &CompactTile, slot: SlotRef) {
        let block_slots = self.pool.block_slots;
        let c = &mut self.compact;
        let (block, local) = ((slot.index / block_slots) as usize, slot.index % block_slots);
        let block_buffer = &c.block(&ctx.device, block, &mut self.init).buffer;
        let n = tile.resident_bytes();
        debug_assert_eq!(n, self.pool.slot_bytes, "a compact tile fills its slot exactly");
        self.uploaded_bytes += n;
        if self.scratch.open {
            // Held back: converted straight into the batch's slot buffer, written by the flush.
            let at = self.scratch.texels.len();
            tile.write_slot_into(&mut self.scratch.texels);
            debug_assert_eq!(&self.scratch.texels[at..], &tile.slot_data()[..]);
            self.scratch.tiles.push((block, local, at));
            return;
        }
        // One slot, one write (REND-016): header, climate block and texels are one contiguous range of the block.
        let mut bytes = Vec::with_capacity(n as usize);
        tile.write_slot_into(&mut bytes);
        debug_assert_eq!(bytes, tile.slot_data());
        self.upload_counters.write_calls += 1;
        self.upload_counters.staging_bytes += bytes.len() as u64;
        ctx.queue.write_buffer(block_buffer, u64::from(local) * n, &bytes);
    }

    /// How tile writes reach the queue (default [`UploadMode::Batched`]); the results are the same in both modes.
    pub fn set_upload_mode(&mut self, mode: UploadMode) {
        self.upload_mode = mode;
    }

    pub fn upload_mode(&self) -> UploadMode {
        self.upload_mode
    }

    /// The bytes of the compact pools as the GPU holds them (REND-016): every allocated block (`block_slots` slots of GEN-025, slot
    /// after slot, `None` for a block not yet allocated). A test aid: it submits copies and waits. Empty for a renderer without the
    /// compact format.
    pub fn read_compact_pools(&self, ctx: &GpuContext) -> Vec<Option<Vec<u8>>> {
        let c = &self.compact;
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
        c.blocks.iter().map(|b| b.as_ref().map(|b| read_buffer(&b.buffer, b.buffer.size()))).collect()
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
    /// twice keeps the last tile, and each run of adjacent slots of one block is one call (header, climate block and texels, GEN-025),
    /// so a frame makes one call per run instead of one per slot. Vertex-format tiles are written at
    /// once, one call each (their 53,724 bytes dominate the call). The bookkeeping of every tile is as with single uploads. On an
    /// error the tiles placed before it are written and the error returned.
    pub fn upload_tiles_at(&mut self, ctx: &GpuContext, tiles: &[(&CompactTile, SlotRef)]) -> Result<(), ExecError> {
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
        let tile_bytes = planet_generators::compact::slot_bytes(self.cells) as usize;
        sc.texel_order.clear();
        sc.texel_order.extend(sc.tiles.iter().enumerate().map(|(i, &(block, local, _))| (block, local, i as u32)));
        sc.texel_order.sort_unstable();
        // Of several writes to one place the last (the greatest index, sorted last) is kept.
        let places = dedup_places(&mut sc.texel_order, |o| (o.0, o.1));
        // Compact slots: runs of adjacent slots of one block, one contiguous range of the buffer.
        if places > 0 {
            let mut k = 0;
            while k < places {
                let (block, first, _) = sc.texel_order[k];
                let mut n = 1;
                while k + n < places && (sc.texel_order[k + n].0, sc.texel_order[k + n].1) == (block, first + n as u32) {
                    n += 1;
                }
                sc.run.clear();
                sc.run.resize(n * tile_bytes, 0);
                for t in 0..n {
                    let at = sc.tiles[sc.texel_order[k + t].2 as usize].2;
                    let src = &sc.texels[at..at + tile_bytes];
                    sc.run[t * tile_bytes..(t + 1) * tile_bytes].copy_from_slice(src);
                }
                let block_buffer = &self.compact.blocks[block].as_ref().expect("block allocated").buffer;
                let trw = std::time::Instant::now();
                ctx.queue.write_buffer(block_buffer, u64::from(first) * tile_bytes as u64, &sc.run);
                self.upload_times.queue += trw.elapsed();
                self.upload_counters.write_calls += 1;
                self.upload_counters.staging_bytes += sc.run.len() as u64;
                k += n;
            }
        }
        sc.texels.clear();
        sc.tiles.clear();
    }

    /// Release the slot `upload_tile` gave to `tile` (reusable after the frames in flight retire) and forget the tile.
    pub fn evict(&mut self, tile: planet_core::TileId) -> bool {
        let Some(r) = self.tiles.remove(&tile) else { return false };
        self.owners.remove(&r.slot);
        self.pools.release(r.slot, self.frame);
        true
    }

    /// Forget `tile` without touching the pool: for a caller that places tiles with [`upload_tile_at`](TerrainRenderer::upload_tile_at).
    pub fn forget(&mut self, tile: planet_core::TileId) -> bool {
        let Some(r) = self.tiles.remove(&tile) else { return false };
        self.owners.remove(&r.slot);
        true
    }

    /// The slot a resident tile occupies.
    pub fn slot_of(&self, tile: planet_core::TileId) -> Option<SlotRef> {
        self.tiles.get(&tile).map(|r| r.slot)
    }

    /// The pool configuration whose `allocated_bytes` bound this renderer's allocations.
    pub fn pool_config(&self) -> &PoolConfig {
        &self.pool
    }

    /// Slots handed out by `upload_tile`, retiring and free in the pool: `(in use, retiring, free)`.
    pub fn main_slot_counts(&self) -> (u32, u32, u32) {
        (self.pools.main.in_use(), self.pools.main.retiring(), self.pools.main.free())
    }

    /// Bytes of the tile pool (the compact blocks), the index buffer and the node record buffers as allocated now (summed from the
    /// buffers' real sizes). The pool and the index buffer equal what `PoolConfig::allocated_bytes_with` says for the live blocks;
    /// the node records are as large as the largest plan so far, at most the configured buffer; the whole stays under the terrain
    /// share (STRM-006).
    pub fn pool_allocated_bytes(&self) -> u64 {
        let blocks: u64 = self.compact.blocks.iter().flatten().map(|b| b.buffer.size()).sum();
        blocks + self.indices.size() + self.node_buffer_bytes()
    }

    /// Bytes of the per-frame buffers (the ring of globals, node records, draw arguments and tile references) as allocated now (it grows with the largest plan, up to the configured maximum).
    pub fn node_buffer_bytes(&self) -> u64 {
        self.ring.iter().map(|s| s.globals.size() + s.nodes.size() + s.args.size() + s.refs.size()).sum()
    }

    /// Blocks of the pool allocated so far.
    pub fn live_blocks(&self) -> u32 {
        self.compact.blocks.iter().flatten().count() as u32
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
        if plan.lit.as_ref().is_some_and(|l| l.atmosphere.is_some()) && self.lit.as_ref().is_some_and(|l| l.path.atmosphere().is_none()) {
            return Err(ExecError::AtmosphereNotEnabled);
        }
        if let Some(w) = &plan.water {
            if plan.lit.is_none() {
                return Err(ExecError::BadWaterPlan(
                    "the plan carries water but no lit part: the water pass belongs to the lit path".into(),
                ));
            }
            w.validate().map_err(ExecError::BadWaterPlan)?;
            if self.water.is_none() {
                return Err(ExecError::WaterNotEnabled);
            }
            if let Some(n) = w.nodes.iter().find(|n| n.node as usize >= plan.nodes.len()) {
                return Err(ExecError::BadWaterPlan(format!(
                    "water node {} is not a node of the plan ({} nodes)",
                    n.node,
                    plan.nodes.len()
                )));
            }
        }
        // A frame draws the water pass only when some node carries water.
        let water_frame = plan.lit.is_some() && plan.water.as_ref().is_some_and(|w| !w.nodes.is_empty());
        self.frame += 1;
        let device = &ctx.device;
        if plan.nodes.len() > self.node_capacity {
            self.node_capacity = plan.nodes.len().next_power_of_two().min(self.max_nodes);
            // Frames in flight keep the buffers they use alive; the new ring serves the frames from here on.
            self.ring = Self::make_ring(device, &self.bgl, self.node_capacity, &self.compact);
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
                globals.extend(f32_bytes(&[lit.water_level_m, lit.overlay.shader_code(), plan.shore_ramp_per_m, 0.0]));
                globals.extend(f32_bytes(&lit.albedo.to_words()));
            }
            None => {
                globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
                globals.extend([view_index(plan.view), 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
                globals.extend(vec![0u8; GLOBALS_BYTES as usize - PREVIEW_BYTES as usize - REGION_BYTES as usize - globals.len()]);
            }
        }
        // The preview table after the albedo table, its count in extra.w (REND-033); zeros when there is no preview.
        globals.resize(GLOBALS_BYTES as usize - PREVIEW_BYTES as usize - REGION_BYTES as usize, 0);
        if self.preview.0.is_empty() {
            globals.extend(vec![0u8; PREVIEW_BYTES as usize]);
        } else {
            globals.extend(preview_table(&self.preview.0, plan.camera.position.0, self.preview.1));
            // extra.w packs the active count (low 6 bits) and the number of the delta's stamps above it: a transient-only node starts after them.
            let packed = self.preview.0.len() + 64 * self.preview_delta_len;
            globals[108..112].copy_from_slice(&(packed as f32).to_le_bytes());
        }
        // The region table at the end, read by the fragment stage of the regions overlay only (VIEW-030): zeros unless the plan shows it.
        let show_regions = plan.lit.is_some_and(|l| l.overlay == planet_frame::look::Overlay::Regions);
        if show_regions {
            globals.extend(region_table(&self.regions));
        } else {
            globals.extend(vec![0u8; REGION_BYTES as usize]);
        }
        debug_assert_eq!(globals.len() as u64, GLOBALS_BYTES);
        // Each frame writes its own set of the ring, so a frame still in flight keeps the records it reads.
        let ring_index = (self.frame % RING as u64) as usize;
        self.used.ring |= 1 << ring_index;
        self.used.modes |= if self.draw_mode == DrawMode::Indirect { 0b10 } else { 0b01 };
        if let Some(l) = &plan.lit {
            self.used.lit |= if l.atmosphere.is_some() { 0b11 } else { 0b01 };
        }
        if !plan.nodes.is_empty() {
            self.used.pipelines |= 0b01;
        }
        let set = &self.ring[ring_index];
        ctx.queue.write_buffer(&set.globals, 0, &globals);
        if let (Some(w), true) = (&plan.water, water_frame) {
            ctx.queue.write_buffer(&set.water_globals, 0, &crate::water::globals_bytes(w));
            ctx.queue.write_buffer(&set.water_nodes, 0, &crate::water::node_records(w, plan.nodes.len()));
        }
        let nodes = node_record_bytes(plan);
        ctx.queue.write_buffer(&set.nodes, 0, &nodes);
        let mut stats = TerrainStats { node_record_bytes: nodes.len() as u64, ..Default::default() };
        // The argument records come from the plan and the slots alone (CON-03: the GPU culls and decides nothing).
        let block_of = |r: &Resident| buffer_of(r.slot, self.pool.block_slots);
        {
            // One tile reference per node (the first word of its slot in its block and the slot index); unused entries stay zero.
            let slot_words = (planet_generators::compact::slot_bytes(self.cells) / 4) as u32;
            let mut bytes = vec![0u8; REF_BYTES as usize * plan.nodes.len().max(1)];
            for (i, d) in plan.nodes.iter().enumerate() {
                let r = &self.tiles[&d.tile];
                let local = r.slot.index % self.pool.block_slots;
                let words = [local * slot_words, 0, r.slot.index, 0u32];
                for (k, v) in words.iter().enumerate() {
                    bytes[i * REF_BYTES as usize + 4 * k..i * REF_BYTES as usize + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            ctx.queue.write_buffer(&set.refs, 0, &bytes);
            stats.tile_ref_bytes = bytes.len() as u64;
        }
        let indirect = (self.draw_mode == DrawMode::Indirect)
            .then(|| build_draw_list_placed(plan.nodes.iter().map(|d| block_of(&self.tiles[&d.tile])), self.index_count));
        if let Some(list) = &indirect {
            ctx.queue.write_buffer(&set.args, 0, &list.record_bytes());
            stats.argument_bytes = list.argument_bytes();
        }

        let timing = if timed { self.timing.as_ref() } else { None };
        let stamps = timing.map(|t| crate::gpu_timing::Stamps::new(&t.queries));
        self.used.timestamps |= timing.is_some() && wait;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain") });
        // The lit path draws into its scene target with its own pipelines and clears to black without coverage; the debug path
        // is the plan's colour target as before.
        if let (Some(l), Some(lit)) = (self.lit.as_mut(), plan.lit.as_ref()) {
            // The compiled graph, the parameters and, with an atmosphere, the tables and the frame's block; then the passes that come
            // before the terrain (the sky-view table and the aerial-perspective volume).
            l.path.prepare(ctx, lit, &plan.camera, water_frame);
            l.path.record_pre(&mut encoder, stamps.as_ref());
        }
        let lit_gpu = plan.lit.as_ref().and(self.lit.as_ref());
        let atmosphere_frame = plan.lit.as_ref().is_some_and(|l| l.atmosphere.is_some());
        let (color_target, pipeline, pipeline_cut) = match (lit_gpu, plan.lit.as_ref()) {
            (Some(l), Some(lit)) if atmosphere_frame => (
                l.path.terrain_target(lit),
                l.atmosphere_pipeline.as_ref().expect("atmosphere enabled"),
                l.atmosphere_pipeline_cut.as_ref().expect("atmosphere enabled"),
            ),
            (Some(l), _) => (l.path.hdr_view(), &l.pipeline, &l.pipeline_cut),
            _ => (&self.color_view, &self.compact.pipeline, &self.compact.pipeline_cut),
        };
        // The nodes drawn over some quadrants (STRM-016) go in their own draws after the whole ones, through the pipeline with the cut on.
        let masked: Vec<bool> = plan.nodes.iter().map(|d| d.quadrants != planet_frame::ALL_QUADRANTS).collect();
        let any_masked = masked.iter().any(|m| *m);
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
                    // The composite and the water pass read the depth, so an atmosphere or water frame keeps it.
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: if atmosphere_frame || water_frame { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: crate::gpu_timing::render_writes(stamps.as_ref(), crate::gpu_timing::GpuPass::Terrain),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(0, &set.bind, &[]);
            pass.set_bind_group(1, &set.frame_bind, &[]);
            if let Some(a) = lit_gpu.filter(|_| atmosphere_frame).and_then(|l| l.path.atmosphere()) {
                pass.set_bind_group(3, a.terrain_bind(), &[]);
            }
            // The block bound to group 2: a node of another block rebinds it (the lowest-slot-first free list keeps a settled view in
            // one or two blocks).
            let mut bound: Option<u32> = None;
            // The whole nodes, then (if there are any) the nodes drawn over some quadrants with the cut pipeline: the layouts are the
            // same, so the bind groups stay bound across the switch.
            for class in [false, true] {
                if class {
                    if !any_masked {
                        break;
                    }
                    pass.set_pipeline(pipeline_cut);
                }
                match &indirect {
                    Some(list) => {
                        // One multi-draw per run of consecutive nodes of one class in one block; the node index is the first instance.
                        for run in &list.runs {
                            let b = run.buffer.0;
                            let (mut i, end) = (run.first as usize, (run.first + run.count) as usize);
                            while i < end {
                                if masked[i] != class {
                                    i += 1;
                                    continue;
                                }
                                let from = i;
                                while i < end && masked[i] == class {
                                    i += 1;
                                }
                                if bound != Some(b) {
                                    pass.set_bind_group(
                                        2,
                                        &self.compact.blocks[b as usize].as_ref().expect("block of a resident tile").bind,
                                        &[],
                                    );
                                    bound = Some(b);
                                }
                                pass.multi_draw_indexed_indirect(&set.args, from as u64 * DRAW_RECORD_BYTES, (i - from) as u32);
                                stats.encoded_draws += 1;
                            }
                        }
                    }
                    None => {
                        for (i, d) in plan.nodes.iter().enumerate().filter(|(i, _)| masked[*i] == class) {
                            let b = block_of(&self.tiles[&d.tile]).0;
                            if bound != Some(b) {
                                pass.set_bind_group(
                                    2,
                                    &self.compact.blocks[b as usize].as_ref().expect("block of a resident tile").bind,
                                    &[],
                                );
                                bound = Some(b);
                            }
                            pass.draw_indexed(0..self.index_count, 0, i as u32..i as u32 + 1);
                            stats.encoded_draws += 1;
                        }
                    }
                }
            }
            stats.draw_calls = plan.nodes.len() as u32;
            stats.triangles = plan.nodes.len() as u64 * u64::from(self.index_count) / 3;
        }
        if let (true, Some(w), Some(wp), Some(l), Some(lit)) =
            (water_frame, self.water.as_ref(), plan.water.as_ref(), lit_gpu, plan.lit.as_ref())
        {
            // The scene target is copied (the water reads the seabed through itself, and a pass cannot sample what it draws into),
            // then the terrain grid is drawn again at the water level over the same target and depth.
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: l.path.terrain_texture(lit),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &w.copy,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
            );
            let flavour = if atmosphere_frame { w.atmosphere.as_ref().expect("atmosphere water enabled") } else { &w.plain };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: if atmosphere_frame { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: crate::gpu_timing::render_writes(stamps.as_ref(), crate::gpu_timing::GpuPass::Water),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_pipeline(&flavour.compact);
            pass.set_bind_group(0, &set.bind, &[]);
            pass.set_bind_group(1, &set.frame_bind, &[]);
            pass.set_bind_group(3, &flavour.bind, &[]);
            let mut bound: Option<u32> = None;
            for n in &wp.nodes {
                let i = n.node as usize;
                let b = block_of(&self.tiles[&plan.nodes[i].tile]).0;
                if bound != Some(b) {
                    pass.set_bind_group(2, &self.compact.blocks[b as usize].as_ref().expect("block of a resident tile").bind, &[]);
                    bound = Some(b);
                }
                pass.draw_indexed(0..self.index_count, 0, i as u32..i as u32 + 1);
                stats.water_draws += 1;
            }
        }
        if let Some(l) = self.lit.as_ref().filter(|_| plan.lit.is_some()) {
            l.path.record_post(&mut encoder, &self.color_view, stamps.as_ref());
        }
        // Every pass that took timestamps resolves its own pair (an unwritten query is never resolved), then the used range is copied.
        let written = stamps.as_ref().map_or(0, |s| s.written());
        if let Some(t) = timing {
            let mut top = 0u64;
            for p in crate::gpu_timing::GpuPass::ALL.into_iter().filter(|p| written & (1 << *p as u16) != 0) {
                let q = p.first_query();
                let at = (p as u64) * crate::gpu_timing::RESOLVE_STRIDE;
                encoder.resolve_query_set(&t.queries, q..q + 2, &t.resolve, at);
                top = top.max(at + 16);
            }
            if top > 0 {
                encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, top);
            }
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
            return Ok((FrameTiming { cpu_ms, total_ms: cpu_ms, ..Default::default() }, stats));
        }
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let total_ms = started.elapsed().as_secs_f64() * 1000.0;
        let passes = timing
            .filter(|_| written != 0)
            .map(|t| {
                let slice = t.readback.slice(..);
                slice.map_async(wgpu::MapMode::Read, |r| r.expect("timestamp map failed"));
                ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
                let data = slice.get_mapped_range().expect("timestamp range");
                let tick = |k: usize| u64::from_le_bytes(data[k * 8..k * 8 + 8].try_into().expect("8 bytes"));
                let stride = (crate::gpu_timing::RESOLVE_STRIDE / 8) as usize;
                let ticks: Vec<u64> = (0..crate::gpu_timing::PASS_QUERIES as usize).map(|q| tick((q / 2) * stride + q % 2)).collect();
                // A zero or negative interval is a broken query, not a free frame: `from_ticks` reports nothing instead of 0 ms.
                let out = crate::gpu_timing::PassTimes::from_ticks(&ticks, written, f64::from(ctx.queue.get_timestamp_period()));
                drop(data);
                t.readback.unmap();
                out
            })
            .unwrap_or_default();
        let (gpu_ms, water_gpu_ms) = (passes.get(crate::gpu_timing::GpuPass::Terrain), passes.get(crate::gpu_timing::GpuPass::Water));
        self.reap();
        Ok((FrameTiming { cpu_ms, total_ms, gpu_ms, water_gpu_ms, passes }, stats))
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

/// The compact pools of a one-shot render of `nodes` nodes: blocks just large enough for the tiles (a block never over 128 MiB, so
/// a big plan spans several blocks).
fn one_shot_pool(nodes: usize, cells: u32) -> Result<PoolConfig, String> {
    let slot = planet_generators::compact::slot_bytes(cells);
    let nodes = u32::try_from(nodes.max(1)).map_err(|e| e.to_string())?;
    let block_slots = nodes.min(planet_frame::pool::block_slots_in(planet_frame::pool::COMPACT_BLOCK_BYTES, cells)).max(1);
    let blocks = u64::from(nodes.div_ceil(block_slots));
    let share = planet_streaming::budget::FIXED_RESERVE_BYTES + blocks * u64::from(block_slots) * slot;
    planet_frame::pool::compact_format_pool_with(share, cells, block_slots, 1)
}

/// Execute the plan on the GPU with the compact tiles of its nodes and read the frame back: the one-shot wrapper over
/// [`TerrainRenderer`]. The renderer's pools are sized to the plan (one block just large enough for its tiles).
pub fn execute_terrain_tiles(ctx: &GpuContext, plan: &TerrainPlan, tiles: &[&CompactTile]) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    execute_terrain_tiles_preview(ctx, plan, tiles, &[], 0.0)
}

/// [`execute_terrain_tiles`] with the edit preview (REND-033): the nodes of the plan flagged `preview` show the delta of `stamps` on the
/// body of radius `radius_m`. An empty list is [`execute_terrain_tiles`] exactly.
pub fn execute_terrain_tiles_preview(
    ctx: &GpuContext,
    plan: &TerrainPlan,
    tiles: &[&CompactTile],
    stamps: &[planet_frame::preview::PreviewStamp],
    radius_m: f64,
) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    execute_terrain_tiles_preview_split(ctx, plan, tiles, stamps, stamps.len(), radius_m)
}

/// [`execute_terrain_tiles_preview`] where only the first `delta_len` of `stamps` are the delta of an edit in flight and the rest are the
/// transient preview (VIEW-036): a node flagged `transient_only` applies only the latter.
pub fn execute_terrain_tiles_preview_split(
    ctx: &GpuContext,
    plan: &TerrainPlan,
    tiles: &[&CompactTile],
    stamps: &[planet_frame::preview::PreviewStamp],
    delta_len: usize,
    radius_m: f64,
) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    let lookup: HashMap<_, _> = tiles.iter().map(|t| (t.id(), *t)).collect();
    let cells = plan.cells;
    let pool = one_shot_pool(plan.nodes.len(), cells).map_err(ExecError::BadPool)?;
    let mut renderer = TerrainRenderer::with_pools(ctx, plan.size, cells, pool);
    renderer.set_preview(stamps, radius_m);
    renderer.preview_delta_len = delta_len.min(renderer.preview.0.len());
    for d in &plan.nodes {
        let t = lookup.get(&d.tile).ok_or(ExecError::MissingTile(d.tile))?;
        renderer.upload_tile(ctx, t)?;
    }
    let (_, mut stats) = renderer.render(ctx, plan, false)?;
    stats.vertex_bytes = renderer.uploaded_bytes;
    Ok((renderer.read_color(ctx), stats))
}

#[cfg(test)]
mod one_shot_tests {
    use super::*;

    // spec: REND-033
    #[test]
    fn the_shaders_preview_table_holds_the_cap_of_stamps_and_the_globals_fit_the_uniform_limit() {
        let max = planet_frame::preview::MAX_PREVIEW_STAMPS;
        // Two vec4 per stamp: the WGSL array, the bytes of the table and the globals follow the one constant.
        assert!(SHADER_HEAD.contains(&format!("preview: array<vec4<f32>, {}>", 2 * max)), "the WGSL table is {} vec4s", 2 * max);
        assert_eq!(PREVIEW_BYTES, 32 * max as u64);
        // The count and the number of the delta's stamps share extra.w: 6 bits for the count, which the cap must fit.
        assert!(max < 64, "the active count must fit the low 6 bits of extra.w");
        const { assert!(GLOBALS_BYTES <= 65_536, "the globals fit the default uniform binding limit") };
    }

    // spec: REND-030
    #[test]
    fn the_shaders_ramp_function_is_the_references_with_the_same_cap() {
        let cap = format!("min(per_m * d, {:.1})", planet_frame::lod::SHORE_RAMP_MAX_M);
        assert!(
            SHADER_TAIL.contains("fn shore_ramp_at(per_m: f32, d: f32) -> f32") && SHADER_TAIL.contains(&cap),
            "the cap of the WGSL is the reference's"
        );
        assert!(
            SHADER_TAIL.contains("shore_ramp_at(g.extra.z, camera_distance(nodes[in.node_index].origin_rel, in.local))"),
            "the lit fragment ramps with its own distance from the camera"
        );
    }

    // spec: REND-033
    #[test]
    fn the_preview_profiles_of_the_vertex_stage_are_the_stamp_stages_and_a_frame_without_a_preview_never_enters_them() {
        let shader = crate::compact::compact_terrain_shader(16);
        // GEN-026: hill a (1 - x^2)^2; crater a (-bowl + 0.2 rim), bowl = 1 - s(min(x / 0.8, 1)), rim on 0.7..1 at (x - 0.85) / 0.15;
        // plateau w(x) clamp(target - h, -M, M), w = 1 - s(clamp((x - 0.7) / 0.3, 0, 1)).
        for needle in [
            "let q = 1.0 - x * x;",
            "1.0 - smooth01(min(x / 0.8, 1.0))",
            "if (x >= 0.7)",
            "let u = (x - 0.85) / 0.15;",
            "0.2 * rim",
            "1.0 - smooth01(clamp((x - 0.7) / 0.3, 0.0, 1.0))",
            "clamp(a - h, -m, m)",
            "t * t * (3.0 - 2.0 * t)",
        ] {
            assert!(shader.contains(needle), "the preview profile lost '{needle}'");
        }
        // The stage is entered only for a flagged node when the count is above 0, and the normal is touched only then.
        assert!(shader.contains("let previewed = g.extra.w > 0.5 && node.morph.z > 0.5;"));
        assert!(shader.contains("out.normal = normalize(mix(out.normal, vup, flat_w) - grad);"));
        // No planet-radius value in the preview code (CON-11): the distance is the tile-tangent plane's.
        let preview_code = &shader[shader.find("fn preview_profile").unwrap()..shader.find("fn terrain_vertex").unwrap()];
        let code: String = preview_code.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join(
            "
",
        );
        assert!(!code.contains("radius") && !code.contains("6371"));
    }

    // spec: REND-030
    #[test]
    fn the_node_record_carries_no_ramp_so_the_same_ground_point_ramps_alike_under_a_tile_of_any_level() {
        use planet_core::{PlanetFixed, TileId, Vec3};
        let node = |tile: TileId| planet_frame::TerrainDraw {
            tile,
            level: tile.level(),
            origin: PlanetFixed(Vec3::new(1.0, 2.0, 3.0)),
            camera_relative_origin: [1.0, 2.0, 3.0],
            morph: 0.5,
            morph_start_m: 10.0,
            morph_end_m: 20.0,
            color: [1, 2, 3, 255],
            mesh_key: 7,
            preview: false,
            transient_only: false,
            capped: false,
            quadrants: 0b1111,
        };
        let (coarse, fine) = (TileId::new(planet_core::Face(0), 17, 1, 1).unwrap(), TileId::new(planet_core::Face(0), 20, 8, 8).unwrap());
        let mut plan = planet_frame::viewer::lit_warm_plan(
            planet_frame::camera::Camera::looking_at_centre(
                PlanetFixed(Vec3::new(0.0, 0.0, 7.0e6)),
                Vec3::new(0.0, 1.0, 0.0),
                1.0,
                1.0,
                1.0,
            ),
            (64, 64),
            32,
            planet_frame::look::LitPlan::fixed(0.0),
        );
        plan.shore_ramp_per_m = 0.01;
        plan.nodes = vec![node(coarse), node(fine)];
        let bytes = node_record_bytes(&plan);
        let word = |record: usize, w: usize| {
            let at = record * NODE_BYTES as usize + 4 * w;
            f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
        };
        for record in 0..2 {
            assert_eq!(word(record, 10), 0.0, "record {record}: word 10 is padding");
            assert_eq!(word(record, 11), [17.0, 20.0][record], "record {record}: word 11 is the tile's level (the overlay's)");
            assert_eq!((word(record, 8), word(record, 9)), (10.0, 20.0), "the morph interval is still there");
        }
    }

    // spec: REND-016
    #[test]
    fn a_one_shot_plan_larger_than_a_block_spans_several_blocks_instead_of_failing() {
        for (nodes, cells) in [(1usize, 32u32), (100, 16), (14_639, 32), (14_640, 32), (32_768, 32), (4_000, 64)] {
            let pool = one_shot_pool(nodes, cells).unwrap_or_else(|e| panic!("{nodes} nodes at cells {cells}: {e}"));
            assert!(pool.main_slots() as usize >= nodes, "{nodes} nodes at cells {cells}: {} slots", pool.main_slots());
            assert!(pool.block_bytes() <= planet_frame::pool::COMPACT_BLOCK_BYTES);
        }
    }
}
