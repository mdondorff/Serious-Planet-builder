//! `generators`: pure, versioned field generators (CON-06..08, ADR 0006, ADR 0010).
//!
//! It holds the height field chain (base, erosion, terrain, stamps, water), the climate, ocean and global bake, the compact tile form and the CPU dumps used to look at data without a
//! renderer. Everything here is integer hashing plus IEEE `+ - * /` and `libm::floor`, so results are
//! bit-identical across platforms and thread counts.

pub mod chain;
pub mod chunk;
pub mod chunkfield;
pub mod classes;
pub mod climate;
pub mod coarsehorizon;
pub mod compact;
pub mod coupled;
pub mod coupledinput;
pub mod descriptors;
pub mod drivers;
pub mod dump;
pub mod erosion;
pub mod global;
pub mod height;
pub mod height_fields;
pub mod horizon;
pub mod horizon_lookup;
pub mod manifest;
pub mod mesh;
pub mod normals;
pub mod ocean;
pub mod preview;
pub mod stage;
pub mod stamp;
/// Test-only stress terrain; only test builds enable the `stress` feature (dev-dependencies).
#[cfg(feature = "stress")]
pub mod stress;
pub mod terrain;
pub mod water;
pub mod water_summary;
pub mod wet_floor;

pub use height::{cache_key, generate_region, generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
