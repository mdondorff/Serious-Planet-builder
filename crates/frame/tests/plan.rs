//! TEST-009 (FramePlan text form, snapshot), TEST-008 (repro bundle) and the plan builder's decisions.

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, FramePlan, PlanError, PlanRequest, ReproBundle};

fn tile(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

fn request(view: DebugView) -> PlanRequest {
    let t = tile(3, 3, 5, 2);
    PlanRequest {
        tiles: vec![t, t.neighbor(&TangentWarp, Side::East)],
        camera: PlanetFixed(Vec3::new(1.0e7, -2.0e6, 3.0e6)),
        size: (256, 128),
        view,
        radius_m: 6_371_000.0,
        face_mapping: "tangent-v1".into(),
    }
}

// spec: TEST-009
#[test]
fn plan_text_round_trips_exactly() {
    for view in [DebugView::Face, DebugView::TileId, DebugView::Height] {
        let plan = plan_tiles(&request(view)).unwrap();
        let back = FramePlan::from_text(&plan.to_text()).unwrap();
        assert_eq!(back, plan);
        assert_eq!(back.plan_hash(), plan.plan_hash());
        assert!(plan.diff(&back).is_empty());
    }
}

// spec: TEST-009
#[test]
fn plan_text_matches_the_committed_snapshot() {
    let text = plan_tiles(&request(DebugView::Face)).unwrap().to_text();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/two_tiles_face.plan");
    if std::env::var("PLANET_UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("snapshot missing; run with PLANET_UPDATE_SNAPSHOTS=1 and review the diff");
    assert_eq!(text, expected, "FramePlan text changed; review the diff and update the snapshot deliberately");
}

// spec: TEST-009
#[test]
fn malformed_plans_are_rejected_with_the_offending_line() {
    let good = plan_tiles(&request(DebugView::Face)).unwrap().to_text();
    assert!(FramePlan::from_text("planet-frameplan 99\n").unwrap_err().contains("unsupported plan header"));
    let bad = good.replace("view face", "view sepia");
    assert!(FramePlan::from_text(&bad).unwrap_err().contains("sepia"));
    let bad = good.replacen("rect ", "rekt ", 1);
    assert!(FramePlan::from_text(&bad).unwrap_err().contains("cannot parse plan line"));
}

// spec: CON-03
#[test]
fn builder_lays_out_tiles_and_converts_to_camera_relative_in_f64_first() {
    let req = request(DebugView::TileId);
    let plan = plan_tiles(&req).unwrap();
    assert_eq!(plan.draws.len(), 2);
    assert_eq!(plan.draws[0].rect, [0, 0, 128, 128]);
    assert_eq!(plan.draws[1].rect, [128, 0, 128, 128]);
    for d in &plan.draws {
        let exact = d.origin.0 - req.camera.0;
        let rel = d.camera_relative_origin;
        for (got, want) in rel.iter().zip([exact.x, exact.y, exact.z]) {
            assert_eq!(*got, want as f32, "camera-relative origin must be the f64 difference rounded once");
        }
        assert!((d.origin.0.length() - req.radius_m).abs() < 1e-6, "tile origins lie on the sphere");
    }
    assert_ne!(plan.draws[0].color, plan.draws[1].color, "tile-id colours must differ");
    assert_ne!(plan.draws[0].color, plan.clear_color);
    // Deterministic.
    assert_eq!(plan, plan_tiles(&req).unwrap());
}

// spec: CON-03
#[test]
fn builder_reports_unusable_requests() {
    let mut r = request(DebugView::Face);
    r.tiles.clear();
    assert_eq!(plan_tiles(&r).unwrap_err(), PlanError::NoTiles);
    let mut r = request(DebugView::Face);
    r.face_mapping = "equal-area".into();
    assert!(plan_tiles(&r).unwrap_err().to_string().contains("equal-area"));
    let mut r = request(DebugView::Face);
    r.size = (1, 8);
    assert!(matches!(plan_tiles(&r).unwrap_err(), PlanError::FrameTooSmall { .. }));
}

fn bundle() -> ReproBundle {
    let request = request(DebugView::Height);
    let plan = plan_tiles(&request).unwrap();
    ReproBundle {
        generator_version: 2,
        implementation_id: "cpu-ref-height-v2".into(),
        seed: 1,
        res: 16,
        definition_hash: 0,
        adapter: "dx12-microsoft-basic-render-driver".into(),
        request,
        plan,
    }
}

// spec: TEST-008
#[test]
fn bundle_round_trips_and_detects_corruption() {
    let b = bundle();
    let text = b.to_text();
    assert_eq!(ReproBundle::from_text(&text).unwrap(), b);
    // Editing the embedded plan without updating the recorded hash is detected, with both hashes in the message.
    let tampered = text.replacen("rect 0 0 128 128", "rect 0 0 127 128", 1);
    let e = ReproBundle::from_text(&tampered).unwrap_err();
    assert!(e.contains("corrupt") && e.contains("recorded plan hash"), "{e}");
    assert!(ReproBundle::from_text("planet-repro 7\n---\n").unwrap_err().contains("unsupported bundle header"));
    assert!(ReproBundle::from_text("no separator").unwrap_err().contains("separator"));
}

// spec: TEST-008
#[test]
fn rebuilding_the_plan_from_the_recorded_request_reproduces_it() {
    let b = ReproBundle::from_text(&bundle().to_text()).unwrap();
    let rebuilt = plan_tiles(&b.request).unwrap();
    assert!(rebuilt.diff(&b.plan).is_empty(), "{:?}", rebuilt.diff(&b.plan));
    // A changed camera shows up as numbers in the diff.
    let mut moved = b.request.clone();
    moved.camera = PlanetFixed(Vec3::new(1.0e7 + 1.0, -2.0e6, 3.0e6));
    let d = plan_tiles(&moved).unwrap().diff(&b.plan);
    assert!(d.iter().any(|l| l.starts_with("camera")) && d.iter().any(|l| l.contains("camera-relative origin")), "{d:?}");
}
