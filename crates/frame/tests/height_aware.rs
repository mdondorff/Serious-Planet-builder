//! LOD-008: selection and morph see the terrain height, so the camera is never inside the drawn surface.
//!
//! The drawn surface is rebuilt on the CPU the way the shader draws it: the leaf's mesh, each vertex morphed towards the
//! parent grid by its distance to the camera and the plan's interval.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainDraw, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::{grid_indices, tile_mesh_field};
use planet_generators::stress::Steep;

const R: f64 = 6_371_000.0;
const NEAR: f64 = 0.1;

fn reference_params() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 96.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// The steep field's relief at tile scale needs a larger ratio of split distance to tile size than the reference
/// field's, or no crack-free morph interval exists (the plan rejects such parameters).
fn steep_params() -> LodParams {
    LodParams { cells: 4, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() }
}

/// The leaf's grid vertices as the shader draws them (morphed by their distance to the camera), camera-relative, f64.
fn drawn_vertices(field: &dyn HeightField, d: &TerrainDraw, cam: PlanetFixed, cells: u32) -> Vec<Vec3> {
    let mesh = tile_mesh_field(field, &TangentWarp, d.tile, cells, R);
    mesh.vertices
        .iter()
        .map(|v| {
            let at = |p: [f32; 3]| mesh.origin.0 + Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
            let (fine, coarse) = (at(v.pos), at(v.coarse));
            let dist = (fine - cam.0).length();
            let m = ((dist - f64::from(d.morph_start_m)) / f64::from((d.morph_end_m - d.morph_start_m).max(1e-6))).clamp(0.0, 1.0);
            fine * (1.0 - m) + coarse * m - cam.0
        })
        .collect()
}

/// Closest point of triangle `abc` to the origin (Ericson, Real-Time Collision Detection 5.1.5), as a distance.
fn origin_distance_to_triangle(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (ab, ac) = (b - a, c - a);
    let (d1, d2) = (ab.dot(-a), ac.dot(-a));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a.length();
    }
    let bp = -b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b.length();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (a + ab * (d1 / (d1 - d3))).length();
    }
    let cp = -c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c.length();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (a + ac * (d2 / (d2 - d6))).length();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return (b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)))).length();
    }
    let denom = 1.0 / (va + vb + vc);
    (a + ab * (vb * denom) + ac * (vc * denom)).length()
}

/// Smallest distance from the camera (the origin) to the drawn surface of one leaf.
fn distance_to_drawn_surface(drawn: &[Vec3], cells: u32) -> f64 {
    grid_indices(cells)
        .chunks(3)
        .map(|t| origin_distance_to_triangle(drawn[t[0] as usize], drawn[t[1] as usize], drawn[t[2] as usize]))
        .fold(f64::MAX, f64::min)
}

/// Greatest `s` (metres along `up` from the camera) at which the radial line through the camera crosses the drawn leaf.
fn drawn_surface_offset(drawn: &[Vec3], cam: PlanetFixed, cells: u32) -> Option<f64> {
    let up = cam.0.normalized();
    let mut best: Option<f64> = None;
    for tri in grid_indices(cells).chunks(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| drawn[i as usize]);
        let normal = (b - a).cross(c - a);
        let denom = normal.dot(up);
        if denom.abs() < 1e-30 {
            continue;
        }
        let s = normal.dot(a) / denom;
        let p = up * s;
        // Barycentric test of the hit point.
        let (v0, v1, v2) = (b - a, c - a, p - a);
        let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
        let den = d00 * d11 - d01 * d01;
        if den.abs() < 1e-30 {
            continue;
        }
        let (u, w) = ((d11 * d20 - d01 * d21) / den, (d00 * d21 - d01 * d20) / den);
        // Views on a tile border put the radial line exactly on a shared edge.
        let eps = 1e-6;
        if u >= -eps && w >= -eps && u + w <= 1.0 + eps {
            best = Some(best.map_or(s, |x: f64| x.max(s)));
        }
    }
    best
}

fn check_field(field: &dyn HeightField, label: &str, params: LodParams) {
    let map = TangentWarp;
    let mut failures = Vec::new();
    for (name, cam) in scripted_views_field(R, 4.0 / 3.0, field) {
        let req = TerrainRequest {
            camera: cam,
            size: (128, 96),
            view: TerrainView::Level,
            lod: params.clone(),
            face_mapping: "tangent-v1".into(),
            seed: field.seed(),
        };
        let plan = plan_terrain_field(&req, field).unwrap();
        let under = TileId::from_direction(&map, cam.position.0.normalized(), 26);
        let contains = |t: TileId| std::iter::successors(Some(under), |x| x.parent()).any(|x| x == t);
        let Some(d) = plan.nodes.iter().find(|d| contains(d.tile)) else {
            // Looking obliquely from high up, the frustum may leave the nadir out; near the ground it must not.
            let altitude = cam.position.0.length() - R;
            if altitude < 10_000.0 {
                failures.push(format!("{label} {name}: no leaf under the camera"));
            }
            continue;
        };
        let drawn = drawn_vertices(field, d, cam.position, params.cells);
        let Some(s) = drawn_surface_offset(&drawn, cam.position, params.cells) else {
            failures.push(format!("{label} {name}: the radial line through the camera misses leaf {:?}", d.tile));
            continue;
        };
        // Height of the camera above the exact terrain, and the projected sample spacing of the leaf there.
        let dir = cam.position.0.normalized();
        let above_terrain = cam.position.0.length() - (R + field.height(dir));
        let spacing = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << d.level) / f64::from(params.cells);
        let px = spacing * params.viewport_h_px / (2.0 * libm::tan(60f64.to_radians() / 2.0) * above_terrain.max(NEAR));
        eprintln!(
            "{label} {name}: leaf level {}, drawn surface {:.3} m below the camera, {above_terrain:.1} m above terrain, {px:.2} px",
            d.level, -s
        );
        if s > -NEAR {
            failures.push(format!(
                "{label} {name}: camera is {:.3} m above the drawn surface of leaf {:?} (level {}); need at least the near plane {NEAR} m",
                -s, d.tile, d.level
            ));
        }
        // The nearest drawn surface among the leaf and every node whose volume reaches the camera: a surface that rises
        // beside the camera is as bad as one under it.
        let mut nearest = distance_to_drawn_surface(&drawn, params.cells);
        for other in plan.nodes.iter().filter(|o| o.tile != d.tile) {
            let reach = 2.0 * params.node_radius_bound(other.level);
            if (other.origin.0 - cam.position.0).length() < 1.5 * reach {
                let v = drawn_vertices(field, other, cam.position, params.cells);
                nearest = nearest.min(distance_to_drawn_surface(&v, params.cells));
            }
        }
        if nearest < NEAR {
            failures.push(format!(
                "{label} {name}: the drawn surface is {nearest:.3} m from the camera, inside the near plane {NEAR} m (leaf {:?}, level {})",
                d.tile, d.level
            ));
        }
        if d.level < params.max_level && px > params.tau_px * 1.0001 {
            failures.push(format!(
                "{label} {name}: leaf {:?} (level {}) projects its sample spacing to {px:.2} px at the camera height {above_terrain:.1} m; tau_px is {}",
                d.tile, d.level, params.tau_px
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// spec: LOD-008
#[test]
fn camera_is_above_the_drawn_surface_in_every_scripted_view_on_the_reference_field() {
    check_field(&Reference(1), "reference", reference_params());
}

// spec: LOD-008
// spec: TEST-015
#[test]
fn camera_is_above_the_drawn_surface_in_every_scripted_view_on_the_steep_field() {
    check_field(&Steep(1), "steep", steep_params());
}

// spec: LOD-008
#[test]
fn terrain_too_steep_for_the_parameters_is_a_plan_error_naming_the_fix() {
    // Production-like parameters (cells 32, 240 pixels of tile detail per ratio) leave the steep field's neighbourhoods no
    // crack-free morph interval at ground level: the plan must say so, not narrow the interval.
    let field = Steep(1);
    let lod = LodParams { cells: 32, viewport_h_px: 1440.0, tau_px: 6.0, max_level: 26, ..LodParams::earth_1080p() };
    let (name, cam) = scripted_views_field(R, 16.0 / 9.0, &field)[5];
    let req = TerrainRequest { camera: cam, size: (256, 144), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 };
    let e = plan_terrain_field(&req, &field).expect_err(name).to_string();
    assert!(e.contains("morph interval") && e.contains("viewport_h_px / tau_px 240") && e.contains("reduce cells"), "{e}");
    // The reference field plans the same request.
    assert!(plan_terrain_field(&req, &Reference(1)).is_ok());
}
