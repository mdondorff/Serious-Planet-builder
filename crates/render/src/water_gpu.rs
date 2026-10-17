//! The water pass's GPU objects and the probes that run its shader functions (see `water` for the shaders and the contract).

use crate::atmosphere::AtmospherePath;
use crate::terrain::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::water::{water_compact_source, water_vertex_source, WATER_MATH};
use crate::GpuContext;
use planet_frame::look::water::{LiquidAppearance, LIQUID_VEC4S, WAVES};
use planet_generators::mesh::MeshVertex;

/// One lighting flavour of the water pass: its group 3 and its pipelines.
pub(crate) struct WaterFlavour {
    pub(crate) bind: wgpu::BindGroup,
    pub(crate) vertex: wgpu::RenderPipeline,
    pub(crate) compact: Option<wgpu::RenderPipeline>,
}

/// The water pass's long-lived GPU objects (made once by `enable_water`, so no frame creates one, REND-017).
pub(crate) struct WaterGpu {
    /// The copy of the scene target the water reads the seabed from.
    pub(crate) copy: wgpu::Texture,
    pub(crate) copy_view: wgpu::TextureView,
    /// Bytes of the copy: a frame target of the share.
    pub(crate) copy_bytes: u64,
    pub(crate) plain: WaterFlavour,
    pub(crate) atmosphere: Option<WaterFlavour>,
    /// The long-lived objects made so far (texture, bind groups, pipelines).
    pub(crate) objects: u32,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// The compact format's two bind group layouts (frame records and block), which the compact water pipeline shares with the terrain's.
pub(crate) struct CompactLayouts<'a> {
    pub(crate) frame: &'a wgpu::BindGroupLayout,
    pub(crate) block: &'a wgpu::BindGroupLayout,
    pub(crate) cells: u32,
}

fn pipelines(
    device: &wgpu::Device,
    bgl0: &wgpu::BindGroupLayout,
    g3: &wgpu::BindGroupLayout,
    compact: Option<&CompactLayouts<'_>>,
    atmosphere: bool,
) -> (wgpu::RenderPipeline, Option<wgpu::RenderPipeline>) {
    let targets = [Some(wgpu::ColorTargetState { format: crate::lit::HDR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })];
    let depth = wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(DEPTH_COMPARE),
        stencil: Default::default(),
        bias: Default::default(),
    };
    let attr = |format, k: usize, loc| wgpu::VertexAttribute { format, offset: 4 * MeshVertex::OFFSETS[k] as u64, shader_location: loc };
    let attrs = [
        attr(wgpu::VertexFormat::Float32x3, 0, 0),
        attr(wgpu::VertexFormat::Float32x3, 1, 1),
        attr(wgpu::VertexFormat::Float32x3, 2, 2),
        attr(wgpu::VertexFormat::Float32, 3, 3),
        attr(wgpu::VertexFormat::Float32, 4, 4),
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float16x2, offset: MeshVertex::STRIDE, shader_location: 5 },
    ];
    let make = |label: &str, source: String, layout: &wgpu::PipelineLayout, buffers: &[Option<wgpu::VertexBufferLayout>]| {
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_water"), compilation_options: Default::default(), buffers },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_water"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let vertex_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("water-layout"),
        bind_group_layouts: &[Some(bgl0), None, None, Some(g3)],
        immediate_size: 0,
    });
    let vertex = make(
        "water-pipeline",
        water_vertex_source(atmosphere),
        &vertex_layout,
        &[Some(wgpu::VertexBufferLayout {
            array_stride: MeshVertex::GPU_STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        })],
    );
    let compact = compact.map(|c| {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water-compact-layout"),
            bind_group_layouts: &[Some(bgl0), Some(c.frame), Some(c.block), Some(g3)],
            immediate_size: 0,
        });
        make("water-compact-pipeline", water_compact_source(c.cells, atmosphere), &layout, &[])
    });
    (vertex, compact)
}

impl WaterGpu {
    /// The copy target, the plain flavour's group 3 and pipelines (the lit path without an atmosphere).
    pub(crate) fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        bgl0: &wgpu::BindGroupLayout,
        compact: Option<&CompactLayouts<'_>>,
    ) -> WaterGpu {
        let copy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water-scene-copy"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::lit::HDR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let copy_view = copy.create_view(&Default::default());
        let g3 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("water-plain"), entries: &[texture_entry(4)] });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water-plain"),
            layout: &g3,
            entries: &[wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&copy_view) }],
        });
        let (vertex, compact_pipeline) = pipelines(device, bgl0, &g3, compact, false);
        // The copy, the bind group and the pipelines.
        let objects = 1 + 1 + 1 + u32::from(compact_pipeline.is_some());
        let copy_bytes = u64::from(size.0) * u64::from(size.1) * crate::graph::HDR_BYTES_PER_PIXEL;
        WaterGpu { copy, copy_view, copy_bytes, plain: WaterFlavour { bind, vertex, compact: compact_pipeline }, atmosphere: None, objects }
    }

    /// The atmosphere flavour: group 3 holds the atmosphere's block, tables and sky-view table beside the copy.
    pub(crate) fn add_atmosphere(
        &mut self,
        device: &wgpu::Device,
        bgl0: &wgpu::BindGroupLayout,
        compact: Option<&CompactLayouts<'_>>,
        atmosphere: &AtmospherePath,
    ) {
        if self.atmosphere.is_some() {
            return;
        }
        let (layout, bind) = atmosphere.water_group(device, &self.copy_view);
        let (vertex, compact_pipeline) = pipelines(device, bgl0, &layout, compact, true);
        self.objects += 1 + 1 + u32::from(compact_pipeline.is_some());
        self.atmosphere = Some(WaterFlavour { bind, vertex, compact: compact_pipeline });
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Probes (tier B): the shader's own functions run on the GPU
// ---------------------------------------------------------------------------------------------------------------------------

const PROBE_SOURCE: &str = r#"
struct WaveProbe { liq: WLiquid, local_spacing: vec4<f32>, up_depth: vec4<f32>, phases: vec4<f32>, extra: vec4<f32> };
@group(0) @binding(0) var<storage, read> wave_probes: array<WaveProbe>;
@group(0) @binding(1) var<storage, read_write> results: array<vec4<f32>>;

@compute @workgroup_size(1)
fn probe_waves(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = wave_probes[id.x];
    let w = water_waves(p.liq, p.local_spacing.xyz, p.up_depth.xyz, p.phases, p.local_spacing.w, p.up_depth.w, p.extra.x, p.extra.y);
    results[2u * id.x] = vec4<f32>(w.disp, 0.0);
    results[2u * id.x + 1u] = vec4<f32>(w.slope, 0.0);
}

struct SurfaceProbe {
    liq: WLiquid, la: vec4<f32>, lb: vec4<f32>, n: vec4<f32>, v: vec4<f32>, up: vec4<f32>, sun_depth: vec4<f32>, seabed_fm: vec4<f32>,
    diffuse: vec4<f32>, sun_t: vec4<f32>, sky: vec4<f32>,
};
@group(0) @binding(0) var<storage, read> surface_probes: array<SurfaceProbe>;

@compute @workgroup_size(1)
fn probe_surface(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = surface_probes[id.x];
    let rgb = water_surface(p.liq, p.la, p.lb, p.n.xyz, p.v.xyz, p.up.xyz, p.sun_depth.xyz, p.sun_depth.w, p.seabed_fm.w, p.seabed_fm.xyz,
                            p.diffuse.xyz, p.sun_t.xyz, p.sky.xyz);
    results[id.x] = vec4<f32>(rgb, 1.0);
}
"#;

/// One input of [`probe_waves`]: the arguments of the shader's `water_waves`.
#[derive(Clone, Copy, Debug)]
pub struct WaveProbe {
    pub liquid: LiquidAppearance,
    pub local: [f32; 3],
    pub spacing: f32,
    pub up: [f32; 3],
    pub depth: f32,
    pub phases: [f32; WAVES],
    pub choppy: f32,
    /// The pixel's size on the surface (m); 0 for the vertex stage's evaluation.
    pub footprint: f32,
}

/// One input of [`probe_surface`]: the arguments of the shader's `water_surface`.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceProbe {
    pub liquid: LiquidAppearance,
    /// (shore fade, foam width, foam strength, choppy) and (Fresnel F0, glint roughness, fade scale, 0).
    pub la: [f32; 4],
    pub lb: [f32; 4],
    pub n: [f32; 3],
    pub v: [f32; 3],
    pub up: [f32; 3],
    pub sun: [f32; 3],
    pub depth: f32,
    pub foam_mod: f32,
    pub seabed: [f32; 3],
    pub diffuse: [f32; 3],
    pub sun_t: [f32; 3],
    pub sky: [f32; 3],
}

fn run_probe(ctx: &GpuContext, entry: &str, input: &[f32], count: usize, outputs: usize) -> Vec<[f32; 4]> {
    use wgpu::util::DeviceExt;
    let device = &ctx.device;
    let bytes: Vec<u8> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    let probe_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("water-probes"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_bytes = (count * outputs * 16) as u64;
    let results = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("water-probe-results"),
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let source = format!("{WATER_MATH}\n{PROBE_SOURCE}");
    let module = device
        .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("water-probe"), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let entry_layout = |binding, read_only| wgpu::BindGroupLayoutEntry {
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
        label: Some("water-probe"),
        entries: &[entry_layout(0, true), entry_layout(1, false)],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("water-probe"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("water-probe"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("water-probe"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: probe_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: results.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("water-probe") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("water-probe"), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(count as u32, 1, 1);
    }
    ctx.queue.submit([encoder.finish()]);
    let data = crate::lit::read_buffer_bytes(ctx, &results, out_bytes);
    data.as_chunks::<16>()
        .0
        .iter()
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[4 * i], c[4 * i + 1], c[4 * i + 2], c[4 * i + 3]]);
            [f(0), f(1), f(2), f(3)]
        })
        .collect()
}

fn push4(out: &mut Vec<f32>, a: [f32; 3], w: f32) {
    out.extend([a[0], a[1], a[2], w]);
}

/// Evaluate the shader's `water_waves` at `probes` on the GPU: the displacement and the slope of each. The tier B oracle of REND-030.
pub fn probe_waves(ctx: &GpuContext, probes: &[WaveProbe]) -> Vec<([f32; 3], [f32; 3])> {
    let mut input: Vec<f32> = Vec::with_capacity(probes.len() * (LIQUID_VEC4S + 4) * 4);
    for p in probes {
        input.extend(p.liquid.to_words());
        push4(&mut input, p.local, p.spacing);
        push4(&mut input, p.up, p.depth);
        input.extend(p.phases);
        input.extend([p.choppy, p.footprint, 0.0, 0.0]);
    }
    let r = run_probe(ctx, "probe_waves", &input, probes.len(), 2);
    r.chunks(2).map(|c| ([c[0][0], c[0][1], c[0][2]], [c[1][0], c[1][1], c[1][2]])).collect()
}

/// Evaluate the shader's `water_surface` at `probes` on the GPU: the radiance of each. The tier B oracle of REND-031.
pub fn probe_surface(ctx: &GpuContext, probes: &[SurfaceProbe]) -> Vec<[f32; 3]> {
    let mut input: Vec<f32> = Vec::with_capacity(probes.len() * (LIQUID_VEC4S + 10) * 4);
    for p in probes {
        input.extend(p.liquid.to_words());
        input.extend(p.la);
        input.extend(p.lb);
        push4(&mut input, p.n, 0.0);
        push4(&mut input, p.v, 0.0);
        push4(&mut input, p.up, 0.0);
        push4(&mut input, p.sun, p.depth);
        push4(&mut input, p.seabed, p.foam_mod);
        push4(&mut input, p.diffuse, 0.0);
        push4(&mut input, p.sun_t, 0.0);
        push4(&mut input, p.sky, 0.0);
    }
    run_probe(ctx, "probe_surface", &input, probes.len(), 1).into_iter().map(|r| [r[0], r[1], r[2]]).collect()
}
