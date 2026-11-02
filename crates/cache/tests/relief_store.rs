//! Tier A, STRM-067 and STRM-068: the class weights and the relief channels as stored channels. A round trip through a store, a store without the kinds reading as
//! absent, a changed class lattice, version or step limit reading as absent (never as wrong), the class-list version refused naming both versions, and an incomplete or
//! damaged channel an error. Small lattices; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::pack::{PackWriter, Record, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_cache::relief_store::*;
use planet_core::Vec3;
use planet_generators::classes::{ClassLattice, ClassSource, CLASS_LIST_VERSION, CLASS_SOURCE_VERSION};
use planet_generators::relief_channels::{ReliefChannels, DEFAULT_MAX_STEP};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;
const CELLS: u32 = 32;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-relief-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn world() -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4)
}

fn surface(d: Vec3) -> f64 {
    2800.0 * libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y) + 900.0 * libm::sin(7.0 * d.z) - 300.0
}

fn lattice(seed: u64) -> ClassLattice {
    ClassLattice::build_with(&ClassSource::new(seed), 16, 2)
}

fn spec(l: &ClassLattice) -> ReliefSpec {
    ReliefSpec {
        class_list_version: CLASS_LIST_VERSION,
        class_source_version: CLASS_SOURCE_VERSION,
        lattice_hash: lattice_hash(l),
        max_step: DEFAULT_MAX_STEP,
    }
}

fn store_of(dir: &Dir, w: &GlobalWorld, records: &[Record]) -> TileStore {
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in records {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    TileStore::open(dir.path(), w.params()).unwrap()
}

// spec: STRM-067
// spec: STRM-068
#[test]
fn the_class_weights_and_the_relief_channels_round_trip_through_a_store() {
    let (w, l) = (world(), lattice(3));
    let channels = ReliefChannels::compute(&l, &surface, R, CELLS as usize, 2);
    let s = spec(&l);
    let mut records = class_weights_records(&w, RES, &l, CLASS_LIST_VERSION, CLASS_SOURCE_VERSION);
    records.extend(relief_records(&w, RES, &s, &channels));
    assert_eq!(records.len(), 12);
    let d = Dir::new("round-trip");
    let store = store_of(&d, &w, &records);
    let (back, source) = load_class_weights(&store, &w, RES, 16, CLASS_LIST_VERSION).unwrap().expect("the class weights are stored");
    assert_eq!((back.to_bytes() == l.to_bytes(), source), (true, CLASS_SOURCE_VERSION));
    let got = load_relief(&store, &w, RES, &s, CELLS).unwrap().expect("the relief channels are stored");
    assert!(got.to_bytes() == channels.to_bytes(), "the channels read back differ from the ones written");
}

// spec: STRM-067
// spec: STRM-068
#[test]
fn a_store_without_the_kinds_opens_and_reads_them_as_absent() {
    let (w, l) = (world(), lattice(3));
    let d = Dir::new("without");
    let store = store_of(&d, &w, &[]);
    assert!(load_class_weights(&store, &w, RES, 16, CLASS_LIST_VERSION).unwrap().is_none());
    assert!(load_relief(&store, &w, RES, &spec(&l), CELLS).unwrap().is_none());
}

// spec: STRM-068
#[test]
fn a_changed_class_lattice_version_or_step_limit_reads_as_absent_not_as_wrong() {
    let (w, l) = (world(), lattice(3));
    let channels = ReliefChannels::compute(&l, &surface, R, CELLS as usize, 2);
    let s = spec(&l);
    let d = Dir::new("changed");
    let store = store_of(&d, &w, &relief_records(&w, RES, &s, &channels));
    assert!(load_relief(&store, &w, RES, &s, CELLS).unwrap().is_some(), "the channel is there under its own spec");
    let other = lattice(4);
    assert_ne!(lattice_hash(&other), s.lattice_hash, "another seed gives another lattice");
    for (what, changed) in [
        ("class lattice", ReliefSpec { lattice_hash: lattice_hash(&other), ..s }),
        ("class-list version", ReliefSpec { class_list_version: s.class_list_version + 1, ..s }),
        ("class-source version", ReliefSpec { class_source_version: s.class_source_version + 1, ..s }),
        ("step limit", ReliefSpec { max_step: 1.5, ..s }),
    ] {
        assert!(load_relief(&store, &w, RES, &changed, CELLS).unwrap().is_none(), "a changed {what} must read as absent");
    }
    assert!(load_relief(&store, &w, 128, &s, CELLS).unwrap().is_none(), "another bake (resolution) reads as absent");
}

// spec: STRM-067
#[test]
fn a_store_of_another_class_list_version_is_refused_naming_both_versions() {
    let (w, l) = (world(), lattice(3));
    let d = Dir::new("version");
    let store = store_of(&d, &w, &class_weights_records(&w, RES, &l, CLASS_LIST_VERSION + 7, CLASS_SOURCE_VERSION));
    let e = load_class_weights(&store, &w, RES, 16, CLASS_LIST_VERSION).unwrap_err().to_string();
    assert!(e.contains(&format!("{}", CLASS_LIST_VERSION + 7)) && e.contains(&format!("{CLASS_LIST_VERSION}")), "{e}");
}

// spec: STRM-068
#[test]
fn an_incomplete_or_damaged_channel_is_an_error_naming_the_record() {
    let (w, l) = (world(), lattice(3));
    let channels = ReliefChannels::compute(&l, &surface, R, CELLS as usize, 2);
    let s = spec(&l);
    let mut records = relief_records(&w, RES, &s, &channels);
    // A missing face: the channel is incomplete.
    let missing = records.remove(3);
    let d = Dir::new("incomplete");
    let store = store_of(&d, &w, &records);
    let e = load_relief(&store, &w, RES, &s, CELLS).unwrap_err().to_string();
    assert!(e.contains("face 3") && e.contains("incomplete"), "{e}");
    // A payload of the wrong length.
    let mut bad = missing.clone();
    bad.payload.truncate(bad.payload.len() - 4);
    records.insert(3, bad);
    let d2 = Dir::new("damaged");
    let store = store_of(&d2, &w, &records);
    let e = load_relief(&store, &w, RES, &s, CELLS).unwrap_err().to_string();
    assert!(e.contains("face 3") && e.contains("bytes"), "{e}");
}

// spec: STRM-013
// spec: STRM-068
#[test]
fn the_records_of_both_kinds_are_live_for_their_world_and_stale_after_a_version_change() {
    let (w, l) = (world(), lattice(3));
    let channels = ReliefChannels::compute(&l, &surface, R, CELLS as usize, 2);
    let mut records = class_weights_records(&w, RES, &l, CLASS_LIST_VERSION, CLASS_SOURCE_VERSION);
    records.extend(relief_records(&w, RES, &spec(&l), &channels));
    for r in &records {
        assert!(w.is_live(r), "a record of kind {} is live for its own world", r.kind);
        let mut stale = r.clone();
        stale.generator_version += 1;
        assert!(!w.is_live(&stale), "a record of kind {} of another generator version is stale", r.kind);
    }
}

// spec: STRM-067
#[test]
fn a_damaged_class_weights_channel_is_an_error_naming_the_record_or_the_faces() {
    let (w, l) = (world(), lattice(3));
    let good = class_weights_records(&w, RES, &l, CLASS_LIST_VERSION, CLASS_SOURCE_VERSION);
    // An incomplete face set.
    let mut records = good.clone();
    records.remove(2);
    let d = Dir::new("cw-incomplete");
    let e = load_class_weights(&store_of(&d, &w, &records), &w, RES, 16, CLASS_LIST_VERSION).unwrap_err().to_string();
    assert!(e.contains("face 2") && e.contains("incomplete"), "{e}");
    // Weights that still sum to 255 but are not the ones the hash was made of: two class bytes of a point swapped.
    let mut swapped = good.clone();
    let at = 29 + 8 * 40;
    let (a, b) = (swapped[1].payload[at], swapped[1].payload[at + 1]);
    assert_ne!(a, b, "the point has two different class bytes");
    swapped[1].payload.swap(at, at + 1);
    let d2 = Dir::new("cw-hash");
    let e = load_class_weights(&store_of(&d2, &w, &swapped), &w, RES, 16, CLASS_LIST_VERSION).unwrap_err().to_string();
    assert!(e.contains("lattice hash") && e.contains("faces 0 to 5"), "{e}");
    // Weights that do not sum to 255.
    let mut broken = good.clone();
    broken[4].payload[29 + 8 * 7] = broken[4].payload[29 + 8 * 7].wrapping_add(1);
    let d3 = Dir::new("cw-sum");
    let e = load_class_weights(&store_of(&d3, &w, &broken), &w, RES, 16, CLASS_LIST_VERSION).unwrap_err().to_string();
    assert!(e.contains("sum to 255"), "{e}");
}

// spec: STRM-068
#[test]
fn the_channels_of_a_planet_stay_under_a_gigabyte() {
    // The payloads' sizes from their layouts: relief 5 + 8 (n + 1)^2 bytes a face at 2048 cells, class weights 29 + 8 (n + 1)^2 at 256.
    let relief = 6 * (5 + 8 * 2049u64 * 2049);
    let weights = 6 * (29 + 8 * 257u64 * 257);
    assert!(relief + weights < 1_000_000_000, "{} bytes", relief + weights);
    // And the code writes exactly those lengths (checked at a small size).
    let (w, l) = (world(), lattice(3));
    let channels = ReliefChannels::compute(&l, &surface, R, CELLS as usize, 2);
    assert_eq!(relief_records(&w, RES, &spec(&l), &channels)[0].payload.len(), 5 + 8 * 33 * 33);
    assert_eq!(class_weights_records(&w, RES, &l, CLASS_LIST_VERSION, CLASS_SOURCE_VERSION)[0].payload.len(), 29 + 8 * 17 * 17);
}
