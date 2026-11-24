//! Tier C, TEST-153: the lit shading of a smooth scarp does not depend on its direction against the mesh's cell diagonal (REND-068, lead #200).
//! A scarp across the centre of face 0, once at 45 degrees and once at 135 degrees to the tile grid, each lit by a low sun across it: at the face
//! centre the equiangular grid, the nadir camera and the field are symmetric under a quarter turn about the face normal, so the 135-degree picture
//! turned back by 90 degrees is the 45-degree picture, except where the triangulation shows (a quarter turn maps each cell's a-c diagonal to b-d).
//! The images and their difference go to `target/review/quad-split/`.

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain_field, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::HeightField;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_testkit::{repo_root, write_png, Image};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (256, 256);
const CELLS: u32 = 16;
/// The finest level: tiles of about 610 m, cells of about 38 m, about 16 px a cell from the camera height below.
const FINEST: u8 = 14;
const ALT_M: f64 = 530.0;
/// A dim exposure: a sun 10 degrees high lights the ground weakly, and the pictures should use the range.
const EV: f32 = 11.0;
/// The scarp: `H tanh(d / W)` across a line through the face centre (d the distance from the line in metres).
const H_M: f64 = 30.0;
const W_M: f64 = 60.0;

/// A smooth scarp through the centre of face 0, its line at `angle_deg` from the face's u axis towards its v axis.
struct Scarp {
    angle_deg: f64,
}

impl HeightField for Scarp {
    fn height(&self, dir: Vec3) -> f64 {
        let f = Face(0);
        let (n, u, v) = (f.normal(), f.u_axis(), f.v_axis());
        // gnomonic coordinates on face 0's tangent plane, in metres at the centre
        let k = dir.dot(n);
        let (x, y) = (R * dir.dot(u) / k, R * dir.dot(v) / k);
        let a = self.angle_deg.to_radians();
        // the signed distance from the line through the centre at angle `a`: along its normal (-sin a, cos a)
        let d = -x * libm::sin(a) + y * libm::cos(a);
        H_M * libm::tanh(d / W_M)
    }
    fn seed(&self) -> u64 {
        0x5ca_7900 + self.angle_deg as u64
    }
    fn implementation_id(&self) -> &'static str {
        "test-scarp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        H_M
    }
}

/// The lit picture of the scarp at `angle_deg`, the sun 10 degrees high across it (from the side its normal points to).
fn render(ctx: &GpuContext, angle_deg: f64) -> Image {
    let f = Face(0);
    let (n, u, v) = (f.normal(), f.u_axis(), f.v_axis());
    let field = Scarp { angle_deg };
    let camera = Camera::looking_at_centre(PlanetFixed(n * (R + ALT_M)), v, 60f64.to_radians(), 1.0, 1.0);
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    let mut plan: TerrainPlan = plan_terrain_field(
        &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: field.seed() },
        &field,
    )
    .unwrap();
    let a = angle_deg.to_radians();
    let across = (u * -libm::sin(a) + v * libm::cos(a)).normalized();
    let alt = 10f64.to_radians();
    let sun = (n * libm::sin(alt) + across * libm::cos(alt)).normalized();
    let mut lit = LitPlan::fixed(EV);
    lit.sun_dir = [sun.x as f32, sun.y as f32, sun.z as f32];
    lit.ground_detail = 0.0;
    plan.lit = Some(lit);
    let slots = 24 * 24;
    let pool =
        compact_format_pool_with((128 << 20) + 4 * u64::from(slots) * planet_generators::compact::slot_bytes(CELLS), CELLS, slots, 1)
            .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool);
    r.enable_lit(ctx);
    for node in &plan.nodes {
        if r.slot_of(node.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, node.tile, CELLS, R)).unwrap();
        }
    }
    r.render(ctx, &plan, false).unwrap();
    let c = r.read_color(ctx);
    Image::new(c.width, c.height, c.rgba)
}

/// The picture turned by a quarter turn (90 degrees, so that a feature at +u ends up at +v in the image's frame of the camera, whose up is +v).
fn quarter_turn(img: &Image, clockwise: bool) -> Image {
    let (w, h) = (img.width, img.height);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = if clockwise { (y, h - 1 - x) } else { (w - 1 - y, x) };
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
    Image::new(w, h, out)
}

/// The mean and the 99th percentile of the absolute luminance difference (0-255) over the central half of the picture.
fn difference(a: &Image, b: &Image) -> (f64, f64, Image) {
    let (w, h) = (a.width, a.height);
    let lum =
        |img: &Image, k: usize| 0.2126 * f64::from(img.rgba[k]) + 0.7152 * f64::from(img.rgba[k + 1]) + 0.0722 * f64::from(img.rgba[k + 2]);
    let mut ds = Vec::new();
    let mut diff = vec![0u8; (w * h * 4) as usize];
    for y in h / 4..3 * h / 4 {
        for x in w / 4..3 * w / 4 {
            let k = ((y * w + x) * 4) as usize;
            let d = (lum(a, k) - lum(b, k)).abs();
            ds.push(d);
            let v = (d * 8.0).min(255.0) as u8;
            diff[k..k + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    ds.sort_by(f64::total_cmp);
    let mean = ds.iter().sum::<f64>() / ds.len() as f64;
    (mean, ds[ds.len() * 99 / 100], Image::new(w, h, diff))
}

// spec: TEST-153, REND-068
#[test]
fn a_scarp_across_the_cell_diagonal_is_lit_as_one_along_it() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let along = render(&ctx, 45.0);
    let across = render(&ctx, 135.0);
    // the 135-degree scarp turned back by a quarter turn lies where the 45-degree one does, with its sun
    let back = quarter_turn(&across, true);
    let (mean, p99, diff) = difference(&along, &back);
    let dir = repo_root().join("target").join("review").join("quad-split");
    std::fs::create_dir_all(&dir).unwrap();
    write_png(&dir.join("scarp-045.png"), &along).unwrap();
    write_png(&dir.join("scarp-135.png"), &across).unwrap();
    write_png(&dir.join("scarp-135-turned.png"), &back).unwrap();
    write_png(&dir.join("difference-x8.png"), &diff).unwrap();
    eprintln!("TEST-153 quad split: luminance difference 45 against 135 turned back: mean {mean:.3}, p99 {p99:.3} (0-255)");
    // the picture is not empty: the scarp is lit on one side and shaded on the other
    let (lo, hi) = along.rgba.chunks(4).fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    assert!(hi > lo + 20, "the scarp shows: green {lo}..{hi}");
    // REND-068: the two pictures are one up to rounding. With the vertex normals interpolated over each triangle (before the change, measured on
    // WARP 2026-11-24) the difference was mean 1.94, p99 22.6 along the scarp's edges; with the bilinear texel normal it is mean 0.044, p99 0.93
    assert!(
        mean <= 0.3 && p99 <= 5.0,
        "the shading shows the cell diagonal: mean {mean:.3}, p99 {p99:.3} (bounds 0.3 and 5); see {}",
        dir.display()
    );
}
