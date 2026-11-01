//! Tier C: the water pass through the terrain renderer (ADR 0041 points 4 to 6 and 9, ADR 0042 point 2, ADR 0053): water on the
//! terrain's own grid on compact tiles, with and without the atmosphere; the shoreline from `W - h_morph` against a CPU oracle of
//! the depth; the synthetic lava lake through the same pass (the retrofit, ADR 0041 point 7); the coastal-flight flicker metric; nothing
//! created per frame; the frame targets equal to the compiled graph's count; goldens of the discrete shoreline views.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::water::{class_flicker_both, water_class, ClassMap, WaterClass, WaterLook, FLICKER_BUDGET};
use planet_frame::look::{self, LitPlan};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::water::{plan_water, WaterPlan, WaterRequest, WaterSource, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::global::ReadError;
use planet_generators::height::{HeightField, Reference};
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_render::graph::{lit_frame_graph, LitShape};
use planet_render::{AdapterPolicy, ExecError, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::{GoldenChecks, Image, Tolerance};
use planet_world_def::{LiquidKind, LiquidKindId, LiquidTable, WorldDefinition};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const FINEST: u8 = 26;
/// A sunny day's fixed exposure.
const EV: f32 = 14.0;
const FOV: f64 = 60.0;

fn inspect_dir() -> std::path::PathBuf {
    planet_testkit::repo_root().join("target").join("review").join("water-inspection")
}

fn lod_for(size: (u32, u32)) -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: 2.0 * f64::from(size.1).max(128.0),
        tau_px: 4.0,
        max_level: FINEST,
        max_height_m: 40_000.0,
        ..LodParams::earth_1080p()
    }
}

fn compact_pool() -> PoolConfig {
    let block_slots = 40 * 40;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn new_renderer(ctx: &GpuContext, size: (u32, u32)) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, size, CELLS, compact_pool());
    r.enable_lit(ctx);
    r
}

fn upload_all(ctx: &GpuContext, r: &mut TerrainRenderer, plan: &TerrainPlan, field: &dyn HeightField) {
    for n in &plan.nodes {
        if r.slot_of(n.tile).is_some() {
            continue;
        }
        r.upload_tile(ctx, &compact_tile_field(field, &TangentWarp, n.tile, CELLS, R)).unwrap();
    }
}

/// The sea of the tests: water wherever the reference terrain is at or below the level (a test-only producer: the sea of the
/// production bake needs a bake, and the pass reads W through the same trait).
struct HeightSea<'a> {
    field: &'a dyn HeightField,
    level: f64,
}

impl WaterSource for HeightSea<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.field.height(dir) <= self.level).then_some(WaterSample {
            level_m: self.level,
            body: WaterBodyId::SEA,
            kind: LiquidKindId::WATER,
        }))
    }
}

/// The first coast east of 30 E on 20 N whose sea lies to the west: the direction of the coast and the heading from the sea to the land.
fn coast(field: &dyn HeightField, level: f64) -> (Vec3, Vec3) {
    let at = |s: f64| {
        let lon = 30f64.to_radians() + s / (R * libm::cos(20f64.to_radians()));
        let la = 20f64.to_radians();
        Vec3::new(libm::cos(la) * libm::cos(lon), libm::cos(la) * libm::sin(lon), libm::sin(la))
    };
    let mut s = 0.0;
    let mut prev = field.height(at(s)) - level;
    loop {
        s += 50.0;
        let cur = field.height(at(s)) - level;
        if prev < 0.0 && cur >= 0.0 {
            let (mut a, mut b) = (s - 50.0, s);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if field.height(at(m)) - level < 0.0 {
                    a = m
                } else {
                    b = m
                }
            }
            return (at(b), (at(b + 10.0) - at(b - 10.0)).normalized());
        }
        prev = cur;
        assert!(s < 4.0e6, "no coast found");
    }
}

/// A camera `alt` m above the water `back` m out to sea from the coast, looking along the heading to the land pitched down.
#[allow(clippy::too_many_arguments)]
fn coast_camera(c: Vec3, heading: Vec3, level: f64, alt: f64, back: f64, along: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let pos_dir = (c + heading * ((along - back) / R)).normalized();
    Camera::at_surface_point(PlanetFixed(pos_dir * (R + level + alt)), heading, pitch_deg.to_radians(), FOV.to_radians(), aspect, 0.1)
}

struct Scene<'a> {
    camera: Camera,
    size: (u32, u32),
    field: &'a dyn HeightField,
}

impl Scene<'_> {
    fn terrain(&self) -> TerrainPlan {
        // The plans of the lit path bound their tiles up to the sea level (`WaterBounded`): the water above a culled seabed is drawn.
        let field = planet_generators::water::WaterBounded::borrowed(self.field, 0.0);
        let field: &dyn HeightField = &field;
        plan_terrain_field(
            &TerrainRequest {
                camera: self.camera,
                size: self.size,
                view: TerrainView::Level,
                lod: lod_for(self.size),
                face_mapping: "tangent-v1".into(),
                seed: self.field.seed(),
            },
            field,
        )
        .unwrap()
    }
}

fn water_plan(plan: &TerrainPlan, source: &dyn WaterSource, table: &LiquidTable, size: (u32, u32), t: f64, view: WaterView) -> WaterPlan {
    let look = LookParams::default();
    plan_water(
        &WaterRequest { source, map: &TangentWarp, liquids: table, look: &look.water, lod: &lod_for(size), time_s: t, view, cut: None },
        &mut WaterTileCache::new(),
        plan,
    )
    .unwrap()
}

fn lit_plan(atmosphere: bool) -> LitPlan {
    let mut l = LitPlan::fixed(EV);
    if atmosphere {
        l = l.with_atmosphere(&LookParams::default(), R).unwrap();
    }
    l
}

/// Render `plan` lit (with `water`, or without) and read the HDR target back.
fn frame(ctx: &GpuContext, r: &mut TerrainRenderer, plan: &TerrainPlan, water: Option<&WaterPlan>, atmosphere: bool) -> Vec<[f32; 4]> {
    let mut p = plan.clone();
    p.lit = Some(lit_plan(atmosphere));
    p.water = water.cloned();
    r.render(ctx, &p, false).unwrap();
    r.lit_path().unwrap().read_hdr(ctx)
}

fn image_of(r: &TerrainRenderer, ctx: &GpuContext) -> Image {
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

fn factor() -> f64 {
    look::pre_exposure_factor_with(&look::SceneUnits::EARTH, f64::from(EV))
}

/// The class of a pixel of the class view by its palette colour (None when the colour is no palette entry).
fn decode(p: &[f32; 4]) -> Option<WaterClass> {
    let scale = core::f64::consts::FRAC_1_PI * factor();
    let c = [f64::from(p[0]) / scale, f64::from(p[1]) / scale, f64::from(p[2]) / scale];
    [WaterClass::Film, WaterClass::Foam, WaterClass::Shallow, WaterClass::Mid, WaterClass::Deep].into_iter().find(|k| {
        let w = WaterClass::COLOURS[*k as usize];
        (0..3).all(|i| (c[i] - w[i]).abs() < 0.02)
    })
}

/// The class map of a frame: the pixels the class view changed against the frame without water, decoded.
fn class_map(base: &[[f32; 4]], with: &[[f32; 4]], size: (u32, u32)) -> ClassMap {
    let mut c = vec![0u8; (size.0 * size.1) as usize];
    for (i, (a, b)) in base.iter().zip(with).enumerate() {
        if a != b {
            c[i] = decode(b).unwrap_or_else(|| panic!("pixel {i} of the class view is no palette colour: {b:?}")) as u8;
        }
    }
    ClassMap::new(size, c)
}

fn write_inspection(name: &str, img: &Image) {
    let dir = inspect_dir();
    std::fs::create_dir_all(&dir).unwrap();
    planet_testkit::write_png(&dir.join(format!("{name}.png")), img).unwrap();
}

/// The vertical depth of the water at the point of the water sphere a pixel's ray meets (the CPU oracle of the class view).
fn cpu_depth(cam: &Camera, size: (u32, u32), x: u32, y: u32, field: &dyn HeightField, level: f64) -> Option<f64> {
    let (right, up, fwd) = cam.ray_frame();
    let (u, v) = (2.0 * (f64::from(x) + 0.5) / f64::from(size.0) - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(size.1));
    let dir = (fwd + right * u + up * v).normalized();
    let from = cam.position.0;
    let b = from.dot(dir);
    let l = from.length();
    let radius = R + level;
    let c = (l - radius) * (l + radius);
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = if c >= 0.0 { -b - disc.sqrt() } else { -b + disc.sqrt() };
    if t <= 0.0 {
        return None;
    }
    let p = from + dir * t;
    Some(level - field.height(p.normalized()))
}

fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

// spec: REND-016, ATMO-004
#[test]
fn the_lit_water_and_atmosphere_frames_over_compact_blocks_are_the_recorded_pixels() {
    // The hash of the HDR and colour targets of five frames of the aerial coast (no water, the atmosphere, the class view, the lit
    // water without and with the atmosphere), as the atlas textures drew them (recorded by the differential of ADR 0054's change,
    // byte-identical to the buffer blocks on WARP and on the RTX; re-recorded by the pixel-footprint fade of the waves, ADR 0056). A driver not in the table prints its hash and records nothing.
    const KNOWN: [(&str, u64); 2] = [
        ("Microsoft Basic Render Driver | Dx12 | Cpu | driver '10.0.26100.9711' ''", 0x089f_3f75_fa7c_fd77),
        ("NVIDIA GeForce RTX 3080 Ti Laptop GPU | Vulkan | DiscreteGpu | driver 'NVIDIA' '591.86'", 0xa678_7a98_e2dc_b4a3),
    ];
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 120.0, 800.0, 0.0, 12.0, 1.6);
    let plan = Scene { camera, size, field: &field }.terrain();
    let lit_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Lit);
    let class_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Class);
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
    r.enable_water(&ctx);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (water, atmosphere) in
        [(None, false), (None, true), (Some(&class_water), false), (Some(&lit_water), false), (Some(&lit_water), true)]
    {
        for p in frame(&ctx, &mut r, &plan, water, atmosphere) {
            for v in p {
                hash = fnv(hash, &v.to_bits().to_le_bytes());
            }
        }
        hash = fnv(hash, &r.read_color(&ctx).rgba);
    }
    let driver = ctx.driver_string();
    match KNOWN.iter().find(|(d, _)| *d == driver) {
        Some((_, want)) => assert_eq!(hash, *want, "the lit water frames of the buffer path changed on {driver}"),
        None => println!("KNOWN-ANSWER water: no hash recorded for '{driver}'; got {hash:016x}"),
    }
}

// spec: ATMO-016
#[test]
fn a_water_plan_needs_the_water_pass_and_names_the_missing_call_and_a_bad_node_is_refused() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (96, 64);
    let (c, heading) = coast(&field, 0.0);
    let scene = Scene { camera: coast_camera(c, heading, 0.0, 200.0, 800.0, 0.0, 15.0, 1.5), size, field: &field };
    let plan = scene.terrain();
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let wp = water_plan(&plan, &src, &table, size, 1.0, WaterView::Lit);
    assert!(!wp.nodes.is_empty());
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    let mut p = plan.clone();
    p.lit = Some(lit_plan(false));
    p.water = Some(wp.clone());
    let e = r.render(&ctx, &p, false).unwrap_err();
    assert!(matches!(e, ExecError::WaterNotEnabled) && e.to_string().contains("enable_water"), "{e}");
    r.enable_water(&ctx);
    r.render(&ctx, &p, false).unwrap();
    // A water part without a lit part, and a node the plan does not have.
    let mut debug = plan.clone();
    debug.water = Some(wp.clone());
    assert!(matches!(r.render(&ctx, &debug, false).unwrap_err(), ExecError::BadWaterPlan(m) if m.contains("lit")));
    let mut bad = p.clone();
    bad.water.as_mut().unwrap().nodes[0].node = 99_999;
    assert!(matches!(r.render(&ctx, &bad, false).unwrap_err(), ExecError::BadWaterPlan(m) if m.contains("99999")));
    let mut bad = p;
    bad.water.as_mut().unwrap().nodes[0].phases[0] = 9.0;
    assert!(matches!(r.render(&ctx, &bad, false).unwrap_err(), ExecError::BadWaterPlan(m) if m.contains("phase")));
}

// spec: ATMO-004, ATMO-015
#[test]
fn the_class_view_follows_the_depth_of_the_terrain_under_the_water() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    // The aerial coast (2 km up, 4 km out) and the low coast (3 m up) at their own bands.
    let cases = [("aerial", 2000.0, 4000.0, 25.0), ("low", 3.0, 300.0, 4.0)];
    let mut maps: Vec<(String, ClassMap)> = Vec::new();
    for (name, alt, back, pitch) in cases {
        let camera = coast_camera(c, heading, 0.0, alt, back, 0.0, pitch, 1.6);
        let scene = Scene { camera, size, field: &field };
        let plan = scene.terrain();
        let wp = water_plan(&plan, &src, &table, size, 2.0, WaterView::Class);
        let mut r = new_renderer(&ctx, size);
        upload_all(&ctx, &mut r, &plan, &field);
        r.enable_water(&ctx);
        let base = frame(&ctx, &mut r, &plan, None, false);
        let with = frame(&ctx, &mut r, &plan, Some(&wp), false);
        // The water pass changes no pixel the terrain covers that is not water, and never adds coverage.
        assert!(base.iter().zip(&with).all(|(a, b)| (a[3] > 0.0) == (b[3] > 0.0)), "{name}: the water adds no coverage");
        let map = class_map(&base, &with, size);
        // The oracle: the class of every water pixel is the class of the CPU's depth at the point its ray meets the water, up to the
        // mesh's own interpolation (the vertex heights are exact, the surface between them is linear) near the band edges.
        let water: usize = map.class.iter().filter(|c| **c != 0).count();
        assert!(water > 1000, "{name}: {water} water pixels");
        let mut wrong = 0;
        let mut checked = 0;
        for y in 0..size.1 {
            for x in 0..size.0 {
                let got = map.class[(y * size.0 + x) as usize];
                let Some(d) = cpu_depth(&camera, size, x, y, &field, 0.0) else { continue };
                if got == 0 {
                    // Land or sky to the renderer: the oracle must agree unless the point is right at the waterline.
                    if d > 0.02 && d < 1.0e6 {
                        // A pixel the water pass leaves: the terrain is above the sphere there (the ray meets terrain first) or
                        // the pixel is sky. Only count a depth the oracle says is deep.
                        if d > 5.0 {
                            wrong += 1;
                        }
                    }
                    continue;
                }
                checked += 1;
                let tol = 0.15 + 0.03 * d.abs();
                let lo = water_class(d - tol, &WaterLook::EARTH) as u8;
                let hi = water_class(d + tol, &WaterLook::EARTH) as u8;
                if !(lo.min(hi)..=lo.max(hi)).contains(&got) {
                    wrong += 1;
                }
            }
        }
        assert!(checked > 1000, "{name}: {checked} water pixels checked");
        assert!(wrong * 100 <= checked, "{name}: {wrong} of {checked} pixels have a class the CPU's depth does not give");
        // (The comparison of the two tile formats' shorelines went with the vertex format: the CPU depth oracle above is the reference.)
        println!("{name}: {water} water pixels, {wrong} of {checked} off the oracle");
        // Along the middle column the classes only grow with the distance from the shore (monotone out to sea).
        let col = size.0 / 2;
        let mut last = 0u8;
        let mut order_breaks = 0;
        for y in (0..size.1).rev() {
            let k = map.class[(y * size.0 + col) as usize];
            if k == 0 {
                continue;
            }
            if k + 1 < last || k > last + 2 {
                order_breaks += 1;
            }
            last = k;
        }
        assert!(order_breaks <= 3, "{name}: the classes along the middle column are not ordered: {order_breaks} breaks");
        maps.push((name.to_string(), map));
    }
    assert!(maps.iter().any(|(_, m)| m.class.contains(&(WaterClass::Foam as u8))), "a foam band shows on the coast");
}

// spec: ATMO-004, ATMO-015
#[test]
fn the_lit_water_covers_what_the_class_view_covers_is_finite_and_the_atmosphere_hazes_it() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 120.0, 800.0, 0.0, 12.0, 1.6);
    let scene = Scene { camera, size, field: &field };
    let plan = scene.terrain();
    let lit_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Lit);
    let class_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Class);
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
    r.enable_water(&ctx);
    let mut luminance = [0.0f64; 2];
    for (k, atmosphere) in [false, true].into_iter().enumerate() {
        let base = frame(&ctx, &mut r, &plan, None, atmosphere);
        let class = frame(&ctx, &mut r, &plan, Some(&class_water), false);
        let plain_base = frame(&ctx, &mut r, &plan, None, false);
        let water_mask: Vec<bool> = plain_base.iter().zip(&class).map(|(a, b)| a != b).collect();
        let lit = frame(&ctx, &mut r, &plan, Some(&lit_water), atmosphere);
        let n = water_mask.iter().filter(|m| **m).count();
        assert!(n > 2000, "{n} water pixels");
        assert!(lit.iter().all(|p| p.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= look::HDR_CLAMP)), "finite and clamped");
        // Only water pixels change.
        for (i, (a, b)) in base.iter().zip(&lit).enumerate() {
            if !water_mask[i] {
                assert_eq!(a, b, "pixel {i} is not water and must not change");
            }
        }
        let mean: f64 = lit
            .iter()
            .zip(&water_mask)
            .filter(|(_, m)| **m)
            .map(|(p, _)| look::luminance([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]))
            .sum::<f64>()
            / n as f64;
        luminance[k] = mean;
        assert!(mean > 0.0, "the water is lit");
        if atmosphere {
            write_inspection("coast_aerial_lit_atmosphere", &image_of(&r, &ctx));
        }
    }
    assert!((luminance[0] - luminance[1]).abs() > 1e-3 * luminance[0].max(1e-9), "the atmosphere changes the water: {luminance:?}");
}

/// The reference terrain with its relief scaled up: the shelf and the coast of the reference field are a gradient of 2.5 m per km, far
/// flatter than a beach (1:20 to 1:50), so a flight over them has bands of centimetres of depth spread over kilometres. Scaled by `k`
/// the shore is a beach.
struct Steepened {
    inner: Reference,
    k: f64,
}

impl HeightField for Steepened {
    fn height(&self, dir: Vec3) -> f64 {
        self.k * self.inner.height(dir)
    }

    fn seed(&self) -> u64 {
        self.inner.seed()
    }

    fn implementation_id(&self) -> &'static str {
        "test-steepened-v1"
    }

    fn version(&self) -> u32 {
        1
    }

    fn max_abs_height_m(&self) -> f64 {
        self.k * self.inner.max_abs_height_m()
    }

    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.inner.height_bounds(centre, angular_radius);
        (self.k * lo, self.k * hi)
    }
}

/// A basin in the highland: the reference terrain with a bowl cut into it (a test-only height field), and the lake that fills it.
struct BasinField {
    inner: Reference,
    centre: Vec3,
    radius_rad: f64,
    depth_m: f64,
}

impl HeightField for BasinField {
    fn height(&self, dir: Vec3) -> f64 {
        let theta = libm::acos(dir.dot(self.centre).clamp(-1.0, 1.0));
        let x = theta / self.radius_rad;
        let bowl = if x < 1.0 { (1.0 - x * x) * (1.0 - x * x) } else { 0.0 };
        self.inner.height(dir) - self.depth_m * bowl
    }

    fn seed(&self) -> u64 {
        self.inner.seed()
    }

    fn implementation_id(&self) -> &'static str {
        "test-basin-v1"
    }

    fn version(&self) -> u32 {
        1
    }

    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m() + self.depth_m
    }

    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let (lo, hi) = self.inner.height_bounds(centre, angular_radius);
        let near = libm::acos(centre.dot(self.centre).clamp(-1.0, 1.0)) < angular_radius + self.radius_rad;
        (if near { lo - self.depth_m } else { lo }, hi)
    }
}

/// The retrofit's second producer (ADR 0041 point 7): a lake of its own level, body and kind in a basin, before the sea.
struct Lake<'a> {
    field: &'a BasinField,
    cos_radius: f64,
    level: f64,
    sea: HeightSea<'a>,
}

impl WaterSource for Lake<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        let inside = dir.dot(self.field.centre) >= self.cos_radius;
        if inside && self.field.height(dir) <= self.level {
            return Ok(Some(WaterSample { level_m: self.level, body: WaterBodyId(2), kind: LiquidKindId(1) }));
        }
        self.sea.water_at(dir)
    }
}

fn lava_table() -> LiquidTable {
    let mut def = WorldDefinition::single_planet(R, 1);
    def.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    def.set_liquid(
        LiquidKindId(1),
        &LiquidKind {
            colour: [0.8, 0.1, 0.02],
            emission: [1.2, 0.25, 0.02],
            viscosity_pa_s: 1.0e3,
            wave_amplitude_m: 0.0,
            wave_length_m: 0.0,
        },
    )
    .unwrap();
    def.liquids().unwrap()
}

/// A highland point (a fixed search over the reference field) for the basin.
fn highland() -> Vec3 {
    let f = Reference(1);
    for k in 0..4000 {
        let a = f64::from(k) * 0.0137;
        let lat = 0.6 * libm::sin(a * 0.7);
        let lon = 0.5 + a;
        let d = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
        if (400.0..700.0).contains(&f.height(d)) {
            return d;
        }
    }
    panic!("no highland");
}

// spec: ATMO-018
#[test]
fn a_lava_lake_with_its_own_level_and_emission_goes_through_the_same_water_pass_as_the_sea() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let centre = highland();
    let inner = Reference(1);
    let rim = inner.height(centre);
    let field = BasinField { inner, centre, radius_rad: 0.004, depth_m: 380.0 };
    let level = rim - 220.0;
    let src = Lake { field: &field, cos_radius: libm::cos(0.0045), level, sea: HeightSea { field: &field, level: 0.0 } };
    let table = lava_table();
    let size = (256, 160);
    // 40 km above the lake looking down at it: the lake and the land around it.
    let pos = PlanetFixed(centre * (R + rim + 40_000.0));
    let north = Vec3::new(0.0, 0.0, 1.0);
    let camera = Camera::at_surface_point(pos, north, 80f64.to_radians(), FOV.to_radians(), 1.6, 0.1);
    let scene = Scene { camera, size, field: &field };
    let plan = scene.terrain();
    let lit_water = water_plan(&plan, &src, &table, size, 5.0, WaterView::Lit);
    // The plan holds two liquids (the lake's lava and nothing of the sea here): the lake is liquid 1 at its own level.
    let lake_nodes: Vec<_> = lit_water.nodes.iter().filter(|n| f64::from(n.level_m) == f64::from(level as f32)).collect();
    assert!(!lake_nodes.is_empty(), "the lake's tiles carry the lake's level {level}");
    let lava_slot = lit_water.liquids.iter().position(|l| l.kind == 1).expect("the lava is in the plan");
    assert!(lake_nodes.iter().all(|n| usize::from(n.liquid) == lava_slot));
    assert!(!lit_water.liquids[lava_slot].has_waves(), "lava has no waves");
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    r.enable_water(&ctx);
    let (objects, built) = (r.lit_objects(), r.init_counts());
    let base = frame(&ctx, &mut r, &plan, None, false);
    let lit = frame(&ctx, &mut r, &plan, Some(&lit_water), false);
    // The same pipelines draw the lake: nothing new was made for a second liquid.
    assert_eq!((r.lit_objects(), r.init_counts()), (objects, built));
    let changed: Vec<usize> = base.iter().zip(&lit).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
    assert!(changed.len() > 800, "the lake covers {} pixels", changed.len());
    // Emission: the lava is far brighter than the sunlit terrain around it, and red.
    let lum = |p: &[f32; 4]| look::luminance([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
    let lake_mean = changed.iter().map(|&i| lum(&lit[i])).sum::<f64>() / changed.len() as f64;
    let land: Vec<usize> = (0..lit.len()).filter(|i| lit[*i][3] > 0.0 && base[*i] == lit[*i]).collect();
    let land_mean = land.iter().map(|&i| lum(&lit[i])).sum::<f64>() / land.len() as f64;
    assert!(lake_mean > 5.0 * land_mean, "the lava glows: {lake_mean} against the land's {land_mean}");
    let centre_px = changed[changed.len() / 2];
    assert!(lit[centre_px][0] > lit[centre_px][1] && lit[centre_px][1] > lit[centre_px][2], "the lava is red: {:?}", lit[centre_px]);
    // No waves: the wave view is one level on the whole lake, and the water view of the class bands shows its depth.
    let mut waves = water_plan(&plan, &src, &table, size, 5.0, WaterView::Waves);
    let wave_frame = frame(&ctx, &mut r, &plan, Some(&waves), false);
    let levels: std::collections::BTreeSet<u32> = changed.iter().map(|&i| (wave_frame[i][0] * 1000.0).round() as u32).collect();
    assert_eq!(levels.len(), 1, "{levels:?}");
    waves.view = WaterView::Class;
    let class_frame = frame(&ctx, &mut r, &plan, Some(&waves), false);
    let map = class_map(&base, &class_frame, size);
    let classes: std::collections::BTreeSet<u8> = map.class.iter().copied().filter(|c| *c != 0).collect();
    assert!(classes.len() >= 3, "the basin has a shoreline with bands: {classes:?}");
    write_inspection("lava_lake_lit", &{
        let _ = frame(&ctx, &mut r, &plan, Some(&lit_water), false);
        image_of(&r, &ctx)
    });
}

// spec: ATMO-013
#[test]
fn the_waves_move_with_the_clock_and_only_on_water_and_stand_still_for_the_same_time() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (192, 120);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 30.0, 3500.0, 0.0, 8.0, 1.6);
    let scene = Scene { camera, size, field: &field };
    let plan = scene.terrain();
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    r.enable_water(&ctx);
    let base = frame(&ctx, &mut r, &plan, None, false);
    let mut at = |t: f64, view: WaterView| frame(&ctx, &mut r, &plan, Some(&water_plan(&plan, &src, &table, size, t, view)), false);
    let (a, b, a2) = (at(10.0, WaterView::Lit), at(11.3, WaterView::Lit), at(10.0, WaterView::Lit));
    assert_eq!(a, a2, "the same time draws the same frame");
    let diff = |x: &[[f32; 4]], y: &[[f32; 4]]| x.iter().zip(y).filter(|(p, q)| p != q).count();
    let water_pixels = diff(&base, &a);
    assert!(water_pixels > 2000, "{water_pixels}");
    assert!(diff(&a, &b) > 200, "the waves moved: {} pixels differ", diff(&a, &b));
    // Terrain pixels never change with the clock.
    for (i, p) in base.iter().enumerate() {
        if a[i] == *p {
            assert_eq!(b[i], *p, "pixel {i} is terrain and the clock must not move it");
        }
    }
    // The wave view has several levels at one time.
    let waves = at(10.0, WaterView::Waves);
    let levels: std::collections::BTreeSet<u32> =
        waves.iter().zip(&base).filter(|(w, b)| w != b).map(|(w, _)| (w[0] * 1000.0).round() as u32).collect();
    assert!(levels.len() >= 3, "the swell shows in {} levels", levels.len());
}

// spec: ATMO-016
#[test]
fn nothing_is_created_per_frame_after_enable_water_and_the_frame_targets_equal_the_graphs_count() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (128, 80);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 200.0, 1500.0, 0.0, 20.0, 1.6);
    let scene = Scene { camera, size, field: &field };
    let plan = scene.terrain();
    let wp = water_plan(&plan, &src, &table, size, 3.0, WaterView::Lit);
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    let before = r.lit_frame_target_bytes().unwrap();
    r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
    r.enable_water(&ctx);
    let held = r.lit_frame_target_bytes().unwrap();
    assert!(held > before);
    let with = lit_frame_graph(&LitShape { size, adaptive: true, atmosphere: true, water: true }).compile().unwrap();
    assert_eq!(held, with.frame_target_bytes(), "what the renderer allocates is what the compiled graph counts");
    // The first frame sizes the per-frame ring for the plan (demand growth counts); from there nothing is made.
    let mut first = plan.clone();
    first.lit = Some(lit_plan(true));
    first.water = Some(wp.clone());
    r.render(&ctx, &first, false).unwrap();
    let (built, objects) = (r.init_counts(), r.lit_objects());
    for k in 0..4 {
        let mut p = plan.clone();
        p.lit = Some(lit_plan(true));
        let _ = k;
        p.water = Some(wp.clone());
        let (_, stats) = r.render(&ctx, &p, false).unwrap();
        assert_eq!(stats.water_draws as usize, wp.nodes.len());
        assert_eq!((r.lit_objects(), r.init_counts()), (objects, built), "frame {k}");
    }
    let path = r.lit_path().unwrap();
    assert_eq!(path.graph_compiles(), 1, "the compile happens when the shape changes, and the shape did not");
    assert_eq!(
        path.executed_passes(),
        [
            "atmosphere-luts",
            "sky-view",
            "aerial-perspective",
            "terrain",
            "scene-copy",
            "water",
            "sky-composite",
            "exposure-adapt",
            "tonemap"
        ]
    );
}

/// One frame of a flight: the class map of the class view (the pixels the water pass drew, by class), the camera and the set of
/// tiles the frame drew.
struct FlightFrame {
    camera: Camera,
    classes: ClassMap,
    tiles: Vec<TileId>,
    nodes: usize,
    /// The same frame without water (the terrain alone).
    base_lum: Vec<f64>,
    /// The lit frame as displayed (for the inspection strips).
    shown: Image,
    /// What the water does to the terrain's luminance: lit over terrain alone (the shore pixels' own contribution, free of the
    /// seabed's shading).
    ratio: Vec<f64>,
}

/// Fly `cameras` over the coast: each frame is planned, its tiles uploaded, its water planned at the clock `100 + k dt` (the waves move
/// all the while) and drawn in the class view and without water.
fn fly(
    ctx: &GpuContext,
    field: &dyn HeightField,
    size: (u32, u32),
    cameras: &[Camera],
    dt: f64,
    waves: bool,
    shore_filter: f64,
) -> Vec<FlightFrame> {
    let table = LiquidTable::earth();
    let src = HeightSea { field, level: 0.0 };
    let mut look = LookParams::default();
    // The metric excludes the intended wave motion by measuring on a calm sea (the look's wave scale 0): with waves the surface a
    // pixel sees at a grazing angle is a displaced one, which moves with the clock.
    if !waves {
        look.water.wave_scale = 0.0;
    }
    look.water.shore_filter = shore_filter;
    let mut cache = WaterTileCache::new();
    let mut r = new_renderer(ctx, size);
    r.enable_water(ctx);
    let mut frames = Vec::new();
    for (k, camera) in cameras.iter().enumerate() {
        let scene = Scene { camera: *camera, size, field };
        let plan = scene.terrain();
        upload_all(ctx, &mut r, &plan, field);
        let wp = plan_water(
            &WaterRequest {
                source: &src,
                map: &TangentWarp,
                liquids: &table,
                look: &look.water,
                lod: &lod_for(size),
                time_s: 100.0 + dt * k as f64,
                view: WaterView::Class,
                cut: None,
            },
            &mut cache,
            &plan,
        )
        .unwrap();
        let base = frame(ctx, &mut r, &plan, None, false);
        let with = frame(ctx, &mut r, &plan, Some(&wp), false);
        let mut lit_wp = wp.clone();
        lit_wp.view = WaterView::Lit;
        let lit = frame(ctx, &mut r, &plan, Some(&lit_wp), false);
        let lum: Vec<f64> = lit.iter().map(|p| look::luminance([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])).collect();
        let base_lum: Vec<f64> = base.iter().map(|p| look::luminance([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])])).collect();
        frames.push(FlightFrame {
            shown: image_of(&r, ctx),
            ratio: lum.iter().zip(&base_lum).map(|(l, b): (&f64, &f64)| l / b.max(1.0e-3)).collect(),
            base_lum,
            camera: *camera,
            classes: class_map(&base, &with, size),
            tiles: plan.nodes.iter().map(|n| n.tile).collect(),
            nodes: plan.nodes.len(),
        });
    }
    frames
}

fn lit_shore_of(frames: &[FlightFrame], field: &dyn HeightField) -> planet_frame::look::water::LitShoreReport {
    lit_shore_with(frames, field, |f| &f.ratio)
}

fn lit_shore_with(
    frames: &[FlightFrame],
    field: &dyn HeightField,
    lum: impl Fn(&FlightFrame) -> &Vec<f64>,
) -> planet_frame::look::water::LitShoreReport {
    let mut total = planet_frame::look::water::LitShoreReport::default();
    let inside = |x: Vec3| x.length() - R < field.height(x.normalized());
    for w in frames.windows(3) {
        total.add(planet_frame::look::water::lit_shore_flash(
            &w[1].classes,
            lum(&w[1]),
            &w[1].camera,
            (lum(&w[0]), w[0].classes.size, &w[0].camera),
            (lum(&w[2]), w[2].classes.size, &w[2].camera),
            R,
            planet_frame::look::water::LIT_SHORE_THRESHOLD,
            Some(&inside),
        ));
    }
    total
}

fn flicker_of(frames: &[FlightFrame], field: &dyn HeightField) -> planet_frame::look::water::FlickerReport {
    let mut total = planet_frame::look::water::FlickerReport::default();
    let inside = |x: Vec3| x.length() - R < field.height(x.normalized());
    for w in frames.windows(2) {
        total.add(class_flicker_both(&w[0].classes, &w[0].camera, &w[1].classes, &w[1].camera, R, Some(&inside)));
    }
    total
}

// spec: ATMO-017, ATMO-004, REND-030
#[test]
fn a_coastal_flight_at_low_altitude_flickers_within_the_budget_while_the_lod_changes_under_it() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // A beach, not the reference's gradient of 2.5 m per km: the relief of the reference field scaled by 8 (1:50 at the coast).
    let field = Steepened { inner: Reference(1), k: 8.0 };
    let size = (192, 120);
    let (c, heading) = coast(&field, 0.0);
    let aspect = f64::from(size.0) / f64::from(size.1);
    // Flight A: a low glide towards the coast, 6 m over the water, 12 m a frame from 400 m out (the foam band and the film approach).
    let glide: Vec<Camera> = (0..20).map(|k| coast_camera(c, heading, 0.0, 6.0, 400.0, 12.0 * f64::from(k), 6.0, aspect)).collect();
    let a = fly(&ctx, &field, size, &glide, 0.25, false, 1.0);
    let a_unfiltered = fly(&ctx, &field, size, &glide, 0.25, false, 0.0);
    // Flight B: a descent from 300 m to 4 m (log-spaced) towards the coast: every distance band of the LOD passes under the camera.
    let descent: Vec<Camera> = (0..24)
        .map(|k| {
            let t = f64::from(k) / 23.0;
            coast_camera(c, heading, 0.0, 300.0 * libm::pow(4.0 / 300.0, t), 1200.0 - 700.0 * t, 0.0, 10.0, aspect)
        })
        .collect();
    let b = fly(&ctx, &field, size, &descent, 0.25, false, 1.0);
    let b_unfiltered = fly(&ctx, &field, size, &descent, 0.25, false, 0.0);
    for (name, frames) in [("glide", &a), ("descent", &b)] {
        // Frame strips for the gate's contact sheet (ADR 0037 point 2): every fourth frame of the flight, side by side, drawn with the
        // fwidth filter on (the look's default) and off, so the owner sees both.
        let unfiltered_frames = if name == "glide" { &a_unfiltered } else { &b_unfiltered };
        for (tag, strip_frames) in [("filter1", frames), ("filter0", unfiltered_frames)] {
            let picks: Vec<&FlightFrame> = strip_frames.iter().step_by(4).collect();
            let (w, h) = (picks[0].shown.width, picks[0].shown.height);
            let mut strip = Image::filled(w * picks.len() as u32, h, [0, 0, 0, 255]);
            for (i, f) in picks.iter().enumerate() {
                for y in 0..h {
                    for x in 0..w {
                        strip.set_pixel(i as u32 * w + x, y, f.shown.pixel(x, y));
                    }
                }
            }
            write_inspection(&format!("flight_{name}_{tag}_strip"), &strip);
        }
        for k in [5usize, 6] {
            let m = &frames[k].classes;
            let mut img = Image::filled(m.size.0, m.size.1, [0, 0, 0, 255]);
            for y in 0..m.size.1 {
                for x in 0..m.size.0 {
                    let c = WaterClass::COLOURS[usize::from(m.class[(y * m.size.0 + x) as usize])];
                    img.set_pixel(x, y, [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255]);
                }
            }
            write_inspection(&format!("flight_{name}_{k}"), &img);
        }
        let report = flicker_of(frames, &field);
        let unfiltered = if name == "glide" { &a_unfiltered } else { &b_unfiltered };
        let (lit, lit_before) = (lit_shore_of(frames, &field), lit_shore_of(unfiltered, &field));
        println!(
            "{name}: lit shore flash of the water over the terrain > {:.0} %: {:.3} % of {} shore pixels with the fwidth filter, {:.3} % of {} without (the terrain's own shading flashes {:.3} % of them); exact-class share {:.3} % with, {:.3} % without",
            planet_frame::look::water::LIT_SHORE_THRESHOLD * 100.0,
            lit.share() * 100.0,
            lit.covered,
            lit_before.share() * 100.0,
            lit_before.covered,
            lit_shore_with(frames, &field, |f| &f.base_lum).share() * 100.0,
            report.changed_fine as f64 / report.covered.max(1) as f64 * 100.0,
            flicker_of(unfiltered, &field).changed_fine as f64 / flicker_of(unfiltered, &field).covered.max(1) as f64 * 100.0
        );
        // The terrain's own shading at the shore does not flash (REND-030): with the seabed-to-shore ramp as tall as the mesh's tolerance,
        // at most 0.5 % of the shore-band pixels change their terrain-only luminance by more than a quarter between three frames
        // (6.3 % on the glide and 36.8 % on the descent with the look's fixed 1 m ramp).
        let own = lit_shore_with(frames, &field, |f| &f.base_lum);
        assert!(own.covered > 1000, "{name}: {own:?}");
        assert!(
            own.share() <= 0.005,
            "{name}: the terrain's own shading flashes {:.3} % of {} shore pixels",
            own.share() * 100.0,
            own.covered
        );
        // The tiles of the frames change along the flight (the LOD changes under the shoreline).
        let sets: std::collections::BTreeSet<Vec<TileId>> = frames.iter().map(|f| f.tiles.clone()).collect();
        let nodes: Vec<usize> = frames.iter().map(|f| f.nodes).collect();
        assert!(sets.len() >= frames.len() / 2, "{name}: the plans differ along the flight ({} distinct of {})", sets.len(), frames.len());
        assert!(report.covered > 20_000, "{name}: {report:?}");
        println!(
            "{name}: flicker {:.4} % ({} of {} covered water pixels changed class between consecutive frames, {} left the frame); nodes per frame {nodes:?}",
            report.share() * 100.0,
            report.changed,
            report.covered,
            report.left_frame
        );
        assert!(
            report.share() <= FLICKER_BUDGET,
            "{name}: {:.4} % of the water pixels change class between frames, budget {:.2} %: {report:?}",
            report.share() * 100.0,
            FLICKER_BUDGET * 100.0
        );
    }
}

// spec: ATMO-004, TEST-010
#[test]
fn water_class_views_match_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let aspect = f64::from(size.0) / f64::from(size.1);
    let orbit = planet_frame::scripted_views(R, aspect, 1)[0].1;
    let cases = [
        ("water_class_coast_aerial2km", coast_camera(c, heading, 0.0, 2000.0, 4000.0, 0.0, 25.0, aspect)),
        ("water_class_coast_low3m", coast_camera(c, heading, 0.0, 3.0, 300.0, 0.0, 4.0, aspect)),
        ("water_class_orbit", orbit),
    ];
    for (name, camera) in cases {
        let scene = Scene { camera, size, field: &field };
        let plan = scene.terrain();
        let wp = water_plan(&plan, &src, &table, size, 0.0, WaterView::Class);
        let mut r = new_renderer(&ctx, size);
        upload_all(&ctx, &mut r, &plan, &field);
        r.enable_water(&ctx);
        let base = frame(&ctx, &mut r, &plan, None, false);
        let with = frame(&ctx, &mut r, &plan, Some(&wp), false);
        let map = class_map(&base, &with, size);
        let water = map.class.iter().filter(|c| **c != 0).count();
        // The test's own oracle for the candidate: water is there and every water pixel is a palette colour.
        assert!(water > 1000, "{name}: only {water} water pixels");
        let img = image_of(&r, &ctx);
        goldens.check(name, &img, Tolerance::Exact);
    }
    // The lava lake's class view: the basin's shoreline in its own level.
    let centre = highland();
    let inner = Reference(1);
    let rim = inner.height(centre);
    let basin = BasinField { inner, centre, radius_rad: 0.004, depth_m: 380.0 };
    let level = rim - 220.0;
    let lake = Lake { field: &basin, cos_radius: libm::cos(0.0045), level, sea: HeightSea { field: &basin, level: 0.0 } };
    let lava = lava_table();
    let camera = Camera::at_surface_point(
        PlanetFixed(centre * (R + rim + 40_000.0)),
        Vec3::new(0.0, 0.0, 1.0),
        80f64.to_radians(),
        FOV.to_radians(),
        aspect,
        0.1,
    );
    let scene = Scene { camera, size, field: &basin };
    let plan = scene.terrain();
    let wp = water_plan(&plan, &lake, &lava, size, 0.0, WaterView::Class);
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &basin);
    r.enable_water(&ctx);
    let base = frame(&ctx, &mut r, &plan, None, false);
    let with = frame(&ctx, &mut r, &plan, Some(&wp), false);
    assert!(class_map(&base, &with, size).class.iter().filter(|c| **c != 0).count() > 800);
    goldens.check("water_class_lava_lake", &image_of(&r, &ctx), Tolerance::Exact);
    goldens.finish();
}

// spec: ATMO-004
#[test]
fn the_lit_water_views_are_written_for_inspection() {
    // Continuous GPU shading differs across adapters by a few pixels at the glint and the foam edge: these views are for the eye and
    // the gate's contact sheet (ADR 0037 point 2), not goldens. The test writes them and checks only that they show water.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (320, 200);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let aspect = f64::from(size.0) / f64::from(size.1);
    let views = [
        ("coast_low3m_ev14", coast_camera(c, heading, 0.0, 3.0, 300.0, 0.0, 4.0, aspect), false),
        ("coast_low3m_atmosphere_ev14", coast_camera(c, heading, 0.0, 3.0, 300.0, 0.0, 4.0, aspect), true),
        ("coast_aerial2km_atmosphere_ev14", coast_camera(c, heading, 0.0, 2000.0, 4000.0, 0.0, 25.0, aspect), true),
        ("orbit_atmosphere_ev14", planet_frame::scripted_views(R, aspect, 1)[0].1, true),
    ];
    for (name, camera, atmosphere) in views {
        let scene = Scene { camera, size, field: &field };
        let plan = scene.terrain();
        let wp = water_plan(&plan, &src, &table, size, 7.0, WaterView::Lit);
        let mut r = new_renderer(&ctx, size);
        upload_all(&ctx, &mut r, &plan, &field);
        if atmosphere {
            r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
        }
        r.enable_water(&ctx);
        let base = frame(&ctx, &mut r, &plan, None, atmosphere);
        let with = frame(&ctx, &mut r, &plan, Some(&wp), atmosphere);
        let water = base.iter().zip(&with).filter(|(a, b)| a != b).count();
        assert!(water > 500, "{name}: {water} water pixels");
        write_inspection(name, &image_of(&r, &ctx));
    }
}

// spec: REND-028
#[test]
fn a_timed_lit_frame_reports_the_gpu_time_of_every_pass_it_ran_and_the_pixels_do_not_change() {
    use planet_render::GpuPass;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 120.0, 800.0, 0.0, 12.0, 1.6);
    let plan = Scene { camera, size, field: &field }.terrain();
    let water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Lit);
    assert!(!water.nodes.is_empty(), "the view holds water");
    let mut r = new_renderer(&ctx, size);
    upload_all(&ctx, &mut r, &plan, &field);
    r.enable_atmosphere(&ctx, &Atmosphere::earth(R));
    r.enable_water(&ctx);
    let mut lit = plan.clone();
    lit.lit = Some(lit_plan(true));
    lit.water = Some(water);
    // Adaptive exposure runs the histogram pass (a fixed exposure culls it).
    lit.lit = Some(planet_frame::look::LitPlan::adaptive(11.0, 0.016).with_atmosphere(&LookParams::default(), R).unwrap());
    // Untimed and timed frames draw the same pixels.
    r.render(&ctx, &lit, false).unwrap();
    let untimed = r.read_color(&ctx).rgba;
    let mut timed = None;
    for _ in 0..3 {
        timed = Some(r.render(&ctx, &lit, true).unwrap().0);
    }
    let timed = timed.unwrap();
    assert_eq!(untimed, r.read_color(&ctx).rgba, "timestamps change no pixel");
    if !ctx.timestamps_supported() {
        assert!(timed.passes.span_ms.is_none() && timed.gpu_ms.is_none(), "no timestamps, no times: {timed:?}");
        return;
    }
    // The long passes always read an interval; a very short one may read zero on a coarse clock and then reports nothing, never 0 ms.
    for p in [GpuPass::Terrain, GpuPass::Water, GpuPass::SkyView, GpuPass::AerialPerspective, GpuPass::SkyComposite] {
        let ms = timed.passes.get(p).unwrap_or_else(|| panic!("{}: no time in {timed:?}", p.name()));
        assert!(ms > 0.0 && ms.is_finite(), "{}: {ms}", p.name());
    }
    for p in GpuPass::ALL {
        assert!(timed.passes.get(p).is_none_or(|ms| ms > 0.0 && ms.is_finite()), "{}: {timed:?}", p.name());
    }
    assert_eq!(timed.gpu_ms, timed.passes.get(GpuPass::Terrain));
    assert_eq!(timed.water_gpu_ms, timed.passes.get(GpuPass::Water));
    // The copy is the gap between two passes: a clock that overlaps them reports none, never a negative time.
    let span = timed.passes.span_ms.unwrap();
    let longest = GpuPass::ALL.iter().filter_map(|p| timed.passes.get(*p)).fold(0.0f64, f64::max);
    assert!(
        timed.passes.scene_copy_ms.is_none_or(|c| c > 0.0) && span >= longest,
        "span {span} covers the longest pass {longest}: {timed:?}"
    );
    // A debug frame times the terrain pass alone.
    let mut debug = plan.clone();
    debug.lit = None;
    debug.water = None;
    let t = r.render(&ctx, &debug, true).unwrap().0;
    assert!(
        t.passes.get(GpuPass::Terrain).is_some() && t.passes.get(GpuPass::Water).is_none() && t.passes.scene_copy_ms.is_none(),
        "{t:?}"
    );
    // A fixed-exposure frame without the atmosphere runs no sky pass and no histogram.
    let mut plain = plan.clone();
    plain.lit = Some(lit_plan(false));
    let t = r.render(&ctx, &plain, true).unwrap().0;
    assert!(
        t.passes.get(GpuPass::SkyView).is_none()
            && t.passes.get(GpuPass::SkyComposite).is_none()
            && t.passes.get(GpuPass::ExposureHistogram).is_none(),
        "{t:?}"
    );
    assert!(t.passes.get(GpuPass::Tonemap).is_some() && t.passes.get(GpuPass::ExposureAdapt).is_some(), "{t:?}");
}
