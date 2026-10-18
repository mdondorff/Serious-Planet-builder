//! Tier C: the lit path with the atmosphere through the terrain renderer (ADR 0040, ADR 0042 points 1-3 and 5; ATMO-002, ATMO-011):
//! sky and aerial perspective over the terrain on compact tiles, the frame targets equal to the compiled graph's count, nothing
//! created after the atmosphere is enabled, the lit path without an atmosphere unchanged, goldens of the sky at a fixed exposure.

use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::{Atmosphere, AtmosphereView};
use planet_frame::look::file::LookParams;
use planet_frame::look::{self, LitPlan};
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
    let one = compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    PoolConfig { vertex_slots: 256, fallback_slots: 192, ..one }
}

/// A compact-format renderer holding the tiles of `plans`.
fn renderer(ctx: &GpuContext, plans: &[&TerrainPlan], enable_lit: bool) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_compact_pools(ctx, SIZE, CELLS, compact_pool());
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

fn atmosphere_plan(view: AtmosphereView) -> LitPlan {
    let mut l = LitPlan::fixed(EV).with_atmosphere(&LookParams::default(), R).unwrap();
    l.atmosphere.as_mut().unwrap().view = view;
    l
}

fn renderer_atmosphere(ctx: &GpuContext, plans: &[&TerrainPlan]) -> TerrainRenderer {
    let mut r = renderer(ctx, plans, true);
    r.enable_atmosphere(ctx, &Atmosphere::earth(R));
    r
}

fn luminance_of(p: &[f32; 4]) -> f64 {
    look::luminance(rgb64(p))
}

// spec: ATMO-011
#[test]
fn an_atmosphere_plan_needs_the_atmosphere_and_names_the_missing_call() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &[&p], true);
    let e = r.render(&ctx, &lit(p, atmosphere_plan(AtmosphereView::Composite)), false).unwrap_err();
    assert!(matches!(e, ExecError::AtmosphereNotEnabled) && e.to_string().contains("enable_atmosphere"), "{e}");
}

// spec: ATMO-002
#[test]
fn the_atmosphere_lights_the_sky_and_hazes_the_terrain() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 3, 6] {
        let what = format!("view {index}");
        let base = plan(index);
        let mut r = renderer_atmosphere(&ctx, &[&base]);
        r.render(&ctx, &lit(base.clone(), LitPlan::fixed(EV)), false).unwrap();
        let plain = r.lit_path().unwrap().read_hdr(&ctx);
        r.render(&ctx, &lit(base.clone(), atmosphere_plan(AtmosphereView::Composite)), false).unwrap();
        let hdr = r.lit_path().unwrap().read_hdr(&ctx);
        // The terrain covers the same pixels (alpha is the terrain's coverage).
        let covered: Vec<bool> = hdr.iter().map(|p| p[3] > 0.0).collect();
        assert_eq!(covered, plain.iter().map(|p| p[3] > 0.0).collect::<Vec<_>>(), "{what}: same coverage");
        assert!(hdr.iter().all(|p| p.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= look::HDR_CLAMP)), "{what}: finite and clamped");
        // Without terrain the plain frame is black; with the atmosphere the sky is lit.
        let sky: Vec<&[f32; 4]> = hdr.iter().filter(|p| p[3] == 0.0).collect();
        let lit_sky = sky.iter().filter(|p| luminance_of(p) > 0.0).count();
        match index {
            3 => assert!(lit_sky * 2 > sky.len(), "{what}: the sky is lit"),
            0 => assert!(lit_sky > 20, "{what}: the limb glows ({lit_sky} sky pixels)"),
            _ => {}
        }
        assert!(plain.iter().filter(|p| p[3] == 0.0).all(|p| luminance_of(p) == 0.0), "{what}: no sky without an atmosphere");
        // The terrain is lit differently: the sun's light is dimmed and the sky adds ambient and haze.
        let a: f64 = hdr.iter().filter(|p| p[3] > 0.0).map(luminance_of).sum();
        let b: f64 = plain.iter().filter(|p| p[3] > 0.0).map(luminance_of).sum();
        assert!(a > 0.0 && (a - b).abs() > 1e-3 * b.max(1e-9), "{what}: terrain luminance {a} vs {b}");
    }
}

// spec: ATMO-011
#[test]
fn the_frame_targets_with_an_atmosphere_equal_the_compiled_graphs_count_and_nothing_is_created_per_frame() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer_atmosphere(&ctx, &[&p]);
    let compiled = lit_frame_graph(&LitShape { size: SIZE, adaptive: true, atmosphere: true, water: false }).compile().unwrap();
    let held = r.lit_frame_target_bytes().unwrap();
    assert_eq!(held, compiled.frame_target_bytes(), "what the renderer allocates is what the compiled graph counts");
    assert!(held <= frame_target_share_bytes(SIZE), "{held} B over the share of {} B", frame_target_share_bytes(SIZE));
    let big = lit_frame_graph(&LitShape { size: (2560, 1440), adaptive: true, atmosphere: true, water: false }).compile().unwrap();
    assert!(big.frame_target_bytes() <= frame_target_share_bytes((2560, 1440)), "{} B at 1440p", big.frame_target_bytes());
    let (built, objects) = (r.init_counts(), r.lit_objects());
    for plan in [
        lit(p.clone(), atmosphere_plan(AtmosphereView::Composite)),
        lit(p.clone(), LitPlan::fixed(EV)),
        lit(p.clone(), atmosphere_plan(AtmosphereView::SkyViewLut)),
    ] {
        r.render(&ctx, &plan, false).unwrap();
        assert_eq!(r.lit_objects(), objects);
    }
    assert_eq!(r.init_counts().pipelines, built.pipelines);
    let path = r.lit_path().unwrap();
    assert_eq!(path.atmosphere().unwrap().table_uploads(), 1, "the tables are uploaded once for the body, not per frame");
    assert_eq!(path.graph_compiles(), 3, "atmosphere, none, atmosphere: each change of shape compiles");
    let names = path.executed_passes();
    assert_eq!(names, ["atmosphere-luts", "sky-view", "aerial-perspective", "terrain", "sky-composite", "exposure-adapt", "tonemap"]);
}

// spec: ATMO-011
#[test]
fn a_plan_without_an_atmosphere_draws_what_it_drew_with_the_atmosphere_enabled() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut plain = renderer(&ctx, &[&base], true);
    plain.render(&ctx, &lit(base.clone(), LitPlan::fixed(EV)), false).unwrap();
    let want = plain.read_color(&ctx);
    let mut with = renderer_atmosphere(&ctx, &[&base]);
    with.render(&ctx, &lit(base.clone(), LitPlan::fixed(EV)), false).unwrap();
    assert_eq!(with.read_color(&ctx).rgba, want.rgba);
    with.render(&ctx, &base, false).unwrap();
    let mut debug = renderer(&ctx, &[&base], false);
    debug.render(&ctx, &base, false).unwrap();
    assert_eq!(with.read_color(&ctx).rgba, debug.read_color(&ctx).rgba, "the debug path is untouched");
}

// spec: ATMO-011
#[test]
fn the_atmosphere_debug_views_are_discrete_levels_and_the_table_views_show_their_table() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(0);
    let mut r = renderer_atmosphere(&ctx, &[&base]);
    for view in [
        AtmosphereView::TransmittanceLut,
        AtmosphereView::MultiScatterLut,
        AtmosphereView::SkyViewLut,
        AtmosphereView::AerialPerspective,
        AtmosphereView::ViewTransmittance,
    ] {
        r.render(&ctx, &lit(base.clone(), atmosphere_plan(view)), false).unwrap();
        let hdr = r.lit_path().unwrap().read_hdr(&ctx);
        let levels: std::collections::BTreeSet<u32> = hdr.iter().flat_map(|p| p[..3].iter().map(|v| (v * 80.0).round() as u32)).collect();
        assert!(!levels.is_empty() && levels.len() <= 9, "{view:?}: {} distinct levels", levels.len());
        // The two views of the static tables (the ones with goldens that every adapter must agree on) show structure: several levels.
        if matches!(view, AtmosphereView::TransmittanceLut | AtmosphereView::MultiScatterLut) {
            assert!(levels.len() >= 3, "{view:?}: only {} levels", levels.len());
        }
    }
}

/// A sun `elevation` radians above the local horizon at the plan's camera, towards the east.
fn sun_at(p: &TerrainPlan, elevation: f64) -> [f32; 3] {
    let pos = p.camera.position.0;
    let len = (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt();
    let up = [pos.x / len, pos.y / len, pos.z / len];
    let east = [-up[1], up[0], 0.0];
    let el = (east[0] * east[0] + east[1] * east[1]).sqrt();
    let east = [east[0] / el, east[1] / el, 0.0];
    let (c, s) = (libm::cos(elevation), libm::sin(elevation));
    [0, 1, 2].map(|k| (c * east[k] + s * up[k]) as f32)
}

// spec: ATMO-011, TEST-010
#[test]
fn sky_frames_match_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    let mut cases: Vec<(&str, usize, LitPlan)> = vec![
        ("sky_orbit_ev15", 0, atmosphere_plan(AtmosphereView::Composite)),
        ("sky_aerial20km_ev15", 3, atmosphere_plan(AtmosphereView::Composite)),
        ("sky_ground1m_ev15", 6, atmosphere_plan(AtmosphereView::Composite)),
        ("sky_orbit_transmittance_lut", 0, atmosphere_plan(AtmosphereView::TransmittanceLut)),
        ("sky_orbit_multiscatter_lut", 0, atmosphere_plan(AtmosphereView::MultiScatterLut)),
    ];
    let mut sunset = LitPlan::fixed(11.0).with_atmosphere(&LookParams::default(), R).unwrap();
    sunset.sun_dir = sun_at(&plan(6), 0.01);
    cases.push(("sky_ground1m_sunset_ev11", 6, sunset));
    for (golden, index, l) in cases {
        let p = lit(plan(index), l);
        let mut r = renderer_atmosphere(&ctx, &[&p]);
        r.render(&ctx, &p, false).unwrap();
        let img = image(&r, &ctx);
        let lit_px = img.pixels().filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8).count();
        assert!(lit_px * 10 >= img.pixels().count(), "{golden}: only {lit_px} lit pixels");
        goldens.check(golden, &img, Tolerance::Exact);
    }
    goldens.finish();
}

// spec: ATMO-002
#[test]
fn the_atmosphere_lit_terrain_is_the_albedo_times_the_lighting() {
    use planet_frame::look::shading::ShadeView;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let base = plan(3);
    let mut r = renderer_atmosphere(&ctx, &[&base]);
    let mut scene = |view: ShadeView| {
        let mut l = atmosphere_plan(AtmosphereView::Composite);
        l.view = view;
        r.render(&ctx, &lit(base.clone(), l), false).unwrap();
        r.lit_path().unwrap().atmosphere().unwrap().read_scene(&ctx)
    };
    let (lit_px, albedo, lighting) = (scene(ShadeView::Lit), scene(ShadeView::Albedo), scene(ShadeView::Lighting));
    let mut checked = 0;
    for i in 0..lit_px.len() {
        if lit_px[i][3] == 0.0 {
            continue;
        }
        for k in 0..3 {
            // The albedo view holds albedo / pi, the lighting view the light; every value is written times the pre-exposure.
            let factor = look::pre_exposure_factor(f64::from(EV));
            let want = f64::from(albedo[i][k]) * std::f64::consts::PI * f64::from(lighting[i][k]) / factor;
            let got = f64::from(lit_px[i][k]);
            assert!((got - want).abs() <= 3e-3 * want.abs() + 2e-3, "pixel {i} channel {k}: {got} vs {want}");
        }
        checked += 1;
    }
    assert!(checked > 100, "{checked} terrain pixels");
}
