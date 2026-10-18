//! The compact tile format on the GPU (REND-016, ADR 0033 points 3 and 4 as amended by ADR 0054 and ADR 0062): storage-buffer blocks of
//! slots (GEN-025: a 128-byte header, the 324-byte climate block, a pad word and 8-byte texels, a 32-bit height step count and two
//! 15-bit octahedral normal components), one slot per tile and no second resource per tile, and the vertex stage that places a vertex
//! from the texels of the tile and of its neighbours (the exact morph target). The renderer executes the placement; it decides
//! nothing (CON-03).
//!
//! How a draw names its slot: the draw list groups consecutive nodes of one block into a run (`DrawBuffer::Compact`), the run's bind
//! group (group 2) holds that block, and the node's *tile reference* (a 16-byte entry of a per-frame storage buffer indexed by the
//! instance index, which is `first_instance` = the node index) holds the index of the slot's first word in the block and the slot
//! index. No binding arrays or texture-index-per-draw features are needed, so the same path runs on WARP, lavapipe and the discrete
//! GPU. Fifteen blocks (mid share) cost at most fifteen bind-group changes along a plan; the lowest-slot-first free list keeps a
//! settled view in one or two. A block is at most 128 MiB, the `max_storage_buffer_binding_size` every adapter offers (VIEW-001
//! requires it by name).

use crate::terrain::TERRAIN_SHADERS;
use planet_generators::compact::{NORMAL_SCALE, SLOT_CLIMATE_OFFSET, SLOT_TEXEL_OFFSET};
use planet_streaming::SlotPool;

/// Bytes of one tile reference: the slot's first texel in its block, reserved, slot index, reserved.
pub const REF_BYTES: u64 = 16;

/// The compact pipeline's shader for tiles of `cells`: the product shader's header, fragment stage and helpers unchanged (spliced
/// at build time so the two pipelines cannot drift apart there), the vertex stage replaced by vertex pulling from the block.
pub fn compact_terrain_shader(cells: u32) -> String {
    let src = TERRAIN_SHADERS[0].1;
    let vs_at = src.find("fn terrain_vertex(").expect("product shader has a vertex stage");
    let tail_at = src.find("// Distance in pixels to the nearest integer").expect("product shader helper marker");
    let head = &src[..vs_at];
    assert!(head.contains("@group(0) @binding(1) var<storage, read> nodes: array<Node>;"), "product node binding changed");
    let vertex = COMPACT_VERTEX
        .replace("{CELLS}", &cells.to_string())
        .replace("{CLIMATE_WORD}", &(SLOT_CLIMATE_OFFSET / 4).to_string())
        .replace("{TEXEL_WORD}", &(SLOT_TEXEL_OFFSET / 4).to_string())
        .replace("{NORMAL_SCALE}", &format!("{NORMAL_SCALE:?}"));
    format!("{head}{vertex}{}", &src[tail_at..])
}

const COMPACT_VERTEX: &str = r#"
// Compact tiles (REND-016, GEN-025). Group 1: the per-frame tile reference of each node (first word of its slot in the block, reserved,
// slot index, reserved). Group 2: the block, read as words: the slot's header (nine control points as 27 f32, hr_min, hr_step,
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

fn terrain_vertex(inst: u32, vi: u32) -> VsOut {
    let node = nodes[inst];
    let tref = tile_refs[inst];
    load_tile(tref.x);
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
    out.normal = texel_normal(own_t);
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
    /// Group 1: the frame's tile references.
    pub(crate) bgl_frame: wgpu::BindGroupLayout,
    /// Group 2: one block.
    pub(crate) bgl_block: wgpu::BindGroupLayout,
    /// The blocks, allocated on first use.
    pub(crate) blocks: Vec<Option<Block>>,
    /// Bytes of one block: `block_slots * slot_bytes`.
    pub(crate) block_bytes: u64,
    /// Placement of the tiles that fell back to the vertex format (the top of the small vertex pool).
    pub(crate) fallback: SlotPool,
    /// First vertex-pool slot of the fallback region.
    pub(crate) fallback_base: u32,
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
