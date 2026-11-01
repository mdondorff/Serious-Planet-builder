//! The shadow clipmap's gather (REND-047, ADR 0085 point 2): a compute pass that writes the dirty rectangles of the frame's
//! [`ClipmapPlan`] from the compact tile slots into an `R32Float` array texture (one layer a level, toroidal), applies the frame's
//! preview stamps with the vertex stage's own WGSL function, and rebuilds the max-height mips of the touched region. The renderer
//! executes the plan; it decides nothing (CON-03). Nothing draws the clipmap yet.
//!
//! The stored height is the tile's height above the sphere (bilinear in the plan's `(u, v)` over the tile's lattice) minus the plan's
//! drop of the block, so the shadow march works in the anchor's flat frame without a planet radius (CON-11). The preview stamps are
//! given as plane positions around the anchor ([`ClipmapStamp`]); the distance is measured in that plane, which differs from the
//! vertex stage's tangent-plane distance by about the reach times (rho / R)^2 / 6 at a distance rho from the anchor (a metre for a
//! kilometre of reach at 500 km, where the texels are a kilometre).

use crate::compact::{slot_access_wgsl, PREVIEW_STAMPS};
use crate::terrain::TerrainRenderer;
use crate::GpuContext;
use planet_core::TileId;
use planet_frame::clipmap::{BlockMap, ClipmapPlan, TexelRect};
use planet_frame::preview::PreviewStamp;
use std::collections::BTreeMap;

pub(crate) const CLIPMAP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const JOB_BYTES: u64 = 112;
const MIP_JOB_BYTES: u64 = 32;
/// The most stamps the gather's table holds (the terrain table's 64 entries, two a stamp).
pub const MAX_CLIPMAP_STAMPS: usize = 32;

/// A preview stamp in the clipmap's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipmapStamp {
    /// The anchor of the stamp in the plane around the clipmap's anchor, metres.
    pub plane_m: [f32; 2],
    pub reach_m: f32,
    pub amount_m: f32,
    /// 1 hill, 2 crater, 3 plateau (`PreviewStamp::kind_code`).
    pub kind: f32,
    pub sign: f32,
    pub max_delta_m: f32,
}

impl ClipmapStamp {
    /// The stamp of `s` around the unit direction `anchor` of a planet of `radius_m`.
    pub fn of(s: &PreviewStamp, anchor: planet_core::Vec3, radius_m: f64) -> Option<ClipmapStamp> {
        // A stamp on the far hemisphere is nowhere near a window (and its plane position would be planet-sized, CON-11).
        if s.dir.dot(anchor) <= 0.0 {
            return None;
        }
        let (x, y) = planet_frame::clipmap::plane_position(anchor, s.dir, radius_m);
        Some(ClipmapStamp {
            plane_m: [x as f32, y as f32],
            reach_m: s.reach_m as f32,
            amount_m: s.amount_m as f32,
            kind: s.kind_code(),
            sign: f32::from(s.sign),
            max_delta_m: s.max_delta_m as f32,
        })
    }
}

/// What the gather needs of the frame's preview (REND-033, REND-045).
#[derive(Clone, Debug, Default)]
pub struct ClipmapPreview {
    /// The stamps of the edit's delta first, then the transient ones.
    pub stamps: Vec<ClipmapStamp>,
    /// How many of `stamps` belong to the delta.
    pub delta_count: u32,
    /// The tiles whose node the vertex stage previews: 1 for the delta and the transient stamps, 2 for the transient ones only.
    pub flagged: BTreeMap<TileId, u8>,
}

/// What the gather writes where no block covers a texel (and what the mips of such texels are): far below any terrain.
pub const NO_DATA_M: f32 = -1.0e30;

const GATHER_HEAD: &str = r#"
const NO_DATA: f32 = {NO_DATA};
struct Globals {
    extra: vec4<f32>,
    preview: array<vec4<f32>, 64>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var clipmap: texture_storage_2d_array<r32float, write>;
// The job that owns each texel of each level: the highest job index that covers it (jobs are in overwrite order), so overlapping
// jobs write in an order that does not depend on how the adapter schedules the dispatches.
@group(0) @binding(2) var<storage, read_write> owner: array<atomic<u32>>;
// One block of one tile on one level, intersected with one dirty rectangle.
struct Job {
    rect: vec4<i32>,
    dirty: vec4<i32>,
    // u0, du per texel in i, du per texel in j, the margin (in uv) a texel may lie outside the tile.
    u: vec4<f32>,
    // v0, dv per texel in i, dv per texel in j, the level's texel in metres.
    v: vec4<f32>,
    // c0 + c1 x + c2 y + c3 (x^2 + y^2), x and y metres from the block's centre.
    drop: vec4<f32>,
    // The slot's first word, the layer, the preview flag, the level's texels.
    misc: vec4<u32>,
    // The job's id (1 and up, in overwrite order).
    params: vec4<u32>,
};
@group(1) @binding(0) var<uniform> job: Job;
"#;

const GATHER_MAIN: &str = r#"
fn slot_of(c: i32, n: i32) -> i32 {
    return ((c % n) + n) % n;
}

// The texel of this invocation inside the job's dirty rectangle, or false.
var<private> ti: i32;
var<private> tj: i32;
fn texel_of_invocation(gid: vec3<u32>) -> bool {
    let w = u32(job.dirty.z - job.dirty.x);
    let h = u32(job.dirty.w - job.dirty.y);
    if (gid.x >= w || gid.y >= h) {
        return false;
    }
    ti = job.dirty.x + i32(gid.x);
    tj = job.dirty.y + i32(gid.y);
    return true;
}

fn owner_index(i: i32, j: i32) -> u32 {
    let n = i32(job.misc.w);
    return job.misc.y * u32(n * n) + u32(slot_of(j, n) * n + slot_of(i, n));
}

@compute @workgroup_size(8, 8, 1)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (!texel_of_invocation(gid)) {
        return;
    }
    atomicStore(&owner[owner_index(ti, tj)], 0u);
    // No data until a job writes the texel: far below any terrain, so a ray sees no occluder there.
    let n = i32(job.misc.w);
    textureStore(clipmap, vec2<i32>(slot_of(ti, n), slot_of(tj, n)), i32(job.misc.y), vec4<f32>(NO_DATA, 0.0, 0.0, 0.0));
}

fn coords() -> vec2<f32> {
    return vec2<f32>(
        job.u.x + job.u.y * f32(ti - job.rect.x) + job.u.z * f32(tj - job.rect.y),
        job.v.x + job.v.y * f32(ti - job.rect.x) + job.v.z * f32(tj - job.rect.y),
    );
}

fn covered(uv: vec2<f32>) -> bool {
    let m = job.u.w;
    return !(uv.x < -m || uv.x > 1.0 + m || uv.y < -m || uv.y > 1.0 + m);
}

@compute @workgroup_size(8, 8, 1)
fn claim(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (!texel_of_invocation(gid) || !covered(coords())) {
        return;
    }
    atomicMax(&owner[owner_index(ti, tj)], job.params.x);
}

@compute @workgroup_size(8, 8, 1)
fn store_texel(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (!texel_of_invocation(gid)) {
        return;
    }
    if (atomicLoad(&owner[owner_index(ti, tj)]) != job.params.x) {
        return;
    }
    let i = ti;
    let j = tj;
    let uv = coords();
    load_tile(job.misc.x);
    let fu = clamp(uv.x, 0.0, 1.0) * f32(CELLS);
    let fv = clamp(uv.y, 0.0, 1.0) * f32(CELLS);
    let i0 = min(i32(floor(fu)), CELLS - 1);
    let j0 = min(i32(floor(fv)), CELLS - 1);
    let tu = fu - f32(i0);
    let tv = fv - f32(j0);
    let h00 = rel_height(texel(i0, j0));
    let h10 = rel_height(texel(i0 + 1, j0));
    let h01 = rel_height(texel(i0, j0 + 1));
    let h11 = rel_height(texel(i0 + 1, j0 + 1));
    var height = h_origin + mix(mix(h00, h10, tu), mix(h01, h11, tu), tv);
    let tm = job.v.w;
    if (job.misc.z > 0u && g.extra.w > 0.5) {
        let px = f32(2 * i + 1) * 0.5 * tm;
        let py = f32(2 * j + 1) * 0.5 * tm;
        let k0 = select(0u, u32(g.extra.w) >> 6u, job.misc.z > 1u);
        let bump = preview_bump(vec3<f32>(px, py, 0.0), vec3<f32>(0.0, 0.0, 1.0), height, k0);
        height = height + bump.delta;
    }
    let x = f32(2 * i + 1 - (job.rect.x + job.rect.z)) * 0.5 * tm;
    let y = f32(2 * j + 1 - (job.rect.y + job.rect.w)) * 0.5 * tm;
    let drop = job.drop.x + job.drop.y * x + job.drop.z * y + job.drop.w * (x * x + y * y);
    let n = i32(job.misc.w);
    textureStore(clipmap, vec2<i32>(slot_of(i, n), slot_of(j, n)), i32(job.misc.y), vec4<f32>(height - drop, 0.0, 0.0, 0.0));
}
"#;

/// The gather shader for tiles of `cells`: the vertex stage's slot reader and stamp function around the gather's own entry.
pub fn gather_shader(cells: u32) -> String {
    format!("{}{}{PREVIEW_STAMPS}{GATHER_MAIN}", GATHER_HEAD.replace("{NO_DATA}", &format!("{NO_DATA_M:?}")), slot_access_wgsl(cells))
}

/// The mip shader (a function so the shader scan can reach it).
pub fn mip_shader() -> String {
    MIP_SHADER.to_string()
}

const MIP_SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d_array<f32>;
@group(0) @binding(1) var dst: texture_storage_2d_array<r32float, write>;
struct MipJob {
    // The child rectangle in world texel indices of the child mip.
    rect: vec4<i32>,
    // The layer and the child's edge in texels.
    params: vec4<u32>,
};
@group(1) @binding(0) var<uniform> job: MipJob;

fn wrap(c: i32, n: i32) -> i32 {
    return ((c % n) + n) % n;
}

// Each child texel is the maximum over the parent texels of its 2 x 2 footprint dilated by one texel (SPIKE-56: the maximum of the
// footprint alone misses rays that cross a texel corner), toroidally.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(job.rect.z - job.rect.x);
    let h = u32(job.rect.w - job.rect.y);
    if (gid.x >= w || gid.y >= h) {
        return;
    }
    let cx = job.rect.x + i32(gid.x);
    let cy = job.rect.y + i32(gid.y);
    let n = i32(job.params.y);
    let pn = 2 * n;
    var best = -3.4e38;
    for (var dy = -1; dy <= 2; dy = dy + 1) {
        for (var dx = -1; dx <= 2; dx = dx + 1) {
            let p = vec2<i32>(wrap(2 * cx + dx, pn), wrap(2 * cy + dy, pn));
            best = max(best, textureLoad(src, p, i32(job.params.x), 0).x);
        }
    }
    textureStore(dst, vec2<i32>(wrap(cx, n), wrap(cy, n)), i32(job.params.x), vec4<f32>(best, 0.0, 0.0, 0.0));
}
"#;

fn floor_div(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

/// The rectangle of mip `m + 1` that a change of `r` in mip `m` touches (the dilated footprint), at most one edge wide.
pub fn parent_rect(r: &TexelRect, child_edge: i64) -> TexelRect {
    let x0 = floor_div(r.x0 - 1, 2);
    let y0 = floor_div(r.y0 - 1, 2);
    let x1 = floor_div(r.x1, 2) + 1;
    let y1 = floor_div(r.y1, 2) + 1;
    TexelRect { x0, y0, x1: x1.min(x0 + child_edge), y1: y1.min(y0 + child_edge) }
}

pub struct ClipmapGpu {
    cells: u32,
    texels: u32,
    levels: usize,
    mips: u32,
    texture: wgpu::Texture,
    mip_views: Vec<wgpu::TextureView>,
    clear_pipeline: wgpu::ComputePipeline,
    claim_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    owner: wgpu::Buffer,
    gather_bgl0: wgpu::BindGroupLayout,
    gather_bgl1: wgpu::BindGroupLayout,
    gather_bgl2: wgpu::BindGroupLayout,
    mip_pipeline: wgpu::ComputePipeline,
    mip_bgl0: wgpu::BindGroupLayout,
    mip_bgl1: wgpu::BindGroupLayout,
}

impl ClipmapGpu {
    pub fn new(ctx: &GpuContext, texels: u32, levels: usize, cells: u32) -> ClipmapGpu {
        assert!(texels.is_power_of_two() && texels >= 2, "clipmap edge must be a power of two");
        let device = &ctx.device;
        let mips = texels.trailing_zeros() + 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("clipmap"),
            size: wgpu::Extent3d { width: texels, height: texels, depth_or_array_layers: levels as u32 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CLIPMAP_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mip_views = (0..mips)
            .map(|m| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("clipmap-mip"),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: m,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let c = wgpu::ShaderStages::COMPUTE;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: c, ty, count: None };
        let uniform = |dynamic| wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        };
        let storage_tex = wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: CLIPMAP_FORMAT,
            view_dimension: wgpu::TextureViewDimension::D2Array,
        };
        let bgl = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        let gather_bgl0 = bgl(
            "clipmap-gather-0",
            &[
                entry(0, uniform(false)),
                entry(1, storage_tex),
                entry(
                    2,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
            ],
        );
        let gather_bgl1 = bgl("clipmap-gather-1", &[entry(0, uniform(true))]);
        let gather_bgl2 = bgl(
            "clipmap-gather-2",
            &[entry(
                0,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )],
        );
        let mip_bgl0 = bgl(
            "clipmap-mip-0",
            &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(1, storage_tex),
            ],
        );
        let mip_bgl1 = bgl("clipmap-mip-1", &[entry(0, uniform(true))]);
        let module = |label, source: String| {
            device
                .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
        };
        let pipeline = |label, entry_point: &str, layouts: &[&wgpu::BindGroupLayout], source: String| {
            let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &layouts,
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                module: &module(label, source),
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let gather_layouts = [&gather_bgl0, &gather_bgl1, &gather_bgl2];
        let clear_pipeline = pipeline("clipmap-clear", "clear", &gather_layouts, gather_shader(cells));
        let claim_pipeline = pipeline("clipmap-claim", "claim", &gather_layouts, gather_shader(cells));
        let gather_pipeline = pipeline("clipmap-gather", "store_texel", &gather_layouts, gather_shader(cells));
        let owner = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clipmap-owner"),
            size: u64::from(texels) * u64::from(texels) * levels as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let mip_pipeline = pipeline("clipmap-mip", "main", &[&mip_bgl0, &mip_bgl1], MIP_SHADER.to_string());
        ClipmapGpu {
            cells,
            texels,
            levels,
            mips,
            texture,
            mip_views,
            clear_pipeline,
            claim_pipeline,
            gather_pipeline,
            owner,
            gather_bgl0,
            gather_bgl1,
            gather_bgl2,
            mip_pipeline,
            mip_bgl0,
            mip_bgl1,
        }
    }

    pub fn texels(&self) -> u32 {
        self.texels
    }

    /// Write the dirty rectangles of `plan` from the tiles resident in `terrain`, then rebuild the touched mips. A block whose tile is not
    /// resident in `terrain` is skipped and returned: its texels hold [`NO_DATA_M`] unless a coarser block covers them, and the caller
    /// passes the tile as refreshed to the planner once it is resident.
    pub fn gather(&mut self, ctx: &GpuContext, terrain: &TerrainRenderer, plan: &ClipmapPlan, preview: &ClipmapPreview) -> Vec<TileId> {
        assert_eq!(plan.levels.len(), self.levels, "the plan and the texture have different level counts");
        assert_eq!(terrain.compact_cells(), self.cells, "the clipmap was made for tiles of another size");
        assert!(
            plan.levels.iter().all(|l| l.window.width() == i64::from(self.texels) && l.window.height() == i64::from(self.texels)),
            "the plan's windows are not the texture's edge"
        );
        let mut skipped: Vec<TileId> = Vec::new();
        let device = &ctx.device;
        let align = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(JOB_BYTES);
        let n = i64::from(self.texels);

        // Jobs, level by level, coarse blocks first: the order is the overwrite order.
        let mut jobs: Vec<(usize, Vec<u8>)> = Vec::new();
        let mut buffers: BTreeMap<usize, &wgpu::Buffer> = BTreeMap::new();
        for (k, level) in plan.levels.iter().enumerate() {
            for b in level.blocks.iter() {
                let Some((block, buffer, first_word)) = terrain.slot_words(b.tile) else {
                    skipped.push(b.tile);
                    continue;
                };
                buffers.insert(block, buffer);
                for d in &level.dirty {
                    let cut = b.rect.intersect(d);
                    if cut.is_empty() {
                        continue;
                    }
                    let flag = preview.flagged.get(&b.tile).copied().unwrap_or(0);
                    let id = jobs.len() as u32 + 1;
                    jobs.push((block, job_bytes(b, &cut, first_word, k as u32, flag, level.texel_m, self.texels, id)));
                }
            }
        }
        let mut stamps = vec![0f32; 4 + 64 * 4];
        let count = preview.stamps.len().min(MAX_CLIPMAP_STAMPS);
        stamps[3] = count as f32 + 64.0 * preview.delta_count.min(count as u32) as f32;
        for (s, chunk) in preview.stamps.iter().take(count).zip(stamps[4..].chunks_mut(8)) {
            chunk.copy_from_slice(&[s.plane_m[0], s.plane_m[1], 0.0, s.reach_m, s.amount_m, s.kind, s.sign, s.max_delta_m]);
        }
        let globals = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("clipmap-globals"),
            size: (stamps.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&globals, 0, &stamps.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
        let bind0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("clipmap-gather-0"),
            layout: &self.gather_bgl0,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.mip_views[0]) },
                wgpu::BindGroupEntry { binding: 2, resource: self.owner.as_entire_binding() },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clipmap-gather") });
        if !jobs.is_empty() {
            let jobs_buffer = job_buffer(ctx, "clipmap-jobs", align, jobs.iter().map(|(_, b)| b.as_slice()));
            let clears: Vec<Vec<u8>> = plan
                .levels
                .iter()
                .enumerate()
                .flat_map(|(k, l)| l.dirty.iter().map(move |d| (k, *d)))
                .map(|(k, d)| clear_bytes(&d, k as u32, self.texels))
                .collect();
            let clear_buffer = job_buffer(ctx, "clipmap-clears", align, clears.iter().map(Vec::as_slice));
            let clear_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clipmap-gather-1"),
                layout: &self.gather_bgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &clear_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(JOB_BYTES),
                    }),
                }],
            });
            let bind1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clipmap-gather-1"),
                layout: &self.gather_bgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &jobs_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(JOB_BYTES),
                    }),
                }],
            });
            let block_binds: BTreeMap<usize, wgpu::BindGroup> = buffers
                .iter()
                .map(|(block, buffer)| {
                    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("clipmap-gather-2"),
                        layout: &self.gather_bgl2,
                        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
                    });
                    (*block, bind)
                })
                .collect();
            let any_block = block_binds.values().next().expect("a job has a block");
            // Three passes, so no texel is written by two dispatches that the adapter may run in either order: clear the owner of every
            // dirty texel, let every job claim the texels it covers (the highest job index wins: the finest block), then let each job
            // write the texels it owns.
            for (label, pipeline, pick) in [
                ("clipmap-clear", &self.clear_pipeline, 0),
                ("clipmap-claim", &self.claim_pipeline, 1),
                ("clipmap-gather", &self.gather_pipeline, 2),
            ] {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(label), timestamp_writes: None });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind0, &[]);
                if pick == 0 {
                    for (idx, bytes) in clears.iter().enumerate() {
                        let dirty = job_dirty(bytes);
                        pass.set_bind_group(1, &clear_bind, &[(idx as u64 * align) as u32]);
                        pass.set_bind_group(2, any_block, &[]);
                        pass.dispatch_workgroups(((dirty.x1 - dirty.x0) as u32).div_ceil(8), ((dirty.y1 - dirty.y0) as u32).div_ceil(8), 1);
                    }
                    continue;
                }
                for (idx, (block, bytes)) in jobs.iter().enumerate() {
                    let dirty = job_dirty(bytes);
                    pass.set_bind_group(1, &bind1, &[(idx as u64 * align) as u32]);
                    pass.set_bind_group(2, if pick == 1 { any_block } else { &block_binds[block] }, &[]);
                    pass.dispatch_workgroups(((dirty.x1 - dirty.x0) as u32).div_ceil(8), ((dirty.y1 - dirty.y0) as u32).div_ceil(8), 1);
                }
            }
        }
        ctx.queue.submit([encoder.finish()]);

        // The mips: one pass a mip, so a mip is read only after the one below it is written.
        let mut rects: Vec<Vec<TexelRect>> =
            plan.levels.iter().map(|l| l.dirty.iter().filter(|r| !r.is_empty()).copied().collect()).collect();
        for m in 1..self.mips {
            let edge = n >> m;
            let mut mip_jobs: Vec<Vec<u8>> = Vec::new();
            for (k, level_rects) in rects.iter_mut().enumerate() {
                for r in level_rects.iter_mut() {
                    *r = parent_rect(r, edge);
                    let mut bytes = Vec::with_capacity(MIP_JOB_BYTES as usize);
                    for v in [r.x0 as i32, r.y0 as i32, r.x1 as i32, r.y1 as i32] {
                        bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    for v in [k as u32, edge as u32, 0, 0] {
                        bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    mip_jobs.push(bytes);
                }
            }
            if mip_jobs.is_empty() {
                continue;
            }
            let align = align.max(MIP_JOB_BYTES);
            let job_buffer = job_buffer(ctx, "clipmap-mip-jobs", align, mip_jobs.iter().map(Vec::as_slice));
            let bind0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clipmap-mip-0"),
                layout: &self.mip_bgl0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.mip_views[m as usize - 1]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.mip_views[m as usize]) },
                ],
            });
            let bind1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("clipmap-mip-1"),
                layout: &self.mip_bgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &job_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(MIP_JOB_BYTES),
                    }),
                }],
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clipmap-mip") });
            {
                let mut pass =
                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("clipmap-mip"), timestamp_writes: None });
                pass.set_pipeline(&self.mip_pipeline);
                pass.set_bind_group(0, &bind0, &[]);
                for (idx, bytes) in mip_jobs.iter().enumerate() {
                    let w = i32::from_le_bytes(bytes[8..12].try_into().unwrap()) - i32::from_le_bytes(bytes[0..4].try_into().unwrap());
                    let h = i32::from_le_bytes(bytes[12..16].try_into().unwrap()) - i32::from_le_bytes(bytes[4..8].try_into().unwrap());
                    pass.set_bind_group(1, &bind1, &[(idx as u64 * align) as u32]);
                    pass.dispatch_workgroups((w as u32).div_ceil(8), (h as u32).div_ceil(8), 1);
                }
            }
            ctx.queue.submit([encoder.finish()]);
        }
        skipped.sort();
        skipped.dedup();
        skipped
    }

    /// The texels of one level at one mip, row-major, `(texels >> mip)^2` values: a test readback.
    pub fn read_level(&self, ctx: &GpuContext, level: usize, mip: u32) -> Vec<f32> {
        read_texture_layer(ctx, &self.texture, self.texels, level, mip)
    }
}

/// One layer of an `R32Float` array texture at one mip, row-major.
fn read_texture_layer(ctx: &GpuContext, texture: &wgpu::Texture, texels: u32, layer: usize, mip: u32) -> Vec<f32> {
    let edge = (texels >> mip).max(1);
    let row = (edge * 4).next_multiple_of(256);
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("clipmap-readback"),
        size: u64::from(row) * u64::from(edge),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("clipmap-readback") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: mip,
            origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(edge) },
        },
        wgpu::Extent3d { width: edge, height: edge, depth_or_array_layers: 1 },
    );
    ctx.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("readback range").to_vec();
    buffer.unmap();
    let mut out = Vec::with_capacity((edge * edge) as usize);
    for y in 0..edge as usize {
        for x in 0..edge as usize {
            let at = y * row as usize + x * 4;
            out.push(f32::from_le_bytes(data[at..at + 4].try_into().unwrap()));
        }
    }
    out
}

struct DirtyRect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

fn job_dirty(bytes: &[u8]) -> DirtyRect {
    let at = |k: usize| i32::from_le_bytes(bytes[k * 4..k * 4 + 4].try_into().unwrap());
    DirtyRect { x0: at(4), y0: at(5), x1: at(6), y1: at(7) }
}

fn job_buffer<'a>(ctx: &GpuContext, label: &str, align: u64, jobs: impl Iterator<Item = &'a [u8]> + Clone) -> wgpu::Buffer {
    let count = jobs.clone().count() as u64;
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count * align).max(align),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut all = vec![0u8; (count * align) as usize];
    for (idx, bytes) in jobs.enumerate() {
        all[idx * align as usize..idx * align as usize + bytes.len()].copy_from_slice(bytes);
    }
    ctx.queue.write_buffer(&buffer, 0, &all);
    buffer
}

#[allow(clippy::too_many_arguments)]
fn job_bytes(b: &BlockMap, cut: &TexelRect, first_word: u32, layer: u32, flag: u8, texel_m: f64, texels: u32, id: u32) -> Vec<u8> {
    let margin = 0.25 * (b.u[1].abs() + b.u[2].abs()).max(b.v[1].abs() + b.v[2].abs());
    let mut out = Vec::with_capacity(JOB_BYTES as usize);
    for v in
        [b.rect.x0 as i32, b.rect.y0 as i32, b.rect.x1 as i32, b.rect.y1 as i32, cut.x0 as i32, cut.y0 as i32, cut.x1 as i32, cut.y1 as i32]
    {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [b.u[0], b.u[1], b.u[2], margin, b.v[0], b.v[1], b.v[2], texel_m as f32, b.drop[0], b.drop[1], b.drop[2], b.drop[3]] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [first_word, layer, u32::from(flag), texels] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&[0u8; 12]);
    debug_assert_eq!(out.len() as u64, JOB_BYTES);
    out
}

/// A job that only names the texels to clear: the dirty rectangle, the layer and the level's edge.
fn clear_bytes(d: &TexelRect, layer: u32, texels: u32) -> Vec<u8> {
    let mut out = vec![0u8; JOB_BYTES as usize];
    for (k, v) in [d.x0 as i32, d.y0 as i32, d.x1 as i32, d.y1 as i32].into_iter().enumerate() {
        out[16 + k * 4..20 + k * 4].copy_from_slice(&v.to_le_bytes());
    }
    out[84..88].copy_from_slice(&layer.to_le_bytes());
    out[92..96].copy_from_slice(&texels.to_le_bytes());
    out
}

/// Texels per cube face edge (REND-048).
pub const COARSE_CUBE_TEXELS: u32 = 512;

const CUBE_JOB_BYTES: u64 = 32;

const CUBE_HEAD: &str = r#"
@group(0) @binding(0) var cube: texture_storage_2d_array<r32float, write>;
struct CubeJob {
    // The tile's first cube texel on its face, the tile's edge in cube texels and the face.
    tile: vec4<i32>,
    // The slot's first word.
    misc: vec4<u32>,
};
@group(1) @binding(0) var<uniform> job: CubeJob;
"#;

const CUBE_MAIN: &str = r#"
// One invocation a cube texel of the tile: the maximum of the lattice samples its footprint touches (the corners of the cells it meets:
// a bilinear surface has its maximum over a cell at a corner), above the sphere.
@compute @workgroup_size(8, 8, 1)
fn cube_max(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = job.tile.z;
    if (i32(gid.x) >= size || i32(gid.y) >= size) {
        return;
    }
    load_tile(job.misc.x);
    let lx = i32(gid.x);
    let ly = i32(gid.y);
    let i0 = (lx * CELLS) / size;
    let i1 = min(((lx + 1) * CELLS + size - 1) / size, CELLS);
    let j0 = (ly * CELLS) / size;
    let j1 = min(((ly + 1) * CELLS + size - 1) / size, CELLS);
    var best = -3.4e38;
    for (var j = j0; j <= j1; j = j + 1) {
        for (var i = i0; i <= i1; i = i + 1) {
            best = max(best, rel_height(texel(i, j)));
        }
    }
    textureStore(cube, vec2<i32>(job.tile.x + lx, job.tile.y + ly), job.tile.w, vec4<f32>(h_origin + best, 0.0, 0.0, 0.0));
}
"#;

/// The cube's gather shader for tiles of `cells`.
pub fn cube_shader(cells: u32) -> String {
    format!("{CUBE_HEAD}{}{CUBE_MAIN}", slot_access_wgsl(cells))
}

const CUBE_MIP_SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d_array<f32>;
@group(0) @binding(1) var dst: texture_storage_2d_array<r32float, write>;
struct MipJob {
    // The layer and the child's edge in texels.
    params: vec4<u32>,
};
@group(1) @binding(0) var<uniform> job: MipJob;

// Each child texel is the maximum over the parent texels of its 2 x 2 footprint dilated by one texel, clamped to the face.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = i32(job.params.y);
    if (i32(gid.x) >= n || i32(gid.y) >= n) {
        return;
    }
    let cx = i32(gid.x);
    let cy = i32(gid.y);
    var best = -3.4e38;
    for (var dy = -1; dy <= 2; dy = dy + 1) {
        for (var dx = -1; dx <= 2; dx = dx + 1) {
            let p = vec2<i32>(clamp(2 * cx + dx, 0, 2 * n - 1), clamp(2 * cy + dy, 0, 2 * n - 1));
            best = max(best, textureLoad(src, p, i32(job.params.x), 0).x);
        }
    }
    textureStore(dst, vec2<i32>(cx, cy), i32(job.params.x), vec4<f32>(best, 0.0, 0.0, 0.0));
}
"#;

/// The cube mip shader (a function so the shader scan can reach it).
pub fn cube_mip_shader() -> String {
    CUBE_MIP_SHADER.to_string()
}

/// The coarse max-height cube (REND-048, ADR 0085 point 4): six faces of `texels`^2 `R32Float` with max mips, each texel the maximum height
/// above the sphere of the tiles over it, taken over the lattice samples a texel's footprint touches (not over finer tiles drawn there).
/// The caller passes every floor tile (disjoint footprints, covering every face) and rebuilds when one changes.
///
/// Layout for the march (REND-049): a 2D array texture, not a cube view; layer = the `planet_core` face index; texel `x` runs along the face
/// coordinate `s` and row `y` along `t`; texels are uniform in the face coordinates before the `TangentWarp`. The values are heights above
/// the sphere, not drop-corrected: beyond the clipmap the march adds the sphere's drop itself (with a curvature coefficient as a uniform,
/// CON-11). The mips stop at the face edge (clamped), so a walk within one coarse texel of a face edge reads mip 0 or the neighbour face.
/// Texels no tile has written read [`NO_DATA_M`].
pub struct CoarseCubeGpu {
    cells: u32,
    texels: u32,
    mips: u32,
    texture: wgpu::Texture,
    mip_views: Vec<wgpu::TextureView>,
    cube_pipeline: wgpu::ComputePipeline,
    cube_bgl0: wgpu::BindGroupLayout,
    cube_bgl1: wgpu::BindGroupLayout,
    cube_bgl2: wgpu::BindGroupLayout,
    mip_pipeline: wgpu::ComputePipeline,
    mip_bgl0: wgpu::BindGroupLayout,
    mip_bgl1: wgpu::BindGroupLayout,
}

impl CoarseCubeGpu {
    pub fn new(ctx: &GpuContext, texels: u32, cells: u32) -> CoarseCubeGpu {
        assert!(texels.is_power_of_two() && texels >= 2, "cube edge must be a power of two");
        let device = &ctx.device;
        let mips = texels.trailing_zeros() + 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("coarse-cube"),
            size: wgpu::Extent3d { width: texels, height: texels, depth_or_array_layers: 6 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CLIPMAP_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for m in 0..mips {
            let edge = (texels >> m).max(1);
            let data = vec![NO_DATA_M; (edge * edge * 6) as usize].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: m,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(edge * 4), rows_per_image: Some(edge) },
                wgpu::Extent3d { width: edge, height: edge, depth_or_array_layers: 6 },
            );
        }
        let mip_views = (0..mips)
            .map(|m| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("coarse-cube-mip"),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: m,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let c = wgpu::ShaderStages::COMPUTE;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: c, ty, count: None };
        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: None };
        let storage_tex = wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: CLIPMAP_FORMAT,
            view_dimension: wgpu::TextureViewDimension::D2Array,
        };
        let bgl = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        let cube_bgl0 = bgl("coarse-cube-0", &[entry(0, storage_tex)]);
        let cube_bgl1 = bgl("coarse-cube-1", &[entry(0, uniform)]);
        let cube_bgl2 = bgl(
            "coarse-cube-2",
            &[entry(
                0,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )],
        );
        let mip_bgl0 = bgl(
            "coarse-cube-mip-0",
            &[
                entry(
                    0,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(1, storage_tex),
            ],
        );
        let mip_bgl1 = bgl("coarse-cube-mip-1", &[entry(0, uniform)]);
        let pipeline = |label, entry_point: &str, layouts: &[&wgpu::BindGroupLayout], source: String| {
            let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &layouts,
                immediate_size: 0,
            });
            let module = device
                .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let cube_pipeline = pipeline("coarse-cube", "cube_max", &[&cube_bgl0, &cube_bgl1, &cube_bgl2], cube_shader(cells));
        let mip_pipeline = pipeline("coarse-cube-mip", "main", &[&mip_bgl0, &mip_bgl1], cube_mip_shader());
        CoarseCubeGpu {
            cells,
            texels,
            mips,
            texture,
            mip_views,
            cube_pipeline,
            cube_bgl0,
            cube_bgl1,
            cube_bgl2,
            mip_pipeline,
            mip_bgl0,
            mip_bgl1,
        }
    }

    /// Rewrite the whole cube from `tiles` (the floor tiles, disjoint footprints, each of a level at most `log2(texels)`), then its mips.
    /// A tile that is not resident in `terrain` is skipped and returned; its texels stay as they were (NO_DATA_M at first) until it is.
    pub fn rebuild(&mut self, ctx: &GpuContext, terrain: &TerrainRenderer, tiles: &[TileId]) -> Vec<TileId> {
        assert_eq!(terrain.compact_cells(), self.cells, "the cube was made for tiles of another size");
        let device = &ctx.device;
        let align = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(CUBE_JOB_BYTES);
        let mut skipped = Vec::new();
        let mut jobs: Vec<(usize, Vec<u8>)> = Vec::new();
        let mut buffers: BTreeMap<usize, &wgpu::Buffer> = BTreeMap::new();
        let mut area = 0u64;
        let mut claimed: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
        for &t in tiles {
            assert!(u32::from(t.level()) <= self.texels.trailing_zeros(), "tile {t:?} is finer than a cube texel");
            let size = self.texels >> t.level();
            let (x0, y0) = (t.x() * size, t.y() * size);
            let face = u32::from(t.face().0);
            assert!(
                claimed.iter().all(|&(f, ax, ay, ax1, ay1)| f != face || x0 >= ax1 || ax >= x0 + size || y0 >= ay1 || ay >= y0 + size),
                "floor tiles overlap at {t:?}"
            );
            claimed.push((face, x0, y0, x0 + size, y0 + size));
            area += u64::from(size) * u64::from(size);
            let Some((block, buffer, first_word)) = terrain.slot_words(t) else {
                skipped.push(t);
                continue;
            };
            buffers.insert(block, buffer);
            let mut bytes = Vec::with_capacity(CUBE_JOB_BYTES as usize);
            for v in [x0 as i32, y0 as i32, size as i32, face as i32] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            for v in [first_word, 0, 0, 0] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            jobs.push((block, bytes));
        }
        assert_eq!(area, 6 * u64::from(self.texels) * u64::from(self.texels), "the floor tiles must cover every face of the cube");
        if !jobs.is_empty() {
            let jobs_buffer = job_buffer(ctx, "coarse-cube-jobs", align, jobs.iter().map(|(_, b)| b.as_slice()));
            let bind0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("coarse-cube-0"),
                layout: &self.cube_bgl0,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.mip_views[0]) }],
            });
            let bind1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("coarse-cube-1"),
                layout: &self.cube_bgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &jobs_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(CUBE_JOB_BYTES),
                    }),
                }],
            });
            let block_binds: BTreeMap<usize, wgpu::BindGroup> = buffers
                .iter()
                .map(|(block, buffer)| {
                    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("coarse-cube-2"),
                        layout: &self.cube_bgl2,
                        entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
                    });
                    (*block, bind)
                })
                .collect();
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("coarse-cube") });
            {
                let mut pass =
                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("coarse-cube"), timestamp_writes: None });
                pass.set_pipeline(&self.cube_pipeline);
                pass.set_bind_group(0, &bind0, &[]);
                for (idx, (block, bytes)) in jobs.iter().enumerate() {
                    let size = i32::from_le_bytes(bytes[8..12].try_into().unwrap()) as u32;
                    pass.set_bind_group(1, &bind1, &[(idx as u64 * align) as u32]);
                    pass.set_bind_group(2, &block_binds[block], &[]);
                    pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 1);
                }
            }
            ctx.queue.submit([encoder.finish()]);
        }
        for m in 1..self.mips {
            let edge = self.texels >> m;
            let mip_jobs: Vec<Vec<u8>> = (0..6u32)
                .map(|face| {
                    let mut bytes = Vec::with_capacity(CUBE_JOB_BYTES as usize);
                    for v in [face, edge, 0, 0] {
                        bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    bytes
                })
                .collect();
            let jobs_buffer = job_buffer(ctx, "coarse-cube-mip-jobs", align, mip_jobs.iter().map(Vec::as_slice));
            let bind0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("coarse-cube-mip-0"),
                layout: &self.mip_bgl0,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.mip_views[m as usize - 1]) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.mip_views[m as usize]) },
                ],
            });
            let bind1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("coarse-cube-mip-1"),
                layout: &self.mip_bgl1,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &jobs_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                }],
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("coarse-cube-mip") });
            {
                let mut pass =
                    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("coarse-cube-mip"), timestamp_writes: None });
                pass.set_pipeline(&self.mip_pipeline);
                pass.set_bind_group(0, &bind0, &[]);
                for idx in 0..6u64 {
                    pass.set_bind_group(1, &bind1, &[(idx * align) as u32]);
                    pass.dispatch_workgroups(edge.div_ceil(8), edge.div_ceil(8), 1);
                }
            }
            ctx.queue.submit([encoder.finish()]);
        }
        skipped
    }

    pub fn texels(&self) -> u32 {
        self.texels
    }

    /// One face at one mip, row-major: a test readback.
    pub fn read_face(&self, ctx: &GpuContext, face: usize, mip: u32) -> Vec<f32> {
        read_texture_layer(ctx, &self.texture, self.texels, face, mip)
    }
}
