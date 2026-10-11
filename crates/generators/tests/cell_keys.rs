//! WDEF-005: an edit of one authored item invalidates exactly the tiles that read its cell.

use planet_core::cube::TangentWarp;
use planet_core::tile::Side;
use planet_core::{FaceMapping, TileId, Vec3};
use planet_generators::height::{cache_key_field, HeightField, Reference};
use planet_generators::mesh::{mesh_key_field, MeshKeyer};
use planet_world_def::cells::CELL_MAPPING_ID;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
/// How far past its cell a stamp reaches, in cells; also covers the tile border row and the normal halo.
const MARGIN: u32 = 1;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

/// `Reference(seed)` that also reads the authored items under each tile's footprint, as a stamp generator would: the
/// tile's inputs hash is the base read set plus a fresh context's spatial queries.
struct Stamped<'a> {
    inner: Reference,
    def: &'a WorldDefinition,
}

impl HeightField for Stamped<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.inner.height(d)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "stamped-test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        assert_eq!(map.id(), CELL_MAPPING_ID, "cells are tangent-warp cells: refuse any other mapping");
        let ctx = GenContext::new(self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_tile(BodyId::PLANET, tile, MARGIN, 64).unwrap();
        ctx.read_set_hash()
    }
}

fn tile_at(d: Vec3, level: u8) -> TileId {
    TileId::from_direction(&TangentWarp, d, level)
}

/// Height and mesh keys of the probe tiles for a definition.
fn keys(def: &WorldDefinition, tiles: &[TileId]) -> Vec<(u64, u64)> {
    let f = Stamped { inner: Reference(7), def };
    let keyer = MeshKeyer::new(&f, &TangentWarp, CELLS, R);
    tiles
        .iter()
        .map(|t| {
            let h = cache_key_field(&f, &TangentWarp, *t, CELLS);
            let m = mesh_key_field(&f, &TangentWarp, *t, CELLS, R);
            assert_eq!(keyer.key(*t), m, "the batch keyer follows the per-tile inputs");
            (h, m)
        })
        .collect()
}

fn changed(a: &[(u64, u64)], b: &[(u64, u64)]) -> Vec<bool> {
    a.iter().zip(b).map(|(x, y)| x != y).collect()
}

// spec: WDEF-005
#[test]
fn an_edit_invalidates_exactly_the_tiles_that_read_its_cell() {
    let item_at = dir(1.0, 0.2, 0.1);
    let elsewhere = dir(-1.0, 0.3, 0.2);
    let cell = tile_at(item_at, 8);
    let beside = cell.neighbor(&TangentWarp, Side::East); // inside the margin ring
    let two_away = beside.neighbor(&TangentWarp, Side::East); // outside it
                                                              // Probe tiles, each paired with whether an item at `item_at` must change its keys.
    let probes: [(TileId, bool); 8] = [
        (tile_at(item_at, 11), true),                               // below the cell level, inside the item's cell
        (cell, true),                                               // exactly the item's cell
        (tile_at(item_at, 7), true),                                // four cells, one of them the item's
        (tile_at(beside.center_direction(&TangentWarp), 11), true), // the neighbouring cell: border row, normals and reach
        (two_away, false),                                          // two cells away: outside the ring
        (tile_at(two_away.center_direction(&TangentWarp), 11), false),
        (tile_at(elsewhere, 11), false),
        (tile_at(elsewhere, 7), false),
    ];
    let tiles: Vec<TileId> = probes.iter().map(|p| p.0).collect();
    let affected: Vec<bool> = probes.iter().map(|p| p.1).collect();

    let plain = WorldDefinition::single_planet(R, 7);
    let before = keys(&plain, &tiles);
    let mut edited = plain.clone();
    edited.put_item(BodyId::PLANET, 1, item_at, 42).unwrap();
    let after = keys(&edited, &tiles);
    for (i, t) in tiles.iter().enumerate() {
        assert_eq!(before[i] != after[i], affected[i], "{t:?}: keys change exactly when the tile reads the item's cell");
        assert_eq!(before[i].0 != after[i].0, before[i].1 != after[i].1, "{t:?}: height and mesh keys agree");
    }
    // Other content in the same cell invalidates the same tiles; the same content does not.
    let mut other_content = edited.clone();
    other_content.put_item(BodyId::PLANET, 1, item_at, 43).unwrap();
    assert_eq!(changed(&after, &keys(&other_content, &tiles)), affected);
    // Removing the item restores every key exactly.
    other_content.remove_item(BodyId::PLANET, 1);
    assert_eq!(keys(&other_content, &tiles), before);
    // Moving the item far away swaps the affected set (the far tiles read the new cell).
    let mut moved = edited.clone();
    moved.put_item(BodyId::PLANET, 1, elsewhere, 42).unwrap();
    let moved_changed = changed(&before, &keys(&moved, &tiles));
    assert_eq!(moved_changed, [false, false, false, false, false, false, true, true]);
}

// spec: WDEF-005
#[test]
fn the_ring_of_cells_crosses_face_edges() {
    // An item filed in a cell right at the edge between two cube faces: the tile across the edge reads it too.
    let item_at = dir(1.0, 0.9999995, 0.1);
    let cell = tile_at(item_at, 8);
    let across: Vec<TileId> = Side::ALL.iter().map(|s| cell.neighbor(&TangentWarp, *s)).filter(|n| n.face() != cell.face()).collect();
    assert!(!across.is_empty(), "the item's cell must touch a face edge");
    let plain = WorldDefinition::single_planet(R, 7);
    let mut edited = plain.clone();
    edited.put_item(BodyId::PLANET, 1, item_at, 42).unwrap();
    for n in across {
        let tiles = [tile_at(n.center_direction(&TangentWarp), 11), n];
        let (b, a) = (keys(&plain, &tiles), keys(&edited, &tiles));
        assert_eq!(changed(&b, &a), [true, true], "{n:?} across the face edge");
    }
}

// spec: WDEF-005
#[test]
fn editing_a_node_no_generator_read_changes_no_key_and_a_field_without_spatial_reads_ignores_items() {
    let d = dir(1.0, 0.2, 0.1);
    let tiles = [tile_at(d, 11), tile_at(d, 7)];
    let plain = WorldDefinition::single_planet(R, 7);
    let mut other_node = plain.clone();
    other_node.set("unrelated.setting", planet_world_def::NodeValue::F64(3.0));
    other_node.set_body_radius(BodyId(5), 1_000_000.0);
    assert_eq!(keys(&plain, &tiles), keys(&other_node, &tiles));
    // A field that never queries cells has one inputs hash for every tile, which the batch keyer computes once.
    let r = Reference(7);
    assert!(!r.reads_cells());
    for t in tiles {
        assert_eq!(r.inputs_hash_for(t, &TangentWarp), r.inputs_hash());
    }
    let keyer = MeshKeyer::new(&r, &TangentWarp, CELLS, R);
    for t in tiles {
        assert_eq!(keyer.key(t), mesh_key_field(&r, &TangentWarp, t, CELLS, R));
    }
}
