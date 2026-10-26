//! Tier C: the base shading end to end (ADR 0039, REND-026, GEN-021): the climate field of a tile reaches the fragment stage in the
//! compact format, the albedo table and the water level come from the plan, the debug views show what they name, and the picture
//! does not depend on where a tile sits. Presentation data (ADR 0035); bounds in `oracle_lit_shading.rs`.

use planet_core::cube::TangentWarp;
use planet_frame::look::shading::{climate_view_colour, AlbedoParams, ShadeView};
use planet_frame::look::{self, LitPlan};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::global::sample;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::{GoldenChecks, Image, Tolerance};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 15.0;

fn plan(index: usize) -> (TerrainPlan, planet_frame::Camera) {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    let p = plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
        .unwrap();
    (p, camera)
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

fn renderer(ctx: &GpuContext, p: &TerrainPlan) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, compact_pool());
    r.enable_lit(ctx);
    let field = Reference(1);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_some() {
            continue;
        }
        r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
    }
    r
}

fn hdr_of(r: &mut TerrainRenderer, ctx: &GpuContext, p: &TerrainPlan, plan: LitPlan) -> Vec<[f32; 4]> {
    r.render(ctx, &lit(p.clone(), plan), false).unwrap();
    r.lit_path().unwrap().read_hdr(ctx)
}

fn view_plan(view: ShadeView) -> LitPlan {
    let mut l = LitPlan::fixed(EV);
    l.view = view;
    l
}

/// The colour a debug view wrote: the written value over the pre-exposure factor, times pi (the views write colour / pi).
fn colour_of(px: &[f32; 4]) -> [f64; 3] {
    let f = look::pre_exposure_factor(f64::from(EV));
    [0, 1, 2].map(|c| f64::from(px[c]) / f * core::f64::consts::PI)
}

/// The unit direction of the planet point under pixel `(x, y)` of a camera, or `None` when the ray misses the sphere of radius `r`.
fn hit_direction(cam: &planet_frame::Camera, size: (u32, u32), x: u32, y: u32, r: f64) -> Option<[f64; 3]> {
    let f = cam.forward.normalized();
    let right = f.cross(cam.up).normalized();
    let up = right.cross(f);
    let t = libm::tan(cam.fov_y_rad * 0.5);
    let nx = (f64::from(x) + 0.5) / f64::from(size.0) * 2.0 - 1.0;
    let ny = 1.0 - (f64::from(y) + 0.5) / f64::from(size.1) * 2.0;
    let d = (f + right * (nx * t * cam.aspect) + up * (ny * t)).normalized();
    let o = cam.position.0;
    let b = o.dot(d);
    let c = o.dot(o) - r * r;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let s = -b - disc.sqrt();
    let p = o + d * s;
    let n = p.normalized();
    Some([n.x, n.y, n.z])
}

// spec: GEN-021, REND-027
#[test]
fn the_climate_views_show_the_climate_field_of_the_world() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // The orbit view (the floor level, 3) and the 20 km view, both compact tiles.
    for index in [0usize, 3] {
        let (p, cam) = plan(index);
        let mut r = renderer(&ctx, &p);
        for view in [ShadeView::Temperature, ShadeView::Precipitation] {
            let hdr = hdr_of(&mut r, &ctx, &p, view_plan(view));
            let (mut n, mut sum, mut worst, mut off) = (0usize, 0.0f64, 0.0f64, 0usize);
            for (i, px) in hdr.iter().enumerate() {
                if px[3] <= 0.0 {
                    continue;
                }
                let (x, y) = ((i as u32) % SIZE.0, (i as u32) / SIZE.0);
                let Some(d) = hit_direction(&cam, SIZE, x, y, R) else { continue };
                // The world's climate at the point the pixel sees: the analytic function the reference field's tiles are sampled from.
                let s = sample(1, planet_core::Vec3::new(d[0], d[1], d[2]));
                let want = climate_view_colour(view, f64::from(s.temperature_c), f64::from(s.precipitation_mm));
                let got = colour_of(px);
                let err = (0..3).map(|c| (got[c] - want[c]).abs()).fold(0.0, f64::max);
                sum += err;
                worst = worst.max(err);
                off += usize::from(err > 0.1);
                n += 1;
            }
            assert!(n > 300, "view {index} {view:?}: the planet covers {n} pixels");
            // The 9 x 9 texture is a coarse picture of the field: a few hundredths of the colour range on average.
            println!("view {index} {view:?}: {n} pixels, mean error {:.4}, worst {worst:.3}", sum / n as f64);
            // Declared bounds (REND-027): 0.02 on the coarse orbit view, 0.005 on the 20 km view where tiles are fine.
            let bound = if index == 0 { 0.02 } else { 0.005 };
            assert!(sum / (n as f64) < bound, "{view:?}: mean colour error {} against the world's climate", sum / n as f64);
            // The few pixels further off sit on the limb, where the faceted coarse tiles and the sphere the test intersects part ways.
            assert!(off * 30 <= n, "{view:?}: {off} of {n} pixels are more than 0.1 off the world's climate (worst {worst})");
        }
    }
}

/// The same renderer with the tiles scattered over the blocks in reverse order with gaps (a manager's placement), not the free list's.
fn renderer_scattered(ctx: &GpuContext, p: &TerrainPlan) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, compact_pool());
    r.enable_lit(ctx);
    let field = Reference(1);
    let mut slot = r.pool_config().main_slots() - 1;
    for n in &p.nodes {
        let tile = compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R);
        r.upload_tile_at(ctx, &tile, planet_streaming::SlotRef { pool: planet_streaming::PoolKind::Main, index: slot }).unwrap();
        slot -= 3;
    }
    r
}

// spec: REND-027, GEN-021
#[test]
fn the_shading_does_not_depend_on_where_the_tiles_sit_in_every_view() {
    // Replaces the comparison of the two tile formats' shading (`remove-vertex-render-path`): the climate block rides in the tile's
    // slot, so the picture must be the same wherever the slot is, free-list placement or scattered.
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 3] {
        let (p, _) = plan(index);
        let (mut rc, mut rv) = (renderer(&ctx, &p), renderer_scattered(&ctx, &p));
        for view in ShadeView::ALL {
            let (a, b) = (hdr_of(&mut rc, &ctx, &p, view_plan(view)), hdr_of(&mut rv, &ctx, &p, view_plan(view)));
            let covered = a.iter().filter(|px| px[3] > 0.0).count();
            assert!(covered > 100);
            // The same bytes in other slots: the picture is identical, bit for bit.
            assert!(a == b, "view {index} {view:?}: the picture depends on where the tiles sit");
        }
    }
}

// spec: LOOK-007
#[test]
fn the_albedo_table_and_the_water_level_come_from_the_plan() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(3);
    let mut r = renderer(&ctx, &p);
    let unit = look::pre_exposure_factor(f64::from(EV)) / core::f64::consts::PI;
    // A table whose every surface has the albedo 0.5: the albedo view is a flat grey wherever terrain is, whatever the climate,
    // slope and height (it proves the table reaches the shader and nothing else colours the albedo view).
    let mut grey = view_plan(ShadeView::Albedo);
    let g = [0.5; 3];
    grey.albedo = AlbedoParams { rock: g, snow: g, grass: g, forest: g, desert: g, tundra: g, beach: g, seabed: g, ..AlbedoParams::EARTH };
    let hdr = hdr_of(&mut r, &ctx, &p, grey);
    let mut covered = 0;
    for px in hdr.iter().filter(|px| px[3] > 0.0) {
        covered += 1;
        for c in 0..3 {
            let want = 0.5 * unit;
            assert!((f64::from(px[c]) - want).abs() <= 2.0e-3 * want, "{px:?} against a flat albedo of 0.5 ({want})");
        }
    }
    assert!(covered > 100);
    // The water level is the plan's: with the water far above the terrain every pixel is the seabed, with it far below none is.
    let seabed = AlbedoParams::EARTH.seabed;
    let mut deep = view_plan(ShadeView::Albedo);
    deep.water_level_m = 1.0e5;
    for px in hdr_of(&mut r, &ctx, &p, deep).iter().filter(|px| px[3] > 0.0) {
        for c in 0..3 {
            assert!((f64::from(px[c]) - seabed[c] * unit).abs() <= 2.0e-3 * seabed[c] * unit + 1e-7, "{px:?} is not the seabed");
        }
    }
    let mut dry = view_plan(ShadeView::Albedo);
    dry.water_level_m = -1.0e5;
    let dry_img = hdr_of(&mut r, &ctx, &p, dry);
    let seabed_px = dry_img
        .iter()
        .filter(|px| px[3] > 0.0)
        .filter(|px| (0..3).all(|c| (f64::from(px[c]) - seabed[c] * unit).abs() <= 2.0e-3 * seabed[c] * unit + 1e-7))
        .count();
    assert_eq!(seabed_px, 0, "no seabed with the water far below the terrain");
    // The default table gives another picture than the grey one, and the lit view is the albedo view lit by the sun.
    let earth = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::Albedo));
    let lit_img = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::Lit));
    let light = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::Lighting));
    let mut checked = 0;
    for ((a, l), s) in earth.iter().zip(&lit_img).zip(&light) {
        if a[3] <= 0.0 {
            continue;
        }
        // lit = albedo x lighting / (1 / pi) in written units: (albedo / pi) x (n.l / pi) x pi = lit.
        for c in 0..3 {
            let want = f64::from(a[c]) * f64::from(s[c]) / unit;
            assert!((f64::from(l[c]) - want).abs() <= 2.0e-3 * want + 4.0e-3 * unit * 1e-2, "lit {} vs albedo x lighting {want}", l[c]);
        }
        checked += 1;
    }
    assert!(checked > 100);
}

// spec: REND-027, CON-18
#[test]
fn the_lighting_view_is_the_sun_alone_and_the_sun_comes_from_the_plan() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, cam) = plan(0);
    let mut r = renderer(&ctx, &p);
    let unit = look::pre_exposure_factor(f64::from(EV)) / core::f64::consts::PI;
    // Suns towards the camera (the lit hemisphere is the visible one) and away from it, at a time of day for the first form of the
    // test: noon over longitude 0 is towards +x.
    let towards = cam.position.0.normalized();
    let towards = [towards.x as f32, towards.y as f32, towards.z as f32];
    let away = towards.map(|c| -c);
    let mut images = Vec::new();
    for sun in [towards, away] {
        let mut l = view_plan(ShadeView::Lighting);
        l.sun_dir = sun;
        let hdr = hdr_of(&mut r, &ctx, &p, l);
        // Lighting is n.l in [0, 1]: no pixel is brighter than the sun-facing white, and a pixel is grey (one value in every channel).
        assert!(hdr.iter().all(|px| f64::from(px[0]) <= unit * 1.002 && px[0] == px[1] && px[1] == px[2]), "lighting is n.l in [0, 1]");
        images.push(hdr);
    }
    let lit_towards = images[0].iter().filter(|px| px[3] > 0.0 && px[0] > 0.0).count();
    let lit_away = images[1].iter().filter(|px| px[3] > 0.0 && px[0] > 0.0).count();
    assert!(lit_towards > 300, "a sun behind the camera lights the visible hemisphere: {lit_towards} pixels");
    assert!(lit_away * 10 < lit_towards, "a sun behind the planet lights almost nothing of what the camera sees: {lit_away} pixels");
    // The time of day is a way to choose the sun: the noon direction is a unit vector the plan carries.
    let noon = planet_frame::look::shading::sun_direction(12.0, 0.0);
    let mut l = view_plan(ShadeView::Lighting);
    l.sun_dir = noon;
    assert!(l.validate().is_ok());
}

// spec: REND-026, REND-027, GEN-021, CON-18, TEST-010
#[test]
fn the_shading_debug_views_match_the_adapters_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    for (index, view, golden) in [
        (0usize, ShadeView::Temperature, "lit_orbit_temperature_ev15"),
        (0, ShadeView::Precipitation, "lit_orbit_precipitation_ev15"),
        (3, ShadeView::Albedo, "lit_aerial20km_albedo_ev15"),
        (0, ShadeView::Lighting, "lit_orbit_lighting_ev15"),
    ] {
        let (p, _) = plan(index);
        let mut r = renderer(&ctx, &p);
        r.render(&ctx, &lit(p.clone(), view_plan(view)), false).unwrap();
        let f = r.read_color(&ctx);
        let img = Image::new(f.width, f.height, f.rgba);
        // The test's own oracle for the candidate: the planet is there and shows more than one colour.
        let covered = r.lit_path().unwrap().read_hdr(&ctx).iter().filter(|px| px[3] > 0.0).count();
        assert!(covered > 100, "{golden}: only {covered} terrain pixels");
        let colours: std::collections::BTreeSet<[u8; 3]> = img.pixels().map(|px| [px[0], px[1], px[2]]).collect();
        assert!(colours.len() >= 3, "{golden}: {} colours", colours.len());
        goldens.check(golden, &img, Tolerance::Exact);
    }
    goldens.finish();
}

fn detail_plan(view: ShadeView, strength: f64) -> LitPlan {
    let mut l = view_plan(view);
    l.ground_detail = strength;
    l
}

fn luminance(px: &[f32; 4]) -> f64 {
    0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2])
}

// spec: REND-034, LOOK-009
#[test]
fn the_ground_detail_is_inert_when_off_changes_the_lit_picture_when_on_and_leaves_the_climate_views_alone() {
    use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // The ground at 1 m above the terrain (the scripted view 6): the finest tiles fill the picture.
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let off = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 0.0));
    let covered = off.iter().filter(|px| px[3] > 0.0).count();
    assert!(covered > 1000, "{covered} covered pixels");
    // Off: whatever the material table says, the picture is the same bit for bit (the strength 0 returns before the weights).
    let mut odd = detail_plan(ShadeView::Lit, 0.0);
    odd.materials = MaterialParams { curves: [[[0.0, 0.0]; INPUTS]; MATERIALS] };
    assert!(hdr_of(&mut r, &ctx, &p, odd) == off, "the strength 0 is not inert");
    // On: the picture differs on most of the ground, is deterministic, and keeps its mean brightness within a factor of two.
    let on = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    assert!(on == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0)), "two renders of the same plan differ");
    let changed =
        off.iter().zip(&on).filter(|(a, b)| a[3] > 0.0 && (luminance(a) - luminance(b)).abs() > 0.02 * luminance(a).max(1e-6)).count();
    assert!(changed * 5 > covered, "the detail changed {changed} of {covered} pixels by more than 2 %");
    let mean = |img: &[[f32; 4]]| img.iter().filter(|px| px[3] > 0.0).map(luminance).sum::<f64>() / covered as f64;
    let ratio = mean(&on) / mean(&off);
    assert!((0.5..2.0).contains(&ratio), "mean brightness ratio {ratio}");
    // The table drives the picture: with every material off the detail does nothing; a table of soil alone differs from one of rock alone.
    let table = |m: usize| {
        let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
        // A response of 1 for the reserved soil-depth input (it reads 0, the curve rises from -1 to 0): the material is on everywhere.
        curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
        MaterialParams { curves }
    };
    let mut none = detail_plan(ShadeView::Lit, 1.0);
    none.materials = MaterialParams { curves: [[[0.0, 0.0]; INPUTS]; MATERIALS] };
    assert!(hdr_of(&mut r, &ctx, &p, none) == off, "no material, no detail");
    let (mut rock, mut soil) = (detail_plan(ShadeView::Lit, 1.0), detail_plan(ShadeView::Lit, 1.0));
    rock.materials = table(0);
    soil.materials = table(3);
    let (a, b) = (hdr_of(&mut r, &ctx, &p, rock), hdr_of(&mut r, &ctx, &p, soil));
    let apart = a.iter().zip(&b).filter(|(x, y)| x[3] > 0.0 && (luminance(x) - luminance(y)).abs() > 0.02 * luminance(x).max(1e-6)).count();
    assert!(apart * 5 > covered, "rock alone and soil alone differ on {apart} of {covered} pixels only");
    // The climate views show the climate, not the detail.
    for view in [ShadeView::Temperature, ShadeView::Precipitation] {
        assert!(
            hdr_of(&mut r, &ctx, &p, detail_plan(view, 1.0)) == hdr_of(&mut r, &ctx, &p, detail_plan(view, 0.0)),
            "{view:?} shows the detail"
        );
    }
    // Placement: the same bytes in other slots give the same detail (the anchor is in the node record, not in the slot).
    let mut scattered = renderer_scattered(&ctx, &p);
    assert!(hdr_of(&mut scattered, &ctx, &p, detail_plan(ShadeView::Lit, 1.0)) == on, "the detail depends on where the tiles sit");
}

// spec: REND-037
#[test]
fn each_material_alone_reads_as_itself_in_colour_and_value() {
    use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let mut mean = |m: usize| {
        let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
        curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
        let mut l = detail_plan(ShadeView::Lit, 1.0);
        l.materials = MaterialParams { curves };
        let img = hdr_of(&mut r, &ctx, &p, l);
        let px: Vec<_> = img.iter().filter(|px| px[3] > 0.0).collect();
        let n = px.len() as f64;
        assert!(n > 1000.0, "{n} covered pixels");
        [0, 1, 2].map(|c| px.iter().map(|x| f64::from(x[c])).sum::<f64>() / n)
    };
    let rock = mean(0);
    let sand = mean(1);
    let grass = mean(2);
    let soil = mean(3);
    let lum = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    assert!(grass[1] > 1.3 * grass[0] && grass[1] > 1.5 * grass[2], "grass is not green: {grass:?}");
    let (lo, hi) = (rock.iter().cloned().fold(f64::MAX, f64::min), rock.iter().cloned().fold(0.0, f64::max));
    assert!(hi < 1.12 * lo, "rock is not grey: {rock:?}");
    assert!(lum(sand) > 2.0 * lum(soil), "sand {sand:?} is not twice as bright as soil {soil:?}");
    assert!(lum(soil) < lum(rock), "soil {soil:?} is not darker than rock {rock:?}");
}

// spec: REND-035, LOOK-010
#[test]
fn the_sun_adds_a_highlight_to_the_detailed_ground_that_follows_the_gloss_table_and_the_detail_strength() {
    use planet_frame::look::materials::GlossParams;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // Looking at the ground 1 m above it from the scripted view 6; the sun is the look's.
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    // With f0 0 only the Fresnel's grazing rise is left: the term is monotone in f0, so the picture with the default f0 is nowhere darker.
    let mut no_f0 = detail_plan(ShadeView::Lit, 1.0);
    no_f0.gloss.f0 = 0.0;
    let base = hdr_of(&mut r, &ctx, &p, no_f0);
    let covered = base.iter().filter(|px| px[3] > 0.0).count();
    assert!(covered > 1000);
    // The highlight only adds light, and adds some.
    let shiny = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    let (mut darker, mut brighter) = (0, 0);
    for (a, b) in base.iter().zip(&shiny).filter(|(a, _)| a[3] > 0.0) {
        for c in 0..3 {
            darker += usize::from(b[c] < a[c]);
        }
        brighter += usize::from(luminance(b) > luminance(a) * 1.001);
    }
    assert_eq!(darker, 0, "the specular term never darkens");
    assert!(brighter > 0, "the specular term shows nowhere ({covered} pixels)");
    // The table drives it: a smoother ground gives another picture, a rougher one another still; the total light differs.
    let mut table = |rough: f64| {
        let mut l = detail_plan(ShadeView::Lit, 1.0);
        l.gloss = GlossParams { roughness: [rough; 4], ..GlossParams::EARTH };
        hdr_of(&mut r, &ctx, &p, l)
    };
    let (smooth, rough) = (table(0.2), table(1.0));
    assert!(smooth != rough && smooth != shiny, "the roughness table does not reach the picture");
    // A dark exposure with the smoothest ground multiplies the highlight by a large factor: every pixel stays finite and under the clamp.
    let mut dark = detail_plan(ShadeView::Lit, 1.0);
    dark.exposure = planet_frame::look::ExposureMode::Fixed { ev100: -2.0 };
    dark.gloss = GlossParams { roughness: [0.2; 4], f0: 1.0, ..GlossParams::EARTH };
    for px in hdr_of(&mut r, &ctx, &p, dark).iter().filter(|px| px[3] > 0.0) {
        assert!((0..3).all(|c| px[c].is_finite() && px[c] <= 60000.0), "{px:?}");
    }
    // Strength 0 has no highlight whatever the gloss; the albedo and climate views have none.
    let mut shiny_off = detail_plan(ShadeView::Lit, 0.0);
    shiny_off.gloss.f0 = 1.0;
    assert!(hdr_of(&mut r, &ctx, &p, shiny_off) == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 0.0)), "gloss without detail");
    let mut albedo = detail_plan(ShadeView::Albedo, 1.0);
    albedo.gloss.f0 = 1.0;
    assert!(hdr_of(&mut r, &ctx, &p, albedo) == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Albedo, 1.0)), "gloss in the albedo view");
}

// spec: GEN-071, GEN-072, CON-18
#[test]
fn the_driver_views_show_the_drivers_of_the_tiles_driver_block() {
    use planet_frame::look::shading::driver_view_colour;
    use planet_generators::drivers::{decode_soil, decode_unit, pack_driver_word, DriverParams};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(3);
    let params = DriverParams::default();
    // Every texel of every tile holds soil code 180, moisture 100 and insolation 200 (neutral insolation 128).
    let codes = [180u8, 100, 200];
    let word = pack_driver_word(codes, 128);
    let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, compact_pool());
    r.enable_lit(&ctx);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            let mut tile = compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R);
            tile.driver_words = [word; planet_generators::drivers::DRIVER_TEXELS];
            r.upload_tile(&ctx, &tile).unwrap();
        }
    }
    let (soil, moisture, insolation) = (decode_soil(codes[0], &params), decode_unit(codes[1]), decode_unit(codes[2]));
    for view in [ShadeView::SoilDepth, ShadeView::Moisture, ShadeView::Insolation] {
        let hdr = hdr_of(&mut r, &ctx, &p, view_plan(view));
        let want = driver_view_colour(view, soil, moisture, insolation);
        let (mut n, mut worst) = (0usize, 0.0f64);
        for px in hdr.iter().filter(|px| px[3] > 0.0) {
            let got = colour_of(px);
            worst = worst.max((0..3).map(|c| (got[c] - want[c]).abs()).fold(0.0, f64::max));
            n += 1;
        }
        assert!(n > 300, "{view:?}: the planet covers {n} pixels");
        assert!(worst < 0.003, "{view:?}: the colour is {worst} off the CPU's {want:?}");
    }
    // The three views differ from one another and from the lit picture.
    let lit_hdr = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::Lit));
    let soil_hdr = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::SoilDepth));
    assert_ne!(lit_hdr, soil_hdr);
}

// spec: GEN-074
#[test]
fn a_ramp_in_the_driver_block_shows_in_the_views_between_the_cpu_mirrors_extremes() {
    use planet_frame::look::shading::driver_view_colour;
    use planet_generators::drivers::{pack_driver_word, tile_drivers_at, DriverParams, DRIVER_TEXELS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(3);
    let params = DriverParams::default();
    // The soil code runs with the texel column, the moisture with its row, the insolation with their sum: a transposed index shows.
    let mut words = [0u32; DRIVER_TEXELS];
    for b in 0..9 {
        for a in 0..9 {
            words[b * 9 + a] = pack_driver_word([(a * 28) as u8, (b * 28) as u8, ((a + b) * 14) as u8], 60);
        }
    }
    let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, compact_pool());
    r.enable_lit(&ctx);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            let mut tile = compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R);
            tile.driver_words = words;
            r.upload_tile(&ctx, &tile).unwrap();
        }
    }
    for view in [ShadeView::SoilDepth, ShadeView::Moisture, ShadeView::Insolation] {
        let hdr = hdr_of(&mut r, &ctx, &p, view_plan(view));
        let seen: Vec<[f64; 3]> = hdr.iter().filter(|px| px[3] > 0.0).map(colour_of).collect();
        let corner = |u: f64, v: f64| {
            let d = tile_drivers_at(&words, u, v, 1.0, &params);
            driver_view_colour(view, d[0], d[1], d[2])
        };
        let (lo, hi) = (corner(0.0, 0.0), corner(1.0, 1.0));
        let span = |c: usize| (lo[c].min(hi[c]) - 0.003, lo[c].max(hi[c]) + 0.003);
        let inside = seen.iter().filter(|g| (0..3).all(|c| g[c] >= span(c).0 && g[c] <= span(c).1)).count();
        assert!(
            seen.len() > 300 && inside * 20 >= seen.len() * 19,
            "{view:?}: {inside} of {} pixels inside the ramp's colours",
            seen.len()
        );
        let distinct: std::collections::BTreeSet<u64> = seen.iter().map(|g| (g[0] * 1000.0) as u64).collect();
        assert!(distinct.len() > 3, "{view:?}: the ramp shows as one colour");
    }
}
