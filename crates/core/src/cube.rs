//! Cube-sphere faces and the face warp (ADR 0003).
//!
//! Face coordinates `(s, t)` lie in [-1, 1]². The warp maps them to plane coordinates `(u, v)`; the
//! direction is `normalize(n + u·U + v·V)`. Evaluation is arranged so that points on a shared face edge
//! produce bit-identical directions from both faces: edge values are exact (`u = ±1`), the warp is odd, and
//! every vector component is a sum with exact zeros.

use crate::vec3::Vec3;

/// Faces 0..5 = +X, -X, +Y, -Y, +Z, -Z of PlanetFixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Face(pub u8);

pub const FACE_COUNT: u8 = 6;

/// `(normal, U, V)` per face with `U × V = normal` (ADR 0003).
const AXES: [(Vec3, Vec3, Vec3); 6] = [
    (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(0.0, 1.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0)),
];

impl Face {
    pub const ALL: [Face; 6] = [Face(0), Face(1), Face(2), Face(3), Face(4), Face(5)];

    pub fn normal(self) -> Vec3 {
        AXES[self.0 as usize].0
    }
    pub fn u_axis(self) -> Vec3 {
        AXES[self.0 as usize].1
    }
    pub fn v_axis(self) -> Vec3 {
        AXES[self.0 as usize].2
    }
}

/// Warp between face coordinate `s` in [-1, 1] and plane coordinate `u`. Must be odd, monotone, and map
/// ±1 to ±1 exactly.
pub trait FaceMapping {
    /// Part of the generator `implementation_id` (changing the warp changes every cache key).
    fn id(&self) -> &'static str;
    fn face_to_plane(&self, s: f64) -> f64;
    fn plane_to_face(&self, u: f64) -> f64;
}

/// Tangent-adjusted warp: `u = tan(s·π/4)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TangentWarp;

const QUARTER_PI: f64 = core::f64::consts::FRAC_PI_4;

impl FaceMapping for TangentWarp {
    fn id(&self) -> &'static str {
        "tangent-v1"
    }

    fn face_to_plane(&self, s: f64) -> f64 {
        if s.abs() == 1.0 {
            return s; // tan(π/4) is not exactly 1 in floating point; the edge must be exact.
        }
        libm::tan(s * QUARTER_PI)
    }

    fn plane_to_face(&self, u: f64) -> f64 {
        if u.abs() >= 1.0 {
            return u.signum();
        }
        libm::atan(u) / QUARTER_PI
    }
}

/// Unit direction for face coordinates (extended coordinates beyond ±1 continue onto the plane).
pub fn face_to_direction(map: &dyn FaceMapping, face: Face, s: f64, t: f64) -> Vec3 {
    let (n, uax, vax) = AXES[face.0 as usize];
    (n + uax * map.face_to_plane(s) + vax * map.face_to_plane(t)).normalized()
}

/// Face containing `dir` and its face coordinates. The face is the axis with the largest absolute component;
/// ties go to X, then Y, then Z, and a non-negative component selects the positive face.
pub fn direction_to_face(map: &dyn FaceMapping, dir: Vec3) -> (Face, f64, f64) {
    let (ax, ay, az) = (dir.x.abs(), dir.y.abs(), dir.z.abs());
    let face = if ax >= ay && ax >= az {
        if dir.x >= 0.0 {
            0
        } else {
            1
        }
    } else if ay >= az {
        if dir.y >= 0.0 {
            2
        } else {
            3
        }
    } else if dir.z >= 0.0 {
        4
    } else {
        5
    };
    let face = Face(face);
    let w = dir.dot(face.normal());
    assert!(w > 0.0 && w.is_finite(), "direction {dir:?} is zero or not finite");
    let u = dir.dot(face.u_axis()) / w;
    let v = dir.dot(face.v_axis()) / w;
    (face, map.plane_to_face(u), map.plane_to_face(v))
}
