//! `generators`: pure, versioned field generators (CON-06..08, ADR 0006, ADR 0010).
//!
//! M1 holds the CPU reference height generator for one tile and the CPU dumps used to look at data without a
//! renderer. Everything here is integer hashing plus IEEE `+ - * /` and `libm::floor`, so results are
//! bit-identical across platforms and thread counts.

pub mod dump;
pub mod erosion;
pub mod global;
pub mod height;
pub mod manifest;
pub mod mesh;
pub mod stage;
/// Test-only stress terrain; only test builds enable the `stress` feature (dev-dependencies).
#[cfg(feature = "stress")]
pub mod stress;
pub mod terrain;

pub use height::{cache_key, generate_region, generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
