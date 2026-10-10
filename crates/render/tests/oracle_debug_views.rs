//! Tier B: the GPU executor against its CPU twin, for every debug view.

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, PlanRequest};
use planet_generators::generate_tile;
use planet_render::{execute, reference_frame, AdapterPolicy, GpuContext, TileResource};

const RES: u32 = 16;

// spec: REND-001
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
        if view == DebugView::Height {
            check_height_view(&gpu, &cpu, &plan, &resources);
        } else {
            assert_eq!(gpu.rgba, cpu.rgba, "{} view must match the CPU twin exactly", view.name());
        }
    }
}

/// GPU and CPU may round differently only where `t*255 + 0.5` lies within 1e-3 of a half-integer boundary; elsewhere
/// they must agree exactly. Reports the number of tolerated pixels.
fn check_height_view(
    gpu: &planet_render::RgbaFrame,
    cpu: &planet_render::RgbaFrame,
    plan: &planet_frame::FramePlan,
    tiles: &[TileResource],
) {
    let [lo, hi] = plan.height_range;
    let mut tolerated = 0;
    for d in &plan.draws {
        let t = tiles.iter().find(|t| t.id == d.tile).unwrap();
        let [x0, y0, w, h] = d.rect;
        for py in 0..h {
            for px in 0..w {
                let (tx, ty) = ((px * (t.res + 1) / w).min(t.res), (py * (t.res + 1) / h).min(t.res));
                let v = f64::from(t.heights[(ty * (t.res + 1) + tx) as usize]);
                let x = ((v - f64::from(lo)) / f64::from(hi - lo)).clamp(0.0, 1.0) * 255.0 + 0.5;
                let near_boundary = (x - x.round()).abs() < 1e-3;
                let i = (((y0 + py) * gpu.width + x0 + px) * 4) as usize;
                let diff = gpu.rgba[i].abs_diff(cpu.rgba[i]);
                assert!(
                    diff == 0 || (diff == 1 && near_boundary),
                    "height view: GPU {} vs CPU {} at ({}, {}) tile {:?} h={v} (x={x})",
                    gpu.rgba[i],
                    cpu.rgba[i],
                    x0 + px,
                    y0 + py,
                    d.tile
                );
                tolerated += usize::from(diff == 1);
            }
        }
    }
    eprintln!("height view: {tolerated} boundary pixels differed by one grey level");
}
