//! STRM-013 (ADR 0019 space reclamation): compaction and the size cap of the tile pack.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::key::CacheKey;
use planet_cache::pack::*;
use planet_core::{Face, TileId};

const PARAMS: StoreParams = StoreParams { hash_version: 1, body: 0 };
const HEAD: u64 = 61;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-compact-tests-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

/// A record with generator version `gv` (the liveness tests treat version 3 as current) and a payload of 20 bytes.
fn record(i: u32, gv: u32) -> Record {
    Record {
        key: key(i),
        kind: 1,
        tile: tile(i),
        generator_version: gv,
        hash_version: 1,
        node_set_id: [0; 16],
        payload: vec![(i % 251) as u8; 20],
    }
}

/// Write `records` into `dir`, `per_pack` records to a pack (every test record takes 81 bytes; a pack header 24).
fn write(dir: &Path, records: &[Record], per_pack: usize) {
    let mut w = PackWriter::open(dir, PARAMS, 24 + per_pack as u64 * (HEAD + 20)).unwrap();
    for r in records {
        w.append(r).unwrap();
    }
    w.finish().unwrap();
}

fn current(r: &Record) -> bool {
    r.generator_version == 3
}

fn policy<'a>(live: &'a dyn Fn(&Record) -> bool, cap: Option<u64>, max_pack: u64) -> CompactPolicy<'a> {
    CompactPolicy { live, cap_bytes: cap, rank: &rank_by_pack_age, max_pack_bytes: max_pack }
}

/// The content of the packs of `dir` in order, without their names.
fn packs_content(dir: &Path) -> Vec<Vec<u8>> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tpack"))
        .collect();
    names.sort();
    names.iter().map(|n| fs::read(dir.join(n)).unwrap()).collect()
}

fn keys_in(dir: &Path) -> Vec<CacheKey> {
    let store = TileStore::open(dir, PARAMS).unwrap();
    let mut v: Vec<CacheKey> = store.all_entries().into_iter().map(|(_, k)| k).collect();
    v.sort();
    v
}

// spec: STRM-013
#[test]
fn compaction_drops_dead_and_superseded_records_and_keeps_the_rest_readable() {
    let d = Dir::new("basic");
    // 0..10 current, 10..14 of an old generator version (dead), and 0..3 written again later (superseded).
    let mut records: Vec<Record> = (0..10).map(|i| record(i, 3)).collect();
    records.extend((10..14).map(|i| record(i, 2)));
    records.extend((0..3).map(|i| record(i, 3)));
    write(d.path(), &records, 5);
    let packs_before = TileStore::open(d.path(), PARAMS).unwrap().pack_count();
    assert!(packs_before >= 3);
    let r = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((r.kept, r.dead, r.corrupt, r.evicted), (10, 4, 0, 0), "{r:?}");
    assert!(r.superseded >= 3, "{r:?}");
    assert_eq!(r.packs_out, 1);
    let store = TileStore::open(d.path(), PARAMS).unwrap();
    assert_eq!(store.pack_count(), 1);
    assert_eq!(store.len(), 10);
    for i in 0..10 {
        assert_eq!(store.get(1, tile(i), key(i)).unwrap().unwrap(), record(i, 3), "record {i}");
    }
    for i in 10..14 {
        assert!(store.get(1, tile(i), key(i)).unwrap().is_none(), "dead record {i} is gone");
    }
    assert!(r.bytes_after < r.bytes_before);
}

// spec: STRM-013
#[test]
fn the_output_does_not_depend_on_how_or_in_what_order_the_records_were_written() {
    let live: Vec<Record> = (0..40).map(|i| record(i, 3)).collect();
    let mut reversed = live.clone();
    reversed.reverse();
    let mut shuffled: Vec<Record> = (0..40).map(|i| record((i * 17) % 40, 3)).collect();
    shuffled.extend((100..105).map(|i| record(i, 2)));
    let mut outputs = Vec::new();
    for (name, records, per_pack) in [("a", &live, 40usize), ("b", &reversed, 7), ("c", &shuffled, 11)] {
        let d = Dir::new(name);
        write(d.path(), records, per_pack);
        let r = compact(d.path(), PARAMS, &policy(&current, None, 700)).unwrap();
        assert!(r.packs_out >= 2, "the limit of 700 bytes rolls the output over: {r:?}");
        outputs.push((r.output_hash, packs_content(d.path()), r.kept));
    }
    assert_eq!((outputs[0].0, outputs[0].2), (outputs[1].0, outputs[1].2), "write order and pack boundaries do not matter");
    assert_eq!((outputs[0].0, outputs[0].2), (outputs[2].0, outputs[2].2), "dead records do not matter");
    assert!(outputs[0].1 == outputs[1].1 && outputs[0].1 == outputs[2].1, "the output packs are byte for byte equal");
}

// spec: STRM-013
#[test]
fn compacting_again_changes_nothing() {
    let d = Dir::new("again");
    let records: Vec<Record> = (0..20).map(|i| record(i, 3)).collect();
    write(d.path(), &records, 6);
    let first = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
    let content = packs_content(d.path());
    let second = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((first.kept, first.output_hash), (second.kept, second.output_hash));
    assert_eq!(content, packs_content(d.path()));
    assert_eq!((second.dead, second.superseded, second.corrupt, second.evicted), (0, 0, 0, 0));
}

// spec: STRM-013
#[test]
fn the_cap_evicts_the_oldest_packs_first_and_never_exceeds_the_bytes_given() {
    let d = Dir::new("cap");
    let records: Vec<Record> = (0..30).map(|i| record(i, 3)).collect();
    write(d.path(), &records, 1);
    let per = HEAD + 20;
    // One record per pack, so every rank differs. Room for 17 records: the 13 oldest are evicted, the newest 17 stay.
    let r = compact(d.path(), PARAMS, &policy(&current, Some(17 * per), DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((r.kept, r.evicted), (17, 13), "{r:?}");
    let kept = keys_in(d.path());
    let mut want: Vec<CacheKey> = (13..30).map(key).collect();
    want.sort();
    assert_eq!(kept, want);
    let data: u64 = packs_content(d.path()).iter().map(|p| p.len() as u64 - 24).sum();
    assert!(data <= 17 * per, "{data} bytes of records for a cap of {}", 17 * per);
    // A cap that already fits changes nothing; a cap of zero empties the store.
    let r = compact(d.path(), PARAMS, &policy(&current, Some(17 * per), DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((r.kept, r.evicted), (17, 0));
    let r = compact(d.path(), PARAMS, &policy(&current, Some(0), DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((r.kept, r.evicted, r.packs_out), (0, 17, 0));
    assert_eq!(TileStore::open(d.path(), PARAMS).unwrap().pack_count(), 0);
}

// spec: STRM-013
#[test]
fn packs_of_another_hash_version_go_and_unusable_files_stay() {
    let d = Dir::new("versions");
    let old = StoreParams { hash_version: 0, body: 0 };
    let mut w = PackWriter::open(d.path(), old, DEFAULT_MAX_PACK_BYTES).unwrap();
    w.append(&Record { hash_version: 0, ..record(1, 3) }).unwrap();
    w.finish().unwrap();
    let mut w = PackWriter::open(d.path(), PARAMS, 1).unwrap();
    w.append(&record(2, 3)).unwrap();
    w.finish().unwrap();
    let junk = d.path().join("pack-00000009.tpack");
    fs::write(&junk, b"this is not a pack at all, not even close").unwrap();
    let r = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert_eq!((r.kept, r.dead_packs), (1, 1), "{r:?}");
    assert!(junk.exists(), "a file that is not a usable pack is left alone");
    assert!(r.problems.iter().any(|p| p.contains("pack-00000009") && p.contains("left alone")), "{:?}", r.problems);
    assert!(!d.path().join("pack-00000001.tpack").exists(), "the pack of the other hash version is removed");
    assert_eq!(keys_in(d.path()), vec![key(2)]);
}

// spec: STRM-013
#[test]
fn a_corrupt_newest_copy_drops_the_key_and_other_records_survive() {
    let d = Dir::new("corrupt");
    write(d.path(), &[record(1, 3), record(2, 3), record(3, 3)], 3);
    // The newest pack holds a second copy of key 2 with a flipped payload byte.
    let mut w = PackWriter::open(d.path(), PARAMS, 1).unwrap();
    w.append(&record(2, 3)).unwrap();
    w.finish().unwrap();
    let newest =
        fs::read_dir(d.path()).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "tpack")).max().unwrap();
    let mut bytes = fs::read(&newest).unwrap();
    let n = bytes.len();
    bytes[n - 3] ^= 0xff;
    fs::write(&newest, bytes).unwrap();
    let r = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
    assert!(r.corrupt >= 1, "{r:?}");
    assert_eq!(keys_in(d.path()), vec![key(1), key(3)], "ADR 0024 point 8: a corrupt newest copy hides the older intact one");
}

// spec: STRM-013
#[test]
fn compaction_takes_the_writer_lock() {
    let d = Dir::new("lock");
    let w = PackWriter::open(d.path(), PARAMS, DEFAULT_MAX_PACK_BYTES).unwrap();
    let e = compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap_err();
    assert!(matches!(e, PackError::Locked { .. }), "{e}");
    drop(w);
    compact(d.path(), PARAMS, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap();
}

// spec: STRM-013
#[test]
fn another_body_is_an_error_and_nothing_is_removed() {
    let d = Dir::new("body");
    write(d.path(), &[record(1, 3)], 1);
    let other = StoreParams { hash_version: 1, body: 7 };
    let e = compact(d.path(), other, &policy(&current, None, DEFAULT_MAX_PACK_BYTES)).unwrap_err();
    assert!(matches!(e, PackError::WrongStore { .. }), "{e}");
    assert_eq!(keys_in(d.path()), vec![key(1)]);
}
