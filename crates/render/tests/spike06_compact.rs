//! SPIKE-06 (docs/spikes.md): checks of the experimental compact tile format. Compiled only with `--features spike06`:
//! `cargo test -p planet-render --features spike06 --test spike06_compact` (PLANET_ADAPTER=hardware for the real GPU).
#![cfg(feature = "spike06")]

use planet_core::cube::TangentWarp;
use planet_frame::{plan_terrain, plan_terrain_field, scripted_views, scripted_views_field, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::{tile_mesh_field, vertex_count};
use planet_generators::stress::Steep;
use planet_render::spike_compact::{compact_shader, compact_tile, CompactRenderer};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;

/// Violations in one WGSL source (same rule as `shader_scan.rs`): literals of planet-radius magnitude and radius-like names.
fn scan(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for tok in code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '+' || c == '-')) {
            let t = tok.trim_end_matches(['f', 'u', 'i']);
            if t.starts_with(|c: char| c.is_ascii_digit()) && t.parse::<f64>().is_ok_and(|v| v.abs() >= 1.0e6) {
                hits.push(format!("line {}: literal {tok}", n + 1));
            }
        }
        let lower = code.to_lowercase();
        for word in ["radius", "planet_r", "earth"] {
            if lower.contains(word) {
                hits.push(format!("line {}: identifier containing '{word}'", n + 1));
            }
        }
    }
    hits
}

// spec: REND-005
#[test]
fn the_compact_shader_contains_no_planet_scale_values() {
    let hits = scan(&compact_shader());
    assert!(hits.is_empty(), "CON-11: {hits:?}");
}

/// Largest distance (metres) between the CPU mirror of the compact vertex shader and the product mesh, over every vertex,
/// as a fraction of the tile's edge length, plus the largest height difference and normal angle.
fn mirror_error(field: &dyn HeightField, cells: u32, tiles: &[planet_core::TileId], check_step: bool) -> (f64, f64, f64) {
    let (mut pos, mut height, mut normal) = (0f64, 0f64, 0f64);
    for id in tiles {
        let mesh = tile_mesh_field(field, &TangentWarp, *id, cells, R);
        let ct = compact_tile(field, &TangentWarp, *id, cells, R);
        let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
        let shaded = ct.shade_all();
        assert_eq!(shaded.len(), vertex_count(cells));
        for (a, b) in mesh.vertices.iter().zip(&shaded) {
            let d: f64 = (0..3)
                .map(|k| {
                    let e = f64::from(a.pos[k]) - f64::from(b.pos[k]);
                    e * e
                })
                .sum::<f64>()
                .sqrt();
            let c: f64 = (0..3)
                .map(|k| {
                    let e = f64::from(a.coarse[k]) - f64::from(b.coarse[k]);
                    e * e
                })
                .sum::<f64>()
                .sqrt();
            // The biquadratic base surface errs by about 0.018 theta^2 of the tile edge (theta: the tile's angular size).
            // CON-13: where a tile is fine (level 16 and finer) the step stays under 1 mm for these fields.
            if check_step && id.level() >= 16 {
                assert!(ct.hr_step < 1e-3, "level {} step {}", id.level(), ct.hr_step);
            }
            let theta = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
            // ... plus one height quantisation step and the f32 error of the base normal (about 2e-7 rad) times the tile's relief: a tile
            // whose relief is large against its edge (the steep stress field) moves a vertex by that much.
            pos = pos.max(
                d.max(c) / (edge * (0.03 * theta * theta + 1e-6) + 0.51 * f64::from(ct.hr_step) + 5e-7 * 65535.0 * f64::from(ct.hr_step)),
            );
            // The quantisation step bounds the height error, plus f32 rounding of the sum (relative 2.4e-7).
            let slack = 0.51 * f64::from(ct.hr_step) + 2.4e-7 * f64::from(a.height.abs().max(1.0));
            height = height.max(f64::from((a.height - b.height).abs().max((a.coarse_height - b.coarse_height).abs())) - slack);
            let dot: f64 = (0..3).map(|k| f64::from(a.normal[k]) * f64::from(b.normal[k])).sum();
            normal = normal.max(libm::acos(dot.clamp(-1.0, 1.0)).to_degrees());
        }
    }
    (pos, height, normal)
}

// spec: LOD-004, CON-13
#[test]
fn the_cpu_mirror_matches_the_product_mesh_on_the_scripted_views_and_the_steep_field() {
    let reference = Reference(1);
    let views = scripted_views(R, 4.0 / 3.0, 1);
    let lod = LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let mut tiles = Vec::new();
    for (_, camera) in views {
        let plan = plan_terrain(&TerrainRequest {
            camera,
            size: (128, 96),
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: "tangent-v1".into(),
            seed: 1,
        })
        .unwrap();
        tiles.extend(plan.nodes.iter().step_by(7).map(|n| n.tile));
    }
    let (pos, height, normal) = mirror_error(&reference, 16, &tiles, true);
    // Position within 1e-6 (f32 rounding) plus 0.03 theta^2 (measured: 0.018) of the tile edge (the biquadratic base surface; 0.28 % at level 2, under 0.05 % from
    // level 4), height within the quantisation step, normal within the octahedral snorm8 bound (about 1 degree).
    assert!(pos < 1.0, "reference field: position error {pos} x the biquadratic bound");
    assert!(height < 1e-6, "reference field: height error beyond the quantisation step: {height}");
    assert!(normal < 1.2, "reference field: normal error {normal} degrees");

    let steep = Steep(1);
    let lod = LodParams { cells: 4, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let mut tiles = Vec::new();
    for (_, camera) in scripted_views_field(R, 4.0 / 3.0, &steep) {
        if let Ok(plan) = plan_terrain_field(
            &TerrainRequest {
                camera,
                size: (128, 96),
                view: TerrainView::Level,
                lod: lod.clone(),
                face_mapping: "tangent-v1".into(),
                seed: 1,
            },
            &steep,
        ) {
            tiles.extend(plan.nodes.iter().step_by(5).map(|n| n.tile));
        }
    }
    assert!(!tiles.is_empty());
    let (pos, height, normal) = mirror_error(&steep, 4, &tiles, false);
    assert!(pos < 1.0, "steep field: position error {pos} x the biquadratic bound");
    assert!(height < 1e-6, "steep field: height error beyond the quantisation step: {height}");
    assert!(normal < 1.2, "steep field: normal error {normal} degrees");
}

// spec: REND-001
#[test]
fn the_compact_renderer_draws_the_same_images_as_the_product_renderer() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("{}", ctx.adapter_log_line());
    let size = (128u32, 96u32);
    let cells = 16;
    let lod = LodParams { cells, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let field = Reference(1);
    for (index, view) in [(0usize, TerrainView::Level), (3, TerrainView::Morph), (6, TerrainView::Relief), (6, TerrainView::Normals)] {
        let (name, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
        let plan =
            plan_terrain(&TerrainRequest { camera, size, view, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
        let mut product = TerrainRenderer::new(&ctx, size, cells);
        let mut compact = CompactRenderer::new(&ctx, size, cells, plan.nodes.len() as u32);
        for n in &plan.nodes {
            product.upload(&ctx, &tile_mesh_field(&field, &TangentWarp, n.tile, cells, R)).unwrap();
            compact.upload(&ctx, &compact_tile(&field, &TangentWarp, n.tile, cells, R)).unwrap();
        }
        product.render(&ctx, &plan, false).unwrap();
        compact.render(&ctx, &plan, false).unwrap();
        let (a, b) = (product.read_color(&ctx), compact.read_color(&ctx));
        let wrong = a.rgba.chunks(4).zip(b.rgba.chunks(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 12)).count();
        let limit = (a.rgba.len() / 4) / 200;
        assert!(wrong <= limit, "{name} {}: {wrong} pixels differ by more than 12 levels (limit {limit})", view.name());
    }
}
