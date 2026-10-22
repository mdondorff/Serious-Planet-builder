//! The scale references pass (REND-036): the boxes of `planet_frame::scalerefs` drawn into the terrain pass of a lit frame, with the terrain's depth
//! test, flat-shaded by the plan's sun in the same radiance units as the lit terrain. The renderer decides nothing: it draws the boxes it is
//! given, as (camera-relative f32) instances of one unit cube (CON-11).

use crate::GpuContext;
use planet_frame::scalerefs::{RefBox, MAX_BOXES};

/// One instance: the centre relative to the camera, the three axes scaled by the half sizes, the albedo (5 x vec4).
const INSTANCE_BYTES: u64 = 80;
/// The frame uniform: the view-projection (16 floats), the sun direction and the pre-exposure factor (2 x vec4).
const UNIFORM_BYTES: u64 = 96;
/// The light the sun does not reach: a share of the sun's illuminance every face gets, so a face turned away is dark but not black.
const AMBIENT: f32 = 0.12;

const SHADER: &str = r#"
struct U {
    vp: mat4x4<f32>,
    sun: vec4<f32>,
    // x: the pre-exposure factor, y: the ambient share.
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) centre: vec4<f32>,
    @location(3) a0: vec4<f32>,
    @location(4) a1: vec4<f32>,
    @location(5) a2: vec4<f32>,
    @location(6) colour: vec4<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) n: vec3<f32>,
    @location(1) colour: vec3<f32>,
};

@vertex
fn vs(i: VsIn) -> VsOut {
    // The vertex relative to the camera: the box's centre (camera-relative) plus the corner along the scaled axes. No planet-radius value.
    let rel = i.centre.xyz + i.a0.xyz * i.pos.x + i.a1.xyz * i.pos.y + i.a2.xyz * i.pos.z;
    var o: VsOut;
    o.clip = u.vp * vec4<f32>(rel, 1.0);
    // The axes are orthogonal, so the face normal is the axis of the face, normalised.
    o.n = normalize(i.a0.xyz * i.nrm.x + i.a1.xyz * i.nrm.y + i.a2.xyz * i.nrm.z);
    o.colour = i.colour.xyz;
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let ndl = max(dot(normalize(in.n), u.sun.xyz), 0.0);
    let inv_pi = 0.31830988618;
    let rgb = in.colour * (ndl + u.params.y) * inv_pi * u.params.x;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(60000.0)), 1.0);
}
"#;

pub(crate) struct RefPass {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    cube: wgpu::Buffer,
    indices: wgpu::Buffer,
    instances: wgpu::Buffer,
    count: std::cell::Cell<u32>,
}

/// The unit cube as 24 vertices (position, face normal) and 36 indices, counter-clockwise seen from outside.
fn cube() -> (Vec<f32>, Vec<u16>) {
    let mut v: Vec<f32> = Vec::new();
    let mut idx: Vec<u16> = Vec::new();
    for axis in 0..3 {
        for sign in [-1.0f32, 1.0] {
            let mut n = [0.0f32; 3];
            n[axis] = sign;
            let (u, w) = ((axis + 1) % 3, (axis + 2) % 3);
            let base = (v.len() / 6) as u16;
            for (a, b) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let mut p = [0.0f32; 3];
                p[axis] = sign;
                p[u] = a;
                p[w] = b;
                v.extend(p);
                v.extend(n);
            }
            // (u, w, axis) is a right-handed frame for every axis, so the quad is counter-clockwise from outside on the + side.
            if sign > 0.0 {
                idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
            }
        }
    }
    (v, idx)
}

impl RefPass {
    pub(crate) fn new(ctx: &GpuContext) -> RefPass {
        let device = &ctx.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scale-refs"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scale-refs-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scale-refs-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let v4 =
            |location: u32, offset: u64| wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset, shader_location: location };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scale-refs-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 24,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                        ],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: INSTANCE_BYTES,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[v4(2, 0), v4(3, 16), v4(4, 32), v4(5, 48), v4(6, 64)],
                    }),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::lit::HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            // No culling: the depth test picks the nearest face whatever the handedness of the projection.
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::terrain::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(crate::terrain::DEPTH_COMPARE),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let uniform = buffer("scale-refs-uniform", UNIFORM_BYTES, wgpu::BufferUsages::UNIFORM);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scale-refs-bind"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        let (verts, indices) = cube();
        let cube_buf = buffer("scale-refs-cube", (verts.len() * 4) as u64, wgpu::BufferUsages::VERTEX);
        ctx.queue.write_buffer(&cube_buf, 0, &verts.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
        let index_buf = buffer("scale-refs-indices", (indices.len() * 2) as u64, wgpu::BufferUsages::INDEX);
        ctx.queue.write_buffer(&index_buf, 0, &indices.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>());
        let instances = buffer("scale-refs-instances", MAX_BOXES as u64 * INSTANCE_BYTES, wgpu::BufferUsages::VERTEX);
        RefPass { pipeline, uniform, bind, cube: cube_buf, indices: index_buf, instances, count: std::cell::Cell::new(0) }
    }

    /// Write this frame's uniform and instances: the boxes relative to `camera` (f64 subtraction, then f32), the plan's view-projection, sun and
    /// pre-exposure factor. The queue orders the writes after the frames already submitted, so a frame in flight keeps the data it reads.
    pub(crate) fn upload(
        &self,
        ctx: &GpuContext,
        view_projection: &[f32; 16],
        sun: [f32; 3],
        factor: f32,
        camera: planet_core::Vec3,
        boxes: &[RefBox],
    ) {
        let boxes = &boxes[..boxes.len().min(MAX_BOXES)];
        let mut u: Vec<f32> = view_projection.to_vec();
        u.extend([sun[0], sun[1], sun[2], 0.0, factor, AMBIENT, 0.0, 0.0]);
        ctx.queue.write_buffer(&self.uniform, 0, &u.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());
        let mut bytes: Vec<u8> = Vec::with_capacity(boxes.len() * INSTANCE_BYTES as usize);
        for b in boxes {
            let rel = b.centre.0 - camera;
            let mut words: Vec<f32> = vec![rel.x as f32, rel.y as f32, rel.z as f32, 0.0];
            for k in 0..3 {
                let a = b.axes[k] * f64::from(b.half_m[k]);
                words.extend([a.x as f32, a.y as f32, a.z as f32, 0.0]);
            }
            words.extend([b.colour[0], b.colour[1], b.colour[2], 1.0]);
            bytes.extend(words.iter().flat_map(|f| f.to_le_bytes()));
        }
        if !bytes.is_empty() {
            ctx.queue.write_buffer(&self.instances, 0, &bytes);
        }
        self.count.set(boxes.len() as u32);
    }

    /// Draw the instances uploaded by the last [`upload`](RefPass::upload) into `pass` (the terrain pass: same targets, same depth).
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let count = self.count.get();
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.cube.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..36, 0, 0..count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REND-036
    #[test]
    fn the_cube_has_six_faces_whose_triangles_wind_counter_clockwise_seen_from_outside() {
        let (v, idx) = cube();
        assert_eq!((v.len() / 6, idx.len()), (24, 36));
        for tri in idx.chunks(3) {
            let p = |k: usize| {
                let o = tri[k] as usize * 6;
                (glam_like(&v[o..o + 3]), glam_like(&v[o + 3..o + 6]))
            };
            let ((a, n), (b, _), (c, _)) = (p(0), p(1), p(2));
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cross = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]];
            let along: f32 = (0..3).map(|k| cross[k] * n[k]).sum();
            assert!(along > 0.0, "triangle {tri:?} faces inwards: {along}");
            // The face lies on the cube's surface in the direction of its normal.
            let height: f32 = (0..3).map(|k| a[k] * n[k]).sum();
            assert_eq!(height, 1.0);
        }
    }

    fn glam_like(s: &[f32]) -> [f32; 3] {
        [s[0], s[1], s[2]]
    }
}
