//! `core`: f64 frames, reference surface and geo conversions, cube-sphere addressing, integer
//! hashing and RNG. Bottom of the crate stack (CON-02): no graphics dependency, and all
//! transcendental functions come from `libm` (CON-14, ADR 0010).

pub mod cube;
pub mod cube_patch;
pub mod frames;
pub mod global_cell;
pub mod hash;
pub mod math;
pub mod par;
pub mod surface;
pub mod thread;
pub mod tile;
pub mod vec3;

pub use cube::{Face, FaceMapping, TangentWarp};
pub use cube_patch::CubePatch;
pub use frames::{CameraRelative, PlanetFixed, TileLocal};
pub use global_cell::GlobalCell;
pub use math::smoothstep;
pub use surface::{Geo, ReferenceSurface, Sphere, EARTH_RADIUS_M};
pub use tile::{Corner, Side, TileId};
pub use vec3::Vec3;
