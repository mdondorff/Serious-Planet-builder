//! Tier C: the water covers the sea of a real world (ATMO-019, ADR 0056). Over a large in-memory bake (the eroded field over the
//! production definition), in an aerial and a low view from the sea towards a coastal plain, every pixel whose CPU ray meets the sea
//! sphere where the terrain lies below the level and the point is ocean-connected SHALL be a water pixel of the class view, within an
//! edge allowance. The plain is the one whose erosion puts the terrain below the level while the bake calls it land.

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::litscenes::{coast_camera, Coast};
use planet_frame::look::file::LookParams;
use planet_frame::look::water::{ClassMap, WaterClass};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::water::{plan_water, WaterPlan, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::HeightField;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::budget::FIXED_RESERVE_BYTES;
use planet_streaming::PoolConfig;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 14.0;
/// The reach of the oracle's own connectivity test: an ocean vertex within this distance of the point (ADR 0056).
const CONNECT_M: f64 = 100_000.0;

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

/// Compact pools: 8 blocks of 1,024 slots.
fn compact_pool() -> PoolConfig {
    compact_format_pool_with(FIXED_RESERVE_BYTES + 8 * 1024 * planet_generators::compact::slot_bytes(CELLS), CELLS, 1024, 1).unwrap()
}

/// The first edge of the exact sea (the bake's sea by the water field) whose land side is a plain below the level: the mean terrain
/// 20 to 60 km inland is under -8 m. Walks the parallels 20 km a step and bisects the edge.
fn plain_coast(world: &WorldTerrain, field: &dyn HeightField) -> Coast {
    let is_sea =
        |d: Vec3| world.with_water(|f| planet_frame::water::WaterSource::water_at(f, d).ok().flatten().is_some()).unwrap().unwrap();
    for lat_deg in [0.0f64, 10.0, -10.0, 20.0, -20.0, 30.0, -30.0, 40.0, -40.0] {
        let lat = lat_deg.to_radians();
        let at = |s: f64| {
            let lon = 30f64.to_radians() + s / (R * libm::cos(lat));
            Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
        };
        let steps = (std::f64::consts::TAU * R * libm::cos(lat) / 20_000.0) as usize;
        let mut prev = is_sea(at(0.0));
        for k in 1..steps {
            let s = k as f64 * 20_000.0;
            let cur = is_sea(at(s));
            if prev && !cur {
                let (mut a, mut b) = (s - 20_000.0, s);
                for _ in 0..40 {
                    let m = 0.5 * (a + b);
                    if is_sea(at(m)) {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                let heading = (at(b + 10.0) - at(b - 10.0)).normalized();
                let dir = at(b);
                let inland = |km: f64| field.height((dir + heading * (km * 1000.0 / R)).normalized());
                let mean = (20..=60).step_by(5).map(|km| inland(f64::from(km))).sum::<f64>() / 9.0;
                if is_sea(at(b - 3_000.0)) && mean < -8.0 {
                    return Coast { dir, heading };
                }
            }
            prev = cur;
        }
    }
    panic!("no coast with a plain below the level in this world");
}

fn palette_class(p: &[f32; 4]) -> Option<WaterClass> {
    let factor = planet_frame::look::pre_exposure_factor_with(&planet_frame::look::SceneUnits::EARTH, f64::from(EV));
    let scale = core::f64::consts::FRAC_1_PI * factor;
    let c = [f64::from(p[0]) / scale, f64::from(p[1]) / scale, f64::from(p[2]) / scale];
    [WaterClass::Film, WaterClass::Foam, WaterClass::Shallow, WaterClass::Mid, WaterClass::Deep]
        .into_iter()
        .find(|k| (0..3).all(|i| (c[i] - WaterClass::COLOURS[*k as usize][i]).abs() < 0.02))
}

fn class_map(base: &[[f32; 4]], with: &[[f32; 4]], size: (u32, u32)) -> ClassMap {
    let mut c = vec![0u8; (size.0 * size.1) as usize];
    for (i, (a, b)) in base.iter().zip(with).enumerate() {
        if a != b {
            c[i] = palette_class(b).unwrap_or_else(|| panic!("pixel {i} is no palette colour: {b:?}")) as u8;
        }
    }
    ClassMap::new(size, c)
}

/// The point of the sea sphere (level 0) a pixel's ray meets, if any.
fn sea_point(cam: &Camera, size: (u32, u32), x: u32, y: u32) -> Option<Vec3> {
    let (right, up, fwd) = cam.ray_frame();
    let (u, v) = (2.0 * (f64::from(x) + 0.5) / f64::from(size.0) - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(size.1));
    let dir = (fwd + right * u + up * v).normalized();
    let from = cam.position.0;
    let (b, l) = (from.dot(dir), from.length());
    let c = (l - R) * (l + R);
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = if c >= 0.0 { -b - disc.sqrt() } else { -b + disc.sqrt() };
    (t > 0.0).then(|| (from + dir * t).normalized())
}

/// The oracle's connectivity: an ocean vertex of the bake within [`CONNECT_M`] of `p` (probed on a 5 km grid of tangent offsets),
/// memoised per 10 km cell.
struct Connected<'a> {
    world: &'a WorldTerrain,
    memo: std::collections::HashMap<(i64, i64, i64), bool>,
}

impl Connected<'_> {
    fn at(&mut self, p: Vec3) -> bool {
        let key = ((p.x * R / 10_000.0).round() as i64, (p.y * R / 10_000.0).round() as i64, (p.z * R / 10_000.0).round() as i64);
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        let centre = Vec3::new(key.0 as f64, key.1 as f64, key.2 as f64) * (10_000.0 / R);
        let centre = centre.normalized();
        let helper = if centre.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let east = centre.cross(helper).normalized();
        let north = centre.cross(east).normalized();
        let n = (CONNECT_M / 5_000.0) as i32;
        let mut found = false;
        'scan: for j in -n..=n {
            for i in -n..=n {
                let (dx, dy) = (f64::from(i) * 5_000.0, f64::from(j) * 5_000.0);
                if dx * dx + dy * dy > CONNECT_M * CONNECT_M {
                    continue;
                }
                let d = (centre + east * (dx / R) + north * (dy / R)).normalized();
                if self.world.bake_probe(d).is_some_and(|b| b.ocean_vertex) {
                    found = true;
                    break 'scan;
                }
            }
        }
        self.memo.insert(key, found);
        found
    }
}

fn water_plan_of(
    world: &WorldTerrain,
    plan: &TerrainPlan,
    size: (u32, u32),
    cut: Option<&dyn HeightField>,
    exact: bool,
    view: WaterView,
) -> WaterPlan {
    let look = LookParams::default();
    let table = world.definition().liquids().unwrap();
    let lod = lod_for(size);
    // The exact minimum comes from the book the tile jobs filled (the test uploads the meshes it built, as a job does).
    let mut cache = if exact { WaterTileCache::with_book(world.water_book()) } else { WaterTileCache::new() };
    world
        .with_water(|f| {
            plan_water(
                &WaterRequest { source: f, map: &TangentWarp, liquids: &table, look: &look.water, lod: &lod, time_s: 2.0, view, cut },
                &mut cache,
                plan,
            )
        })
        .unwrap()
        .unwrap()
        .unwrap()
}

fn frame(ctx: &GpuContext, r: &mut TerrainRenderer, plan: &TerrainPlan, water: Option<&WaterPlan>) -> Vec<[f32; 4]> {
    let mut p = plan.clone();
    p.lit = Some(LitPlan::fixed(EV));
    p.water = water.cloned();
    r.render(ctx, &p, false).unwrap();
    r.lit_path().unwrap().read_hdr(ctx)
}

fn coverage(views: &[&str]) {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let world = WorldTerrain::open(WorldDefinition::single_planet(R, 5), 1024).expect("the world opens").bounding_water();
    let field = world.field().unwrap();
    let coast = plain_coast(&world, field.as_ref());
    let size = (480, 270);
    let aspect = f64::from(size.0) / f64::from(size.1);
    // The aerial view (2 km up, 3 km out, level) and the low view (30 m up, level), both from the sea towards the plain.
    let beach = planet_frame::litscenes::find_coast(&|d| planet::litperf::carries_sea(&world, d), &|d| field.height(d), 0.0, 20.0, 30.0, R)
        .expect("the world has a beach: the coast views of the lit scenes stand there");
    let cases = [
        ("aerial", coast, 2000.0, 3000.0, 2.0),
        ("low", coast, 30.0, 600.0, 0.5),
        ("beach", beach, 2000.0, 3000.0, 25.0),
        ("inland", beach, 600.0, -4000.0, 10.0),
    ];
    let mut exact_cut = 0;
    for (name, coast, alt, back, pitch) in cases.into_iter().filter(|c| views.contains(&c.0)) {
        let camera = coast_camera(&coast, R, 0.0, alt, back, 0.0, pitch, aspect);
        let bounded = planet_generators::water::WaterBounded::borrowed(field.as_ref(), 0.0);
        let plan = plan_terrain_field(
            &TerrainRequest { camera, size, view: TerrainView::Level, lod: lod_for(size), face_mapping: "tangent-v1".into(), seed: 5 },
            &bounded,
        )
        .unwrap();
        let mut r = TerrainRenderer::with_pools(&ctx, size, CELLS, compact_pool());
        r.enable_lit(&ctx);
        for n in &plan.nodes {
            if r.slot_of(n.tile).is_none() {
                let tile = compact_tile_field(field.as_ref(), &TangentWarp, n.tile, CELLS, R);
                r.upload_tile(&ctx, &tile).unwrap();
                world.fill_water(n.tile, &tile);
            }
        }
        r.enable_water(&ctx);
        // The nodes the pass draws are the cut ones (ATMO-020); the full selection draws the same pixels.
        let wp = water_plan_of(&world, &plan, size, Some(field.as_ref()), false, WaterView::Class);
        let exact = water_plan_of(&world, &plan, size, Some(field.as_ref()), true, WaterView::Class);
        let full = water_plan_of(&world, &plan, size, None, false, WaterView::Class);
        let base = frame(&ctx, &mut r, &plan, None);
        let with = frame(&ctx, &mut r, &plan, Some(&wp));
        let map = class_map(&base, &with, size);
        let with_full = frame(&ctx, &mut r, &plan, Some(&full));
        let with_exact = frame(&ctx, &mut r, &plan, Some(&exact));
        assert!(exact.nodes.len() <= wp.nodes.len());
        exact_cut += wp.nodes.len() - exact.nodes.len();
        assert!(
            with_exact == with_full,
            "{name}: the exact dry-tile cut ({} nodes, {} by the loose cut, {} before) changes pixels of the picture",
            exact.nodes.len(),
            wp.nodes.len(),
            full.nodes.len()
        );
        // The lit view too (waves, absorption, glint): the colours of the exact cut are the full selection's, bit for bit.
        // (Where the cut is large: the beach and the inland view.)
        if matches!(name, "beach" | "inland") {
            let (lit_full, lit_exact) = (
                water_plan_of(&world, &plan, size, None, false, WaterView::Lit),
                water_plan_of(&world, &plan, size, Some(field.as_ref()), true, WaterView::Lit),
            );
            assert!(lit_exact.nodes.len() <= lit_full.nodes.len());
            assert!(
                frame(&ctx, &mut r, &plan, Some(&lit_full)) == frame(&ctx, &mut r, &plan, Some(&lit_exact)),
                "{name}: the lit view changes"
            );
        }
        assert!(wp.nodes.len() <= full.nodes.len());
        assert!(with == with_full, "{name}: the cut ({} nodes of {}) changes pixels of the picture", wp.nodes.len(), full.nodes.len());
        let mut connected = Connected { world: &world, memo: Default::default() };
        let (mut expected, mut missing, mut dry_checked, mut flooded) = (0u32, 0u32, 0u32, 0u32);
        let mut first_missing = None;
        for y in (0..size.1).step_by(2) {
            for x in (0..size.0).step_by(2) {
                if base[(y * size.0 + x) as usize][3] <= 0.0 {
                    continue;
                }
                let Some(p) = sea_point(&camera, size, x, y) else { continue };
                // Terrain above the level by more than the interpolation: dry land, never water.
                if field.height(p) > 12.0 {
                    dry_checked += 1;
                    flooded += u32::from(map.class[(y * size.0 + x) as usize] != 0);
                    continue;
                }
                // Below the level by more than the mesh's interpolation of the heights, in the plain's own noise.
                if field.height(p) > -12.0 || !connected.at(p) {
                    continue;
                }
                expected += 1;
                if map.class[(y * size.0 + x) as usize] == 0 {
                    missing += 1;
                    first_missing.get_or_insert((x, y, field.height(p)));
                }
            }
        }
        println!(
            "{name}: {expected} sea pixels expected, {missing} not water (first {first_missing:?}), {} water nodes ({} before the cut, {} with the exact minimum) of {}",
            wp.nodes.len(),
            full.nodes.len(),
            exact.nodes.len(),
            plan.nodes.len()
        );
        let least = match name {
            "inland" => 50,
            "beach" => 100,
            _ => 2000,
        };
        assert!(expected > least, "{name}: only {expected} pixels to check: the view does not look at enough sea");
        // The edge allowance: the shore of the water mesh and the tile edges, 1 % of the pixels.
        assert!(flooded * 100 <= dry_checked.max(1), "{name}: {flooded} of {dry_checked} pixels over land above the level are water");
        assert!(
            missing * 100 <= expected,
            "{name}: {missing} of {expected} ocean-connected sea pixels are not water (first {first_missing:?})"
        );
    }
    assert!(exact_cut > 0, "the exact minimum cut no node the loose bound kept in any view");
}

// spec: ATMO-019, ATMO-020
#[test]
fn the_water_class_covers_every_ocean_connected_sea_pixel_of_a_real_world_out_to_the_horizon() {
    coverage(&["aerial", "low"]);
}

// spec: ATMO-019, ATMO-020
#[test]
fn the_water_class_covers_the_sea_at_a_beach_and_inland_and_the_exact_dry_tile_cut_draws_the_same_pixels() {
    coverage(&["beach", "inland"]);
}
