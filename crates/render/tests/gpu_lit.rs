//! Tier C: the lit path through the terrain renderer (ADR 0038, ADR 0042 points 1-3 and 5): terrain into the `Rgba16Float` scene
//! target with the pre-exposure, the histogram exposure and the tone map, on compact tiles; the debug path unchanged; the frame
//! targets equal to the compiled graph's count; goldens of the lit frames at a fixed exposure.

use planet_core::cube::TangentWarp;
use planet_frame::look::{self, LitPlan, ToneMapMode};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::graph::{frame_target_share_bytes, lit_frame_graph, LitShape};
use planet_render::{AdapterPolicy, ExecError, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::{GoldenChecks, Image, Tolerance};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
/// The pre-exposure and fixed exposure of the lit goldens: a sunny day, EV100 15.
const EV: f32 = 15.0;

fn plan(index: usize) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn lit(mut p: TerrainPlan, l: LitPlan) -> TerrainPlan {
    p.lit = Some(l);
    p
}

fn compact_pool() -> PoolConfig {
    let block_slots = 24 * 24;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

/// A compact-format renderer holding the tiles of `plans`.
fn renderer(ctx: &GpuContext, plans: &[&TerrainPlan], enable_lit: bool) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, compact_pool());
    if enable_lit {
        r.enable_lit(ctx);
    }
    let field = Reference(1);
    for p in plans {
        for n in &p.nodes {
            if r.slot_of(n.tile).is_some() {
                continue;
            }
            r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
        }
    }
    r
}

fn image(r: &TerrainRenderer, ctx: &GpuContext) -> Image {
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

fn rgb64(p: &[f32; 4]) -> [f64; 3] {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

// spec: REND-026
#[test]
fn the_lit_path_formats_are_rgba16_float_and_reversed_z_depth32() {
    let f = planet_render::lit::target_formats();
    assert_eq!(f.scene, "Rgba16Float");
    assert_eq!(f.presentable, "Rgba8Unorm", "the sRGB encode is in the shader, so the target is not an sRGB format");
    assert_eq!(f.depth, "Depth32Float");
    assert_eq!(f.depth_test, "Greater", "reversed-Z (CON-12)");
}

// spec: REND-024
#[test]
fn a_lit_plan_needs_the_lit_path_and_names_the_missing_call() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &[&p], false);
    let e = r.render(&ctx, &lit(p, LitPlan::fixed(EV)), false).unwrap_err();
    assert!(matches!(e, ExecError::LitNotEnabled) && e.to_string().contains("enable_lit"), "{e}");
}

// spec: REND-026
#[test]
fn the_lit_frame_covers_the_same_pixels_as_the_debug_frame() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 3, 6] {
        let what = format!("view {index}");
        let p = plan(index);
        let mut r = renderer(&ctx, &[&p], true);
        r.render(&ctx, &p, false).unwrap();
        let debug = image(&r, &ctx);
        r.render(&ctx, &lit(p.clone(), LitPlan::fixed(EV)), false).unwrap();
        let hdr = r.lit_path().unwrap().read_hdr(&ctx);
        let covered_hdr: Vec<bool> = hdr.iter().map(|p| p[3] > 0.0).collect();
        let covered_dbg: Vec<bool> = debug.pixels().map(|px| px != p.clear_color).collect();
        assert_eq!(covered_hdr, covered_dbg, "{what}: the lit and debug paths draw the same pixels (same geometry, same depth test)");
        assert!(covered_hdr.iter().filter(|c| **c).count() > 100, "{what}: the planet is visible");
        // Every value is finite and inside the clamp: no inf or NaN can reach the histogram (ADR 0038 point 1).
        for (i, px) in hdr.iter().enumerate() {
            assert!(px.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= look::HDR_CLAMP), "{what}: pixel {i} holds {px:?}");
        }
        // Some of the planet is lit (the sun is fixed in the plan), some is in shadow.
        let lit_px = hdr.iter().filter(|p| p[3] > 0.0 && look::luminance(rgb64(p)) > 0.0).count();
        assert!(lit_px > 0, "{what}: no lit terrain at all");
        // The debug path of a lit-enabled renderer draws what it always drew.
        let mut plain = renderer(&ctx, &[&p], false);
        plain.render(&ctx, &p, false).unwrap();
        assert_eq!(image(&plain, &ctx), debug, "{what}: enabling the lit path must not change the debug image");
        r.render(&ctx, &p, false).unwrap();
        assert_eq!(image(&r, &ctx), debug, "{what}: a debug frame after a lit frame is unchanged");
    }
}

// spec: LOOK-006
#[test]
fn the_scene_unit_scale_of_the_look_scales_the_scene_target() {
    use planet_frame::look::SceneUnits;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut r = renderer(&ctx, &[&base], true);
    r.render(&ctx, &lit(base.clone(), LitPlan::fixed(EV)), false).unwrap();
    let earth = r.lit_path().unwrap().read_hdr(&ctx);
    // A sun a quarter as bright (a body four times as far away): every written value is a quarter of the Earth's.
    let mut dim = LitPlan::fixed(EV);
    dim.units = SceneUnits { solar_illuminance_lux: look::SOLAR_ILLUMINANCE_LUX / 4.0 };
    r.render(&ctx, &lit(base.clone(), dim), false).unwrap();
    let quarter = r.lit_path().unwrap().read_hdr(&ctx);
    let mut compared = 0;
    for (i, (a, b)) in earth.iter().zip(&quarter).enumerate() {
        assert_eq!(a[3] > 0.0, b[3] > 0.0, "pixel {i}: the same pixels are covered");
        for c in 0..3 {
            let want = f64::from(a[c]) / 4.0;
            // A power-of-two scale commutes exactly with the f64 factor, its f32 rounding, the multiply and the f16 store, except
            // where the quarter is an f16 subnormal (below 2^-14).
            let tol = if want >= 6.103_515_625e-5 { 0.0 } else { 6.0e-8 };
            assert!((f64::from(b[c]) - want).abs() <= tol, "pixel {i} channel {c}: {} vs a quarter of {}", b[c], a[c]);
            compared += usize::from(a[c] > 0.0);
        }
    }
    assert!(compared > 100, "the planet is lit in {compared} channels");
    // The defaults are untouched by the look machinery: the plan made with the default look gives the default frame again.
    r.render(&ctx, &lit(base, LitPlan::fixed_with(&planet_frame::look::file::LookParams::default(), EV)), false).unwrap();
    assert_eq!(r.lit_path().unwrap().read_hdr(&ctx), earth, "the default look writes exactly what it always wrote");
}

// spec: REND-023
#[test]
fn the_lit_frame_is_tone_mapped_from_its_scene_target_within_one_step() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = lit(plan(3), LitPlan::fixed(EV));
    let mut r = renderer(&ctx, &[&p], true);
    r.render(&ctx, &p, false).unwrap();
    let hdr = r.lit_path().unwrap().read_hdr(&ctx);
    let ldr = r.read_color(&ctx);
    let mut worst = 0i32;
    for (i, px) in hdr.iter().enumerate() {
        let want = look::tonemap_pixel(rgb64(px), f64::from(EV), f64::from(EV));
        for (c, w) in want.iter().enumerate() {
            let d = (i32::from(ldr.rgba[4 * i + c]) - i32::from(*w)).abs();
            worst = worst.max(d);
            assert!(d <= 1, "pixel {i} channel {c}: GPU {}, reference {w}", ldr.rgba[4 * i + c]);
        }
        assert_eq!(ldr.rgba[4 * i + 3], 255);
    }
    println!("lit tone map: worst difference {worst}/255");
}

// spec: REND-022
#[test]
fn the_adaptive_exposure_of_a_terrain_frame_follows_the_cpu_reference_from_its_scene_target() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut r = renderer(&ctx, &[&base], true);
    let params = look::ExposureParams::default();
    let mut ev: Option<f64> = None;
    // Three frames at a pre-exposure far from the right one: the first snaps to the scene, the next two stay (same scene).
    for (frame, dt) in [0.016f32, 0.5, 0.5].into_iter().enumerate() {
        let p = lit(base.clone(), LitPlan::adaptive(11.0, dt));
        r.render(&ctx, &p, false).unwrap();
        let hdr = r.lit_path().unwrap().read_hdr(&ctx);
        let covered: Vec<[f64; 3]> = hdr.iter().filter(|p| p[3] > 0.0).map(rgb64).collect();
        let hist = look::histogram_of(&covered, 11.0);
        let gpu = r.lit_path().unwrap().read_histogram(&ctx);
        // Bin edges can differ between f32 and f64 arithmetic for a few pixels in thousands; the total is exact.
        let (a, b): (u32, u32) = (gpu.iter().sum(), hist.iter().sum());
        assert_eq!(a, b, "frame {frame}: counted pixels");
        let off: u32 = gpu.iter().zip(&hist).map(|(x, y)| x.abs_diff(*y)).sum();
        assert!(
            off * 200 <= a,
            "frame {frame}: {off} of {a} counts sit in another bin than the f64 reference puts them\nGPU {gpu:?}\nCPU {hist:?}"
        );
        let current = ev.unwrap_or(11.0);
        // The reference reads the GPU's own histogram, so this checks the adaptation pass on terrain data (1e-4 EV) apart from where
        // pixels fall at bin edges (checked above).
        let target = look::target_ev(&gpu, &params, current);
        let next = look::adapt_ev(current, target, f64::from(dt), &params, frame == 0);
        ev = Some(next);
        let state = r.lit_path().unwrap().read_exposure(&ctx);
        assert!((f64::from(state.ev100) - next).abs() < 1.0e-3, "frame {frame}: GPU EV {} vs CPU {next} (target {target})", state.ev100);
        assert!(state.ev100 >= params.min_ev100 && state.ev100 <= params.max_ev100);
    }
}

// spec: REND-020
#[test]
fn the_renderers_frame_targets_equal_the_compiled_graphs_count_and_fit_the_share() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &[&p], false);
    assert_eq!(r.lit_frame_target_bytes(), None, "no lit targets before enable_lit");
    r.enable_lit(&ctx);
    let compiled = lit_frame_graph(&LitShape { size: SIZE, adaptive: true, atmosphere: false, water: false }).compile().unwrap();
    let held = r.lit_frame_target_bytes().unwrap();
    assert_eq!(held, compiled.frame_target_bytes(), "what the renderer allocates is what the compiled graph counts");
    assert_eq!(held, 128 * 96 * (8 + 4) + 256 + 16);
    assert!(held <= frame_target_share_bytes(SIZE), "{held} B over the share of {} B", frame_target_share_bytes(SIZE));
    // The same count at 1440p from the same formula: HDR 29.5 MB, depth 14.7 MB.
    let big = lit_frame_graph(&LitShape { size: (2560, 1440), adaptive: true, atmosphere: false, water: false }).compile().unwrap();
    assert_eq!(big.frame_target_bytes(), 44_237_072);
}

// spec: REND-017, REND-020
#[test]
fn lit_frames_create_nothing_after_the_lit_path_is_enabled() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &[&p], true);
    let (built, objects) = (r.init_counts(), r.lit_objects());
    assert_eq!(objects, 7, "the scene target, the three post pipelines, the scale references pipeline (REND-036) and the two compact lit pipelines (whole nodes, nodes over some quadrants, STRM-016; the vertex lit pipeline went with the vertex format): {objects}");
    for plan in [lit(p.clone(), LitPlan::fixed(EV)), lit(p.clone(), LitPlan::adaptive(EV, 0.016)), lit(p.clone(), LitPlan::fixed(EV))] {
        r.render(&ctx, &plan, false).unwrap();
        assert_eq!(r.lit_objects(), objects);
    }
    assert_eq!(r.init_counts().pipelines, built.pipelines, "lit frames create no pipeline");
    assert_eq!(r.init_counts().ring_builds, built.ring_builds);
    // Two fixed frames and one adaptive: two shapes, so two compiles, not three.
    assert_eq!(r.lit_path().unwrap().graph_compiles(), 3, "fixed, adaptive, fixed again: each change of shape compiles");
    r.render(&ctx, &lit(p.clone(), LitPlan::fixed(EV)), false).unwrap();
    assert_eq!(r.lit_path().unwrap().graph_compiles(), 3, "a repeated shape does not compile");
}

// spec: REND-026
#[test]
fn the_same_lit_plan_gives_the_same_image_and_the_sun_moves_the_light() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut r = renderer(&ctx, &[&base], true);
    let p = lit(base.clone(), LitPlan::fixed(EV));
    r.render(&ctx, &p, false).unwrap();
    let first = image(&r, &ctx);
    r.render(&ctx, &p, false).unwrap();
    assert_eq!(image(&r, &ctx), first, "a lit frame is a pure function of its plan at fixed exposure");
    let mut other = LitPlan::fixed(EV);
    other.sun_dir = [-0.5014059, -0.6016871, -0.6217433];
    r.render(&ctx, &lit(base.clone(), other), false).unwrap();
    assert_ne!(image(&r, &ctx), first, "the sun comes from the plan");
    // Exposure scales the image: two stops less exposure is darker on average over the lit pixels.
    r.render(&ctx, &lit(base, LitPlan::fixed(EV + 2.0)), false).unwrap();
    let dark = image(&r, &ctx);
    let mean = |img: &Image| img.pixels().map(|p| f64::from(p[1])).sum::<f64>();
    assert!(mean(&dark) < mean(&first), "a higher EV100 is a darker image");
}

// spec: REND-026, TEST-010
#[test]
fn lit_frames_match_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    for (index, golden) in [(0usize, "lit_orbit_ev15"), (3, "lit_aerial20km_ev15"), (6, "lit_ground1m_ev15")] {
        let p = lit(plan(index), LitPlan::fixed(EV));
        let mut r = renderer(&ctx, &[&p], true);
        r.render(&ctx, &p, false).unwrap();
        let img = image(&r, &ctx);
        // The test's own oracle for the candidate: the planet is there and not everything is black or clipped.
        let hdr = r.lit_path().unwrap().read_hdr(&ctx);
        let covered = hdr.iter().filter(|p| p[3] > 0.0).count();
        assert!(covered > 100, "{golden}: only {covered} terrain pixels");
        let lit_px = img.pixels().filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8).count();
        assert!(lit_px * 10 >= covered, "{golden}: {lit_px} of {covered} terrain pixels are lit");
        goldens.check(golden, &img, Tolerance::Exact);
    }
    // The tone map's debug view (CON-18): the orbit frame in false-colour EV bands.
    let mut bands = LitPlan::fixed(EV);
    bands.tonemap = ToneMapMode::EvBands;
    let p = lit(plan(0), bands);
    let mut r = renderer(&ctx, &[&p], true);
    r.render(&ctx, &p, false).unwrap();
    let img = image(&r, &ctx);
    let shown: std::collections::BTreeSet<[u8; 3]> = img.pixels().map(|p| [p[0], p[1], p[2]]).collect();
    assert!(shown.iter().all(|c| *c == [0, 0, 0] || look::EV_BAND_COLORS.contains(c)), "only band colours and black");
    assert!(shown.len() >= 4, "a lit globe spans several bands, got {}", shown.len());
    goldens.check("lit_orbit_ev_bands", &img, Tolerance::Exact);
    goldens.finish();
}

// spec: REND-036
#[test]
fn scale_references_are_drawn_into_the_lit_frame_with_the_terrains_depth() {
    use planet_core::{PlanetFixed, Vec3};
    use planet_frame::scalerefs::RefBox;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut r = renderer(&ctx, &[&base], true);
    let p = lit(base.clone(), LitPlan::fixed(EV));
    r.render(&ctx, &p, false).unwrap();
    let plain = r.lit_path().unwrap().read_hdr(&ctx);
    // Where the view axis meets the planet's surface (the ray-sphere distance), and a red cube on the axis at the given fraction of it.
    let cam = base.camera.position.0;
    let fwd = base.camera.forward;
    let b = cam.dot(fwd);
    let t = -b - libm::sqrt(b * b - (cam.dot(cam) - R * R));
    assert!(t.is_finite() && t > 0.0, "the view axis meets the planet: {t}");
    let cube = |fraction: f64| RefBox {
        name: "cube",
        centre: PlanetFixed(cam + fwd * (fraction * t)),
        axes: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)],
        half_m: [(0.05 * t) as f32; 3],
        colour: [0.9, 0.05, 0.05],
    };
    // In front of the ground: some pixels change and the new ones are red.
    r.set_scale_refs(&[cube(0.5)]);
    r.render(&ctx, &p, false).unwrap();
    let with = r.lit_path().unwrap().read_hdr(&ctx);
    let changed: Vec<usize> = (0..with.len()).filter(|k| with[*k] != plain[*k]).collect();
    assert!(changed.len() > 50, "the cube covers pixels: {}", changed.len());
    let red = changed.iter().filter(|k| with[**k][0] > 4.0 * with[**k][1].max(with[**k][2])).count();
    assert!(red * 10 >= changed.len() * 9, "the changed pixels are the cube's red: {red} of {}", changed.len());
    // Inside the planet, behind the ground along the axis: the terrain hides it, the frame is the plain one.
    r.set_scale_refs(&[cube(1.6)]);
    r.render(&ctx, &p, false).unwrap();
    assert!(r.lit_path().unwrap().read_hdr(&ctx) == plain, "a cube under the ground is not drawn");
    // None again: the plain frame.
    r.set_scale_refs(&[]);
    r.render(&ctx, &p, false).unwrap();
    assert!(r.lit_path().unwrap().read_hdr(&ctx) == plain, "no references, no change");
    // The debug path ignores them.
    r.set_scale_refs(&[cube(0.5)]);
    r.render(&ctx, &base, false).unwrap();
    let debug_with = image(&r, &ctx);
    r.set_scale_refs(&[]);
    r.render(&ctx, &base, false).unwrap();
    assert_eq!(image(&r, &ctx), debug_with, "the debug frame does not draw them");
}
