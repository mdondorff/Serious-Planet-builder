//! STRM-008 (ADR 0019): the tile pack, its index and its lock.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::hilbert::sort_key;
use planet_cache::key::CacheKey;
use planet_cache::pack::*;
use planet_core::{Face, TileId};

const PARAMS: StoreParams = StoreParams { hash_version: 1, body: 0 };

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

// spec: STRM-008
#[test]
fn records_round_trip_through_a_pack_and_misses_are_misses() {
    let d = Dir::new("round-trip");
    write_all(d.path(), 200, DEFAULT_MAX_PACK_BYTES);
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 200);
    assert_eq!(store.pack_count(), 1);
    for i in 0..200 {
        let r = store.get(record(i).kind, tile(i), key(i)).unwrap().unwrap_or_else(|| panic!("record {i} missing"));
        assert_eq!(r, record(i));
    }
    // Another key, another kind, another tile: misses.
    assert_eq!(store.get(1, tile(0), key(999)).unwrap(), None);
    assert_eq!(store.get(3, tile(0), key(0)).unwrap(), None);
    assert_eq!(store.get(record(0).kind, tile(1), key(0)).unwrap(), None);
    // An empty or missing directory is an empty store.
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
    assert!(fs::metadata(&data).unwrap().len() > intact_len);

    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 50, "the torn record is not an entry");
    assert_eq!(store.get(record(7).kind, tile(7), key(7)).unwrap().unwrap(), record(7));
    assert_eq!(fs::metadata(&data).unwrap().len(), intact_len + (half.len() / 2) as u64, "a reader never modifies the file");
    drop(store);

    // The writer cuts the tail off and carries on; the new record lands right after the last good one.
    let mut w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&record(51)).unwrap();
    w.finish().unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 51);
    assert_eq!(store.get(record(51).kind, tile(51), key(51)).unwrap().unwrap(), record(51));
    assert_eq!(store.get(record(7).kind, tile(7), key(7)).unwrap().unwrap(), record(7));
}

// spec: STRM-008
#[test]
fn a_flipped_payload_byte_is_rejected_by_the_crc_and_is_a_miss_to_regenerate() {
    let d = Dir::new("flip");
    write_all(d.path(), 300, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    assert!(bytes.len() > 20_000, "the damage must be well before the CRC-checked tail of the file");
    // Find record 4's payload: it starts after its key; flip a byte in the middle of the payload.
    let needle = key(4).0;
    let at = bytes.windows(16).position(|w| w == needle).unwrap();
    bytes[at + 57 + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();

    let store = TileStore::open(d.path(), PARAMS).unwrap();
    let err = store.get(record(4).kind, tile(4), key(4)).unwrap_err();
    match &err {
        PackError::Corrupt { offset, key: k, .. } => assert!(*offset > 0 && *k == key(4), "{err}"),
        other => panic!("expected a CRC failure, got {other}"),
    }
    assert!(err.to_string().contains(&key(4).to_hex()) && err.to_string().contains("offset"), "{err}");
    // Its neighbours, before and after, are fine.
    assert_eq!(store.get(record(3).kind, tile(3), key(3)).unwrap().unwrap(), record(3));
    assert_eq!(store.get(record(5).kind, tile(5), key(5)).unwrap().unwrap(), record(5));
    assert_eq!(store.get(record(299).kind, tile(299), key(299)).unwrap().unwrap(), record(299));
}

// spec: STRM-008
#[test]
fn damage_that_changes_the_checked_tail_makes_the_index_stale_and_the_scan_stops_at_it() {
    // In a small pack the tail CRC covers everything: the index is not trusted, the scan rebuilds it, and a record that
    // fails its CRC ends the scan like a torn tail: that record and the ones after it are misses to regenerate.
    let d = Dir::new("flip-small");
    write_all(d.path(), 20, DEFAULT_MAX_PACK_BYTES);
    let data = pack_file(d.path(), 1);
    let mut bytes = fs::read(&data).unwrap();
    let at = bytes.windows(16).position(|w| w == key(4).0).unwrap();
    bytes[at + 57 + 3] ^= 0x40;
    fs::write(&data, &bytes).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 4, "records 0..4 survive, 4 and later are cut");
    assert_eq!(store.get(record(3).kind, tile(3), key(3)).unwrap().unwrap(), record(3));
    assert_eq!(store.get(record(4).kind, tile(4), key(4)).unwrap(), None);
    assert_eq!(store.get(record(5).kind, tile(5), key(5)).unwrap(), None);
}

// spec: STRM-008
#[test]
fn a_missing_or_stale_index_is_rebuilt_and_the_rebuilt_index_equals_the_written_one() {
    let d = Dir::new("index");
    write_all(d.path(), 120, DEFAULT_MAX_PACK_BYTES);
    let written = fs::read(index_file(d.path(), 1)).unwrap();
    // Layout: a 40-byte header and 40-byte entries.
    assert_eq!(written.len(), 40 + 120 * 40);
    assert_eq!(&written[0..8], b"PLNTTIDX");
    // Missing: readers rebuild in memory and see the same store.
    fs::remove_file(index_file(d.path(), 1)).unwrap();
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.len(), 120);
    assert_eq!(store.get(record(9).kind, tile(9), key(9)).unwrap().unwrap(), record(9));
    drop(store);
    // The writer rebuilds and writes it again, byte for byte the same.
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
    assert!(store.get(record(500).kind, tile(500), key(500)).unwrap().is_some());
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
fn index_entries_are_in_hilbert_order_within_a_kind_face_and_level() {
    let d = Dir::new("order");
    // Every tile of one face at level 4, written in scrambled order.
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
    // Within a pack, the later offset wins.
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
        assert_eq!(store.get(record(i).kind, tile(i), key(i)).unwrap().unwrap(), record(i), "record {i}");
    }
    for seq in 1..=store.pack_count() as u32 {
        assert!(
            fs::metadata(pack_file(d.path(), seq)).unwrap().len() <= 2000 + 200,
            "pack {seq} within the limit (one record may complete it)"
        );
    }
}

// spec: STRM-008
#[test]
fn readers_see_whole_records_while_one_writer_appends() {
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
                let mut max_seen = 0usize;
                for _ in 0..60 {
                    let store = match TileStore::open(&dir, PARAMS) {
                        Ok(s) => s,
                        Err(PackError::Io(_)) | Err(PackError::BadFile { .. }) => continue, // a pack being created or renamed
                        Err(e) => panic!("reader {r}: {e}"),
                    };
                    max_seen = max_seen.max(store.len());
                    for i in (0..400).step_by(17) {
                        if let Ok(Some(rec)) = store.get(record(i).kind, tile(i), key(i)) {
                            assert_eq!(rec, record(i), "reader {r} saw a damaged record {i}");
                        }
                    }
                    std::thread::yield_now();
                }
                max_seen
            })
        })
        .collect();
    writer.join().unwrap();
    for h in readers {
        h.join().unwrap();
    }
    let store = TileStore::open(&dir, PARAMS).unwrap();
    assert_eq!(store.len(), 400);
}

// spec: STRM-008
#[test]
fn a_second_writer_is_refused_naming_the_holder_and_a_stale_lock_file_does_not_block() {
    let d = Dir::new("lock");
    let first = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    let err = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).err().expect("a second writer must be refused");
    match &err {
        PackError::Locked { holder, path } => {
            assert!(holder.contains(&format!("pid {}", std::process::id())) && path.ends_with(".lock"), "{err}")
        }
        other => panic!("expected Locked, got {other}"),
    }
    assert!(err.to_string().contains("locked by another writer"));
    // Readers need no lock.
    assert!(TileStore::open(d.path(), PARAMS).is_ok());
    drop(first);
    PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).expect("the lock is released with its writer");

    // A crashed writer leaves its lock file behind, but the OS dropped its lock: nothing blocks, whatever the file says.
    let d2 = Dir::new("stale-lock");
    fs::create_dir_all(d2.path()).unwrap();
    fs::write(d2.path().join("pack-00000001.lock"), "pid 999999 started at unix time 1\n").unwrap();
    let mut w = PackWriter::open(d2.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).expect("a stale lock file must not block the cache");
    w.append(&record(1)).unwrap();
    w.finish().unwrap();
}

// spec: STRM-008
#[test]
fn a_pack_of_another_store_or_not_a_pack_is_refused_naming_the_file() {
    let d = Dir::new("wrong");
    write_all(d.path(), 3, DEFAULT_MAX_PACK_BYTES);
    let other_hash = StoreParams { hash_version: 2, body: 0 };
    let other_body = StoreParams { hash_version: 1, body: 4 };
    for p in [other_hash, other_body] {
        let e = TileStore::open(d.path(), p).err().unwrap();
        assert!(matches!(&e, PackError::WrongStore { found, wanted, .. } if *found == PARAMS && *wanted == p), "{e}");
        assert!(e.to_string().contains("pack-00000001.tpack") && e.to_string().contains("another cache directory"), "{e}");
        assert!(matches!(PackWriter::open(d.path(), p, DEFAULT_MAX_PACK_BYTES).err().unwrap(), PackError::WrongStore { .. }));
    }
    fs::write(pack_file(d.path(), 1), b"definitely not a pack, but longer than a header....").unwrap();
    assert!(matches!(TileStore::open(d.path(), PARAMS).err().unwrap(), PackError::BadFile { why, .. } if why.contains("bad magic")));
    fs::write(pack_file(d.path(), 1), b"short").unwrap();
    assert!(matches!(TileStore::open(d.path(), PARAMS).err().unwrap(), PackError::BadFile { .. }));
}

// spec: STRM-008
#[test]
fn a_record_encodes_to_the_wire_format_and_decodes_only_if_its_crc_holds() {
    let r = record(42);
    let bytes = encode_record(&r).unwrap();
    assert_eq!(bytes.len(), 57 + r.payload.len());
    assert_eq!(&bytes[0..16], &r.key.0);
    assert_eq!(bytes[16], r.kind);
    assert_eq!(u64::from_le_bytes(bytes[17..25].try_into().unwrap()), r.tile.raw());
    assert_eq!(u32::from_le_bytes(bytes[25..29].try_into().unwrap()), 3);
    assert_eq!(u32::from_le_bytes(bytes[49..53].try_into().unwrap()) as usize, r.payload.len());
    assert_eq!(decode_record(&bytes), Some(r));
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert_eq!(decode_record(&bad), None);
    assert_eq!(decode_record(&bytes[..bytes.len() - 1]), None, "a short record");
    assert_eq!(decode_record(&bytes[..10]), None);
}
