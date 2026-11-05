//! Tier A, GEN-110, GEN-111 and STRM-072 (m9-bake): the erosion layer's two baked channels in a store. The drainage area of a face and the erosion and deposition depth of a tile come back as
//! the computed codes, 1 and 8 threads store the same bytes, a channel of another layer or a damaged stream is refused, the depth net equals the change, a store made before the channels is
//! completed without writing a tile twice, and the reader returns the stored values. A tiny planet: tiles of 24 cells, faces of 24 cells.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

fn caches(area_bytes: usize, tile_bytes: usize) -> ChannelCaches {
    ChannelCaches { area_bytes, area_window: 8, tile_bytes }
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
fn the_deposited_depth_codes_are_1_m_steps_and_the_removed_depth_is_what_the_change_leaves() {
    assert_eq!(depth_code(0.0), 0);
    assert_eq!(depth_code(-3.0), 0);
    assert_eq!(depth_code(0.49), 0);
    assert_eq!(depth_code(0.51), 1);
    assert_eq!(depth_code(254.6), 255);
    assert_eq!(depth_code(1.0e6), 255);
    assert!((0..255u8).all(|c| depth_m(c) < depth_m(c + 1)));
    assert_eq!((depth_m(0), depth_m(7), depth_m(255)), (0.0, 7.0, 255.0));
    assert_eq!((removed_depth_m(5.0, -3.0), removed_depth_m(5.0, 8.0), removed_depth_m(0.0, 0.0)), (8.0, 0.0, 0.0));
    let g = TileGrid { cells: 96, tile: 24, halo: 6, radius_m: R };
    for face_change in [0.0, -30.0, 10.0] {
        let r = pass2_tile(&g, tiles()[0], &base, &|_: Vec3| face_change, None, &Pass2Params::standard(9, 0.0), 2);
        assert_eq!(r.deposited.len(), 24 * 24);
        assert!(r.deposited.iter().any(|&c| c > 0) && r.deposited.contains(&0), "deposition happens in places");
        // The removed depth the reader derives, max(0, deposited - change), is what the cell lost net of its deposit, and never negative: eroded = deposited - change + the pass-1 raise, so
        // the reading is short by the raise where pass 1 raised the cell (the caveat) and equal elsewhere, to the 1 m step of the code and the 0.125 m step of the change.
        for (p, c) in r.core.iter().enumerate() {
            let (d, c) = (f64::from(depth_m(r.deposited[p])), f64::from(*c));
            let removed = f64::from(removed_depth_m(d as f32, c as f32));
            assert!(removed >= 0.0);
            assert!(
                d - c >= -face_change.max(0.0) - 0.7,
                "face change {face_change}, cell {p}: deposited {d} less the change {c} is more negative than the raise allows"
            );
            if face_change <= 0.0 && r.deposited[p] < 255 {
                assert!(
                    d - c >= -0.7,
                    "face change {face_change}, cell {p}: what was removed would be negative: deposited {d}, change {c}"
                );
            }
        }
    }
}

// spec: GEN-110
#[test]
fn the_drainage_area_codes_start_at_a_cells_own_area_and_the_face_change_is_the_same_as_without() {
    let spec = FaceSpec { face: Face(0), cells: 24, radius_m: R, sea_level: 0.0, seed: 17, halo: 4 };
    let (change, area) = pass1_face_with_area(&spec, &base, None, &CarveParams::default(), 2);
    assert_eq!(area.len(), 24 * 24);
    assert!(area.contains(&0), "a cell with nothing above it holds its own area");
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
        assert_eq!(d.deposited, computed.deposited, "tile {}/{}/{}", id.face.0, id.ti, id.tj);
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
    assert!(e.contains("layer depth 0/0/0"), "{e}");
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
    let ch = LayerChannels::new(Arc::new(store), Arc::new(w.clone()), RES, RES, pass1(), s, caches(2 * 24 * 24, 4 * 24 * 24));
    let r = pass2_tile(&s.grid, tiles()[0], &base, &face_change_reader(&f), None, &s.params, 2);
    // The centre of core cell (10, 7) of tile (0, 0) of face 0.
    let d = s.grid.direction(Face(0), 10, 7);
    let dep = ch.depth_m(d).unwrap().expect("depth there");
    assert!((dep - depth_m(r.deposited[7 * 24 + 10])).abs() < 1e-3, "{dep}");
    let l = ch.area_log2_cells(d).unwrap().expect("area there");
    assert!((0.0..32.0).contains(&l), "{l}");
    assert!(ch.area_cells(d).unwrap().unwrap() >= 1.0);
}

// spec: STRM-072
#[test]
fn the_depth_reads_across_a_tile_border_without_a_step() {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let a = pass2_dir(&w, &s, &f, "a", 2);
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let ch = LayerChannels::new(Arc::new(store), Arc::new(w.clone()), RES, RES, pass1(), s, caches(2 * 24 * 24, 4 * 24 * 24));
    // Tiles (0, 0) and (1, 0) of face 0 meet between cells 23 and 24 of the 96-cell grid; read along row 10 in tenths of a cell.
    let at = |i: f64| {
        let n = 96.0;
        let d = planet_core::cube::face_to_direction(
            &planet_core::cube::TangentWarp,
            Face(0),
            2.0 * (i + 0.5) / n - 1.0,
            2.0 * (10.0 + 0.5) / n - 1.0,
        );
        ch.depth_m(d).unwrap().expect("depth")
    };
    let (c23, c24) = (at(23.0), at(24.0));
    let mut prev = c23;
    for k in 1..=10 {
        let v = at(23.0 + f64::from(k) / 10.0);
        assert!((v - prev).abs() <= (c24 - c23).abs() + 1e-3, "a step at {k}/10: {prev} to {v}");
        assert!(v >= c23.min(c24) - 1e-3 && v <= c23.max(c24) + 1e-3, "{v} leaves {c23} {c24}");
        prev = v;
    }
}

/// A reader over a layer store of the tiny planet (a face of 24 cells) with windows of 8 cells and the given byte caps; the two directories are the stores' (kept alive).
fn reader(area_bytes: usize, tile_bytes: usize) -> (LayerChannels, Dir, Dir) {
    let (w, p1) = (world(), Dir::new("p1"));
    let s = spec(&w);
    let f = faces(p1.path(), &w);
    let a = pass2_dir(&w, &s, &f, "a", 2);
    copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap();
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let caps = ChannelCaches { area_bytes, area_window: 8, tile_bytes };
    (LayerChannels::new(Arc::new(store), Arc::new(w), RES, RES, pass1(), s, caps), p1, a)
}

/// The direction of cell `(x, 7)` of face 0 on pass 2's 96-cell grid: pass 1's cell about a quarter of `x` (windows of 8 cells: `x` 10 is window 0, `x` 40 is window 1).
fn at(x: i64) -> Vec3 {
    spec(&world()).grid.direction(Face(0), x, 7)
}

// spec: STRM-075
#[test]
fn a_window_of_the_area_equals_the_same_cells_of_the_whole_face_and_a_bad_window_is_refused() {
    let (w, p1) = (world(), Dir::new("p1"));
    faces(p1.path(), &w);
    let a = Dir::new("a");
    copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap();
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let n = pass1().cells;
    let whole = load_area(&store, &w, RES, &pass1(), Face(0)).unwrap().expect("an area");
    // Every window of 9 cells (8 and the cell past), a window cut off at the face's edge, and the whole face.
    for (y0, x0) in [(0, 0), (8, 0), (0, 8), (16, 16), (15, 15), (0, 0)] {
        let (rows, cols) = (y0..(y0 + 9).min(n), x0..(x0 + 9).min(n));
        let win = load_area_window(&store, &w, RES, &pass1(), Face(0), rows.clone(), cols.clone()).unwrap().expect("a window");
        assert_eq!((win.x0, win.y0, win.w, win.h), (x0, y0, cols.len(), rows.len()));
        for j in rows.clone() {
            for i in cols.clone() {
                assert_eq!(win.at(i, j), whole[j * n + i], "cell ({i}, {j}) of the window at ({x0}, {y0})");
            }
        }
    }
    let all = load_area_window(&store, &w, RES, &pass1(), Face(0), 0..n, 0..n).unwrap().unwrap();
    assert_eq!(all.codes, whole);
    // A window outside the face is refused, and a face without an area is none.
    assert!(load_area_window(&store, &w, RES, &pass1(), Face(0), 0..n + 1, 0..n).is_err());
    assert!(load_area_window(&store, &w, RES, &pass1(), Face(0), 3..3, 0..n).is_err());
}

// spec: STRM-075
#[test]
fn the_decoded_caches_are_bounded_by_bytes_and_a_window_over_the_cap_is_held_alone() {
    let window_bytes = 9 * 9;
    // The cap holds one window: reading windows 0, 1, 0 decodes three times (the second evicts the first) and the cache never holds more than one window's bytes.
    let (ch, _p1, _a) = reader(window_bytes, 24 * 24);
    for x in [10, 40, 10] {
        assert!(ch.area_log2_cells(at(x)).unwrap().is_some());
    }
    let (s, held, _) = ch.stats();
    assert_eq!((s.window_decodes, s.window_hits), (3, 0), "{s:?}");
    assert_eq!(held, window_bytes, "one window is held, not two");
    // A cap below one window keeps the window just decoded (never refused), and a second read of it is a hit.
    let (ch, _p1, _a) = reader(1, 1);
    assert!(ch.area_log2_cells(at(10)).unwrap().is_some() && ch.area_log2_cells(at(12)).unwrap().is_some());
    let (s, held, _) = ch.stats();
    assert_eq!((s.window_decodes, s.window_hits, held), (1, 1, window_bytes), "{s:?}");
    // A roomy cap holds both windows: 0, 1, 0 decodes twice and hits once; a read inside a held window is free.
    let (ch, _p1, _a) = reader(8 * window_bytes, 8 * 24 * 24);
    for x in [10, 40, 10] {
        ch.area_log2_cells(at(x)).unwrap();
    }
    assert_eq!((ch.stats().0.window_decodes, ch.stats().0.window_hits), (2, 1));
}

// spec: STRM-075
#[test]
fn workers_asking_for_one_window_at_once_decode_it_once_and_the_trait_reads_what_the_reader_reads() {
    use planet_generators::channels::ChannelSource;
    let (ch, _p1, _a) = reader(1 << 20, 1 << 20);
    let d = at(10);
    let ch = Arc::new(ch);
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let ch = ch.clone();
            std::thread::spawn(move || ch.area_log2_cells(d).unwrap().unwrap())
        })
        .collect();
    let values: Vec<f32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(values.iter().all(|v| *v == values[0]));
    let (s, _, _) = ch.stats();
    assert_eq!((s.window_decodes, s.window_hits), (1, 5), "six workers, one decode: {s:?}");
    // The trait gives the same numbers as the inherent reads, and a depth read goes through the tile cache.
    let src: &dyn ChannelSource = ch.as_ref();
    assert_eq!(src.area_log2_cells(d).unwrap(), Some(values[0]));
    assert_eq!(src.deposited_depth_m(d).unwrap(), ch.depth_m(d).unwrap());
    assert!(ch.stats().0.tile_decodes >= 1);
}

// spec: STRM-075
#[test]
fn the_diagnostic_decode_that_skips_the_key_check_is_called_by_no_runtime_code() {
    // Every use of the name outside its definition, its tests and the probe examples would make the key check optional in the viewer or the generation path.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut uses = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name != "target" && name != "tests" && name != "examples" {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|x| x == "rs")
                && fs::read_to_string(&path).unwrap().contains("diagnostic_decode_unchecked")
            {
                uses.push(path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    assert_eq!(uses, ["cache/src/layer_channels.rs"], "the diagnostic decode is used outside its definition: {uses:?}");
}

// spec: STRM-075
#[test]
fn a_read_at_a_window_boundary_equals_the_bilinear_mean_of_the_whole_face_and_a_channel_of_another_layer_is_refused() {
    use planet_core::cube::{direction_to_face, TangentWarp};
    assert_eq!(ChannelCaches::DEFAULT, ChannelCaches { area_bytes: 16 << 20, area_window: 256, tile_bytes: 256 << 20 });
    let (w, p1) = (world(), Dir::new("p1"));
    let spec_t = spec(&w);
    let f = faces(p1.path(), &w);
    let _ = f;
    let a = Dir::new("a");
    copy_areas(p1.path(), a.path(), &w, RES, &pass1(), &all_faces()).unwrap();
    let store = Arc::new(TileStore::open(a.path(), w.params()).unwrap());
    let whole = load_area(&store, &w, RES, &pass1(), Face(0)).unwrap().expect("an area");
    let n = pass1().cells;
    let ch = LayerChannels::new(
        store.clone(),
        Arc::new(w.clone()),
        RES,
        RES,
        pass1(),
        spec_t,
        ChannelCaches { area_bytes: 1 << 20, area_window: 8, tile_bytes: 1 << 20 },
    );
    // Pass 2's cells 28 to 33 put the point between pass 1's cells 7 and 8, 15 and 16: the cell past the window's step is read from the window's one-past column.
    for x in [28i64, 29, 30, 31, 32, 33, 60, 61, 62, 63] {
        let d = spec_t.grid.direction(Face(0), x, 7);
        let (_, s, t) = direction_to_face(&TangentWarp, d);
        let (fx, fy) =
            (((s + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64), ((t + 1.0) * 0.5 * n as f64 - 0.5).clamp(0.0, (n - 1) as f64));
        let (i, j) = ((fx.floor() as usize).min(n - 2), (fy.floor() as usize).min(n - 2));
        let (ax, ay) = (fx - i as f64, fy - j as f64);
        let c = |a: usize, b: usize| f64::from(whole[b * n + a]);
        let expect = (1.0 - ay) * ((1.0 - ax) * c(i, j) + ax * c(i + 1, j)) + ay * ((1.0 - ax) * c(i, j + 1) + ax * c(i + 1, j + 1));
        let got = ch.area_log2_cells(d).unwrap().expect("an area");
        assert_eq!(got, (expect / 8.0) as f32, "x {x}: cells ({i}, {j})");
    }
    // A window of a layer made with other parameters is refused by name, never mixed.
    let other = LayerSpec { seed: pass1().seed + 1, ..pass1() };
    let e = load_area_window(&store, &w, RES, &other, Face(0), 0..9, 0..9).unwrap_err().to_string();
    assert!(e.contains("another layer") || e.contains("not mixed"), "{e}");
}

// spec: STRM-075
#[test]
fn the_identity_of_the_channels_follows_the_resolutions_of_their_keys_and_the_readers_version() {
    use planet_generators::channels::ChannelSource;
    let (w, s) = (world(), spec(&world()));
    let store = Arc::new(TileStore::open(Dir::new("empty").path(), w.params()).unwrap_or_else(|_| panic!("a store")));
    let make = |a: u32, d: u32| LayerChannels::new(store.clone(), Arc::new(w.clone()), a, d, pass1(), s, ChannelCaches::DEFAULT).identity();
    assert_eq!(make(RES, RES), make(RES, RES), "equal specs, equal identity");
    assert_ne!(make(RES, RES), make(RES + 1, RES), "the area keys' resolution is part of it");
    assert_ne!(make(RES, RES), make(RES, RES + 1), "the depth keys' resolution is part of it");
    assert_eq!(CHANNEL_READER_VERSION, 1, "a change of the sampling rule raises the version, which enters the identity");
}
