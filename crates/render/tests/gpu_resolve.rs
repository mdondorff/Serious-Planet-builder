//! REND-050, REND-053, tier C: the receivers rebuilt from the depth buffer, the depth-aware upsample, the temporal pass with its rejection
//! over changed clipmap cells and after a camera cut, and the checkerboard spread of a low sun. Small frames (64 x 36) over a synthetic
//! 256^2 clipmap on the software adapter.

use planet_frame::clipmap::TexelRect;
use planet_render::march::{MarchParams, ShadowMarchGpu, PENUMBRA_SEARCH_M, SUN_DISC_TAN};
use planet_render::resolve::{
    fill_shader, receivers_shader, spread_for_sun, temporal_shader, upsample_shader, DepthTarget, ResolveCamera, ResolveParams,
    ShadowResolveGpu,
};
use planet_render::shadow::{ClipmapGpu, CoarseCubeGpu};
use planet_render::{AdapterPolicy, GpuContext};

const TEXELS: u32 = 256;
const CELLS: u32 = 16;
const CUBE: u32 = 16;
const LEVELS: usize = 3;
const FIRST_TEXEL_M: f32 = 8.0;
const W: u32 = 64;
const H: u32 = 36;
const CAMERA_X: f32 = 200.0;
const CAMERA_HEIGHT: f32 = 200.0;

struct Scene {
    ctx: GpuContext,
    clip: ClipmapGpu,
    cube: CoarseCubeGpu,
    march: ShadowMarchGpu,
}

fn scene() -> Scene {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let clip = ClipmapGpu::new(&ctx, TEXELS, LEVELS, CELLS);
    let cube = CoarseCubeGpu::new(&ctx, CUBE, CELLS);
    let march = ShadowMarchGpu::new(&ctx);
    let s = Scene { ctx, clip, cube, march };
    set_wall(&s, None);
    s
}

/// A wall one level-0 texel wide at world x = `at` (a multiple of 8 m), 100 m high along all of y; no wall for `None`.
fn set_wall(s: &Scene, at: Option<i32>) {
    set_wall_of(s, at, 100.0);
}

/// As `set_wall`, `height_m` high.
fn set_wall_of(s: &Scene, at: Option<i32>, height_m: f32) {
    for level in 0..LEVELS {
        let mut heights = vec![0f32; (TEXELS * TEXELS) as usize];
        if let Some(x) = at {
            let texel = (x as f32 / (FIRST_TEXEL_M * (1 << level) as f32)).floor() as i64;
            let slot = texel.rem_euclid(i64::from(TEXELS)) as usize;
            for j in 0..TEXELS as usize {
                heights[j * TEXELS as usize + slot] = height_m;
            }
        }
        s.clip.upload_level(&s.ctx, level, &heights);
    }
    s.clip.rebuild_all_mips(&s.ctx);
}

fn march_params(sun_elevation_deg: f32) -> MarchParams {
    let e = sun_elevation_deg.to_radians();
    MarchParams {
        sun: [-libm::cosf(e), 0.0, libm::sinf(e)],
        sun_tan: SUN_DISC_TAN,
        drop_coefficient: 0.0,
        max_distance_m: 20_000.0,
        penumbra_search_m: PENUMBRA_SEARCH_M,
        levels: (0..LEVELS)
            .map(|k| ([-(TEXELS as i32) / 2, -(TEXELS as i32) / 2, TEXELS as i32 / 2, TEXELS as i32 / 2], FIRST_TEXEL_M * (1 << k) as f32))
            .collect(),
        use_cube: false,
        anchor: [0.0, 0.0, 1.0],
        east: [1.0, 0.0, 0.0],
        north: [0.0, 1.0, 0.0],
        cube_texel_m: 100.0,
        dirty_since: 0,
    }
}

/// A camera looking straight down from `CAMERA_HEIGHT` over x = `x`: a pixel at depth z sees the ground z metres below it.
fn camera(x: f32) -> ResolveCamera {
    ResolveCamera {
        position: [x, 0.0, CAMERA_HEIGHT],
        forward: [0.0, 0.0, -1.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        tan_half_fov: [0.5, 0.5],
        near_m: 1.0,
    }
}

fn frame_params(frame: u32, cut: bool, weight: f32, spread: bool) -> ResolveParams {
    ResolveParams { camera: camera(CAMERA_X), previous: camera(CAMERA_X), history_weight: weight, cut, frame, spread }
}

/// Depth of the flat ground below the camera everywhere, with `ledge` (a distance below the camera) over the right half of the frame.
fn depths(ledge: Option<f32>, sky_rows: u32) -> Vec<f32> {
    depths_with_edge(ledge, sky_rows, W / 2)
}

/// As `depths`, with the ledge from column `edge` on.
fn depths_with_edge(ledge: Option<f32>, sky_rows: u32, edge: u32) -> Vec<f32> {
    let mut d = vec![1.0 / CAMERA_HEIGHT; (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = (y * W + x) as usize;
            if y < sky_rows {
                d[i] = 0.0;
            } else if let (Some(z), true) = (ledge, x >= edge) {
                d[i] = 1.0 / z;
            }
        }
    }
    d
}

fn depth_target(s: &Scene, d: &[f32]) -> DepthTarget {
    let t = DepthTarget::new(&s.ctx, W, H);
    t.write(&s.ctx, d);
    t
}

fn world_x(px: u32, z: f32) -> f32 {
    let ndc = (px as f32 + 0.5) / W as f32 * 2.0 - 1.0;
    CAMERA_X + ndc * 0.5 * z
}

// spec: REND-050
#[test]
fn the_receivers_come_from_the_depth_buffer() {
    let s = scene();
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let depth = depth_target(&s, &depths(None, 4));
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.8, false));
    let receivers = r.read_receivers(&s.ctx);
    let (hw, hh) = (W / 2, H / 2);
    for hy in 0..hh {
        for hx in 0..hw {
            let p = receivers[(hy * hw + hx) as usize];
            if 2 * hy < 4 {
                assert_eq!(p[3], 0.0, "half pixel ({hx}, {hy}) sees the sky");
                continue;
            }
            // Equal depths: the quad's first pixel is the receiver.
            let (px, py) = (2 * hx as i32, 2 * hy as i32);
            let ex = CAMERA_X + ((px as f32 + 0.5) / W as f32 * 2.0 - 1.0) * 0.5 * CAMERA_HEIGHT;
            let ey = (1.0 - (py as f32 + 0.5) / H as f32 * 2.0) * 0.5 * CAMERA_HEIGHT;
            assert_eq!(p[3], 1.0, "half pixel ({hx}, {hy}) has a receiver");
            assert!(
                (p[0] - ex).abs() < 0.05 && (p[1] - ey).abs() < 0.05 && p[2].abs() < 0.05,
                "({hx}, {hy}): {p:?} against ({ex}, {ey}, 0)"
            );
        }
    }
}

// spec: REND-050
#[test]
fn the_upsample_does_not_blend_across_a_depth_edge() {
    let s = scene();
    // The ground is in the shadow of a 100 m wall; a ledge 120 m high over the right half of the frame stands above the wall's top.
    set_wall(&s, Some(-96));
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let depth = depth_target(&s, &depths(Some(CAMERA_HEIGHT - 120.0), 0));
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.0, false));
    let shadow = r.read_shadow(&s.ctx);
    for y in 0..H {
        for x in 0..W {
            let v = shadow[(y * W + x) as usize];
            if x < W / 2 {
                assert!(v < 0.02, "ground pixel ({x}, {y}) at x = {} m must stay in the shadow, got {v}", world_x(x, CAMERA_HEIGHT));
            } else {
                assert!(v > 0.98, "ledge pixel ({x}, {y}) must be lit, got {v}");
            }
        }
    }
}

// spec: REND-050
#[test]
fn the_upsample_keeps_a_depth_edge_inside_a_quad_of_receivers() {
    let s = scene();
    set_wall(&s, Some(-96));
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    // The ledge starts at an odd column: the 2x2 quad of columns 32 and 33 holds both surfaces, and its receiver is the nearer one.
    let depth = depth_target(&s, &depths_with_edge(Some(CAMERA_HEIGHT - 120.0), 0, 33));
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.0, false));
    let shadow = r.read_shadow(&s.ctx);
    for y in 0..H {
        for x in 0..W {
            let v = shadow[(y * W + x) as usize];
            if x < 33 {
                assert!(v < 0.02, "ground pixel ({x}, {y}) must stay in the shadow, got {v}");
            } else {
                assert!(v > 0.98, "ledge pixel ({x}, {y}) must be lit, got {v}");
            }
        }
    }
}

// spec: REND-050
#[test]
fn a_march_across_a_changed_cell_is_flagged_and_one_elsewhere_is_not() {
    let s = scene();
    set_wall(&s, Some(-96));
    let depth = depth_target(&s, &depths(None, 0));
    let wall = |level: usize, x0: i64| (level, TexelRect { x0, y0: -128, x1: x0 + 1, y1: 128 });
    let all = (W / 2 * (H / 2)) as usize;
    // (the mark: level and rectangle, the frame it is marked in), the frame resolved, the flagged count.
    #[allow(clippy::type_complexity)]
    let cases: [(&str, Option<((usize, TexelRect), u32)>, u32, usize); 7] = [
        ("nothing changed", None, 2, 0),
        ("behind the receivers, off every ray", Some(((0, TexelRect { x0: 100, y0: -128, x1: 104, y1: 128 }), 2)), 2, 0),
        ("the wall's cell this frame", Some((wall(0, -12), 2)), 2, all),
        ("the wall's cell last frame", Some((wall(0, -12), 1)), 2, all),
        ("the wall's cell two frames ago", Some((wall(0, -12), 1)), 3, 0),
        ("a coarse level the walk does not use", Some((wall(1, -6), 2)), 2, 0),
        ("the wall's cell, addressed one clipmap edge further (toroidal)", Some((wall(0, -12 + i64::from(TEXELS)), 2)), 2, all),
    ];
    for (what, mark, frame, expected) in cases {
        let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
        if let Some(((level, rect), at)) = mark {
            r.generations_mut().mark(&s.ctx, at, level, &rect);
        }
        r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(frame, false, 0.8, false));
        let flagged = r.read_half_changed(&s.ctx).into_iter().filter(|c| *c).count();
        assert_eq!(flagged, expected, "{what}");
    }
}

fn shadow_of_drag(weight: f32, mark: bool) -> (Vec<f32>, f32) {
    let s = scene();
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let depth = depth_target(&s, &depths(None, 0));
    // The wall is dragged (the scenario's plateau stamp, scaled to this 2 km clipmap of 8 m texels): its shadow's tip stands at 136, 376 and
    // 236 m, so the third frame lights what the second shadowed.
    let walls = [-440, -200, -340];
    let mut last = None;
    for (i, x) in walls.iter().enumerate() {
        let frame = i as u32 + 1;
        set_wall(&s, Some(*x));
        if mark {
            let cell = |x: i32| TexelRect { x0: i64::from(x / 8), y0: -128, x1: i64::from(x / 8) + 1, y1: 128 };
            if i > 0 {
                r.generations_mut().mark(&s.ctx, frame, 0, &cell(walls[i - 1]));
            }
            r.generations_mut().mark(&s.ctx, frame, 0, &cell(*x));
        }
        r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(frame, false, weight, false));
        last = Some(r.read_shadow(&s.ctx));
    }
    (last.unwrap(), 236.0)
}

// spec: REND-050
#[test]
fn a_dragged_wall_leaves_no_ghost() {
    let (with, edge) = shadow_of_drag(0.8, true);
    let (without, _) = shadow_of_drag(0.0, false);
    for y in 0..H {
        for x in 0..W {
            // The edge of the upsample's pixels and of the penumbra (about 5 m) is excused.
            if (world_x(x, CAMERA_HEIGHT) - edge).abs() < 12.0 {
                continue;
            }
            let (a, b) = (with[(y * W + x) as usize], without[(y * W + x) as usize]);
            assert!((a - b).abs() <= 1.0 / 255.0, "pixel ({x}, {y}): {a} with history, {b} without");
        }
    }
}

// spec: REND-050
#[test]
fn a_camera_cut_voids_the_history() {
    let s = scene();
    let depth = depth_target(&s, &depths(None, 0));
    let mut a = ShadowResolveGpu::new(&s.ctx, &s.clip);
    set_wall(&s, Some(-96));
    a.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.8, false));
    set_wall(&s, None);
    a.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(2, true, 0.8, false));
    let mut b = ShadowResolveGpu::new(&s.ctx, &s.clip);
    b.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.8, false));
    assert_eq!(a.read_shadow(&s.ctx), b.read_shadow(&s.ctx), "after a cut the frame is drawn from its own march alone");
}

// spec: REND-050
#[test]
fn the_spread_fills_every_pixel_within_two_frames_and_a_cut_marches_all() {
    let s = scene();
    set_wall(&s, Some(-96));
    let depth = depth_target(&s, &depths(None, 0));
    let sun = 2.0f32;
    let spread = spread_for_sun(libm::sinf(sun.to_radians()));
    assert!(spread && !spread_for_sun(libm::sinf(6f32.to_radians())), "the spread is for a sun below 5 degrees");
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let mut marched = Vec::new();
    let mut shadows = Vec::new();
    for frame in 1..=3u32 {
        r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(sun), &depth, &frame_params(frame, false, 0.8, spread));
        marched.push(r.read_half_visibility(&s.ctx).into_iter().map(|v| v >= 0.0).collect::<Vec<_>>());
        shadows.push(r.read_shadow(&s.ctx));
    }
    assert!(marched[0].iter().all(|m| *m), "the first frame has no history: every ray is marched");
    let count = |m: &[bool]| m.iter().filter(|x| **x).count();
    let half = marched[1].len() / 2;
    assert_eq!(count(&marched[1]), half, "half of the rays in the second frame");
    assert_eq!(count(&marched[2]), half, "and the other half in the third");
    assert!(marched[1].iter().zip(&marched[2]).all(|(a, b)| a != b), "two frames cover every pixel exactly once");
    // The scene is still, so the shadow does not move while it is filled in.
    for (i, (a, b)) in shadows[0].iter().zip(&shadows[2]).enumerate() {
        assert!((a - b).abs() <= 1.0 / 255.0, "pixel {i}: {a} in the first frame, {b} in the third");
    }
    // A cut marches everything in its frame.
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(sun), &depth, &frame_params(4, true, 0.8, spread));
    assert!(r.read_half_visibility(&s.ctx).iter().all(|v| *v >= 0.0));
}

// spec: REND-053
#[test]
fn the_same_sequence_draws_the_same_bytes_and_the_passes_hold_no_atomics() {
    let run = || {
        let s = scene();
        let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
        let depth = depth_target(&s, &depths(Some(CAMERA_HEIGHT - 120.0), 2));
        let mut out = Vec::new();
        for (i, x) in [-96, -64, -32].iter().enumerate() {
            set_wall(&s, Some(*x));
            r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(3.0), &depth, &frame_params(i as u32 + 1, false, 0.8, true));
            out.push(r.read_shadow(&s.ctx));
        }
        out
    };
    assert_eq!(run(), run());
    for (name, src) in
        [("receivers", receivers_shader()), ("fill", fill_shader()), ("upsample", upsample_shader()), ("temporal", temporal_shader())]
    {
        assert!(!src.contains("atomic"), "REND-053: {name} must not use atomics");
    }
}

// spec: REND-050
#[test]
fn a_still_scene_with_a_shadow_edge_in_view_does_not_flicker_under_the_spread() {
    let s = scene();
    // A 5 m wall under a 2 degree sun: the shadow ends at 199 m, inside the frame (x from 100 to 300 m).
    set_wall_of(&s, Some(48), 5.0);
    let depth = depth_target(&s, &depths(None, 0));
    let sun = 2.0f32;
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let mut frames = Vec::new();
    for frame in 1..=4u32 {
        r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(sun), &depth, &frame_params(frame, false, 0.8, true));
        frames.push(r.read_shadow(&s.ctx));
    }
    let lit = frames[0].iter().filter(|v| **v > 0.98).count();
    let dark = frames[0].iter().filter(|v| **v < 0.02).count();
    assert!(lit > 100 && dark > 100, "the frame holds both sides of the edge ({lit} lit, {dark} dark)");
    for (f, frame) in frames.iter().enumerate().skip(1) {
        for (i, (a, b)) in frames[0].iter().zip(frame).enumerate() {
            assert!((a - b).abs() <= 1.0 / 255.0, "pixel {i}: {a} in the first frame, {b} in frame {}", f + 1);
        }
    }
}

// spec: REND-050
#[test]
fn the_sky_does_not_widen_the_history_clamp_below_the_horizon() {
    let s = scene();
    let depth = depth_target(&s, &depths(None, 4));
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    // The first frame is lit (history 1); the wall then rises without a cut or a mark, so the clamp alone has to refuse the lit history.
    set_wall(&s, None);
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(1, true, 0.8, false));
    set_wall(&s, Some(-96));
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(10.0), &depth, &frame_params(2, false, 0.8, false));
    let shadow = r.read_shadow(&s.ctx);
    for x in 0..W {
        let v = shadow[(4 * W + x) as usize];
        assert!(v < 0.02, "the row under the sky at column {x} must be dark, got {v}");
    }
}

// spec: REND-052
/// A standalone cost probe of the resolve passes (receivers, march, fill, upsample, temporal) at 1440p over a flat synthetic clipmap, as the
/// median of waited frames. Not a verdict: REND-052 is judged on the frame harness with GPU timestamps; this is a quick number for the
/// passes alone (`cargo test -p planet-render --test gpu_all resolve_cost -- --ignored --nocapture`, on the discrete GPU with
/// `PLANET_ADAPTER=hardware`).
#[test]
#[ignore = "a timing probe, run by hand"]
fn resolve_cost_probe_at_1440p() {
    let s = scene();
    set_wall(&s, Some(-96));
    let (w, h) = (2560u32, 1440u32);
    let target = DepthTarget::new(&s.ctx, w, h);
    target.write(&s.ctx, &vec![1.0 / CAMERA_HEIGHT; (w * h) as usize]);
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let timed = r.enable_timing(&s.ctx);
    for sun in [45.0f32, 2.0] {
        let mut passes: Vec<[f64; 5]> = Vec::new();
        let mut times = Vec::new();
        let mut cpu = Vec::new();
        for frame in 1..=24u32 {
            let params = ResolveParams {
                camera: camera(CAMERA_X),
                previous: camera(CAMERA_X),
                history_weight: 0.8,
                cut: frame == 1,
                frame,
                spread: spread_for_sun(libm::sinf(sun.to_radians())),
            };
            let t = std::time::Instant::now();
            r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &march_params(sun), &target, &params);
            let submitted = t.elapsed().as_secs_f64() * 1000.0;
            s.ctx.wait_idle();
            if frame > 3 {
                passes.extend(r.pass_ms(&s.ctx));
                cpu.push(submitted);
                times.push(t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        cpu.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!(
            "resolve at 1440p, sun {sun} deg: median {:.2} ms (CPU until submitted {:.2} ms), min {:.2} ms, max {:.2} ms (waited frames)",
            times[times.len() / 2],
            cpu[cpu.len() / 2],
            times[0],
            times[times.len() - 1]
        );
        if timed && !passes.is_empty() {
            let median = |i: usize| {
                let mut v: Vec<f64> = passes.iter().map(|p| p[i]).collect();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                v[v.len() / 2]
            };
            let rows: Vec<String> =
                planet_render::resolve::RESOLVE_PASS_NAMES.iter().enumerate().map(|(i, n)| format!("{n} {:.2} ms", median(i))).collect();
            eprintln!("  GPU timestamps, median per pass: {}", rows.join(", "));
        }
    }
}

// spec: REND-050
#[test]
fn receivers_far_from_the_anchor_land_in_the_clipmaps_azimuthal_frame() {
    // A real planet radius, on the CPU side of the test only: the shader gets the coefficient 1 / (2 R).
    const PLANET_R: f64 = 6_371_000.0;
    let s = scene();
    let c = (0.5 / PLANET_R) as f32;
    // The camera 30 km up looking at the horizon: a pixel at a depth of 400 km sees a Euclidean point far from the anchor.
    let cam = ResolveCamera {
        position: [0.0, 0.0, 30_000.0],
        forward: [0.0, 1.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        tan_half_fov: [0.5, 0.5],
        near_m: 1.0,
    };
    let z_view = 400_000.0f32;
    let mut mp = march_params(10.0);
    mp.drop_coefficient = c;
    let depth = depth_target(&s, &vec![1.0 / z_view; (W * H) as usize]);
    let mut r = ShadowResolveGpu::new(&s.ctx, &s.clip);
    let rp = ResolveParams { camera: cam, previous: cam, history_weight: 0.8, cut: true, frame: 1, spread: false };
    r.resolve(&s.ctx, &s.march, &s.clip, &s.cube, &mp, &depth, &rp);
    let receivers = r.read_receivers(&s.ctx);
    for (hx, hy) in [(0u32, 0u32), (16, 9), (31, 17), (5, 12)] {
        let got = receivers[(hy * (W / 2) + hx) as usize];
        // The Euclidean point the depth puts at the quad's first pixel, then the exact plane position and height - drop in f64.
        let (px, py) = ((2 * hx) as f64 + 0.5, (2 * hy) as f64 + 0.5);
        let (nx, ny) = (px / f64::from(W) * 2.0 - 1.0, 1.0 - py / f64::from(H) * 2.0);
        let (ex, ey, ez) = (nx * 0.5 * f64::from(z_view), f64::from(z_view), 30_000.0 + ny * 0.5 * f64::from(z_view));
        let rho_e = libm::hypot(ex, ey);
        let theta = libm::atan2(rho_e, ez + PLANET_R);
        let plane = PLANET_R * theta;
        let height = libm::hypot(rho_e, ez + PLANET_R) - PLANET_R;
        let drop = 2.0 * PLANET_R * libm::sin(0.5 * theta) * libm::sin(0.5 * theta);
        let want = [ex * plane / rho_e, ey * plane / rho_e, height - drop];
        for k in 0..3 {
            assert!(
                (f64::from(got[k]) - want[k]).abs() < 2.0e-6 * want[k].abs().max(1.0e3),
                "half pixel ({hx}, {hy}), axis {k}: {} against {} (the chord differs from the arc by {:.0} m)",
                got[k],
                want[k],
                (plane - rho_e).abs()
            );
        }
    }
}
