//! The test-only reference renderer (ADR 0062 point 1, `remove-vertex-render-path`): draws a `TerrainPlan` from CPU `TileMesh`es
//! with one pipeline, one vertex buffer per tile and no pools, no fallback, no lit path, no atmosphere and no water. It exists so
//! the tier-C tests that compare the compact renderer's images keep an *independent* geometry source (a shader bug in the compact
//! vertex stage shows as a difference to the mesh's own vertices). It shares the product shader's header and fragment stage with the
//! compact pipeline (spliced at build time, like `compact_terrain_shader`), and replaces only the vertex stage.
//!
//! It is not a second production path: the feature `reference-renderer` is off by default, no product crate enables it and
//! `cargo xtask layering` fails when a non-dev dependency edge does (REND-031).

use crate::terrain::{node_record_bytes, view_index, DEPTH_COMPARE, DEPTH_FORMAT, GLOBALS_BYTES, TERRAIN_SHADERS};
use crate::{read_rgba8, ExecError, GpuContext, RgbaFrame};
use planet_frame::TerrainPlan;
use planet_generators::mesh::{grid_indices, MeshVertex, TileMesh};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// The vertex stage of a mesh vertex: position, parent-grid position, normal, height and parent-grid height as 11 floats.
const REFERENCE_VERTEX: &str = r#"
fn terrain_vertex(inst: u32, pos: vec3<f32>, coarse: vec3<f32>, normal: vec3<f32>, height: f32, coarse_height: f32) -> VsOut {
    let node = nodes[inst];
    let m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let local = mix(pos, coarse, m);
    let p = node.origin_rel.xyz + local;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = normal;
    out.height = mix(height, coarse_height, m);
    out.height_fine = height;
    out.morph = m;
    out.local = local;
    out.node_index = inst;
    out.aux = vec2<f32>(0.0, 0.0);
    return out;
}

@vertex
fn vs(@builtin(instance_index) inst: u32, @location(0) pos: vec3<f32>, @location(1) coarse: vec3<f32>, @location(2) normal: vec3<f32>, @location(3) height: f32, @location(4) coarse_height: f32) -> VsOut {
    return terrain_vertex(inst, pos, coarse, normal, height, coarse_height);
}

fn tile_climate(in: VsOut) -> vec2<f32> {
    return in.aux;
}

"#;

fn reference_shader() -> String {
    let src = TERRAIN_SHADERS[0].1;
    let vs_at = src.find("fn terrain_vertex(").expect("product shader has a vertex stage");
    let tail_at = src.find("// Distance in pixels to the nearest integer").expect("product shader helper marker");
    assert!(src[..vs_at].contains("@group(0) @binding(1) var<storage, read> nodes: array<Node>;"), "product node binding changed");
    format!("{}{REFERENCE_VERTEX}{}", &src[..vs_at], &src[tail_at..])
}

/// Render the debug view of `plan` from `meshes` (one per planned tile, the key checked against the plan's) into an RGBA frame.
/// A plan with a lit part is refused: this renderer has no lit path.
pub fn execute_reference(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[&TileMesh]) -> Result<RgbaFrame, ExecError> {
    if plan.lit.is_some() {
        return Err(ExecError::LitNotEnabled);
    }
    let [lo, hi] = plan.height_range;
    if !(lo.is_finite() && hi.is_finite() && lo < hi) {
        return Err(ExecError::BadHeightRange { lo, hi });
    }
    let lookup: HashMap<_, _> = meshes.iter().map(|m| (m.id, *m)).collect();
    for d in &plan.nodes {
        match lookup.get(&d.tile) {
            None => return Err(ExecError::MissingTile(d.tile)),
            Some(m) if m.key != d.mesh_key => return Err(ExecError::StaleMesh { tile: d.tile, resident: m.key, planned: d.mesh_key }),
            Some(m) if m.cells != plan.cells => return Err(ExecError::BadResolution { id: d.tile, res: m.cells }),
            Some(_) => {}
        }
    }
    let device = &ctx.device;
    let (w, h) = plan.size;
    let target = |label, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = target("reference-color", wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
    let depth = target("reference-depth", DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));

    let mut globals: Vec<u8> = plan.view_projection.iter().flat_map(|v| v.to_le_bytes()).collect();
    globals.extend([lo, hi, 0.0, 0.0].iter().flat_map(|v: &f32| v.to_le_bytes()));
    globals.extend([view_index(plan.view), 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
    globals.resize(GLOBALS_BYTES as usize, 0);
    let globals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("reference-globals"),
        contents: &globals,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("reference-nodes"),
        contents: &node_record_bytes(plan),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    };
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("reference-bgl"),
        entries: &[entry(0, wgpu::BufferBindingType::Uniform), entry(1, wgpu::BufferBindingType::Storage { read_only: true })],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("reference-bind"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: nodes.as_entire_binding() },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("reference-layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("reference-terrain"),
        source: wgpu::ShaderSource::Wgsl(reference_shader().into()),
    });
    // 11 floats per vertex: pos, coarse, normal, height, coarse height (`MeshVertex::to_floats`).
    let attr = |format, k: usize, shader_location| wgpu::VertexAttribute { format, offset: 4 * MeshVertex::OFFSETS[k] as u64, shader_location };
    let attrs = [
        attr(wgpu::VertexFormat::Float32x3, 0, 0),
        attr(wgpu::VertexFormat::Float32x3, 1, 1),
        attr(wgpu::VertexFormat::Float32x3, 2, 2),
        attr(wgpu::VertexFormat::Float32, 3, 3),
        attr(wgpu::VertexFormat::Float32, 4, 4),
    ];
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("reference-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout { array_stride: MeshVertex::STRIDE, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
        // Reversed-Z (ADR 0002), as the product: near = 1, far = 0.
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
    let index_data = grid_indices(plan.cells);
    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("reference-indices"),
        contents: &index_data.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::INDEX,
    });
    let buffers: HashMap<_, _> = plan
        .nodes
        .iter()
        .map(|d| {
            let m = lookup[&d.tile];
            let bytes: Vec<u8> = m.vertices.iter().flat_map(|v| v.to_floats()).flat_map(|x| x.to_le_bytes()).collect();
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("reference-vertices"),
                contents: &bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
            (d.tile, buffer)
        })
        .collect();

    let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("reference") });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("reference-pass"),
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
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(0, &bind, &[]);
        for (i, d) in plan.nodes.iter().enumerate() {
            pass.set_vertex_buffer(0, buffers[&d.tile].slice(..));
            pass.draw_indexed(0..index_data.len() as u32, 0, i as u32..i as u32 + 1);
        }
    }
    ctx.queue.submit(Some(encoder.finish()));
    Ok(read_rgba8(ctx, &color, w, h))
}
