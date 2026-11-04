//! Tier A, GEN-115 and STRM-073: a store baked from the class source. Its keys are another bake's than the legacy world's, a block is the field's bytes whatever the thread count, a reader
//! with the legacy world or another seed's class source is refused naming both keys, and the legacy world's keys are what they were.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use planet_cache::global_store::{build_global, GlobalWorld, KIND_GLOBAL_BLOCK};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::global::{bake_block_sampled, class_sample};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 8;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-class-source-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn legacy(seed: u64) -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, seed)).unwrap()
}

fn class(seed: u64) -> GlobalWorld {
    legacy(seed).with_class_source(Arc::new(BakeInputs::new(seed, 2)))
}

fn pack_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tpack"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

// spec: STRM-073
#[test]
fn the_class_source_is_part_of_every_key_and_the_legacy_keys_are_untouched() {
    let (l, c, c_other) = (legacy(3), class(3), class(4));
    let (level, cells) = l.layout(RES).unwrap();
    let tile = planet_core::TileId::new(planet_core::Face(0), level, 0, 0).unwrap();
    assert_ne!(l.stage_key(RES), c.stage_key(RES));
    assert_ne!(l.block_key(tile, cells), c.block_key(tile, cells));
    assert_ne!(c.block_key(tile, cells), c_other.block_key(tile, cells));
    // Asking twice gives the same key, and a world that never asks keeps the key it had.
    assert_eq!(c.stage_key(RES), class(3).stage_key(RES));
    assert_eq!(l.stage_key(RES), legacy(3).stage_key(RES));
    assert_eq!(
        l.stage_key(RES).to_string(),
        "62c78c8b3d81fc13b27d66fe0b4d18a1",
        "the legacy world's stage key (seed 3, 8 cells) is a pinned value"
    );
    assert!(l.class_source().is_none() && c.class_source().is_some());
}

// spec: GEN-115
#[test]
fn a_class_store_holds_the_class_fields_blocks_on_any_thread_count() {
    let c = class(3);
    let (one, four) = (Dir::new("one"), Dir::new("four"));
    build_global(one.path(), &c, RES, 1, DEFAULT_MAX_PACK_BYTES).unwrap();
    build_global(four.path(), &c, RES, 4, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(pack_bytes(one.path()), pack_bytes(four.path()));
    // A stored block is the class sample at its lattice, which differs from the legacy field's over land.
    let (level, cells) = c.layout(RES).unwrap();
    let tile = planet_core::TileId::new(planet_core::Face(2), level, 0, 0).unwrap();
    let inputs = c.class_source().unwrap().clone();
    let expected = bake_block_sampled(&|d| class_sample(3, d, &inputs), &TangentWarp, tile, cells);
    let legacy_block = planet_generators::global::bake_block(3, &TangentWarp, tile, cells);
    assert_ne!(expected, legacy_block);
    let store = TileStore::open(one.path(), c.params()).unwrap();
    let record = store.get(KIND_GLOBAL_BLOCK, tile, c.block_key(tile, cells)).unwrap().expect("the block under the class world's key");
    // The stored record is the class block's record: the same payload as the freshly baked class block under the same key.
    let fresh = c.block_record(tile, cells, &expected).unwrap();
    assert_eq!((record.key, &record.payload), (fresh.key, &fresh.payload));
    assert_eq!(c.field(tile.sample_direction(&TangentWarp, cells, 1, 1)), expected[(cells as usize + 1) + 1]);
}

// spec: STRM-073
#[test]
#[should_panic(expected = "the class source is of seed 4")]
fn a_class_source_of_another_seed_than_the_worlds_is_refused() {
    let _ = legacy(3).with_class_source(Arc::new(BakeInputs::new(4, 2)));
}

// spec: STRM-073
#[test]
fn a_reader_of_another_field_source_is_refused_naming_both_keys() {
    let (l, c) = (legacy(3), class(3));
    let (class_store, legacy_store, empty) = (Dir::new("class"), Dir::new("legacy"), Dir::new("empty"));
    build_global(class_store.path(), &c, RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    build_global(legacy_store.path(), &l, RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    let on_class = TileStore::open(class_store.path(), c.params()).unwrap();
    let on_legacy = TileStore::open(legacy_store.path(), l.params()).unwrap();
    assert_eq!(c.check_source(&on_class, RES), Ok(true));
    assert_eq!(l.check_source(&on_legacy, RES), Ok(true));
    let e = l.check_source(&on_class, RES).unwrap_err();
    assert!(e.contains("another field source") && e.contains("legacy generator") && e.contains("key"), "{e}");
    let e = c.check_source(&on_legacy, RES).unwrap_err();
    assert!(e.contains("another field source") && e.contains("class source"), "{e}");
    assert!(class(4).check_source(&on_class, RES).is_err(), "another seed's class source is refused too");
    // A store with no block of the tile reads as absent, not as a mismatch.
    fs::create_dir_all(empty.path()).unwrap();
    planet_cache::pack::PackWriter::open(empty.path(), c.params(), DEFAULT_MAX_PACK_BYTES).unwrap().finish().unwrap();
    let on_empty = TileStore::open(empty.path(), c.params()).unwrap();
    assert_eq!(c.check_source(&on_empty, RES), Ok(false));
}

// spec: STRM-073
#[test]
fn an_offline_reader_picks_the_world_by_the_recorded_source_and_refuses_a_store_of_both_unless_told_which() {
    use planet_cache::global_store::FieldSource;
    let def = WorldDefinition::single_planet(R, 3);
    let (class_store, legacy_store, both, empty) = (Dir::new("c"), Dir::new("l"), Dir::new("b"), Dir::new("e"));
    build_global(class_store.path(), &class(3), RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    build_global(legacy_store.path(), &legacy(3), RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    build_global(both.path(), &class(3), RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    build_global(both.path(), &legacy(3), RES, 2, DEFAULT_MAX_PACK_BYTES).unwrap();
    // The store decides: a class store opens as a class world, a legacy store as the legacy one.
    let w = GlobalWorld::for_store(&def, class_store.path(), RES, None, 2).unwrap();
    assert!(w.class_source().is_some() && w.stage_key(RES) == class(3).stage_key(RES));
    let w = GlobalWorld::for_store(&def, legacy_store.path(), RES, None, 2).unwrap();
    assert!(w.class_source().is_none() && w.stage_key(RES) == legacy(3).stage_key(RES));
    // A store of both is refused unless told which, and then it follows the choice.
    let e = GlobalWorld::for_store(&def, both.path(), RES, None, 2).err().expect("refused");
    assert!(e.contains("legacy bake and a class-source bake") && e.contains("--class-source"), "{e}");
    assert!(GlobalWorld::for_store(&def, both.path(), RES, Some(FieldSource::Class), 2).unwrap().class_source().is_some());
    assert!(GlobalWorld::for_store(&def, both.path(), RES, Some(FieldSource::Legacy), 2).unwrap().class_source().is_none());
    // A choice the store does not hold, another seed and a store with no bake are refused, naming the keys or the resolution.
    assert!(GlobalWorld::for_store(&def, class_store.path(), RES, Some(FieldSource::Legacy), 2).is_err());
    assert!(GlobalWorld::for_store(&WorldDefinition::single_planet(R, 4), class_store.path(), RES, None, 2).is_err());
    assert!(GlobalWorld::for_store(&def, class_store.path(), RES * 2, None, 2).is_err());
    fs::create_dir_all(empty.path()).unwrap();
    planet_cache::pack::PackWriter::open(empty.path(), legacy(3).params(), DEFAULT_MAX_PACK_BYTES).unwrap().finish().unwrap();
    assert!(GlobalWorld::for_store(&def, empty.path(), RES, None, 2).is_err());
}
