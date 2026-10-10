//! REND-013: the persistent terrain renderer reuses its resources and times frames.

use planet_core::cube::TangentWarp;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::mesh::tile_mesh;
use planet_render::{execute_terrain, AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;

fn plan(index: usize) -> planet_frame::TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

// spec: REND-013
#[test]
fn a_persistent_renderer_uploads_each_tile_once_and_matches_the_one_shot_path() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (p_near, p_far) = (plan(4), plan(3));
    let meshes: Vec<_> = p_near.nodes.iter().chain(&p_far.nodes).map(|n| tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).collect();
    let mut r = TerrainRenderer::new(&ctx, SIZE, CELLS);
    for m in &meshes {
        r.upload(&ctx, m).unwrap();
    }
    let uploaded = r.uploaded_bytes;
    let resident = r.resident_tiles();
    // Uploading everything again changes nothing.
    for m in &meshes {
        r.upload(&ctx, m).unwrap();
    }
    assert_eq!((r.uploaded_bytes, r.resident_tiles()), (uploaded, resident), "tiles must be uploaded once");

    for p in [&p_near, &p_far, &p_near] {
        let (_, stats) = r.render(&ctx, p, false).unwrap();
        assert_eq!(stats.draw_calls as usize, p.nodes.len());
        let frame = r.read_color(&ctx);
        let refs: Vec<_> = meshes.iter().filter(|m| p.nodes.iter().any(|n| n.tile == m.id)).collect();
        let (one_shot, _) = execute_terrain(&ctx, p, &refs).unwrap();
        assert_eq!(frame, one_shot, "the persistent renderer must draw exactly what the one-shot path draws");
    }
    assert_eq!((r.uploaded_bytes, r.resident_tiles()), (uploaded, resident), "rendering must not upload");
}

// spec: REND-013
#[test]
fn frames_are_timed_and_a_missing_tile_is_an_error() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    let mut r = TerrainRenderer::new(&ctx, SIZE, CELLS);
    let e = r.render(&ctx, &p, true).unwrap_err();
    assert!(matches!(e, planet_render::ExecError::MissingTile(_)), "{e}");
    for n in &p.nodes {
        r.upload(&ctx, &tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).unwrap();
    }
    let (t, _) = r.render(&ctx, &p, true).unwrap();
    assert!(t.cpu_ms > 0.0 && t.total_ms >= t.cpu_ms, "{t:?}");
    if ctx.timestamps_supported() {
        let g = t.gpu_ms.expect("timestamps are supported, so GPU time must be reported");
        assert!(g > 0.0 && g < 10_000.0, "implausible GPU time {g} ms");
    } else {
        assert!(t.gpu_ms.is_none());
    }
    // A plan for another frame size says so (and does not blame a tile resolution).
    let mut other = plan(5);
    other.size = (64, 48);
    let e = r.render(&ctx, &other, false).unwrap_err();
    assert!(matches!(e, planet_render::ExecError::SizeMismatch { .. }) && e.to_string().contains("64x48"), "{e}");
    // Wrong resolution is rejected.
    assert!(r.upload(&ctx, &tile_mesh(1, &TangentWarp, p.nodes[0].tile, 8, R)).is_err());
}
