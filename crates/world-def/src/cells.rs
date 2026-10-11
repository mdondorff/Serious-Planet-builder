//! The spatial cell index (WDEF-005, ADR 0006, ADR 0007).
//!
//! Authored items are indexed by **cell**: a `TileId` at one fixed level per body, chosen so a cell is about 40 km wide
//! (level 8 at Earth's radius). A generator's spatial query reads whole cells, and the hash of every cell it read,
//! including an empty one, enters its read set. The cell is the invalidation unit: adding, moving, editing or removing
//! an item changes the keys of exactly the tiles whose footprint contains its cell.
//!
//! The cell grid uses the tangent-warp face mapping whatever mapping the renderer uses, so a cell never moves.

use planet_core::cube::TangentWarp;
use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::{TileId, Vec3};

/// Target width of a cell in metres.
pub const CELL_EDGE_M: f64 = 40_000.0;

/// Deepest cell level (a cell is never smaller than the finest level a body this small could need).
pub const MAX_CELL_LEVEL: u8 = 20;

/// Cell level of a body of the given radius: the level whose tiles are closest to [`CELL_EDGE_M`] wide, so cells are
/// level 8 at Earth's radius. Uses `libm` so the level is the same on every platform (CON-14).
pub fn cell_level(radius_m: f64) -> u8 {
    let tiles_across_a_face_edge = (core::f64::consts::FRAC_PI_2 * radius_m / CELL_EDGE_M).max(1.0);
    let level = libm::round(libm::log2(tiles_across_a_face_edge));
    if level.is_nan() {
        return 0;
    }
    level.clamp(0.0, f64::from(MAX_CELL_LEVEL)) as u8
}

/// The cell of a body of this radius that contains the unit direction `dir`.
pub fn cell_of(radius_m: f64, dir: Vec3) -> TileId {
    TileId::from_direction(&TangentWarp, dir, cell_level(radius_m))
}

/// The cells a tile's footprint touches: the one ancestor cell for a tile at or below the cell level, the `4^k`
/// descendant cells for a coarser tile. `None` when that would be more than `max_cells`: such a tile reads the whole
/// body (or a coarse summary), not its cells. Sorted by raw id.
pub fn cells_covering(tile: TileId, cell_level: u8, max_cells: usize) -> Option<Vec<TileId>> {
    if tile.level() >= cell_level {
        let mut t = tile;
        while t.level() > cell_level {
            t = t.parent().expect("a tile above level 0 has a parent");
        }
        return Some(vec![t]);
    }
    let depth = u32::from(cell_level - tile.level());
    let count = 1usize.checked_shl(2 * depth).filter(|c| *c <= max_cells)?;
    let mut cells = vec![tile];
    for _ in 0..depth {
        cells = cells.iter().flat_map(|c| c.children().expect("below the maximum level")).collect();
    }
    debug_assert_eq!(cells.len(), count);
    cells.sort_by_key(|c| c.raw());
    Some(cells)
}

/// Hash of the items in one cell, `(item id, content hash)` pairs in id order. An empty cell has a fixed hash, so a
/// query that found nothing is still a read whose result can change.
pub fn cell_hash(items: &std::collections::BTreeMap<u64, u64>) -> u64 {
    let mut v = vec![u64::from(HASH_VERSION), items.len() as u64];
    for (id, content) in items {
        v.push(*id);
        v.push(*content);
    }
    hash_u64s(0x0063_656c, &v)
}

/// [`cell_hash`] of a cell with no items.
pub fn empty_cell_hash() -> u64 {
    cell_hash(&std::collections::BTreeMap::new())
}
