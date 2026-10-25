//! The compact tile format on the GPU (REND-016, ADR 0033 points 3 and 4 as amended by ADR 0054 and ADR 0062): storage-buffer blocks of
//! slots (GEN-025: a 128-byte header, the 324-byte climate block, a pad word and 8-byte texels, a 32-bit height step count and two
//! 15-bit octahedral normal components), one slot per tile and no second resource per tile, and the vertex stage that places a vertex
//! from the texels of the tile and of its neighbours (the exact morph target). The renderer executes the placement; it decides
//! nothing (CON-03).
//!
//! How a draw names its slot: the draw list groups consecutive nodes of one block into a run (`DrawBuffer`), the run's bind
//! group (group 2) holds that block, and the node's *tile reference* (a 16-byte entry of a per-frame storage buffer indexed by the
//! instance index, which is `first_instance` = the node index) holds the index of the slot's first word in the block and the slot
//! index. No binding arrays or texture-index-per-draw features are needed, so the same path runs on WARP, lavapipe and the discrete
//! GPU. Fifteen blocks (mid share) cost at most fifteen bind-group changes along a plan; the lowest-slot-first free list keeps a
//! settled view in one or two. A block is at most 128 MiB, the `max_storage_buffer_binding_size` every adapter offers (VIEW-001
//! requires it by name).

use crate::terrain::{SHADER_HEAD, SHADER_TAIL};
use planet_generators::compact::{NORMAL_SCALE, SLOT_CLIMATE_OFFSET, SLOT_TEXEL_OFFSET};

/// Bytes of one tile reference: the slot's first texel in its block, the entry of the tile's fade plane plus one (0: none, REND-039), slot index, reserved.
pub const REF_BYTES: u64 = 16;

/// The compact pipeline's shader for tiles of `cells`: the product shader's header, fragment stage and helpers (shared with the
/// reference renderer so the two cannot drift apart there) around the vertex stage that pulls vertices from the block.
pub fn compact_terrain_shader(cells: u32) -> String {
    let (head, tail) = (SHADER_HEAD, SHADER_TAIL);
    assert!(head.contains("@group(0) @binding(1) var<storage, read> nodes: array<Node>;"), "product node binding changed");
    let vertex = COMPACT_VERTEX
        .replace("{CELLS}", &cells.to_string())
        .replace("{CLIMATE_WORD}", &(SLOT_CLIMATE_OFFSET / 4).to_string())
        .replace("{TEXEL_WORD}", &(SLOT_TEXEL_OFFSET / 4).to_string())
        .replace("{NORMAL_SCALE}", &format!("{NORMAL_SCALE:?}"));
    format!("{head}{vertex}{tail}")
}

const COMPACT_VERTEX: &str = r#"
// Compact tiles (REND-016, GEN-025). Group 1: the per-frame tile reference of each node (first word of its slot in the block, the entry of its
// fade plane plus one or 0, slot index, reserved), then the frame's fade planes (REND-039). Group 2: the block, read as words: the slot's header (nine control points as 27 f32, hr_min, hr_step,
// h_origin, skirt depth, the flags word), the climate block (81 words) and the texels (two words each).
@group(1) @binding(0) var<storage, read> tile_refs: array<vec4<u32>>;
@group(2) @binding(0) var<storage, read> block: array<u32>;

const CELLS: i32 = {CELLS};
// Word offsets inside the slot (GEN-025: climate block at byte 128, texels at byte 456).
const CLIMATE_WORD: u32 = {CLIMATE_WORD}u;
const TEXEL_WORD: u32 = {TEXEL_WORD}u;
const NORMAL_SCALE: f32 = {NORMAL_SCALE};

var<private> c0: vec3<f32>;
var<private> c1: vec3<f32>;
var<private> c2: vec3<f32>;
var<private> c3: vec3<f32>;
var<private> c4: vec3<f32>;
var<private> c5: vec3<f32>;
var<private> c6: vec3<f32>;
var<private> c7: vec3<f32>;
var<private> c8: vec3<f32>;
var<private> hr_min: f32;
var<private> hr_step: f32;
var<private> h_origin: f32;
var<private> skirt_depth: f32;
var<private> flip: bool;
// Index of the slot's first word in the block.
var<private> origin: u32;
// The chunk change fading in (REND-039): the entry of the frame's tile-reference buffer where the tile's change plane starts, plus one (0: the
// tile has no plane in this frame), and the fade `1 - w` of the node; 0 reads no plane, so the old arithmetic is untouched.
var<private> plane_entry: u32;
var<private> fade: f32;

fn hf(k: u32) -> f32 {
    return bitcast<f32>(block[origin + k]);
}

fn hv(k: u32) -> vec3<f32> {
    return vec3<f32>(hf(k), hf(k + 1u), hf(k + 2u));
}

fn load_tile(first_word: u32) {
    origin = first_word;
    c0 = hv(0u);
    c1 = hv(3u);
    c2 = hv(6u);
    c3 = hv(9u);
    c4 = hv(12u);
    c5 = hv(15u);
    c6 = hv(18u);
    c7 = hv(21u);
    c8 = hv(24u);
    hr_min = hf(27u);
    hr_step = hf(28u);
    h_origin = hf(29u);
    skirt_depth = hf(30u);
    flip = (block[origin + 31u] & 1u) != 0u;
}

fn basis(t: f32) -> vec3<f32> {
    return vec3<f32>((1.0 - t) * (1.0 - 2.0 * t), 4.0 * t * (1.0 - t), t * (2.0 * t - 1.0));
}

fn dbasis(t: f32) -> vec3<f32> {
    return vec3<f32>(4.0 * t - 3.0, 4.0 - 8.0 * t, 4.0 * t - 1.0);
}

// Row blends of the nine control points along u (row b = control points 3b..3b+2); unrolled on purpose (SPIKE-06: a loop over
// a uniform array indexed at run time costs several times more than the arithmetic).
fn row0(w: vec3<f32>) -> vec3<f32> { return c0 * w.x + c1 * w.y + c2 * w.z; }
fn row1(w: vec3<f32>) -> vec3<f32> { return c3 * w.x + c4 * w.y + c5 * w.z; }
fn row2(w: vec3<f32>) -> vec3<f32> { return c6 * w.x + c7 * w.y + c8 * w.z; }

fn base_point(u: f32, v: f32) -> vec3<f32> {
    let bu = basis(u);
    let bv = basis(v);
    return row0(bu) * bv.x + row1(bu) * bv.y + row2(bu) * bv.z;
}

// Unit normal of the base surface at (u, v), outward.
fn base_normal(u: f32, v: f32) -> vec3<f32> {
    let bu = basis(u);
    let bv = basis(v);
    let du = dbasis(u);
    let dv = dbasis(v);
    let pv = row0(bu) * dv.x + row1(bu) * dv.y + row2(bu) * dv.z;
    let pu = row0(du) * bv.x + row1(du) * bv.y + row2(du) * bv.z;
    return normalize(cross(pu, pv)) * select(1.0, -1.0, flip);
}

// Texel (i, j): word 0 the 32-bit height step count, word 1 the two 15-bit octahedral normal components (bits 0-14 and 15-29).
fn texel(i: i32, j: i32) -> vec2<u32> {
    let k = origin + TEXEL_WORD + u32(j * (CELLS + 1) + i) * 2u;
    return vec2<u32>(block[k], block[k + 1u]);
}

fn rel_height(t: vec2<u32>) -> f32 {
    return hr_min + f32(t.x) * hr_step;
}

// The height of texel (i, j) with the fade of the chunk change taken off: `h_q - fade x change` (REND-039). Without a plane in this frame
// or at fade 0 it is `hr` itself, the same operations as before.
fn faded_height(i: i32, j: i32, hr: f32) -> f32 {
    if (fade == 0.0) {
        return hr;
    }
    let k = u32(j * (CELLS + 1) + i);
    let v = tile_refs[plane_entry - 1u + k / 4u];
    let lane = k & 3u;
    var bits = v.x;
    if (lane == 1u) { bits = v.y; }
    if (lane == 2u) { bits = v.z; }
    if (lane == 3u) { bits = v.w; }
    return hr - fade * bitcast<f32>(bits);
}

fn texel_normal(t: vec2<u32>) -> vec3<f32> {
    let w = bitcast<i32>(t.y);
    let x = max(f32(extractBits(w, 0u, 15u)) / NORMAL_SCALE, -1.0);
    let y = max(f32(extractBits(w, 15u, 15u)) / NORMAL_SCALE, -1.0);
    return oct_decode(vec2<f32>(x, y));
}

// Position (tile-local) of lattice vertex (i, j) with its tile-relative height `hr`, and the height itself in w.
fn place(i: i32, j: i32, hr: f32) -> vec4<f32> {
    let u = f32(i) / f32(CELLS);
    let v = f32(j) / f32(CELLS);
    return vec4<f32>(base_point(u, v) + base_normal(u, v) * hr, hr);
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        let s = select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));
        n = vec3<f32>((1.0 - abs(n.y)) * s.x, (1.0 - abs(n.x)) * s.y, n.z);
    }
    return normalize(n);
}

// The edit preview (REND-033): the profiles of GEN-026 at x = distance / reach, kind 1 hill, 2 crater, 3 plateau, `a` the amount (the plateau's
// target is absolute), `m` the bound, `h` the running height above the reference sphere.
fn smooth01(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

fn preview_profile(kind: f32, a: f32, m: f32, x: f32, h: f32) -> f32 {
    if (x >= 1.0) {
        return 0.0;
    }
    if (kind < 1.5) {
        let q = 1.0 - x * x;
        return a * q * q;
    }
    if (kind < 2.5) {
        let bowl = 1.0 - smooth01(min(x / 0.8, 1.0));
        var rim = 0.0;
        if (x >= 0.7) {
            let u = (x - 0.85) / 0.15;
            let v = 1.0 - u * u;
            rim = v * v;
        }
        return a * (-bowl + 0.2 * rim);
    }
    return (1.0 - smooth01(clamp((x - 0.7) / 0.3, 0.0, 1.0))) * clamp(a - h, -m, m);
}

struct Bump {
    delta: f32,
    grad: vec3<f32>,
    flat_w: f32,
};

// The summed signed delta of the preview stamps at a point (camera-relative `p_rel`, the radial up `up` AT the point, height `h0` above the
// sphere), the gradient of the delta along the ground and how flat a plateau makes the ground there (0 none, 1 fully). The distance is
// measured in the tangent plane of the point's own up, so two tiles that share a border vertex agree on it (CON-11: no planet radius).
fn preview_bump(p_rel: vec3<f32>, up: vec3<f32>, h0: f32, k0: u32) -> Bump {
    var total = 0.0;
    var grad = vec3<f32>(0.0);
    var flat_w = 0.0;
    // extra.w packs the active count (low 6 bits) and 64 times the number of stamps of the edit's delta, which come first in the table: a
    // node an exact tile holds the edit for starts after them (`k0`).
    let n = u32(g.extra.w) & 63u;
    for (var k = k0; k < n; k = k + 1u) {
        let a = g.preview[2u * k];
        let b = g.preview[2u * k + 1u];
        let d3 = p_rel - a.xyz;
        let dh = d3 - up * dot(d3, up);
        let d = length(dh);
        if (d < a.w) {
            let x = d / a.w;
            let h = h0 + total;
            // The profiles are even in x: the difference mirrors at 0, so the slope at the centre is 0.
            let slope = (preview_profile(b.y, b.x, b.w, x + 0.01, h) - preview_profile(b.y, b.x, b.w, abs(x - 0.01), h)) / 0.02;
            let dir = select(vec3<f32>(0.0), dh / d, d > 1e-6);
            total = total + b.z * preview_profile(b.y, b.x, b.w, x, h);
            grad = grad + dir * (b.z * slope / a.w);
            if (b.y > 2.5 && b.z > 0.0 && abs(b.x - h) <= b.w) {
                flat_w = max(flat_w, 1.0 - smooth01(clamp((x - 0.7) / 0.3, 0.0, 1.0)));
            }
        }
    }
    return Bump(total, grad, flat_w);
}

// A lattice vertex placed with the preview's delta along its own radial up (the position and, in w, the height with the delta), so the
// parent-grid target built from displaced neighbours is the displaced parent grid.
fn preview_place(o_rel: vec3<f32>, nup: vec4<f32>, i: i32, j: i32, hr: f32, k0: u32) -> vec4<f32> {
    let p = place(i, j, hr);
    let up = normalize(nup.xyz + p.xyz * nup.w);
    let b = preview_bump(o_rel + p.xyz, up, h_origin + p.w, k0);
    return vec4<f32>(p.xyz + up * b.delta, p.w + b.delta);
}

// A neighbour for the parent-grid target: the preview's displaced vertex for a previewed node, the plain one otherwise.
fn cplace(previewed: bool, o_rel: vec3<f32>, nup: vec4<f32>, i: i32, j: i32, k0: u32) -> vec4<f32> {
    let hr = faded_height(i, j, rel_height(texel(i, j)));
    if (previewed) {
        return preview_place(o_rel, nup, i, j, hr, k0);
    }
    return place(i, j, hr);
}

fn terrain_vertex(inst: u32, vi: u32) -> VsOut {
    let node = nodes[inst];
    let tref = tile_refs[inst];
    load_tile(tref.x);
    plane_entry = tref.y;
    fade = select(0.0, node.phase[6].w, tref.y != 0u);
    let w = CELLS + 1;
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
            j = CELLS;
        } else if (k == 2) {
            j = t;
        } else {
            i = CELLS;
            j = t;
        }
    }
    let own_t = texel(i, j);
    // A previewed node (REND-033) shows the edit's delta over its stale tile; every other node, and every frame without a preview, skips this.
    let previewed = g.extra.w > 0.5 && node.morph.z > 0.5;
    // 2 marks a node only the transient stamps (after the delta's) apply to (VIEW-036).
    let k0 = select(0u, u32(g.extra.w) >> 6u, node.morph.z > 1.5);
    var own = place(i, j, faded_height(i, j, rel_height(own_t)));
    var grad = vec3<f32>(0.0);
    var flat_w = 0.0;
    var vup = vec3<f32>(0.0);
    if (previewed) {
        vup = normalize(node.up.xyz + own.xyz * node.up.w);
        let b0 = preview_bump(node.origin_rel.xyz + own.xyz, vup, h_origin + own.w, k0);
        grad = b0.grad;
        flat_w = b0.flat_w;
        own = vec4<f32>(own.xyz + vup * b0.delta, own.w + b0.delta);
    }
    // The parent-grid target (exact): the average of the neighbours' positions and heights, as tile_mesh_field builds it.
    var coarse = own;
    let odd_i = (i & 1) == 1;
    let odd_j = (j & 1) == 1;
    if (odd_i && !odd_j) {
        coarse = 0.5 * (cplace(previewed, node.origin_rel.xyz, node.up, i - 1, j, k0) + cplace(previewed, node.origin_rel.xyz, node.up, i + 1, j, k0));
    } else if (!odd_i && odd_j) {
        coarse = 0.5 * (cplace(previewed, node.origin_rel.xyz, node.up, i, j - 1, k0) + cplace(previewed, node.origin_rel.xyz, node.up, i, j + 1, k0));
    } else if (odd_i && odd_j) {
        coarse = 0.5 * (cplace(previewed, node.origin_rel.xyz, node.up, i - 1, j - 1, k0) + cplace(previewed, node.origin_rel.xyz, node.up, i + 1, j + 1, k0));
    }
    var pos = own.xyz;
    var cpos = coarse.xyz;
    if (skirt) {
        let d = base_normal(f32(i) / f32(CELLS), f32(j) / f32(CELLS)) * skirt_depth;
        pos = pos - d;
        cpos = cpos - d;
    }
    var m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    // The edge masks of a node next to a capped tile (ADR 0074; 0 elsewhere, which leaves `m` as it was): a vertex on a stitched edge (it faces a
    // capped leaf one level coarser) is fully morphed, on a fine edge (a capped leaf facing finer ones) not at all, whatever the distance. The
    // skirt vertices carry the grid coordinates of their edge vertex and follow it.
    let masks = u32(max(node.morph.w, 0.0) + 0.5) >> 8u;
    var edges = 0u;
    if (i == CELLS) { edges = edges | 1u; }
    if (i == 0) { edges = edges | 2u; }
    if (j == CELLS) { edges = edges | 4u; }
    if (j == 0) { edges = edges | 8u; }
    if ((edges & masks & 15u) != 0u) {
        m = 1.0;
    } else if ((edges & (masks >> 4u)) != 0u) {
        m = 0.0;
    }
    let local = mix(pos, cpos, m);
    let p = node.origin_rel.xyz + local;
    let h = h_origin + own.w;
    let ch = h_origin + coarse.w;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = texel_normal(own_t);
    if (previewed) {
        // A plateau flattens the ground: the normal leans to the vertex's up by how flat it makes it, then the slope of the delta tilts it.
        out.normal = normalize(mix(out.normal, vup, flat_w) - grad);
    }
    out.height = mix(h, ch, m);
    out.height_fine = h;
    out.morph = m;
    out.local = local;
    out.node_index = inst;
    out.aux = vec2<f32>(f32(i), f32(j)) / f32(CELLS);
    return out;
}

@vertex
fn vs(@builtin(instance_index) inst: u32, @builtin(vertex_index) vi: u32) -> VsOut {
    return terrain_vertex(inst, vi);
}

// The debug overlay (VIEW-014), `mode` 1 (tiles) or 2 (mesh), all coloured by the tile's level of detail (eight colours, wrapping) and
// dimmed with the morph factor (a vertex at full detail is bright, fully morphed onto the parent grid dim):
//  - a faint tint of the level's colour over the whole tile, so the levels show even where the mesh is finer than a pixel;
//  - the outline of the tile (u or v at 0 or 1), always drawn;
//  - mode 2 only: a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (a, c of
//    each cell: from (i, j) to (i + 1, j + 1), `planet_generators::mesh::grid_indices`), a fixed line of about a pixel that does not fade
//    out however fine the mesh is (the LOD keeps an edge about 2.5 px long; the coarse-LOD view makes them about 10 px).
// The lines are radiance in the scene units of the frame (`factor` is its pre-exposure), so they stay visible at any exposure. Needs
// derivatives: the caller takes it in uniform control flow (the overlay code is a uniform).
fn overlay_wire(in: VsOut, rgb: vec3<f32>, factor: f32, mode: f32) -> vec3<f32> {
    let gx = in.aux.x * f32(CELLS);
    let gy = in.aux.y * f32(CELLS);
    let wx = max(fwidth(gx), 1e-6);
    let wy = max(fwidth(gy), 1e-6);
    let wd = max(fwidth(gx - gy), 1e-6);
    let fx = fract(gx);
    let fy = fract(gy);
    let dx = min(fx, 1.0 - fx) / wx;
    let dy = min(fy, 1.0 - fy) / wy;
    let dd = abs(fx - fy) / wd;
    let d = min(min(dx, dy), dd);
    let cell_line = select(0.0, 1.0 - smoothstep(0.3, 1.0, d), mode > 1.5);
    let ux = in.aux.x;
    let uy = in.aux.y;
    let tile_d = min(min(ux, 1.0 - ux) / max(fwidth(ux), 1e-9), min(uy, 1.0 - uy) / max(fwidth(uy), 1e-9));
    let tile_line = 1.0 - smoothstep(0.5, 1.5, tile_d);
    var palette = array<vec3<f32>, 8>(
        vec3<f32>(1.0, 0.15, 0.15),
        vec3<f32>(1.0, 0.55, 0.1),
        vec3<f32>(1.0, 0.95, 0.15),
        vec3<f32>(0.2, 0.95, 0.2),
        vec3<f32>(0.1, 0.9, 0.95),
        vec3<f32>(0.25, 0.4, 1.0),
        vec3<f32>(0.75, 0.3, 1.0),
        vec3<f32>(1.0, 1.0, 1.0),
    );
    // The node record's word is the level plus 32 times the node's state (the stale-tile overlay, VIEW-044).
    let level = u32(max(nodes[in.node_index].morph.w, 0.0) + 0.5) % 32u;
    // A capped tile (state 4, ADR 0074) is grey, the colour of the stale-tile overlay's legend, whatever its level.
    let capped = ((u32(max(nodes[in.node_index].morph.w, 0.0) + 0.5) / 32u) % 8u) == 4u;
    let hue = select(palette[level % 8u], vec3<f32>(0.6, 0.6, 0.6), capped);
    let brightness = mix(1.0, 0.3, clamp(in.morph, 0.0, 1.0));
    let tinted = mix(rgb, rgb * hue * 1.6, 0.4);
    let line = hue * (brightness * 0.8 * factor);
    return mix(tinted, line, max(cell_line, tile_line));
}

// The stale-tile overlay (VIEW-044), `mode` 4: a tint of the node's state over the lit colour and the outline of the tile in the state's colour. The
// state is the node record's word over 32 (0 exact, 1 stale, 2 stand-in, 3 previewed, 4 capped); the colours are the legend's, the same as
// `STATE_COLOURS` of the app. The outline is radiance in the scene units of the frame (`factor` is its pre-exposure).
fn stale_tint(in: VsOut, rgb: vec3<f32>, factor: f32) -> vec3<f32> {
    var palette = array<vec3<f32>, 5>(
        vec3<f32>(0.15, 0.85, 0.30),
        vec3<f32>(1.00, 0.60, 0.10),
        vec3<f32>(0.95, 0.20, 0.20),
        vec3<f32>(0.70, 0.30, 1.00),
        vec3<f32>(0.60, 0.60, 0.60),
    );
    let state = min((u32(max(nodes[in.node_index].morph.w, 0.0) + 0.5) / 32u) % 8u, 4u);
    let hue = palette[state];
    let ux = in.aux.x;
    let uy = in.aux.y;
    let tile_d = min(min(ux, 1.0 - ux) / max(fwidth(ux), 1e-9), min(uy, 1.0 - uy) / max(fwidth(uy), 1e-9));
    let tile_line = 1.0 - smoothstep(0.5, 1.5, tile_d);
    let tinted = mix(rgb, rgb * hue * 1.8, 0.55);
    return mix(tinted, hue * (0.8 * factor), tile_line);
}

// The climate texel `idx` (row-major over the 9 x 9 texture) of the tile whose slot starts at word `base`: one word of the slot's
// climate block, the temperature as f16 in the low half and the precipitation in the high half (GEN-021, GEN-025).
fn climate_texel(base: u32, idx: i32) -> vec2<f32> {
    return unpack2x16float(block[base + CLIMATE_WORD + u32(idx)]);
}

// The climate at a fragment of a compact tile (presentation data): the bilinear interpolation of the tile's 9 x 9 texture at the
// fragment's tile coordinates (`aux`, interpolated from the vertices). The f16 decode is exact.
fn tile_climate(in: VsOut) -> vec2<f32> {
    let tref = tile_refs[in.node_index];
    let base = tref.x;
    let x = clamp(in.aux.x, 0.0, 1.0) * 8.0;
    let y = clamp(in.aux.y, 0.0, 1.0) * 8.0;
    let a = min(i32(x), 7);
    let b = min(i32(y), 7);
    let wx = x - f32(a);
    let wy = y - f32(b);
    let i00 = b * 9 + a;
    let c00 = climate_texel(base, i00);
    let c10 = climate_texel(base, i00 + 1);
    let c01 = climate_texel(base, i00 + 9);
    let c11 = climate_texel(base, i00 + 10);
    return mix(mix(c00, c10, wx), mix(c01, c11, wx), wy);
}

"#;

/// One block of the compact main pool: a storage buffer of `block_slots` slots (GEN-025, slot after slot) and the bind
/// group that names it for a run (group 2).
pub(crate) struct Block {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind: wgpu::BindGroup,
}

/// GPU state of the compact format.
pub(crate) struct CompactGpu {
    pub(crate) pipeline: wgpu::RenderPipeline,
    /// The same shader with the quadrant cut on, for the nodes drawn over some quadrants (STRM-016).
    pub(crate) pipeline_cut: wgpu::RenderPipeline,
    /// Group 1: the frame's tile references.
    pub(crate) bgl_frame: wgpu::BindGroupLayout,
    /// Group 2: one block.
    pub(crate) bgl_block: wgpu::BindGroupLayout,
    /// The blocks, allocated on first use.
    pub(crate) blocks: Vec<Option<Block>>,
    /// Bytes of one block: `block_slots * slot_bytes`.
    pub(crate) block_bytes: u64,
}

impl CompactGpu {
    /// Block `block`, created (buffer and bind group) on first use.
    pub(crate) fn block(&mut self, device: &wgpu::Device, block: usize, init: &mut crate::terrain::InitCounts) -> &Block {
        if self.blocks.len() <= block {
            self.blocks.resize_with(block + 1, || None);
        }
        let (bgl, size) = (&self.bgl_block, self.block_bytes);
        self.blocks[block].get_or_insert_with(|| {
            init.pool_allocations += 1;
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("compact-block"),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compact-block-bind"),
                layout: bgl,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
            });
            Block { buffer, bind }
        })
    }
}
