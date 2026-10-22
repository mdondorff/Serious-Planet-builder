//! SPIKE-32 (spikes/realism): the realism prototype's GPU side. NOT production code, never merged.
//!
//! One region (see `planet_generators::spike_realism`) is shaded by the lit terrain pass with: a terrain horizon map for the sun's
//! shadows, a material map (five weights and three extra channels per 2 m texel), and CC0 photo sets in texture arrays blended by the
//! weights with height-based blending, triplanar projection and de-tiled sampling. The resources and the frame's uniform reach the
//! renderer through statics because this is a spike: the app installs them before the renderer is built.
//!
//! CON-11: the shader forms the region coordinates from the fragment's camera-relative position (tile-local numbers) plus the camera's
//! position in the region frame, decided in f64 on the CPU; no planet-radius value reaches it (the inverse radius only scales the
//! gnomonic correction of a few km).

use crate::GpuContext;
use std::sync::{Arc, Mutex, OnceLock};

/// Photo sets as texture-array layers: `[layer][mip]` RGBA8 (albedo sRGB with the height in alpha; normal xy, roughness, ambient
/// occlusion).
pub struct Photo {
    pub size: u32,
    pub albedo: Vec<Vec<Vec<u8>>>,
    pub normal: Vec<Vec<Vec<u8>>>,
}

pub struct Resources {
    pub mat_n: u32,
    pub mat0: Vec<u8>,
    pub mat1: Vec<u8>,
    pub hor_n: u32,
    pub hor: [Vec<u8>; 4],
    pub photo: Option<Photo>,
    pub extent_m: f64,
}

static RESOURCES: OnceLock<Arc<Resources>> = OnceLock::new();

pub fn install(res: Resources) {
    let _ = RESOURCES.set(Arc::new(res));
}

/// The frame's uniform block (96 bytes; layout of `Realism` in the shader).
#[derive(Clone, Copy, Default)]
pub struct Frame {
    pub cam: [f32; 4],
    pub east: [f32; 4],
    pub north: [f32; 4],
    pub up: [f32; 4],
    pub flags: [u32; 4],
    pub tune: [f32; 4],
}

static FRAME: Mutex<Frame> = Mutex::new(Frame {
    cam: [0.0; 4],
    east: [0.0; 4],
    north: [0.0; 4],
    up: [0.0; 4],
    flags: [0; 4],
    tune: [0.0; 4],
});

pub fn set_frame(f: Frame) {
    *FRAME.lock().expect("frame lock") = f;
}

fn frame_bytes() -> Vec<u8> {
    let f = *FRAME.lock().expect("frame lock");
    let mut b = Vec::with_capacity(96);
    for v in [f.cam, f.east, f.north, f.up, f.tune] {
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
    }
    // flags sit fifth in the shader block: rebuild in order cam, east, north, up, flags, tune.
    let mut out = Vec::with_capacity(96);
    for v in [f.cam, f.east, f.north, f.up] {
        for x in v {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
    for x in f.flags {
        out.extend_from_slice(&x.to_le_bytes());
    }
    for x in f.tune {
        out.extend_from_slice(&x.to_le_bytes());
    }
    drop(b);
    out
}

pub const FIRST_BINDING: u32 = 4;
pub const UNIFORM_BYTES: u64 = 96;

pub struct Gpu {
    pub uniform: wgpu::Buffer,
    pub mat: wgpu::TextureView,
    pub alb: wgpu::TextureView,
    pub nrm: wgpu::TextureView,
    pub hor: wgpu::TextureView,
    pub rep: wgpu::Sampler,
    pub clamp: wgpu::Sampler,
    /// Bytes of the resident textures (for the cost table).
    pub texture_bytes: u64,
}

fn mip_chain_rgba(base: &[u8], w: usize, h: usize) -> Vec<(usize, usize, Vec<u8>)> {
    let mut out = vec![(w, h, base.to_vec())];
    loop {
        let (pw, ph, prev) = out.last().expect("one level").clone();
        if pw == 1 && ph == 1 {
            break;
        }
        let (nw, nh) = ((pw / 2).max(1), (ph / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let (x0, y0, x1, y1) = (x * 2, y * 2, (x * 2 + 1).min(pw - 1), (y * 2 + 1).min(ph - 1));
                    let s = u32::from(prev[(y0 * pw + x0) * 4 + c])
                        + u32::from(prev[(y0 * pw + x1) * 4 + c])
                        + u32::from(prev[(y1 * pw + x0) * 4 + c])
                        + u32::from(prev[(y1 * pw + x1) * 4 + c]);
                    next[(y * nw + x) * 4 + c] = ((s + 2) / 4) as u8;
                }
            }
        }
        out.push((nw, nh, next));
    }
    out
}

fn write_layer(ctx: &GpuContext, tex: &wgpu::Texture, mip: u32, layer: u32, w: u32, h: u32, data: &[u8]) {
    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: tex, mip_level: mip, origin: wgpu::Origin3d { x: 0, y: 0, z: layer }, aspect: wgpu::TextureAspect::All },
        data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

fn array_texture(ctx: &GpuContext, label: &str, w: u32, h: u32, layers: u32, mips: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: layers },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn view_of(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
}

impl Gpu {
    pub fn new(ctx: &GpuContext) -> Gpu {
        let device = &ctx.device;
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("realism-uniform"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let res = RESOURCES.get().cloned();
        let mut bytes = 0u64;
        let (mat, alb, nrm, hor);
        let rgba = wgpu::TextureFormat::Rgba8Unorm;
        match &res {
            Some(r) => {
                let n = r.mat_n as usize;
                let m0 = mip_chain_rgba(&r.mat0, n, n);
                let m1 = mip_chain_rgba(&r.mat1, n, n);
                let t = array_texture(ctx, "realism-material", r.mat_n, r.mat_n, 2, m0.len() as u32, rgba);
                for (k, (w, h, d)) in m0.iter().enumerate() {
                    write_layer(ctx, &t, k as u32, 0, *w as u32, *h as u32, d);
                    bytes += d.len() as u64;
                }
                for (k, (w, h, d)) in m1.iter().enumerate() {
                    write_layer(ctx, &t, k as u32, 1, *w as u32, *h as u32, d);
                    bytes += d.len() as u64;
                }
                mat = view_of(&t);
                let t = array_texture(ctx, "realism-horizon", r.hor_n, r.hor_n, 4, 1, rgba);
                for (l, d) in r.hor.iter().enumerate() {
                    write_layer(ctx, &t, 0, l as u32, r.hor_n, r.hor_n, d);
                    bytes += d.len() as u64;
                }
                hor = view_of(&t);
                match &r.photo {
                    Some(p) => {
                        let mips = p.albedo[0].len() as u32;
                        let layers = p.albedo.len() as u32;
                        let ta = array_texture(ctx, "realism-albedo", p.size, p.size, layers, mips, wgpu::TextureFormat::Rgba8UnormSrgb);
                        let tn = array_texture(ctx, "realism-normal", p.size, p.size, layers, mips, rgba);
                        for l in 0..layers as usize {
                            for k in 0..mips as usize {
                                let s = (p.size >> k).max(1);
                                write_layer(ctx, &ta, k as u32, l as u32, s, s, &p.albedo[l][k]);
                                write_layer(ctx, &tn, k as u32, l as u32, s, s, &p.normal[l][k]);
                                bytes += (p.albedo[l][k].len() + p.normal[l][k].len()) as u64;
                            }
                        }
                        alb = view_of(&ta);
                        nrm = view_of(&tn);
                    }
                    None => {
                        let (a, n2) = Self::dummy(ctx, 1);
                        alb = a;
                        nrm = n2;
                    }
                }
            }
            None => {
                let (a, n2) = Self::dummy(ctx, 1);
                mat = Self::dummy(ctx, 2).0;
                hor = Self::dummy(ctx, 4).0;
                alb = a;
                nrm = n2;
            }
        }
        let rep = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("realism-repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("realism-clamp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Gpu { uniform, mat, alb, nrm, hor, rep, clamp, texture_bytes: bytes }
    }

    fn dummy(ctx: &GpuContext, layers: u32) -> (wgpu::TextureView, wgpu::TextureView) {
        let make = |label| {
            let t = array_texture(ctx, label, 1, 1, layers, 1, wgpu::TextureFormat::Rgba8Unorm);
            for l in 0..layers {
                write_layer(ctx, &t, 0, l, 1, 1, &[128, 128, 128, 128]);
            }
            view_of(&t)
        };
        (make("realism-dummy-a"), make("realism-dummy-b"))
    }

    /// Write the frame's uniform (the queue orders it before the submission that draws).
    pub fn write_frame(&self, ctx: &GpuContext) {
        ctx.queue.write_buffer(&self.uniform, 0, &frame_bytes());
    }

    pub fn layout_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        };
        let samp = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        vec![
            wgpu::BindGroupLayoutEntry {
                binding: FIRST_BINDING,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            tex(5),
            tex(6),
            tex(7),
            tex(8),
            samp(9),
            samp(10),
        ]
    }

    pub fn bind_entries(&self) -> Vec<wgpu::BindGroupEntry<'_>> {
        vec![
            wgpu::BindGroupEntry { binding: FIRST_BINDING, resource: self.uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.mat) },
            wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.alb) },
            wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.nrm) },
            wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&self.hor) },
            wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&self.rep) },
            wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::Sampler(&self.clamp) },
        ]
    }
}

/// The declarations, added to the compact terrain shader (bindings of group 0).
pub const DECLS: &str = r#"
// SPIKE-32 realism block.
struct Realism {
    // xyz: the camera in the region frame (x east, y north, z up above the reference surface, metres); w: the inverse planet radius.
    cam: vec4<f32>,
    // xyz: the planet-fixed unit east of the region; w: half the region's side (m).
    east: vec4<f32>,
    // xyz: the unit north; w: the width of the skirt (m).
    north: vec4<f32>,
    // xyz: the unit up at the region's centre; w unused.
    upv: vec4<f32>,
    // x: bit 1 shadows, 2 material map, 4 photo textures, 8 debug view in y; y: debug view (1 weights, 2 shadow, 3 normals).
    flags: vec4<u32>,
    // x: metres per repeat of the photo sets (scale of the base layer), y: the height blend depth, z: macro variation, w unused.
    tune: vec4<f32>,
};
@group(0) @binding(4) var<uniform> RZ: Realism;
@group(0) @binding(5) var rz_mat: texture_2d_array<f32>;
@group(0) @binding(6) var rz_alb: texture_2d_array<f32>;
@group(0) @binding(7) var rz_nrm: texture_2d_array<f32>;
@group(0) @binding(8) var rz_hor: texture_2d_array<f32>;
@group(0) @binding(9) var rz_rep: sampler;
@group(0) @binding(10) var rz_clamp: sampler;
"#;

/// The functions, appended to the atmosphere's terrain shader after its lighting (they use `atm_*` and the node records).
pub const MAIN: &str = r#"
// ---- SPIKE-32 realism ----
fn rz_hash(p: vec2<i32>, s: u32) -> f32 {
    var h = (bitcast<u32>(p.x) * 0x8da6b343u) ^ (bitcast<u32>(p.y) * 0xd8163841u) ^ (s * 0xcb1ab31fu);
    h = h ^ (h >> 15u);
    h = h * 0x2c1b3c6du;
    h = h ^ (h >> 12u);
    h = h * 0x297a2d39u;
    h = h ^ (h >> 15u);
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

fn rz_noise(p: vec2<f32>, s: u32) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let c = vec2<i32>(i);
    let a = rz_hash(c, s);
    let b = rz_hash(c + vec2<i32>(1, 0), s);
    let cc = rz_hash(c + vec2<i32>(0, 1), s);
    let d = rz_hash(c + vec2<i32>(1, 1), s);
    return mix(mix(a, b, u.x), mix(cc, d, u.x), u.y);
}

// Two-sample de-tiling (random offset per cell of a low-frequency noise, blended where the cells meet).
fn rz_tex(arr: texture_2d_array<f32>, uv: vec2<f32>, layer: i32, dx: vec2<f32>, dy: vec2<f32>) -> vec4<f32> {
    let k = rz_noise(uv * 1.3, u32(layer) + 40u);
    let l = k * 8.0;
    let f = fract(l);
    let ia = floor(l + 0.5);
    let ib = floor(l);
    let f2 = min(f, 1.0 - f) * 2.0;
    let offa = sin(vec2<f32>(3.0, 7.0) * ia);
    let offb = sin(vec2<f32>(3.0, 7.0) * ib);
    let a = textureSampleGrad(arr, rz_rep, uv + offa, layer, dx, dy);
    let b = textureSampleGrad(arr, rz_rep, uv + offb, layer, dx, dy);
    return mix(a, b, smoothstep(0.2, 0.8, f2 - 0.1 * ((a.x - b.x) + (a.y - b.y) + (a.z - b.z))));
}

struct RzPlane {
    alb: vec3<f32>,
    h: f32,
    n: vec2<f32>,
    rough: f32,
};

// One projection plane of one material: the set at its own scale and, for the rocky sets, again at 8 times the scale so the faces
// carry structure over 3 to 25 m, not only at the set's grain.
fn rz_plane(c: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, scale: f32, layer: i32, big: bool) -> RzPlane {
    var o: RzPlane;
    let uv = c / scale;
    let a = rz_tex(rz_alb, uv, layer, dx / scale, dy / scale);
    let n = rz_tex(rz_nrm, uv, layer, dx / scale, dy / scale);
    var alb = a.rgb;
    var t = n.xy * 2.0 - 1.0;
    var h = a.a;
    if (big) {
        let s2 = scale * 8.0;
        let uv2 = c / s2;
        let a2 = textureSampleGrad(rz_alb, rz_rep, uv2 + vec2<f32>(0.37, 0.11), layer, dx / s2, dy / s2);
        let n2 = textureSampleGrad(rz_nrm, rz_rep, uv2 + vec2<f32>(0.37, 0.11), layer, dx / s2, dy / s2);
        let lum = dot(a2.rgb, vec3<f32>(0.3, 0.55, 0.15));
        let mean = 0.14;
        alb = alb * clamp(0.5 + 0.5 * lum / mean, 0.5, 1.8) * select(1.0, 1.5, layer == 0);
        t = t + (n2.xy * 2.0 - 1.0) * 1.2;
        h = mix(h, a2.a, 0.5);
    }
    o.alb = alb;
    o.h = h;
    o.n = t;
    o.rough = n.b;
    return o;
}

struct RzMat {
    alb: vec3<f32>,
    h: f32,
    n: vec3<f32>,
    rough: f32,
};

// One material: the photo set sampled triplanar (the planes weighted by `bw`, in the region frame), or a flat tint with procedural variation.
fn rz_material(layer: i32, scale: f32, p: vec3<f32>, bw: vec3<f32>, dpx: vec3<f32>, dpy: vec3<f32>, photo: bool, tint: vec3<f32>, foot: f32) -> RzMat {
    var out: RzMat;
    let v1 = rz_noise(p.xy * (1.0 / 1.7) + p.z * 0.31, 60u + u32(layer));
    let v2 = rz_noise(p.xy * (1.0 / 0.45) + p.z * 0.7, 70u + u32(layer));
    let flat_h = 0.5 * v1 + 0.5 * v2;
    out.alb = tint * (0.78 + 0.44 * (0.6 * v1 + 0.4 * v2));
    out.h = flat_h;
    out.n = vec3<f32>(0.0);
    out.rough = 0.8;
    if (!photo) {
        return out;
    }
    // The detail fades to the set's mean colour (the last mip) while a pixel covers more than 2 to 6 m.
    let det = 1.0 - smoothstep(2.0, 6.0, foot);
    let mean_a = textureSampleLevel(rz_alb, rz_rep, vec2<f32>(0.5), layer, 11.0);
    let mean_n = textureSampleLevel(rz_nrm, rz_rep, vec2<f32>(0.5), layer, 11.0);
    out.alb = mean_a.rgb;
    out.h = mean_a.a;
    out.rough = mean_n.b;
    if (det <= 0.0) {
        return out;
    }
    var acc_a = vec3<f32>(0.0);
    var acc_h = 0.0;
    var acc_n = vec3<f32>(0.0);
    var acc_r = 0.0;
    var wsum = 0.0;
    let big = layer == 0 || layer == 1 || layer == 4;
    if (bw.z > 0.04) {
        let q = rz_plane(p.xy, dpx.xy, dpy.xy, scale, layer, big);
        acc_a = acc_a + q.alb * bw.z;
        acc_h = acc_h + q.h * bw.z;
        acc_n = acc_n + vec3<f32>(q.n.x, q.n.y, 0.0) * bw.z;
        acc_r = acc_r + q.rough * bw.z;
        wsum = wsum + bw.z;
    }
    if (bw.y > 0.04) {
        let q = rz_plane(p.xz, dpx.xz, dpy.xz, scale, layer, big);
        acc_a = acc_a + q.alb * bw.y;
        acc_h = acc_h + q.h * bw.y;
        acc_n = acc_n + vec3<f32>(q.n.x, 0.0, q.n.y) * bw.y;
        acc_r = acc_r + q.rough * bw.y;
        wsum = wsum + bw.y;
    }
    if (bw.x > 0.04) {
        let q = rz_plane(p.yz, dpx.yz, dpy.yz, scale, layer, big);
        acc_a = acc_a + q.alb * bw.x;
        acc_h = acc_h + q.h * bw.x;
        acc_n = acc_n + vec3<f32>(0.0, q.n.x, q.n.y) * bw.x;
        acc_r = acc_r + q.rough * bw.x;
        wsum = wsum + bw.x;
    }
    let inv = 1.0 / max(wsum, 1e-3);
    out.alb = mix(out.alb, acc_a * inv, det);
    out.h = mix(out.h, acc_h * inv, det);
    out.n = acc_n * inv * det;
    out.rough = mix(out.rough, acc_r * inv, det);
    return out;
}

fn rz_vis(uv: vec2<f32>, s: vec3<f32>) -> f32 {
    let elev = degrees(asin(clamp(s.z, -1.0, 1.0)));
    var az = atan2(s.y, s.x);
    if (az < 0.0) {
        az = az + 6.2831853;
    }
    let f = az * (16.0 / 6.2831853);
    let j0 = u32(floor(f)) % 16u;
    let j1 = (j0 + 1u) % 16u;
    let t = fract(f);
    let h0 = textureSampleLevel(rz_hor, rz_clamp, uv, i32(j0 >> 2u), 0.0)[j0 & 3u];
    let h1 = textureSampleLevel(rz_hor, rz_clamp, uv, i32(j1 >> 2u), 0.0)[j1 & 3u];
    let hor = mix(h0, h1, t) * 80.0 - 10.0;
    return smoothstep(hor - 0.35, hor + 0.9, elev);
}

struct RzSurface {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    rough: f32,
    vis: f32,
    w: f32,
    has_mat: bool,
    debug: vec3<f32>,
};

const RZ_TINTS = array<vec3<f32>, 5>(
    vec3<f32>(0.20, 0.19, 0.18),
    vec3<f32>(0.30, 0.27, 0.23),
    vec3<f32>(0.80, 0.83, 0.88),
    vec3<f32>(0.10, 0.17, 0.05),
    vec3<f32>(0.16, 0.11, 0.07),
);
const RZ_SCALES = array<f32, 5>(3.0, 1.2, 2.5, 1.0, 1.6);

fn rz_surface(in: VsOut, sun: vec3<f32>) -> RzSurface {
    var out: RzSurface;
    out.albedo = vec3<f32>(0.0);
    out.normal = normalize(in.normal);
    out.rough = 0.8;
    out.vis = 1.0;
    out.w = 0.0;
    out.has_mat = false;
    out.debug = vec3<f32>(0.0);
    let flags = RZ.flags.x;
    let rel = nodes[in.node_index].origin_rel.xyz + in.local;
    let pc = RZ.cam.xyz + vec3<f32>(dot(rel, RZ.east.xyz), dot(rel, RZ.north.xyz), dot(rel, RZ.upv.xyz));
    let k = 1.0 / (1.0 + pc.z * RZ.cam.w);
    let p = vec3<f32>(pc.x * k, pc.y * k, pc.z);
    // The derivatives come first, in uniform control flow.
    let dpx = dpdx(p);
    let dpy = dpdy(p);
    if ((flags & 7u) == 0u) {
        return out;
    }
    let half = RZ.east.w;
    let edge = max(abs(p.x), abs(p.y)) - half;
    out.w = 1.0 - smoothstep(0.0, RZ.north.w, edge);
    if (out.w <= 0.0) {
        return out;
    }
    let uv = (p.xy + vec2<f32>(half)) / (2.0 * half);
    let duvx = dpx.xy / (2.0 * half);
    let duvy = dpy.xy / (2.0 * half);
    // The normal and the sun in the region frame.
    let nw = normalize(in.normal);
    let nr = vec3<f32>(dot(nw, RZ.east.xyz), dot(nw, RZ.north.xyz), dot(nw, RZ.upv.xyz));
    let sr = vec3<f32>(dot(sun, RZ.east.xyz), dot(sun, RZ.north.xyz), dot(sun, RZ.upv.xyz));
    if ((flags & 1u) != 0u) {
        out.vis = rz_vis(uv, sr);
    }
    if ((flags & 6u) == 0u) {
        return out;
    }
    out.has_mat = true;
    let m0 = textureSampleGrad(rz_mat, rz_clamp, uv, 0, duvx, duvy);
    let m1 = textureSampleGrad(rz_mat, rz_clamp, uv, 1, duvx, duvy);
    var ws = array<f32, 5>(m0.x, m0.y, m0.z, m0.w, m1.x);
    let depth = m1.w;
    let photo = (flags & 4u) != 0u;
    let foot = 0.5 * (length(dpx) + length(dpy));
    var bw = pow(abs(nr), vec3<f32>(5.0));
    bw = bw / (bw.x + bw.y + bw.z);
    var alb = array<vec3<f32>, 5>(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    var nrm = array<vec3<f32>, 5>(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    var hgt = array<f32, 5>(-10.0, -10.0, -10.0, -10.0, -10.0);
    var rgh = array<f32, 5>(0.8, 0.8, 0.8, 0.8, 0.8);
    var ma = -10.0;
    for (var i = 0; i < 5; i = i + 1) {
        if (ws[i] > 0.012) {
            let m = rz_material(i, RZ_SCALES[i] * RZ.tune.x, p, bw, dpx, dpy, photo, RZ_TINTS[i], foot);
            alb[i] = m.alb;
            nrm[i] = m.n;
            rgh[i] = m.rough;
            var h = m.h * 0.7;
            if (i == 2) {
                h = h + (depth - 0.45) * 0.5;
            }
            hgt[i] = h;
            ma = max(ma, ws[i] + h);
        }
    }
    var sum = 0.0;
    var acc_a = vec3<f32>(0.0);
    var acc_n = vec3<f32>(0.0);
    var acc_r = 0.0;
    for (var i = 0; i < 5; i = i + 1) {
        if (ws[i] > 0.012) {
            let b = max(ws[i] + hgt[i] - (ma - RZ.tune.y), 0.0);
            sum = sum + b;
            acc_a = acc_a + alb[i] * b;
            acc_n = acc_n + nrm[i] * b;
            acc_r = acc_r + rgh[i] * b;
        }
    }
    let inv = 1.0 / max(sum, 1e-4);
    var a = acc_a * inv;
    // Macro variation breaks the repetition of the sets over 10 to 100 m.
    let mv = (0.86 + 0.28 * rz_noise(p.xy * (1.0 / 47.0), 11u)) * (0.92 + 0.16 * rz_noise(p.xy * (1.0 / 9.0), 12u));
    a = a * mix(1.0, mv, RZ.tune.z);
    out.albedo = a;
    out.rough = acc_r * inv;
    let strength = select(0.0, 0.9, photo);
    let pn = normalize(nr + acc_n * inv * strength);
    out.normal = normalize(RZ.east.xyz * pn.x + RZ.north.xyz * pn.y + RZ.upv.xyz * pn.z);
    if (RZ.flags.y == 1u) {
        out.debug = vec3<f32>(ws[0] + ws[1] * 0.5, ws[3] + ws[4] * 0.4, ws[2]);
    } else if (RZ.flags.y == 2u) {
        out.debug = vec3<f32>(out.vis);
    } else if (RZ.flags.y == 3u) {
        out.debug = pn * 0.5 + vec3<f32>(0.5);
    }
    return out;
}

// The sun's light with the shadow of the terrain's horizon on the direct term only (the sky's ambient is not shadowed).
fn atm_terrain_lighting_vis(normal: vec3<f32>, up: vec3<f32>, h_alt: f32, sun: vec3<f32>, vis: f32) -> vec3<f32> {
    let n = normalize(normal);
    let ndl = max(dot(n, sun), 0.0);
    let mu_s = clamp(dot(up, sun), -1.0, 1.0);
    let h = clamp(h_alt, 0.0, A.body.y);
    let t = atm_transmittance(h, mu_s);
    let shadow = atm_planet_shadow(h, mu_s);
    let ms = atm_multiscatter(h, mu_s);
    let hemi = 0.5 + 0.5 * clamp(dot(n, up), -1.0, 1.0);
    return t * (shadow * ndl * vis * 0.31830988618) + ms * hemi;
}

// The main path's radiance with the horizon shadow (vis = 1 reproduces `atm_terrain_radiance`).
fn atm_terrain_radiance_v(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, ramp_min: f32, h_alt: f32, sun: vec3<f32>, vis: f32) -> vec3<f32> {
    let n = normalize(normal);
    let cs = clamp(dot(n, up), 0.0, 1.0);
    if (view >= 3u) {
        return climate_colour(view, t, p) * 0.31830988618;
    }
    let light = atm_terrain_lighting_vis(normal, up, h_alt, sun, vis);
    if (view == 2u) {
        return light;
    }
    let a = albedo_of(tab, t, p, cs, h, ramp_min);
    if (view == 1u) {
        return a * 0.31830988618;
    }
    return a * light;
}
"#;
