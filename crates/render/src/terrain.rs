//! Executes a `TerrainPlan` (CON-03): one indexed draw per planned node, camera-relative f32 positions, reversed-Z
//! `Depth32Float` with an infinite far plane (ADR 0002). The renderer makes no decisions: nodes, matrices, morph
//! intervals and colours all come from the plan. Shaders see only tile-local and camera-relative quantities (CON-11).

use crate::{read_rgba8, ExecError, GpuContext, RgbaFrame};
use planet_frame::{TerrainPlan, TerrainView};
use planet_generators::mesh::{grid_indices, MeshVertex, TileMesh};
use std::collections::HashMap;

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    range: vec4<f32>,
    mode: vec4<u32>,
};
struct Node {
    origin_rel: vec4<f32>,
    color: vec4<f32>,
    morph: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<uniform> node: Node;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) height: f32,
    @location(2) morph: f32,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) coarse: vec3<f32>, @location(2) normal: vec3<f32>, @location(3) height: f32, @location(4) ref_pos: vec3<f32>) -> VsOut {
    // The morph distance is measured to the height-free reference position, the same quantity node selection uses.
    let m = clamp((length(node.origin_rel.xyz + ref_pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let p = node.origin_rel.xyz + mix(pos, coarse, m);
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = normal;
    out.height = height;
    out.morph = m;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let mode = g.mode.x;
    if (mode <= 2u) {
        return vec4<f32>(node.color.rgb, 1.0);
    }
    if (mode == 3u) {
        return vec4<f32>(in.morph, in.morph, in.morph, 1.0);
    }
    if (mode == 4u) {
        let t = clamp((in.height - g.range.x) / (g.range.y - g.range.x), 0.0, 1.0);
        let v = floor(t * 255.0 + 0.5) / 255.0;
        return vec4<f32>(v, v, v, 1.0);
    }
    if (mode == 5u) {
        let n = normalize(in.normal) * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
        return vec4<f32>(n, 1.0);
    }
    let v = clamp(log2(in.pos.z) / 40.0 + 1.0, 0.0, 1.0);
    return vec4<f32>(v, v, v, 1.0);
}
"#;

pub const TERRAIN_SHADERS: &[(&str, &str)] = &[("terrain", SHADER)];

fn view_index(v: TerrainView) -> u32 {
    TerrainView::ALL.iter().position(|x| *x == v).expect("known view") as u32
}

/// Counts that are deterministic on every adapter (CON-20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainStats {
    pub draw_calls: u32,
    pub triangles: u64,
    pub vertex_bytes: u64,
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn vertex_bytes(vertices: &[MeshVertex]) -> Vec<u8> {
    vertices.iter().flat_map(|v| f32_bytes(&v.to_floats())).collect()
}

/// Execute the plan on the GPU and read the frame back.
pub fn execute_terrain(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[&TileMesh]) -> Result<(RgbaFrame, TerrainStats), ExecError> {
    use crate::wgpu;
    use wgpu::util::DeviceExt;
    let lookup: HashMap<_, _> = meshes.iter().map(|m| (m.id, *m)).collect();
    for d in &plan.nodes {
        let m = lookup.get(&d.tile).ok_or(ExecError::MissingTile(d.tile))?;
        if m.cells != plan.cells {
            return Err(ExecError::BadResolution { id: d.tile, res: m.cells });
        }
    }
    let [lo, hi] = plan.height_range;
    if !(lo.is_finite() && hi.is_finite() && lo < hi) {
        return Err(ExecError::BadHeightRange { lo, hi });
    }
    let device = &ctx.device;
    let (w, h) = plan.size;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain-color"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain-depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));

    // Globals and per-node uniforms (dynamic offsets).
    let mut globals = f32_bytes(&plan.view_projection);
    globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
    globals.extend([view_index(plan.view), 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
    let gbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("globals"),
        contents: &globals,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let stride = 48u64.next_multiple_of(u64::from(device.limits().min_uniform_buffer_offset_alignment));
    let mut nodes = vec![0u8; (stride as usize) * plan.nodes.len().max(1)];
    for (i, d) in plan.nodes.iter().enumerate() {
        let r = d.camera_relative_origin;
        let c = d.color.map(|v| f32::from(v) / 255.0);
        let b = f32_bytes(&[r[0], r[1], r[2], 0.0, c[0], c[1], c[2], c[3], d.morph_start_m, d.morph_end_m, d.morph, f32::from(d.level)]);
        nodes[i * stride as usize..i * stride as usize + 48].copy_from_slice(&b);
    }
    let nbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("nodes"),
        contents: &nodes,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let module = device
        .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("terrain"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terrain-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(48),
                },
                count: None,
            },
        ],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain-bind"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: gbuf.as_entire_binding() },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &nbuf, offset: 0, size: wgpu::BufferSize::new(48) }),
            },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain-layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let attrs = [
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 4 * MeshVertex::OFFSETS[0] as u64, shader_location: 0 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 4 * MeshVertex::OFFSETS[1] as u64, shader_location: 1 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 4 * MeshVertex::OFFSETS[2] as u64, shader_location: 2 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 4 * MeshVertex::OFFSETS[3] as u64, shader_location: 3 },
        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 4 * MeshVertex::OFFSETS[4] as u64, shader_location: 4 },
    ];
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terrain-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: MeshVertex::STRIDE,
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

    let indices = grid_indices(plan.cells);
    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain-indices"),
        contents: &indices.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::INDEX,
    });
    let mut stats = TerrainStats::default();
    let vbufs: Vec<wgpu::Buffer> = plan
        .nodes
        .iter()
        .map(|d| {
            let bytes = vertex_bytes(&lookup[&d.tile].vertices);
            stats.vertex_bytes += bytes.len() as u64;
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terrain-vertices"),
                contents: &bytes,
                usage: wgpu::BufferUsages::VERTEX,
            })
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain") });
    {
        let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terrain-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
        for (i, vb) in vbufs.iter().enumerate() {
            pass.set_bind_group(0, &bind, &[(i as u64 * stride) as u32]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            stats.draw_calls += 1;
            stats.triangles += indices.len() as u64 / 3;
        }
    }
    ctx.queue.submit([encoder.finish()]);
    Ok((read_rgba8(ctx, &color, w, h), stats))
}
