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

// spec: REND-013, TEST-011
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
    // A renderer that only ever saw one plan holds exactly the tiles and bytes the plan implies (TEST-011).
    let mut single = TerrainRenderer::new(&ctx, SIZE, CELLS);
    for n in &p_near.nodes {
        single.upload(&ctx, &tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).unwrap();
    }
    let counts = p_near.counts();
    assert_eq!((single.resident_tiles() as u32, single.uploaded_bytes), (counts.resident_tiles, counts.upload_bytes));
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
        // A very fast frame can quantise to a zero timestamp delta: that is a valid measurement, not a failure.
        assert!((0.0..10_000.0).contains(&g), "implausible GPU time {g} ms");
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

// spec: REND-013
#[test]
fn a_mesh_with_another_key_replaces_the_resident_buffer() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5);
    let tile = p.nodes[0].tile;
    let (one, two) = (tile_mesh(1, &TangentWarp, tile, CELLS, R), tile_mesh(2, &TangentWarp, tile, CELLS, R));
    assert_ne!(one.key, two.key);
    let mut r = TerrainRenderer::new(&ctx, SIZE, CELLS);
    r.upload(&ctx, &one).unwrap();
    let after_one = r.uploaded_bytes;
    r.upload(&ctx, &two).unwrap();
    assert_eq!((r.tile_key(tile), r.resident_tiles()), (Some(two.key), 1), "the seed-2 mesh must replace the seed-1 buffer");
    assert!(r.uploaded_bytes > after_one, "replacing uploads the new vertices");
    // Draw that single tile and compare with a renderer that only ever saw the seed-2 mesh.
    let mut single = plan(5);
    single.nodes.truncate(1);
    single.nodes[0].mesh_key = two.key; // the plan of a seed-2 request expects the seed-2 mesh
    r.render(&ctx, &single, false).unwrap();
    let replaced = r.read_color(&ctx);
    let mut fresh = TerrainRenderer::new(&ctx, SIZE, CELLS);
    fresh.upload(&ctx, &two).unwrap();
    fresh.render(&ctx, &single, false).unwrap();
    assert_eq!(replaced, fresh.read_color(&ctx), "a replaced buffer must draw exactly like a fresh upload of the new mesh");
}

// spec: REND-013
#[test]
fn a_plan_for_another_seed_refuses_stale_buffers() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(5); // seed 1
    let mut r = TerrainRenderer::new(&ctx, SIZE, CELLS);
    for n in &p.nodes {
        r.upload(&ctx, &tile_mesh(2, &TangentWarp, n.tile, CELLS, R)).unwrap(); // seed 2: stale for this plan
    }
    let e = r.render(&ctx, &p, false).unwrap_err();
    assert!(matches!(e, planet_render::ExecError::StaleMesh { tile, .. } if tile == p.nodes[0].tile), "{e}");
    assert!(!r.has_mesh(p.nodes[0].tile, p.nodes[0].mesh_key));
    // Re-uploading the current meshes makes the plan drawable.
    for n in &p.nodes {
        r.upload(&ctx, &tile_mesh(1, &TangentWarp, n.tile, CELLS, R)).unwrap();
    }
    assert!(r.has_mesh(p.nodes[0].tile, p.nodes[0].mesh_key));
    r.render(&ctx, &p, false).unwrap();
}
