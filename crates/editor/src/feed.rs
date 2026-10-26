//! From the places an edit changed to the tiles to refresh (VIEW-019): the footprint find over the resident tiles. `ManagedLoop::edit` takes
//! the result. False positives only cost a refresh; a false negative would leave a stale tile, so the test is the footprint test with its
//! margins and a halo around each tile.

use crate::commands::Affected;
use planet_core::cube::FaceMapping;
use planet_core::TileId;
use planet_world_def::footprint::{tile_cap, Cap, EditReach};
use planet_world_def::BodyId;
use std::collections::HashSet;

/// How much wider than a tile's own cap the cap is taken to hold every cap of the tiles under it (their regions lie inside the tile's
/// extended region, their caps are no wider than it and their centres lie inside it): a tile whose widened cap an edit cannot touch has no
/// tile under it that the edit can touch.
const DESCENDANT_CAP_FACTOR: f64 = 3.0;

/// The halo, in tile edges, around a tile in which an edit still changes the tile's data (a stage reads its neighbourhood).
pub const REFRESH_HALO: f64 = 0.25;

/// Whether the place can change something in `cap`: the reach disc touches it and, for a brush, a stroke does (WDEF-007).
fn reaches(a: &Affected, cap: Cap, radius_m: f64) -> bool {
    EditReach { dir: a.dir, reach_m: a.reach_m, strokes: a.brush.as_ref().map(|b| b.strokes()) }.touches_cap(cap, radius_m)
}

/// The `resident` tiles of `body` whose area plus [`REFRESH_HALO`] a reach in `affected` can touch, without repeats, in tile order.
pub fn edited_tiles(affected: &[Affected], body: BodyId, resident: &[TileId], radius_m: f64, map: &dyn FaceMapping) -> Vec<TileId> {
    edited_with_halo(affected, body, resident, REFRESH_HALO, radius_m, map)
}

/// The `chunks` (tiles of the chunk grid) of `body` whose square plus a halo of `halo_m` metres a reach in `affected` can touch, without
/// repeats, in tile order: the chunks an edit re-keys (the A-net of a chunk reads its halo). A halo beyond one chunk edge is taken as one.
pub fn edited_chunks(
    affected: &[Affected],
    body: BodyId,
    chunks: &[TileId],
    grid_level: u8,
    halo_m: f64,
    radius_m: f64,
    map: &dyn FaceMapping,
) -> Vec<TileId> {
    let edge_m = radius_m * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << grid_level);
    edited_with_halo(affected, body, chunks, (halo_m / edge_m).clamp(0.0, 1.0), radius_m, map)
}

/// The `tiles` of `body` whose area plus `halo` tile edges a reach in `affected` can touch (see [`edited_tiles`]).
fn edited_with_halo(
    affected: &[Affected],
    body: BodyId,
    resident: &[TileId],
    halo: f64,
    radius_m: f64,
    map: &dyn FaceMapping,
) -> Vec<TileId> {
    let mine: Vec<&Affected> = affected.iter().filter(|a| a.body == body).collect();
    if mine.is_empty() {
        return Vec::new();
    }
    // Coarse to fine: a tile no edit can touch, even through the widened cap, takes the tiles under it out of the search without the cost
    // of their caps. The result is the same set as testing every tile (the test below is the one the scan made). The set is only asked
    // whether it holds a tile, never walked, so the order does not matter (CON-14).
    let mut order: Vec<TileId> = resident.to_vec();
    order.sort_by_key(|t| (t.level(), t.raw()));
    let mut skipped: HashSet<TileId> = HashSet::with_capacity(order.len());
    let mut out: Vec<TileId> = Vec::new();
    for tile in order {
        if tile.parent().is_some_and(|p| skipped.contains(&p)) {
            skipped.insert(tile);
            continue;
        }
        let cap = tile_cap(map, tile, halo);
        if mine.iter().any(|a| reaches(a, cap, radius_m)) {
            out.push(tile);
            continue;
        }
        let wide = Cap { centre: cap.centre, angle: (DESCENDANT_CAP_FACTOR * cap.angle).min(core::f64::consts::PI) };
        if !mine.iter().any(|a| reaches(a, wide, radius_m)) {
            skipped.insert(tile);
        }
    }
    out.sort_by_key(|t| t.raw());
    out.dedup();
    out
}
