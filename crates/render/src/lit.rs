//! The lit path's post chain (ADR 0038, ADR 0042 points 1-3): the `Rgba16Float` scene target, the luminance histogram and the
//! exposure adaptation (compute), and the tone map into the presentable `Rgba8Unorm` target (ACES fit and sRGB encode in the
//! shader). The renderer executes the compiled graph (`graph::lit_frame_graph`) and decides nothing (CON-03): the exposure mode,
//! the pre-exposure EV, `dt`, the speeds and the clamps arrive in the [`LitPlan`]; no wall clock, no readback. The maths each
//! shader implements has its f64 reference in `planet_frame::look`.
//!
//! Class and bounds (ADR 0035): presentation data. The tone-map pass matches `look::tonemap_pixel` within 1/255 per channel
//! (REND-023); the histogram counts match `look::histogram_of` exactly on pixels away from bin edges, and the adaptation matches
//! `look::adapt_ev` within 1e-4 EV (REND-022). Nothing here uses planet-sized values (CON-11).

use crate::graph::{lit_frame_graph, CompiledGraph, LitShape, RenderGraph};
use crate::{GpuContext, RgbaFrame};
use planet_frame::look::{self, ExposureMode, ExposureParams, LitPlan, ToneMapMode};

const COMMON: &str = r#"
struct Params {
    // x: pre-exposure EV100, y: minimum EV100, z: maximum EV100, w: dt in seconds
    a: vec4<f32>,
    // x: speed up, y: speed down, z: the fixed EV100, w: EV offset of the histogram (log2(1.2) + pre-exposure EV100)
    b: vec4<f32>,
    // x: adaptive (1) or fixed (0), y: reset, z: tone map mode (0 ACES, 1 EV bands), w: unused
    c: vec4<u32>,
    // The histogram axis from the look data: x: lowest EV, y: bins per stop (64 / span), z: stops per bin (span / 64), w: unused
    d: vec4<f32>,
    // The histogram band from the look data: x: lower fraction, y: upper fraction, zw: unused
    e: vec4<f32>,
};
"#;

const HISTOGRAM: &str = r#"
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> hist: array<atomic<u32>, 64>;
var<workgroup> local_bins: array<atomic<u32>, 64>;

// 64 bins of EV100 = log2(8 L) over the look data's axis (Earth: -10 to +20, ADR 0038 point 2). Pixels without light are not counted.
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    if (li < 64u) {
        atomicStore(&local_bins[li], 0u);
    }
    workgroupBarrier();
    let dims = textureDimensions(hdr);
    if (gid.x < dims.x && gid.y < dims.y) {
        let c = textureLoad(hdr, vec2<i32>(gid.xy), 0).rgb;
        let y = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        if (y > 0.0) {
            let ev = log2(8.0 * y) + p.b.w;
            let b = clamp(floor((ev - p.d.x) * p.d.y), 0.0, 63.0);
            atomicAdd(&local_bins[u32(b)], 1u);
        }
    }
    workgroupBarrier();
    if (li < 64u) {
        let n = atomicLoad(&local_bins[li]);
        if (n > 0u) {
            atomicAdd(&hist[li], n);
        }
    }
}
"#;

const ADAPT: &str = r#"
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> hist: array<u32, 64>;
// x: the adapted EV100, y: the target EV100, z: the factor from the pre-exposed value to the exposed one (2^(pre - ev)), w: unused
@group(0) @binding(2) var<storage, read_write> state: vec4<f32>;

@compute @workgroup_size(1)
fn main() {
    let pre = p.a.x;
    let cur = state.x;
    var ev = p.b.z;
    var tgt = p.b.z;
    if (p.c.x != 0u) {
        var total = 0.0;
        for (var i = 0u; i < 64u; i = i + 1u) {
            total = total + f32(hist[i]);
        }
        let lo = p.e.x * total;
        let hi = p.e.y * total;
        var below = 0.0;
        var weight = 0.0;
        var sum = 0.0;
        for (var i = 0u; i < 64u; i = i + 1u) {
            let c = f32(hist[i]);
            let a = below;
            let b = below + c;
            let overlap = max(min(b, hi) - max(a, lo), 0.0);
            weight = weight + overlap;
            sum = sum + overlap * (p.d.x + (f32(i) + 0.5) * p.d.z);
            below = b;
        }
        tgt = cur;
        if (weight > 0.0) {
            tgt = clamp(sum / weight, p.a.y, p.a.z);
        }
        ev = tgt;
        if (p.c.y == 0u) {
            let dt = max(p.a.w, 0.0);
            ev = cur + clamp(tgt - cur, -(p.b.y * dt), p.b.x * dt);
        }
    }
    state = vec4<f32>(ev, tgt, exp2(pre - ev), 0.0);
}
"#;

const TONEMAP: &str = r#"
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var hdr: texture_2d<f32>;
@group(0) @binding(2) var<storage, read> state: vec4<f32>;

struct V { @builtin(position) pos: vec4<f32> };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    // One triangle covering the target.
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: V;
    o.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

// The ACES filmic fit (Narkowicz 2015) on exposed linear colour.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let m = max(x, vec3<f32>(0.0));
    return clamp((m * (2.51 * m + 0.03)) / (m * (2.43 * m + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The sRGB transfer function, in the shader, so the window and the goldens encode identically (ADR 0038 point 3).
fn srgb(l: vec3<f32>) -> vec3<f32> {
    let c = clamp(l, vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs(v: V) -> @location(0) vec4<f32> {
    let exposed = textureLoad(hdr, vec2<i32>(v.pos.xy), 0).rgb * state.z;
    if (p.c.z == 1u) {
        let y = dot(exposed, vec3<f32>(0.2126, 0.7152, 0.0722));
        if (!(y > 0.0)) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        let band = u32(clamp(floor(max(-log2(y), 0.0)), 0.0, {BAND_MAX}.0));
        return vec4<f32>(band_colors[band], 1.0);
    }
    return vec4<f32>(srgb(aces(exposed)), 1.0);
}
"#;

/// The tone-map shader with the band colours of `look::EV_BAND_COLORS` spliced in.
pub fn tonemap_shader() -> String {
    let colors: Vec<String> =
        look::EV_BAND_COLORS.iter().map(|c| format!("vec3<f32>({}.0 / 255.0, {}.0 / 255.0, {}.0 / 255.0)", c[0], c[1], c[2])).collect();
    let table = format!("const band_colors = array<vec3<f32>, {}>(\n    {}\n);\n", look::EV_BAND_COUNT, colors.join(",\n    "));
    let body = TONEMAP.replace("{BAND_MAX}", &(look::EV_BAND_COUNT - 1).to_string());
    format!("{COMMON}{table}{body}")
}

/// The compute shaders of the exposure, with the common header.
pub fn histogram_shader() -> String {
    format!("{COMMON}{HISTOGRAM}")
}

pub fn adapt_shader() -> String {
    format!("{COMMON}{ADAPT}")
}

/// Bytes of the parameter block (five `vec4`).
const PARAMS_BYTES: u64 = 80;

/// What the exposure adaptation holds between frames, read back by tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureState {
    /// The adapted EV100 the frame was exposed at.
    pub ev100: f32,
    /// The EV100 the histogram asked for (the fixed EV100 in fixed mode).
    pub target_ev100: f32,
    /// The factor from the pre-exposed scene value to the exposed one.
    pub factor: f32,
}

pub struct LitPath {
    size: (u32, u32),
    hdr: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    histogram: wgpu::Buffer,
    state: wgpu::Buffer,
    params: wgpu::Buffer,
    hist_pipeline: wgpu::ComputePipeline,
    hist_bind: wgpu::BindGroup,
    adapt_pipeline: wgpu::ComputePipeline,
    adapt_bind: wgpu::BindGroup,
    tone_pipeline: wgpu::RenderPipeline,
    tone_bind: wgpu::BindGroup,
    /// The compiled frame for the current shape, recompiled only when the shape changes.
    compiled: Option<(LitShape, RenderGraph, CompiledGraph)>,
    compiles: u32,
    /// Whether a lit frame has run: the first one snaps the exposure and seeds the history.
    has_state: bool,
    /// Bytes this path allocated (targets and buffers).
    allocated: u64,
}

impl LitPath {
    /// Create the scene target, the exposure buffers and the three pipelines for frames of `size`. Everything long-lived is made
    /// here, so no lit frame creates a pipeline or a target (REND-017).
    pub fn new(ctx: &GpuContext, size: (u32, u32)) -> Self {
        let device = &ctx.device;
        let hdr = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lit-hdr"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let hdr_view = hdr.create_view(&Default::default());
        let buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        };
        let histogram = buffer(
            "lit-histogram",
            crate::graph::HISTOGRAM_BYTES,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        );
        let state = buffer(
            "lit-exposure-state",
            crate::graph::EXPOSURE_STATE_BYTES,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        );
        let params = buffer("lit-params", PARAMS_BYTES, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);

        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let uniform = |binding, vis| {
            entry(
                binding,
                vis,
                wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            )
        };
        let storage = |binding, vis, read_only| {
            entry(
                binding,
                vis,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
        };
        let texture = |binding, vis| {
            entry(
                binding,
                vis,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            )
        };
        let (c, f) = (wgpu::ShaderStages::COMPUTE, wgpu::ShaderStages::FRAGMENT);
        let module = |label, source: String| {
            device
                .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
        };
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            (bgl, pl)
        };
        let bind = |label, bgl: &wgpu::BindGroupLayout, resources: [wgpu::BindingResource; 3]| {
            let entries: Vec<wgpu::BindGroupEntry> =
                resources.into_iter().enumerate().map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource }).collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout: bgl, entries: &entries })
        };

        let (hist_bgl, hist_pl) = layout("lit-histogram", &[uniform(0, c), texture(1, c), storage(2, c, false)]);
        let hist_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lit-histogram"),
            layout: Some(&hist_pl),
            module: &module("lit-histogram", histogram_shader()),
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let hist_bind = bind(
            "lit-histogram",
            &hist_bgl,
            [params.as_entire_binding(), wgpu::BindingResource::TextureView(&hdr_view), histogram.as_entire_binding()],
        );

        let (adapt_bgl, adapt_pl) = layout("lit-adapt", &[uniform(0, c), storage(1, c, true), storage(2, c, false)]);
        let adapt_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lit-adapt"),
            layout: Some(&adapt_pl),
            module: &module("lit-adapt", adapt_shader()),
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let adapt_bind =
            bind("lit-adapt", &adapt_bgl, [params.as_entire_binding(), histogram.as_entire_binding(), state.as_entire_binding()]);

        let (tone_bgl, tone_pl) = layout("lit-tonemap", &[uniform(0, f), texture(1, f), storage(2, f, true)]);
        let tone_module = module("lit-tonemap", tonemap_shader());
        let tone_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lit-tonemap"),
            layout: Some(&tone_pl),
            vertex: wgpu::VertexState {
                module: &tone_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &tone_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: LDR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let tone_bind = bind(
            "lit-tonemap",
            &tone_bgl,
            [params.as_entire_binding(), wgpu::BindingResource::TextureView(&hdr_view), state.as_entire_binding()],
        );

        let allocated = u64::from(size.0) * u64::from(size.1) * crate::graph::HDR_BYTES_PER_PIXEL
            + crate::graph::HISTOGRAM_BYTES
            + crate::graph::EXPOSURE_STATE_BYTES;
        Self {
            size,
            hdr,
            hdr_view,
            histogram,
            state,
            params,
            hist_pipeline,
            hist_bind,
            adapt_pipeline,
            adapt_bind,
            tone_pipeline,
            tone_bind,
            compiled: None,
            compiles: 0,
            has_state: false,
            allocated,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The scene target the terrain pass draws into.
    pub(crate) fn hdr_view(&self) -> &wgpu::TextureView {
        &self.hdr_view
    }

    /// Bytes of the scene target, the histogram and the exposure history this path allocated (the depth target is the terrain
    /// renderer's). With the depth target it equals `frame_target_bytes` of the compiled adaptive graph.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated
    }

    /// How often the frame graph was compiled: once per frame shape, not per frame (REND-019).
    pub fn graph_compiles(&self) -> u32 {
        self.compiles
    }

    /// The compiled graph of the last frame.
    pub fn compiled_graph(&self) -> Option<&CompiledGraph> {
        self.compiled.as_ref().map(|(_, _, c)| c)
    }

    /// The pass names of the last frame in execution order.
    pub fn executed_passes(&self) -> Vec<&'static str> {
        self.compiled.as_ref().map(|(_, g, c)| c.order.iter().map(|&i| g.passes()[i].name).collect()).unwrap_or_default()
    }

    fn params_bytes(plan: &LitPlan, reset: bool) -> [u8; PARAMS_BYTES as usize] {
        let (adaptive, params, fixed_ev) = match plan.exposure {
            ExposureMode::Fixed { ev100 } => (0u32, ExposureParams::default(), ev100),
            ExposureMode::Adaptive { params, .. } => (1u32, params, 0.0),
        };
        let pre = plan.exposure.pre_ev100();
        let offset = look::histogram_ev_offset(f64::from(pre)) as f32;
        let f = [pre, params.min_ev100, params.max_ev100, plan.dt_s, params.speed_up, params.speed_down, fixed_ev, offset];
        let u = [adaptive, u32::from(reset), if plan.tonemap == ToneMapMode::EvBands { 1 } else { 0 }, 0];
        // The histogram axis and band of the look data, rounded once from f64 (the CPU decides them, CON-03).
        let g = plan.histogram;
        let span = g.ev_max - g.ev_min;
        let d = [g.ev_min as f32, (look::HIST_BINS as f64 / span) as f32, (span / look::HIST_BINS as f64) as f32, 0.0];
        let e = [g.band_low as f32, g.band_high as f32, 0.0, 0.0];
        let mut bytes = [0u8; PARAMS_BYTES as usize];
        for (i, v) in f.iter().enumerate() {
            bytes[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in u.iter().enumerate() {
            bytes[32 + 4 * i..32 + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in d.iter().chain(e.iter()).enumerate() {
            bytes[48 + 4 * i..48 + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes
    }

    /// Record the passes after the terrain pass into `encoder`, in the order of the compiled graph, ending with the tone map into
    /// `target` (an `Rgba8Unorm` view of this path's size). Compiles the frame graph when the shape (size, exposure mode) differs
    /// from the last frame's.
    pub(crate) fn record_post(&mut self, ctx: &GpuContext, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, plan: &LitPlan) {
        let shape = LitShape { size: self.size, adaptive: plan.exposure.is_adaptive() };
        if self.compiled.as_ref().map(|(s, _, _)| *s) != Some(shape) {
            let graph = lit_frame_graph(&shape);
            let compiled = graph.compile().unwrap_or_else(|e| panic!("the lit frame graph does not compile: {e}"));
            self.compiled = Some((shape, graph, compiled));
            self.compiles += 1;
        }
        // The first lit frame seeds the history at the pre-exposure and snaps to the target, whatever the plan says.
        let reset = plan.reset_exposure || !self.has_state;
        if !self.has_state {
            let pre = plan.exposure.pre_ev100();
            let seed: Vec<u8> = [pre, pre, 1.0f32, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect();
            ctx.queue.write_buffer(&self.state, 0, &seed);
            self.has_state = true;
        }
        // The queue orders this write before the submission that holds the passes below, and after every earlier submission, so
        // frames in flight read their own parameters.
        ctx.queue.write_buffer(&self.params, 0, &Self::params_bytes(plan, reset));
        let (_, graph, compiled) = self.compiled.as_ref().expect("compiled above");
        for &i in &compiled.order {
            match graph.passes()[i].name {
                // Recorded by the terrain renderer into `hdr_view`.
                "terrain" => {}
                "exposure-histogram" => {
                    encoder.clear_buffer(&self.histogram, 0, None);
                    let mut pass = encoder
                        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("exposure-histogram"), timestamp_writes: None });
                    pass.set_pipeline(&self.hist_pipeline);
                    pass.set_bind_group(0, &self.hist_bind, &[]);
                    pass.dispatch_workgroups(self.size.0.div_ceil(16), self.size.1.div_ceil(16), 1);
                }
                "exposure-adapt" => {
                    let mut pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("exposure-adapt"), timestamp_writes: None });
                    pass.set_pipeline(&self.adapt_pipeline);
                    pass.set_bind_group(0, &self.adapt_bind, &[]);
                    pass.dispatch_workgroups(1, 1, 1);
                }
                "tonemap" => {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("tonemap"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: target,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&self.tone_pipeline);
                    pass.set_bind_group(0, &self.tone_bind, &[]);
                    pass.draw(0..3, 0..1);
                }
                other => panic!("the lit executor has no pass '{other}'"),
            }
        }
    }

    /// Write `pixels` (row-major, `size.0 * size.1` RGBA values as written, so already pre-exposed) into the scene target: the
    /// input of the tier B oracles.
    pub fn upload_hdr(&self, ctx: &GpuContext, pixels: &[[f32; 4]]) {
        assert_eq!(pixels.len() as u64, u64::from(self.size.0) * u64::from(self.size.1), "one RGBA value per pixel");
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.iter().flat_map(|v| f32_to_f16_bits(*v).to_le_bytes())).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.hdr, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.size.0 * 8), rows_per_image: Some(self.size.1) },
            wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
        );
    }

    /// Run the post chain on the scene target as it is and return the tone-mapped image (tests and one-shot tools).
    pub fn tone_map_to_image(&mut self, ctx: &GpuContext, plan: &LitPlan) -> RgbaFrame {
        let target = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lit-test-target"),
            size: wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: LDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lit-post") });
        self.record_post(ctx, &mut encoder, &view, plan);
        ctx.queue.submit([encoder.finish()]);
        crate::read_rgba8(ctx, &target, self.size.0, self.size.1)
    }

    /// The histogram of the last frame (counts per bin). Meaningful after an adaptive frame.
    pub fn read_histogram(&self, ctx: &GpuContext) -> [u32; look::HIST_BINS] {
        let bytes = read_buffer(ctx, &self.histogram, crate::graph::HISTOGRAM_BYTES);
        let mut h = [0u32; look::HIST_BINS];
        for (i, c) in bytes.as_chunks::<4>().0.iter().enumerate() {
            h[i] = u32::from_le_bytes(*c);
        }
        h
    }

    /// The exposure the last frame used.
    pub fn read_exposure(&self, ctx: &GpuContext) -> ExposureState {
        let bytes = read_buffer(ctx, &self.state, crate::graph::EXPOSURE_STATE_BYTES);
        let f = |i: usize| f32::from_le_bytes([bytes[4 * i], bytes[4 * i + 1], bytes[4 * i + 2], bytes[4 * i + 3]]);
        ExposureState { ev100: f(0), target_ev100: f(1), factor: f(2) }
    }

    /// The scene target as f32 RGBA, row-major.
    pub fn read_hdr(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        let (w, h) = self.size;
        let unpadded = w * 8;
        let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lit-hdr-readback"),
            size: u64::from(padded * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lit-hdr-readback") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &self.hdr, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        ctx.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let data = slice.get_mapped_range().expect("readback range");
        let mut out = Vec::with_capacity((w * h) as usize);
        for row in data.chunks_exact(padded as usize) {
            for px in row[..unpadded as usize].as_chunks::<8>().0 {
                let c = |i: usize| f16_bits_to_f32(u16::from_le_bytes([px[2 * i], px[2 * i + 1]]));
                out.push([c(0), c(1), c(2), c(3)]);
            }
        }
        drop(data);
        readback.unmap();
        out
    }
}

/// The names of the formats and the depth test the lit path draws with (wgpu types stay behind the seam, ADR 0004), for the test
/// that pins them (CON-12, ADR 0038 points 1 and 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetFormats {
    /// The scene target.
    pub scene: String,
    /// The tone map's output (the presentable target): not an sRGB format, the encode is in the shader.
    pub presentable: String,
    /// The depth target of both paths.
    pub depth: String,
    /// The depth comparison (reversed-Z: Greater).
    pub depth_test: String,
}

pub fn target_formats() -> TargetFormats {
    TargetFormats {
        scene: format!("{HDR_FORMAT:?}"),
        presentable: format!("{LDR_FORMAT:?}"),
        depth: format!("{:?}", crate::terrain::DEPTH_FORMAT),
        depth_test: format!("{:?}", crate::terrain::DEPTH_COMPARE),
    }
}

/// Format of the scene target (ADR 0038 point 1).
pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Format of the tone-map output: the encode is in the shader, so the target is not an sRGB format (ADR 0038 point 3).
pub(crate) const LDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn read_buffer(ctx: &GpuContext, buffer: &wgpu::Buffer, bytes: u64) -> Vec<u8> {
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lit-readback"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lit-readback") });
    encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, bytes);
    ctx.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("readback range").to_vec();
    readback.unmap();
    data
}

/// One input of [`probe_lit_radiance`]: the arguments of the terrain shader's `lit_radiance`.
#[derive(Clone, Copy, Debug)]
pub struct LitProbe {
    pub normal: [f32; 3],
    pub height: f32,
    pub range: [f32; 2],
    pub sun: [f32; 3],
    pub factor: f32,
}

/// Evaluate the terrain shader's `lit_radiance` at `probes` on the GPU and return the pre-exposed, clamped radiance of each: the
/// tier B oracle of the lit shading bound (REND-025). The terrain module is compiled with one extra compute entry that calls the
/// function, so the test runs the shader's own code.
pub fn probe_lit_radiance(ctx: &GpuContext, probes: &[LitProbe]) -> Vec<[f32; 3]> {
    use wgpu::util::DeviceExt;
    const PROBE: &str = r#"
struct Probe { n: vec4<f32>, hr: vec4<f32>, sun: vec4<f32>, scale: vec4<f32> };
@group(0) @binding(8) var<storage, read> probes: array<Probe>;
@group(0) @binding(9) var<storage, read_write> results: array<vec4<f32>>;

@compute @workgroup_size(1)
fn probe(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = probes[id.x];
    results[id.x] = vec4<f32>(lit_radiance(p.n.xyz, p.n.w, p.hr.xy, p.sun.xyz, p.scale.x), 1.0);
}
"#;
    let device = &ctx.device;
    let mut input: Vec<u8> = Vec::new();
    for p in probes {
        let words = [
            p.normal[0],
            p.normal[1],
            p.normal[2],
            p.height,
            p.range[0],
            p.range[1],
            0.0,
            0.0,
            p.sun[0],
            p.sun[1],
            p.sun[2],
            0.0,
            p.factor,
            0.0,
            0.0,
            0.0,
        ];
        input.extend(words.iter().flat_map(|v| v.to_le_bytes()));
    }
    let probe_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("lit-probes"),
        contents: &input,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_bytes = (probes.len() * 16) as u64;
    let results = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lit-probe-results"),
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let source = format!(
        "{}
{PROBE}",
        crate::terrain::TERRAIN_SHADERS[0].1
    );
    let module = device
        .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("lit-probe"), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let entry = |binding, read_only| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("lit-probe"),
        entries: &[entry(8, true), entry(9, false)],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lit-probe"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("lit-probe"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("lit-probe"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 8, resource: probe_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 9, resource: results.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lit-probe") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("lit-probe"), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(probes.len() as u32, 1, 1);
    }
    ctx.queue.submit([encoder.finish()]);
    let data = read_buffer(ctx, &results, out_bytes);
    data.as_chunks::<16>()
        .0
        .iter()
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[4 * i], c[4 * i + 1], c[4 * i + 2], c[4 * i + 3]]);
            [f(0), f(1), f(2)]
        })
        .collect()
}

/// IEEE half precision from f32, round to nearest even (values past the half range become infinity).
pub fn f32_to_f16_bits(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let round = u32::from(rem > mid || (rem == mid && (half & 1) == 1));
        return sign | (half + round) as u16;
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    let round = u32::from(rem > 0x1000 || (rem == 0x1000 && (half & 1) == 1));
    sign | (half + round) as u16
}

/// f32 from IEEE half precision.
pub fn f16_bits_to_f32(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = match (exp, man) {
        (0, 0) => sign,
        (0, m) => {
            // Subnormal: normalise.
            let shift = m.leading_zeros() - 21;
            sign | ((127 - 15 + 1 - shift) << 23) | (((m << shift) & 0x3ff) << 13)
        }
        (0x1f, 0) => sign | 0x7f80_0000,
        (0x1f, m) => sign | 0x7f80_0000 | (m << 13),
        (e, m) => sign | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// The shaders of the lit path's post chain, by name, for the validation test.
pub fn lit_post_shaders() -> Vec<(&'static str, String)> {
    vec![("lit-histogram", histogram_shader()), ("lit-adapt", adapt_shader()), ("lit-tonemap", tonemap_shader())]
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: LOOK-006
    #[test]
    fn at_the_earth_defaults_the_histogram_block_holds_the_numbers_of_the_old_literals() {
        let bytes = LitPath::params_bytes(&LitPlan::adaptive(11.0, 0.016), false);
        let words: Vec<u32> = bytes[48..80].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        let want = [-10.0f32, (64.0f64 / 30.0) as f32, 30.0 / 64.0, 0.0, 0.4f32, 0.9f32, 0.0, 0.0].map(f32::to_bits);
        assert_eq!(words, want, "the shader's axis and band are the constants it was written with");
    }

    // spec: REND-022
    #[test]
    fn half_precision_conversion_round_trips_and_rounds_to_nearest_even() {
        for v in [0.0f32, 1.0, -1.0, 0.5, 60_000.0, 65_504.0, 6.1e-5, 5.96e-8, 0.333_251_95, 1024.0] {
            let h = f32_to_f16_bits(v);
            let back = f16_bits_to_f32(h);
            assert!((back - v).abs() <= v.abs() * 1.0e-3 + 1.0e-7, "{v} -> {h:#06x} -> {back}");
        }
        assert_eq!(f32_to_f16_bits(1.0), 0x3c00);
        assert_eq!(f32_to_f16_bits(-2.0), 0xc000);
        assert_eq!(f32_to_f16_bits(65_504.0), 0x7bff);
        assert_eq!(f32_to_f16_bits(1.0e9), 0x7c00, "past the half range is infinity");
        assert_eq!(f16_bits_to_f32(0x3c00), 1.0);
        assert_eq!(f16_bits_to_f32(0x0001), 5.960_464_5e-8, "the smallest subnormal");
        assert_eq!(f16_bits_to_f32(0x0400), 6.103_515_6e-5, "the smallest normal");
        // 1 + 2^-11 is the tie between 1.0 and 1 + 2^-10: round to the even one (1.0); 1 + 3 * 2^-11 rounds up to 1 + 2^-9.
        assert_eq!(f32_to_f16_bits(1.0 + (1.0f32 / 2048.0)), 0x3c00);
        assert_eq!(f32_to_f16_bits(1.0 + 3.0 * (1.0f32 / 2048.0)), 0x3c02);
        assert!(f16_bits_to_f32(f32_to_f16_bits(f32::NAN)).is_nan());
    }
}
