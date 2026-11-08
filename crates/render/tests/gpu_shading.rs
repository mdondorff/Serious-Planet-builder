//! Tier C: the base shading end to end (ADR 0039, REND-026, GEN-021): the climate field of a tile reaches the fragment stage in the
//! compact format, the albedo table and the water level come from the plan, the debug views show what they name, and the picture
//! does not depend on where a tile sits. Presentation data (ADR 0035); bounds in `oracle_lit_shading.rs`.

use planet_core::cube::TangentWarp;
use planet_frame::look::shading::{climate_view_colour, AlbedoParams, ShadeView};
use planet_frame::look::{self, LitPlan};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::global::climate_from_elevation;
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
                // The field's climate at the point the pixel sees: the climate formula over the reference field's own height there (GEN-021), what its tiles are sampled from.
                let at = planet_core::Vec3::new(d[0], d[1], d[2]);
                let (temperature, precipitation) = climate_from_elevation(1, at, planet_generators::height::height_at(1, at), 0.0);
                let want = climate_view_colour(view, f64::from(temperature), f64::from(precipitation));
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

// spec: LOOK-013
#[test]
fn the_octave_coefficients_drive_the_picture_and_the_strength_zero_is_inert() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let plain = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    let covered = plain.iter().filter(|px| px[3] > 0.0).count();
    // (That the Earth values draw what the old shader constants drew is the evidence of the exact goldens of the lit ground, `gpu_ground_look`: they need no candidate.)
    // Every coefficient at 0: no albedo variation and no tilt, so the picture differs on most of the ground; the strength 0 is still the off picture.
    let mut flat = detail_plan(ShadeView::Lit, 1.0);
    for o in &mut flat.coef.octaves {
        o.albedo = [0.0; 3];
        o.grass = [0.0; 3];
        o.tilt = [0.0; 4];
    }
    flat.coef.crack = 0.0;
    let flat_img = hdr_of(&mut r, &ctx, &p, flat);
    let apart = plain
        .iter()
        .zip(&flat_img)
        .filter(|(a, b)| a[3] > 0.0 && (luminance(a) - luminance(b)).abs() > 0.02 * luminance(a).max(1e-6))
        .count();
    assert!(apart * 5 > covered, "zero coefficients differ on {apart} of {covered} pixels only");
    let mut off = detail_plan(ShadeView::Lit, 0.0);
    off.coef = flat.coef;
    assert!(hdr_of(&mut r, &ctx, &p, off) == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 0.0)), "the strength 0 is not inert");
}

// spec: LOOK-013
#[test]
fn a_changed_16_m_tilt_moves_the_picture() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let plain = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    let mut tilted = detail_plan(ShadeView::Lit, 1.0);
    tilted.coef.octaves[0].tilt = [4.0; 4];
    let img = hdr_of(&mut r, &ctx, &p, tilted);
    assert!(img != plain, "a 16 m tilt of 4 changed nothing");
    // (The 3 cm octave fades out at this distance, so it is left out.) Each octave's tilt and grass entries are read from their own place (a lane's material may be absent from the frame, so a lane alone proves nothing).
    for octave in 0..3 {
        let mut c = detail_plan(ShadeView::Lit, 1.0);
        c.coef.octaves[octave].tilt = [4.0; 4];
        assert!(hdr_of(&mut r, &ctx, &p, c) != plain, "octave {octave} tilt is not read");
        let mut c = detail_plan(ShadeView::Lit, 1.0);
        c.coef.octaves[octave].grass = [4.0; 3];
        assert!(hdr_of(&mut r, &ctx, &p, c) != plain, "octave {octave} grass is not read");
    }
}

// spec: REND-056
#[test]
fn scree_and_wet_ground_are_off_in_the_earth_default_and_draw_their_own_look_when_on() {
    use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let plain = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    let mut alone = |m: usize| {
        let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
        curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
        let mut l = detail_plan(ShadeView::Lit, 1.0);
        l.materials = MaterialParams { curves };
        hdr_of(&mut r, &ctx, &p, l)
    };
    let (scree, wet, soil) = (alone(4), alone(5), alone(3));
    assert!(scree != plain && wet != plain, "a slot that is on draws the Earth picture");
    assert!(scree != wet && scree != soil && wet != soil, "two materials draw the same picture");
    // Scree is rock-like grey and wet ground is darker than scree.
    let mean = |img: &[[f32; 4]]| {
        let px: Vec<_> = img.iter().filter(|x| x[3] > 0.0).collect();
        px.iter().map(|x| f64::from(x[0] + x[1] + x[2])).sum::<f64>() / px.len() as f64
    };
    assert!(mean(&wet) < mean(&scree), "wet ground is not darker than scree");
    // Each octave's scree and wet entries are read from their own place (the 3 cm octave fades out at this distance).
    for (m, base) in [(4usize, &scree), (5, &wet)] {
        for octave in 0..3 {
            for what in 0..2 {
                let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
                curves[m][Input::SoilDepth as usize] = [-1.0, 0.0];
                let mut l = detail_plan(ShadeView::Lit, 1.0);
                l.materials = MaterialParams { curves };
                let o = &mut l.coef.octaves[octave];
                let entry = if m == 4 { &mut o.scree } else { &mut o.wet };
                entry[what] = 4.0;
                assert!(&hdr_of(&mut r, &ctx, &p, l) != base, "material {m} octave {octave} entry {what} is not read");
            }
        }
    }
}

// spec: REND-057
#[test]
fn the_64_m_and_8_m_octaves_are_off_in_the_earth_default_and_each_draws_when_given_coefficients() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    let plain = hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 1.0));
    let mut covered = 0;
    for octave in [4usize, 5] {
        for tilt_only in [false, true] {
            let mut l = detail_plan(ShadeView::Lit, 1.0);
            let o = &mut l.coef.octaves[octave];
            if tilt_only {
                o.tilt = [0.3; 4];
            } else {
                o.albedo = [0.3; 3];
                o.grass = [0.3; 3];
            }
            let img = hdr_of(&mut r, &ctx, &p, l);
            assert!(img != plain, "octave {octave} (tilt only: {tilt_only}) changed nothing");
            covered += 1;
        }
    }
    assert_eq!(covered, 4);
    // Each new octave has a lattice of its own: its coefficients copied from a scale neighbour draw another picture than the neighbour alone.
    for (new, old) in [(4usize, 0usize), (5, 1)] {
        let mut neighbour = detail_plan(ShadeView::Lit, 1.0);
        neighbour.coef.octaves[new] = neighbour.coef.octaves[old];
        neighbour.coef.octaves[new].albedo = [0.3; 3];
        let mut alone = detail_plan(ShadeView::Lit, 1.0);
        alone.coef.octaves[new] = alone.coef.octaves[old];
        alone.coef.octaves[new].albedo = [0.3; 3];
        alone.coef.octaves[old].albedo = [0.0; 3];
        assert!(hdr_of(&mut r, &ctx, &p, neighbour) != hdr_of(&mut r, &ctx, &p, alone), "octave {new} is a copy of octave {old}");
    }
    // The strength 0 is still the off picture with them on.
    let mut off = detail_plan(ShadeView::Lit, 0.0);
    off.coef.octaves[4].albedo = [0.3; 3];
    assert!(hdr_of(&mut r, &ctx, &p, off) == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 0.0)), "the strength 0 is not inert");
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
        l.gloss = GlossParams { roughness: [rough; 6], ..GlossParams::EARTH };
        hdr_of(&mut r, &ctx, &p, l)
    };
    let (smooth, rough) = (table(0.2), table(1.0));
    assert!(smooth != rough && smooth != shiny, "the roughness table does not reach the picture");
    // A dark exposure with the smoothest ground multiplies the highlight by a large factor: every pixel stays finite and under the clamp.
    let mut dark = detail_plan(ShadeView::Lit, 1.0);
    dark.exposure = planet_frame::look::ExposureMode::Fixed { ev100: -2.0 };
    dark.gloss = GlossParams { roughness: [0.2; 6], f0: 1.0, ..GlossParams::EARTH };
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

// spec: GEN-071, GEN-072, GEN-074, CON-18
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

// spec: GEN-119
#[test]
fn tiles_built_from_a_fields_channels_show_them_in_the_driver_views_and_the_default_look_draws_what_it_drew() {
    use planet_generators::drivers::ChannelSample;
    // The reference surface with made-up channels: the deposited depth follows z (0.5 to 3.5 m), the area x.
    struct Channelled(Reference);
    impl planet_generators::height::HeightField for Channelled {
        fn height(&self, dir: planet_core::Vec3) -> f64 {
            self.0.height(dir)
        }
        fn seed(&self) -> u64 {
            self.0.seed()
        }
        fn implementation_id(&self) -> &'static str {
            self.0.implementation_id()
        }
        fn version(&self) -> u32 {
            self.0.version()
        }
        fn max_abs_height_m(&self) -> f64 {
            self.0.max_abs_height_m()
        }
        fn driver_channels(&self, dirs: &[planet_core::Vec3]) -> Option<Vec<ChannelSample>> {
            Some(
                dirs.iter()
                    .map(|d| ChannelSample {
                        soil_m: Some((2.0 + 1.5 * d.z) as f32),
                        area_log2_cells: Some((5.0 + 5.0 * d.x) as f32),
                        ..Default::default()
                    })
                    .collect(),
            )
        }
    }
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(3);
    let draw = |channels: bool| {
        let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, compact_pool());
        r.enable_lit(&ctx);
        for n in &p.nodes {
            if r.slot_of(n.tile).is_none() {
                let tile = if channels {
                    compact_tile_field(&Channelled(Reference(1)), &TangentWarp, n.tile, CELLS, R)
                } else {
                    compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R)
                };
                r.upload_tile(&ctx, &tile).unwrap();
            }
        }
        [ShadeView::Lit, ShadeView::SoilDepth, ShadeView::Moisture].map(|v| hdr_of(&mut r, &ctx, &p, view_plan(v)))
    };
    let (neutral, filled) = (draw(false), draw(true));
    // The default look ignores the drivers (GEN-073): the lit picture is the same bit for bit; the two driver views show the channels and differ from the neutral ones.
    let bits = |img: &[[f32; 4]]| -> Vec<[u32; 4]> { img.iter().map(|px| px.map(f32::to_bits)).collect() };
    assert_eq!(bits(&neutral[0]), bits(&filled[0]), "the default look draws what it drew");
    for (k, name) in [(1usize, "soil depth"), (2, "moisture")] {
        assert_ne!(bits(&neutral[k]), bits(&filled[k]), "the {name} view shows the channels");
        // The neutral view is one colour over the planet, the filled one is not.
        let spread = |img: &[[f32; 4]]| {
            let l: Vec<f64> = img.iter().filter(|px| px[3] > 0.0).map(|px| f64::from(px[0] + px[1] + px[2])).collect();
            l.iter().cloned().fold(f64::MIN, f64::max) - l.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(
            spread(&filled[k]) > 4.0 * spread(&neutral[k]).max(1e-3),
            "the {name} view varies over the planet: {} against {}",
            spread(&filled[k]),
            spread(&neutral[k])
        );
    }
}

// spec: GEN-073
#[test]
fn the_default_look_draws_the_same_pixels_whatever_the_driver_block_holds() {
    use planet_generators::drivers::{pack_driver_word, DRIVER_TEXELS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // A uniform block far from neutral, and a ramp whose soil, moisture and insolation run across the tile.
    let uniform = [pack_driver_word([180, 100, 200], 128); DRIVER_TEXELS];
    let mut ramp = [0u32; DRIVER_TEXELS];
    for b in 0..9 {
        for a in 0..9 {
            ramp[b * 9 + a] = pack_driver_word([(a * 28) as u8, (b * 28) as u8, ((a + b) * 14) as u8], 60);
        }
    }
    // The orbit-to-region view without ground detail, and the ground at 1 m with it (the material weights and the gloss run there).
    for (index, strength) in [(3usize, 0.0), (6, 1.0)] {
        let (p, _) = plan(index);
        let draw = |words: Option<[u32; DRIVER_TEXELS]>| -> Vec<Vec<[u32; 4]>> {
            let mut r = TerrainRenderer::with_pools(&ctx, SIZE, CELLS, compact_pool());
            r.enable_lit(&ctx);
            for n in &p.nodes {
                if r.slot_of(n.tile).is_none() {
                    let mut tile = compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R);
                    if let Some(w) = words {
                        tile.driver_words = w;
                    }
                    r.upload_tile(&ctx, &tile).unwrap();
                }
            }
            // Bit for bit: the pixels as their f32 bits (a sign of zero would count).
            [ShadeView::Lit, ShadeView::Albedo]
                .map(|v| hdr_of(&mut r, &ctx, &p, detail_plan(v, strength)).iter().map(|px| px.map(f32::to_bits)).collect())
                .to_vec()
        };
        let neutral = draw(None);
        let covered = neutral[0].iter().filter(|px| f32::from_bits(px[3]) > 0.0).count();
        assert!(covered > 300, "view {index}: {covered} covered pixels");
        for (name, words) in [("uniform", uniform), ("ramp", ramp)] {
            assert!(
                draw(Some(words)) == neutral,
                "view {index}, detail {strength}: the {name} driver block changed the default look's pixels"
            );
        }
    }
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

// spec: REND-055
#[test]
fn the_patch_break_up_is_inert_when_off_and_moves_the_material_boundary_when_on() {
    use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(6);
    let mut r = renderer(&ctx, &p);
    // A table whose rock rises as the slope cosine falls from 1 to 0.9 (rock where the ground is steeper), soil everywhere: the boundary the patches shift.
    let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
    curves[0][Input::SlopeCos as usize] = [1.0, 0.9];
    curves[3][Input::SoilDepth as usize] = [-1.0, 0.0];
    let table = MaterialParams { curves };
    let with = |patch: f64| {
        let mut l = detail_plan(ShadeView::Lit, 1.0);
        l.materials = table;
        l.ground_patch = patch;
        l
    };
    let off = hdr_of(&mut r, &ctx, &p, with(0.0));
    let covered = off.iter().filter(|px| px[3] > 0.0).count();
    assert!(covered > 1000, "{covered} covered pixels");
    // Off is today's picture (the default plan has patch 0, and the shader's patch branch is not taken).
    assert!(
        hdr_of(&mut r, &ctx, &p, {
            let mut l = detail_plan(ShadeView::Lit, 1.0);
            l.materials = table;
            l
        }) == off,
        "patch 0 is not inert"
    );
    let on = hdr_of(&mut r, &ctx, &p, with(0.25));
    assert!(on == hdr_of(&mut r, &ctx, &p, with(0.25)), "two renders of the same plan differ");
    let changed =
        off.iter().zip(&on).filter(|(a, b)| a[3] > 0.0 && (luminance(a) - luminance(b)).abs() > 0.01 * luminance(a).max(1e-6)).count();
    // The patches are 8 and 32 m cells and the scene a few metres across: where the noise leaves the boundary alone nothing changes, so the check is that the boundary moves somewhere, by a lot.
    let worst = off
        .iter()
        .zip(&on)
        .filter(|(a, _)| a[3] > 0.0)
        .map(|(a, b)| (luminance(a) - luminance(b)).abs() / luminance(a).max(1e-6))
        .fold(0.0, f64::max);
    assert!(
        changed >= 100 && worst > 0.2,
        "the patches changed {changed} of {covered} pixels by more than 1 %, the largest change {worst}"
    );
    let mean = |img: &[[f32; 4]]| img.iter().filter(|px| px[3] > 0.0).map(luminance).sum::<f64>() / covered as f64;
    let ratio = mean(&on) / mean(&off);
    assert!((1.0 / 1.4..1.4).contains(&ratio), "mean brightness ratio {ratio}");
    // The detail's strength 0 still returns before the patches: the picture is the off picture whatever the patch.
    let mut none = detail_plan(ShadeView::Lit, 0.0);
    none.ground_patch = 0.25;
    assert!(
        hdr_of(&mut r, &ctx, &p, none) == hdr_of(&mut r, &ctx, &p, detail_plan(ShadeView::Lit, 0.0)),
        "the patch acts with the detail off"
    );
}

/// A pool whose every tile of `p` holds the channel block `words`.
fn channel_renderer(ctx: &GpuContext, p: &TerrainPlan, words: Option<[u32; planet_generators::drivers::DRIVER_TEXELS]>) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, compact_pool());
    r.enable_lit(ctx);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            let mut tile = compact_tile_field(&Reference(1), &TangentWarp, n.tile, CELLS, R);
            if let Some(w) = words {
                tile.channel_words = w;
            }
            r.upload_tile(ctx, &tile).unwrap();
        }
    }
    r
}

// spec: GEN-126, CON-18
#[test]
fn the_channel_views_show_the_fill_and_erosion_depth_of_the_tiles_channel_block() {
    use planet_frame::look::shading::channel_view_colour;
    use planet_generators::drivers::{decode_depth, pack_channel_word, DRIVER_TEXELS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p, _) = plan(3);
    // Every texel holds fill code 120 and erosion code 40.
    let codes = [120u8, 40];
    let mut r = channel_renderer(&ctx, &p, Some([pack_channel_word(codes[0], codes[1]); DRIVER_TEXELS]));
    let (fill, erosion) = (decode_depth(codes[0]), decode_depth(codes[1]));
    for view in [ShadeView::FillDepth, ShadeView::ErosionDepth] {
        let hdr = hdr_of(&mut r, &ctx, &p, view_plan(view));
        let want = channel_view_colour(view, fill, erosion);
        let (mut n, mut worst) = (0usize, 0.0f64);
        for px in hdr.iter().filter(|px| px[3] > 0.0) {
            let got = colour_of(px);
            worst = worst.max((0..3).map(|c| (got[c] - want[c]).abs()).fold(0.0, f64::max));
            n += 1;
        }
        assert!(n > 300, "{view:?}: the planet covers {n} pixels");
        assert!(worst < 0.003, "{view:?}: the colour is {worst} off the CPU's {want:?}");
    }
    // A neutral block shows no fill and no erosion, and the two views differ from each other.
    let mut neutral = channel_renderer(&ctx, &p, None);
    let a = hdr_of(&mut neutral, &ctx, &p, view_plan(ShadeView::FillDepth));
    let b = hdr_of(&mut r, &ctx, &p, view_plan(ShadeView::FillDepth));
    assert_ne!(a, b);
    let want = channel_view_colour(ShadeView::FillDepth, 0.0, 0.0);
    let got = colour_of(a.iter().find(|px| px[3] > 0.0).unwrap());
    assert!((0..3).all(|c| (got[c] - want[c]).abs() < 0.003), "neutral: {got:?} against {want:?}");
}

// spec: GEN-126, GEN-073
#[test]
fn the_default_look_draws_the_same_pixels_whatever_the_channel_block_holds_and_a_curve_on_the_fill_depth_moves_the_picture() {
    use planet_frame::look::materials::{Input, MaterialParams, INPUTS, MATERIALS};
    use planet_generators::drivers::{pack_channel_word, DRIVER_TEXELS};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let uniform = [pack_channel_word(200, 120); DRIVER_TEXELS];
    let mut ramp = [0u32; DRIVER_TEXELS];
    for b in 0..9 {
        for a in 0..9 {
            ramp[b * 9 + a] = pack_channel_word((a * 28) as u8, (b * 28) as u8);
        }
    }
    let bits = |img: Vec<[f32; 4]>| -> Vec<[u32; 4]> { img.iter().map(|px| px.map(f32::to_bits)).collect() };
    for (index, strength) in [(3usize, 0.0), (6, 1.0)] {
        let (p, _) = plan(index);
        let draw = |words| {
            let mut r = channel_renderer(&ctx, &p, words);
            [ShadeView::Lit, ShadeView::Albedo].map(|v| bits(hdr_of(&mut r, &ctx, &p, detail_plan(v, strength))))
        };
        let neutral = draw(None);
        for (name, words) in [("uniform", uniform), ("ramp", ramp)] {
            assert!(
                draw(Some(words)) == neutral,
                "view {index}, detail {strength}: the {name} channel block changed the default look's pixels"
            );
        }
    }
    // A look that makes wet ground the only material where the fill is deep (and rock elsewhere) draws differently over a deep-fill block than over a neutral one.
    let (p, _) = plan(6);
    let mut curves = [[[0.0, 0.0]; INPUTS]; MATERIALS];
    curves[0][Input::FillDepth as usize] = [30.0, 0.0];
    curves[5][Input::FillDepth as usize] = [0.0, 30.0];
    let mut look = detail_plan(ShadeView::Lit, 1.0);
    look.materials = MaterialParams { curves };
    let deep = [pack_channel_word(255, 0); DRIVER_TEXELS];
    let (mut a, mut b) = (channel_renderer(&ctx, &p, None), channel_renderer(&ctx, &p, Some(deep)));
    let (shallow_img, deep_img) = (hdr_of(&mut a, &ctx, &p, look), hdr_of(&mut b, &ctx, &p, look));
    let covered = shallow_img.iter().filter(|px| px[3] > 0.0).count();
    let apart = shallow_img
        .iter()
        .zip(&deep_img)
        .filter(|(x, y)| x[3] > 0.0 && (luminance(x) - luminance(y)).abs() > 0.02 * luminance(x).max(1e-6))
        .count();
    assert!(apart * 5 > covered, "a deep fill changed {apart} of {covered} pixels only");
}
