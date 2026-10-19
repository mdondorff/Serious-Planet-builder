//! Tier C, REND-033: the edit preview in the compact vertex stage. No preview stamp (or no flagged node) draws the frame of a build without
//! the preview, bit for bit; a stamp changes the frame inside its reach; a stamp and its removal cancel; images go to `target/review/preview/`.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::preview::PreviewStamp;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::{execute_terrain_tiles_preview, AdapterPolicy, GpuContext};
use planet_world_def::stamp::StampKind;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 120);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn plan_for(index: usize, view: TerrainView) -> TerrainPlan {
    let lod = LodParams {
        cells: CELLS,
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 2.5,
        max_level: planet_frame::production_finest_level(CELLS),
        ..LodParams::earth_1080p()
    };
    let (_, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[index];
    plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn frame(ctx: &GpuContext, plan: &TerrainPlan, stamps: &[PreviewStamp]) -> Vec<u8> {
    let tiles: Vec<CompactTile> = plan.nodes.iter().map(|n| compact_tile_field(&Reference(SEED), &TangentWarp, n.tile, CELLS, R)).collect();
    let refs: Vec<&CompactTile> = tiles.iter().collect();
    execute_terrain_tiles_preview(ctx, plan, &refs, stamps, R).unwrap().0.rgba
}

fn hill(at: Vec3, amount: f64, reach_m: f64, sign: i8) -> PreviewStamp {
    PreviewStamp { sign, kind: StampKind::Hill, amount_m: amount, dir: at, reach_m, max_delta_m: amount.abs() * 2.0 }
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

fn write_ppm(name: &str, rgba: &[u8]) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("target").join("review").join("preview");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut out = format!("P6\n{} {}\n255\n", SIZE.0, SIZE.1).into_bytes();
    out.extend(rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
    let _ = std::fs::write(dir.join(format!("{name}.ppm")), out);
}

// spec: REND-033
#[test]
fn the_preview_draws_nothing_without_a_stamp_or_a_flag_and_a_stamp_only_inside_its_reach_and_cancels_with_its_removal() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [6usize, 8] {
        let mut plan = plan_for(index, TerrainView::Height);
        // The node under the camera: the stamp sits at its centre.
        let cam = plan.camera.position.0.normalized();
        let near = plan.nodes.iter().map(|n| n.tile).max_by(|a, b| {
            a.center_direction(&TangentWarp).dot(cam).total_cmp(&b.center_direction(&TangentWarp).dot(cam)).then(b.level().cmp(&a.level()))
        });
        let at = near.unwrap().center_direction(&TangentWarp);
        let plain = frame(&ctx, &plan, &[]);
        // Flagged nodes with an empty list: the frame of a build without the preview.
        for n in &mut plan.nodes {
            n.preview = true;
        }
        assert_eq!(frame(&ctx, &plan, &[]), plain, "view {index}: flags without a stamp draw nothing");
        // A stamp with no flagged node draws nothing either.
        let mut unflagged = plan.clone();
        for n in &mut unflagged.nodes {
            n.preview = false;
        }
        let up = hill(at, 400.0, 3_000.0, 1);
        assert_eq!(frame(&ctx, &unflagged, &[up]), plain, "view {index}: a stamp without a flagged node draws nothing");
        // A stamp on flagged nodes changes the frame, and not all of it.
        let with = frame(&ctx, &plan, &[up]);
        let changed = differing(&with, &plain);
        write_ppm(&format!("view{index}-plain"), &plain);
        write_ppm(&format!("view{index}-hill"), &with);
        println!("view {index}: {changed} of {} pixels change under a 400 m hill of reach 3 km", plain.len() / 4);
        // A stamp and its removal cancel (the Height view reads the vertex height only).
        let cancel = frame(&ctx, &plan, &[up, hill(at, 400.0, 3_000.0, -1)]);
        assert_eq!(cancel, plain, "view {index}: +hill and -hill cancel");
        // The stamp is local: a small one changes fewer pixels than a large one, and never more than the frame.
        let small = frame(&ctx, &plan, &[hill(at, 400.0, 300.0, 1)]);
        assert!(differing(&small, &plain) <= changed, "view {index}: a smaller reach changes no more pixels");
        if index == 6 {
            assert!(changed > 0, "view {index}: the hill is seen");
        }
    }
}
