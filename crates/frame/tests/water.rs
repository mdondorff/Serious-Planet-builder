//! Tier A: the water surface's CPU side (ADR 0041 points 4 to 6, ADR 0051 point 6, ADR 0053): the reduced wave phases, the waves'
//! seams and bounds, the shoreline functions, the per-tile water selection, the plan's text form and the coastal flicker metric.

use planet_core::cube::TangentWarp;
use planet_core::hash::hash_u64s;
use planet_core::tile::Side;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::look::file::LookParams;
use planet_frame::look::water::{
    class_flicker, class_flicker_both, foam_amount, shore_cover, surface_radiance, tile_phase, tile_phases, water_class, wave_argument,
    wave_normal, wave_sample, wave_weights, ClassMap, LiquidAppearance, SurfaceInputs, WaterClass, WaterLook, Wave, WavePoint,
    FLICKER_BUDGET, MAX_LIQUIDS, TANGENT_SHARE, WAVES, WAVE_ARGUMENT_BOUND, WAVE_DIRECTIONS,
};
use planet_frame::water::{plan_water, WaterError, WaterPlan, WaterRequest, WaterSource, WaterTileCache, WaterView, TILE_SAMPLES};
use planet_frame::{plan_terrain, scripted_views, Camera, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::global::ReadError;
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_world_def::{LiquidKind, LiquidKindId, LiquidTable, WorldDefinition};

const R: f64 = 6_371_000.0;

fn water_liquid() -> LiquidAppearance {
    LiquidAppearance::new(0, &LiquidKind::WATER, &WaterLook::EARTH)
}

// spec: ATMO-013
#[test]
fn a_waves_tile_phase_is_reduced_in_f64_to_known_answers_at_planet_sized_origins_and_at_a_million_seconds() {
    let w = Wave { k: [0.1, 0.0, 0.0], amplitude: 0.5, phi: 0.5, omega: 2.0 };
    let o = [6_371_000.0, 0.0, 0.0];
    // Known answers from 50-digit arithmetic (the sum is about 6e5 rad: f32 would hold 0.04 rad of it).
    for (t, want) in [(0.0, 4.359_407_911_469_999), (1.5, 1.359_407_911_469_999_2), (1.0e6, 5.074_536_245_641_469)] {
        let got = tile_phase(&w, o, t);
        assert!((got - want).abs() < 1.0e-8, "t = {t}: {got} vs {want}");
        assert!((0.0..core::f64::consts::TAU).contains(&got));
    }
    let w2 = Wave { k: [0.0125, 0.0, 0.0], amplitude: 0.5, phi: 0.5, omega: 2.0 };
    let got = tile_phase(&w2, [-3.25e6, 0.0, 0.0], 1.0e6);
    assert!((got - 3.291_324_557_377_63).abs() < 1.0e-8, "{got}");
    // One period later the phase is the same (the clock's planet-sized part is gone).
    let period = core::f64::consts::TAU / w.omega;
    let a = tile_phase(&w, o, 1.0e6);
    let b = tile_phase(&w, o, 1.0e6 + 1000.0 * period);
    let d = (a - b).abs().min(core::f64::consts::TAU - (a - b).abs());
    assert!(d < 1.0e-6, "{d}");
    // The f32 the shader receives is within half an ulp of the reduced value (a number below 2 pi: 5e-7).
    let l = water_liquid();
    for p in tile_phases(&l, [1.0e6, 5.0e6, -2.0e6], 123_456.789) {
        assert!((0.0..=core::f32::consts::TAU).contains(&p), "{p}");
    }
}

// spec: ATMO-013
#[test]
fn two_tiles_that_hold_one_point_agree_about_its_waves_also_across_a_face_edge() {
    let l = water_liquid();
    let f = 40.0;
    let field = Reference(1);
    let mut checked = 0;
    // A tile at the edge of face 0 and its neighbour across the edge (another face, another frame), and an interior pair.
    let edge = TileId::new(planet_core::Face(0), 7, 127, 40).unwrap();
    let inner = TileId::new(planet_core::Face(2), 8, 100, 100).unwrap();
    for (tile, side) in [(edge, Side::East), (inner, Side::East), (inner, Side::North)] {
        let nb = tile.neighbor(&TangentWarp, side);
        // A point of the shared border: the tile's sample on that side, which the neighbour also holds.
        let n = 4;
        for k in 0..=n {
            let d = match side {
                Side::East => tile.sample_direction(&TangentWarp, n, n, k),
                Side::North => tile.sample_direction(&TangentWarp, n, k, n),
                _ => unreachable!(),
            };
            // The water at this point: depth 3 m, camera 400 m away; evaluated from each tile's origin with f32-rounded local numbers.
            let eval = |t: TileId| {
                let o = t.center_direction(&TangentWarp) * (R + field.height(t.center_direction(&TangentWarp)));
                let p = d * (R + 0.0);
                let local_f64 = p - o;
                let local = [local_f64.x as f32, local_f64.y as f32, local_f64.z as f32].map(f64::from);
                let phases = tile_phases(&l, [o.x, o.y, o.z], 77.7).map(f64::from);
                let up = d;
                let point = WavePoint { local, up: [up.x, up.y, up.z], phases, spacing: 2.0, depth: f };
                (wave_sample(&l, &point, 0.8), local_f64.length())
            };
            let (a, la) = eval(tile);
            let (b, lb) = eval(nb);
            assert!(la < 1.0e6 && lb < 1.0e6);
            let bound = 1.0e-3 + 1.0e-3 * l.amplitude_sum();
            for c in 0..3 {
                assert!((a.disp[c] - b.disp[c]).abs() <= bound, "{tile:?} {side:?} {k}: {:?} vs {:?}", a.disp, b.disp);
                assert!((a.slope[c] - b.slope[c]).abs() <= 1.0e-3, "{:?} vs {:?}", a.slope, b.slope);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 15);
}

fn point(local: [f64; 3], up: [f64; 3], spacing: f64, depth: f64) -> WavePoint {
    WavePoint { local, up, phases: [0.3, 1.7, 2.2, 5.1], spacing, depth }
}

// spec: ATMO-013
#[test]
fn a_waves_weight_is_continuous_and_a_wave_that_the_grid_cannot_hold_or_that_points_out_of_the_surface_has_none() {
    let l = water_liquid();
    let up = [0.0, 0.0, 1.0];
    // A wave on a grid of fewer than four vertices per wavelength is gone; from eight it is whole.
    for (i, w) in l.waves.iter().enumerate() {
        let lambda = core::f64::consts::TAU / (w.k.iter().map(|c| c * c).sum::<f64>()).sqrt();
        let near = wave_weights(&l, &point([0.0; 3], up, lambda / 9.0, 100.0))[i];
        let far = wave_weights(&l, &point([0.0; 3], up, lambda / 3.9, 100.0))[i];
        let mid = wave_weights(&l, &point([0.0; 3], up, lambda / 6.0, 100.0))[i];
        // The tangent share of tetrahedron wave i at this up decides the rest.
        let kn = WAVE_DIRECTIONS[i][2];
        let s = (1.0 - kn * kn).sqrt();
        let tan = {
            let t = ((s - TANGENT_SHARE.0) / (TANGENT_SHARE.1 - TANGENT_SHARE.0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        assert!((near - tan).abs() < 1.0e-12 && far == 0.0 && mid > 0.0 && mid < near, "wave {i}: {near} {mid} {far} {tan}");
    }
    // Along the wave's own direction the wave has no tangent share: a surface whose up is wave 0's direction does not carry it.
    let up0 = WAVE_DIRECTIONS[0];
    assert_eq!(wave_weights(&l, &point([0.0; 3], up0, 1.0, 100.0))[0], 0.0);
    // Continuity: a tiny move changes the weights tiny.
    let a = wave_weights(&l, &point([0.0; 3], up, 5.0, 0.8));
    let b = wave_weights(&l, &point([0.0; 3], up, 5.001, 0.8001));
    for i in 0..WAVES {
        assert!((a[i] - b[i]).abs() < 1.0e-4, "{i}: {} {}", a[i], b[i]);
    }
    // No waves for a liquid with no amplitude (a lava lake).
    let lava = LiquidAppearance::new(
        1,
        &LiquidKind {
            colour: [0.8, 0.1, 0.02],
            emission: [40.0, 9.0, 0.5],
            viscosity_pa_s: 1.0e3,
            wave_amplitude_m: 0.0,
            wave_length_m: 0.0,
        },
        &WaterLook::EARTH,
    );
    assert!(!lava.has_waves());
    let s = wave_sample(&lava, &point([1.0, 2.0, 3.0], up, 1.0, 50.0), 0.8);
    assert_eq!((s.disp, s.slope), ([0.0; 3], [0.0; 3]));
}

// spec: ATMO-013, ATMO-015
#[test]
fn the_waves_stay_within_their_amplitudes_and_never_lift_the_surface_below_the_seabed_where_the_water_is_thin() {
    let l = water_liquid();
    let total = l.amplitude_sum();
    let mut worst_ratio: f64 = 0.0;
    let mut n = 0;
    for ix in 0..40 {
        for depth in [0.001, 0.05, 0.3, 1.0, 1.9, 2.0, 5.0, 40.0] {
            let local = [f64::from(ix) * 1.37 - 20.0, f64::from(ix) * 0.61, f64::from(ix % 7)];
            let up = {
                let v = Vec3::new(0.3 + 0.01 * f64::from(ix), -0.5, 0.8).normalized();
                [v.x, v.y, v.z]
            };
            let p = WavePoint { local, up, phases: [0.1 * f64::from(ix), 2.0, 3.0, 4.0], spacing: 1.0, depth };
            let s = wave_sample(&l, &p, 0.8);
            let vertical = s.disp[0] * up[0] + s.disp[1] * up[1] + s.disp[2] * up[2];
            assert!(vertical.abs() <= total + 1e-12, "{vertical} against {total}");
            // The vertical displacement is at most half the depth: the surface stays above the terrain where the water is thin.
            assert!(vertical >= -0.5 * depth - 1e-12, "depth {depth}: the surface dips to {vertical}");
            worst_ratio = worst_ratio.max(-vertical / depth);
            n += 1;
        }
    }
    assert_eq!(n, 320);
    assert!(worst_ratio <= 0.5 + 1e-9, "{worst_ratio}");
    // The shading normal is a unit vector close to up for these slopes.
    let p = point([3.0, 1.0, 0.0], [0.0, 0.0, 1.0], 100.0, 40.0);
    let s = wave_sample(&l, &p, 0.8);
    let nrm = wave_normal(p.up, s.slope);
    assert!(((nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]) - 1.0).abs() < 1e-12 && nrm[2] > 0.9, "{nrm:?}");
}

// spec: ATMO-013
#[test]
fn the_argument_the_shader_adds_stays_small_wherever_a_wave_has_weight_on_real_plans() {
    // Every grid vertex of every water tile of the scripted views, orbit to 1 m: |k . local| where the wave has weight.
    let field = Reference(1);
    let look = WaterLook::EARTH;
    let table = LiquidTable::earth();
    let cells = 32;
    let lod = LodParams { cells, viewport_h_px: 1080.0, tau_px: 6.0, max_level: 22, ..LodParams::earth_1080p() };
    let src = HeightSea { field: &field, level: 0.0 };
    let l = LiquidAppearance::new(0, table.get(LiquidKindId::WATER).unwrap(), &look);
    let mut worst: f64 = 0.0;
    let mut weighted_vertices = 0u64;
    for (name, camera) in scripted_views(R, 16.0 / 9.0, 1) {
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size: (1920, 1080),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        let wp = plan_water(
            &WaterRequest { source: &src, map: &TangentWarp, liquids: &table, look: &look, lod: &lod, time_s: 0.0, view: WaterView::Lit },
            &mut WaterTileCache::new(),
            &plan,
        )
        .unwrap();
        for n in &wp.nodes {
            let d = &plan.nodes[n.node as usize];
            let mesh = tile_mesh_field(&field, &TangentWarp, d.tile, cells, R);
            for v in mesh.vertices.iter().take(((cells + 1) * (cells + 1)) as usize) {
                let pos = Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
                let depth = 0.0 - f64::from(v.height);
                // Up at the vertex from the tile-local numbers, as the shader forms it.
                let o = d.origin.0;
                let inv_r = 1.0 / o.length();
                let upv = (o.normalized() + pos * inv_r).normalized();
                let up = [upv.x, upv.y, upv.z];
                let base = pos + upv * depth;
                let spacing = f64::from(n.spacing_m);
                let p = WavePoint { local: [base.x, base.y, base.z], up, phases: [0.0; WAVES], spacing, depth: depth.max(0.0) };
                let weights = wave_weights(&l, &p);
                for (i, w) in l.waves.iter().enumerate() {
                    if weights[i] > 0.0 {
                        let arg = w.k[0] * base.x + w.k[1] * base.y + w.k[2] * base.z;
                        worst = worst.max(arg.abs());
                        weighted_vertices += 1;
                    }
                }
            }
        }
        let _ = name;
    }
    assert!(weighted_vertices > 1000, "the views see waves: {weighted_vertices}");
    assert!(worst < WAVE_ARGUMENT_BOUND / 4.0, "the largest |k . local| of a weighted wave is {worst} rad, bound {WAVE_ARGUMENT_BOUND}");
    println!("largest |k . local| of a weighted wave over the scripted views: {worst:.1} rad ({weighted_vertices} weighted wave vertices)");
}

struct HeightSea<'a> {
    field: &'a Reference,
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

/// A source with a table of answers by direction octant, to pin the selection rule.
struct Scripted<F: Fn(Vec3) -> Option<WaterSample>>(F);

impl<F: Fn(Vec3) -> Option<WaterSample>> WaterSource for Scripted<F> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.0)(dir))
    }
}

fn plan_of(index: usize) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: (128, 96), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
        .unwrap()
}

fn lod16() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

fn request<'a>(src: &'a dyn WaterSource, table: &'a LiquidTable, look: &'a WaterLook, lod: &'a LodParams, t: f64) -> WaterRequest<'a> {
    WaterRequest { source: src, map: &TangentWarp, liquids: table, look, lod, time_s: t, view: WaterView::Lit }
}

// spec: ATMO-012
#[test]
fn a_tile_carries_water_when_any_sample_of_it_is_wet_and_the_answer_is_asked_once_per_tile() {
    let field = Reference(1);
    let src = HeightSea { field: &field, level: 0.0 };
    let (table, look, lod) = (LiquidTable::earth(), WaterLook::EARTH, lod16());
    let plan = plan_of(4);
    let mut cache = WaterTileCache::new();
    let wp = plan_water(&request(&src, &table, &look, &lod, 5.0), &mut cache, &plan).unwrap();
    assert!(!wp.nodes.is_empty() && wp.nodes.len() < plan.nodes.len(), "{} of {}", wp.nodes.len(), plan.nodes.len());
    let samples = u64::from((TILE_SAMPLES + 1) * (TILE_SAMPLES + 1));
    assert_eq!(cache.queries, samples * plan.nodes.len() as u64, "one query per sample of every tile the first time");
    assert_eq!(cache.tile_misses, plan.nodes.len() as u64);
    // Exactly the tiles with a wet sample.
    for (i, d) in plan.nodes.iter().enumerate() {
        let wet = (0..=TILE_SAMPLES)
            .any(|l| (0..=TILE_SAMPLES).any(|k| field.height(d.tile.sample_direction(&TangentWarp, TILE_SAMPLES, k, l)) <= 0.0));
        assert_eq!(wp.nodes.iter().any(|n| n.node as usize == i), wet, "node {i} {:?}", d.tile);
    }
    // The second plan asks nothing, and is identical (a pure function of the plan, the field, the table, the look and the time).
    let before = cache.queries;
    let again = plan_water(&request(&src, &table, &look, &lod, 5.0), &mut cache, &plan).unwrap();
    assert_eq!(cache.queries, before);
    assert_eq!(again, wp);
    assert_eq!(plan_water(&request(&src, &table, &look, &lod, 5.0), &mut WaterTileCache::new(), &plan).unwrap(), wp);
    // The nodes name the liquid slot, the level and the phases of their own origin at the plan's time.
    let l = LiquidAppearance::new(0, table.get(LiquidKindId::WATER).unwrap(), &look);
    assert_eq!(wp.liquids, vec![l]);
    for n in &wp.nodes {
        let o = plan.nodes[n.node as usize].origin.0;
        assert_eq!((n.liquid, n.level_m), (0, 0.0));
        assert_eq!(n.phases, tile_phases(&l, [o.x, o.y, o.z], 5.0));
    }
    // A later time moves the phases and nothing else.
    let later = plan_water(&request(&src, &table, &look, &lod, 6.0), &mut cache, &plan).unwrap();
    assert_eq!(later.nodes.len(), wp.nodes.len());
    assert!(later.nodes.iter().zip(&wp.nodes).all(|(a, b)| a.node == b.node && a.phases != b.phases));
    assert_eq!(wp.validate(), Ok(()));
}

// spec: ATMO-012
#[test]
fn a_tile_with_two_bodies_takes_the_most_frequent_and_a_tie_the_lower_id_and_a_missing_liquid_is_refused() {
    let table = LiquidTable::earth();
    let (look, lod) = (WaterLook::EARTH, lod16());
    let plan = plan_of(3);
    // West half sea (body 1), east half lake (body 9, level 12 m): the tiles that straddle take the majority of their samples.
    let src = Scripted(|d: Vec3| {
        Some(if d.y < 0.0 {
            WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }
        } else {
            WaterSample { level_m: 12.0, body: WaterBodyId(9), kind: LiquidKindId::WATER }
        })
    });
    let mut cache = WaterTileCache::new();
    let wp = plan_water(&request(&src, &table, &look, &lod, 0.0), &mut cache, &plan).unwrap();
    for n in &wp.nodes {
        let t = plan.nodes[n.node as usize].tile;
        let wet: Vec<f64> = (0..=TILE_SAMPLES)
            .flat_map(|l| (0..=TILE_SAMPLES).map(move |k| (k, l)))
            .map(|(k, l)| if t.sample_direction(&TangentWarp, TILE_SAMPLES, k, l).y < 0.0 { 0.0 } else { 12.0 })
            .collect();
        let sea = wet.iter().filter(|v| **v == 0.0).count();
        let want = if sea > wet.len() - sea || (sea == wet.len() - sea) { 0.0 } else { 12.0 };
        assert_eq!(f64::from(n.level_m), want, "{t:?}: {sea} of {} samples are sea", wet.len());
    }
    // An unknown liquid names its id.
    let bad = Scripted(|_d: Vec3| Some(WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId(7) }));
    let e = plan_water(&request(&bad, &table, &look, &lod, 0.0), &mut WaterTileCache::new(), &plan).unwrap_err();
    assert_eq!(e, WaterError::UnknownLiquid(LiquidKindId(7)));
    assert!(e.to_string().contains("liquid 7"), "{e}");
    // More liquids than the pass draws is refused naming both numbers.
    let mut def = WorldDefinition::single_planet(R, 1);
    for k in 0..=MAX_LIQUIDS as u32 {
        def.set_liquid(LiquidKindId(k), &LiquidKind::WATER).unwrap();
    }
    let many_table = def.liquids().unwrap();
    let many = Scripted(|d: Vec3| {
        let k = ((libm::floor(d.x * 40.0) as i64 + 3 * libm::floor(d.y * 40.0) as i64).rem_euclid(MAX_LIQUIDS as i64 + 1)) as u32;
        Some(WaterSample { level_m: 0.0, body: WaterBodyId::SEA, kind: LiquidKindId(k) })
    });
    let e = plan_water(&request(&many, &many_table, &look, &lod, 0.0), &mut WaterTileCache::new(), &plan_of(2)).unwrap_err();
    assert!(matches!(e, WaterError::TooManyLiquids { max, .. } if max == MAX_LIQUIDS), "{e}");
    // A level that is not a height is refused naming the tile.
    let high = Scripted(|_d: Vec3| Some(WaterSample { level_m: 6.0e6, body: WaterBodyId::SEA, kind: LiquidKindId::WATER }));
    let e = plan_water(&request(&high, &table, &look, &lod, 0.0), &mut WaterTileCache::new(), &plan).unwrap_err();
    assert!(matches!(e, WaterError::BadLevel { .. }), "{e}");
}

// spec: ATMO-012
#[test]
fn the_water_plan_round_trips_through_the_terrain_plan_text_and_a_plan_without_water_keeps_its_text() {
    let field = Reference(1);
    let src = HeightSea { field: &field, level: 0.0 };
    let (table, look, lod) = (LiquidTable::earth(), LookParams::default().water, lod16());
    let mut plan = plan_of(4);
    let plain_text = plan.to_text();
    assert!(!plain_text.contains("water") && !plain_text.contains("wnode"), "a plan without water has no water line");
    plan.water = Some(plan_water(&request(&src, &table, &look, &lod, 9.25), &mut WaterTileCache::new(), &plan).unwrap());
    let text = plan.to_text();
    assert!(text.contains("\nwater time ") && text.contains("\nliquid 0 ") && text.contains("\nwnode "));
    let back = TerrainPlan::from_text(&text).unwrap();
    assert_eq!(back, plan);
    assert_eq!(back.to_text(), text);
    assert_ne!(back.plan_hash(), TerrainPlan::from_text(&plain_text).unwrap().plan_hash());
    // A truncated water line is refused naming it.
    let broken = text.replace("\nwnode ", "\nwnode x ");
    assert!(TerrainPlan::from_text(&broken).is_err());
    let _ = hash_u64s;
}

// spec: ATMO-015
#[test]
fn the_shoreline_functions_are_zero_at_the_waterline_continuous_and_discrete_by_class() {
    let look = WaterLook::EARTH;
    assert_eq!(water_class(0.0, &look), WaterClass::None);
    assert_eq!(water_class(-3.0, &look), WaterClass::None);
    assert_eq!(water_class(0.001, &look), WaterClass::Film);
    assert_eq!(water_class(0.1, &look), WaterClass::Foam);
    assert_eq!(water_class(1.0, &look), WaterClass::Shallow);
    assert_eq!(water_class(5.0, &look), WaterClass::Mid);
    assert_eq!(water_class(50.0, &look), WaterClass::Deep);
    assert_eq!(shore_cover(0.0, &look, 0.0), 0.0);
    assert_eq!(shore_cover(look.shore_fade_m, &look, 0.0), 1.0);
    assert_eq!(foam_amount(0.0, &look, 1.0, 0.0), 0.0, "no foam on the waterline itself");
    assert_eq!(foam_amount(look.foam_width_m, &look, 1.0, 0.0), 0.0);
    assert!(foam_amount(0.15 * look.foam_width_m, &look, 1.0, 0.0) > 0.7 * look.foam_strength);
    // Continuity: no step anywhere on a fine scan; the largest step is far below one grey level of the foam.
    let mut prev = (shore_cover(0.0, &look, 0.0), foam_amount(0.0, &look, 0.5, 0.0));
    let mut worst: f64 = 0.0;
    for k in 1..=4000 {
        let d = f64::from(k) * 0.001;
        let cur = (shore_cover(d, &look, 0.0), foam_amount(d, &look, 0.5, 0.0));
        worst = worst.max((cur.0 - prev.0).abs()).max((cur.1 - prev.1).abs());
        prev = cur;
    }
    assert!(worst < 0.03, "the largest step per millimetre of depth: {worst}");
    // The classes are ordered by depth.
    let mut last = WaterClass::None;
    for k in 0..2000 {
        let c = water_class(f64::from(k) * 0.01, &look);
        assert!(c >= last);
        last = c;
    }
    assert_eq!(WaterClass::COLOURS.len(), 6);
}

fn inputs(depth: f64) -> SurfaceInputs {
    SurfaceInputs {
        n: [0.0, 0.0, 1.0],
        v: Vec3::new(0.0, 0.5, 0.8).normalized().into_array(),
        up: [0.0, 0.0, 1.0],
        sun: Vec3::new(0.0, -0.5, 0.8).normalized().into_array(),
        depth,
        foam_mod: 0.5,
        depth_width: 0.0,
        seabed: [0.2, 0.18, 0.12],
        diffuse: [0.25, 0.25, 0.25],
        sun_t: [0.9, 0.85, 0.8],
        sky: [0.3, 0.45, 0.8],
    }
}

trait IntoArray {
    fn into_array(self) -> [f64; 3];
}

impl IntoArray for Vec3 {
    fn into_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

// spec: ATMO-015
#[test]
fn the_surface_shows_the_seabed_at_the_waterline_and_the_liquid_in_deep_water_and_a_lava_lake_glows() {
    let look = WaterLook::EARTH;
    let l = water_liquid();
    // At the waterline the surface layer has not faded in: the pixel is the seabed (foam is zero there too).
    let at = surface_radiance(&l, &look, &inputs(0.0));
    for c in 0..3 {
        assert!((at[c] - inputs(0.0).seabed[c]).abs() < 1e-12, "{at:?}");
    }
    // Deep water shows the liquid's own colour under the same light plus the reflections; the seabed does not show through it.
    let deep_a = surface_radiance(&l, &look, &inputs(200.0));
    let mut other = inputs(200.0);
    other.seabed = [0.9, 0.9, 0.9];
    let deep_b = surface_radiance(&l, &look, &other);
    for c in 0..3 {
        assert!((deep_a[c] - deep_b[c]).abs() < 1e-9, "the seabed does not show through 200 m of water");
    }
    // Red is absorbed first: at 3 m the red of a white seabed is below its blue.
    let mut white = inputs(3.0);
    white.seabed = [1.0, 1.0, 1.0];
    white.sky = [0.0; 3];
    white.sun_t = [0.0; 3];
    let w3 = surface_radiance(&l, &look, &white);
    assert!(w3[0] < w3[2], "{w3:?}");
    // Fresnel: grazing views reflect the sky more than looking down.
    let mut grazing = inputs(200.0);
    grazing.v = Vec3::new(0.0, 0.999, 0.04).normalized().into_array();
    let mut down = inputs(200.0);
    down.v = [0.0, 0.0, 1.0];
    assert!(surface_radiance(&l, &look, &grazing)[2] > surface_radiance(&l, &look, &down)[2]);
    // The sun's glint peaks in the mirror direction and is gone away from it.
    // `inputs` looks along the sun's mirror direction: the glint. Looking away from it the glint is gone.
    let m = surface_radiance(&l, &look, &inputs(200.0));
    let mut away = inputs(200.0);
    away.v = Vec3::new(0.0, -0.6, 0.8).normalized().into_array();
    let off = surface_radiance(&l, &look, &away);
    assert!(m[0] > 10.0 * off[0], "glint {} against {}", m[0], off[0]);
    // A lava lake (no waves, high viscosity, emission) is bright where a water lake of the same depth is dark.
    let lava = LiquidAppearance::new(
        1,
        &LiquidKind {
            colour: [0.8, 0.1, 0.02],
            emission: [40.0, 9.0, 0.5],
            viscosity_pa_s: 1.0e3,
            wave_amplitude_m: 0.0,
            wave_length_m: 0.0,
        },
        &look,
    );
    let mut view = inputs(5.0);
    view.v = Vec3::new(0.0, -0.6, 0.8).normalized().into_array();
    let glow = surface_radiance(&lava, &look, &view);
    let dark = surface_radiance(&l, &look, &view);
    assert!(glow[0] > 10.0 && dark[0] < 2.0, "{glow:?} against {dark:?}");
    assert!(glow[0] > glow[1] && glow[1] > glow[2], "the glow is red: {glow:?}");
    assert_eq!(lava.waves[0].amplitude, 0.0);
}

// spec: ATMO-017
#[test]
fn the_flicker_metric_counts_class_changes_after_reprojection_and_not_the_camera_motion() {
    let size = (192u32, 128u32);
    let radius = R;
    let look = WaterLook::EARTH;
    // A synthetic coast: the depth of the water surface point P is its distance from a shoreline great circle (a plane across the view),
    // in metres, so the class bands are stripes on the sea.
    let depth_at = |p: Vec3| 0.05 * (p.y - 15.0);
    let class_map = |cam: &Camera| {
        let (right, up, fwd) = cam.ray_frame();
        let mut c = vec![0u8; (size.0 * size.1) as usize];
        for y in 0..size.1 {
            for x in 0..size.0 {
                let (u, v) = (2.0 * (f64::from(x) + 0.5) / f64::from(size.0) - 1.0, 1.0 - 2.0 * (f64::from(y) + 0.5) / f64::from(size.1));
                let dir = (fwd + right * u + up * v).normalized();
                let from = cam.position.0;
                let b = from.dot(dir);
                let l = from.length();
                let cc = (l - radius) * (l + radius);
                let disc = b * b - cc;
                if disc < 0.0 {
                    continue;
                }
                let t = -b - disc.sqrt();
                if t <= 0.0 {
                    continue;
                }
                let d = depth_at(from + dir * t);
                c[(y * size.0 + x) as usize] = water_class(d, &look) as u8;
            }
        }
        ClassMap::new(size, c)
    };
    let cam = |lat: f64, alt: f64, heading: f64| {
        let pos = PlanetFixed(Vec3::new(libm::cos(lat) * libm::cos(0.0), libm::cos(lat) * libm::sin(0.0), libm::sin(lat)) * (R + alt));
        Camera::at_surface_point(pos, Vec3::new(0.0, libm::cos(heading), libm::sin(heading)), 0.21, 60f64.to_radians(), 1.5, 0.1)
    };
    let a = cam(0.0, 12.0, 0.2);
    let b = cam(0.0, 11.0, 0.205);
    let (ma, mb) = (class_map(&a), class_map(&b));
    let covered = ma.class.iter().filter(|c| **c != 0).count();
    assert!(covered > 3000, "{covered} water pixels in the first frame");
    // The same frame twice, and a frame after a real camera move: nothing changes class but at the pixel-wide band edges.
    let still = class_flicker(&ma, &a, &ma, &a, radius, None);
    assert_eq!((still.changed, still.changed_fine, still.covered + still.unresolved), (0, 0, covered as u64));
    let moved = class_flicker_both(&ma, &a, &mb, &b, radius, None);
    assert!(moved.covered > 3000, "{moved:?}");
    assert!(moved.share() <= FLICKER_BUDGET, "a clean move shows {:.4} % flicker ({moved:?})", moved.share() * 100.0);
    // A renderer that drops one water pixel in twenty (isolated dry pixels in the water: the coastline's flicker) is caught far above
    // the budget; one that flips the class of those pixels is caught by the fine count.
    let mut noisy = mb.clone();
    let mut recoloured = mb.clone();
    let mut flipped = 0;
    for i in 0..noisy.class.len() {
        if noisy.class[i] != 0 && hash_u64s(7, &[i as u64]).is_multiple_of(20) {
            recoloured.class[i] = if noisy.class[i] == WaterClass::Deep as u8 { WaterClass::Film as u8 } else { WaterClass::Deep as u8 };
            noisy.class[i] = 0;
            flipped += 1;
        }
    }
    assert!(flipped > 20);
    let bad = class_flicker_both(&ma, &a, &noisy, &b, radius, None);
    assert!(bad.share() > 10.0 * FLICKER_BUDGET, "{bad:?}");
    let fine = class_flicker_both(&ma, &a, &recoloured, &b, radius, None);
    assert!(fine.changed_fine > 20 && fine.changed == 0, "a recoloured pixel is still water: {fine:?}");
    // The metric counts a point that left the frame separately, and a frame with no water has no share.
    let away = cam(0.0, 12.0, 3.1);
    let gone = class_flicker(&ma, &a, &class_map(&away), &away, radius, None);
    assert!(gone.left_frame > 0);
    let blank = ClassMap::new(size, vec![0; (size.0 * size.1) as usize]);
    assert_eq!(class_flicker(&blank, &a, &blank, &b, radius, None).share(), 0.0, "no water in either frame: no share");
    // Water in the second frame where the first shows none (a coastline that jumped) is a hole of the first: counted.
    assert!(class_flicker(&blank, &a, &mb, &b, radius, None).share() > 0.5);
}

// spec: ATMO-013
#[test]
fn a_water_plan_that_names_a_liquid_slot_or_a_phase_that_cannot_exist_is_refused_before_the_gpu() {
    let field = Reference(1);
    let src = HeightSea { field: &field, level: 0.0 };
    let (table, look, lod) = (LiquidTable::earth(), WaterLook::EARTH, lod16());
    let plan = plan_of(4);
    let good: WaterPlan = plan_water(&request(&src, &table, &look, &lod, 1.0), &mut WaterTileCache::new(), &plan).unwrap();
    assert_eq!(good.validate(), Ok(()));
    let mut bad = good.clone();
    bad.nodes[0].liquid = 3;
    assert!(bad.validate().unwrap_err().contains("liquid slot 3"));
    let mut bad = good.clone();
    bad.nodes[0].phases[2] = 7.0;
    assert!(bad.validate().unwrap_err().contains("phase"));
    let mut bad = good.clone();
    bad.nodes[0].level_m = f32::NAN;
    assert!(bad.validate().unwrap_err().contains("level"));
    let mut bad = good;
    bad.look.foam_width_m = 0.0;
    assert!(bad.validate().unwrap_err().contains("foam_width_m"));
    // The argument reduction used by the shader brings any argument into [-pi, pi].
    let w = Wave { k: [0.2, 0.0, 0.0], amplitude: 0.1, phi: 0.0, omega: 1.0 };
    for k in 0..100 {
        let a = wave_argument(&w, [f64::from(k) * 37.1, 0.0, 0.0], 3.0);
        assert!(a.abs() <= core::f64::consts::PI + 1e-12);
    }
}

// spec: ATMO-016
#[test]
fn a_field_that_names_a_water_level_keeps_the_tiles_of_water_above_a_seabed_the_view_does_not_see() {
    use planet_generators::water::WaterBounded;
    let field = Reference(1);
    // A point over deep water: march east along 20 N until the terrain is 40 m under the sea.
    let at = |s: f64| {
        let lon = 30f64.to_radians() + s / (R * libm::cos(20f64.to_radians()));
        let la = 20f64.to_radians();
        Vec3::new(libm::cos(la) * libm::cos(lon), libm::cos(la) * libm::sin(lon), libm::sin(la))
    };
    let s = (0..4000).map(|k| f64::from(k) * 500.0).find(|s| field.height(at(*s)) < -40.0).expect("deep water");
    let dir = at(s);
    let camera =
        Camera::at_surface_point(PlanetFixed(dir * (R + 6.0)), Vec3::new(0.0, 0.0, 1.0), 6f64.to_radians(), 60f64.to_radians(), 1.6, 0.1);
    let lod = LodParams { cells: 16, viewport_h_px: 256.0, tau_px: 4.0, max_level: 26, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let request = TerrainRequest { camera, size: (192, 120), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let plain = planet_frame::plan_terrain_field(&request, &field).unwrap();
    let bounded = WaterBounded::borrowed(&field, 0.0);
    let kept = planet_frame::plan_terrain_field(&request, &bounded).unwrap();
    let nearest = |p: &TerrainPlan| p.nodes.iter().map(|n| (n.origin.0 - camera.position.0).length()).fold(f64::INFINITY, f64::min);
    assert!(kept.nodes.len() > plain.nodes.len(), "{} tiles against {}", kept.nodes.len(), plain.nodes.len());
    assert!(nearest(&kept) < 0.8 * nearest(&plain), "the nearest tile is {:.1} m away, against {:.1} m", nearest(&kept), nearest(&plain));
    // One selection cache serving a field with and without a water level gives the plans of fresh caches (the level is in its key).
    let mut shared = planet_frame::SelectionCache::new();
    let first = planet_frame::plan_terrain_cached(&request, &field, &mut shared).unwrap();
    let second = planet_frame::plan_terrain_cached(&request, &bounded, &mut shared).unwrap();
    assert_eq!(first.nodes.len(), plain.nodes.len());
    assert_eq!(second.nodes, kept.nodes, "a cache that saw the plain field must not hand its volumes to the bounded one");
    // Over deep water the grid under the camera is spaced for the seabed's distance: the waves that grid cannot hold (fewer than four
    // vertices per wavelength at its own spacing) have no weight, and the argument of those that have stays small, tile by tile.
    let l = water_liquid();
    let lod16 =
        LodParams { cells: 16, viewport_h_px: 256.0, tau_px: 4.0, max_level: 26, max_height_m: 40_000.0, ..LodParams::earth_1080p() };
    let mut held = 0;
    for n in &kept.nodes {
        let spacing = lod16.sample_spacing(n.level);
        for w in &l.waves {
            let klen = (w.k.iter().map(|c| c * c).sum::<f64>()).sqrt();
            let lambda = core::f64::consts::TAU / klen;
            let p = WavePoint { local: [0.0; 3], up: [0.0, 0.0, 1.0], phases: [0.0; WAVES], spacing, depth: 100.0 };
            let weight = wave_weights(&l, &p)[l.waves.iter().position(|x| x == w).unwrap()];
            if weight > 0.0 {
                held += 1;
                assert!(lambda >= 4.0 * spacing, "level {}: a wave of {lambda} m on a grid of {spacing} m", n.level);
                assert!(klen * lod16.node_radius_bound(n.level) < WAVE_ARGUMENT_BOUND);
            }
        }
    }
    assert!(held > 0);
    // Heights, keys and bounds are the field's own: every tile of the plain plan is in the kept one with the same mesh key.
    for n in &plain.nodes {
        let k = kept.nodes.iter().find(|m| m.tile == n.tile).expect("a tile of the plain plan is kept");
        assert_eq!((k.mesh_key, k.origin), (n.mesh_key, n.origin));
    }
}

// spec: ATMO-012, ATMO-004
#[test]
fn a_sea_level_other_than_zero_moves_the_water_nodes_and_the_level_the_plan_carries() {
    // ADR 0053 point 4: the field's wet tiles follow the sea's own level (a raised sea touches more tiles, a lowered one fewer), every
    // node carries that level, and the pass then draws water wherever the level is above the terrain.
    let field = Reference(1);
    let (table, look, lod) = (LiquidTable::earth(), WaterLook::EARTH, lod16());
    let plan = plan_of(4);
    let nodes_at = |level: f64| {
        let src = HeightSea { field: &field, level };
        let wp = plan_water(&request(&src, &table, &look, &lod, 0.0), &mut WaterTileCache::new(), &plan).unwrap();
        assert!(wp.nodes.iter().all(|n| f64::from(n.level_m) == level), "every node carries the level {level}");
        wp.nodes.iter().map(|n| n.node).collect::<std::collections::BTreeSet<_>>()
    };
    let (low, zero, high) = (nodes_at(-300.0), nodes_at(0.0), nodes_at(300.0));
    assert!(low.is_subset(&zero) && zero.is_subset(&high), "{} <= {} <= {}", low.len(), zero.len(), high.len());
    assert!(low.len() < high.len(), "the level moves the selection: {} against {}", low.len(), high.len());
}

// spec: ATMO-015
#[test]
fn the_shore_filter_spreads_a_band_narrower_than_a_pixel_and_leaves_the_waterline_and_wide_bands_alone() {
    let look = WaterLook::EARTH;
    // The waterline stays zero at any width; the foam of a band far wider than the filter is what it was.
    for width in [0.0, 0.01, 0.3, 2.0] {
        assert_eq!(shore_cover(0.0, &look, width), 0.0);
        assert_eq!(foam_amount(0.0, &look, 1.0, width), 0.0);
    }
    let sharp = foam_amount(0.06, &look, 1.0, 0.0);
    assert!((foam_amount(0.06, &look, 1.0, 1.0e-4) - sharp).abs() < 1.0e-3 * sharp, "a filter of 0.1 mm leaves the band");
    // A pixel 3 m of depth wide (a 1:50 beach seen from far away) cannot hold the 0.4 m band: the filtered foam is a fraction of the
    // sharp peak everywhere, and it changes by far less between neighbouring depths than the sharp band does.
    let step = |w: f64| (0..4000).map(|k| f64::from(k) * 0.001).map(|d| foam_amount(d, &look, 1.0, w)).collect::<Vec<_>>();
    let (sharp_band, filtered) = (step(0.0), step(3.0));
    let peak = |v: &[f64]| v.iter().cloned().fold(0.0, f64::max);
    assert!(peak(&filtered) < 0.2 * peak(&sharp_band), "{} against {}", peak(&filtered), peak(&sharp_band));
    // The cover rises over about two filter widths.
    assert!(shore_cover(look.shore_fade_m, &look, 1.0) < 0.1 && shore_cover(2.5, &look, 1.0) > 0.99);
}
