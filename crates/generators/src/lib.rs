//! `generators`: pure, versioned field generators (CON-06..08, ADR 0006, ADR 0010).
//!
//! M1 holds the CPU reference height generator for one tile and the CPU dumps used to look at data without a
//! renderer. Everything here is integer hashing plus IEEE `+ - * /` and `libm::floor`, so results are
//! bit-identical across platforms and thread counts.

pub mod dump;
pub mod height;

pub use height::{generate_region, generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
