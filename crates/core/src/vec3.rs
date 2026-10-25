//! Minimal f64 vector. Only IEEE-exact operations (`+ - * /` and `sqrt`) are used, so results are
//! identical on every platform.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Unit vector in the same direction. Panics on the zero vector (a bug, never data).
    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        assert!(l > 0.0 && l.is_finite(), "cannot normalise {self:?}");
        self * (1.0 / l)
    }

    pub fn distance(self, o: Vec3) -> f64 {
        (self - o).length()
    }

    /// The angle in radians between two directions, in `[0, pi]`. The `atan2` of the cross and dot products keeps its precision for tiny
    /// angles and for angles near pi, where `acos` of the dot product loses it.
    pub fn angle_to(self, o: Vec3) -> f64 {
        libm::atan2(self.cross(o).length(), self.dot(o))
    }

    /// The unit direction of a latitude and a longitude in radians (x towards lat 0 lon 0, z towards the north pole).
    pub fn from_lat_lon(lat_rad: f64, lon_rad: f64) -> Vec3 {
        Vec3::new(libm::cos(lat_rad) * libm::cos(lon_rad), libm::cos(lat_rad) * libm::sin(lon_rad), libm::sin(lat_rad))
    }

    /// Latitude and longitude in radians of a direction (it need not be a unit vector): the latitude from `atan2` of the height over the
    /// distance from the axis, which keeps its precision at the poles where `asin` loses it; the longitude is `atan2(y, x)` in `(-pi, pi]`
    /// and 0 on the axis. The inverse of [`Vec3::from_lat_lon`] away from the poles.
    pub fn lat_lon(self) -> (f64, f64) {
        (libm::atan2(self.z, libm::hypot(self.x, self.y)), libm::atan2(self.y, self.x))
    }

    /// An orthonormal pair `(e1, e2)` of the tangent plane at the unit vector `self`, right-handed with it (`e1 x e2 = self`). The helper
    /// axis is z, or x within 0.9 of a pole, so the pair is a fixed function of the direction.
    pub fn tangent_basis(self) -> (Vec3, Vec3) {
        let helper = if self.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let e1 = helper.cross(self).normalized();
        (e1, self.cross(e1))
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
