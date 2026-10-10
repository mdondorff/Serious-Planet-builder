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

// Distance in pixels to the nearest integer of `x`, so lines stay about one pixel wide at any scale.
fn line(x: f32, w: f32) -> f32 {
    let d = abs(fract(x + 0.5) - 0.5);
    return 1.0 - smoothstep(0.0, max(w, 1e-6) * 1.5, d);
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
        return vec4<f32>(node.color.rgb, 1.0);
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
    let v = clamp(band_z / 40.0 + 1.0, 0.0, 1.0);
    let k = 1.0 - 0.8 * line(band_z, w_z);
    return vec4<f32>(v * k, v * k, v * k, 1.0);
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
    pipeline: crate::wgpu::RenderPipeline,
    bgl: crate::wgpu::BindGroupLayout,
    globals: crate::wgpu::Buffer,
    nodes: crate::wgpu::Buffer,
    node_capacity: usize,
    bind: crate::wgpu::BindGroup,
    stride: u64,
    indices: crate::wgpu::Buffer,
    index_count: u32,
    vertex_buffers: HashMap<planet_core::TileId, crate::wgpu::Buffer>,
    color: crate::wgpu::Texture,
    color_view: crate::wgpu::TextureView,
    depth_view: crate::wgpu::TextureView,
    timing: Option<TimingResources>,
    /// Bytes of vertex data uploaded so far.
    pub uploaded_bytes: u64,
}

struct TimingResources {
    queries: crate::wgpu::QuerySet,
    resolve: crate::wgpu::Buffer,
    readback: crate::wgpu::Buffer,
}

impl TerrainRenderer {
    pub fn new(ctx: &GpuContext, size: (u32, u32), cells: u32) -> Self {
        use crate::wgpu;
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
        let color = target("terrain-color", format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let depth = target("terrain-depth", wgpu::TextureFormat::Depth32Float, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stride = 48u64.next_multiple_of(u64::from(device.limits().min_uniform_buffer_offset_alignment));
        let node_capacity = 256;
        let nodes = Self::node_buffer(device, stride, node_capacity);
        let module = device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("terrain"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let uniform = |binding, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: if dynamic { wgpu::BufferSize::new(48) } else { None },
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain-bgl"),
            entries: &[uniform(0, false), uniform(1, true)],
        });
        let bind = Self::bind_group(device, &bgl, &globals, &nodes);
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
            attr(wgpu::VertexFormat::Float32x3, 4, 4),
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
            globals,
            nodes,
            node_capacity,
            bind,
            stride,
            indices,
            index_count: index_data.len() as u32,
            vertex_buffers: HashMap::new(),
            color,
            color_view,
            depth_view,
            timing,
            uploaded_bytes: 0,
        }
    }

    fn node_buffer(device: &crate::wgpu::Device, stride: u64, capacity: usize) -> crate::wgpu::Buffer {
        device.create_buffer(&crate::wgpu::BufferDescriptor {
            label: Some("nodes"),
            size: stride * capacity as u64,
            usage: crate::wgpu::BufferUsages::UNIFORM | crate::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group(
        device: &crate::wgpu::Device,
        bgl: &crate::wgpu::BindGroupLayout,
        globals: &crate::wgpu::Buffer,
        nodes: &crate::wgpu::Buffer,
    ) -> crate::wgpu::BindGroup {
        use crate::wgpu;
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain-bind"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: nodes,
                        offset: 0,
                        size: wgpu::BufferSize::new(48),
                    }),
                },
            ],
        })
    }

    /// Upload a tile's vertices once; later calls for the same tile do nothing.
    pub fn upload(&mut self, ctx: &GpuContext, mesh: &TileMesh) -> Result<(), ExecError> {
        use crate::wgpu;
        use wgpu::util::DeviceExt;
        if mesh.cells != self.cells {
            return Err(ExecError::BadResolution { id: mesh.id, res: mesh.cells });
        }
        if self.vertex_buffers.contains_key(&mesh.id) {
            return Ok(());
        }
        let bytes = vertex_bytes(&mesh.vertices);
        self.uploaded_bytes += bytes.len() as u64;
        let buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain-vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.vertex_buffers.insert(mesh.id, buffer);
        Ok(())
    }

    pub fn has_tile(&self, tile: planet_core::TileId) -> bool {
        self.vertex_buffers.contains_key(&tile)
    }

    pub fn resident_tiles(&self) -> usize {
        self.vertex_buffers.len()
    }

    /// Draw the plan. Every planned tile must have been uploaded. With `timed`, GPU time is measured when supported.
    pub fn render(&mut self, ctx: &GpuContext, plan: &TerrainPlan, timed: bool) -> Result<(FrameTiming, TerrainStats), ExecError> {
        use crate::wgpu;
        let started = std::time::Instant::now();
        if plan.size != self.size {
            return Err(ExecError::SizeMismatch { plan: plan.size, renderer: self.size });
        }
        if plan.cells != self.cells {
            return Err(ExecError::BadResolution { id: planet_core::TileId::from_raw(0).expect("root"), res: plan.cells });
        }
        for d in &plan.nodes {
            if !self.vertex_buffers.contains_key(&d.tile) {
                return Err(ExecError::MissingTile(d.tile));
            }
        }
        let [lo, hi] = plan.height_range;
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(ExecError::BadHeightRange { lo, hi });
        }
        let device = &ctx.device;
        if plan.nodes.len() > self.node_capacity {
            self.node_capacity = plan.nodes.len().next_power_of_two();
            self.nodes = Self::node_buffer(device, self.stride, self.node_capacity);
            self.bind = Self::bind_group(device, &self.bgl, &self.globals, &self.nodes);
        }
        let mut globals = f32_bytes(&plan.view_projection);
        globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
        globals.extend([view_index(plan.view), 0, 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
        ctx.queue.write_buffer(&self.globals, 0, &globals);
        let mut nodes = vec![0u8; self.stride as usize * plan.nodes.len().max(1)];
        for (i, d) in plan.nodes.iter().enumerate() {
            let r = d.camera_relative_origin;
            let c = d.color.map(|v| f32::from(v) / 255.0);
            let b =
                f32_bytes(&[r[0], r[1], r[2], 0.0, c[0], c[1], c[2], c[3], d.morph_start_m, d.morph_end_m, d.morph, f32::from(d.level)]);
            nodes[i * self.stride as usize..i * self.stride as usize + 48].copy_from_slice(&b);
        }
        ctx.queue.write_buffer(&self.nodes, 0, &nodes);

        let timing = if timed { self.timing.as_ref() } else { None };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("terrain") });
        let mut stats = TerrainStats::default();
        {
            let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (i, d) in plan.nodes.iter().enumerate() {
                pass.set_bind_group(0, &self.bind, &[(i as u64 * self.stride) as u32]);
                pass.set_vertex_buffer(0, self.vertex_buffers[&d.tile].slice(..));
                pass.draw_indexed(0..self.index_count, 0, 0..1);
                stats.draw_calls += 1;
                stats.triangles += u64::from(self.index_count) / 3;
            }
        }
        if let Some(t) = timing {
            encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, 16);
        }
        let command_buffer = encoder.finish();
        ctx.queue.submit([command_buffer]);
        let cpu_ms = started.elapsed().as_secs_f64() * 1000.0;
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
        Ok((FrameTiming { cpu_ms, total_ms, gpu_ms }, stats))
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
