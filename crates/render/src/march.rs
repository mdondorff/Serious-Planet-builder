//! The shadow march (REND-049, ADR 0085 points 1, 3 and 5): a compute pass that gives every receiver a sun visibility in [0, 1] by
//! marching from it toward the sun over the height clipmap (a texel walk with max-mip skipping) and, beyond the outermost level, the
//! coarse cube. Positions are in the anchor's flat frame (x east, y north, z up, heights with the sphere's drop taken off), so the shader
//! holds no planet-radius value: the curvature arrives as a uniform (CON-11). The pass uses no atomics (REND-053).
//!
//! This is the pass as a probe over a list of receivers (their anchor-frame positions in a texture); the frame path reconstructs the
//! receivers from the depth buffer and applies the result in a later change. Nothing here draws.

use crate::resolve::GenerationMap;
use crate::shadow::{ClipmapGpu, CoarseCubeGpu};
use crate::GpuContext;

/// The most levels the pass reads.
pub const MARCH_MAX_LEVELS: usize = 16;

const MARCH_SHADER: &str = r#"
struct March {
    // xyz: the unit direction to the sun in the anchor frame; w: the tangent of the sun disc's angular size.
    sun: vec4<f32>,
    // x: the drop coefficient (the sphere falls c rho^2 below the tangent plane at a distance rho; half the curvature, a uniform); y: the longest march; z: the penumbra search distance; w: 1 when the
    // coarse cube continues the march beyond the outermost level.
    curv: vec4<f32>,
    // The anchor and its east and north in planet-fixed coordinates (the cube lookup).
    anchor: vec4<f32>,
    east: vec4<f32>,
    north: vec4<f32>,
    // The edge of a level in texels, the level count, the edge of the cube in texels, the highest mip the walk uses.
    info: vec4<u32>,
    // x: a cube texel in metres at the middle of a face.
    cube: vec4<f32>,
    // x: the generation value from which a clipmap cell counts as changed (0: never); y: the shift from texels to generation cells.
    gen: vec4<u32>,
    windows: array<vec4<i32>, 16>,
    texel: array<vec4<f32>, 16>,
};
@group(0) @binding(0) var<uniform> m: March;
@group(0) @binding(1) var clipmap: texture_2d_array<f32>;
@group(0) @binding(2) var cube: texture_2d_array<f32>;
// x, y, z in the anchor frame; w is 1 for a receiver and 0 for a pixel without one.
@group(0) @binding(3) var receivers: texture_2d<f32>;
@group(0) @binding(4) var out_visibility: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var out_steps: texture_storage_2d<r32uint, write>;
// Per level, the frame at which each generation cell of the clipmap last changed (REND-050).
@group(0) @binding(6) var gens: texture_2d_array<u32>;

const MAX_STEPS: i32 = 2048;
const MAX_CUBE_STEPS: i32 = 256;
const QUARTER_TURN_ANGLE: f32 = 0.7853982;

fn slot(c: i32, n: i32) -> i32 {
    return ((c % n) + n) % n;
}

// The finest level whose window holds the point with a margin of two texels, or -1.
fn level_at(x: f32, y: f32) -> i32 {
    for (var k = 0; k < i32(m.info.y); k = k + 1) {
        let t = m.texel[k].x;
        let w = m.windows[k];
        if (x >= (f32(w.x) + 2.0) * t && x < (f32(w.z) - 2.0) * t && y >= (f32(w.y) + 2.0) * t && y < (f32(w.w) - 2.0) * t) {
            return k;
        }
    }
    return -1;
}

// Whether the cell (cx, cy) of the walk's current mip at level k lies in a generation cell that changed (the temporal pass rejects its
// history there). The generation cells are 8 texels at mip 0 and double with the walk's mip, so a walk cell lies in exactly one of them.
fn changed_at(k: i32, mip: i32, cx: i32, cy: i32) -> bool {
    if (m.gen.x == 0u) {
        return false;
    }
    let gm = max(mip - i32(m.gen.y), 0);
    let sh = u32(i32(m.gen.y) + gm - mip);
    let n = (i32(m.info.x) >> m.gen.y) >> u32(gm);
    return textureLoad(gens, vec2<i32>(slot(cx >> sh, n), slot(cy >> sh, n)), k, gm).x >= m.gen.x;
}

fn level_max(k: i32, mip: i32, cx: i32, cy: i32) -> f32 {
    let n = i32(m.info.x) >> u32(mip);
    return textureLoad(clipmap, vec2<i32>(slot(cx, n), slot(cy, n)), k, mip).x;
}

// The cube texel of a unit direction at a mip: (height, 1 when the texel is within one texel of a face edge at that mip).
fn cube_probe(dir: vec3<f32>, mip: i32) -> vec2<f32> {
    let a = abs(dir);
    var face = 0;
    var n = vec3<f32>(1.0, 0.0, 0.0);
    var u = vec3<f32>(0.0, 1.0, 0.0);
    var v = vec3<f32>(0.0, 0.0, 1.0);
    if (a.x >= a.y && a.x >= a.z) {
        if (dir.x < 0.0) { face = 1; n = vec3<f32>(-1.0, 0.0, 0.0); u = vec3<f32>(0.0, -1.0, 0.0); }
    } else if (a.y >= a.z) {
        if (dir.y >= 0.0) {
            face = 2; n = vec3<f32>(0.0, 1.0, 0.0); u = vec3<f32>(-1.0, 0.0, 0.0);
        } else {
            face = 3; n = vec3<f32>(0.0, -1.0, 0.0); u = vec3<f32>(1.0, 0.0, 0.0);
        }
    } else if (dir.z >= 0.0) {
        face = 4; n = vec3<f32>(0.0, 0.0, 1.0); u = vec3<f32>(1.0, 0.0, 0.0); v = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        face = 5; n = vec3<f32>(0.0, 0.0, -1.0); u = vec3<f32>(1.0, 0.0, 0.0); v = vec3<f32>(0.0, -1.0, 0.0);
    }
    let dn = dot(dir, n);
    let s = atan(dot(dir, u) / dn) / QUARTER_TURN_ANGLE;
    let t = atan(dot(dir, v) / dn) / QUARTER_TURN_ANGLE;
    let e = i32(m.info.z) >> u32(mip);
    let cx = clamp(i32(floor((s * 0.5 + 0.5) * f32(e))), 0, e - 1);
    let cy = clamp(i32(floor((t * 0.5 + 0.5) * f32(e))), 0, e - 1);
    var edge = 0.0;
    if (cx <= 1 || cy <= 1 || cx >= e - 2 || cy >= e - 2) {
        edge = 1.0;
    }
    return vec2<f32>(textureLoad(cube, vec2<i32>(cx, cy), face, mip).x, edge);
}

// The direction of the planet-fixed unit vector at the plane point q (the drop coefficient c is half the curvature).
fn cube_direction(q: vec2<f32>) -> vec3<f32> {
    let rho = length(q);
    var dir = m.anchor.xyz;
    if (rho > 1.0e-3) {
        let theta = rho * 2.0 * m.curv.x;
        dir = normalize(m.anchor.xyz * cos(theta) + (m.east.xyz * q.x + m.north.xyz * q.y) * (sin(theta) / rho));
    }
    return dir;
}

// Visibility from the smallest clearance over distance, across the sun disc.
fn visibility_of(pen: f32) -> f32 {
    let w = m.sun.w;
    return clamp((pen + w) / (2.0 * w), 0.0, 1.0);
}

// (visibility, steps) of one receiver.
fn march(o: vec3<f32>) -> vec3<f32> {
    let s = m.sun.xyz;
    // The drop of the sphere at a distance rho from the anchor is c rho^2 (c is a uniform, half the curvature).
    let c = m.curv.x;
    // The planet's own shadow: the sun below the receiver's geometric horizon.
    let up = normalize(vec3<f32>(o.x * 2.0 * c, o.y * 2.0 * c, 1.0));
    let height = max(o.z + c * (o.x * o.x + o.y * o.y), 0.0);
    if (dot(s, up) < -sqrt(4.0 * height * c)) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let hl = length(s.xy);
    if (hl < 1.0e-5) {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    let d = s.xy / hl;
    let slope = s.z / hl;
    var tau = 0.0;
    var pen = 1.0e5;
    var steps = 0.0;
    var blocked = false;
    var dirty = false;
    var mip = min(2, i32(m.info.w));
    var left = false;
    for (var it = 0; it < MAX_STEPS; it = it + 1) {
        if (tau > m.curv.y) {
            break;
        }
        let q = o.xy + d * tau;
        let lv = level_at(q.x, q.y);
        if (lv < 0) {
            left = true;
            break;
        }
        let tl = m.texel[lv].x;
        let cs = tl * f32(1 << u32(mip));
        let cx = i32(floor(q.x / cs));
        let cy = i32(floor(q.y / cs));
        var tx = 9.0e5;
        if (d.x > 1.0e-9) {
            tx = (f32(cx + 1) * cs - q.x) / d.x;
        } else if (d.x < -1.0e-9) {
            tx = (f32(cx) * cs - q.x) / d.x;
        }
        var ty = 9.0e5;
        if (d.y > 1.0e-9) {
            ty = (f32(cy + 1) * cs - q.y) / d.y;
        } else if (d.y < -1.0e-9) {
            ty = (f32(cy) * cs - q.y) / d.y;
        }
        let dt = max(min(tx, ty), 1.0e-3 * tl) + 1.0e-3 * tl;
        // A coarse cell that reaches past the level's window is looked at in its parts, so no part is judged by the far side of the torus.
        let exit = o.xy + d * (tau + dt);
        if (mip > 0 && level_at(exit.x, exit.y) != lv) {
            mip = mip - 1;
            continue;
        }
        if (!dirty && changed_at(lv, mip, cx, cy)) {
            dirty = true;
        }
        let low = min(o.z + slope * tau, o.z + slope * (tau + dt));
        // The ground is a box a texel wide at its sample; a receiver on it or on a slope would shadow itself, so the box is lowered by half a
        // texel over the first four texels.
        let relax = 0.5 * tl * clamp(1.0 - tau / (4.0 * tl), 0.0, 1.0);
        let top = level_max(lv, mip, cx, cy) - relax;
        steps = steps + 1.0;
        let dist = max(tau + 0.5 * dt, 1.0);
        // Within the penumbra search a cell must be cleared by what the sun disc spans; beyond it only the sign matters.
        var need = 0.0;
        if (tau < m.curv.z) {
            need = m.sun.w * dist;
        }
        if (low - top >= need) {
            tau = tau + dt;
            mip = min(mip + 1, i32(m.info.w));
        } else if (mip > 0) {
            // A near miss or a hit against a maximum: look at the cells inside, down to the texels, where the clearance is exact.
            mip = mip - 1;
        } else {
            let clearance = (low - top) / dist;
            if (tau < m.curv.z) {
                pen = min(pen, clearance);
            } else if (clearance < 0.0) {
                pen = -1.0;
            }
            if (pen <= -m.sun.w) {
                blocked = true;
                break;
            }
            tau = tau + dt;
        }
    }
    // Beyond the outermost level the coarse cube continues the march, in steps that grow with the distance. Each sample is a mip whose
    // texel (dilated by one) spans the step; within one texel of a face edge, where the mips do not dilate across the edge, the stride is
    // sampled at mip 0 in quarters.
    if (!blocked && left && m.curv.w > 0.5) {
        let top_level = i32(m.info.y) - 1;
        let floor_stride = m.texel[top_level].x;
        let cube_mips = i32(firstTrailingBit(m.info.z));
        let texel_min = max(m.cube.x * 0.70710678, 1.0e-3);
        for (var it = 0; it < MAX_CUBE_STEPS; it = it + 1) {
            if (tau > m.curv.y) {
                break;
            }
            let stride = max(0.03 * tau, floor_stride);
            let mip_c = clamp(i32(ceil(log2(max(stride / texel_min, 1.0)))) + 1, 0, cube_mips);
            let q = o.xy + d * tau;
            let probe = cube_probe(cube_direction(q), mip_c);
            let above = o.z + slope * tau + c * dot(q, q);
            steps = steps + 1.0;
            if (above < probe.x) {
                blocked = true;
                break;
            }
            if (probe.y > 0.5) {
                for (var j = 1; j < 4; j = j + 1) {
                    let tj = tau + stride * f32(j) * 0.25;
                    let qj = o.xy + d * tj;
                    let fine = cube_probe(cube_direction(qj), 0);
                    steps = steps + 1.0;
                    if (o.z + slope * tj + c * dot(qj, qj) < fine.x) {
                        blocked = true;
                    }
                }
                if (blocked) {
                    break;
                }
            }
            tau = tau + stride;
        }
    }
    var flag = 0.0;
    if (dirty) {
        flag = 1.0;
    }
    if (blocked) {
        return vec3<f32>(0.0, steps, flag);
    }
    return vec3<f32>(visibility_of(pen), steps, flag);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(receivers);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }
    let p = vec2<i32>(gid.xy);
    let r = textureLoad(receivers, p, 0);
    var result = vec3<f32>(1.0, 0.0, 0.0);
    if (r.w > 0.5) {
        result = march(r.xyz);
    } else if (r.w < -0.5) {
        // Not marched this frame (the checkerboard of a low sun): the resolve takes it from history.
        result = vec3<f32>(-1.0, 0.0, 0.0);
    }
    // Bit 31 of the step count flags a march that crossed a changed clipmap cell.
    textureStore(out_visibility, p, vec4<f32>(result.x, 0.0, 0.0, 0.0));
    textureStore(out_steps, p, vec4<u32>(u32(result.y) | (u32(result.z) << 31u), 0u, 0u, 0u));
}
"#;

/// The shader (a function so the shader scan and the validation test can reach it).
pub fn march_shader() -> String {
    MARCH_SHADER.to_string()
}

/// The inputs of one march.
#[derive(Clone, Debug)]
pub struct MarchParams {
    /// The unit direction to the sun in the anchor frame (x east, y north, z up).
    pub sun: [f32; 3],
    /// The tangent of the sun disc's angular radius (0.267 degrees).
    pub sun_tan: f32,
    /// The sphere's drop per squared distance from the anchor, `1 / (2 R)`, as a precomputed uniform (CON-11: never R itself; 0 for a flat
    /// synthetic world).
    pub drop_coefficient: f32,
    pub max_distance_m: f32,
    /// How far the penumbra search reaches (2 km, ADR 0085).
    pub penumbra_search_m: f32,
    /// Per level, its window in world texel indices `[x0, y0, x1, y1]` and its texel in metres.
    pub levels: Vec<([i32; 4], f32)>,
    /// The cube continues the march beyond the outermost level.
    pub use_cube: bool,
    pub anchor: [f32; 3],
    pub east: [f32; 3],
    pub north: [f32; 3],
    /// A cube texel in metres at the middle of a face.
    pub cube_texel_m: f32,
    /// The generation value from which a clipmap cell counts as changed (0: the march reports no change), see [`GENERATION_CELL_SHIFT`].
    pub dirty_since: u32,
}

/// The edge of a generation cell as a power of two of clipmap texels (8 texels).
pub const GENERATION_CELL_SHIFT: u32 = 3;

/// The bit of the step count that flags a march across a changed clipmap cell.
pub const STEPS_CHANGED_BIT: u32 = 1 << 31;

/// The penumbra search distance (REND-049), metres.
pub const PENUMBRA_SEARCH_M: f32 = 2_000.0;
/// The longest march (REND-049), metres.
pub const MAX_MARCH_M: f32 = 1_000_000.0;

/// The tangent of the sun's angular radius: 0.267 degrees.
pub const SUN_DISC_TAN: f32 = 0.004_660_3;

/// The answer of a probe.
#[derive(Clone, Debug, PartialEq)]
pub struct MarchResult {
    pub width: u32,
    pub height: u32,
    pub visibility: Vec<f32>,
    pub steps: Vec<u32>,
}

pub struct ShadowMarchGpu {
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
}

impl ShadowMarchGpu {
    pub fn new(ctx: &GpuContext) -> ShadowMarchGpu {
        let device = &ctx.device;
        let c = wgpu::ShaderStages::COMPUTE;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: c, ty, count: None };
        let tex = |dim| wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: dim,
            multisampled: false,
        };
        let storage = |format| wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-march"),
            entries: &[
                entry(
                    6,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(
                    0,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                ),
                entry(1, tex(wgpu::TextureViewDimension::D2Array)),
                entry(2, tex(wgpu::TextureViewDimension::D2Array)),
                entry(3, tex(wgpu::TextureViewDimension::D2)),
                entry(4, storage(wgpu::TextureFormat::R32Float)),
                entry(5, storage(wgpu::TextureFormat::R32Uint)),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow-march"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow-march"),
            source: wgpu::ShaderSource::Wgsl(march_shader().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("shadow-march"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        ShadowMarchGpu { pipeline, bgl }
    }

    /// The uniform block of one march.
    pub(crate) fn uniform_bytes(clip: &ClipmapGpu, cube: &CoarseCubeGpu, params: &MarchParams) -> Vec<u8> {
        assert!(
            params.levels.len() <= MARCH_MAX_LEVELS && params.levels.len() == clip.levels(),
            "the march reads every level of the clipmap"
        );
        let mut uniform: Vec<u8> = Vec::with_capacity(640);
        let f = |out: &mut Vec<u8>, v: &[f32]| v.iter().for_each(|x| out.extend_from_slice(&x.to_le_bytes()));
        f(&mut uniform, &[params.sun[0], params.sun[1], params.sun[2], params.sun_tan]);
        f(
            &mut uniform,
            &[params.drop_coefficient, params.max_distance_m, params.penumbra_search_m, if params.use_cube { 1.0 } else { 0.0 }],
        );
        f(&mut uniform, &[params.anchor[0], params.anchor[1], params.anchor[2], 0.0]);
        f(&mut uniform, &[params.east[0], params.east[1], params.east[2], 0.0]);
        f(&mut uniform, &[params.north[0], params.north[1], params.north[2], 0.0]);
        for v in [clip.texels(), params.levels.len() as u32, cube.texels(), clip.texels().trailing_zeros().min(6)] {
            uniform.extend_from_slice(&v.to_le_bytes());
        }
        f(&mut uniform, &[params.cube_texel_m, 0.0, 0.0, 0.0]);
        for v in [params.dirty_since, GENERATION_CELL_SHIFT, 0, 0] {
            uniform.extend_from_slice(&v.to_le_bytes());
        }
        for k in 0..MARCH_MAX_LEVELS {
            let w = params.levels.get(k).map_or([0; 4], |l| l.0);
            for v in w {
                uniform.extend_from_slice(&v.to_le_bytes());
            }
        }
        for k in 0..MARCH_MAX_LEVELS {
            f(&mut uniform, &[params.levels.get(k).map_or(0.0, |l| l.1), 0.0, 0.0, 0.0]);
        }
        uniform
    }

    /// Record the march of every pixel of `receivers_view` (anchor-frame positions, `w` 1 for a receiver, -1 for a pixel to skip, 0 for none)
    /// into `vis_view` (R32Float; -1 for a skipped pixel) and `steps_view` (R32Uint: steps, with [`STEPS_CHANGED_BIT`]).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &self,
        ctx: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        clip: &ClipmapGpu,
        cube: &CoarseCubeGpu,
        uniform: &wgpu::Buffer,
        gens_view: &wgpu::TextureView,
        receivers_view: &wgpu::TextureView,
        vis_view: &wgpu::TextureView,
        steps_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        timestamps: Option<wgpu::ComputePassTimestampWrites>,
    ) {
        let (clip_view, cube_view) = (clip.sampled_view(), cube.sampled_view());
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-march"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&clip_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&cube_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(receivers_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(vis_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(steps_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(gens_view) },
            ],
        });
        let mut pass =
            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shadow-march"), timestamp_writes: timestamps });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }

    /// March every receiver (`width * height` entries `[x, y, z, w]` in the anchor frame, `w` = 1 for a receiver) over the clipmap and the cube.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        ctx: &GpuContext,
        clip: &ClipmapGpu,
        cube: &CoarseCubeGpu,
        params: &MarchParams,
        receivers: &[[f32; 4]],
        width: u32,
        height: u32,
    ) -> MarchResult {
        assert_eq!(receivers.len(), (width * height) as usize, "one receiver a pixel");
        let device = &ctx.device;
        let uniform = Self::uniform_bytes(clip, cube, params);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-march-uniform"),
            size: uniform.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&uniform_buffer, 0, &uniform);

        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let make = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let receivers_tex =
            make("march-receivers", wgpu::TextureFormat::Rgba32Float, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let bytes: Vec<u8> = receivers.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &receivers_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 16), rows_per_image: Some(height) },
            size,
        );
        let usage = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC;
        let vis_tex = make("march-visibility", wgpu::TextureFormat::R32Float, usage);
        let steps_tex = make("march-steps", wgpu::TextureFormat::R32Uint, usage);
        let gens = GenerationMap::new(ctx, clip.texels(), clip.levels());
        let (receivers_view, vis_view, steps_view) = (
            receivers_tex.create_view(&Default::default()),
            vis_tex.create_view(&Default::default()),
            steps_tex.create_view(&Default::default()),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shadow-march") });
        self.encode(
            ctx,
            &mut encoder,
            clip,
            cube,
            &uniform_buffer,
            &gens.view(),
            &receivers_view,
            &vis_view,
            &steps_view,
            width,
            height,
            None,
        );
        ctx.queue.submit([encoder.finish()]);
        let read = |tex: &wgpu::Texture| -> Vec<u8> { read_texture(ctx, tex, width, height) };
        let visibility = read(&vis_tex).chunks(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        let steps = read(&steps_tex).chunks(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) & !STEPS_CHANGED_BIT).collect();
        MarchResult { width, height, visibility, steps }
    }
}

/// The texels of a 4-byte-a-texel 2D texture, row-major.
pub(crate) fn read_texture(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let row = (width * 4).next_multiple_of(256);
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("march-readback"),
        size: u64::from(row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("march-readback") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    ctx.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("readback range").to_vec();
    buffer.unmap();
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height as usize {
        out.extend_from_slice(&data[y * row as usize..y * row as usize + (width * 4) as usize]);
    }
    out
}
