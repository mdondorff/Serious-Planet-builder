//! SPIKE-07 (docs/archive/M4/spikes.md): a shader detail term below the finest geometry level. NOT production code, behind the `spike07`
//! feature, never used by the product renderer.
//!
//! The term is VISUAL ONLY: it perturbs the shading normal of the relief view and never moves a vertex, a depth value or a
//! height, so `height(dir)` (ADR 0028) stays the one source of heights for collision and picking.
//!
//! It continues the octave series of the eroded field (`cpu-erosion-v1`: ridged creases `0.25 - (1 - |n|)^3` of lattice value
//! noise, octave `o` at `256 * 2^o` cells per unit radius, amplitude `0.55^o`, the field stops at octave 11 = 12 m cells) with
//! octaves 12 to 22 (cells of 6 m down to 6 mm), on the same smoothstep value noise. Differences from the field, all deliberate:
//! the lattice hash is 32 bit (WGSL has no 64-bit integers), the lattice lives in position space (`position * freq / R`, not
//! `direction * freq`), and the amplitude factor `a0 * carve` comes from the fragment's own height and slope instead of the bake.
//! So it is a look-alike continuation, not the field's own octaves.
//!
//! CON-11: the shader never sees a planet-radius value. The CPU splits the lattice position of a tile origin in f64 into an
//! integer cell (`i32`) and a fraction (`f32`), like the relief grid's decade phases; the shader adds `local * cells_per_metre`.
//! Octaves fade out when a pixel covers more than 0.1 to 0.4 of a cell, so far terrain costs and aliases nothing.

// Spike code: platform maths is fine here (the CPU reference is a check, not a canonical field).
#![allow(clippy::disallowed_methods, clippy::needless_range_loop)]

use crate::terrain::{grid_phases, GRID_SPACINGS_M};
use crate::{read_rgba8, ExecError, GpuContext, RgbaFrame};
use planet_core::{TileId, Vec3};
use planet_frame::TerrainPlan;
use planet_generators::mesh::{grid_indices, MeshVertex, TileMesh};
use std::collections::HashMap;

/// Detail octaves, numbered from the field's last octave + 1.
pub const DETAIL_OCTAVES: usize = 11;
pub const FIRST_OCTAVE: u32 = 12;
const BASE_FREQUENCY: f64 = 256.0;
const PERSISTENCE: f64 = 0.55;
/// Field octaves summed in the field's normalisation.
const FIELD_OCTAVES: u32 = 12;
/// An octave is dropped for a node whose half-extent spans more than this many of its cells: the f32 position `fraction +
/// local * cells_per_metre` loses the 0.01 degree normal bound (CON-14) at the field's largest amplitude (980 m): the error
/// grows with the distance in cells and with the octave (emulated: 1.3e-2 degrees at 1,000 cells, under 6e-3 at 300). A pixel
/// resolves an octave only while it covers under 0.4 of a cell, and a tile is drawn at about 192 pixel footprints, so the
/// cut never removes an octave that is still visible.
pub const MAX_CELLS_PER_NODE: f64 = 300.0;

/// Bytes of the detail block of a node uniform: `up` (16), integer cells (11 x 16), fractions and cells per metre (11 x 16),
/// amplitudes (3 x 16, packed four to a vec4).
pub const DETAIL_BYTES: u64 = 16 + 16 * DETAIL_OCTAVES as u64 * 2 + 48;
/// Bytes of the per-node uniform: the product's block (origin, colour, morph, nine phases) and the detail block.
pub const NODE_BYTES: u64 = 48 + 16 * GRID_SPACINGS_M.len() as u64 + DETAIL_BYTES;

/// WGSL: the node's detail members, appended to the product's `Node` struct (and used alone by the probe kernel).
const DETAIL_MEMBERS: &str = "    up: vec4<f32>,
    dbase: array<vec4<i32>, 11>,
    dfrac: array<vec4<f32>, 11>,
    damp: array<vec4<f32>, 3>,
";

/// WGSL: the detail term. Needs a `node` uniform with [`DETAIL_MEMBERS`].
const DETAIL_FN: &str = r#"
// 32-bit integer hash of a lattice corner, exact on every adapter, to [-1, 1) in steps of 2^-23.
fn d_hash(ix: i32, iy: i32, iz: i32, oct: u32) -> f32 {
    var h = (bitcast<u32>(ix) * 0x8da6b343u) ^ (bitcast<u32>(iy) * 0xd8163841u) ^ (bitcast<u32>(iz) * 0xcb1ab31fu) ^ ((oct + 1u) * 0x9e3779b1u);
    h = h ^ (h >> 16u);
    h = h * 0x85ebca6bu;
    h = h ^ (h >> 13u);
    h = h * 0xc2b2ae35u;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) * 1.1920929e-7 - 1.0;
}

// Smoothstep value noise in one octave: xyz = gradient with respect to q, w = value.
fn d_noise(base: vec3<i32>, q: vec3<f32>, oct: u32) -> vec4<f32> {
    let fl = floor(q);
    let u = q - fl;
    let c = base + vec3<i32>(fl);
    let t = u * u * (3.0 - 2.0 * u);
    let dt = 6.0 * u * (1.0 - u);
    let c000 = d_hash(c.x, c.y, c.z, oct);
    let c100 = d_hash(c.x + 1, c.y, c.z, oct);
    let c010 = d_hash(c.x, c.y + 1, c.z, oct);
    let c110 = d_hash(c.x + 1, c.y + 1, c.z, oct);
    let c001 = d_hash(c.x, c.y, c.z + 1, oct);
    let c101 = d_hash(c.x + 1, c.y, c.z + 1, oct);
    let c011 = d_hash(c.x, c.y + 1, c.z + 1, oct);
    let c111 = d_hash(c.x + 1, c.y + 1, c.z + 1, oct);
    let x00 = mix(c000, c100, t.x);
    let x10 = mix(c010, c110, t.x);
    let x01 = mix(c001, c101, t.x);
    let x11 = mix(c011, c111, t.x);
    let value = mix(mix(x00, x10, t.y), mix(x01, x11, t.y), t.z);
    let gx = dt.x * mix(mix(c100 - c000, c110 - c010, t.y), mix(c101 - c001, c111 - c011, t.y), t.z);
    let gy = dt.y * mix(mix(c010 - c000, c110 - c100, t.x), mix(c011 - c001, c111 - c101, t.x), t.z);
    let gz = dt.z * mix(mix(c001 - c000, c101 - c100, t.x), mix(c011 - c010, c111 - c110, t.x), t.y);
    return vec4<f32>(gx, gy, gz, value);
}

// Gradient (metres per metre, tile-local axes) of the summed detail heights; `foot` is the pixel footprint in metres per axis,
// `amp0` the amplitude factor (field's a0 * carve, metres).
fn d_gradient(local: vec3<f32>, foot: vec3<f32>, amp0: f32) -> vec3<f32> {
    var grad = vec3<f32>(0.0);
    let fmax = max(foot.x, max(foot.y, foot.z));
    for (var k = 0u; k < 11u; k = k + 1u) {
        let inv = node.dfrac[k].w;
        let fade = 1.0 - smoothstep(0.1, 0.4, fmax * inv);
        if (fade > 0.0 && node.damp[k / 4u][k % 4u] > 0.0) {
            let q = node.dfrac[k].xyz + local * inv;
            let nz = d_noise(node.dbase[k].xyz, q, 12u + k);
            let t = max(1.0 - abs(nz.w), 0.0);
            let dc = 3.0 * t * t * sign(nz.w);
            grad = grad + (amp0 * node.damp[k / 4u][k % 4u] * fade * dc * inv) * nz.xyz;
        }
    }
    return grad;
}

// The shading normal with the detail slope added; the field's own a0 (grows with height) and carve (grows with slope).
fn d_normal(n: vec3<f32>, local: vec3<f32>, foot: vec3<f32>, height: f32) -> vec3<f32> {
    let c = clamp(dot(n, node.up.xyz), 0.05, 1.0);
    let slope = sqrt(max(1.0 - c * c, 0.0)) / c;
    let a0 = 80.0 + 900.0 * smoothstep(-200.0, 300.0, height);
    let carve = 0.15 + 0.85 * smoothstep(0.01, 0.2, slope);
    let g = d_gradient(local, foot, a0 * carve);
    return normalize(n - (g - dot(g, n) * n));
}
"#;

/// The product relief shader with the detail term spliced in (relief view only; `mode.y != 0` switches it on).
pub fn detail_shader() -> String {
    let src = crate::reference::reference_shader();
    let patch = |s: String, from: &str, to: String| {
        assert!(s.contains(from), "product shader changed: {from:?} not found");
        s.replacen(from, &to, 1)
    };
    let s = patch(src, "    phase: array<vec4<f32>, 9>,\n};", format!("    phase: array<vec4<f32>, 9>,\n{DETAIL_MEMBERS}}};"));
    let s = patch(s, "// Relief: fixed-sun", format!("{DETAIL_FN}\n// Relief: fixed-sun"));
    patch(
        s,
        "fn relief(in: VsOut) -> vec4<f32> {\n    let n = normalize(in.normal);",
        "fn relief(in: VsOut) -> vec4<f32> {\n    var n = normalize(in.normal);\n    if (g.mode.y != 0u) {\n        n = d_normal(n, in.local, abs(fwidth(in.local)), in.height);\n    }"
            .to_string(),
    )
}

/// The detail block of one node: the lattice position of its origin split in f64 into integer cell and fraction.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeDetail {
    pub up: [f32; 3],
    pub base: [[i32; 3]; DETAIL_OCTAVES],
    /// Fraction xyz and cells per metre.
    pub frac_inv: [[f32; 4]; DETAIL_OCTAVES],
    /// Amplitude of each octave in units of `a0 * carve` (metres).
    pub amp: [f32; DETAIL_OCTAVES],
}

/// Cells per metre of detail octave `k` (octave `FIRST_OCTAVE + k`) on a body of radius `radius_m`.
fn cells_per_metre(k: usize, radius_m: f64) -> f64 {
    BASE_FREQUENCY * f64::from(1u32 << (FIRST_OCTAVE as usize + k)) / radius_m
}

fn amplitude_unit(k: usize) -> f64 {
    let norm: f64 = (0..FIELD_OCTAVES).map(|o| PERSISTENCE.powi(o as i32)).sum();
    PERSISTENCE.powi(FIRST_OCTAVE as i32 + k as i32) / norm
}

impl NodeDetail {
    /// `extent_m`: the largest distance of any of the node's points from `origin` (half the tile diagonal).
    pub fn new(origin: Vec3, radius_m: f64, extent_m: f64) -> Self {
        let up = origin.normalized();
        let mut d = NodeDetail {
            up: [up.x as f32, up.y as f32, up.z as f32],
            base: [[0; 3]; DETAIL_OCTAVES],
            frac_inv: [[0.0; 4]; DETAIL_OCTAVES],
            amp: [0.0; DETAIL_OCTAVES],
        };
        for k in 0..DETAIL_OCTAVES {
            let inv = cells_per_metre(k, radius_m);
            let p = [origin.x * inv, origin.y * inv, origin.z * inv];
            for a in 0..3 {
                let f = p[a].floor();
                assert!(f.abs() < 2.0e9, "lattice index leaves i32");
                d.base[k][a] = f as i32;
                d.frac_inv[k][a] = (p[a] - f) as f32;
            }
            d.frac_inv[k][3] = inv as f32;
            d.amp[k] = if extent_m * inv <= MAX_CELLS_PER_NODE { amplitude_unit(k) as f32 } else { 0.0 };
        }
        d
    }

    /// Little-endian bytes in the layout of the WGSL members.
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(DETAIL_BYTES as usize);
        for v in [self.up[0], self.up[1], self.up[2], 0.0] {
            b.extend(v.to_le_bytes());
        }
        for base in &self.base {
            for v in [base[0], base[1], base[2], 0] {
                b.extend(v.to_le_bytes());
            }
        }
        for f in &self.frac_inv {
            for v in f {
                b.extend(v.to_le_bytes());
            }
        }
        for k in 0..12 {
            b.extend(self.amp.get(k).copied().unwrap_or(0.0).to_le_bytes());
        }
        debug_assert_eq!(b.len() as u64, DETAIL_BYTES);
        b
    }
}

fn hash32(ix: i32, iy: i32, iz: i32, oct: u32) -> f64 {
    let mut h = (ix as u32).wrapping_mul(0x8da6_b343)
        ^ (iy as u32).wrapping_mul(0xd816_3841)
        ^ (iz as u32).wrapping_mul(0xcb1a_b31f)
        ^ (oct + 1).wrapping_mul(0x9e37_79b1);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    f64::from((h >> 8) as f32 * (1.0 / 8_388_608.0) - 1.0)
}

/// CPU reference of the shader's gradient: f64 throughout, with the lattice position taken from the absolute position
/// `origin + local` (no integer/fraction split), so it also measures the error of the split. `foot` as in the shader. Also
/// returns the smallest `|noise value|` of the active octaves: the term's creases are kinks at value 0, so a probe with a
/// small margin has no well-defined gradient under f32 rounding.
pub fn reference_gradient(origin: Vec3, radius_m: f64, extent_m: f64, local: [f64; 3], foot: [f64; 3], amp0: f64) -> ([f64; 3], f64) {
    let fmax = foot[0].max(foot[1]).max(foot[2]);
    let mut grad = [0.0; 3];
    let mut margin = f64::MAX;
    for k in 0..DETAIL_OCTAVES {
        let inv = cells_per_metre(k, radius_m);
        if extent_m * inv > MAX_CELLS_PER_NODE {
            continue;
        }
        let w = (fmax * inv) as f32 as f64;
        let x = ((w - 0.1) / 0.3).clamp(0.0, 1.0);
        let fade = 1.0 - x * x * (3.0 - 2.0 * x);
        if fade <= 0.0 {
            continue;
        }
        let pos = [origin.x + local[0], origin.y + local[1], origin.z + local[2]];
        let p = [pos[0] * inv, pos[1] * inv, pos[2] * inv];
        let fl = [p[0].floor(), p[1].floor(), p[2].floor()];
        let u = [p[0] - fl[0], p[1] - fl[1], p[2] - fl[2]];
        let t = u.map(|u| u * u * (3.0 - 2.0 * u));
        let dt = u.map(|u| 6.0 * u * (1.0 - u));
        let c = [fl[0] as i32, fl[1] as i32, fl[2] as i32];
        let oct = FIRST_OCTAVE + k as u32;
        let h = |dx: i32, dy: i32, dz: i32| hash32(c[0] + dx, c[1] + dy, c[2] + dz, oct);
        let (c000, c100, c010, c110) = (h(0, 0, 0), h(1, 0, 0), h(0, 1, 0), h(1, 1, 0));
        let (c001, c101, c011, c111) = (h(0, 0, 1), h(1, 0, 1), h(0, 1, 1), h(1, 1, 1));
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let value = lerp(
            lerp(lerp(c000, c100, t[0]), lerp(c010, c110, t[0]), t[1]),
            lerp(lerp(c001, c101, t[0]), lerp(c011, c111, t[0]), t[1]),
            t[2],
        );
        let g = [
            dt[0] * lerp(lerp(c100 - c000, c110 - c010, t[1]), lerp(c101 - c001, c111 - c011, t[1]), t[2]),
            dt[1] * lerp(lerp(c010 - c000, c110 - c100, t[0]), lerp(c011 - c001, c111 - c101, t[0]), t[2]),
            dt[2] * lerp(lerp(c001 - c000, c101 - c100, t[0]), lerp(c011 - c010, c111 - c110, t[0]), t[1]),
        ];
        margin = margin.min(value.abs());
        let tt = (1.0 - value.abs()).max(0.0);
        let dc = 3.0 * tt * tt * value.signum() * f64::from(value != 0.0);
        let s = amp0 * amplitude_unit(k) * fade * dc * inv;
        for a in 0..3 {
            grad[a] += s * g[a];
        }
    }
    (grad, margin)
}

/// Run the shader's `d_gradient` on the GPU for each probe `(local, foot, amp0)` of a node and return the gradients. The probe
/// kernel splices the same WGSL text as the relief shader.
pub fn gpu_gradients(ctx: &GpuContext, detail: &NodeDetail, probes: &[([f32; 3], [f32; 3], f32)]) -> Vec<[f32; 3]> {
    use wgpu::util::DeviceExt;
    let source = format!(
        "struct Node {{\n{DETAIL_MEMBERS}}};\n@group(0) @binding(0) var<uniform> node: Node;\n\
         struct Probe {{ local: vec4<f32>, foot: vec4<f32>, amp: vec4<f32> }};\n\
         @group(0) @binding(1) var<storage, read> probes: array<Probe>;\n\
         @group(0) @binding(2) var<storage, read_write> outp: array<vec4<f32>>;\n{DETAIL_FN}\n\
         @compute @workgroup_size(64)\nfn cs(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
             if (id.x >= arrayLength(&probes)) {{ return; }}\n\
             let p = probes[id.x];\n\
             outp[id.x] = vec4<f32>(d_gradient(p.local.xyz, p.foot.xyz, p.amp.x), 0.0);\n}}\n"
    );
    let device = &ctx.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("spike07-probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let node = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("probe-node"),
        contents: &detail.bytes(),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let flat: Vec<u8> = probes
        .iter()
        .flat_map(|(l, f, a)| [l[0], l[1], l[2], 0.0, f[0], f[1], f[2], 0.0, *a, 0.0, 0.0, 0.0].into_iter().flat_map(f32::to_le_bytes))
        .collect();
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("probes"),
        contents: &flat,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_bytes = probes.len() as u64 * 16;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe-out"),
        size: out_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe-readback"),
        size: out_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("probe"),
        layout: None,
        module: &module,
        entry_point: Some("cs"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probe-bind"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: node.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: input.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: output.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((probes.len() as u32).div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, out_bytes);
    ctx.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("probe map failed"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("probe range");
    let f = |k: usize| f32::from_le_bytes(data[k * 4..k * 4 + 4].try_into().expect("4 bytes"));
    let result = (0..probes.len()).map(|i| [f(i * 4), f(i * 4 + 1), f(i * 4 + 2)]).collect();
    drop(data);
    readback.unmap();
    result
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The product terrain renderer's pipeline with the detail shader: same vertex format, same draws, one switch (`detail`).
pub struct DetailRenderer {
    size: (u32, u32),
    cells: u32,
    radius_m: f64,
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    globals: wgpu::Buffer,
    nodes: wgpu::Buffer,
    node_capacity: usize,
    bind: wgpu::BindGroup,
    stride: u64,
    indices: wgpu::Buffer,
    index_count: u32,
    vertex_buffers: HashMap<TileId, (u64, wgpu::Buffer)>,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl DetailRenderer {
    pub fn new(ctx: &GpuContext, size: (u32, u32), cells: u32, radius_m: f64) -> Self {
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
        let color = target("detail-color", format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let depth = target("detail-depth", wgpu::TextureFormat::Depth32Float, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let (color_view, depth_view) = (color.create_view(&Default::default()), depth.create_view(&Default::default()));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stride = NODE_BYTES.next_multiple_of(u64::from(device.limits().min_uniform_buffer_offset_alignment));
        let node_capacity = 256;
        let nodes = Self::node_buffer(device, stride, node_capacity);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain-detail"),
            source: wgpu::ShaderSource::Wgsl(detail_shader().into()),
        });
        let uniform = |binding, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: if dynamic { wgpu::BufferSize::new(NODE_BYTES) } else { None },
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("detail-bgl"),
            entries: &[uniform(0, false), uniform(1, true)],
        });
        let bind = Self::bind_group(device, &bgl, &globals, &nodes);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("detail-layout"),
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
            attr(wgpu::VertexFormat::Float32, 4, 4),
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("detail-pipeline"),
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
            label: Some("detail-indices"),
            contents: &index_data.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            size,
            cells,
            radius_m,
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
        }
    }

    fn node_buffer(device: &wgpu::Device, stride: u64, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nodes"),
            size: stride * capacity as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, nodes: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("detail-bind"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: nodes,
                        offset: 0,
                        size: wgpu::BufferSize::new(NODE_BYTES),
                    }),
                },
            ],
        })
    }

    pub fn upload(&mut self, ctx: &GpuContext, mesh: &TileMesh) -> Result<(), ExecError> {
        use wgpu::util::DeviceExt;
        if mesh.cells != self.cells {
            return Err(ExecError::BadResolution { id: mesh.id, res: mesh.cells });
        }
        if self.vertex_buffers.get(&mesh.id).is_some_and(|(key, _)| *key == mesh.key) {
            return Ok(());
        }
        let bytes: Vec<u8> = mesh.vertices.iter().flat_map(|v| f32_bytes(&v.to_floats())).collect();
        let buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("detail-vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.vertex_buffers.insert(mesh.id, (mesh.key, buffer));
        Ok(())
    }

    pub fn has_mesh(&self, tile: TileId, key: u64) -> bool {
        self.vertex_buffers.get(&tile).is_some_and(|(k, _)| *k == key)
    }

    pub fn resident_tiles(&self) -> usize {
        self.vertex_buffers.len()
    }

    /// Draw the plan; `detail` switches the detail term on (it only has an effect in the relief view).
    pub fn render(&mut self, ctx: &GpuContext, plan: &TerrainPlan, detail: bool) -> Result<(), ExecError> {
        if plan.size != self.size {
            return Err(ExecError::SizeMismatch { plan: plan.size, renderer: self.size });
        }
        for d in &plan.nodes {
            match self.vertex_buffers.get(&d.tile) {
                None => return Err(ExecError::MissingTile(d.tile)),
                Some((key, _)) if *key != d.mesh_key => {
                    return Err(ExecError::StaleMesh { tile: d.tile, resident: *key, planned: d.mesh_key })
                }
                Some(_) => {}
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
        let view_index = planet_frame::TerrainView::ALL.iter().position(|x| *x == plan.view).expect("known view") as u32;
        let mut globals = f32_bytes(&plan.view_projection);
        globals.extend(f32_bytes(&[lo, hi, 0.0, 0.0]));
        globals.extend([view_index, u32::from(detail), 0, 0].iter().flat_map(|v: &u32| v.to_le_bytes()));
        ctx.queue.write_buffer(&self.globals, 0, &globals);
        // `SPIKE07_GAIN`: amplitude multiplier for looking at what a stronger term would show (default 1: the field's own law).
        let gain: f32 = std::env::var("SPIKE07_GAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let stride = self.stride as usize;
        let mut nodes = vec![0u8; stride * plan.nodes.len().max(1)];
        for (i, d) in plan.nodes.iter().enumerate() {
            let r = d.camera_relative_origin;
            let c = d.color.map(|v| f32::from(v) / 255.0);
            let mut floats = vec![r[0], r[1], r[2], 0.0, c[0], c[1], c[2], c[3], d.morph_start_m, d.morph_end_m, 0.0, 0.0];
            for p in grid_phases([d.origin.0.x, d.origin.0.y, d.origin.0.z]) {
                floats.extend([p[0], p[1], p[2], 0.0]);
            }
            let mut b = f32_bytes(&floats);
            if detail {
                // Half the tile diagonal, generously: the tile edge at the middle of a face, times one.
                let extent = self.radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << d.level);
                let mut nd = NodeDetail::new(d.origin.0, self.radius_m, extent);
                nd.amp = nd.amp.map(|a| a * gain);
                b.extend(nd.bytes());
            } else {
                b.resize(NODE_BYTES as usize, 0);
            }
            nodes[i * stride..i * stride + NODE_BYTES as usize].copy_from_slice(&b);
        }
        ctx.queue.write_buffer(&self.nodes, 0, &nodes);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("detail") });
        {
            let [r, g, b, a] = plan.clear_color.map(|v| f64::from(v) / 255.0);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("detail-pass"),
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
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            for (i, d) in plan.nodes.iter().enumerate() {
                pass.set_bind_group(0, &self.bind, &[(i as u64 * self.stride) as u32]);
                pass.set_vertex_buffer(0, self.vertex_buffers[&d.tile].1.slice(..));
                pass.draw_indexed(0..self.index_count, 0, 0..1);
            }
        }
        ctx.queue.submit([encoder.finish()]);
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        Ok(())
    }

    pub fn read_color(&self, ctx: &GpuContext) -> RgbaFrame {
        read_rgba8(ctx, &self.color, self.size.0, self.size.1)
    }
}

/// The detail shader's source, for the CON-11 scan.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![("terrain-detail", detail_shader())]
}
