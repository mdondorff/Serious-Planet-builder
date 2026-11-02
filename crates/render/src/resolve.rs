//! The shadow's receivers, upsample and temporal accumulation (REND-050, ADR 0085 point 5): from the depth buffer to the receivers of a
//! half-resolution march, a depth-aware 4-tap upsample of its visibility, and a temporal pass that reprojects the previous frame's shadow,
//! clamps it to the 3x3 neighbourhood and blends it at a weight of at most 0.8. History is rejected after a camera cut and where the march
//! crossed a clipmap cell that changed this frame or the last (a per-level generation texture, [`GenerationMap`]). Below 5 degrees of sun
//! half of the half-resolution rays are marched each frame in a checkerboard and the rest are taken from history.
//!
//! Positions are in the anchor's tangent frame (x east, y north, z up), the frame of the march: camera-relative f32, so the shaders hold no
//! planet-radius value (CON-11). The passes use no atomics and no order-dependent reduction (REND-053).

use planet_frame::clipmap::TexelRect;

use crate::march::{read_texture, MarchParams, ShadowMarchGpu, GENERATION_CELL_SHIFT};
use crate::shadow::{ClipmapGpu, CoarseCubeGpu};
use crate::GpuContext;

/// The history weight cap (REND-050).
pub const MAX_HISTORY_WEIGHT: f32 = 0.8;
/// Below this sun elevation the march rays alternate in a checkerboard (REND-050), degrees.
pub const SPREAD_BELOW_SUN_DEG: f32 = 5.0;
/// The sine of [`SPREAD_BELOW_SUN_DEG`].
pub const SPREAD_BELOW_SUN_SIN: f32 = 0.087_155_74;

/// Whether a sun at `sin_elevation` (the z of its unit direction at the receivers) calls for the two-frame spread.
pub fn spread_for_sun(sin_elevation: f32) -> bool {
    sin_elevation < SPREAD_BELOW_SUN_SIN
}

const COMMON: &str = r#"
struct Cam {
    pos: vec4<f32>,
    fwd: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // x, y: the tangents of the half field of view; z: the near plane (reversed-Z, infinite far: depth = near / distance along forward).
    k: vec4<f32>,
};
struct Resolve {
    cur: Cam,
    prev: Cam,
    // The full-resolution size, the half-resolution size.
    dims: vec4<u32>,
    // x: 1 when half of the rays are skipped this frame; y: the parity of the rays that are marched; z: 1 when history is void;
    // w: unused.
    mode: vec4<u32>,
    // x: the history weight; y: the drop coefficient 1 / (2 R) (a uniform, never R itself).
    blend: vec4<f32>,
};
@group(0) @binding(0) var<uniform> r: Resolve;

// The direction (forward component 1) of the full-resolution pixel centre `px` of a camera.
fn view_dir(c: Cam, px: vec2<f32>) -> vec3<f32> {
    let ndc = vec2<f32>(px.x / f32(r.dims.x) * 2.0 - 1.0, 1.0 - px.y / f32(r.dims.y) * 2.0);
    return c.fwd.xyz + c.right.xyz * (ndc.x * c.k.x) + c.up.xyz * (ndc.y * c.k.y);
}
"#;

const RECEIVERS_BODY: &str = r#"
@group(0) @binding(1) var depth: texture_depth_2d;
// x, y, z in the anchor frame; w is 1 for a receiver to march, -1 for one taken from history this frame and 0 for a pixel without one.
@group(0) @binding(2) var out_receivers: texture_storage_2d<rgba32float, write>;
// The distance along the camera's forward of the receiver, 0 without one.
@group(0) @binding(3) var out_depth: texture_storage_2d<r32float, write>;
// The same receivers in the camera's own (Euclidean) frame, for reprojection.
@group(0) @binding(4) var out_world: texture_storage_2d<rgba32float, write>;

// A point of the anchor's Euclidean tangent frame in the clipmap's frame: the azimuthal equidistant plane position (arc length from the
// anchor, not the chord) and the height above the sphere minus the sphere's drop at that distance. With c = 1 / (2 R) the sphere's centre
// sits at z = -1 / (2 c), so t = tan(theta) = 2 c rho / (1 + 2 c z), the plane distance is theta / (2 c) and the drop sin^2(theta / 2) / c.
// Every term is built from c and small quantities, never from R (CON-11), and without trigonometry (a shader atan2 may be off by 4096 ulp,
// which is 20 m at 400 km): theta / t is a series in t^2 (t is at most 0.2 within 1,000 km, the seventh term is 1e-9), cos(theta) is
// 1 / sqrt(1 + t^2) and 1 - cos(theta) is t^2 cos^2(theta) / (1 + cos(theta)), free of cancellation.
fn to_plane(e: vec3<f32>, c: f32) -> vec3<f32> {
    let a = 1.0 + 2.0 * c * e.z;
    if (c <= 0.0 || a < 0.1) {
        return e;
    }
    let t = 2.0 * c * length(e.xy) / a;
    let t2 = t * t;
    let ct = inverseSqrt(1.0 + t2);
    let omc = t2 * ct * ct / (1.0 + ct);
    let series = 1.0 - t2 * (1.0 / 3.0 - t2 * (1.0 / 5.0 - t2 * (1.0 / 7.0 - t2 * (1.0 / 9.0))));
    // height - drop = z / cos(theta) + (sin^2(theta / 2) / c) (1 - cos(theta)) / cos(theta), with sin^2(theta / 2) = (1 - cos(theta)) / 2.
    let z = e.z / ct + (0.5 * omc / c) * omc / ct;
    return vec3<f32>(e.xy * (series / a), z);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= r.dims.z || gid.y >= r.dims.w) {
        return;
    }
    let hp = vec2<i32>(gid.xy);
    let last = vec2<i32>(i32(r.dims.x) - 1, i32(r.dims.y) - 1);
    // The receiver of a half-resolution pixel is the nearest of its 2x2 full-resolution pixels (reversed-Z: the largest depth).
    var best = 0.0;
    var bp = vec2<i32>(0, 0);
    for (var j = 0; j < 2; j = j + 1) {
        for (var i = 0; i < 2; i = i + 1) {
            let fp = min(hp * 2 + vec2<i32>(i, j), last);
            let d = textureLoad(depth, fp, 0);
            if (d > best) {
                best = d;
                bp = fp;
            }
        }
    }
    var receiver = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var world = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var z = 0.0;
    if (best > 0.0) {
        z = r.cur.k.z / best;
        let pos = r.cur.pos.xyz + view_dir(r.cur, vec2<f32>(bp) + vec2<f32>(0.5, 0.5)) * z;
        var w = 1.0;
        if (r.mode.x == 1u && ((u32(hp.x + hp.y) + r.mode.y) & 1u) != 0u) {
            w = -1.0;
        }
        receiver = vec4<f32>(to_plane(pos, r.blend.y), w);
        world = vec4<f32>(pos, w);
    }
    textureStore(out_world, hp, world);
    textureStore(out_receivers, hp, receiver);
    textureStore(out_depth, hp, vec4<f32>(z, 0.0, 0.0, 0.0));
}
"#;

const FILL_BODY: &str = r#"
@group(0) @binding(1) var half_vis: texture_2d<f32>;
@group(0) @binding(2) var half_steps: texture_2d<u32>;
@group(0) @binding(3) var receivers: texture_2d<f32>;
@group(0) @binding(4) var prev_filled: texture_2d<f32>;
// The half-resolution visibility with the rays the checkerboard skipped taken from the previous frame's: -1 where there is none.
@group(0) @binding(5) var out_filled: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= r.dims.z || gid.y >= r.dims.w) {
        return;
    }
    let hp = vec2<i32>(gid.xy);
    let v = textureLoad(half_vis, hp, 0).x;
    if (v >= 0.0) {
        textureStore(out_filled, hp, vec4<f32>(v, 0.0, 0.0, 0.0));
        return;
    }
    let hmax = vec2<i32>(i32(r.dims.z) - 1, i32(r.dims.w) - 1);
    // A ray beside a march that crossed a changed clipmap cell has no valid history.
    var changed = false;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            if ((textureLoad(half_steps, clamp(hp + vec2<i32>(i, j), vec2<i32>(0, 0), hmax), 0).x >> 31u) != 0u) {
                changed = true;
            }
        }
    }
    var out = -1.0;
    let rc = textureLoad(receivers, hp, 0);
    if (r.mode.z == 0u && !changed && rc.w < -0.5) {
        let rel = rc.xyz - r.prev.pos.xyz;
        let zc = dot(rel, r.prev.fwd.xyz);
        if (zc > r.prev.k.z) {
            let ndc = vec2<f32>(dot(rel, r.prev.right.xyz) / (zc * r.prev.k.x), dot(rel, r.prev.up.xyz) / (zc * r.prev.k.y));
            let uv = vec2<f32>((ndc.x * 0.5 + 0.5) * f32(r.dims.x), (0.5 - ndc.y * 0.5) * f32(r.dims.y));
            if (uv.x >= 0.0 && uv.y >= 0.0 && uv.x < f32(r.dims.x) && uv.y < f32(r.dims.y)) {
                out = textureLoad(prev_filled, clamp(vec2<i32>(floor(uv * 0.5)), vec2<i32>(0, 0), hmax), 0).x;
            }
        }
    }
    textureStore(out_filled, hp, vec4<f32>(out, 0.0, 0.0, 0.0));
}
"#;

const UPSAMPLE_BODY: &str = r#"
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var half_vis: texture_2d<f32>;
@group(0) @binding(3) var half_depth: texture_2d<f32>;
@group(0) @binding(4) var half_steps: texture_2d<u32>;
// x: the visibility, -1 when no marched tap lies on this pixel's surface; y: 1 when a tap crossed a changed clipmap cell.
@group(0) @binding(5) var out_up: texture_storage_2d<rg32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= r.dims.x || gid.y >= r.dims.y) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let d = textureLoad(depth, p, 0);
    if (d <= 0.0) {
        textureStore(out_up, p, vec4<f32>(-1.0, 0.0, 0.0, 0.0));
        return;
    }
    let zf = r.cur.k.z / d;
    let hx = (f32(p.x) + 0.5) * 0.5 - 0.5;
    let hy = (f32(p.y) + 0.5) * 0.5 - 0.5;
    let x0 = i32(floor(hx));
    let y0 = i32(floor(hy));
    let fx = hx - floor(hx);
    let fy = hy - floor(hy);
    let hmax = vec2<i32>(i32(r.dims.z) - 1, i32(r.dims.w) - 1);
    var sum = 0.0;
    var acc = 0.0;
    var changed = 0.0;
    for (var j = 0; j < 2; j = j + 1) {
        for (var i = 0; i < 2; i = i + 1) {
            let tp = clamp(vec2<i32>(x0 + i, y0 + j), vec2<i32>(0, 0), hmax);
            var bw = fx;
            if (i == 0) {
                bw = 1.0 - fx;
            }
            if (j == 0) {
                bw = bw * (1.0 - fy);
            } else {
                bw = bw * fy;
            }
            let zh = textureLoad(half_depth, tp, 0).x;
            let v = textureLoad(half_vis, tp, 0).x;
            if (zh <= 0.0 || v < 0.0) {
                continue;
            }
            // A tap on another surface counts for little: the weight falls with the relative depth difference.
            let rel = abs(zh - zf) / (0.03 * zf + 0.5);
            // A tap past a few times the depth tolerance is another surface altogether and counts for nothing.
            if (rel > 3.0) {
                continue;
            }
            let w = bw / (1.0 + 16.0 * rel * rel);
            sum = sum + w;
            acc = acc + w * v;
            if ((textureLoad(half_steps, tp, 0).x >> 31u) != 0u) {
                changed = 1.0;
            }
        }
    }
    var vis = -1.0;
    if (sum > 1.0e-6) {
        vis = acc / sum;
    }
    textureStore(out_up, p, vec4<f32>(vis, changed, 0.0, 0.0));
}
"#;

const TEMPORAL_BODY: &str = r#"
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var up: texture_2d<f32>;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var out_shadow: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= r.dims.x || gid.y >= r.dims.y) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let last = vec2<i32>(i32(r.dims.x) - 1, i32(r.dims.y) - 1);
    let d = textureLoad(depth, p, 0);
    if (d <= 0.0) {
        textureStore(out_shadow, p, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }
    let here = textureLoad(up, p, 0);
    var lo = 2.0;
    var hi = -1.0;
    var near_changed = 0.0;
    var mean = 0.0;
    var n = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let t = textureLoad(up, clamp(p + vec2<i32>(i, j), vec2<i32>(0, 0), last), 0);
            near_changed = max(near_changed, t.y);
            if (t.x >= 0.0) {
                lo = min(lo, t.x);
                hi = max(hi, t.x);
                mean = mean + t.x;
                n = n + 1.0;
            }
        }
    }
    // Where the surface sat in the previous frame.
    let zf = r.cur.k.z / d;
    let pos = r.cur.pos.xyz + view_dir(r.cur, vec2<f32>(p) + vec2<f32>(0.5, 0.5)) * zf;
    let rel = pos - r.prev.pos.xyz;
    let zc = dot(rel, r.prev.fwd.xyz);
    var reject = r.mode.z == 1u;
    var hist = 1.0;
    if (!(zc > r.prev.k.z)) {
        reject = true;
    } else {
        let ndc = vec2<f32>(dot(rel, r.prev.right.xyz) / (zc * r.prev.k.x), dot(rel, r.prev.up.xyz) / (zc * r.prev.k.y));
        let uv = vec2<f32>((ndc.x * 0.5 + 0.5) * f32(r.dims.x), (0.5 - ndc.y * 0.5) * f32(r.dims.y));
        if (!(uv.x >= 0.0 && uv.y >= 0.0 && uv.x < f32(r.dims.x) && uv.y < f32(r.dims.y))) {
            reject = true;
        } else {
            let q = uv - vec2<f32>(0.5, 0.5);
            let q0 = vec2<i32>(floor(q));
            let f = q - floor(q);
            let h00 = textureLoad(history, clamp(q0, vec2<i32>(0, 0), last), 0).x;
            let h10 = textureLoad(history, clamp(q0 + vec2<i32>(1, 0), vec2<i32>(0, 0), last), 0).x;
            let h01 = textureLoad(history, clamp(q0 + vec2<i32>(0, 1), vec2<i32>(0, 0), last), 0).x;
            let h11 = textureLoad(history, clamp(q0 + vec2<i32>(1, 1), vec2<i32>(0, 0), last), 0).x;
            hist = mix(mix(h00, h10, f.x), mix(h01, h11, f.x), f.y);
        }
    }
    var result = 1.0;
    if (here.x >= 0.0) {
        // A marched pixel: blend with the history clamped to what its neighbours show now.
        if (here.y > 0.5) {
            reject = true;
        }
        // A rejected history is not read at all (it may hold anything).
        if (reject) {
            result = here.x;
        } else {
            result = mix(here.x, clamp(hist, lo, hi), r.blend.x);
        }
    } else {
        // A pixel the checkerboard skipped: history, unless the clipmap changed around it, then what the neighbours show.
        if (near_changed > 0.5) {
            reject = true;
        }
        if (!reject) {
            if (n > 0.0) {
                result = clamp(hist, lo, hi);
            } else {
                result = hist;
            }
        } else if (n > 0.0) {
            result = mean / n;
        }
    }
    textureStore(out_shadow, p, vec4<f32>(result, 0.0, 0.0, 0.0));
}
"#;

/// The receivers shader (a function so the shader scan and the validation test can reach it).
pub fn receivers_shader() -> String {
    format!("{COMMON}{RECEIVERS_BODY}")
}

/// The fill shader: skipped rays take the previous frame's half-resolution value.
pub fn fill_shader() -> String {
    format!("{COMMON}{FILL_BODY}")
}

/// The upsample shader.
pub fn upsample_shader() -> String {
    format!("{COMMON}{UPSAMPLE_BODY}")
}

/// The temporal shader.
pub fn temporal_shader() -> String {
    format!("{COMMON}{TEMPORAL_BODY}")
}

/// A pinhole camera in the anchor's tangent frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolveCamera {
    pub position: [f32; 3],
    /// Orthonormal basis: where the camera looks, its right and its up.
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    /// The tangents of the half field of view, horizontal and vertical.
    pub tan_half_fov: [f32; 2],
    /// The near plane of the reversed-Z infinite projection (depth = near / distance along forward).
    pub near_m: f32,
}

/// The inputs of one frame of the resolve.
#[derive(Clone, Copy, Debug)]
pub struct ResolveParams {
    pub camera: ResolveCamera,
    /// The camera of the previous frame, for reprojection.
    pub previous: ResolveCamera,
    /// The weight of the history, at most [`MAX_HISTORY_WEIGHT`]; 0 draws the frame alone.
    pub history_weight: f32,
    /// A camera cut or teleport, or a re-anchor of the clipmap: history is void and every ray is marched.
    pub cut: bool,
    /// The frame number, counting from 1 (the generation values of [`GenerationMap`] use the same count).
    pub frame: u32,
    /// The sun is low enough for the checkerboard ([`spread_for_sun`]).
    pub spread: bool,
}

const DEPTH_WRITE_SHADER: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
struct Out {
    @builtin(frag_depth) depth: f32,
};
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> Out {
    var out: Out;
    out.depth = textureLoad(source, vec2<i32>(pos.xy), 0).x;
    return out;
}
"#;

/// A depth buffer for tests and probes (`Depth32Float`, reversed-Z, 0 for the sky); the frame path hands the lit pass's own.
pub struct DepthTarget {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

impl DepthTarget {
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> DepthTarget {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("resolve-depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        DepthTarget { texture, width, height }
    }

    /// Write `width * height` reversed-Z depths, row-major (a depth texture takes no copy, so a pass draws them through `frag_depth`).
    pub fn write(&self, ctx: &GpuContext, depths: &[f32]) {
        assert_eq!(depths.len(), (self.width * self.height) as usize, "one depth a pixel");
        let device = &ctx.device;
        let size = wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 };
        let source = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("resolve-depth-source"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data: Vec<u8> = depths.iter().flat_map(|v| v.to_le_bytes()).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &source, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.width * 4), rows_per_image: Some(self.height) },
            size,
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("resolve-depth-write"),
            source: wgpu::ShaderSource::Wgsl(DEPTH_WRITE_SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("resolve-depth-write"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("resolve-depth-write"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("resolve-depth-write"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let source_view = source.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resolve-depth-write"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&source_view) }],
        });
        let target = self.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("resolve-depth-write") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("resolve-depth-write"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        ctx.queue.submit([encoder.finish()]);
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// Per level, the frame at which each 8x8-texel cell of the clipmap last changed (REND-050), with a max mip chain (a cell of mip `g` is
/// the maximum over its children): the march reports whether it walked a cell changed this frame or the last, and the temporal pass drops
/// its history there. Cells are addressed toroidally like the clipmap.
pub struct GenerationMap {
    texture: wgpu::Texture,
    cells: u32,
    levels: usize,
    /// Per level, per mip, the values row-major.
    mirror: Vec<Vec<Vec<u32>>>,
    /// The latest frame any cell was marked in (0: never).
    last_marked: u32,
}

impl GenerationMap {
    pub fn new(ctx: &GpuContext, texels: u32, levels: usize) -> GenerationMap {
        assert!(texels.is_power_of_two() && texels >= 8, "the generation cells need a clipmap of at least 8 texels");
        let cells = texels >> GENERATION_CELL_SHIFT;
        let mips = cells.trailing_zeros() + 1;
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("clipmap-gen"),
            size: wgpu::Extent3d { width: cells, height: cells, depth_or_array_layers: levels as u32 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let chain = || (0..mips).map(|g| vec![0u32; ((cells >> g) * (cells >> g)) as usize]).collect::<Vec<_>>();
        let map = GenerationMap { texture, cells, levels, mirror: (0..levels).map(|_| chain()).collect(), last_marked: 0 };
        for level in 0..levels {
            map.upload(ctx, level);
        }
        map
    }

    pub(crate) fn view(&self) -> wgpu::TextureView {
        self.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("clipmap-gen"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    }

    fn upload(&self, ctx: &GpuContext, level: usize) {
        for (g, values) in self.mirror[level].iter().enumerate() {
            let n = self.cells >> g;
            let data: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: g as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: level as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(n) },
                wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 },
            );
        }
    }

    /// Rebuild the mips of `level` from its mip 0.
    fn rebuild(&mut self, level: usize) {
        let chain = &mut self.mirror[level];
        for g in 1..chain.len() {
            let n = (self.cells >> g) as usize;
            for y in 0..n {
                for x in 0..n {
                    let prev = &chain[g - 1];
                    let w = 2 * n;
                    let v = prev[2 * y * w + 2 * x]
                        .max(prev[2 * y * w + 2 * x + 1])
                        .max(prev[(2 * y + 1) * w + 2 * x])
                        .max(prev[(2 * y + 1) * w + 2 * x + 1]);
                    chain[g][y * n + x] = v;
                }
            }
        }
    }

    /// Mark the cells of each rectangle (world texel indices of its level) as changed in `frame` (counting from 1), rebuilding and uploading
    /// each touched level once however many rectangles it has. A march reads the level whose window holds it, so a change must be marked
    /// in every level whose window it intersects. The mip chain is a maximum in slot space: a coarse cell of a window not aligned to it may
    /// flag a walk near the opposite window edge too (extra rejection, never a miss).
    pub fn mark_rects<'a>(&mut self, ctx: &GpuContext, frame: u32, rects: impl IntoIterator<Item = (usize, &'a TexelRect)>) {
        assert!(frame > 0);
        self.last_marked = self.last_marked.max(frame);
        let n = i64::from(self.cells);
        let mut touched = vec![false; self.levels];
        for (level, rect) in rects {
            assert!(level < self.levels);
            if rect.is_empty() {
                continue;
            }
            touched[level] = true;
            let (cx0, cx1) = (rect.x0 >> GENERATION_CELL_SHIFT, (rect.x1 - 1) >> GENERATION_CELL_SHIFT);
            let (cy0, cy1) = (rect.y0 >> GENERATION_CELL_SHIFT, (rect.y1 - 1) >> GENERATION_CELL_SHIFT);
            for cy in cy0..=cy1.min(cy0 + n - 1) {
                for cx in cx0..=cx1.min(cx0 + n - 1) {
                    let (sx, sy) = (cx.rem_euclid(n) as usize, cy.rem_euclid(n) as usize);
                    self.mirror[level][0][sy * self.cells as usize + sx] = frame;
                }
            }
        }
        for level in (0..self.levels).filter(|l| touched[*l]) {
            self.rebuild(level);
            self.upload(ctx, level);
        }
    }

    /// [`GenerationMap::mark_rects`] for one rectangle.
    pub fn mark(&mut self, ctx: &GpuContext, frame: u32, level: usize, rect: &TexelRect) {
        self.mark_rects(ctx, frame, [(level, rect)]);
    }

    /// Mark every cell of every level (a re-anchor).
    /// The latest frame any cell was marked in (0: never).
    pub fn last_marked(&self) -> u32 {
        self.last_marked
    }

    pub fn mark_all(&mut self, ctx: &GpuContext, frame: u32) {
        assert!(frame > 0);
        self.last_marked = self.last_marked.max(frame);
        for level in 0..self.levels {
            self.mirror[level][0].iter_mut().for_each(|v| *v = frame);
            self.rebuild(level);
            self.upload(ctx, level);
        }
    }
}

struct Targets {
    width: u32,
    height: u32,
    receivers: wgpu::Texture,
    world: wgpu::Texture,
    half_depth: wgpu::Texture,
    half_vis: wgpu::Texture,
    half_steps: wgpu::Texture,
    filled: [wgpu::Texture; 2],
    up: wgpu::Texture,
    shadow: [wgpu::Texture; 2],
}

/// The five passes of a resolve, in order (the timing probe's rows).
pub const RESOLVE_PASS_NAMES: [&str; 5] = ["receivers", "march", "fill", "upsample", "temporal"];

/// GPU timestamps around each pass of the resolve (a probe, off unless [`ShadowResolveGpu::enable_timing`] is called).
struct PassTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}

impl PassTimer {
    fn writes(&self, pass: usize) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(2 * pass as u32),
            end_of_pass_write_index: Some(2 * pass as u32 + 1),
        }
    }
}

/// The receivers, upsample and temporal passes with their history. One instance per view.
pub struct ShadowResolveGpu {
    receivers: wgpu::ComputePipeline,
    receivers_bgl: wgpu::BindGroupLayout,
    fill: wgpu::ComputePipeline,
    fill_bgl: wgpu::BindGroupLayout,
    upsample: wgpu::ComputePipeline,
    upsample_bgl: wgpu::BindGroupLayout,
    temporal: wgpu::ComputePipeline,
    temporal_bgl: wgpu::BindGroupLayout,
    generations: GenerationMap,
    targets: Option<Targets>,
    timer: Option<PassTimer>,
    uniform: Option<wgpu::Buffer>,
    march_uniform: Option<wgpu::Buffer>,
    /// The index of the shadow texture holding the last frame.
    current: usize,
    /// The next frame must void the history: first frame or a new size.
    fresh: bool,
}

impl ShadowResolveGpu {
    pub fn new(ctx: &GpuContext, clip: &ClipmapGpu) -> ShadowResolveGpu {
        let device = &ctx.device;
        let c = wgpu::ShaderStages::COMPUTE;
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: c,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let depth = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: c,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampled = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: c,
            ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: false };
        let storage = |binding, format| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: c,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format,
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        };
        let pipeline = |label: &str, source: String, entries: &[wgpu::BindGroupLayoutEntry]| {
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            let module = device
                .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            (pipeline, bgl)
        };
        let (receivers, receivers_bgl) = pipeline(
            "shadow-receivers",
            receivers_shader(),
            &[
                uniform,
                depth(1),
                storage(2, wgpu::TextureFormat::Rgba32Float),
                storage(3, wgpu::TextureFormat::R32Float),
                storage(4, wgpu::TextureFormat::Rgba32Float),
            ],
        );
        let (fill, fill_bgl) = pipeline(
            "shadow-fill",
            fill_shader(),
            &[
                uniform,
                sampled(1, float),
                sampled(2, wgpu::TextureSampleType::Uint),
                sampled(3, float),
                sampled(4, float),
                storage(5, wgpu::TextureFormat::R32Float),
            ],
        );
        let (upsample, upsample_bgl) = pipeline(
            "shadow-upsample",
            upsample_shader(),
            &[
                uniform,
                depth(1),
                sampled(2, float),
                sampled(3, float),
                sampled(4, wgpu::TextureSampleType::Uint),
                storage(5, wgpu::TextureFormat::Rg32Float),
            ],
        );
        let (temporal, temporal_bgl) = pipeline(
            "shadow-temporal",
            temporal_shader(),
            &[uniform, depth(1), sampled(2, float), sampled(3, float), storage(4, wgpu::TextureFormat::R32Float)],
        );
        ShadowResolveGpu {
            receivers,
            receivers_bgl,
            fill,
            fill_bgl,
            upsample,
            upsample_bgl,
            temporal,
            temporal_bgl,
            generations: GenerationMap::new(ctx, clip.texels(), clip.levels()),
            targets: None,
            timer: None,
            uniform: None,
            march_uniform: None,
            current: 0,
            fresh: true,
        }
    }

    /// Time each pass with GPU timestamps from the next frame on; false when the device has no timestamp queries.
    pub fn enable_timing(&mut self, ctx: &GpuContext) -> bool {
        if !ctx.timestamps_supported() {
            return false;
        }
        let n = 2 * RESOLVE_PASS_NAMES.len() as u32;
        let bytes = u64::from(n) * 8;
        self.timer = Some(PassTimer {
            queries: ctx.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("shadow-resolve-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: n,
            }),
            resolve: ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-resolve-timestamps-resolve"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-resolve-timestamps-readback"),
                size: bytes,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
        });
        true
    }

    /// The GPU milliseconds of each pass of the last frame ([`RESOLVE_PASS_NAMES`] order), `None` without [`ShadowResolveGpu::enable_timing`].
    pub fn pass_ms(&self, ctx: &GpuContext) -> Option<[f64; 5]> {
        let t = self.timer.as_ref()?;
        let slice = t.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("timestamp map failed"));
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let data = slice.get_mapped_range().expect("timestamp range");
        let tick = |k: usize| u64::from_le_bytes(data[k * 8..k * 8 + 8].try_into().expect("8 bytes"));
        let period = f64::from(ctx.queue.get_timestamp_period());
        let mut out = [0.0; 5];
        for (i, o) in out.iter_mut().enumerate() {
            *o = tick(2 * i + 1).saturating_sub(tick(2 * i)) as f64 * period / 1.0e6;
        }
        drop(data);
        t.readback.unmap();
        Some(out)
    }

    /// The generation map the march reads: mark the frame's dirty rectangles in it before [`ShadowResolveGpu::resolve`].
    pub fn generations_mut(&mut self) -> &mut GenerationMap {
        &mut self.generations
    }

    fn ensure(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        if self.targets.as_ref().is_some_and(|t| t.width == width && t.height == height) {
            return;
        }
        let (hw, hh) = (width.div_ceil(2), height.div_ceil(2));
        let make = |label: &str, w: u32, h: u32, format, usage| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
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
        let rw = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC;
        let shadow = || make("shadow", width, height, wgpu::TextureFormat::R32Float, rw);
        self.targets = Some(Targets {
            width,
            height,
            receivers: make("shadow-receivers", hw, hh, wgpu::TextureFormat::Rgba32Float, rw),
            world: make("shadow-receivers-world", hw, hh, wgpu::TextureFormat::Rgba32Float, rw),
            half_depth: make("shadow-half-depth", hw, hh, wgpu::TextureFormat::R32Float, rw),
            half_vis: make("shadow-half", hw, hh, wgpu::TextureFormat::R32Float, rw),
            half_steps: make("shadow-half-steps", hw, hh, wgpu::TextureFormat::R32Uint, rw),
            filled: [
                make("shadow-half-filled", hw, hh, wgpu::TextureFormat::R32Float, rw),
                make("shadow-half-filled", hw, hh, wgpu::TextureFormat::R32Float, rw),
            ],
            up: make("shadow-up", width, height, wgpu::TextureFormat::Rg32Float, rw),
            shadow: [shadow(), shadow()],
        });
        self.fresh = true;
    }

    /// Resolve one frame: receivers from `depth`, the march at half resolution over the clipmap and the cube, the upsample and the temporal
    /// pass. `march_params.dirty_since` is set here to the previous frame, so a cell marked in `frame - 1` or `frame` counts as changed.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        &mut self,
        ctx: &GpuContext,
        march: &ShadowMarchGpu,
        clip: &ClipmapGpu,
        cube: &CoarseCubeGpu,
        march_params: &MarchParams,
        depth: &DepthTarget,
        params: &ResolveParams,
    ) {
        let view = depth.texture.create_view(&wgpu::TextureViewDescriptor { aspect: wgpu::TextureAspect::DepthOnly, ..Default::default() });
        self.resolve_view(ctx, march, clip, cube, march_params, &view, (depth.width, depth.height), params);
    }

    /// [`ShadowResolveGpu::resolve`] over the lit pass's own depth view (a `Depth32Float` texture of `size`, reversed-Z).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_view(
        &mut self,
        ctx: &GpuContext,
        march: &ShadowMarchGpu,
        clip: &ClipmapGpu,
        cube: &CoarseCubeGpu,
        march_params: &MarchParams,
        depth_view: &wgpu::TextureView,
        size: (u32, u32),
        params: &ResolveParams,
    ) {
        assert!(params.frame > 0, "frames count from 1");
        for cam in [&params.camera, &params.previous] {
            assert!(
                cam.near_m > 0.0 && cam.tan_half_fov[0] > 0.0 && cam.tan_half_fov[1] > 0.0,
                "a camera needs a near plane and a field of view"
            );
        }
        self.ensure(ctx, size.0, size.1);
        let device = &ctx.device;
        let (width, height) = size;
        let (hw, hh) = (width.div_ceil(2), height.div_ceil(2));
        let cut = params.cut || self.fresh;
        let mut p = march_params.clone();
        // The march looks for changed cells only while one was marked this frame or the last: in a quiet frame it skips the lookups.
        let since = params.frame.saturating_sub(1).max(1);
        p.dirty_since = if self.generations.last_marked() >= since { since } else { 0 };
        let march_uniform = ShadowMarchGpu::uniform_bytes(clip, cube, &p);
        // The uniform buffers are made once and rewritten each frame (queue order keeps the frames in flight apart).
        let march_buffer = self.march_uniform.get_or_insert_with(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-march-uniform"),
                size: march_uniform.len() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        ctx.queue.write_buffer(march_buffer, 0, &march_uniform);

        let mut bytes: Vec<u8> = Vec::with_capacity(224);
        let f = |out: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes()));
        for cam in [&params.camera, &params.previous] {
            f(&mut bytes, &[cam.position[0], cam.position[1], cam.position[2], 0.0]);
            f(&mut bytes, &[cam.forward[0], cam.forward[1], cam.forward[2], 0.0]);
            f(&mut bytes, &[cam.right[0], cam.right[1], cam.right[2], 0.0]);
            f(&mut bytes, &[cam.up[0], cam.up[1], cam.up[2], 0.0]);
            f(&mut bytes, &[cam.tan_half_fov[0], cam.tan_half_fov[1], cam.near_m, 0.0]);
        }
        let skip = u32::from(params.spread && !cut);
        for v in [width, height, hw, hh, skip, params.frame & 1, u32::from(cut), 0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        f(&mut bytes, &[params.history_weight.clamp(0.0, MAX_HISTORY_WEIGHT), march_params.drop_coefficient, 0.0, 0.0]);
        let uniform = self.uniform.get_or_insert_with(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-resolve-uniform"),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        ctx.queue.write_buffer(uniform, 0, &bytes);

        let t = self.targets.as_ref().expect("targets");
        let view = |tex: &wgpu::Texture| tex.create_view(&Default::default());
        let (receivers_view, half_depth_view, half_vis_view, half_steps_view, up_view) =
            (view(&t.receivers), view(&t.half_depth), view(&t.half_vis), view(&t.half_steps), view(&t.up));
        let world_view = view(&t.world);
        let (filled_prev, filled_view) = (view(&t.filled[self.current]), view(&t.filled[1 - self.current]));
        let (history_view, out_view) = (view(&t.shadow[self.current]), view(&t.shadow[1 - self.current]));
        let entry = |binding, view| wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) };
        let ubind = wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() };

        let stamp = |pass: usize| self.timer.as_ref().map(|t| t.writes(pass));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shadow-resolve") });
        {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow-receivers"),
                layout: &self.receivers_bgl,
                entries: &[
                    ubind.clone(),
                    entry(1, depth_view),
                    entry(2, &receivers_view),
                    entry(3, &half_depth_view),
                    entry(4, &world_view),
                ],
            });
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shadow-receivers"), timestamp_writes: stamp(0) });
            pass.set_pipeline(&self.receivers);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(hw.div_ceil(8), hh.div_ceil(8), 1);
        }
        march.encode(
            ctx,
            &mut encoder,
            clip,
            cube,
            march_buffer,
            &self.generations.view(),
            &receivers_view,
            &half_vis_view,
            &half_steps_view,
            hw,
            hh,
            stamp(1),
        );
        {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow-fill"),
                layout: &self.fill_bgl,
                entries: &[
                    ubind.clone(),
                    entry(1, &half_vis_view),
                    entry(2, &half_steps_view),
                    entry(3, &world_view),
                    entry(4, &filled_prev),
                    entry(5, &filled_view),
                ],
            });
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shadow-fill"), timestamp_writes: stamp(2) });
            pass.set_pipeline(&self.fill);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(hw.div_ceil(8), hh.div_ceil(8), 1);
        }
        {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow-upsample"),
                layout: &self.upsample_bgl,
                entries: &[
                    ubind.clone(),
                    entry(1, depth_view),
                    entry(2, &filled_view),
                    entry(3, &half_depth_view),
                    entry(4, &half_steps_view),
                    entry(5, &up_view),
                ],
            });
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shadow-upsample"), timestamp_writes: stamp(3) });
            pass.set_pipeline(&self.upsample);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow-temporal"),
                layout: &self.temporal_bgl,
                entries: &[ubind, entry(1, depth_view), entry(2, &up_view), entry(3, &history_view), entry(4, &out_view)],
            });
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shadow-temporal"), timestamp_writes: stamp(4) });
            pass.set_pipeline(&self.temporal);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        if let Some(t) = &self.timer {
            let n = 2 * RESOLVE_PASS_NAMES.len() as u32;
            encoder.resolve_query_set(&t.queries, 0..n, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.readback, 0, u64::from(n) * 8);
        }
        ctx.queue.submit([encoder.finish()]);
        self.current = 1 - self.current;
        self.fresh = false;
    }

    /// The resolved shadow of the last frame as a sampled view (the shadow view draws it).
    pub(crate) fn shadow_texture_view(&self) -> Option<wgpu::TextureView> {
        self.targets.as_ref().map(|t| t.shadow[self.current].create_view(&Default::default()))
    }

    /// The resolved shadow of the last frame, `width * height` visibilities in [0, 1], row-major.
    pub fn read_shadow(&self, ctx: &GpuContext) -> Vec<f32> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        floats(&read_texture(ctx, &t.shadow[self.current], t.width, t.height))
    }

    /// The march's half-resolution visibility of the last frame: -1 where the checkerboard skipped the ray.
    pub fn read_half_visibility(&self, ctx: &GpuContext) -> Vec<f32> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        floats(&read_texture(ctx, &t.half_vis, t.width.div_ceil(2), t.height.div_ceil(2)))
    }

    /// Per half-resolution pixel, whether its march crossed a clipmap cell that changed this frame or the last.
    pub fn read_half_changed(&self, ctx: &GpuContext) -> Vec<bool> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        read_texture(ctx, &t.half_steps, t.width.div_ceil(2), t.height.div_ceil(2))
            .chunks(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) & crate::march::STEPS_CHANGED_BIT != 0)
            .collect()
    }

    /// The step count of each half-resolution march of the last frame (0 for a pixel without a receiver or a skipped ray).
    pub fn read_half_steps(&self, ctx: &GpuContext) -> Vec<u32> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        read_texture(ctx, &t.half_steps, t.width.div_ceil(2), t.height.div_ceil(2))
            .chunks(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) & !crate::march::STEPS_CHANGED_BIT)
            .collect()
    }

    /// The receivers' distances along the camera's forward at half resolution (0 without a receiver).
    pub fn read_half_depth(&self, ctx: &GpuContext) -> Vec<f32> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        floats(&read_texture(ctx, &t.half_depth, t.width.div_ceil(2), t.height.div_ceil(2)))
    }

    /// The receivers of the last frame at half resolution, `[x, y, z, w]` (see the receivers shader).
    pub fn read_receivers(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        let t = self.targets.as_ref().expect("a frame was resolved");
        let (w, h) = (t.width.div_ceil(2), t.height.div_ceil(2));
        let row = (w * 16).next_multiple_of(256);
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("receivers-readback"),
            size: u64::from(row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("receivers-readback") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.receivers,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        ctx.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
        ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
        let data = slice.get_mapped_range().expect("readback range").to_vec();
        buffer.unmap();
        let mut out = Vec::with_capacity((w * h) as usize);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let o = y * row as usize + x * 16;
                let v = |i: usize| f32::from_le_bytes(data[o + 4 * i..o + 4 * i + 4].try_into().unwrap());
                out.push([v(0), v(1), v(2), v(3)]);
            }
        }
        out
    }
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect()
}
