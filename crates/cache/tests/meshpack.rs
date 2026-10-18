//! STRM-021 (pre-filled coarse tiles in the tile pack, ADR 0045 point 3): the payload codec, the keys, the pre-fill and its
//! idempotence, thread-count independence and equality with the on-demand tile.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::meshpack::{
    decode_render_tile, encode_render_tile, missing_tiles, payload_form, prefill, prefill_tile_count, prefill_tile_count_from,
    prefill_tiles, prefill_tiles_from, render_key, MeshCodecError, PrefillError, RenderForm, TileSpec, KIND_MESH, MAX_PREFILL_LEVELS,
    PAYLOAD_VERSION,
};
use planet_cache::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::height::Reference;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const MAX_LEVEL: u8 = 22;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-meshpack-tests-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

/// Every pack and index file of a directory with its bytes, in name order (the lock files are not data).
fn data_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("tpack" | "tidx")))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect();
    v.sort();
    v
}

fn progress(_: u8, _: usize) {}

// spec: STRM-021
#[test]
fn a_render_tile_round_trips_bit_for_bit_in_both_forms_and_a_hostile_payload_is_refused() {
    let field = Reference(7);
    for (form, level) in [(RenderForm::Vertex, 2u8), (RenderForm::Compact, 4)] {
        let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, form);
        let id = TileId::new(Face(3), level, 1, 2).unwrap();
        let tile = spec.generate(id);
        assert_eq!(tile.is_compact(), form == RenderForm::Compact);
        let bytes = encode_render_tile(&tile).unwrap();
        assert_eq!(payload_form(&bytes).unwrap(), (form, CELLS));
        assert_eq!(decode_render_tile(id, &bytes).unwrap(), tile, "{form:?}: every bit of the tile comes back");
        // Hostile: short, other version, other form, bad cells, wrong raw length, damaged block.
        assert_eq!(decode_render_tile(id, &bytes[..5]).unwrap_err(), MeshCodecError::Short);
        let mut b = bytes.clone();
        b[0] = 9;
        assert_eq!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::Version(9));
        let mut b = bytes.clone();
        b[1] = 7;
        assert_eq!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::Form(7));
        let mut b = bytes.clone();
        b[2..6].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::Cells(3));
        let mut b = bytes.clone();
        b[2..6].copy_from_slice(&(1u32 << 30).to_le_bytes());
        assert!(matches!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::Cells(_)), "a hostile cell count allocates nothing");
        let mut b = bytes.clone();
        b[6..10].copy_from_slice(&5u32.to_le_bytes());
        assert!(matches!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::RawLength { claimed: 5, .. }));
        let mut b = bytes.clone();
        let n = b.len();
        b.truncate(n - n / 2);
        assert!(matches!(decode_render_tile(id, &b).unwrap_err(), MeshCodecError::Lz4(_)));
    }
}

// spec: STRM-021
#[test]
fn the_key_names_the_field_the_form_and_the_cells_and_levels_0_to_2_share_both_forms() {
    let (a, b) = (Reference(7), Reference(8));
    let tile = |level| TileId::new(Face(1), level, 0, 1).unwrap();
    let key = |f: &Reference, level, cells, form| render_key(f, &TangentWarp, tile(level), cells, R, form);
    assert_ne!(key(&a, 4, 8, RenderForm::Compact), key(&b, 4, 8, RenderForm::Compact), "another field");
    assert_ne!(key(&a, 4, 16, RenderForm::Compact), key(&a, 4, 32, RenderForm::Compact), "other cells");
    assert_ne!(key(&a, 4, 8, RenderForm::Compact), key(&a, 4, 8, RenderForm::Vertex), "the form is part of the key");
    assert_ne!(key(&a, 4, 8, RenderForm::Compact), key(&a, 3, 8, RenderForm::Compact), "another tile");
    // Levels 0-2 are in the vertex form whichever form the run draws: one record serves both runs.
    for level in 0..3 {
        assert_eq!(RenderForm::of_level(level, RenderForm::Compact), RenderForm::Vertex);
        assert_eq!(RenderForm::of_level(level, RenderForm::Vertex), RenderForm::Vertex);
    }
    assert_eq!(RenderForm::of_level(3, RenderForm::Compact), RenderForm::Compact);
    let (ca, cb) =
        (TileSpec::new(&a, R, CELLS, MAX_LEVEL, RenderForm::Compact), TileSpec::new(&a, R, CELLS, MAX_LEVEL, RenderForm::Vertex));
    assert_eq!(ca.generate(tile(1)), cb.generate(tile(1)), "a level-1 tile is the same tile in both forms");
}

// spec: STRM-021
#[test]
fn the_tiles_of_levels_0_to_n_are_counted_and_ordered() {
    assert_eq!(prefill_tile_count(0), 6);
    assert_eq!(prefill_tile_count(4), 2046, "6 x (1 + 4 + 16 + 64 + 256)");
    for n in 0..=3 {
        let t = prefill_tiles(n);
        assert_eq!(t.len(), prefill_tile_count(n));
        assert!(t.windows(2).all(|w| w[0].level() <= w[1].level()), "coarsest first");
        let mut raw: Vec<u64> = t.iter().map(|x| x.raw()).collect();
        raw.sort_unstable();
        raw.dedup();
        assert_eq!(raw.len(), t.len(), "no tile twice");
    }
}

// spec: STRM-021, LOD-011
#[test]
fn the_pre_fill_range_starts_at_the_render_floor() {
    assert_eq!((prefill_tile_count_from(3, 4), prefill_tile_count_from(0, 4)), (1920, 2046), "6 x (64 + 256) from floor 3, 6 x 341 from 0");
    assert_eq!((prefill_tile_count_from(3, 3), prefill_tile_count_from(3, 2), prefill_tile_count_from(0, 1)), (384, 0, 30));
    assert_eq!(prefill_tile_count_from(0, 3), prefill_tile_count(3));
    for (floor, n) in [(3u8, 4u8), (3, 3), (0, 2), (2, 4)] {
        let t = prefill_tiles_from(floor, n);
        assert_eq!(t.len(), prefill_tile_count_from(floor, n), "floor {floor} to {n}");
        assert!(t.iter().all(|x| x.level() >= floor && x.level() <= n) && t.windows(2).all(|w| w[0].level() <= w[1].level()));
    }
    assert!(prefill_tiles_from(3, 2).is_empty(), "a depth below the floor names no tile");
    assert_eq!(prefill_tiles_from(0, 3), prefill_tiles(3));
}

// spec: STRM-021, LOD-011
#[test]
fn a_floor_3_pre_fill_stores_levels_3_and_4_only_and_a_depth_below_the_floor_writes_nothing() {
    let field = Reference(5);
    let floor3 = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact).with_floor(3);
    let floor0 = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let d = Dir::new("floor3");
    let r = prefill(d.path(), &floor3, 4, 4, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((r.total, r.present, r.written), (1920, 0, 1920));
    let store = TileStore::open(d.path(), planet_cache::meshpack::store_params()).unwrap();
    assert!(missing_tiles(&store, &floor3, 4).is_empty());
    // Levels 0-2 are not stored: a floor-0 view of the same pack misses exactly those 126 tiles, and a floor-3 reader never looks them up.
    let missing: Vec<TileId> = missing_tiles(&store, &floor0, 4);
    assert_eq!(missing.len(), 126);
    assert!(missing.iter().all(|t| t.level() <= 2));
    let low = TileId::new(Face(1), 2, 1, 1).unwrap();
    assert!(!floor3.storable(low) && floor3.lookup(&store, low).is_none() && floor0.storable(low));
    // Every stored tile is the generated one.
    for t in prefill_tiles_from(3, 4).into_iter().step_by(37) {
        assert_eq!(floor3.lookup(&store, t).unwrap(), floor3.generate(t), "{t:?}");
    }
    drop(store);
    // The same with floor 0 and depth 1: 30 tiles of levels 0-1; with floor 3 and depth 1: nothing, and no file appears.
    let (a, b) = (Dir::new("floor0-depth1"), Dir::new("floor3-depth1"));
    let r = prefill(a.path(), &floor0, 1, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((r.total, r.written), (30, 30));
    let none = prefill(b.path(), &floor3, 1, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((none.total, none.present, none.written, none.payload_bytes), (0, 0, 0, 0));
    assert!(!b.path().exists() || fs::read_dir(b.path()).unwrap().next().is_none(), "a depth below the floor creates nothing");
}

// spec: STRM-021
#[test]
fn a_prefill_writes_the_on_demand_tiles_adds_nothing_when_repeated_and_does_not_depend_on_the_thread_count() {
    let field = Reference(5);
    let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let (one, many) = (Dir::new("one"), Dir::new("many"));
    let r1 = prefill(one.path(), &spec, 3, 1, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    let r5 = prefill(many.path(), &spec, 3, 5, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    let total = prefill_tile_count(3);
    assert_eq!((r1.total, r1.present, r1.written), (total, 0, total));
    assert_eq!(r1, r5, "the report does not depend on the threads");
    assert_eq!(data_files(one.path()), data_files(many.path()), "the bytes of the pack do not depend on the thread count (CON-14)");
    // Every stored tile is the on-demand tile, bit for bit.
    let store = TileStore::open(one.path(), planet_cache::meshpack::store_params()).unwrap();
    for t in prefill_tiles(3) {
        let stored = spec.lookup(&store, t).unwrap_or_else(|| panic!("tile {:#x} is stored", t.raw()));
        assert_eq!(stored, spec.generate(t), "level {} tile {:#x}", t.level(), t.raw());
    }
    assert!(missing_tiles(&store, &spec, 3).is_empty());
    drop(store);
    // Repeated: nothing generated, nothing written, no byte (and no file time) changed.
    let before = data_files(one.path());
    let times: Vec<_> = fs::read_dir(one.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap()).collect();
    let again = prefill(one.path(), &spec, 3, 4, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((again.written, again.present, again.payload_bytes), (0, total, 0));
    assert_eq!(data_files(one.path()), before);
    let after: Vec<_> = fs::read_dir(one.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().modified().unwrap()).collect();
    assert_eq!(times, after, "a second run touches no file");
}

// spec: STRM-021
#[test]
fn a_deeper_prefill_adds_only_the_new_levels_and_a_damaged_record_is_written_again() {
    let field = Reference(5);
    let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let d = Dir::new("deeper");
    prefill(d.path(), &spec, 1, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    let deeper = prefill(d.path(), &spec, 2, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((deeper.present, deeper.written), (prefill_tile_count(1), 6 * 16));
    // A record under the right key whose payload does not decode counts as missing, and is replaced by the newer record.
    let t = TileId::new(Face(0), 2, 1, 1).unwrap();
    let key = render_key(&field, &TangentWarp, t, CELLS, R, RenderForm::Vertex);
    let good = TileStore::open(d.path(), planet_cache::meshpack::store_params()).unwrap().get(KIND_MESH, t, key).unwrap().unwrap();
    let mut w = PackWriter::open(d.path(), planet_cache::meshpack::store_params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&Record { payload: vec![1, 2, 3], ..good.clone() }).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), planet_cache::meshpack::store_params()).unwrap();
    assert_eq!(missing_tiles(&store, &spec, 2), vec![t], "the undecodable record is a miss");
    assert!(spec.lookup(&store, t).is_none());
    drop(store);
    let fixed = prefill(d.path(), &spec, 2, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!(fixed.written, 1);
    let store = TileStore::open(d.path(), planet_cache::meshpack::store_params()).unwrap();
    assert_eq!(spec.lookup(&store, t).unwrap(), spec.generate(t));
}

// spec: STRM-021
#[test]
fn a_prefill_never_stores_a_finest_level_tile_or_a_depth_beyond_the_limit_and_another_field_misses() {
    let field = Reference(5);
    let d = Dir::new("limits");
    let spec = TileSpec::new(&field, R, CELLS, 3, RenderForm::Compact);
    let e = prefill(d.path(), &spec, 3, 1, DEFAULT_MAX_PACK_BYTES, &progress).unwrap_err();
    assert!(matches!(&e, PrefillError::Levels(s) if s.contains("finest level 3")), "{e}");
    let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let e = prefill(d.path(), &spec, MAX_PREFILL_LEVELS + 1, 1, DEFAULT_MAX_PACK_BYTES, &progress).unwrap_err();
    assert!(matches!(&e, PrefillError::Levels(s) if s.contains("too deep")), "{e}");
    assert!(!d.path().exists() || data_files(d.path()).is_empty(), "a refused pre-fill writes nothing");
    // A store pre-filled for one field is a miss for another, and for a run whose finest level is the tile's own.
    prefill(d.path(), &spec, 1, 1, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    let store = TileStore::open(d.path(), planet_cache::meshpack::store_params()).unwrap();
    let t = TileId::new(Face(2), 1, 0, 0).unwrap();
    assert!(spec.lookup(&store, t).is_some());
    let other = Reference(6);
    assert!(TileSpec::new(&other, R, CELLS, MAX_LEVEL, RenderForm::Compact).lookup(&store, t).is_none(), "another seed");
    assert!(
        TileSpec::new(&field, R, CELLS, 1, RenderForm::Compact).lookup(&store, t).is_none(),
        "a finest tile is generated by its relief"
    );
    assert!(TileSpec::new(&field, R, 32, MAX_LEVEL, RenderForm::Compact).lookup(&store, t).is_none(), "other cells");
}

// spec: STRM-021, GEN-021, GEN-025
#[test]
fn a_pack_pre_filled_under_mesh_version_7_is_all_misses_and_the_prefill_writes_it_again() {
    use planet_cache::key::{cache_key, mesh_key_inputs, PARAM_MESH_VERSION};
    use planet_cache::meshpack::{store_params, PARAM_RENDER_FORM};
    let field = Reference(5);
    let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let d = Dir::new("stale");
    // The records of an older build: the version-7 key and a version-2 payload.
    let tiles = prefill_tiles(1);
    let mut w = PackWriter::open(d.path(), store_params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for t in &tiles {
        let mut inputs = mesh_key_inputs(&field, &TangentWarp, *t, CELLS, R);
        inputs.output_params.insert(PARAM_MESH_VERSION, 7u32.to_le_bytes().to_vec());
        inputs.output_params.insert(PARAM_RENDER_FORM, vec![RenderForm::of_level(t.level(), RenderForm::Compact) as u8]);
        let mut payload = encode_render_tile(&spec.generate(*t)).unwrap();
        payload[0] = 2;
        let current = spec.record(&spec.generate(*t)).unwrap();
        w.append(&Record { key: cache_key(&inputs), payload, ..current }).unwrap();
    }
    w.finish().unwrap();
    let store = TileStore::open(d.path(), store_params()).unwrap();
    assert_eq!(missing_tiles(&store, &spec, 1).len(), tiles.len(), "every old record is a miss: its key is another key");
    assert!(tiles.iter().all(|t| spec.lookup(&store, *t).is_none()));
    // Even under the current key a version-1 or version-2 payload is refused naming the version, not misread.
    let t = tiles[0];
    for old_version in [1u8, 2] {
        let mut old = spec.record(&spec.generate(t)).unwrap();
        old.payload[0] = old_version;
        assert_eq!(decode_render_tile(t, &old.payload).unwrap_err(), MeshCodecError::Version(old_version));
    }
    drop(store);
    let again = prefill(d.path(), &spec, 1, 2, DEFAULT_MAX_PACK_BYTES, &progress).unwrap();
    assert_eq!((again.present, again.written), (0, tiles.len()), "the pre-fill writes every tile again");
    let store = TileStore::open(d.path(), store_params()).unwrap();
    assert!(tiles.iter().all(|t| spec.lookup(&store, *t).unwrap() == spec.generate(*t)));
}

// spec: STRM-021, GEN-025
#[test]
fn the_compact_payload_is_the_slot_and_a_damaged_slot_is_refused() {
    use planet_generators::compact::{slot_bytes, RenderTile};
    let field = Reference(7);
    let spec = TileSpec::new(&field, R, CELLS, MAX_LEVEL, RenderForm::Compact);
    let id = TileId::new(Face(3), 4, 1, 2).unwrap();
    let tile = spec.generate(id);
    let RenderTile::Compact(c) = &tile else { panic!("level 4 is compact") };
    assert_eq!(PAYLOAD_VERSION, 3);
    // The payload's raw bytes end in the slot, byte for byte (GEN-025).
    let payload = encode_render_tile(&tile).unwrap();
    let raw_len = u32::from_le_bytes(payload[6..10].try_into().unwrap()) as usize;
    let raw = lz4_flex::decompress(&payload[10..], raw_len).unwrap();
    assert_eq!(&raw[raw.len() - slot_bytes(CELLS) as usize..], &c.slot_data()[..]);
    // A slot with a reserved bit set is refused naming the slot, not decoded into a tile.
    let mut bad = raw.clone();
    let flags_at = raw.len() - slot_bytes(CELLS) as usize + 124;
    bad[flags_at] = 7;
    let mut p = payload[..10].to_vec();
    p.extend_from_slice(&lz4_flex::compress(&bad));
    let e = decode_render_tile(id, &p).unwrap_err();
    assert!(e.to_string().contains("slot"), "{e}");
}

// spec: GEN-019, STRM-021
#[test]
fn a_compact_prefill_below_16_cells_is_refused_naming_the_minimum_before_anything_is_written() {
    let field = Reference(5);
    let d = Dir::new("small-cells");
    let spec = TileSpec::new(&field, R, 8, MAX_LEVEL, RenderForm::Compact);
    let e = prefill(d.path(), &spec, 3, 1, DEFAULT_MAX_PACK_BYTES, &progress).unwrap_err();
    assert!(matches!(&e, PrefillError::Levels(s) if s.contains("at least 16 cells") && s.contains("got 8")), "{e}");
    assert!(!d.path().exists() || data_files(d.path()).is_empty(), "nothing was written");
    // Levels 0-2 are the vertex form whatever is asked, so a shallow pre-fill needs no compact tile and is accepted.
    assert!(prefill(d.path(), &spec, 2, 1, DEFAULT_MAX_PACK_BYTES, &progress).is_ok());
}
