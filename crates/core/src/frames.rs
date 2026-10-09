//! Typed positions per frame (ADR 0001) so frames cannot be mixed by accident.

use crate::vec3::Vec3;

/// Authoritative position, float64, planet-fixed frame (+Z north pole, +X at lat 0 / lon 0, +Y at lon 90°E).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlanetFixed(pub Vec3);

/// Offset from a tile origin (the tile centre in `PlanetFixed`), float32.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileLocal(pub [f32; 3]);

/// Offset from the camera, float32: what shaders receive.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraRelative(pub [f32; 3]);

impl PlanetFixed {
    /// `self - camera` computed in f64, then rounded once to f32 (COORD-002). The subtraction is exact
    /// enough that the only error is the final rounding, bounded by the size of the offset, not of the planet.
    pub fn camera_relative(self, camera: PlanetFixed) -> CameraRelative {
        let d = self.0 - camera.0;
        CameraRelative([d.x as f32, d.y as f32, d.z as f32])
    }

    /// Offset of `self` from a tile origin, rounded to f32.
    pub fn tile_local(self, origin: PlanetFixed) -> TileLocal {
        let d = self.0 - origin.0;
        TileLocal([d.x as f32, d.y as f32, d.z as f32])
    }
}

impl TileLocal {
    /// Back to `PlanetFixed` given the tile origin.
    pub fn to_planet_fixed(self, origin: PlanetFixed) -> PlanetFixed {
        PlanetFixed(origin.0 + Vec3::new(f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2])))
    }
}

impl CameraRelative {
    pub fn to_vec3(self) -> Vec3 {
        Vec3::new(f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2]))
    }
}
