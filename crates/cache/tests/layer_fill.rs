//! Tier A, GEN-121 and STRM-077 (m9 keep-pits-seams PR 2): the post-pass of the layer's fill depth. The final heights of every tile of a face are written as pass 2 would leave them (a scratch store),
//! the post-pass writes kind 17, and the depth equals the depth below the planet-wide spill level of the assembled face (a global minimax flood with a ring of sea around the face: the tiles it
//! does not hold are sea). A basin crosses tile edges and corners; 1 and 8 threads store the same bytes; a second run writes nothing; a depth of another layer is refused; the runtime layer
//! reads the change with the depth added back. A tiny planet: 96 cells a face edge, tiles of 24.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_fill::*;
use planet_cache::layer_fillpass::run_fill_pass;
use planet_cache::layer_store::LayerSpec;
use planet_cache::layer_tiles::*;
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::{fill_code, fill_m};
use planet_generators::bake::tile::{Pass2Params, TileGrid, TileId as PassTile};
use planet_generators::erosion_layer::LayerSource;
use planet_world_def::WorldDefinition;

const R: f64 = 100.0 * 96.0 / core::f64::consts::FRAC_PI_2;
const RES: u32 = 64;
const N: usize = 96;
const T: usize = 24;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static C: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-layer-fill-{}-{}-{name}", std::process::id(), C.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
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

fn pass1() -> LayerSpec {
    LayerSpec { cells: 24, radius_m: R, sea_level: 0.0, seed: 17, carve: CarveParams::default(), inputs_id: 1, uses_factor: false, halo: 4 }
}

fn spec(w: &GlobalWorld) -> TileLayerSpec {
    TileLayerSpec {
        grid: TileGrid { cells: N, tile: T, halo: 6, radius_m: R },
        params: Pass2Params::standard(9, 0.0),
        inputs_id: 1,
        uses_factor: false,
        faces: faces_digest(w, RES, &pass1()),
    }
}

fn face_tiles(face: u8) -> Vec<PassTile> {
    (0..4u32).flat_map(|tj| (0..4u32).map(move |ti| PassTile { face: Face(face), ti, tj })).collect()
}

/// The final heights of face 0 as a function of the global cell: a plateau of 200 m ringed by land at 30 m that falls to the sea at the face's edge, with three bowls: one across a tile edge, one across
/// a four-tile corner, and one wide bowl over the whole middle of the face with a notch to the sea. Multiples of 0.25 m, so f32 holds them exactly.
fn surface(i: usize, j: usize) -> f64 {
    let (x, y) = (i as f64, j as f64);
    let r = |cx: f64, cy: f64| libm::hypot(x - cx, y - cy);
    let mut z = 200.0;
    let edge = i.min(j).min(N - 1 - i).min(N - 1 - j);
    if edge < 6 {
        z = 30.0 - 6.0 * (6 - edge) as f64; // the coast: land at 30 m at 6 cells in, below the sea at the edge
    }
    for (cx, cy, rad, floor) in [(24.0, 12.0, 6.0, 150.0), (48.0, 48.0, 10.0, 120.0), (72.0, 70.0, 5.0, 100.0)] {
        if r(cx, cy) < rad {
            z = z.min(floor);
        }
    }
    // A notch out of the middle bowl to the east coast at 140 m.
    if (46..=50).contains(&j) && (48..N - 6).contains(&i) {
        z = z.min(140.0);
    }
    (z * 4.0).round() / 4.0
}

fn tile_heights(id: PassTile) -> Vec<f32> {
    (0..T * T).map(|p| surface(id.ti as usize * T + p % T, id.tj as usize * T + p / T) as f32).collect()
}

fn write_scratch(dir: &Path, w: &GlobalWorld, s: &TileLayerSpec, tiles: &[PassTile]) {
    let mut writer = PackWriter::open(dir, w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for &id in tiles {
        writer.append(&heights_record_of(w, RES, s, id, &tile_heights(id)).unwrap()).unwrap();
    }
    writer.write_index().unwrap();
    writer.finish().unwrap();
}

/// The oracle: the minimax level of the whole face, with a ring of sea around it (the tiles of the other faces are not held, so they are sea).
fn oracle() -> Vec<f64> {
    let m = N + 2;
    let at = |i: usize, j: usize| j * m + i;
    let mut z = vec![-1.0f64; m * m];
    for j in 0..N {
        for i in 0..N {
            z[at(i + 1, j + 1)] = surface(i, j);
        }
    }
    let key = |v: f64| {
        let b = v.to_bits();
        if b & (1 << 63) != 0 {
            !b
        } else {
            b | (1 << 63)
        }
    };
    let mut level = vec![f64::INFINITY; m * m];
    let mut heap = BinaryHeap::new();
    for p in 0..m * m {
        if z[p] <= 0.0 {
            level[p] = z[p];
            heap.push(Reverse((key(z[p]), p)));
        }
    }
    while let Some(Reverse((k, p))) = heap.pop() {
        if k != key(level[p]) {
            continue;
        }
        let (i, j) = ((p % m) as i64, (p / m) as i64);
        for dj in -1i64..=1 {
            for di in -1i64..=1 {
                let (ii, jj) = (i + di, j + dj);
                if (di, dj) != (0, 0) && ii >= 0 && jj >= 0 && ii < m as i64 && jj < m as i64 {
                    let q = at(ii as usize, jj as usize);
                    let cand = level[p].max(z[q]);
                    if cand < level[q] {
                        level[q] = cand;
                        heap.push(Reverse((key(cand), q)));
                    }
                }
            }
        }
    }
    (0..N * N).map(|p| level[at(p % N + 1, p / N + 1)]).collect()
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

fn fill_pass(
    w: &GlobalWorld,
    s: &TileLayerSpec,
    scratch: &Path,
    tiles: &Path,
    which: &[PassTile],
    threads: usize,
) -> planet_cache::layer_fillpass::FillPassReport {
    run_fill_pass(tiles, scratch, w, RES, s, which, threads, &mut |_, _| {}).unwrap()
}

// spec: GEN-121
#[test]
fn the_post_pass_depth_is_the_depth_below_the_planet_wide_spill_level_across_tile_edges_and_corners() {
    let (w, scratch, out) = (world(), Dir::new("scratch"), Dir::new("tiles"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    write_scratch(scratch.path(), &w, &s, &tiles);
    let report = fill_pass(&w, &s, scratch.path(), out.path(), &tiles, 2);
    assert_eq!(report.written.len(), 16);
    assert_eq!(report.unreached_cells, 0);
    let level = oracle();
    let store = TileStore::open(out.path(), w.params()).unwrap();
    let (mut pits, mut across) = (0usize, 0usize);
    for &id in &tiles {
        let codes = load_fill(&store, &w, RES, &s, id).unwrap().expect("a fill record");
        for (p, &c) in codes.iter().enumerate() {
            let (i, j) = (id.ti as usize * T + p % T, id.tj as usize * T + p / T);
            let want = fill_code((level[j * N + i] - surface(i, j)).max(0.0));
            assert_eq!(
                c,
                want,
                "tile {}/{}/{} cell ({i}, {j}): depth {} m, the oracle's {} m",
                id.face.0,
                id.ti,
                id.tj,
                fill_m(c),
                fill_m(want)
            );
            pits += usize::from(c > 0);
            // Cells of one basin on both sides of a tile edge hold the same level.
            across += usize::from(c > 0 && (p % T == 0 || p % T == T - 1));
        }
    }
    assert!(pits > 400, "the surface holds too few pit cells to test with: {pits}");
    assert!(across > 20, "no basin crosses a tile edge: {across}");
    // The wide middle bowl is filled to its notch (140 m) on both sides of its tile edges: the cell (47, 47) is in the bowl's floor at 120 m.
    let floor = level[47 * N + 47];
    assert!((floor - 140.0).abs() < 1e-9, "the middle bowl spills at its notch, 140 m, not {floor}");
}

// spec: STRM-077
#[test]
fn the_post_pass_stores_the_same_bytes_on_1_and_8_threads_skips_what_it_holds_and_refuses_another_layer() {
    let (w, scratch) = (world(), Dir::new("scratch"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    write_scratch(scratch.path(), &w, &s, &tiles);
    let (a, b) = (Dir::new("one"), Dir::new("eight"));
    fill_pass(&w, &s, scratch.path(), a.path(), &tiles, 1);
    fill_pass(&w, &s, scratch.path(), b.path(), &tiles, 8);
    assert_eq!(pack_bytes(a.path()), pack_bytes(b.path()), "1 and 8 threads store other bytes");
    let again = fill_pass(&w, &s, scratch.path(), a.path(), &tiles, 2);
    assert!(again.written.is_empty() && again.skipped.len() == 16, "a second run wrote {:?}", again.written);
    let store = TileStore::open(a.path(), w.params()).unwrap();
    let other = TileLayerSpec { inputs_id: 2, ..s };
    let e = load_fill(&store, &w, RES, &other, tiles[0]).unwrap_err().to_string();
    assert!(e.contains("another layer") && e.contains("key"), "{e}");
    assert_eq!(load_fill(&store, &w, RES, &s, PassTile { face: Face(1), ti: 0, tj: 0 }).unwrap(), None);
    // A tile the scratch store lacks is an error naming the tile, not a silent sea.
    let few = Dir::new("few");
    write_scratch(few.path(), &w, &s, &tiles[..3]);
    let c = Dir::new("c");
    let e = run_fill_pass(c.path(), few.path(), &w, RES, &s, &tiles[..4], 1, &mut |_, _| {}).unwrap_err().to_string();
    assert!(e.contains("no final heights") && e.contains("0/3/0"), "{e}");
}

// spec: STRM-077
#[test]
fn a_run_over_a_region_treats_the_tiles_it_does_not_hold_as_sea() {
    // Two tiles of the middle row: the bowl that spans them spills at the notch into the east edge, which this run does not hold: the east tile's edge is sea.
    let (w, scratch, out) = (world(), Dir::new("scratch"), Dir::new("tiles"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    write_scratch(scratch.path(), &w, &s, &tiles);
    let two: Vec<PassTile> = tiles.iter().copied().filter(|t| t.tj == 2 && t.ti < 2).collect();
    let report = fill_pass(&w, &s, scratch.path(), out.path(), &two, 2);
    assert_eq!(report.written.len(), 2);
    let store = TileStore::open(out.path(), w.params()).unwrap();
    let codes = load_fill(&store, &w, RES, &s, two[1]).unwrap().unwrap();
    assert!(codes.iter().all(|c| *c == 0), "a bowl that reaches the border of the run was filled: the run's edge is sea");
    // The same directory refuses a run over another set of tiles: its depths would be those of the first run.
    let e = run_fill_pass(out.path(), scratch.path(), &w, RES, &s, &tiles, 2, &mut |_, _| {}).unwrap_err().to_string();
    assert!(e.contains("another set of tiles"), "{e}");
}

// spec: GEN-121
#[test]
fn the_runtime_layer_reads_the_change_with_the_post_pass_depth_added_back() {
    let (w, scratch, dir) = (world(), Dir::new("scratch"), Dir::new("layer"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    // Pass 2 on a smooth bowl terrain, with the scratch store; then the post-pass.
    let base = |d: Vec3| {
        let b = 300.0 * libm::exp(-(1.0 - d.x) / 0.004) * 0.0 + 150.0 - 400.0 * libm::exp(-(1.0 - d.x * 0.97 - d.y * 0.24) / 0.0015);
        if d.x > 0.5 {
            b.max(5.0)
        } else {
            -50.0
        }
    };
    let none = |_: Vec3| 0.0;
    let report = run_pass2(
        dir.path(),
        &w,
        RES,
        &s,
        &Pass2Inputs { base: &base, face_change: &none, factor: None, scratch: Some(scratch.path()) },
        &tiles,
        2,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(report.computed.len(), 16);
    let fp = fill_pass(&w, &s, scratch.path(), dir.path(), &tiles, 2);
    assert_eq!(fp.written.len(), 16);
    // A rerun after the scratch store is gone: pass 2 does not run again (the tiles hold their fill), and the post-pass finds nothing to do without any scratch.
    let fresh = Dir::new("fresh");
    let again = run_pass2(
        dir.path(),
        &w,
        RES,
        &s,
        &Pass2Inputs { base: &base, face_change: &none, factor: None, scratch: Some(fresh.path()) },
        &tiles,
        2,
        &mut |_| {},
    )
    .unwrap();
    assert!(again.computed.is_empty() && again.skipped.len() == 16, "a rerun recomputed {:?}", again.computed);
    let nothing = fill_pass(&w, &s, fresh.path(), dir.path(), &tiles, 2);
    assert!(nothing.written.is_empty() && nothing.skipped.len() == 16);
    let store = Arc::new(TileStore::open(dir.path(), w.params()).unwrap());
    let layer = StoreLayer::new(store.clone(), Arc::new(world()), RES, s, 4, default_change_range()).unwrap();
    let mut with_pits = 0;
    for &id in &tiles {
        let stored = load_tile(&store, &w, RES, &s, id).unwrap().expect("tile");
        let codes = load_fill(&store, &w, RES, &s, id).unwrap().expect("fill");
        let tile = layer.tile(id.face, id.ti, id.tj).unwrap().expect("a tile");
        for (p, (c, f)) in stored.core.iter().zip(&codes).enumerate() {
            let want = c + fill_m(*f) as f32;
            assert_eq!(tile.cell(p % T, p / T), want, "tile {}/{}/{} cell {p}", id.face.0, id.ti, id.tj);
        }
        with_pits += codes.iter().filter(|c| **c > 0).count();
    }
    assert!(with_pits > 0, "no tile holds a pit, the test reads nothing");
}

// spec: STRM-077
#[test]
fn the_step_of_the_pit_depth_code_is_part_of_the_key_and_the_records_have_keys_of_their_own() {
    assert_eq!(steps_per_m(), 4.0);
    let w = world();
    let s = spec(&w);
    let id = face_tiles(0)[0];
    let k = fill_key(&w, RES, &s, id).unwrap();
    assert_ne!(k, planet_cache::layer_channels::depth_key(&w, RES, &s, id).unwrap(), "the pit depth shares the deposited depth's key");
    assert_ne!(k, heights_key(&w, RES, &s, id).unwrap(), "the pit depth shares the scratch heights' key");
}
