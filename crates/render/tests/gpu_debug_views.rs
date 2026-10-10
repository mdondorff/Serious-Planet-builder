//! Tier C: debug views of a generated tile pair, end to end (generator -> FramePlan -> GPU -> pixels).

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, FramePlan, PlanRequest};
use planet_generators::dump::FACE_COLORS;
use planet_generators::generate_tile;
use planet_render::{execute, AdapterPolicy, GpuContext, TileResource};
use planet_testkit::{assert_golden, Image, Tolerance};

const RES: u32 = 16;
const SIZE: (u32, u32) = (128, 64);

fn render(ctx: &GpuContext, view: DebugView) -> (FramePlan, Image, u32) {
    let m = TangentWarp;
    let a = TileId::new(Face(3), 3, 5, 2).unwrap();
    let ids = vec![a, a.neighbor(&m, Side::East)];
    let data: Vec<_> = ids.iter().map(|&id| generate_tile(1, &m, id, RES)).collect();
    let resources: Vec<_> = data.iter().map(|t| TileResource { id: t.id, res: t.res, heights: &t.heights }).collect();
    let plan = plan_tiles(&PlanRequest {
        tiles: ids,
        camera: PlanetFixed(Vec3::new(1.2e7, 1.0e6, -3.0e6)),
        size: SIZE,
        view,
        radius_m: 6_371_000.0,
        face_mapping: "tangent-v1".into(),
    })
    .unwrap();
    let (frame, stats) = execute(ctx, &plan, &resources).unwrap();
    (plan, Image::new(frame.width, frame.height, frame.rgba), stats.draw_calls)
}

// spec: REND-001, REND-006, TEST-010
#[test]
fn debug_views_have_no_holes_are_deterministic_and_match_goldens() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("adapter: {:?} {}", ctx.info.backend, ctx.info.name);
    for view in [DebugView::Face, DebugView::TileId, DebugView::Height] {
        let (plan, img, draws) = render(&ctx, view);
        assert_eq!(draws as usize, plan.draws.len(), "draw calls must equal the plan's draws");
        // No clear-colour ("background") pixel inside any planned rectangle: a hole would be a missing draw.
        let holes = img.count(plan.clear_color);
        assert_eq!(holes, 0, "{} view: {holes} hole pixels", view.name());
        // Deterministic: a second render is identical.
        let (_, again, _) = render(&ctx, view);
        assert_eq!(img, again, "{} view is not deterministic", view.name());
        if view == DebugView::Face {
            // Both tiles are on face 3 and flat-coloured exactly.
            assert_eq!(img.count(FACE_COLORS[3]), (SIZE.0 * SIZE.1) as usize);
        }
        if view == DebugView::Height {
            let greys: std::collections::BTreeSet<u8> = img.pixels().map(|p| p[0]).collect();
            assert!(greys.len() > 15, "height view should show relief, got {} grey levels", greys.len());
        }
        assert_golden(&planet_render_key(&ctx), &format!("debug_view_{}", view.name().replace('-', "_")), &img, Tolerance::Exact);
    }
}

fn planet_render_key(ctx: &GpuContext) -> String {
    ctx.adapter_key()
}
