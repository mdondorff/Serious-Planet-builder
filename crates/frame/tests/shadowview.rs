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
fn a_surface_point_lands_at_the_sphere_drop_in_the_frame_of_the_clipmap_heights() {
    let anchor = Vec3::new(-0.2, 0.7, 0.4).normalized();
    let (e1, e2) = anchor.tangent_basis();
    let c = 0.5 / R;
    for (x, y) in [(1_000.0, 0.0), (0.0, -20_000.0), (30_000.0, 40_000.0), (-100_000.0, 20_000.0)] {
        // The point at plane position (x, y) on the sphere (the azimuthal equidistant projection's inverse).
        let rho = libm::hypot(x, y);
        let d = (anchor * libm::cos(rho / R) + (e1 * x + e2 * y) * (libm::sin(rho / R) / rho)).normalized();
        assert!((plane_position(anchor, d, R).0 - x).abs() < 1e-6);
        // The point 50 m above the sphere, seen from a camera at the point itself, stands where the frame says.
        let p = PlanetFixed(d * (R + 50.0));
        let cam = Camera::at_surface_point(p, e1, 0.0, 1.0, 1.0, 0.5);
        let v = ShadowView::new(anchor, &cam, anchor, R);
        // Euclidean tangent coordinates: z = height - drop to first order (the chord against the arc is of the order of rho^3 / R^2).
        let expected_z = 50.0 - c * rho * rho;
        let chord = rho * rho * rho / (R * R);
        assert!(
            (f64::from(v.position[2]) - expected_z).abs() < 0.01 + chord,
            "rho {rho:.0} m: z {} against {expected_z} (tolerance {})",
            v.position[2],
            0.01 + chord
        );
    }
}
