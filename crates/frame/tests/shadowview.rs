//! REND-050, tier A: the camera and the sun in the clipmap anchor's tangent frame. A camera over the anchor stands on the frame's z axis;
//! the basis stays orthonormal; the sun keeps its elevation at the anchor; a point on the sphere lands at `z = -drop`, the frame the
//! clipmap's heights are stored in.

use planet_core::{PlanetFixed, Vec3};
use planet_frame::clipmap::plane_position;
use planet_frame::shadowview::ShadowView;
use planet_frame::Camera;

const R: f64 = 6_371_000.0;

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn camera_over(anchor: Vec3, height: f64, heading_hint: Vec3, pitch: f64) -> Camera {
    Camera::at_surface_point(PlanetFixed(anchor * (R + height)), heading_hint, pitch, 1.0, 16.0 / 9.0, 0.5)
}

// spec: REND-050
#[test]
fn a_camera_over_the_anchor_stands_on_the_z_axis_with_an_orthonormal_basis() {
    for anchor in [Vec3::new(0.3, -0.5, 0.8).normalized(), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0)] {
        let cam = camera_over(anchor, 1_234.0, Vec3::new(0.2, 0.9, 0.1), -0.4);
        let sun = (anchor * 0.5 + Vec3::new(0.0, 1.0, 0.0)).normalized();
        let v = ShadowView::new(anchor, &cam, sun, R);
        assert!(v.position[0].abs() < 1e-3 && v.position[1].abs() < 1e-3, "x, y of {:?}", v.position);
        assert!((v.position[2] - 1_234.0).abs() < 1e-3, "height {}", v.position[2]);
        for (name, a) in [("forward", v.forward), ("right", v.right), ("up", v.up)] {
            assert!((dot(a, a) - 1.0).abs() < 1e-6, "{name} is a unit vector");
        }
        assert!(dot(v.forward, v.right).abs() < 1e-6 && dot(v.forward, v.up).abs() < 1e-6 && dot(v.right, v.up).abs() < 1e-6);
        assert!((v.tan_half_fov[1] - libm::tan(0.5) as f32).abs() < 1e-6);
        assert!((v.tan_half_fov[0] - (libm::tan(0.5) * 16.0 / 9.0) as f32).abs() < 1e-6);
        assert_eq!(v.near_m, 0.5);
        assert!((v.drop_coefficient - (0.5 / R) as f32).abs() < 1e-14);
    }
}

// spec: REND-050
#[test]
fn the_sun_keeps_its_elevation_at_the_anchor() {
    let anchor = Vec3::new(0.6, 0.0, 0.8);
    let (east, _north) = anchor.tangent_basis();
    for deg in [-3.0f64, 0.0, 2.0, 45.0, 90.0] {
        let e = deg.to_radians();
        let sun = anchor * libm::sin(e) + east * libm::cos(e);
        let cam = camera_over(anchor, 10.0, east, 0.0);
        let v = ShadowView::new(anchor, &cam, sun, R);
        assert!((f64::from(v.sun[2]) - libm::sin(e)).abs() < 1e-6, "sun elevation {deg}: z = {}", v.sun[2]);
        assert!((f64::from(v.sun[0]) - libm::cos(e)).abs() < 1e-6, "sun east component at {deg}");
    }
}

// spec: REND-050
#[test]
fn a_surface_point_lands_where_the_euclidean_tangent_frame_puts_it() {
    let anchor = Vec3::new(-0.2, 0.7, 0.4).normalized();
    let (e1, e2) = anchor.tangent_basis();
    for (x, y) in [(1_000.0, 0.0), (0.0, -20_000.0), (30_000.0, 40_000.0), (-100_000.0, 20_000.0), (500_000.0, 300_000.0)] {
        let rho = libm::hypot(x, y);
        let theta = rho / R;
        let d = (anchor * libm::cos(theta) + (e1 * x + e2 * y) * (libm::sin(theta) / rho)).normalized();
        assert!((plane_position(anchor, d, R).0 - x).abs() < 1e-3);
        let p = PlanetFixed(d * (R + 50.0));
        let cam = Camera::at_surface_point(p, e1, 0.0, 1.0, 1.0, 0.5);
        let v = ShadowView::new(anchor, &cam, anchor, R);
        // Exact Euclidean offsets: horizontal (R + h) sin(theta) along the plane direction, vertical (R + h) cos(theta) - R.
        let (ex, ey) = ((R + 50.0) * libm::sin(theta) * x / rho, (R + 50.0) * libm::sin(theta) * y / rho);
        let ez = (R + 50.0) * libm::cos(theta) - R;
        let tol = 1e-6 * (1.0 + rho);
        for (name, got, want) in [("x", v.position[0], ex), ("y", v.position[1], ey), ("z", v.position[2], ez)] {
            assert!((f64::from(got) - want).abs() < tol.max(1e-3), "rho {rho:.0} m, {name}: {got} against {want}");
        }
    }
}

// spec: REND-050
#[test]
fn the_basis_is_the_cameras_own_also_when_it_looks_along_its_up() {
    for dir in [Vec3::new(-0.6, 0.0, -0.8), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.3, 0.5, 0.8).normalized()] {
        let cam = Camera::at_surface_point(
            PlanetFixed(dir * (R + 1_000.0)),
            Vec3::new(0.2, 0.9, 0.1),
            core::f64::consts::FRAC_PI_2,
            1.0,
            16.0 / 9.0,
            0.5,
        );
        let v = ShadowView::new(dir.normalized(), &cam, dir.normalized(), R);
        let (r, u, f) = cam.ray_frame();
        let (east, north) = dir.normalized().tangent_basis();
        let local = |w: Vec3| [w.dot(east) as f32, w.dot(north) as f32, w.dot(dir.normalized()) as f32];
        let (rn, un) = (r.normalized(), u.normalized());
        for (name, got, want) in [("forward", v.forward, local(f)), ("right", v.right, local(rn)), ("up", v.up, local(un))] {
            for k in 0..3 {
                assert!((got[k] - want[k]).abs() < 1e-5, "{name}[{k}] at {dir:?}: {got:?} against {want:?}");
            }
        }
    }
}

// spec: REND-050
#[test]
fn the_ceiling_follows_the_field_and_adds_the_previews_bounds() {
    use planet_frame::preview::PreviewStamp;
    use planet_frame::shadowview::ceiling_m;
    assert_eq!(ceiling_m(9_000.0, &[]), 9_000.0);
    let stamp = |max_delta_m: f64| PreviewStamp {
        dir: Vec3::new(0.0, 0.0, 1.0),
        reach_m: 5_000.0,
        amount_m: max_delta_m,
        kind: planet_world_def::stamp::StampKind::Hill,
        sign: 1,
        max_delta_m,
    };
    assert_eq!(ceiling_m(9_000.0, &[stamp(1_500.0), stamp(200.0), stamp(-300.0)]), 11_000.0, "every stamp adds its bound, whichever its sign");
}
