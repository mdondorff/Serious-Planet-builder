//! `core`: f64 frames, reference surface and geo conversions, cube-sphere addressing, integer
//! hashing and RNG. Bottom of the crate stack (CON-02): no graphics dependency, and all
//! transcendental functions come from `libm` (CON-14, ADR 0010).

pub mod cube;
pub mod frames;
pub mod hash;
pub mod surface;
pub mod tile;
pub mod vec3;

pub use cube::{Face, FaceMapping, TangentWarp};
pub use frames::{CameraRelative, PlanetFixed, TileLocal};
pub use surface::{Geo, ReferenceSurface, Sphere};
pub use tile::{Corner, Side, TileId};
pub use vec3::Vec3;
