//! Geo-coordinates and the `ReferenceSurface` interface (CON-15, ADR 0011).

use crate::frames::PlanetFixed;
use crate::vec3::Vec3;

/// Latitude and longitude in degrees, height in metres above the reference surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geo {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
}

impl Geo {
    pub const fn new(lat_deg: f64, lon_deg: f64, height_m: f64) -> Self {
        Self { lat_deg, lon_deg, height_m }
    }
}

pub const DEG_TO_RAD: f64 = core::f64::consts::PI / 180.0;
pub const RAD_TO_DEG: f64 = 180.0 / core::f64::consts::PI;

/// Conversion between geo-coordinates and `PlanetFixed`. Every implementation must pass the shared contract
/// suite in `tests/surface_contract.rs`.
pub trait ReferenceSurface {
    /// Identifier hashed into definitions and cache keys, e.g. `sphere:6371000`.
    fn id(&self) -> String;
    fn geo_to_planet_fixed(&self, g: Geo) -> PlanetFixed;
    fn planet_fixed_to_geo(&self, p: PlanetFixed) -> Geo;
    /// Unit outward normal of the surface through `p`.
    fn normal(&self, p: PlanetFixed) -> Vec3;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub radius_m: f64,
}

impl Sphere {
    /// Earth mean radius.
    pub const EARTH: Sphere = Sphere { radius_m: 6_371_000.0 };
}

impl ReferenceSurface for Sphere {
    fn id(&self) -> String {
        format!("sphere:{}", self.radius_m)
    }

    fn geo_to_planet_fixed(&self, g: Geo) -> PlanetFixed {
        let (lat, lon) = (g.lat_deg * DEG_TO_RAD, g.lon_deg * DEG_TO_RAD);
        let r = self.radius_m + g.height_m;
        let cl = libm::cos(lat);
        PlanetFixed(Vec3::new(r * cl * libm::cos(lon), r * cl * libm::sin(lon), r * libm::sin(lat)))
    }

    fn planet_fixed_to_geo(&self, p: PlanetFixed) -> Geo {
        let v = p.0;
        // atan2 on (z, hypot) stays accurate at the poles, unlike asin(z / r).
        let rho = (v.x * v.x + v.y * v.y).sqrt();
        let lat = libm::atan2(v.z, rho);
        let lon = libm::atan2(v.y, v.x);
        Geo::new(lat * RAD_TO_DEG, lon * RAD_TO_DEG, v.length() - self.radius_m)
    }

    fn normal(&self, p: PlanetFixed) -> Vec3 {
        p.0.normalized()
    }
}
