//! Tier B: the GPU executor against its CPU twin, for every debug view.

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, PlanRequest};
use planet_generators::generate_tile;
use planet_render::{execute, reference_frame, AdapterPolicy, GpuContext, TileResource};

const RES: u32 = 16;

// spec: GEN-008, REND-001
#[test]
fn gpu_frames_match_the_cpu_twin() {
    let ctx = GpuContext::new(AdapterPolicy::Software).expect("software adapter");
    let m = TangentWarp;
    let a = TileId::new(Face(3), 3, 5, 2).unwrap();
    let tiles_ids = vec![a, a.neighbor(&m, Side::East), TileId::new(Face(0), 0, 0, 0).unwrap()];
    let data: Vec<_> = tiles_ids.iter().map(|&id| generate_tile(1, &m, id, RES)).collect();
    let resources: Vec<_> = data.iter().map(|t| TileResource { id: t.id, res: t.res, heights: &t.heights }).collect();
    for view in [DebugView::Face, DebugView::TileId, DebugView::Height] {
        // 3 tiles in 100 px: cells are 33 px wide, so the last columns stay clear colour (checked below).
        let plan = plan_tiles(&PlanRequest {
            tiles: tiles_ids.clone(),
            camera: PlanetFixed(Vec3::new(1.2e7, 1.0e6, -3.0e6)),
            size: (100, 48),
            view,
            radius_m: 6_371_000.0,
            face_mapping: "tangent-v1".into(),
        })
        .unwrap();
        let (gpu, stats) = execute(&ctx, &plan, &resources).unwrap();
        let cpu = reference_frame(&plan, &resources).unwrap();
        assert_eq!(stats.draw_calls as usize, plan.draws.len());
        let allowed = if view == DebugView::Height { 1 } else { 0 };
        let mut worst = 0u8;
        let mut first_bad = None;
        for (i, (g, c)) in gpu.rgba.iter().zip(&cpu.rgba).enumerate() {
            let d = g.abs_diff(*c);
            if d > worst {
                worst = d;
            }
            if d > allowed && first_bad.is_none() {
                first_bad = Some(((i / 4) as u32 % gpu.width, (i / 4) as u32 / gpu.width, i % 4));
            }
        }
        assert!(
            worst <= allowed,
            "{} view: GPU differs from the CPU twin by {worst} (allowed {allowed}), first at (x, y, channel) {first_bad:?}",
            view.name()
        );
    }
}
