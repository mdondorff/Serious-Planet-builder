//! `generators`: pure, versioned field generators (CON-06..08, ADR 0006, ADR 0010).
//!
//! M1 holds the CPU reference height generator for one tile and the CPU dumps used to look at data without a
//! renderer. Everything here is integer hashing plus IEEE `+ - * /` and `libm::floor`, so results are
//! bit-identical across platforms and thread counts.

pub mod climate;
pub mod compact;
pub mod dump;
pub mod erosion;
pub mod global;
pub mod height;
pub mod height_fields;
pub mod manifest;
pub mod mesh;
pub mod normals;
pub mod ocean;
pub mod stage;
pub mod spike_realism;
pub mod stamp;
/// Test-only stress terrain; only test builds enable the `stress` feature (dev-dependencies).
#[cfg(feature = "stress")]
pub mod stress;
pub mod terrain;
pub mod water;
pub mod water_summary;
pub mod wet_floor;

pub use height::{cache_key, generate_region, generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
