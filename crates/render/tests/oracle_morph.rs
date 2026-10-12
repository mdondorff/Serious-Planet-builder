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

/// The shader's formula in f64: distance from the camera to the vertex position, mapped over the node's
/// morph interval.
fn cpu_morph(plan: &TerrainPlan, node: usize, mesh: &TileMesh, vertex: usize) -> f64 {
    let d = &plan.nodes[node];
    let v = mesh.vertices[vertex];
    let reference = d.origin.0 - plan.camera.position.0 + Vec3::new(f64::from(v.pos[0]), f64::from(v.pos[1]), f64::from(v.pos[2]));
    ((reference.length() - f64::from(d.morph_start_m)) / f64::from(d.morph_end_m - d.morph_start_m)).clamp(0.0, 1.0)
}

/// The height view's observable at a vertex: the shader's morphed height minus the vertex's own, as a fraction of the
/// plan's height range used as the delta window (the `morph-height` view writes exactly this to the grey channels).
fn cpu_morph_height(plan: &TerrainPlan, node: usize, mesh: &TileMesh, vertex: usize) -> f64 {
    let v = mesh.vertices[vertex];
    let delta = cpu_morph(plan, node, mesh, vertex) * (f64::from(v.coarse_height) - f64::from(v.height));
    let (lo, hi) = (f64::from(plan.height_range[0]), f64::from(plan.height_range[1]));
    ((delta - lo) / (hi - lo)).clamp(0.0, 1.0)
}

// spec: REND-014
#[test]
fn the_vertex_shader_morph_matches_the_cpu_twin_within_a_data_derived_bound() {
    check_vertex_function(
        Oracle {
            view: TerrainView::Morph,
            value: cpu_morph,
            views: &[2, 3, 4],
            neutral: 0.0,
            max_offset_px: f64::MAX,
            share_factor: 1.5,
            min_tested: 300,
            min_informative: 30,
            max_worst_error: 0.05,
        },
        |_| None,
    );
}

// spec: REND-014, REND-009
#[test]
fn the_vertex_shader_height_morph_matches_the_cpu_twin_within_a_data_derived_bound() {
    // Only the high views (2,000 and 200 km): from lower altitude on the smooth reference terrain the morph moves heights by millimetres.
    // The delta window is the median nonzero |coarse_height - height| over the examined meshes, so the 8-bit grey resolves
    // a typical delta to a few percent of itself (larger ones saturate on the GPU and the CPU twin alike). The test fails
    // if the shader leaves the height unmorphed (always the neutral grey) or mixes it with another factor.
    let oracle = Oracle {
        view: TerrainView::MorphHeight,
        value: cpu_morph_height,
        views: &[1, 2],
        neutral: 0.5,
        max_offset_px: 0.15,
        share_factor: 5.0,
        min_tested: 60,
        min_informative: 60,
        max_worst_error: 0.25,
    };
    check_vertex_function(oracle, |meshes| {
        let mut deltas: Vec<f64> = meshes
            .iter()
            .flat_map(|m| m.vertices.iter())
            .map(|v| (f64::from(v.coarse_height) - f64::from(v.height)).abs())
            .filter(|d| *d > 0.0)
            .collect();
        deltas.sort_by(f64::total_cmp);
        let window = deltas[deltas.len() / 2];
        assert!(window > 0.01, "the meshes barely morph in height ({window} m): the oracle would mean nothing");
        Some([-window as f32, window as f32])
    });
}

/// One per-vertex function shown by a debug view (values in 0..=1 as drawn) and how tightly its pixels are compared.
struct Oracle {
    view: TerrainView,
    /// The CPU twin: plan, node, mesh, vertex.
    value: fn(&TerrainPlan, usize, &TileMesh, usize) -> f64,
    /// Scripted views to compare in.
    views: &'static [usize],
    /// What a vertex that does not morph shows.
    neutral: f64,
    /// Only pixel centres this close (pixels) to their vertex are compared.
    max_offset_px: f64,
    /// Scale of the interpolation share in the derived bound (1.5: smooth; more where neighbours alternate).
    share_factor: f64,
    min_tested: usize,
    /// At least this many compared vertices must actually morph (so a shader that skips the morph cannot pass).
    min_informative: usize,
    /// The worst observed error must stay below this, so the derived bound cannot be loosened unnoticed.
    max_worst_error: f64,
}

/// Runs the pixel-versus-CPU-twin comparison of `oracle`; `window` may set the plan's height range from the meshes.
fn check_vertex_function(oracle: Oracle, window: impl Fn(&[TileMesh]) -> Option<[f32; 2]>) {
    let Oracle { view, value, views, neutral, max_offset_px, share_factor, min_tested, min_informative, max_worst_error } = oracle;
    let ctx = GpuContext::new(AdapterPolicy::Software).expect("software adapter");
    let (mut tested, mut informative, mut worst_excess, mut worst_err) = (0usize, 0usize, f64::MIN, 0.0f64);
    for &index in views {
        let (mut morph_plan, id_plan) = (plan(index, view), plan(index, TerrainView::TileId));
        let meshes: Vec<TileMesh> = morph_plan.nodes.iter().map(|n| tile_mesh(SEED, &TangentWarp, n.tile, CELLS, R)).collect();
        if let Some(range) = window(&meshes) {
            morph_plan.height_range = range;
        }
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
                    // (The pixel is found from the vertex's own position; a morphing vertex is drawn at `mix(pos, coarse, m)`.
                    // At the high views compared here that moves it by a small fraction of a pixel; a lower view would have to
                    // project the morphed position.)
                    let (px, py) = (fx.floor() as u32, fy.floor() as u32);
                    // Only pixels that show this node (not a nearer one).
                    if ids.pixel(px, py) != id_plan.nodes[n].color {
                        continue;
                    }
                    let want = value(&morph_plan, n, m, k);
                    let (dx, dy) = (fx - f64::from(px) - 0.5, fy - f64::from(py) - 0.5);
                    let off = (dx * dx + dy * dy).sqrt();
                    // Adjacent vertices may differ completely (odd ones morph, even ones do not), so a view of such a
                    // function is compared only where the pixel centre is close to its vertex.
                    if off > max_offset_px {
                        continue;
                    }
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
                            (value(&morph_plan, n, m, nb) - want).abs() * (share_factor * off / dist).min(1.0)
                        })
                        .fold(0.0, f64::max);
                    let bound = steepest + 2.0 / 255.0;
                    let got = f64::from(img.pixel(px, py)[0]) / 255.0;
                    let err = (got - want).abs();
                    tested += 1;
                    // Vertices that actually morph (the view shows something other than the unmorphed value).
                    informative += usize::from((want - neutral).abs() > 4.0 / 255.0);
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
    assert!(tested >= min_tested, "only {tested} vertices could be compared");
    assert!(
        informative >= min_informative,
        "only {informative} of {tested} compared vertices actually morph: a shader that skips the morph would pass"
    );
    // The derived bound must be tight enough to mean something (a few 8-bit steps for the smooth morph value; for the
    // height delta, whose neighbours alternate between morphed and not, a fraction of the window).
    assert!(worst_err < max_worst_error, "worst error {worst_err} (excess over bound {worst_excess}) is too large for a tight oracle");
}
