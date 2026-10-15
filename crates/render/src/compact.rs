//! The compact tile format on the GPU (REND-016, ADR 0033 points 3 and 4): atlas textures of `Rgba16Uint` texels, one slot per tile,
//! a persistent record buffer indexed by slot, and the vertex stage that places a vertex from the texels of the tile and of its
//! neighbours (the exact morph target). The renderer executes the placement; it decides nothing (CON-03).
//!
//! How a draw names its slot (the open decision of ADR 0033 point 3, decided here): the draw list groups consecutive nodes of one
//! atlas into a run (`DrawBuffer::Atlas`), the run's bind group (group 2) holds that atlas, and the node's *tile reference* (a
//! 16-byte entry of a per-frame storage buffer indexed by the instance index, which is `first_instance` = the node index) holds the
//! slot's texel origin and slot index, which selects the 128-byte record in the persistent record buffer (group 1). No binding
//! arrays or texture-index-per-draw features are needed, so the same path runs on WARP, lavapipe and the discrete GPU. Fifteen atlases
//! (mid share) cost at most fifteen bind-group changes along a plan; the lowest-slot-first free list keeps a settled view in one or two.

use crate::terrain::TERRAIN_SHADERS;
use planet_generators::compact::COMPACT_RECORD_BYTES;
use planet_streaming::SlotPool;

/// Bytes of one tile reference: texel origin x, y, slot index, reserved.
pub const REF_BYTES: u64 = 16;

/// The compact pipeline's shader for tiles of `cells`: the product shader's header, fragment stage and helpers unchanged (spliced
/// at build time so the two pipelines cannot drift apart there), the vertex stage replaced by vertex pulling from the atlas.
pub fn compact_terrain_shader(cells: u32) -> String {
    let src = TERRAIN_SHADERS[0].1;
    let vs_at = src.find("@vertex").expect("product shader has a vertex stage");
    let tail_at = src.find("// Distance in pixels to the nearest integer").expect("product shader helper marker");
    let head = &src[..vs_at];
    assert!(head.contains("@group(0) @binding(1) var<storage, read> nodes: array<Node>;"), "product node binding changed");
    let vertex = COMPACT_VERTEX.replace("{CELLS}", &cells.to_string());
    format!("{head}{vertex}{}", &src[tail_at..])
}

const COMPACT_VERTEX: &str = r#"
// Compact tiles (REND-016). Group 1: the persistent record per slot (words: nine control points as 27 f32, hr_min, hr_step,
// h_origin, skirt depth, then the flip flag) and the per-frame tile reference of each node (texel origin x, y, slot). Group 2: the atlas.
struct TileRec {
    w: array<u32, 32>,
};
@group(1) @binding(0) var<storage, read> tile_records: array<TileRec>;
@group(1) @binding(1) var<storage, read> tile_refs: array<vec4<u32>>;
@group(2) @binding(0) var atlas: texture_2d<u32>;

const CELLS: i32 = {CELLS};

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
var<private> origin: vec2<i32>;

fn rf(r: ptr<function, TileRec>, k: i32) -> f32 {
    return bitcast<f32>((*r).w[k]);
}

fn load_tile(slot: u32, o: vec2<u32>) {
    var r = tile_records[slot];
    c0 = vec3<f32>(rf(&r, 0), rf(&r, 1), rf(&r, 2));
    c1 = vec3<f32>(rf(&r, 3), rf(&r, 4), rf(&r, 5));
    c2 = vec3<f32>(rf(&r, 6), rf(&r, 7), rf(&r, 8));
    c3 = vec3<f32>(rf(&r, 9), rf(&r, 10), rf(&r, 11));
    c4 = vec3<f32>(rf(&r, 12), rf(&r, 13), rf(&r, 14));
    c5 = vec3<f32>(rf(&r, 15), rf(&r, 16), rf(&r, 17));
    c6 = vec3<f32>(rf(&r, 18), rf(&r, 19), rf(&r, 20));
    c7 = vec3<f32>(rf(&r, 21), rf(&r, 22), rf(&r, 23));
    c8 = vec3<f32>(rf(&r, 24), rf(&r, 25), rf(&r, 26));
    hr_min = rf(&r, 27);
    hr_step = rf(&r, 28);
    h_origin = rf(&r, 29);
    skirt_depth = rf(&r, 30);
    flip = r.w[31] != 0u;
    origin = vec2<i32>(i32(o.x), i32(o.y));
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

fn texel(i: i32, j: i32) -> vec4<u32> {
    return textureLoad(atlas, origin + vec2<i32>(i, j), 0);
}

fn rel_height(t: vec4<u32>) -> f32 {
    return hr_min + f32(t.x) * hr_step;
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

@vertex
fn vs(@builtin(instance_index) inst: u32, @builtin(vertex_index) vi: u32) -> VsOut {
    let node = nodes[inst];
    let tref = tile_refs[inst];
    load_tile(tref.z, tref.xy);
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
    let own = place(i, j, rel_height(own_t));
    // The parent-grid target (exact): the average of the neighbours' positions and heights, as tile_mesh_field builds it.
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
        let d = base_normal(f32(i) / f32(CELLS), f32(j) / f32(CELLS)) * skirt_depth;
        pos = pos - d;
        cpos = cpos - d;
    }
    let m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let local = mix(pos, cpos, m);
    let p = node.origin_rel.xyz + local;
    let h = h_origin + own.w;
    let ch = h_origin + coarse.w;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = oct_decode(unpack2x16snorm(own_t.y | (own_t.z << 16u)));
    out.height = mix(h, ch, m);
    out.height_fine = h;
    out.morph = m;
    out.local = local;
    out.node_index = inst;
    return out;
}

"#;

/// The record buffer holds one [`COMPACT_RECORD_BYTES`] record per slot of the main pool.
const _: () = assert!(COMPACT_RECORD_BYTES == 128);

/// One atlas texture and the bind group that names it for a run (group 2).
pub(crate) struct Atlas {
    pub(crate) texture: wgpu::Texture,
    pub(crate) bind: wgpu::BindGroup,
}

/// GPU state of the compact format.
pub(crate) struct CompactGpu {
    pub(crate) pipeline: wgpu::RenderPipeline,
    /// Group 1: slot records and the frame's tile references.
    pub(crate) bgl_frame: wgpu::BindGroupLayout,
    /// Group 2: one atlas.
    pub(crate) bgl_atlas: wgpu::BindGroupLayout,
    /// One record per slot of the main pool, allocated with the renderer (a fixed item of the share).
    pub(crate) records: wgpu::Buffer,
    /// The atlases, allocated on first use.
    pub(crate) atlases: Vec<Option<Atlas>>,
    /// Slots along one side of an atlas.
    pub(crate) cols: u32,
    /// Side of an atlas texture in texels.
    pub(crate) side: u32,
    /// Placement of the tiles that fell back to the vertex format (the top of the small vertex pool).
    pub(crate) fallback: SlotPool,
    /// First vertex-pool slot of the fallback region.
    pub(crate) fallback_base: u32,
}
