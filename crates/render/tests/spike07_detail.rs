//! SPIKE-07 (docs/archive/M4/spikes.md): checks of the experimental shader detail term. Compiled only with `--features spike07`:
//! `cargo test -p planet-render --features spike07 --test spike07_detail` (PLANET_ADAPTER=hardware for the real GPU).
#![cfg(feature = "spike07")]

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::{tile_mesh, TileMesh};
use planet_render::spike_detail::{detail_shader, gpu_gradients, reference_gradient, DetailRenderer, NodeDetail, DETAIL_OCTAVES};
use planet_render::{execute_terrain, AdapterPolicy, GpuContext};

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
fn the_detail_shader_validates_and_contains_no_planet_scale_values() {
    let src = detail_shader();
    let hits = scan(&src);
    assert!(hits.is_empty(), "CON-11: {hits:?}");
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    ctx.validate_wgsl("terrain-detail", &src).expect("the detail shader must validate");
}

/// Deterministic probe positions: a small integer generator (no `rand`), spread over a tile-sized cube.
fn probes(count: usize, extent: f64, foot: f64, amp0: f64) -> Vec<([f64; 3], [f64; 3], f64)> {
    let mut s = 0x1234_5678_9abc_def1u64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    (0..count).map(|_| ([next() * extent, next() * extent, next() * extent], [foot; 3], amp0)).collect()
}

/// The angle (degrees) between the perturbed normals `normalize(n - (g - (g.n) n))` of two gradients.
fn normal_angle_deg(n: Vec3, a: [f64; 3], b: [f64; 3]) -> f64 {
    let pert = |g: [f64; 3]| {
        let g = Vec3::new(g[0], g[1], g[2]);
        (n - (g - n * g.dot(n))).normalized()
    };
    libm::acos(pert(a).dot(pert(b)).clamp(-1.0, 1.0)).to_degrees()
}

// spec: REND-005
#[test]
fn the_detail_gradient_on_the_gpu_matches_the_cpu_reference_within_the_normal_bound() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("{}", ctx.adapter_log_line());
    // Three origins: 1,250 m above the sea level far from the planet's axes, near a face edge, and near an axis.
    let origins = [
        Vec3::new(0.31, -0.62, 0.72).normalized() * (R + 1250.0),
        Vec3::new(1.0, 1.0, 0.003).normalized() * (R + 40.0),
        Vec3::new(0.0004, 0.0003, 1.0).normalized() * (R - 300.0),
    ];
    let mut worst = 0.0f64;
    for origin in origins {
        assert_eq!(NodeDetail::new(origin, R, 0.0).base.len(), DETAIL_OCTAVES);
        // Tile-sized offsets (a finest tile is about 2.4 m, a coarser tile up to 600 m) at three pixel footprints: all octaves,
        // a few, none.
        for (extent, foot, amp0) in [
            (2.4, 1.0e-4, 980.0),
            (4.77, 1.0e-4, 980.0),
            (9.54, 1.0e-4, 980.0),
            (2.4, 5.0e-3, 980.0),
            (600.0, 1.0e-2, 980.0),
            (600.0, 5.0, 980.0),
        ] {
            // The probes fill a cube of half-width `extent` (up to 1.73 x that from the origin); the node is declared with
            // `extent` as the renderer does (a tile edge), so the probes go beyond what the rule allows for.
            let node_extent = extent;
            let nd = NodeDetail::new(origin, R, node_extent);
            let ps = probes(2000, extent, foot, amp0);
            let gpu = gpu_gradients(
                &ctx,
                &nd,
                &ps.iter()
                    .map(|(l, f, a)| ([l[0] as f32, l[1] as f32, l[2] as f32], [f[0] as f32, f[1] as f32, f[2] as f32], *a as f32))
                    .collect::<Vec<_>>(),
            );
            let up = origin.normalized();
            // A tilted normal (a 20 degree slope), the case in which the gradient matters most.
            let n = (up + Vec3::new(0.34, -0.2, 0.1)).normalized();
            let mut worst_here = 0.0f64;
            let mut max_g = 0.0f64;
            let mut skipped = 0usize;
            for ((l, f, a), g) in ps.iter().zip(&gpu) {
                // The probe's local offset is rounded to f32 on the way to the GPU; give the reference the same offset.
                let l32 = l.map(|v| f64::from(v as f32));
                let f32v = f.map(|v| f64::from(v as f32));
                let (cpu, margin) = reference_gradient(origin, R, node_extent, l32, f32v, *a);
                // The creases of the term (|n| = 0) are kinks: the gradient jumps there, so a noise value uncertain by the f32
                // rounding of the lattice position (a few 1e-5 cells per cell of reach, 3 x that in value) can land on the
                // other side. Such a probe has no defined answer to the bound; count it and skip it.
                if margin < if extent > 10.0 { 1.0e-2 } else { 2.0e-3 } {
                    skipped += 1;
                    continue;
                }
                let gg = [f64::from(g[0]), f64::from(g[1]), f64::from(g[2])];
                worst_here = worst_here.max(normal_angle_deg(n, cpu, gg));
                max_g = max_g.max((cpu[0] * cpu[0] + cpu[1] * cpu[1] + cpu[2] * cpu[2]).sqrt());
            }
            eprintln!(
                "origin {origin:?} extent {extent} m foot {foot}: max |gradient| {max_g:.4}, worst normal difference {worst_here:.2e} deg, {skipped} of {} probes at a crease skipped",
                ps.len()
            );
            assert!(skipped * 5 <= ps.len(), "more than 20 % of the probes sit at a crease: the test would prove little");
            worst = worst.max(worst_here);
        }
    }
    // CON-14: normals agree within 0.01 degrees between a CPU and a GPU implementation.
    assert!(worst <= 0.01, "worst normal difference {worst} deg exceeds CON-14's 0.01 deg");
}

const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;

fn ground_plan() -> (planet_frame::TerrainPlan, Vec<TileMesh>) {
    let camera = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), 1).iter().find(|(n, _)| *n == "ground-1m").expect("view").1;
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    let plan =
        plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Relief, lod, face_mapping: "tangent-v1".into(), seed: 1 })
            .expect("plan");
    let meshes = plan.nodes.iter().map(|n| tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).collect();
    (plan, meshes)
}

// spec: REND-006
#[test]
fn with_the_term_off_the_detail_renderer_equals_the_product_and_with_it_on_only_shading_changes() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, meshes) = ground_plan();
    let refs: Vec<&TileMesh> = meshes.iter().collect();
    let (product, _) = execute_terrain(&ctx, &plan, &refs).expect("product");
    let mut r = DetailRenderer::new(&ctx, SIZE, CELLS, R);
    for m in &meshes {
        r.upload(&ctx, m).expect("upload");
    }
    r.render(&ctx, &plan, false).expect("off");
    let off = r.read_color(&ctx);
    assert_eq!(off, product, "switched off, the detail renderer must draw the product's pixels");
    r.render(&ctx, &plan, true).expect("on");
    let on = r.read_color(&ctx);
    let changed = on.rgba.chunks(4).zip(off.rgba.chunks(4)).filter(|(a, b)| a != b).count();
    assert!(changed > 0, "the term must change some pixels at 1 m altitude");
    // Visual only: a pixel is background in one image exactly when it is in the other (no geometry moved).
    let clear = plan.clear_color;
    for (a, b) in on.rgba.chunks(4).zip(off.rgba.chunks(4)) {
        assert_eq!(a == clear, b == clear, "the term must not change which pixels the terrain covers");
    }
    eprintln!("detail on changes {changed} of {} pixels", SIZE.0 * SIZE.1);
}
