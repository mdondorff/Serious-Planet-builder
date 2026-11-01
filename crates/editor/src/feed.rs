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

/// The `resident` tiles of `body` an edit re-keys, without repeats, in tile order: those whose area plus [`REFRESH_HALO`] a reach in `affected`
/// can touch, and the coarse tiles whose region summary (WDEF-008) holds the anchor of an item in `affected`. A coarse tile's key reads the
/// summary nodes of its cells and `margin_rings` rings of cells around them (the whole body when the rings overflow), so an edit anchored
/// there re-keys it whether or not its reach touches the tile. Every coarse tile is taken to read its summary (see [`edited_tiles_reading`]).
pub fn edited_tiles(affected: &[Affected], body: BodyId, resident: &[TileId], radius_m: f64, map: &dyn FaceMapping) -> Vec<TileId> {
    edited_tiles_reading(affected, body, resident, radius_m, map, &|_| true)
}

/// [`edited_tiles`] where `reads_summary(tile)` says whether a coarse tile reads its region summary at all (a tile no item touches keeps
/// the base's key, so only a tile with an item in its footprint is re-keyed through the summary): the exact set, for the editor, which
/// knows its definition.
pub fn edited_tiles_reading(
    affected: &[Affected],
    body: BodyId,
    resident: &[TileId],
    radius_m: f64,
    map: &dyn FaceMapping,
    reads_summary: &dyn Fn(TileId) -> bool,
) -> Vec<TileId> {
    let mut out = edited_with_halo(affected, body, resident, REFRESH_HALO, radius_m, map);
    out.extend(summary_re_keyed(affected, body, resident, radius_m, reads_summary));
    out.sort_by_key(|t| t.raw());
    out.dedup();
    out
}

/// The coarse `resident` tiles that read their region summary and whose cover holds the anchor cell of an item in `affected` (WDEF-008).
/// The cover is the tile's cells and the cells within `margin_rings` steps of them by [`TileId::neighbors8`] at the cell level (the walk
/// of `cells::with_margin`), or the whole body when the rings overflow. One walk from each anchor cell out to [`SUMMARY_MAX_RING`] steps
/// gives the fewest steps to each coarse tile; a tile holds the anchor when that is at most its rings. No cover is built (VIEW-061: the
/// find runs on the frame thread).
fn summary_re_keyed(
    affected: &[Affected],
    body: BodyId,
    resident: &[TileId],
    radius_m: f64,
    reads_summary: &dyn Fn(TileId) -> bool,
) -> Vec<TileId> {
    use planet_world_def::cells;
    use planet_world_def::summary::{is_coarse, margin_rings};
    use std::collections::HashMap;
    let level = cells::cell_level(radius_m);
    // The coarse tiles that read their summary, with their rings (`None`: the whole body).
    let coarse: Vec<(TileId, Option<u32>)> = resident
        .iter()
        .copied()
        .filter(|t| is_coarse(*t, level) && reads_summary(*t))
        .map(|t| (t, margin_rings(t, REFRESH_HALO, radius_m, level)))
        .collect();
    // The anchor cell as the world files the item (the normalised direction, bit for bit).
    let mut anchors: Vec<TileId> =
        affected.iter().filter(|a| a.body == body).map(|a| cells::cell_of(radius_m, a.dir.normalized())).collect();
    anchors.sort_by_key(|c| c.raw());
    anchors.dedup();
    if coarse.is_empty() || anchors.is_empty() {
        return Vec::new();
    }
    let reach = coarse.iter().filter_map(|(_, r)| *r).max().unwrap_or(0);
    let top = coarse.iter().map(|(t, _)| t.level()).max().expect("not empty");
    let bottom = coarse.iter().map(|(t, _)| t.level()).min().expect("not empty");
    // One walk from every anchor cell at once (only the fewest steps to a tile counts), out to the most rings any tile needs; each cell
    // climbs its ancestors once, from the finest coarse level up.
    let mut steps: HashMap<u64, u32> = HashMap::new();
    let mut note = |cell: TileId, d: u32| {
        let mut t = cell;
        while t.level() > top {
            t = t.parent().expect("above level 0");
        }
        loop {
            let e = steps.entry(t.raw()).or_insert(d);
            *e = (*e).min(d);
            if t.level() <= bottom {
                break;
            }
            t = t.parent().expect("above level 0");
        }
    };
    let mut seen: HashSet<u64> = anchors.iter().map(|c| c.raw()).collect();
    for c in &anchors {
        note(*c, 0);
    }
    let mut frontier = anchors;
    for d in 1..=reach {
        let mut next = Vec::new();
        for c in &frontier {
            for n in c.neighbors8() {
                if seen.insert(n.raw()) {
                    note(n, d);
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    coarse
        .into_iter()
        .filter(|(t, rings)| match rings {
            None => true,
            Some(r) => steps.get(&t.raw()).is_some_and(|d| d <= r),
        })
        .map(|(t, _)| t)
        .collect()
}

/// The `chunks` (tiles of the chunk grid) of `body` whose square plus a halo of `halo_m` metres a reach in `affected` can touch, without
/// repeats, in tile order: the chunks an edit re-keys (the horizon map of a chunk reads its halo). A halo beyond one chunk edge is taken as one.
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
