//! From the places an edit changed to the tiles to refresh (VIEW-019): the footprint find over the resident tiles. `ManagedLoop::edit` takes
//! the result. False positives only cost a refresh; a false negative would leave a stale tile, so the test is the footprint test with its
//! margins and a halo around each tile.

use crate::commands::Affected;
use planet_core::cube::FaceMapping;
use planet_core::TileId;
use planet_world_def::footprint::{tile_cap, touches};
use planet_world_def::BodyId;

/// The halo, in tile edges, around a tile in which an edit still changes the tile's data (a stage reads its neighbourhood).
pub const REFRESH_HALO: f64 = 0.25;

/// The `resident` tiles of `body` whose area plus [`REFRESH_HALO`] a reach in `affected` can touch, without repeats, in tile order.
pub fn edited_tiles(affected: &[Affected], body: BodyId, resident: &[TileId], radius_m: f64, map: &dyn FaceMapping) -> Vec<TileId> {
    let mine: Vec<&Affected> = affected.iter().filter(|a| a.body == body).collect();
    let mut out: Vec<TileId> = resident
        .iter()
        .copied()
        .filter(|tile| {
            let cap = tile_cap(map, *tile, REFRESH_HALO);
            mine.iter().any(|a| touches(cap, a.dir, a.reach_m, radius_m))
        })
        .collect();
    out.sort_by_key(|t| t.raw());
    out.dedup();
    out
}
