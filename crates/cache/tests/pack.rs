//! STRM-008 (ADR 0019): the tile pack, its index and its lock.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::hilbert::sort_key;
use planet_cache::key::CacheKey;
use planet_cache::pack::*;
use planet_core::{Face, TileId};

const PARAMS: StoreParams = StoreParams { hash_version: 1, body: 0 };
/// Bytes of a record before its payload.
const HEAD: usize = 61;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-pack-tests-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn tile(i: u32) -> TileId {
    TileId::new(Face((i % 6) as u8), 6, (i * 7) % 64, (i * 13) % 64).unwrap()
}

fn key(i: u32) -> CacheKey {
    let mut k = [0u8; 16];
    k[..4].copy_from_slice(&i.to_le_bytes());
    k[4] = 0xa5;
    CacheKey(k)
}

fn payload(i: u32) -> Vec<u8> {
    vec![(i % 251) as u8; 10 + (i % 37) as usize]
}

fn record(i: u32) -> Record {
    Record {
        key: key(i),
        kind: 1 + (i % 2) as u8,
        tile: tile(i),
        generator_version: 3,
        hash_version: 1,
        node_set_id: [i as u8; 16],
        payload: payload(i),
    }
}

fn write_all(dir: &Path, n: u32, max: u64) {
    let mut w = PackWriter::open(dir, PARAMS, max).unwrap();
    for i in 0..n {
        w.append(&record(i)).unwrap();
    }
    w.finish().unwrap();
}

fn pack_file(dir: &Path, seq: u32) -> PathBuf {
    dir.join(format!("pack-{seq:08}.tpack"))
}

fn index_file(dir: &Path, seq: u32) -> PathBuf {
    dir.join(format!("pack-{seq:08}.tidx"))
}

fn get(store: &TileStore, i: u32) -> Option<Record> {
    store.get(record(i).kind, tile(i), key(i)).unwrap()
}

/// Offset of the record with key `i` in a pack's bytes.
fn offset_of(bytes: &[u8], i: u32) -> usize {
    bytes.windows(16).position(|w| w == key(i).0).unwrap()
}

// spec: STRM-008
#[test]
fn records_round_trip_through_a_pack_and_misses_are_misses() {
    let d = Dir::new("round-trip");
    write_all(d.path(), 200, DEFAULT_MAX_PACK_BYTES);
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 200);
    assert_eq!(store.pack_count(), 1);
    assert!(store.problems().is_empty());
    for i in 0..200 {
        assert_eq!(get(&store, i).unwrap_or_else(|| panic!("record {i} missing")), record(i));
    }
    assert_eq!(store.get(1, tile(0), key(999)).unwrap(), None);
    assert_eq!(store.get(3, tile(0), key(0)).unwrap(), None);
    assert_eq!(store.get(record(0).kind, tile(1), key(0)).unwrap(), None);
    assert!(TileStore::open(&d.path().join("nowhere"), PARAMS).unwrap().is_empty());
}

// spec: STRM-008
#[test]
fn several_definitions_of_one_tile_live_side_by_side() {
    let d = Dir::new("versions");
    let t = tile(5);
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    for (n, v) in [(10u32, 1u32), (11, 2), (12, 3)] {
        let r = Record {
            key: key(n),
            kind: 1,
            tile: t,
            generator_version: v,
            hash_version: 1,
            node_set_id: [0; 16],
            payload: vec![v as u8; 8],
        };
        w.append(&r).unwrap();
    }
    w.finish().unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    let mut keys = store.keys_for(1, t);
    keys.sort();
    assert_eq!(keys, vec![key(10), key(11), key(12)]);
    for (n, v) in [(10u32, 1u8), (11, 2), (12, 3)] {
        assert_eq!(store.get(1, t, key(n)).unwrap().unwrap().payload, vec![v; 8]);
    }
}

// spec: STRM-008
#[test]
fn a_torn_tail_is_ignored_by_readers_and_truncated_by_the_writer() {
    let d = Dir::new("torn");
    write_all(d.path(), 50, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let intact_len = fs::metadata(&data).unwrap().len();
    // A crashed writer: half of a record after the last good one.
    let half = encode_record(&record(77)).unwrap();
    fs::OpenOptions::new().append(true).open(&data).unwrap().write_all(&half[..half.len() / 2]).unwrap();
    let torn_len = fs::metadata(&data).unwrap().len();
    assert!(torn_len > intact_len);

    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 50, "the torn record is not an entry");
    assert_eq!(get(&store, 7).unwrap(), record(7));
    assert_eq!(fs::metadata(&data).unwrap().len(), torn_len, "a reader never modifies the file");
    drop(store);

    // The writer cuts the tail off and carries on; the new record lands right after the last good one.
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(fs::metadata(&data).unwrap().len(), intact_len, "the writer truncated the torn tail");
    w.append(&record(51)).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 51);
    assert_eq!(get(&store, 51).unwrap(), record(51));
    assert_eq!(get(&store, 7).unwrap(), record(7));
}

// spec: STRM-008
#[test]
fn a_zero_filled_tail_is_a_torn_tail_and_never_phantom_records() {
    // Some file systems leave a size-extended, zero-filled tail after a crash. Zeros must not decode as records.
    let d = Dir::new("zeros");
    write_all(d.path(), 30, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let intact_len = fs::metadata(&data).unwrap().len();
    fs::OpenOptions::new().append(true).open(&data).unwrap().write_all(&[0u8; 500]).unwrap();
    let _ = fs::remove_file(index_file(d.path(), 1));
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 30);
    assert!(store.problems().is_empty(), "{:?}", store.problems());
    drop(store);
    PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap().finish().unwrap();
    assert_eq!(fs::metadata(&data).unwrap().len(), intact_len, "the writer cut the zeros off");
    assert_eq!(decode_record(&[0u8; 200]), None, "zeros with a zero CRC are not a record");
}

// spec: STRM-008
#[test]
fn a_flipped_payload_byte_is_a_miss_for_that_record_only_in_big_and_small_packs() {
    // Big pack, index current: the entry exists, the read fails its CRC (Corrupt, naming file, offset and key).
    let d = Dir::new("flip");
    write_all(d.path(), 300, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    assert!(bytes.len() > 20_000, "the damage must be well before the CRC-checked tail of the file");
    let at = offset_of(&bytes, 4);
    bytes[at + HEAD + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    let err = store.get(record(4).kind, tile(4), key(4)).unwrap_err();
    match &err {
        PackError::Corrupt { offset, key: k, .. } => assert!(*offset > 0 && *k == key(4), "{err}"),
        other => panic!("expected a CRC failure, got {other}"),
    }
    assert!(err.to_string().contains(&key(4).to_hex()) && err.to_string().contains("offset"), "{err}");
    for i in [3, 5, 299] {
        assert_eq!(get(&store, i).unwrap(), record(i), "record {i}");
    }
    drop(store);

    // Small pack: the tail check sees the change, the index is rebuilt by a scan, the bad record is skipped and every
    // record after it stays readable. The writer does not cut anything.
    let d = Dir::new("flip-small");
    write_all(d.path(), 20, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    let at = offset_of(&bytes, 4);
    bytes[at + HEAD + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 19, "only the damaged record is gone");
    assert_eq!(store.get(record(4).kind, tile(4), key(4)).unwrap(), None);
    for i in (0..20).filter(|i| *i != 4) {
        assert_eq!(get(&store, i).unwrap(), record(i), "record {i} after the damage");
    }
    assert!(store.problems().iter().any(|p| p.contains("fail their payload CRC")), "{:?}", store.problems());
    drop(store);
    let len_before = fs::metadata(&data).unwrap().len();
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(fs::metadata(&data).unwrap().len(), len_before, "a writer never truncates good records after a bad one");
    w.append(&record(100)).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(get(&store, 19).unwrap(), record(19));
    assert_eq!(get(&store, 100).unwrap(), record(100));
}

// spec: STRM-008
#[test]
fn a_long_lived_store_reports_a_bad_payload_once_then_it_is_a_miss_that_regenerates_once() {
    let d = Dir::new("flip-twice");
    write_all(d.path(), 300, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    assert!(bytes.len() > 20_000, "the damage must be well before the CRC-checked tail, so the index stays trusted");
    let at = offset_of(&bytes, 4);
    bytes[at + HEAD + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    let (kind, t, k) = (record(4).kind, tile(4), key(4));
    assert!(matches!(store.get(kind, t, k), Err(PackError::Corrupt { .. })), "the first lookup reports the damage");
    assert_eq!(store.get(kind, t, k).unwrap(), None, "the second is a plain miss");
    assert_eq!(store.get(kind, t, k).unwrap(), None);
    assert_eq!(get(&store, 5).unwrap(), record(5), "its neighbours are untouched");

    // The caller regenerates once: a writer appends the record to the directory, and a store opened afterwards serves it
    // (the new record is later in the pack, so it wins over the damaged one).
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&record(4)).unwrap();
    w.finish().unwrap();
    let fresh = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(get(&fresh, 4).unwrap(), record(4));
}

// spec: STRM-008
#[test]
fn a_dropped_corrupt_newest_copy_stays_a_miss_even_if_an_older_pack_holds_an_intact_one() {
    // ADR 0024 point 8. Pack 1 holds an intact record 4; the newer, big pack 2 holds a copy that fails its CRC.
    let d = Dir::new("flip-older");
    let mut w = PackWriter::open(d.path(), PARAMS, 120).unwrap();
    w.append(&record(4)).unwrap();
    w.append(&record(5)).unwrap();
    assert_eq!(w.seq(), 2, "the second record rolled the pack over");
    w.finish().unwrap();
    write_all(d.path(), 300, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 2);
    let mut bytes = fs::read(&data).unwrap();
    assert!(bytes.len() > 20_000, "the damage must be well before the CRC-checked tail, so the index stays trusted");
    let at = offset_of(&bytes, 4);
    bytes[at + HEAD + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.pack_count(), 2);
    let (kind, t, k) = (record(4).kind, tile(4), key(4));
    assert!(matches!(store.get(kind, t, k), Err(PackError::Corrupt { .. })));
    assert_eq!(store.get(kind, t, k).unwrap(), None, "after the drop the older copy still does not answer");
}

// spec: STRM-008
#[test]
fn a_damaged_record_header_stops_readers_and_leaves_the_pack_to_a_new_one() {
    let d = Dir::new("header-damage");
    write_all(d.path(), 40, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    let at = offset_of(&bytes, 10);
    bytes[at + 20] ^= 0x01; // inside the header: its CRC no longer matches
    fs::write(&data, &bytes).unwrap();
    let _ = fs::remove_file(index_file(d.path(), 1));
    let damaged = fs::read(&data).unwrap();

    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 10, "records before the damaged header are reachable, the rest are not");
    assert!(store.problems().iter().any(|p| p.contains("damaged record header")), "{:?}", store.problems());
    assert_eq!(get(&store, 9).unwrap(), record(9));
    assert_eq!(store.get(record(10).kind, tile(10), key(10)).unwrap(), None);
    drop(store);

    // The writer leaves the damaged pack byte for byte as it is and continues in the next one.
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(w.seq(), 2);
    w.append(&record(200)).unwrap();
    w.finish().unwrap();
    assert_eq!(fs::read(&data).unwrap(), damaged, "the damaged pack was not touched");
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(get(&store, 200).unwrap(), record(200));
    assert_eq!(get(&store, 9).unwrap(), record(9));
}

// spec: STRM-008
#[test]
fn a_missing_or_stale_index_is_rebuilt_and_the_rebuilt_index_equals_the_written_one() {
    let d = Dir::new("index");
    write_all(d.path(), 120, DEFAULT_MAX_PACK_BYTES);
    let written = fs::read(index_file(d.path(), 1)).unwrap();
    assert_eq!(written.len(), 40 + 120 * 40, "a 40-byte header and 40-byte entries");
    assert_eq!(&written[0..8], b"PLNTTIDX");
    fs::remove_file(index_file(d.path(), 1)).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 120);
    assert_eq!(get(&store, 9).unwrap(), record(9));
    drop(store);
    PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap().finish().unwrap();
    assert_eq!(fs::read(index_file(d.path(), 1)).unwrap(), written, "index rebuilt by a scan equals the index written");
    // Stale: an index from before more records were appended is not trusted.
    let old = written.clone();
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&record(500)).unwrap();
    w.finish().unwrap();
    fs::write(index_file(d.path(), 1), &old).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 121, "the stale index is ignored and the data scanned");
    assert!(get(&store, 500).is_some());
    // A damaged index (wrong size, bad magic) is also ignored.
    let mut broken = fs::read(index_file(d.path(), 1)).unwrap();
    broken.truncate(broken.len() - 7);
    fs::write(index_file(d.path(), 1), &broken).unwrap();
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().len(), 121);
    fs::write(index_file(d.path(), 1), b"garbage").unwrap();
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().len(), 121);
}

// spec: STRM-008
#[test]
fn an_index_that_names_records_outside_the_pack_is_not_trusted() {
    let d = Dir::new("hostile-index");
    write_all(d.path(), 60, DEFAULT_MAX_PACK_BYTES);
    let idx_path = index_file(d.path(), 1);
    let good = fs::read(&idx_path).unwrap();
    // The length field of an entry made huge: the header is intact, the entry points far outside the data.
    let mut bad_len = good.clone();
    bad_len[40 + 38] = 0x7f; // high byte of the u32 length of entry 0
    fs::write(&idx_path, &bad_len).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 60, "the index was rebuilt");
    for i in 0..60 {
        assert_eq!(get(&store, i).unwrap(), record(i), "record {i}");
    }
    // An offset past the end of the data.
    let mut bad_off = good.clone();
    bad_off[40 + 27 + 6] = 0x7f;
    fs::write(&idx_path, &bad_off).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 60);
    assert_eq!(get(&store, 3).unwrap(), record(3));
    // A hostile entry count whose size arithmetic overflows must not panic.
    let mut huge = good[..40].to_vec();
    huge[32..40].copy_from_slice(&((usize::MAX / 40) as u64).to_le_bytes());
    fs::write(&idx_path, &huge).unwrap();
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().len(), 60);
    huge[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    fs::write(&idx_path, &huge).unwrap();
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().len(), 60);
}

// spec: STRM-008
#[test]
fn an_index_with_two_entries_for_one_key_is_rebuilt_not_trusted() {
    // Two records of one tile, then the index edited so both entries carry the first record's key: sorted as whole entries,
    // but one (sort key, key) twice. `find` would return the earlier offset; the index must be rebuilt instead.
    let d = Dir::new("duplicate-index");
    let t = tile(5);
    let rec = |n: u32| Record {
        key: key(n),
        kind: 1,
        tile: t,
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: vec![n as u8; 12],
    };
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&rec(10)).unwrap();
    w.append(&rec(11)).unwrap();
    w.finish().unwrap();
    let idx_path = index_file(d.path(), 1);
    let mut idx = fs::read(&idx_path).unwrap();
    assert_eq!(idx.len(), 40 + 2 * 40);
    let first_key: Vec<u8> = idx[40 + 11..40 + 27].to_vec();
    idx[40 + 40 + 11..40 + 40 + 27].copy_from_slice(&first_key);
    fs::write(&idx_path, &idx).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 2, "rebuilt from the data: both records, each under its own key");
    assert_eq!(store.get(1, t, key(10)).unwrap().unwrap().payload, vec![10; 12]);
    assert_eq!(store.get(1, t, key(11)).unwrap().unwrap().payload, vec![11; 12]);
    // The writer rewrites a clean index the same way.
    PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap().finish().unwrap();
    let clean = fs::read(&idx_path).unwrap();
    assert_ne!(clean[40 + 40 + 11..40 + 40 + 27], first_key[..]);
}

// spec: STRM-008
#[test]
fn index_entries_are_in_hilbert_order_within_a_kind_face_and_level() {
    let d = Dir::new("order");
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    for (n, k) in (0..256u32).enumerate() {
        let (x, y) = ((k * 37) % 16, (k * 37) / 16 % 16);
        let t = TileId::new(Face(0), 4, x, y).unwrap();
        w.append(&Record {
            key: key(n as u32),
            kind: 1,
            tile: t,
            generator_version: 1,
            hash_version: 1,
            node_set_id: [0; 16],
            payload: vec![1],
        })
        .unwrap();
    }
    w.finish().unwrap();
    let idx = fs::read(index_file(d.path(), 1)).unwrap();
    let sorts: Vec<&[u8]> = idx[40..].as_chunks::<40>().0.iter().map(|e| &e[0..11]).collect();
    assert!(sorts.windows(2).all(|w| w[0] <= w[1]), "entries sorted by (kind, face, level, Hilbert index)");
    let expected: Vec<[u8; 11]> = {
        let mut v: Vec<[u8; 11]> =
            (0..16u32).flat_map(|x| (0..16u32).map(move |y| sort_key(1, TileId::new(Face(0), 4, x, y).unwrap()))).collect();
        v.sort_unstable();
        v
    };
    assert_eq!(sorts.iter().map(|s| <[u8; 11]>::try_from(*s).unwrap()).collect::<Vec<_>>(), expected);
}

// spec: STRM-008
#[test]
fn the_newest_record_wins_within_a_pack_and_across_packs() {
    let d = Dir::new("newest");
    let t = tile(1);
    let rec =
        |p: u8| Record { key: key(1), kind: 1, tile: t, generator_version: 1, hash_version: 1, node_set_id: [0; 16], payload: vec![p; 16] };
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&rec(1)).unwrap();
    w.append(&rec(2)).unwrap();
    w.finish().unwrap();
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().get(1, t, key(1)).unwrap().unwrap().payload, vec![2; 16]);
    // Across packs (rollover), the newer pack wins; a tiny limit forces one record per pack.
    let d2 = Dir::new("newest-packs");
    let mut w = PackWriter::open(d2.path(), PARAMS, 120).unwrap();
    w.append(&rec(1)).unwrap();
    w.append(&rec(2)).unwrap();
    w.append(&rec(3)).unwrap();
    assert_eq!(w.seq(), 3, "each record rolled the pack over");
    w.finish().unwrap();
    let store = TileStore::open(d2.path(), PARAMS).unwrap();
    assert_eq!(store.pack_count(), 3);
    assert_eq!(store.get(1, t, key(1)).unwrap().unwrap().payload, vec![3; 16]);
}

// spec: STRM-008
#[test]
fn a_pack_rolls_over_at_the_size_limit_and_every_record_stays_reachable() {
    let d = Dir::new("rollover");
    write_all(d.path(), 300, 2000);
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert!(store.pack_count() > 3, "{} packs", store.pack_count());
    assert_eq!(store.len(), 300);
    for i in 0..300 {
        assert_eq!(get(&store, i).unwrap(), record(i), "record {i}");
    }
    for seq in 1..=store.pack_count() as u32 {
        assert!(fs::metadata(pack_file(d.path(), seq)).unwrap().len() <= 2000, "pack {seq} within the limit");
    }
}

// spec: STRM-008
#[test]
fn a_big_payload_is_checked_by_streaming_and_survives_an_index_rebuild() {
    let d = Dir::new("big");
    let big = Record {
        key: key(1),
        kind: 1,
        tile: tile(1),
        generator_version: 1,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: (0..300_000u32).map(|i| (i * 31 % 251) as u8).collect(),
    };
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&big).unwrap();
    w.append(&record(2)).unwrap();
    w.finish().unwrap();
    fs::remove_file(index_file(d.path(), 1)).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.get(1, tile(1), key(1)).unwrap().unwrap(), big);
    assert_eq!(get(&store, 2).unwrap(), record(2));
}

// spec: STRM-008
#[test]
fn readers_see_whole_records_and_no_errors_while_one_writer_appends() {
    let d = Dir::new("concurrent");
    let dir = d.path().to_path_buf();
    let writer_dir = dir.clone();
    let writer = std::thread::spawn(move || {
        let mut w = PackWriter::open(&writer_dir, PARAMS, 6000).unwrap();
        for i in 0..400u32 {
            w.append(&record(i)).unwrap();
            if i % 25 == 0 {
                w.write_index().unwrap();
            }
        }
        w.finish().unwrap();
    });
    let readers: Vec<_> = (0..3)
        .map(|r| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                for _ in 0..60 {
                    // A store being written is always openable: packs in creation or index swaps never fail a reader.
                    let store = TileStore::open(&dir, PARAMS).unwrap_or_else(|e| panic!("reader {r}: {e}"));
                    for i in (0..400).step_by(17) {
                        match store.get(record(i).kind, tile(i), key(i)) {
                            Ok(Some(rec)) => assert_eq!(rec, record(i), "reader {r} saw a damaged record {i}"),
                            Ok(None) => {}
                            Err(e) => panic!("reader {r}: record {i}: {e}"),
                        }
                    }
                    std::thread::yield_now();
                }
            })
        })
        .collect();
    writer.join().unwrap();
    for h in readers {
        h.join().unwrap();
    }
    assert_eq!(TileStore::open(&dir, PARAMS).unwrap().len(), 400);
}

// spec: STRM-008
#[test]
fn a_second_writer_is_refused_naming_the_holder_even_across_rollovers_and_a_stale_lock_file_does_not_block() {
    let d = Dir::new("lock");
    let mut first = PackWriter::open(d.path(), PARAMS, 300).unwrap();
    for i in 0..12 {
        // Every append rolls a pack over; at no point may a second writer get in.
        first.append(&record(i)).unwrap();
        let err = PackWriter::open(d.path(), PARAMS, 300).err().expect("a second writer must be refused");
        match &err {
            PackError::Locked { holder, path } => {
                assert!(holder.contains(&format!("pid {}", std::process::id())) && path.ends_with("writer.lock"), "{err}")
            }
            other => panic!("expected Locked, got {other}"),
        }
        assert!(err.to_string().contains("locked by another writer"));
    }
    assert!(first.seq() > 3);
    assert!(TileStore::open(d.path(), PARAMS).is_ok(), "readers need no lock");
    drop(first);
    assert!(!d.path().join("writer.holder").exists(), "the holder note goes with its writer");
    PackWriter::open(d.path(), PARAMS, 300).expect("the lock is released with its writer");

    // Files left behind by a crashed writer block nothing, whatever they say: the lock is the OS's, not the file's.
    let d2 = Dir::new("stale-lock");
    fs::create_dir_all(d2.path()).unwrap();
    fs::write(d2.path().join("writer.lock"), "left behind").unwrap();
    fs::write(d2.path().join("writer.holder"), "pid 999999 took the lock at unix time 1\n").unwrap();
    let mut w = PackWriter::open(d2.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).expect("a stale lock file must not block the cache");
    w.append(&record(1)).unwrap();
    w.finish().unwrap();
}

// Helper for the next test, run as a child process: take the writer lock, say so, and hold it until killed.
// spec: STRM-008
#[test]
fn lock_holder_child_process_helper() {
    let Ok(dir) = std::env::var("PLANET_PACK_LOCK_CHILD") else { return };
    let _w = PackWriter::open(Path::new(&dir), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    println!("LOCKED pid {}", std::process::id());
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

// spec: STRM-008
#[test]
fn a_writer_process_that_is_killed_releases_its_lock() {
    let d = Dir::new("killed");
    // The helper's name as libtest knows it: this file is a module of the per-tier binary `it` (BUILD-015), so it is `pack::...` there.
    let helper = module_path!().split_once("::").map_or(String::new(), |(_, m)| format!("{m}::")) + "lock_holder_child_process_helper";
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([helper.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env("PLANET_PACK_LOCK_CHILD", d.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if line.contains("LOCKED pid ") {
                let _ = tx.send(line);
                break;
            }
        }
    });
    let line = match rx.recv_timeout(std::time::Duration::from_secs(60)) {
        Ok(line) => line,
        Err(e) => {
            let _ = child.kill();
            panic!("the child process did not take the lock: {e}");
        }
    };
    // libtest prints "test <name> ... " before the test body when output is not captured.
    let child_pid = line.split("LOCKED pid ").nth(1).unwrap().trim().to_string();
    // While the child lives, a writer here is refused and the message names the child.
    let err = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).err();
    // Kill it without any chance to clean up: the OS releases the lock, the files it left behind block nothing.
    child.kill().unwrap();
    child.wait().unwrap();
    let err = err.expect("refused while the child holds the lock");
    assert!(matches!(&err, PackError::Locked { holder, .. } if holder.contains(&format!("pid {child_pid} "))), "{err}");
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).expect("the lock of a killed writer is released");
    w.append(&record(1)).unwrap();
    w.finish().unwrap();
}

// spec: STRM-008
#[test]
fn another_body_is_refused_an_older_hash_version_is_dead_and_unreadable_packs_degrade_to_misses() {
    let d = Dir::new("stores");
    write_all(d.path(), 3, DEFAULT_MAX_PACK_BYTES);
    // Another body: an error naming the file.
    let other_body = StoreParams { hash_version: 1, body: 4 };
    let e = TileStore::open(d.path(), other_body).err().unwrap();
    assert!(matches!(&e, PackError::WrongStore { found, wanted, .. } if *found == PARAMS && *wanted == other_body), "{e}");
    assert!(e.to_string().contains("pack-00000001.tpack") && e.to_string().contains("another body"), "{e}");
    assert!(matches!(PackWriter::open(d.path(), other_body, DEFAULT_MAX_PACK_BYTES).err().unwrap(), PackError::WrongStore { .. }));

    // A hash version bump invalidates the old records; it does not break the store.
    let newer = StoreParams { hash_version: 2, body: 0 };
    let store = TileStore::open(d.path(), newer).unwrap();
    assert!(store.is_empty() && store.problems().is_empty(), "{:?}", store.problems());
    let mut w = PackWriter::open(d.path(), newer, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(w.seq(), 2, "the old pack is left alone and a new one started");
    let mut r = record(1);
    r.hash_version = 2;
    w.append(&r).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), newer).unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.pack_count(), 1);
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().len(), 3, "the old version still sees its own pack");

    // Not a pack at all: skipped by readers with a problem listed.
    fs::write(pack_file(d.path(), 1), b"definitely not a pack, but longer than a header....").unwrap();
    let store = TileStore::open(d.path(), newer).unwrap();
    assert!(store.problems().iter().any(|p| p.contains("bad magic") && p.contains("pack-00000001.tpack")), "{:?}", store.problems());
    assert_eq!(store.len(), 1, "the good pack is still served");
    // A bad newest pack is an error for the writer, which leaves the file alone.
    let junk = b"definitely not a pack, but longer than a header....".to_vec();
    fs::write(pack_file(d.path(), 3), &junk).unwrap();
    let e = PackWriter::open(d.path(), newer, DEFAULT_MAX_PACK_BYTES).err().unwrap();
    assert!(matches!(&e, PackError::BadFile { why, .. } if why.contains("bad magic")), "{e}");
    assert_eq!(fs::read(pack_file(d.path(), 3)).unwrap(), junk);
}

// spec: STRM-008
#[test]
fn a_pack_still_being_created_or_cut_before_its_header_does_not_fail_the_store() {
    let d = Dir::new("short");
    write_all(d.path(), 5, DEFAULT_MAX_PACK_BYTES);
    // An empty next pack (a writer between creating the file and writing the header) and a 10-byte one.
    for (seq, len) in [(2u32, 0usize), (3, 10)] {
        fs::write(pack_file(d.path(), seq), vec![7u8; len]).unwrap();
        let store = TileStore::open(d.path(), PARAMS).unwrap();
        assert_eq!(store.len(), 5, "pack {seq} is skipped, pack 1 is served");
        assert!(store.problems().is_empty());
        fs::remove_file(pack_file(d.path(), seq)).unwrap();
    }
    // A writer re-initialises a pack that has no header yet and carries on.
    fs::write(pack_file(d.path(), 2), [7u8; 10]).unwrap();
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    assert_eq!(w.seq(), 2);
    w.append(&record(50)).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(get(&store, 50).unwrap(), record(50));
    assert_eq!(get(&store, 3).unwrap(), record(3));
}

// spec: STRM-008
#[test]
fn only_canonical_pack_names_count_and_the_sequence_numbers_can_run_out() {
    let d = Dir::new("names");
    write_all(d.path(), 4, DEFAULT_MAX_PACK_BYTES);
    for stray in ["pack-5.tpack", "pack-+5.tpack", "pack-00000005.tpack.bak", "pack-0000000a.tpack", "pack-000000005.tpack", "other.tpack"]
    {
        fs::write(d.path().join(stray), b"junk").unwrap();
    }
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!((store.len(), store.pack_count()), (4, 1));
    assert!(store.problems().is_empty());
    assert_eq!(PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap().seq(), 1);

    // The last sequence number: appending is fine, a rollover has nowhere to go and says so instead of wrapping.
    let d2 = Dir::new("last-seq");
    fs::create_dir_all(d2.path()).unwrap();
    let mut header = vec![0u8; 24];
    header[0..8].copy_from_slice(&PACK_MAGIC);
    header[8..12].copy_from_slice(&PACK_FORMAT_VERSION.to_le_bytes());
    header[12..16].copy_from_slice(&PARAMS.hash_version.to_le_bytes());
    header[16..20].copy_from_slice(&PARAMS.body.to_le_bytes());
    fs::write(pack_file(d2.path(), u32::MAX), header).unwrap();
    let mut w = PackWriter::open(d2.path(), PARAMS, 120).unwrap();
    assert_eq!(w.seq(), u32::MAX);
    w.append(&record(1)).unwrap();
    assert!(matches!(w.append(&record(2)).unwrap_err(), PackError::NoRoom { .. }));
}

// spec: STRM-008
#[test]
fn a_record_encodes_to_the_wire_format_and_decodes_only_if_both_crcs_hold() {
    let r = record(42);
    let bytes = encode_record(&r).unwrap();
    assert_eq!(bytes.len(), HEAD + r.payload.len());
    assert_eq!(&bytes[0..16], &r.key.0);
    assert_eq!(bytes[16], r.kind);
    assert_eq!(u64::from_le_bytes(bytes[17..25].try_into().unwrap()), r.tile.raw());
    assert_eq!(u32::from_le_bytes(bytes[25..29].try_into().unwrap()), 3);
    assert_eq!(u32::from_le_bytes(bytes[49..53].try_into().unwrap()) as usize, r.payload.len());
    assert_eq!(decode_record(&bytes), Some(r.clone()));
    // A flipped payload byte, a flipped header byte (any of them), a short record.
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert_eq!(decode_record(&bad), None);
    for i in 0..HEAD {
        let mut h = bytes.clone();
        h[i] ^= 0x10;
        assert_eq!(decode_record(&h), None, "a flipped bit at header byte {i} must be detected");
    }
    assert_eq!(decode_record(&bytes[..bytes.len() - 1]), None, "a short record");
    assert_eq!(decode_record(&bytes[..10]), None);
}
