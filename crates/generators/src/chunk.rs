//! Chunks of a regional stage on a fixed grid (ADR 0077, GEN-034, GEN-035): the grid level, the chunk's identity in a key and the
//! apply level from which a chunk's detail reaches a tile.

use planet_core::tile::MAX_LEVEL;
use planet_core::{FaceMapping, TileId, Vec3};

/// The level of the tiles whose areas are the chunks (level 11: about 5 km at the face centre). The grid is part of a chunk field's
/// definition: another level is another field version.
pub const CHUNK_GRID_LEVEL: u8 = 11;

/// Cells per tile edge the apply level is computed for (the plan's `LodParams` cells).
pub const TILE_CELLS: u32 = 32;

/// A chunk of a regional stage: the tile of the chunk grid that it covers, with the halo in metres and the step cap its field states
/// (ADR 0077 point 4). All three enter the stage key (STRM-011), so changing any re-keys every chunk of the field. The grid level is
/// the tile's level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkId {
    tile: TileId,
    halo_m_bits: u64,
    step_cap: u32,
}

impl ChunkId {
    /// The chunk over `tile` with a halo of `halo_m` metres and at most `step_cap` steps. `None` for a halo that is negative or not
    /// finite and for a cap of zero. `-0.0` is read as `0.0`, so one chunk has one key.
    pub fn new(tile: TileId, halo_m: f64, step_cap: u32) -> Option<ChunkId> {
        if !halo_m.is_finite() || halo_m < 0.0 || step_cap == 0 {
            return None;
        }
        Some(ChunkId { tile, halo_m_bits: (halo_m + 0.0).to_bits(), step_cap })
    }

    /// The chunk of the grid level that holds the unit direction `dir`.
    pub fn of_direction(map: &dyn FaceMapping, dir: Vec3, halo_m: f64, step_cap: u32) -> Option<ChunkId> {
        ChunkId::new(TileId::from_direction(map, dir, CHUNK_GRID_LEVEL), halo_m, step_cap)
    }

    pub fn tile(self) -> TileId {
        self.tile
    }

    pub fn halo_m(self) -> f64 {
        f64::from_bits(self.halo_m_bits)
    }

    pub fn step_cap(self) -> u32 {
        self.step_cap
    }

    /// The halo as the bits the keys see.
    pub fn halo_bits(self) -> u64 {
        self.halo_m_bits
    }
}

/// The apply level of a chunk field with raster cells of `cell_m` metres on a body of radius `radius_m`: the coarsest tile level whose
/// vertex spacing (the tile edge at the face centre over `cells` intervals) is at most twice the cell. A tile above it is identity on
/// the base field (GEN-035). 8 m cells give level 15 (spacing 9.6 m), 4 m cells level 16; a cell finer than the finest level supports gives
/// `MAX_LEVEL`. `None` for a cell or radius that is not a positive finite number and for zero cells.
pub fn apply_level(cell_m: f64, radius_m: f64, cells: u32) -> Option<u8> {
    if !(cell_m.is_finite() && cell_m > 0.0 && radius_m.is_finite() && radius_m > 0.0 && cells > 0) {
        return None;
    }
    let face_arc_m = radius_m * std::f64::consts::FRAC_PI_2;
    for level in 0..=MAX_LEVEL {
        let spacing = face_arc_m / f64::from(1u32 << level) / f64::from(cells);
        if spacing <= 2.0 * cell_m {
            return Some(level);
        }
    }
    Some(MAX_LEVEL)
}
