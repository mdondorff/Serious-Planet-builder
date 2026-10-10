//! Tier B: the vertex shader's per-vertex morph against an f64 CPU twin, on the software adapter, with a bound derived
//! from the data (not a loose constant). The morph view writes the vertex morph value into the red channel, so the pixel
//! at a vertex shows that vertex's value up to (a) 8-bit quantisation and (b) the interpolation towards the neighbours
//! within the half pixel between the vertex and the pixel centre; both are bounded from the CPU values.

use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::mesh::{tile_mesh, TileMesh};
use planet_render::{execute_terrain, AdapterPolicy, GpuContext};
use planet_testkit::Image;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (256, 192);
const CELLS: u32 = 16;
const SEED: u64 = 1;

fn plan(index: usize, view: TerrainView) -> TerrainPlan {
    let (_, camera) = scripted_views(R, f64::from(SIZE.0) / f64::from(SIZE.1), SEED)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view, lod, face_mapping: "tangent-v1".into(), seed: SEED }).unwrap()
}

fn render(ctx: &GpuContext, plan: &TerrainPlan, meshes: &[TileMesh]) -> Image {
    let refs: Vec<&TileMesh> = meshes.iter().collect();
    let (frame, _) = execute_terrain(ctx, plan, &refs).unwrap();
    Image::new(frame.width, frame.height, frame.rgba)
}

/// The shader's formula in f64: distance from the camera to the height-free reference position, mapped over the node's
/// morph interval.
fn cpu_morph(plan: &TerrainPlan, node: usize, mesh: &TileMesh, vertex: usize) -> f64 {
    let d = &plan.nodes[node];
    let v = mesh.vertices[vertex];
    let reference =
        d.origin.0 - plan.camera.position.0 + Vec3::new(f64::from(v.ref_pos[0]), f64::from(v.ref_pos[1]), f64::from(v.ref_pos[2]));
    ((reference.length() - f64::from(d.morph_start_m)) / f64::from(d.morph_end_m - d.morph_start_m)).clamp(0.0, 1.0)
}

// spec: REND-014
#[test]
fn the_vertex_shader_morph_matches_the_cpu_twin_within_a_data_derived_bound() {
    let ctx = GpuContext::new(AdapterPolicy::Software).expect("software adapter");
    let (mut tested, mut worst_excess, mut worst_err) = (0usize, f64::MIN, 0.0f64);
    for index in [2usize, 3, 4] {
        let (morph_plan, id_plan) = (plan(index, TerrainView::Morph), plan(index, TerrainView::TileId));
        let meshes: Vec<TileMesh> = morph_plan.nodes.iter().map(|n| tile_mesh(SEED, &TangentWarp, n.tile, CELLS, R)).collect();
        let (img, ids) = (render(&ctx, &morph_plan, &meshes), render(&ctx, &id_plan, &meshes));
        let w = CELLS as usize + 1;
        for (n, (d, m)) in morph_plan.nodes.iter().zip(&meshes).enumerate() {
            if d.level == 0 {
                continue;
            }
            for j in 1..CELLS as usize {
                for i in 1..CELLS as usize {
                    let k = j * w + i;
                    let v = m.vertices[k];
                    let rel = d.origin.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]))
                        - morph_plan.camera.position.0;
                    let c = morph_plan.camera.project(rel);
                    let normal = Vec3::new(f64::from(v.normal[0]), f64::from(v.normal[1]), f64::from(v.normal[2]));
                    if c[3] <= 0.0 || normal.dot((-rel).normalized()) < 0.5 {
                        continue;
                    }
                    let (x, y) = (c[0] / c[3], c[1] / c[3]);
                    if x.abs() > 0.95 || y.abs() > 0.95 {
                        continue;
                    }
                    let (fx, fy) = ((x * 0.5 + 0.5) * f64::from(SIZE.0), (0.5 - y * 0.5) * f64::from(SIZE.1));
                    let (px, py) = (fx.floor() as u32, fy.floor() as u32);
                    // Only pixels that show this node (not a nearer one).
                    if ids.pixel(px, py) != id_plan.nodes[n].color {
                        continue;
                    }
                    let want = cpu_morph(&morph_plan, n, m, k);
                    let (dx, dy) = (fx - f64::from(px) - 0.5, fy - f64::from(py) - 0.5);
                    let off = (dx * dx + dy * dy).sqrt();
                    // The value at the pixel centre is the vertex value moved towards its neighbours by the share of
                    // the way the centre lies from the vertex: offset / neighbour distance in pixels (x1.5 for the
                    // triangle geometry and perspective), at most the whole step. 8-bit quantisation adds up to 2/255.
                    let to_px = |kk: usize| {
                        let vv = m.vertices[kk];
                        let r = d.origin.0 + Vec3::new(f64::from(vv.pos[0]), f64::from(vv.pos[1]), f64::from(vv.pos[2]))
                            - morph_plan.camera.position.0;
                        let q = morph_plan.camera.project(r);
                        ((q[0] / q[3] * 0.5 + 0.5) * f64::from(SIZE.0), (0.5 - q[1] / q[3] * 0.5) * f64::from(SIZE.1))
                    };
                    let steepest = [k - 1, k + 1, k - w, k + w, k - w - 1, k - w + 1, k + w - 1, k + w + 1]
                        .iter()
                        .map(|&nb| {
                            let (nx, ny) = to_px(nb);
                            let dist = ((nx - fx) * (nx - fx) + (ny - fy) * (ny - fy)).sqrt().max(1e-9);
                            (cpu_morph(&morph_plan, n, m, nb) - want).abs() * (1.5 * off / dist).min(1.0)
                        })
                        .fold(0.0, f64::max);
                    let bound = steepest + 2.0 / 255.0;
                    let got = f64::from(img.pixel(px, py)[0]) / 255.0;
                    let err = (got - want).abs();
                    tested += 1;
                    worst_err = worst_err.max(err);
                    worst_excess = worst_excess.max(err - bound);
                    assert!(
                        err <= bound,
                        "view {index}, tile {:?}, vertex ({i},{j}): GPU morph {got:.4} vs CPU {want:.4} (error {err:.4} > bound {bound:.4}; interpolation share {steepest:.4}, pixel offset {off:.2})",
                        d.tile
                    );
                }
            }
        }
    }
    assert!(tested >= 300, "only {tested} vertices could be compared");
    // The derived bound must be tight enough to mean something: the worst observed error is a few 8-bit steps.
    assert!(worst_err < 0.05, "worst error {worst_err} (excess over bound {worst_excess}) is too large for a tight oracle");
}
