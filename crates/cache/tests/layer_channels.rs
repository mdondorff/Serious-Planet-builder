//! Tier A, GEN-110, GEN-111 and STRM-072 (m9-bake): the erosion layer's two baked channels in a store. The drainage area of a face and the erosion and deposition depth of a tile come back as
//! the computed codes, 1 and 8 threads store the same bytes, a channel of another layer or a damaged stream is refused, the depth net equals the change, a store made before the channels is
//! completed without writing a tile twice, and the reader returns the stored values. A tiny planet: tiles of 24 cells, faces of 24 cells.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_channels::*;
use planet_cache::layer_store::{load_face_change, run_pass1, LayerSpec};
use planet_cache::layer_tiles::*;
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::{area_cells, pass1_face_with_area, FaceChange, FaceSpec};
use planet_generators::bake::tile::{depth_code, depth_m, pass2_tile, Pass2Params, TileGrid, TileId as PassTile};
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

fn pass2_dir(w: &GlobalWorld, s: &TileLayerSpec, f: &[FaceChange; 6], name: &str, threads: usize) -> Dir {
    let d = Dir::new(name);
    run_pass2(
        d.path(),
        w,
        RES,
        s,
        &Pass2Inputs { base: &base, face_change: &face_change_reader(f), factor: None },
        &tiles(),
        threads,
        &mut |_| {},
    )
    .unwrap();
    d
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

fn all_faces() -> Vec<Face> {
    (0..6).map(Face).collect()
}

// spec: GEN-111
#[test]
fn the_depth_codes_are_exact_at_zero_monotone_and_clamped_and_the_net_equals_the_change() {
    assert_eq!(depth_code(0.0), 0);
    assert_eq!(depth_code(-3.0), 0);
    assert_eq!(depth_code(1.0e6), 255);
    assert!((0..255u8).all(|c| depth_m(c) < depth_m(c + 1)));
    for m in [0.3, 1.0, 7.0, 40.0, 200.0] {
        assert!((f64::from(depth_m(depth_code(m))) - m).abs() <= 0.07 * m.sqrt() + 1e-3, "{m} m round trips to {}", depth_m(depth_code(m)));
    }
    let g = TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R };
    for face_change in [0.0, -30.0] {
        let r = pass2_tile(&g, tiles()[0], &base, &|_: Vec3| face_change, None, &Pass2Params::standard(9, 0.0), 2);
        assert_eq!((r.eroded.len(), r.deposited.len()), (24 * 24, 24 * 24));
        assert!(r.eroded.iter().any(|&c| c > 0) && r.deposited.iter().any(|&c| c > 0), "both signs are kept");
        for (p, c) in r.core.iter().enumerate() {
            let (e, d) = (f64::from(depth_m(r.eroded[p])), f64::from(depth_m(r.deposited[p])));
            assert!(
                (d - e - f64::from(*c)).abs() <= 0.14 * e.max(d).sqrt() + 0.3,
                "face change {face_change}, cell {p}: deposited {d} - eroded {e} is not the change {c}"
            );
        }
    }
}

// spec: GEN-110
#[test]
fn the_drainage_area_codes_start_at_a_cells_own_area_and_the_face_change_is_the_same_as_without() {
    let spec = FaceSpec { face: Face(0), cells: 24, radius_m: R, sea_level: 0.0, seed: 17, halo: 4 };
    let (change, area) = pass1_face_with_area(&spec, &base, None, &CarveParams::default(), 2);
    assert_eq!(area.len(), 24 * 24);
    assert!(area.iter().any(|&c| c == 0), "a cell with nothing above it holds its own area");
    assert!(*area.iter().max().unwrap() >= 8, "somewhere the area is at least two cells");
    assert!((0..255u8).all(|c| area_cells(c) < area_cells(c + 1)) && area_cells(0) == 1.0);
    let plain = planet_generators::bake::face::pass1_face(&spec, &base, None, &CarveParams::default(), 2);
    assert_eq!(change, plain);
    let (_, again) = pass1_face_with_area(&spec, &base, None, &CarveParams::default(), 1);
    assert_eq!(area, again, "1 and 2 threads give the same codes");
}

// spec: STRM-072
#[test]
fn the_channels_come_back_as_computed_and_a_run_stores_the_same_bytes_on_1_and_8_threads() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let (a, b) = (pass2_dir(&w, &s, &f, "one", 1), pass2_dir(&w, &s, &f, "eight", 8));
    assert_eq!(pack_bytes(a.path()), pack_bytes(b.path()));
    let store = TileStore::open(a.path(), w.params()).unwrap();
    for id in tiles() {
        let computed = pass2_tile(&s.grid, id, &base, &face_change_reader(&f), None, &s.params, 2);
        let d = load_depth(&store, &w, RES, &s, id).unwrap().expect("depth");
        assert_eq!((d.eroded, d.deposited), (computed.eroded, computed.deposited), "tile {}/{}/{}", id.face.0, id.ti, id.tj);
    }
    let other = PassTile { face: Face(1), ti: 0, tj: 0 };
    assert_eq!(load_depth(&store, &w, RES, &s, other).unwrap(), None);
    // The area is in the pass-1 store and is copied to the layer store before the pass-1 store is dropped.
    let p1_store = TileStore::open(p1.path(), w.params()).unwrap();
    let codes = load_area(&p1_store, &w, RES, &pass1(), Face(0)).unwrap().expect("area");
    assert_eq!(codes.len(), 24 * 24);
    assert_eq!(copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap().len(), 6);
    assert!(copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap().is_empty(), "a second copy writes nothing");
    let layer = TileStore::open(a.path(), w.params()).unwrap();
    assert_eq!(load_area(&layer, &w, RES, &pass1(), Face(0)).unwrap(), Some(codes));
}

// spec: STRM-072
#[test]
fn a_channel_of_another_layer_or_a_damaged_stream_is_refused_and_a_store_without_channels_is_completed() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let a = pass2_dir(&w, &s, &f, "a", 2);
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let id = tiles()[0];
    // Another layer's parameters: refused naming both keys, never read as absent.
    let other = TileLayerSpec { inputs_id: 2, ..s };
    let e = load_depth(&store, &w, RES, &other, id).unwrap_err().to_string();
    assert!(e.contains("another layer") && e.contains("key"), "{e}");
    // A damaged stream: a record under the right key whose payload is cut.
    let bad = Dir::new("bad");
    let r0 = pass2_tile(&s.grid, id, &base, &face_change_reader(&f), None, &s.params, 2);
    let mut rec = depth_record(&w, RES, &s, &r0).unwrap();
    rec.payload.truncate(rec.payload.len() - 3);
    let mut writer = PackWriter::open(bad.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&rec).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let bad_store = TileStore::open(bad.path(), w.params()).unwrap();
    let e = load_depth(&bad_store, &w, RES, &s, id).unwrap_err().to_string();
    assert!(e.contains("layer depth"), "{e}");
    // A layer stored before the channels: the tile is there, the run computes again for the depth only and writes no tile twice.
    let old = Dir::new("old");
    let mut writer = PackWriter::open(old.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&tile_record(&w, RES, &s, &r0).unwrap()).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let inputs = Pass2Inputs { base: &base, face_change: &face_change_reader(&f), factor: None };
    let report = run_pass2(old.path(), &w, RES, &s, &inputs, &[id], 2, &mut |_| {}).unwrap();
    assert_eq!((report.computed.len(), report.skipped.len()), (1, 0));
    let old_store = TileStore::open(old.path(), w.params()).unwrap();
    assert_eq!(old_store.keys_for(KIND_LAYER_TILE, layer_tile_id(&s.grid, id).unwrap()).len(), 1);
    assert!(load_depth(&old_store, &w, RES, &s, id).unwrap().is_some());
    let again = run_pass2(old.path(), &w, RES, &s, &inputs, &[id], 2, &mut |_| {}).unwrap();
    assert_eq!((again.computed.len(), again.skipped.len()), (0, 1));
}

// spec: STRM-072
#[test]
fn the_reader_returns_the_stored_depth_at_a_cell_centre_and_the_area_in_log2_cells() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let a = pass2_dir(&w, &s, &f, "a", 2);
    copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap();
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let ch = LayerChannels::new(&store, &w, RES, pass1(), s, 2, 4);
    let r = pass2_tile(&s.grid, tiles()[0], &base, &face_change_reader(&f), None, &s.params, 2);
    // The centre of core cell (10, 7) of tile (0, 0) of face 0.
    let d = s.grid.direction(Face(0), 10, 7);
    let (e, dep) = ch.depth_m(d).unwrap().expect("depth there");
    assert!((e - depth_m(r.eroded[7 * 24 + 10])).abs() < 1e-3 && (dep - depth_m(r.deposited[7 * 24 + 10])).abs() < 1e-3, "{e} {dep}");
    let l = ch.area_log2_cells(d).unwrap().expect("area there");
    assert!((0.0..32.0).contains(&l), "{l}");
    assert!(ch.area_cells(d).unwrap().unwrap() >= 1.0);
}
