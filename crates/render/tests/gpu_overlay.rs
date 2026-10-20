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

/// A polygon of `points` (metres east/north-ish in the plane tangent under the camera) in the form `planet_frame::regions` gives.
fn poly_under(camera: planet_core::Vec3, points: Vec<[f32; 2]>, tint: [f32; 3]) -> planet_frame::regions::RegionPoly {
    let up = camera.normalized();
    let (e1, e2) = planet_world_def::region::basis(up);
    let f3 = |v: planet_core::Vec3| [v.x as f32, v.y as f32, v.z as f32];
    let far = points.iter().map(|p| p[0] * p[0] + p[1] * p[1]).fold(0.0f32, f32::max);
    planet_frame::regions::RegionPoly {
        tint,
        anchor: f3(up * R - camera),
        e1: f3(e1),
        e2: f3(e2),
        up: f3(up),
        reach_sq: far * 1.1,
        band_m: 30_000.0,
        points,
    }
}

/// A square of `half_m` metres around the point under the camera.
fn square_under(camera: planet_core::Vec3, half_m: f64, tint: [f32; 3]) -> planet_frame::regions::RegionPoly {
    let h = half_m as f32;
    poly_under(camera, vec![[-h, -h], [h, -h], [h, h], [-h, h]], tint)
}

/// Distance of the point `q` from the segment `a` to `b`.
fn segment_distance(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ab, aq) = ([b[0] - a[0], b[1] - a[1]], [q[0] - a[0], q[1] - a[1]]);
    let t = ((aq[0] * ab[0] + aq[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1])).clamp(0.0, 1.0);
    let d = [aq[0] - t * ab[0], aq[1] - t * ab[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

fn frame_regions(
    r: &mut TerrainRenderer,
    ctx: &GpuContext,
    p: &TerrainPlan,
    list: &planet_frame::regions::RegionList,
) -> (Image, Vec<[f32; 4]>) {
    r.set_regions(list);
    let img = frame(r, ctx, p, Overlay::Regions);
    (img, r.lit_path().unwrap().read_hdr(ctx))
}

// spec: VIEW-030
#[test]
fn the_regions_overlay_mixes_the_tint_at_the_stated_weight_inside_the_polygon_and_changes_nothing_outside() {
    use planet_frame::regions::RegionList;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3, SIZE, 2.5);
    let camera = p.camera.position.0;
    let mut r = renderer(&ctx, &p);
    let plain = frame(&mut r, &ctx, &p, Overlay::None);
    let plain_hdr = r.lit_path().unwrap().read_hdr(&ctx);
    // No regions: the overlay is the plain frame. A region on the far side of the planet: the same.
    let (empty, _) = frame_regions(&mut r, &ctx, &p, &RegionList::default());
    assert!(empty.pixels().eq(plain.pixels()), "the overlay with no polygon is the plain frame");
    let far = RegionList { polys: vec![square_under(-camera, 3_000.0, [0.2, 0.6, 0.1])], dropped: 0 };
    let (away, _) = frame_regions(&mut r, &ctx, &p, &far);
    assert!(away.pixels().eq(plain.pixels()), "a polygon on the other side of the planet tints nothing");
    // A square under the camera: the pixels it covers are tinted by the stated mix, every other pixel is the plain frame.
    let tint = [0.2f32, 0.6, 0.1];
    let list = RegionList { polys: vec![square_under(camera, 20_000.0, tint)], dropped: 0 };
    let (img, hdr) = frame_regions(&mut r, &ctx, &p, &list);
    let factor = planet_frame::look::pre_exposure_factor_with(&LitPlan::fixed(EV).units, f64::from(EV)) as f32;
    let mut tinted = 0usize;
    for (i, (a, b)) in plain_hdr.iter().zip(&hdr).enumerate() {
        if a == b {
            continue;
        }
        tinted += 1;
        assert!(a[3] > 0.0, "pixel {i}: only terrain is tinted");
        for c in 0..3 {
            let want = a[c] * 0.55 + tint[c] * 0.8 * factor * 0.45;
            // The scene target holds f16: a relative error of 1e-3, and the radiance is clamped at 60,000.
            assert!((b[c] - want).abs() <= 2e-3 * want.abs().max(1.0), "pixel {i} channel {c}: {} against the mix {want}", b[c]);
        }
    }
    eprintln!("regions overlay: {tinted} tinted terrain pixels");
    assert!(tinted >= 50, "the square under the camera tints the ground ({tinted} pixels)");
    assert!(!img.pixels().eq(plain.pixels()));
    // A bigger square tints more.
    let bigger = RegionList { polys: vec![square_under(camera, 35_000.0, tint)], dropped: 0 };
    let (_, big_hdr) = frame_regions(&mut r, &ctx, &p, &bigger);
    let big_count = plain_hdr.iter().zip(&big_hdr).filter(|(a, b)| a != b).count();
    assert!(big_count >= tinted, "the bigger polygon tints more ({big_count} against {tinted})");
    // The frame is deterministic, and the inspection image is written.
    let (again, _) = frame_regions(&mut r, &ctx, &p, &list);
    assert!(again.pixels().eq(img.pixels()));
    let dir = planet_testkit::repo_root().join("target").join("review").join("regions");
    std::fs::create_dir_all(&dir).unwrap();
    planet_testkit::write_png(&dir.join("view3-off.png"), &plain).unwrap();
    planet_testkit::write_png(&dir.join("view3-regions.png"), &img).unwrap();
}

// spec: VIEW-030
#[test]
fn the_tint_lands_where_the_polygon_is_not_mirrored_not_swapped_and_with_every_edge() {
    use planet_frame::regions::RegionList;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3, SIZE, 2.5);
    let camera = p.camera.position.0;
    let mut r = renderer(&ctx, &p);
    let _ = frame(&mut r, &ctx, &p, Overlay::None);
    let plain_hdr = r.lit_path().unwrap().read_hdr(&ctx);
    // An L, 16 km across, not symmetric under a swap or a flip of the plane's axes; the corner (0, 0) is the elbow.
    // It is put where the view shows ground: on the sphere under the ray through the lower middle of the picture.
    let up = camera.normalized();
    let (e1, e2) = planet_world_def::region::basis(up);
    let (_, above, forward) = p.camera.ray_frame();
    let ray = (forward - above * 0.6).normalized();
    let b = camera.dot(ray);
    let t = -b - libm::sqrt(b * b - (camera.dot(camera) - R * R));
    let hit = (camera + ray * t).normalized();
    let centre = [R * hit.dot(e1), R * hit.dot(e2)];
    let l: [[f32; 2]; 6] = [[-8e3, -8e3], [8e3, -8e3], [8e3, 0.0], [0.0, 0.0], [0.0, 8e3], [-8e3, 8e3]]
        .map(|[x, y]| [x + centre[0] as f32, y + centre[1] as f32]);
    let poly = poly_under(camera, l.to_vec(), [0.2, 0.6, 0.1]);
    let (_, hdr) = frame_regions(&mut r, &ctx, &p, &RegionList { polys: vec![poly], dropped: 0 });
    let inside = |q: [f64; 2]| {
        let mut inside = false;
        let mut j = l.len() - 1;
        for i in 0..l.len() {
            let (a, b) = ([f64::from(l[i][0]), f64::from(l[i][1])], [f64::from(l[j][0]), f64::from(l[j][1])]);
            if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] {
                inside = !inside;
            }
            j = i;
        }
        inside
    };
    let edge_distance = |q: [f64; 2]| {
        (0..l.len())
            .map(|i| {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                segment_distance(q, [f64::from(a[0]), f64::from(a[1])], [f64::from(b[0]), f64::from(b[1])])
            })
            .fold(f64::MAX, f64::min)
    };
    // Ground points of the plane, every 1.5 km over and around the L, seen on the screen: the pixel there is tinted exactly when the point
    // is inside (a point near an edge is skipped: the terrain's height moves what a pixel shows by a few hundred metres).
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    let (mut seen_in, mut seen_out) = (0, 0);
    for i in -8..=8 {
        for j in -8..=8 {
            let q = [centre[0] + 1_500.0 * f64::from(i) + 120.0, centre[1] + 1_500.0 * f64::from(j) + 70.0];
            if edge_distance(q) < 2_500.0 {
                continue;
            }
            let dir = (up * R + e1 * q[0] + e2 * q[1]).normalized();
            let c = p.camera.project(dir * R - camera);
            if c[3] <= 0.0 {
                continue;
            }
            let (nx, ny) = (c[0] / c[3], c[1] / c[3]);
            if nx.abs() > 0.98 || ny.abs() > 0.98 {
                continue;
            }
            let (px, py) = (((nx * 0.5 + 0.5) * w as f64) as usize, ((0.5 - ny * 0.5) * h as f64) as usize);
            let at = py * w + px;
            if plain_hdr[at][3] <= 0.0 {
                continue;
            }
            let tinted = plain_hdr[at] != hdr[at];
            assert_eq!(tinted, inside(q), "ground point ({:.0}, {:.0}) m at pixel ({px}, {py})", q[0], q[1]);
            if tinted {
                seen_in += 1;
            } else {
                seen_out += 1;
            }
        }
    }
    eprintln!("L polygon: {seen_in} points inside and {seen_out} outside seen");
    assert!(seen_in >= 4 && seen_out >= 4, "the view shows both sides ({seen_in} inside, {seen_out} outside)");
}
