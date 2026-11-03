//! Tier A, STRM-065 and GEN-100 (m9-bake PR 4): the erosion layer's pass 1 in a store. A killed run resumed equals an uninterrupted one, 1 and 8 threads store the same bytes,
//! a layer made for other parameters is refused naming both keys, a damaged record is an error. Small faces; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::*;
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn base(d: Vec3) -> f64 {
    let ridge = 2200.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y) + 1400.0 * libm::sin(9.0 * d.z + 0.3);
    if ridge > 0.0 {
        ridge + 50.0
    } else {
        -2000.0 + 0.5 * ridge
    }
}

fn spec() -> LayerSpec {
    LayerSpec { cells: 24, radius_m: R, sea_level: 0.0, seed: 17, carve: CarveParams::default(), inputs_id: 1 }
}

fn faces() -> Vec<Face> {
    (0..3).map(Face).collect()
}

fn stored(dir: &Path, w: &GlobalWorld, s: &LayerSpec) -> Vec<Vec<u32>> {
    let store = TileStore::open(dir, w.params()).expect("the store opens");
    (0..6)
        .filter_map(|f| load_face_change(&store, w, RES, s, Face(f)).expect("a readable face"))
        .map(|c| c.change.iter().map(|v| v.to_bits()).collect())
        .collect()
}

// spec: STRM-065
#[test]
fn a_killed_run_resumed_stores_the_same_faces_as_an_uninterrupted_one() {
    let (w, s) = (world(), spec());
    let (whole, cut) = (Dir::new("whole"), Dir::new("cut"));
    let r = run_pass1(whole.path(), &w, RES, &s, &base, None, &faces(), 2).unwrap();
    assert_eq!((r.computed.len(), r.skipped.len()), (3, 0));
    // The run is killed after its first face (only that face was asked for), then resumed over all three.
    run_pass1(cut.path(), &w, RES, &s, &base, None, &faces()[..1], 2).unwrap();
    let r = run_pass1(cut.path(), &w, RES, &s, &base, None, &faces(), 2).unwrap();
    assert_eq!((r.computed, r.skipped), (vec![Face(1), Face(2)], vec![Face(0)]));
    let (a, b) = (stored(whole.path(), &w, &s), stored(cut.path(), &w, &s));
    assert_eq!(a.len(), 3);
    assert_eq!(a, b, "a resumed layer differs from an uninterrupted one");
    // A run over a complete store computes nothing.
    let r = run_pass1(cut.path(), &w, RES, &s, &base, None, &faces(), 2).unwrap();
    assert_eq!((r.computed.len(), r.skipped.len()), (0, 3));
}

// spec: STRM-065
// spec: GEN-100
#[test]
fn the_layer_is_the_same_bytes_on_1_and_8_threads() {
    let (w, s) = (world(), spec());
    let (one, eight) = (Dir::new("t1"), Dir::new("t8"));
    run_pass1(one.path(), &w, RES, &s, &base, None, &faces(), 1).unwrap();
    run_pass1(eight.path(), &w, RES, &s, &base, None, &faces(), 8).unwrap();
    assert_eq!(stored(one.path(), &w, &s), stored(eight.path(), &w, &s));
}

// spec: STRM-065
#[test]
fn a_layer_made_for_other_parameters_is_refused_naming_both_keys() {
    let (w, s) = (world(), spec());
    let d = Dir::new("refused");
    run_pass1(d.path(), &w, RES, &s, &base, None, &faces()[..1], 2).unwrap();
    let other = [
        LayerSpec { seed: 18, ..s },
        LayerSpec { sea_level: 5.0, ..s },
        LayerSpec { inputs_id: 2, ..s },
        LayerSpec { carve: CarveParams { steps: s.carve.steps + 1, ..s.carve }, ..s },
    ];
    for o in other {
        let e = run_pass1(d.path(), &w, RES, &o, &base, None, &faces()[..1], 2).expect_err("another layer must be refused").to_string();
        assert!(e.contains("another bake or other parameters") && e.contains("key"), "{e}");
    }
    // The bake's resolution is part of the key too.
    let e = run_pass1(d.path(), &w, RES * 2, &s, &base, None, &faces()[..1], 2).expect_err("another bake must be refused").to_string();
    assert!(e.contains("refused"), "{e}");
    // The refusal changed nothing: the stored face still reads under its own key.
    assert_eq!(stored(d.path(), &w, &s).len(), 1);
}

// spec: STRM-065
#[test]
fn a_store_without_the_layer_reads_as_absent() {
    let (w, s) = (world(), spec());
    let d = Dir::new("absent");
    run_pass1(d.path(), &w, RES, &s, &base, None, &[], 1).unwrap();
    assert!(stored(d.path(), &w, &s).is_empty());
}

// spec: STRM-065
#[test]
fn a_record_has_the_pinned_layout_and_a_damaged_one_is_an_error() {
    use planet_cache::pack::{PackWriter, DEFAULT_MAX_PACK_BYTES};
    let (w, s) = (world(), spec());
    let change = planet_generators::bake::face::pass1_face(
        &planet_generators::bake::face::FaceSpec { face: Face(2), cells: s.cells, radius_m: R, sea_level: 0.0, seed: 17 },
        &base,
        None,
        &s.carve,
        1,
    );
    let r = face_record(&w, RES, &s, &change);
    assert_eq!((r.kind, r.kind), (12, KIND_LAYER_FACE));
    assert_eq!(r.tile, planet_cache::relief_store::face_tile(Face(2)));
    assert_eq!((r.payload[0], u32::from_le_bytes(r.payload[1..5].try_into().unwrap()), &r.payload[5..21]), (1, 24, &r.key.0[..]));
    assert_eq!(r.payload.len(), 21 + 4 * 24 * 24);
    assert!(w.is_live(&r));
    for (name, damage) in [
        ("a short payload", Box::new(|p: &mut Vec<u8>| p.truncate(30)) as Box<dyn Fn(&mut Vec<u8>)>),
        ("another codec", Box::new(|p: &mut Vec<u8>| p[0] = 9)),
        ("other cells", Box::new(|p: &mut Vec<u8>| p[1] = 25)),
        ("another key inside", Box::new(|p: &mut Vec<u8>| p[7] ^= 1)),
    ] {
        let d = Dir::new("damaged");
        let mut bad = r.clone();
        damage(&mut bad.payload);
        let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
        writer.append(&bad).unwrap();
        writer.write_index().unwrap();
        writer.finish().unwrap();
        let store = TileStore::open(d.path(), w.params()).unwrap();
        let e = load_face_change(&store, &w, RES, &s, Face(2)).expect_err(name).to_string();
        assert!(e.contains("layer face 2") && e.contains("key"), "{name}: {e}");
    }
}
