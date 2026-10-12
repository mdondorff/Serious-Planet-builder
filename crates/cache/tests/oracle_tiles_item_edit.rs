//! STRM-004 (cache verification), tier B: an item edit rebuilds exactly the tiles that read its cell. Moved out of
//! `tiles.rs` unchanged (it builds 4,096 tiles twice, about 8 s) so that `cargo xtask test fast` stays quick; it still runs in
//! `cargo xtask test` and in CI.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_cache::tiles::{build_tiles, region_tiles, verify_store, World};
use planet_core::cube::TangentWarp;
use planet_core::tile::TileId as Tile;
use planet_core::{Face, FaceMapping, Vec3};
use planet_generators::height::{generate_tile_field, HeightField, HeightGenerator, Reference};
use planet_world_def::cells::{cell_of, cells_covering, with_margin};
use planet_world_def::{BodyId, GenContext, MissingNode, ReadRecord, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-tiles-tests-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&p);
        Dir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Heights shifted by 100 m inside every cell that holds an item, reading the items through cell queries with a margin.
struct StampGenerator {
    def: WorldDefinition,
}

struct StampField {
    def: WorldDefinition,
}

impl HeightField for StampField {
    fn height(&self, d: Vec3) -> f64 {
        let cell = cell_of(R, d);
        let lifted = !self.def.items_in_cell(BodyId::PLANET, cell).is_empty();
        Reference(7).height(d) + if lifted { 100.0 } else { 0.0 }
    }
    fn seed(&self) -> u64 {
        7
    }
    fn implementation_id(&self) -> &'static str {
        "stamp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        Reference(7).max_abs_height_m() + 100.0
    }
    fn reads_cells(&self) -> bool {
        true
    }
    fn read_records_for(&self, tile: Tile, _map: &dyn FaceMapping) -> Vec<ReadRecord> {
        let ctx = GenContext::new(&self.def);
        ctx.body_seed(BodyId::PLANET).unwrap();
        ctx.query_tile(BodyId::PLANET, tile, 1, 64).unwrap();
        ctx.read_records()
    }
}

impl HeightGenerator for StampGenerator {
    fn implementation_id(&self) -> &'static str {
        "stamp"
    }
    fn version(&self) -> u32 {
        1
    }
    fn field(&self, ctx: &GenContext, body: BodyId) -> Result<Box<dyn HeightField>, MissingNode> {
        ctx.body_seed(body)?;
        Ok(Box::new(StampField { def: self.def.clone() }))
    }
}

// spec: STRM-004
#[test]
fn an_item_edit_rebuilds_exactly_the_tiles_whose_footprint_and_margin_hold_its_cell() {
    let d1 = WorldDefinition::single_planet(R, 7);
    let item_at = Vec3::new(1.0, 0.2, 0.1).normalized();
    let mut d2 = d1.clone();
    let item_cell = d2.put_item(BodyId::PLANET, 1, item_at, 42).unwrap();
    let w1 = World::from_generator(&d1, &StampGenerator { def: d1.clone() }).unwrap();
    let w2 = World::from_generator(&d2, &StampGenerator { def: d2.clone() }).unwrap();

    // Every tile of face 0 at level 6 (4,096): each covers 16 cells, so with a one-cell margin 36 at most.
    let tiles = region_tiles(Face(0), 6).unwrap();
    let dir = Dir::new("stamp");
    let first = build_tiles(dir.path(), &w1, &tiles, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!((first.written, first.reused), (4096, 0));

    // The tiles that must change, computed from the cell grid alone.
    let expected: usize = tiles.iter().filter(|t| with_margin(&cells_covering(**t, 8, 64).unwrap(), 1).contains(&item_cell)).count();
    assert!((1..=40).contains(&expected), "the item must touch a few tiles, got {expected}");
    let second = build_tiles(dir.path(), &w2, &tiles, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(second.written, expected, "an edit of one item rebuilds exactly the tiles that read its cell");
    assert_eq!(second.reused, 4096 - expected);

    // The store now verifies against the edited definition: the current tiles match, the replaced ones are stale.
    let store = TileStore::open(dir.path(), w2.params()).unwrap();
    let r = verify_store(&store, &w2, 0);
    assert!(r.passed(), "{r:?}");
    assert_eq!((r.matching, r.unexplained_stale), (4096, expected));
    // And the lifted heights differ from the old ones where the item is.
    let changed = tiles.iter().filter(|t| w1.tile_key(**t, 2) != w2.tile_key(**t, 2)).count();
    assert_eq!(changed, expected);
    let _ = generate_tile_field(&StampField { def: d2 }, &TangentWarp, tiles[0], 2);
}
