//! Executes a `FramePlan` (CON-03): draws each `TileDraw` into its pixel rectangle through the plan's debug view.
//! The renderer decides nothing: rectangles, colours and the height range all come from the plan.
//! `reference_frame` is the CPU twin used by the tier-B oracle test.

use crate::{read_rgba8, GpuContext, RgbaFrame};
use planet_core::TileId;
use planet_frame::{DebugView, FramePlan};
use std::fmt;

/// Height samples of one tile as uploaded to the GPU: `(res + 1)²` values, row-major.
#[derive(Clone, Copy, Debug)]
pub struct TileResource<'a> {
    pub id: TileId,
    pub res: u32,
    pub heights: &'a [f32],
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExecError {
    MissingTile(TileId),
    BadResource { id: TileId, expected: usize, got: usize },
    OutsideFrame { index: usize, rect: [u32; 4], size: (u32, u32) },
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecError::MissingTile(id) => write!(f, "the plan draws tile {id:?} but no resource for it was supplied"),
            ExecError::BadResource { id, expected, got } => write!(f, "tile {id:?}: expected {expected} height samples, got {got}"),
            ExecError::OutsideFrame { index, rect, size } => {
                write!(f, "draw {index}: rectangle {rect:?} lies outside the {}x{} frame", size.0, size.1)
            }
        }
    }
}

impl std::error::Error for ExecError {}

/// Counts that are deterministic on every adapter (CON-20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub texture_bytes_uploaded: u64,
}

const SHADER: &str = r#"
struct Params {
    rect: vec4<u32>,
    res: u32,
    view: u32,
    pad0: u32,
    pad1: u32,
    color: vec4<f32>,
    range: vec2<f32>,
    pad2: vec2<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var heights: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(pos[i], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    if (p.view != 2u) {
        return p.color;
    }
    let px = u32(frag.x) - p.rect.x;
    let py = u32(frag.y) - p.rect.y;
    let tx = min(px * (p.res + 1u) / p.rect.z, p.res);
    let ty = min(py * (p.res + 1u) / p.rect.w, p.res);
    let h = textureLoad(heights, vec2<i32>(i32(tx), i32(ty)), 0).r;
    let t = clamp((h - p.range.x) / (p.range.y - p.range.x), 0.0, 1.0);
    let g = floor(t * 255.0 + 0.5) / 255.0;
    return vec4<f32>(g, g, g, 1.0);
}
"#;

/// Shader modules shipped by `exec`, for the validation test.
pub const EXEC_SHADERS: &[(&str, &str)] = &[("tile-debug-views", SHADER)];

fn view_index(v: DebugView) -> u32 {
    match v {
        DebugView::Face => 0,
        DebugView::TileId => 1,
        DebugView::Height => 2,
    }
}

fn check(plan: &FramePlan, tiles: &[TileResource]) -> Result<(), ExecError> {
    for (index, d) in plan.draws.iter().enumerate() {
        let [x, y, w, h] = d.rect;
        if w == 0 || h == 0 || x + w > plan.size.0 || y + h > plan.size.1 {
            return Err(ExecError::OutsideFrame { index, rect: d.rect, size: plan.size });
        }
        if plan.view == DebugView::Height {
            let t = tiles.iter().find(|t| t.id == d.tile).ok_or(ExecError::MissingTile(d.tile))?;
            let expected = ((t.res + 1) * (t.res + 1)) as usize;
            if t.heights.len() != expected {
                return Err(ExecError::BadResource { id: t.id, expected, got: t.heights.len() });
            }
        }
    }
    Ok(())
}

fn params_bytes(rect: [u32; 4], res: u32, view: u32, color: [u8; 4], range: [f32; 2]) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    for v in rect {
        b.extend(v.to_le_bytes());
    }
    for v in [res, view, 0, 0] {
        b.extend(v.to_le_bytes());
    }
    for c in color {
        b.extend((f32::from(c) / 255.0).to_le_bytes());
    }
    for v in [range[0], range[1], 0.0, 0.0] {
        b.extend(v.to_le_bytes());
    }
    b
}

/// Execute the plan on the GPU and read the frame back.
pub fn execute(ctx: &GpuContext, plan: &FramePlan, tiles: &[TileResource]) -> Result<(RgbaFrame, ExecStats), ExecError> {
    use crate::wgpu;
    use wgpu::util::DeviceExt;
    check(plan, tiles)?;
    let device = &ctx.device;
    let (w, h) = plan.size;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("frame"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tile-debug-views"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tile-debug-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tile-debug-layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tile-debug-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let mut stats = ExecStats::default();
    let mut bind_groups = Vec::new();
    for d in &plan.draws {
        let (res, heights): (u32, Vec<f32>) = match tiles.iter().find(|t| t.id == d.tile) {
            Some(t) if plan.view == DebugView::Height => (t.res, t.heights.to_vec()),
            _ => (0, vec![0.0]),
        };
        let n = res + 1;
        let tex = device.create_texture_with_data(
            &ctx.queue,
            &wgpu::TextureDescriptor {
                label: Some("tile-heights"),
                size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &heights.iter().flat_map(|h| h.to_le_bytes()).collect::<Vec<u8>>(),
        );
        stats.texture_bytes_uploaded += u64::from(n * n * 4);
        let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile-params"),
            contents: &params_bytes(d.rect, res, view_index(plan.view), d.color, plan.height_range),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tile-bind"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        }));
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
    {
        let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        for (d, bg) in plan.draws.iter().zip(&bind_groups) {
            let [x, y, rw, rh] = d.rect;
            pass.set_viewport(x as f32, y as f32, rw as f32, rh as f32, 0.0, 1.0);
            pass.set_scissor_rect(x, y, rw, rh);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
            stats.draw_calls += 1;
            stats.triangles += 1;
        }
    }
    ctx.queue.submit([encoder.finish()]);
    Ok((read_rgba8(ctx, &target, w, h), stats))
}

/// CPU twin of `execute` (same integer texel selection, same grey ramp in f32).
pub fn reference_frame(plan: &FramePlan, tiles: &[TileResource]) -> Result<RgbaFrame, ExecError> {
    check(plan, tiles)?;
    let (w, h) = plan.size;
    let mut rgba: Vec<u8> = plan.clear_color.repeat((w * h) as usize);
    for d in &plan.draws {
        let [x0, y0, rw, rh] = d.rect;
        let tile = tiles.iter().find(|t| t.id == d.tile);
        for py in 0..rh {
            for px in 0..rw {
                let c = match (plan.view, tile) {
                    (DebugView::Height, Some(t)) => {
                        let tx = (px * (t.res + 1) / rw).min(t.res);
                        let ty = (py * (t.res + 1) / rh).min(t.res);
                        let v = t.heights[(ty * (t.res + 1) + tx) as usize];
                        let [lo, hi] = plan.height_range;
                        let g = (((v - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8;
                        [g, g, g, 255]
                    }
                    _ => d.color,
                };
                let i = (((y0 + py) * w + x0 + px) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&c);
            }
        }
    }
    Ok(RgbaFrame { width: w, height: h, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::{Face, PlanetFixed, Vec3};
    use planet_frame::{plan_tiles, PlanRequest};

    fn plan(view: DebugView) -> FramePlan {
        plan_tiles(&PlanRequest {
            tiles: vec![TileId::new(Face(1), 2, 1, 1).unwrap()],
            camera: PlanetFixed(Vec3::new(1.0e7, 0.0, 0.0)),
            size: (32, 32),
            view,
            radius_m: 6_371_000.0,
            face_mapping: "tangent-v1".into(),
        })
        .unwrap()
    }

    // spec: REND-001
    #[test]
    fn a_plan_that_cannot_be_executed_is_reported_not_guessed() {
        let p = plan(DebugView::Height);
        let id = p.draws[0].tile;
        assert_eq!(reference_frame(&p, &[]).unwrap_err(), ExecError::MissingTile(id));
        let short = [0.0f32; 3];
        let e = reference_frame(&p, &[TileResource { id, res: 4, heights: &short }]).unwrap_err();
        assert_eq!(e, ExecError::BadResource { id, expected: 25, got: 3 });
        let mut off = plan(DebugView::Face);
        off.draws[0].rect = [16, 0, 32, 32];
        assert!(matches!(reference_frame(&off, &[]).unwrap_err(), ExecError::OutsideFrame { index: 0, .. }));
    }

    // spec: REND-001
    #[test]
    fn the_reference_frame_fills_exactly_the_planned_rectangles() {
        let p = plan(DebugView::Face);
        let f = reference_frame(&p, &[]).unwrap();
        let fill = f.rgba.as_chunks::<4>().0.iter().filter(|px| **px == p.draws[0].color).count();
        assert_eq!(fill, 32 * 32);
        assert!(!f.rgba.as_chunks::<4>().0.iter().any(|px| *px == p.clear_color));
    }
}
