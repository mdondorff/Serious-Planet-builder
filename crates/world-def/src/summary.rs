//! Region summaries (WDEF-008, ADR 0066 point 3): a Merkle quadtree over the cell grid, per body and layer.
//!
//! A tile that covers many cells would have to record every item that can touch it. It records instead the hashes of the
//! smallest set of quadtree nodes that covers its cells and a margin ring around them. A leaf is a cell at the cell level, an
//! inner node the hash of its four children, and an empty subtree the same function over empty children, so its hash depends on
//! its level. An edit rewrites one node per level.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};

use crate::footprint::{self, FootprintItem};

const CELL_CONTEXT: &str = "planet-builder cell hash v1";
const NODE_CONTEXT: &str = "planet-builder summary node v1";

/// A tile whose level plus this is at most the cell level covers 64 cells or more and records summary nodes.
pub const SUMMARY_LEVEL_GAP: u8 = 3;

/// The most rings of margin cells a summary read looks through; a tile that needs more reads the six face roots.
pub const SUMMARY_MAX_RING: u32 = 64;

fn derive(context: &str, material: &[u8]) -> [u8; 16] {
    let full = blake3::derive_key(context, material);
    full[..16].try_into().expect("16 of 32 bytes")
}

/// The hash of a cell (a leaf): its item rows of one layer in application order, each a 16-byte id and the 128-bit item hash.
/// A cell with no rows hashes the empty input.
pub fn cell_hash128(rows: &[FootprintItem]) -> [u8; 16] {
    let mut sorted: Vec<&FootprintItem> = rows.iter().collect();
    sorted.sort_by_key(|i| (i.priority, i.id));
    let mut m = Vec::with_capacity(sorted.len() * 32);
    for i in sorted {
        m.extend_from_slice(&footprint::id_bytes(i.id));
        m.extend_from_slice(&footprint::item_hash128(i));
    }
    derive(CELL_CONTEXT, &m)
}

/// The hash of an inner node at `level` over its four children in the order of `TileId::children`.
pub fn node_hash128(level: u8, children: &[[u8; 16]; 4]) -> [u8; 16] {
    let mut m = Vec::with_capacity(65);
    m.push(level);
    for c in children {
        m.extend_from_slice(c);
    }
    derive(NODE_CONTEXT, &m)
}

/// The hash of an empty subtree rooted at `level` in a tree whose leaves are at `cell_level`.
pub fn empty_hash128(level: u8, cell_level: u8) -> [u8; 16] {
    // A miss of the summary map asks for this once per node of a cover (up to some 20,000 for a coarse tile), and each is a chain of hashes: the
    // chain of one cell level is made once (SPIKE-29: 19 ms of a 20 ms tile key).
    type Memo = std::sync::Mutex<std::collections::HashMap<(u8, u8), [u8; 16]>>;
    static MEMO: std::sync::OnceLock<Memo> = std::sync::OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    if let Some(h) = memo.lock().expect("empty hash memo").get(&(level, cell_level)) {
        return *h;
    }
    let mut h = cell_hash128(&[]);
    for l in (level..cell_level).rev() {
        h = node_hash128(l, &[h; 4]);
    }
    memo.lock().expect("empty hash memo").insert((level, cell_level), h);
    h
}

/// Whether a tile records summary nodes on a body whose cells are at `cell_level`.
pub fn is_coarse(tile: TileId, cell_level: u8) -> bool {
    u16::from(tile.level()) + u16::from(SUMMARY_LEVEL_GAP) <= u16::from(cell_level)
}

/// Rings of margin cells around a coarse tile's own cells that hold every anchor of an item that can touch the tile and its halo:
/// the cap's arc less the arc from the tile centre to its nearest edge (the part inside the tile), plus the longest reach, over
/// the narrowest cell edge (half the nominal width), plus one. The nearest edge is taken as the distance from the centre to
/// the great circle of each edge (`asin |n . c|`, the edges of a tangent-warp tile being great-circle arcs), which is never
/// more than the distance to the edge itself, so the margin is never too small. `None` when that is over
/// [`SUMMARY_MAX_RING`].
pub fn margin_rings(tile: TileId, halo: f64, radius_m: f64, cell_level: u8) -> Option<u32> {
    let cap = footprint::tile_cap(&TangentWarp, tile, halo);
    let (s0, s1, t0, t1) = tile.bounds();
    let corner = |s: f64, t: f64| planet_core::cube::face_to_direction(&TangentWarp, tile.face(), s, t);
    let corners = [corner(s0, t0), corner(s1, t0), corner(s1, t1), corner(s0, t1)];
    let inside = (0..4)
        .map(|i| {
            let normal = corners[i].cross(corners[(i + 1) % 4]).normalized();
            libm::asin(normal.dot(cap.centre).abs().min(1.0))
        })
        .fold(f64::INFINITY, f64::min);
    let nominal_edge_m = core::f64::consts::FRAC_PI_2 * radius_m / f64::from(1u32 << cell_level);
    let narrowest_edge_m = 0.5 * nominal_edge_m.min(crate::cells::CELL_EDGE_M * 1.5);
    let outside_m = (cap.angle - inside).max(0.0) * radius_m + crate::cells::CELL_EDGE_M;
    let rings = libm::ceil(outside_m / narrowest_edge_m) + 1.0;
    (rings.is_finite() && rings <= f64::from(SUMMARY_MAX_RING)).then_some(rings as u32)
}

/// The cells of `tile` (a tile above the cell level) along its four edges, at the cell level.
fn boundary_cells(tile: TileId, cell_level: u8) -> Vec<TileId> {
    let depth = u32::from(cell_level - tile.level());
    let side = 1u32 << depth;
    let (x0, y0) = (tile.x() * side, tile.y() * side);
    let mut out = Vec::with_capacity(4 * side as usize);
    for i in 0..side {
        for (x, y) in [(x0 + i, y0), (x0 + i, y0 + side - 1), (x0, y0 + i), (x0 + side - 1, y0 + i)] {
            out.push(TileId::new(tile.face(), cell_level, x, y).expect("a cell of the tile"));
        }
    }
    out
}

/// The smallest set of nodes covering the cells of `tile` and its margin ring of `rings` cells, sorted by (level, raw id):
/// siblings are merged into their parent while all four are present. The tile itself is a node. With `rings` `None` it is
/// the six face roots.
pub fn cover(tile: TileId, cell_level: u8, rings: Option<u32>) -> Vec<TileId> {
    let Some(rings) = rings else {
        return (0..6).map(|f| TileId::new(Face(f), 0, 0, 0).expect("a face root")).collect();
    };
    let inside_tile = |c: &TileId| {
        let mut t = *c;
        while t.level() > tile.level() {
            t = t.parent().expect("above level 0");
        }
        t == tile
    };
    let boundary = boundary_cells(tile, cell_level);
    let mut present: BTreeSet<(u8, u64)> =
        crate::cells::with_margin(&boundary, rings).into_iter().filter(|c| !inside_tile(c)).map(|c| (c.level(), c.raw())).collect();
    present.insert((tile.level(), tile.raw()));
    for level in (1..=cell_level).rev() {
        let parents: BTreeSet<u64> = present
            .iter()
            .filter(|(l, _)| *l == level)
            .map(|(_, raw)| TileId::from_raw(*raw).expect("a node").parent().expect("above level 0").raw())
            .collect();
        for p in parents {
            let children = TileId::from_raw(p).expect("a node").children().expect("below the maximum level");
            if children.iter().all(|c| present.contains(&(level, c.raw()))) {
                for c in children {
                    present.remove(&(level, c.raw()));
                }
                present.insert((level - 1, p));
            }
        }
    }
    let mut out: Vec<TileId> = present.into_iter().map(|(_, raw)| TileId::from_raw(raw).expect("a node")).collect();
    out.sort_by_key(|t| (t.level(), t.raw()));
    out
}

/// Most entries the cover memo holds before it starts over.
const COVER_MEMO_MAX: usize = 256;

type CoverMemo = Mutex<HashMap<(u64, u64, u64), Arc<Vec<TileId>>>>;

static COVER_MEMO: OnceLock<CoverMemo> = OnceLock::new();

/// [`cover`] for a tile read with `halo` on a body of `radius_m` (the margin rings follow from them), memoised: the node set of a tile
/// is a pure function of the tile, the halo, the radius and the cell level (which the radius fixes), so a process-wide memo changes
/// no result and no key, and a plan that reads the same coarse tiles every frame builds each cover once (about 20,000 cells for a
/// level-4 tile at the Earth's cell level with 64 rings). The memo starts over past [`COVER_MEMO_MAX`] entries (256: a cover is up to about 20,000 tile ids, so the memo stays within tens of MB). `halo` is canonical
/// (`-0.0` read as `0.0`).
pub fn cover_cached(tile: TileId, halo: f64, radius_m: f64, cell_level: u8) -> Arc<Vec<TileId>> {
    let halo = if halo == 0.0 { 0.0 } else { halo };
    let key = (tile.raw(), halo.to_bits(), radius_m.to_bits());
    let memo = COVER_MEMO.get_or_init(Default::default);
    if let Some(hit) = memo.lock().expect("cover memo").get(&key) {
        return Arc::clone(hit);
    }
    // Built outside the lock: two threads may build one cover, the results are equal and one entry wins.
    let built = Arc::new(cover(tile, cell_level, margin_rings(tile, halo, radius_m, cell_level)));
    let mut map = memo.lock().expect("cover memo");
    if map.len() >= COVER_MEMO_MAX {
        map.clear();
    }
    Arc::clone(map.entry(key).or_insert(built))
}

/// Every cell (at `cell_level`) under `node`.
pub fn cells_under(node: TileId, cell_level: u8) -> Vec<TileId> {
    let mut cells = vec![node];
    for _ in node.level()..cell_level {
        cells = cells.iter().flat_map(|c| c.children().expect("below the maximum level")).collect();
    }
    cells
}
