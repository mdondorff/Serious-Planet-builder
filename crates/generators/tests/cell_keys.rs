//! WDEF-005: an edit of one authored item invalidates exactly the tiles whose footprint holds its cell.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::{cache_key_field, HeightField, Reference};
use planet_generators::mesh::{mesh_key_field, MeshKeyer};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;

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
    fn inputs_hash_for(&self, tile: TileId) -> u64 {
        let ctx = GenContext::new(self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        // A coarse tile that touches more cells than this reads the whole body in a real generator; the test keeps the
        // limit above the footprints it uses.
        ctx.query_tile(BodyId::PLANET, tile, 64).unwrap().expect("footprint within the limit");
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

// spec: WDEF-005
#[test]
fn an_edit_invalidates_only_the_tiles_whose_footprint_holds_its_cell() {
    let item_at = dir(1.0, 0.2, 0.1);
    let elsewhere = dir(-1.0, 0.3, 0.2);
    // Probe tiles, each named by what it should do when an item appears at `item_at`.
    let inside_deep = tile_at(item_at, 11); // below the cell level, inside the item's cell
    let inside_at_level = tile_at(item_at, 8); // exactly the item's cell
    let inside_coarse = tile_at(item_at, 7); // four cells, one of them the item's
    let far_deep = tile_at(elsewhere, 11);
    let far_coarse = tile_at(elsewhere, 7);
    let tiles = [inside_deep, inside_at_level, inside_coarse, far_deep, far_coarse];
    let affected = [true, true, true, false, false];

    let plain = WorldDefinition::single_planet(R, 7);
    let before = keys(&plain, &tiles);
    let mut edited = plain.clone();
    edited.put_item(BodyId::PLANET, 1, item_at, 42).unwrap();
    let after = keys(&edited, &tiles);
    for (i, t) in tiles.iter().enumerate() {
        assert_eq!(before[i] != after[i], affected[i], "{t:?}: height and mesh keys change exactly when the footprint holds the cell");
        assert_eq!(before[i].0 != after[i].0, before[i].1 != after[i].1, "{t:?}: height and mesh keys agree");
    }
    // A different content hash in the same cell invalidates the same tiles; the same content does not.
    let mut changed = edited.clone();
    changed.put_item(BodyId::PLANET, 1, item_at, 43).unwrap();
    let again = keys(&changed, &tiles);
    for i in 0..tiles.len() {
        assert_eq!(after[i] != again[i], affected[i]);
    }
    // Removing the item restores every key exactly.
    changed.remove_item(BodyId::PLANET, 1);
    assert_eq!(keys(&changed, &tiles), before);
    // Moving the item to the other place swaps the affected set.
    let mut moved = edited.clone();
    moved.put_item(BodyId::PLANET, 1, elsewhere, 42).unwrap();
    let moved_keys = keys(&moved, &tiles);
    for i in 0..tiles.len() {
        assert_eq!(before[i] != moved_keys[i], !affected[i]);
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
    // A field that never queries cells (the reference field) has one inputs hash for every tile.
    let r = Reference(7);
    for t in tiles {
        assert_eq!(r.inputs_hash_for(t), r.inputs_hash());
    }
}
