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
fn the_wireframe_changes_the_pixels_of_the_terrain_only_and_the_off_frame_is_the_plain_lit_frame() {
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
        let on = frame(&mut r, &ctx, &p, Overlay::Wireframe);
        let on_again = frame(&mut r, &ctx, &p, Overlay::Wireframe);
        let covered_count = covered.iter().filter(|c| **c).count();
        assert!(covered_count > 100, "view {index}: the planet is visible");
        assert!(plain.pixels().eq(off.pixels()), "view {index}: an overlay of None is the lit frame");
        assert!(on.pixels().eq(on_again.pixels()), "view {index}: the overlay frame is deterministic");
        let mut changed_on_terrain = 0usize;
        for ((c, a), b) in covered.iter().zip(plain.pixels()).zip(on.pixels()) {
            if *c {
                changed_on_terrain += usize::from(a != b);
            } else {
                assert_eq!(a, b, "view {index}: a pixel outside the planet changed");
            }
        }
        eprintln!("view {index}: {changed_on_terrain} of {covered_count} terrain pixels carry a wire");
        // A near view shows cells wider than a pixel and a half: wires are drawn. (From orbit the cells are finer and fade out.)
        if index == 3 {
            assert!(changed_on_terrain >= 50, "view 3: the wireframe changes the terrain pixels ({changed_on_terrain})");
        }
    }
}

// spec: VIEW-014
#[test]
fn the_wireframe_inspection_images_are_written_for_a_look() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let dir = inspection_dir();
    // The production error bound (2.5 px: the mesh edge is about a pixel and a half) and a coarse one (8 px) for a readable look.
    for (index, tau, name) in [(3usize, 2.5, "production"), (6, 2.5, "production"), (3, 8.0, "coarse"), (6, 8.0, "coarse")] {
        let p = plan(index, BIG, tau);
        let mut r = renderer(&ctx, &p);
        let plain = frame(&mut r, &ctx, &p, Overlay::None);
        let on = frame(&mut r, &ctx, &p, Overlay::Wireframe);
        assert!(!plain.pixels().eq(on.pixels()), "view {index} ({name}): the wireframe shows");
        planet_testkit::write_png(&dir.join(format!("view{index}-{name}-off.png")), &plain).unwrap();
        planet_testkit::write_png(&dir.join(format!("view{index}-{name}-wireframe.png")), &on).unwrap();
    }
}
