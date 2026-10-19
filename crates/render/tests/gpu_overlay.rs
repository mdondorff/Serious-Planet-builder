//! Tier C: the wireframe overlay (VIEW-014) through the lit path on the adapter the run selects (the software adapter in
//! `cargo xtask test gpu`). With the overlay off the frame is the lit frame it was; with it on, pixels of the terrain change and the
//! ones outside the planet do not. Inspection images go to `target/review/overlay-inspection/`, never into the goldens.

use planet_core::cube::TangentWarp;
use planet_frame::look::{LitPlan, Overlay};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::Image;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 120);
/// The size of the inspection images: big enough to see the lines.
const BIG: (u32, u32) = (640, 360);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 15.0;

fn plan(index: usize, size: (u32, u32), tau_px: f64) -> TerrainPlan {
    let aspect = f64::from(size.0) / f64::from(size.1);
    let (_, camera) = scripted_views(R, aspect, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(size.1), tau_px, max_level: FINEST, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn lit(mut p: TerrainPlan, l: LitPlan) -> TerrainPlan {
    p.lit = Some(l);
    p
}

fn compact_pool() -> PoolConfig {
    let block_slots = 48 * 48;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn renderer(ctx: &GpuContext, p: &TerrainPlan) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, p.size, CELLS, compact_pool());
    r.enable_lit(ctx);
    let field = Reference(1);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
        }
    }
    r
}

fn frame(r: &mut TerrainRenderer, ctx: &GpuContext, p: &TerrainPlan, overlay: Overlay) -> Image {
    r.render(ctx, &lit(p.clone(), LitPlan::fixed(EV).with_overlay(overlay)), false).unwrap();
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

fn inspection_dir() -> std::path::PathBuf {
    let dir = planet_testkit::repo_root().join("target").join("review").join("overlay-inspection");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// spec: VIEW-014
#[test]
fn the_overlays_change_the_pixels_of_the_terrain_only_and_the_off_frame_is_the_plain_lit_frame() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // View 3 is close to the ground, view 0 the whole planet from orbit, view 6 in between.
    for index in [0usize, 3, 6] {
        let p = plan(index, SIZE, 2.5);
        let mut r = renderer(&ctx, &p);
        let plain = frame(&mut r, &ctx, &p, Overlay::None);
        // The terrain's pixels are the ones the scene target holds opaque (the lit path's alpha).
        let covered: Vec<bool> = r.lit_path().unwrap().read_hdr(&ctx).iter().map(|px| px[3] > 0.0).collect();
        let off = {
            r.render(&ctx, &lit(p.clone(), LitPlan::fixed(EV)), false).unwrap();
            let f = r.read_color(&ctx);
            Image::new(f.width, f.height, f.rgba)
        };
        let tiles = frame(&mut r, &ctx, &p, Overlay::Tiles);
        let mesh = frame(&mut r, &ctx, &p, Overlay::Mesh);
        let mesh_again = frame(&mut r, &ctx, &p, Overlay::Mesh);
        let covered_count = covered.iter().filter(|c| **c).count();
        assert!(covered_count > 100, "view {index}: the planet is visible");
        assert!(plain.pixels().eq(off.pixels()), "view {index}: an overlay of None is the lit frame");
        assert!(mesh.pixels().eq(mesh_again.pixels()), "view {index}: the mesh frame is deterministic");
        for (name, img) in [("tiles", &tiles), ("mesh", &mesh)] {
            let mut changed = 0usize;
            for ((c, a), b) in covered.iter().zip(plain.pixels()).zip(img.pixels()) {
                if *c {
                    changed += usize::from(a != b);
                } else {
                    assert_eq!(a, b, "view {index} {name}: a pixel outside the planet changed");
                }
            }
            eprintln!("view {index} {name}: {changed} of {covered_count} terrain pixels changed");
            if index == 3 {
                assert!(changed >= 50, "view 3 {name}: the overlay changes the terrain pixels ({changed})");
            }
        }
        // The mesh adds the inner lines to the tiles' picture, however fine the mesh is: the two differ where there is a cell.
        if index == 3 {
            assert!(!tiles.pixels().eq(mesh.pixels()), "view 3: the mesh overlay draws more than the tiles overlay");
        }
    }
}

// spec: VIEW-014
#[test]
fn the_overlay_inspection_images_are_written_for_a_look() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let dir = inspection_dir();
    // The production error bound (2.5 px: the mesh edge is about a pixel and a half) and the coarse one the editor's F4 gives (four
    // times as much, 10 px).
    for (index, tau, name) in [(3usize, 2.5, "production"), (6, 2.5, "production"), (3, 10.0, "coarse"), (6, 10.0, "coarse")] {
        let p = plan(index, BIG, tau);
        let mut r = renderer(&ctx, &p);
        let plain = frame(&mut r, &ctx, &p, Overlay::None);
        let tiles = frame(&mut r, &ctx, &p, Overlay::Tiles);
        let mesh = frame(&mut r, &ctx, &p, Overlay::Mesh);
        assert!(!plain.pixels().eq(tiles.pixels()) && !tiles.pixels().eq(mesh.pixels()), "view {index} ({name}): the overlays show");
        planet_testkit::write_png(&dir.join(format!("view{index}-{name}-off.png")), &plain).unwrap();
        planet_testkit::write_png(&dir.join(format!("view{index}-{name}-tiles.png")), &tiles).unwrap();
        planet_testkit::write_png(&dir.join(format!("view{index}-{name}-mesh.png")), &mesh).unwrap();
    }
}
