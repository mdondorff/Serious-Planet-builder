//! Camera, reversed-Z infinite-far projection (ADR 0002) and frustum planes, all float64 on the CPU (ADR 0001).

use planet_core::{PlanetFixed, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: PlanetFixed,
    /// Unit view direction.
    pub forward: Vec3,
    /// Approximate up; made orthogonal to `forward`.
    pub up: Vec3,
    pub fov_y_rad: f64,
    pub aspect: f64,
    /// Near plane distance, metres (the far plane is at infinity).
    pub near_m: f64,
}

/// Orthonormal camera basis (right, up, forward).
/// The world axis least aligned with `f`, a safe fallback when `up` is (nearly) parallel to the view direction.
fn least_aligned_axis(f: Vec3) -> Vec3 {
    let (ax, ay, az) = (f.x.abs(), f.y.abs(), f.z.abs());
    if ax <= ay && ax <= az {
        Vec3::new(1.0, 0.0, 0.0)
    } else if ay <= az {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    }
}

fn basis(c: &Camera) -> (Vec3, Vec3, Vec3) {
    let f = c.forward.normalized();
    let mut r = f.cross(c.up);
    if r.length() < 1e-9 {
        r = f.cross(least_aligned_axis(f));
    }
    let r = r.normalized();
    let u = r.cross(f);
    (r, u, f)
}

impl Camera {
    /// Local frame at `position` on a sphere centred at the origin: look along the horizon direction `heading`
    /// (unit, tangent) pitched down by `pitch_rad`; at orbit use `looking_at_centre`.
    pub fn at_surface_point(position: PlanetFixed, heading_hint: Vec3, pitch_rad: f64, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        let up = position.0.normalized();
        let mut north = heading_hint - up * heading_hint.dot(up);
        if north.length() < 1e-9 {
            // Hint parallel to the local up (a pole for a north hint): any tangent will do.
            let axis = least_aligned_axis(up);
            north = axis - up * axis.dot(up);
        }
        let north = north.normalized();
        let forward = north * libm::cos(pitch_rad) + (-up) * libm::sin(pitch_rad);
        Camera { position, forward, up, fov_y_rad, aspect, near_m }
    }

    pub fn looking_at_centre(position: PlanetFixed, up_hint: Vec3, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        Camera { position, forward: (-position.0).normalized(), up: up_hint, fov_y_rad, aspect, near_m }
    }

    /// `view_projection` for camera-relative coordinates (translation already removed), column-major, rounded once to f32.
    /// View space looks down -z; reversed-Z with infinite far: `ndc.z = near / distance_along_view`, in (0, 1].
    pub fn view_projection(&self) -> [f32; 16] {
        let (r, u, f) = basis(self);
        let t = libm::tan(self.fov_y_rad / 2.0);
        let (sx, sy) = (1.0 / (t * self.aspect), 1.0 / t);
        // Rows of P * V, with V rows (r, u, -f).
        let rows =
            [[sx * r.x, sx * r.y, sx * r.z, 0.0], [sy * u.x, sy * u.y, sy * u.z, 0.0], [0.0, 0.0, 0.0, self.near_m], [f.x, f.y, f.z, 0.0]];
        let mut m = [0.0f32; 16];
        for (row, vals) in rows.iter().enumerate() {
            for (col, v) in vals.iter().enumerate() {
                m[col * 4 + row] = *v as f32;
            }
        }
        m
    }

    /// Clip-space `(x, y, z, w)` of a camera-relative point evaluated in f64 (reference for the GPU's f32 evaluation).
    pub fn project(&self, rel: Vec3) -> [f64; 4] {
        let (r, u, f) = basis(self);
        let t = libm::tan(self.fov_y_rad / 2.0);
        let d = rel.dot(f);
        [rel.dot(r) / (t * self.aspect), rel.dot(u) / t, self.near_m, d]
    }

    /// The four side planes as `(unit inward normal)`, all passing through the camera position.
    pub fn frustum(&self) -> Frustum {
        let (r, u, f) = basis(self);
        let ty = libm::tan(self.fov_y_rad / 2.0);
        let tx = ty * self.aspect;
        let n = |v: Vec3| v.normalized();
        Frustum { origin: self.position.0, normals: [n(f * tx + r), n(f * tx - r), n(f * ty + u), n(f * ty - u)] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub origin: Vec3,
    /// Inward-pointing unit normals of left, right, bottom and top planes.
    pub normals: [Vec3; 4],
}

impl Frustum {
    /// True when a sphere lies entirely outside one of the side planes.
    pub fn culls_sphere(&self, centre: Vec3, radius: f64) -> bool {
        let rel = centre - self.origin;
        self.normals.iter().any(|n| rel.dot(*n) < -radius)
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        let rel = p - self.origin;
        self.normals.iter().all(|n| rel.dot(*n) >= 0.0)
    }
}
