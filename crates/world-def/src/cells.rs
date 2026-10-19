//! The spatial cell index (WDEF-005, ADR 0006, ADR 0007).
//!
//! Authored items are indexed by **cell**: a `TileId` at one fixed level per body, chosen so a cell is about 40 km wide
//! (level 8 at Earth's radius). A generator's spatial query reads whole cells, and the hash of every cell it read,
//! including an empty one, enters its read set. The cell is the invalidation unit: adding, moving, editing or removing
//! an item changes the keys of exactly the tiles that read its cell.
//!
//! The cell grid uses the tangent-warp face mapping whatever mapping the renderer uses, so a cell never moves; a tile id
//! is only a position on that grid when it was made with the same mapping (see [`CELL_MAPPING_ID`]).

use std::collections::BTreeSet;

use planet_core::cube::TangentWarp;
use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::tile::{Corner, Side};
use planet_core::{FaceMapping, TileId, Vec3};

/// Target width of a cell in metres.
pub const CELL_EDGE_M: f64 = 40_000.0;

/// Deepest cell level (a cell is never smaller than the finest level a body this small could need).
pub const MAX_CELL_LEVEL: u8 = 20;

/// The id of the only face mapping whose tile ids are positions on the cell grid.
pub const CELL_MAPPING_ID: &str = "tangent-v1";

/// Cell level of a body of the given radius: the level whose tiles are closest to [`CELL_EDGE_M`] wide, so cells are
/// level 8 at Earth's radius. Uses `libm` so the level is the same on every platform (CON-14). A NaN, zero or negative
/// radius gives level 0.
pub fn cell_level(radius_m: f64) -> u8 {
    let tiles_across_a_face_edge = (core::f64::consts::FRAC_PI_2 * radius_m / CELL_EDGE_M).max(1.0);
    libm::round(libm::log2(tiles_across_a_face_edge)).clamp(0.0, f64::from(MAX_CELL_LEVEL)) as u8
}

/// The cell of a body of this radius that contains the unit direction `dir`.
pub fn cell_of(radius_m: f64, dir: Vec3) -> TileId {
    TileId::from_direction(&TangentWarp, dir, cell_level(radius_m))
}

/// The cells a tile's footprint touches: the one ancestor cell for a tile at or below the cell level, the `4^k`
/// descendant cells for a coarser tile. `None` when that would be more than `max_cells`. Sorted by raw id. The tile id
/// must have been made with the tangent-warp mapping ([`CELL_MAPPING_ID`]).
pub fn cells_covering(tile: TileId, cell_level: u8, max_cells: usize) -> Option<Vec<TileId>> {
    assert!(cell_level <= MAX_CELL_LEVEL, "cell level {cell_level} is beyond {MAX_CELL_LEVEL}");
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

/// `cells` plus the ring of cells around them, `margin` times: edge and corner neighbours, across cube faces. A tile
/// samples its closed border row and a one-sample halo for normals, and an item reaches beyond the cell it is filed in,
/// so a reader asks for a margin. Sorted by raw id, without duplicates.
pub fn with_margin(cells: &[TileId], margin: u32) -> Vec<TileId> {
    with_margin_interior(cells, margin).unwrap_or_else(|| with_margin_walk(cells, margin))
}

/// [`with_margin`] by the square of grid coordinates around each cell, when every cell is of one level and its square stays on its own
/// face (the common case: a face is hundreds of cells wide): the same set as the walk, without computing a neighbour across an edge
/// 12 times per cell. `None` when a square reaches a face edge, or the cells differ in level.
fn with_margin_interior(cells: &[TileId], margin: u32) -> Option<Vec<TileId>> {
    let first = cells.first()?;
    let level = first.level();
    let n = 1u64.checked_shl(u32::from(level))?;
    let m = u64::from(margin);
    if cells.iter().any(|c| {
        c.level() != level || u64::from(c.x()) < m || u64::from(c.x()) + m >= n || u64::from(c.y()) < m || u64::from(c.y()) + m >= n
    }) {
        return None;
    }
    let side = 2 * margin + 1;
    let mut raws: Vec<u64> = Vec::with_capacity(cells.len() * (side * side) as usize);
    for c in cells {
        for dy in 0..side {
            for dx in 0..side {
                let (x, y) = (c.x() + dx - margin, c.y() + dy - margin);
                raws.push(TileId::new(c.face(), level, x, y).expect("inside the face").raw());
            }
        }
    }
    raws.sort_unstable();
    raws.dedup();
    Some(raws.into_iter().map(|raw| TileId::from_raw(raw).expect("a raw id taken from a tile")).collect())
}

/// [`with_margin`] by walking the neighbours ring by ring, across cube faces: the reference the interior form is tested against.
pub fn with_margin_walk(cells: &[TileId], margin: u32) -> Vec<TileId> {
    let map: &dyn FaceMapping = &TangentWarp;
    let mut all: BTreeSet<u64> = cells.iter().map(|c| c.raw()).collect();
    let mut frontier: Vec<TileId> = cells.to_vec();
    for _ in 0..margin {
        let mut next = Vec::new();
        for cell in &frontier {
            let mut around: Vec<TileId> = Side::ALL.iter().map(|s| cell.neighbor(map, *s)).collect();
            for corner in Corner::ALL {
                around.extend(cell.corner_neighbors(map, corner));
            }
            for n in around {
                if all.insert(n.raw()) {
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    all.into_iter().map(|raw| TileId::from_raw(raw).expect("a raw id taken from a tile")).collect()
}

/// One item as the cell hash sees it: id, content hash and the bits of its unit direction (so moving an item inside its
/// cell changes the hash).
pub type ItemEntry = (crate::item_id::ItemId, crate::item_id::ItemHash, [u64; 3]);

/// Hash of the items in one cell, in id order. An empty cell has a fixed hash, so a query that found nothing is still a
/// read whose result can change.
pub fn cell_hash(items: &[ItemEntry]) -> u64 {
    let mut v = vec![u64::from(HASH_VERSION), items.len() as u64];
    for (id, content, dir) in items {
        v.extend([crate::item_id::lo(*id), crate::item_id::lo(*content), dir[0], dir[1], dir[2]]);
    }
    v.extend(crate::item_id::wide_tail(items.iter().map(|(id, content, _)| (*id, *content))));
    hash_u64s(0x0063_656c, &v)
}

/// [`cell_hash`] of a cell with no items.
pub fn empty_cell_hash() -> u64 {
    cell_hash(&[])
}
