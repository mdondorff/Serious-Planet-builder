//! `generators`: pure, versioned field generators (CON-06..08, ADR 0006, ADR 0010).
//!
//! It holds the height field chain (base, erosion, terrain, stamps, water), the climate, ocean and global bake, the compact tile form and the CPU dumps used to look at data without a
//! renderer. Everything here is integer hashing plus IEEE `+ - * /` and `libm::floor`, so results are
//! bit-identical across platforms and thread counts.

pub mod arid;
pub mod arid_preview;
pub mod bake;
pub mod chain;
pub mod channels;
pub mod chunk;
pub mod chunkfield;
pub mod classes;
pub mod climate;
pub mod coarsehorizon;
pub mod coast;
pub mod compact;
pub mod continents;
pub mod coupled;
pub mod coupledinput;
pub mod descriptors;
pub mod drivers;
pub mod dump;
pub mod erosion;
pub mod erosion_layer;
pub mod global;
pub mod height;
pub mod height_fields;
pub mod horizon;
pub mod horizon_lookup;
pub mod ice;
pub mod ice_preview;
pub mod lake_water;
pub mod lithology;
pub mod lithology_preview;
pub mod manifest;
pub mod mesh;
pub mod normals;
pub mod ocean;
pub mod orography;
pub mod orography_preview;
pub mod preview;
pub mod relief_channels;
pub mod relief_lattice;
pub mod runoff;
pub mod shape;
pub mod shelf;
pub mod shelf_preview;
pub mod stage;
pub mod stamp;
/// Test-only stress terrain; only test builds enable the `stress` feature (dev-dependencies).
#[cfg(feature = "stress")]
pub mod stress;
pub mod terrain;
pub mod uplift;
pub mod uplift_preview;
pub mod water;
pub mod water_summary;
pub mod wet_floor;

pub use height::{cache_key, generate_region, generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
