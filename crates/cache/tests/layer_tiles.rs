//! Tier A, STRM-065 and GEN-101 (m9-bake PR 5): the erosion layer's pass 2 in a store. A killed run resumed equals an uninterrupted one, 1 and 8 threads store the same bytes, a layer made
//! for other parameters or another pass 1 is refused naming both keys, a damaged record is an error, the stored change is within half a quantisation step of the computed one, and the
//! pass-1 faces are dropped only from a store, never from another folder. A tiny planet: tiles of 24 cells, faces of 24 cells; no planet bake.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::{load_face_change, run_pass1, LayerSpec};
use planet_cache::layer_tiles::*;
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::tile::{pass2_tile, Pass2Params, TileGrid, TileId as PassTile, QUANT_STEP_M};
use planet_world_def::WorldDefinition;

const R: f64 = 100.0 * 96.0 / core::f64::consts::FRAC_PI_2;
const RES: u32 = 64;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-tiles-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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
    let ridge = 300.0 * libm::sin(9.0 * d.x + 1.0) * libm::cos(8.0 * d.y) + 200.0 * libm::sin(17.0 * d.z + 0.3) + 60.0;
    if ridge > 0.0 {
        ridge + 20.0
    } else {
        -300.0 + 0.3 * ridge
    }
}

fn pass1() -> LayerSpec {
    LayerSpec { cells: 24, radius_m: R, sea_level: 0.0, seed: 17, carve: CarveParams::default(), inputs_id: 1, uses_factor: false, halo: 4 }
}

fn spec(w: &GlobalWorld) -> TileLayerSpec {
    TileLayerSpec {
        grid: TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R },
        params: Pass2Params::standard(9, 0.0),
        inputs_id: 1,
        uses_factor: false,
        faces: faces_digest(w, RES, &pass1()),
    }
}

fn tiles() -> Vec<PassTile> {
    vec![PassTile { face: Face(0), ti: 0, tj: 0 }, PassTile { face: Face(0), ti: 1, tj: 0 }, PassTile { face: Face(2), ti: 2, tj: 1 }]
}

/// The six faces of pass 1 in `dir`, read back.
fn faces(dir: &Path, w: &GlobalWorld) -> [FaceChange; 6] {
    run_pass1(dir, w, RES, &pass1(), &base, None, &(0..6).map(Face).collect::<Vec<_>>(), 2).unwrap();
    let store = TileStore::open(dir, w.params()).unwrap();
    let v: Vec<FaceChange> = (0..6).map(|f| load_face_change(&store, w, RES, &pass1(), Face(f)).unwrap().expect("a face")).collect();
    v.try_into().expect("six faces")
}

fn run(
    dir: &Path,
    w: &GlobalWorld,
    s: &TileLayerSpec,
    f: &[FaceChange; 6],
    which: &[PassTile],
    threads: usize,
) -> Result<Pass2Report, planet_cache::layer_store::LayerStoreError> {
    let reader = face_change_reader(f);
    run_pass2(dir, w, RES, s, &Pass2Inputs { base: &base, face_change: &reader, factor: None }, which, threads, &mut |_| {})
}

fn stored(dir: &Path, w: &GlobalWorld, s: &TileLayerSpec, which: &[PassTile]) -> Vec<Vec<u32>> {
    let store = TileStore::open(dir, w.params()).unwrap();
    which
        .iter()
        .filter_map(|&t| load_tile(&store, w, RES, s, t).expect("a readable tile"))
        .map(|r| r.core.iter().map(|v| v.to_bits()).collect())
        .collect()
}

// spec: STRM-065
#[test]
fn a_killed_run_resumed_stores_the_same_tiles_and_the_same_bytes_on_1_and_8_threads() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let (whole, cut, eight) = (Dir::new("whole"), Dir::new("cut"), Dir::new("eight"));
    let r = run(whole.path(), &w, &s, &f, &tiles(), 1).unwrap();
    assert_eq!((r.computed.len(), r.skipped.len()), (3, 0));
    run(cut.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let r = run(cut.path(), &w, &s, &f, &tiles(), 2).unwrap();
    assert_eq!((r.computed, r.skipped), (tiles()[1..].to_vec(), tiles()[..1].to_vec()));
    run(eight.path(), &w, &s, &f, &tiles(), 8).unwrap();
    let (a, b, c) = (stored(whole.path(), &w, &s, &tiles()), stored(cut.path(), &w, &s, &tiles()), stored(eight.path(), &w, &s, &tiles()));
    assert_eq!(a.len(), 3);
    assert_eq!(a, b, "a resumed layer differs from an uninterrupted one");
    assert_eq!(a, c, "8 threads store other bytes than 1");
    assert_eq!(run(cut.path(), &w, &s, &f, &tiles(), 2).unwrap().computed.len(), 0, "a complete store computed something");
    // A tile listed twice is computed once.
    let twice = Dir::new("twice");
    assert_eq!(run(twice.path(), &w, &s, &f, &[tiles()[0], tiles()[0]], 2).unwrap().computed.len(), 1);
}

// spec: STRM-065
#[test]
fn the_stored_change_is_within_half_a_step_of_the_computed_one_and_carries_its_statistics() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("round");
    run(d.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let reader = face_change_reader(&f);
    let direct = pass2_tile(&s.grid, tiles()[0], &base, &reader, None, &s.params, 2);
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let back = load_tile(&store, &w, RES, &s, tiles()[0]).unwrap().expect("the tile");
    assert_eq!(back.stats, direct.stats);
    assert!(direct.core.iter().zip(&back.core).all(|(a, b)| (a - b).abs() <= QUANT_STEP_M / 2.0 + 1e-6));
    assert!(back.core.iter().any(|v| *v != 0.0), "the tile changed nothing");
}

// spec: STRM-065
#[test]
fn a_layer_made_for_other_parameters_or_another_pass_1_is_refused_naming_both_keys() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let d = Dir::new("refused");
    run(d.path(), &w, &s, &f, &tiles()[..1], 2).unwrap();
    let other_pass1 = TileLayerSpec { faces: faces_digest(&w, RES, &LayerSpec { seed: 18, ..pass1() }), ..s };
    let other_seed = TileLayerSpec { params: Pass2Params::standard(10, 0.0), ..s };
    let other_grid = TileLayerSpec { grid: TileGrid { halo: 5, ..s.grid }, ..s };
    for (what, o) in [("another pass 1", other_pass1), ("another seed", other_seed), ("another grid", other_grid)] {
        let e = run(d.path(), &w, &o, &f, &tiles()[..1], 2).expect_err(what).to_string();
        assert!(e.contains("another bake, other parameters or another pass 1") && e.contains("key"), "{what}: {e}");
    }
    assert_eq!(stored(d.path(), &w, &s, &tiles()[..1]).len(), 1, "the refusal changed the store");
}

// spec: STRM-065
#[test]
fn a_damaged_tile_record_is_an_error_naming_the_tile() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let reader = face_change_reader(&f);
    let r = pass2_tile(&s.grid, tiles()[1], &base, &reader, None, &s.params, 2);
    let good = tile_record(&w, RES, &s, &r).unwrap();
    assert_eq!(good.kind, 13);
    assert_eq!(&good.payload[57..73], &good.key.0[..]);
    for (name, damage) in [
        ("a short payload", Box::new(|p: &mut Vec<u8>| p.truncate(40)) as Box<dyn Fn(&mut Vec<u8>)>),
        ("another codec", Box::new(|p: &mut Vec<u8>| p[0] = 9)),
        ("another tile size", Box::new(|p: &mut Vec<u8>| p[1] = 25)),
        ("another key inside", Box::new(|p: &mut Vec<u8>| p[58] ^= 1)),
        ("counts that do not decompress", Box::new(|p: &mut Vec<u8>| p.truncate(80))),
    ] {
        let d = Dir::new("damaged");
        let mut bad = good.clone();
        damage(&mut bad.payload);
        let mut writer = PackWriter::open(d.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
        writer.append(&bad).unwrap();
        writer.write_index().unwrap();
        writer.finish().unwrap();
        let store = TileStore::open(d.path(), w.params()).unwrap();
        let e = load_tile(&store, &w, RES, &s, tiles()[1]).expect_err(name).to_string();
        assert!(e.contains("layer tile 0/1/0") && e.contains("key"), "{name}: {e}");
    }
}

// spec: STRM-065
#[test]
fn the_pass_1_faces_are_dropped_from_a_store_and_never_from_another_folder() {
    let (w, p1) = (world(), Dir::new("p1"));
    faces(p1.path(), &w);
    drop_pass1(p1.path()).unwrap();
    assert!(!p1.path().exists(), "the pass-1 store is still there");
    let other = Dir::new("other");
    fs::create_dir_all(other.path()).unwrap();
    fs::write(other.path().join("notes.txt"), "mine").unwrap();
    let e = drop_pass1(other.path()).expect_err("a folder that is not a store").to_string();
    assert!(e.contains("refused") && other.path().join("notes.txt").exists(), "{e}");
}

// spec: STRM-065
#[test]
fn a_grid_that_cannot_be_filed_and_a_factor_that_disagrees_with_the_spec_are_refused() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    // Three tiles a face edge are not a power of two: no quadtree id.
    let odd = TileLayerSpec { grid: TileGrid { cells: 72, ..s.grid }, ..s };
    let e = run(Dir::new("odd").path(), &w, &odd, &f, &tiles()[..1], 1).expect_err("a grid of 3 tiles").to_string();
    assert!(e.contains("power of two"), "{e}");
    let with_factor = TileLayerSpec { uses_factor: true, ..s };
    let e = run(Dir::new("factor").path(), &w, &with_factor, &f, &tiles()[..1], 1).expect_err("a factor that is absent").to_string();
    assert!(e.contains("uses_factor"), "{e}");
}
