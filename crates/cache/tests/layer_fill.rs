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
    // Two lakes below the sea level in the plateau, far from the coast: whether a pit or an outlet depends on the ocean mask.
    for (cx, cy) in [(72.0, 24.0), (72.0, 72.0)] {
        if r(cx, cy) < 4.0 {
            z = -3.0;
        }
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
    oracle_with(&|_, _| true)
}

/// The oracle with a mask: a cell at or below the sea level is an outlet only where `ocean(i, j)` says so (the ring of absent tiles is always sea).
fn oracle_with(ocean: &dyn Fn(usize, usize) -> bool) -> Vec<f64> {
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
        let (pi, pj) = (p % m, p / m);
        let ring = pi == 0 || pj == 0 || pi == m - 1 || pj == m - 1;
        if z[p] <= 0.0 && (ring || ocean(pi - 1, pj - 1)) {
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

/// The layer's tile records for the synthetic heights (base 0, so the stored change is the height), written where a tile has none, as pass 2 would have left them.
fn ensure_tile_records(w: &GlobalWorld, s: &TileLayerSpec, tiles: &Path, which: &[PassTile]) {
    let store = TileStore::open(tiles, w.params()).unwrap();
    let mut writer = PackWriter::open(tiles, w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for &id in which {
        if load_tile(&store, w, RES, s, id).unwrap().is_none() {
            let stats = planet_generators::bake::tile::TileStats {
                land_cells: 0,
                thermal_passes: 0,
                thermal_moving_at_cap: 0,
                eroded_m3: 0.0,
                deposited_m3: 0.0,
                exported_m3: 0.0,
                clamped_cells: 0,
            };
            let r = planet_generators::bake::tile::TileResult {
                id,
                core: tile_heights(id),
                deposited: Vec::new(),
                fill: Vec::new(),
                heights: Vec::new(),
                stats,
            };
            writer.append(&tile_record(w, RES, s, &r).unwrap()).unwrap();
        }
    }
    writer.write_index().unwrap();
    writer.finish().unwrap();
}

fn fill_pass(
    w: &GlobalWorld,
    s: &TileLayerSpec,
    scratch: &Path,
    tiles: &Path,
    which: &[PassTile],
    threads: usize,
) -> planet_cache::layer_fillpass::FillPassReport {
    ensure_tile_records(w, s, tiles, which);
    run_fill_pass(tiles, scratch, w, RES, s, which, threads, &|_| 0.0, &|_| true, "all", &mut |_, _| {}).unwrap()
}

// spec: GEN-121
// spec: GEN-120
#[test]
fn the_post_pass_depth_is_the_depth_below_the_planet_wide_spill_level_across_tile_edges_and_corners() {
    let (w, scratch, out) = (world(), Dir::new("scratch"), Dir::new("tiles"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    write_scratch(scratch.path(), &w, &s, &tiles);
    let report = fill_pass(&w, &s, scratch.path(), out.path(), &tiles, 2);
    assert_eq!(report.written.len(), 16);
    assert_eq!(report.unreached_cells, 0);
    assert_eq!(report.dropped_pairs, 0, "a neighbour pair inside a face was dropped");
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
    let e = run_fill_pass(c.path(), few.path(), &w, RES, &s, &tiles[..4], 1, &|_| 0.0, &|_| true, "all", &mut |_, _| {})
        .unwrap_err()
        .to_string();
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
    let e = run_fill_pass(out.path(), scratch.path(), &w, RES, &s, &tiles, 2, &|_| 0.0, &|_| true, "all", &mut |_, _| {})
        .unwrap_err()
        .to_string();
    assert!(e.contains("another set of tiles"), "{e}");
}

// spec: GEN-121
// spec: GEN-120
// spec: STRM-076
// spec: STRM-077
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
        &Pass2Inputs {
            base: &base,
            face_change: &none,
            factor: None,
            scratch: Some(scratch.path()),
            rock: planet_generators::bake::face::RockFields::none(),
        },
        &tiles,
        2,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(report.computed.len(), 16);
    // The post-pass takes the exact base: the depth is `level - (base + stored change)`, so a floor is flat against the surface the runtime shows (GEN-120).
    let fp = run_fill_pass(dir.path(), scratch.path(), &w, RES, &s, &tiles, 2, &base, &|_| true, "all", &mut |_, _| {}).unwrap();
    assert_eq!(fp.written.len(), 16);
    // A rerun after the scratch store is gone: pass 2 does not run again (the tiles hold their fill), and the post-pass finds nothing to do without any scratch.
    let fresh = Dir::new("fresh");
    let again = run_pass2(
        dir.path(),
        &w,
        RES,
        &s,
        &Pass2Inputs {
            base: &base,
            face_change: &none,
            factor: None,
            scratch: Some(fresh.path()),
            rock: planet_generators::bake::face::RockFields::none(),
        },
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
// spec: STRM-076
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

// spec: GEN-120
#[test]
fn a_filled_floor_is_flat_against_the_exact_base_and_an_outcrop_inside_a_basin_keeps_its_height() {
    use std::collections::VecDeque;
    // Part 1: pass 2 on a smooth bowl terrain, then the post-pass with the exact base: the cells of one connected pit hold one surface (base + stored change + depth) to the code's step.
    let (w, scratch, dir) = (world(), Dir::new("scratch"), Dir::new("layer"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    let base = |d: Vec3| {
        let b = 150.0 - 400.0 * libm::exp(-(1.0 - d.x * 0.97 - d.y * 0.24) / 0.0015);
        if d.x > 0.5 {
            b.max(5.0)
        } else {
            -50.0
        }
    };
    let none = |_: Vec3| 0.0;
    run_pass2(
        dir.path(),
        &w,
        RES,
        &s,
        &Pass2Inputs {
            base: &base,
            face_change: &none,
            factor: None,
            scratch: Some(scratch.path()),
            rock: planet_generators::bake::face::RockFields::none(),
        },
        &tiles,
        2,
        &mut |_| {},
    )
    .unwrap();
    run_fill_pass(dir.path(), scratch.path(), &w, RES, &s, &tiles, 2, &base, &|_| true, "all", &mut |_, _| {}).unwrap();
    let store = TileStore::open(dir.path(), w.params()).unwrap();
    let (mut worst, mut checked) = (0.0f64, 0usize);
    for &id in &tiles {
        let core = load_tile(&store, &w, RES, &s, id).unwrap().unwrap().core;
        let codes = load_fill(&store, &w, RES, &s, id).unwrap().unwrap();
        let mut seen = vec![false; T * T];
        for start in 0..T * T {
            if codes[start] == 0 || seen[start] {
                continue;
            }
            let (mut members, mut queue) = (Vec::new(), VecDeque::from([start]));
            seen[start] = true;
            while let Some(p) = queue.pop_front() {
                members.push(p);
                let (i, j) = ((p % T) as i64, (p / T) as i64);
                for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (ii, jj) = (i + di, j + dj);
                    if (0..T as i64).contains(&ii) && (0..T as i64).contains(&jj) {
                        let q = jj as usize * T + ii as usize;
                        if codes[q] > 0 && !seen[q] {
                            seen[q] = true;
                            queue.push_back(q);
                        }
                    }
                }
            }
            if members.len() < 20 {
                continue;
            }
            let mut v: Vec<f64> = members
                .iter()
                .map(|&p| {
                    let d = s.grid.direction(id.face, (id.ti as usize * T + p % T) as i64, (id.tj as usize * T + p / T) as i64);
                    base(d) + f64::from(core[p]) + fill_m(codes[p])
                })
                .collect();
            v.sort_by(f64::total_cmp);
            let med = v[v.len() / 2];
            worst = worst.max(v.iter().map(|x| (x - med).abs()).fold(0.0, f64::max));
            checked += members.len();
        }
    }
    assert!(checked > 100, "only {checked} pit cells in basins of 20 cells or more");
    assert!(worst <= 0.26, "a filled floor deviates {worst} m from its level against the exact base: the code's step is 0.25 m");
    // Part 2: an outcrop. The synthetic surface has a bowl of 150 m (radius 6 cells around cell (24, 12)) in a plateau of 200 m across tiles (0, 0) and (1, 0); the stored change of one cell inside the bowl is 100 m
    // higher, so its exact surface stands above the level: it keeps its height (depth 0) and is counted, its neighbour is filled to the plateau (50 m deep).
    let (scratch2, out2) = (Dir::new("scratch2"), Dir::new("tiles2"));
    let (id, id1) = (tiles[0], tiles[1]);
    write_scratch(scratch2.path(), &w, &s, &[id, id1]);
    let mut core = tile_heights(id);
    let spire = 12 * T + 22;
    core[spire] += 100.0;
    let stats = planet_generators::bake::tile::TileStats {
        land_cells: 0,
        thermal_passes: 0,
        thermal_moving_at_cap: 0,
        eroded_m3: 0.0,
        deposited_m3: 0.0,
        exported_m3: 0.0,
        clamped_cells: 0,
    };
    let r = planet_generators::bake::tile::TileResult { id, core, deposited: Vec::new(), fill: Vec::new(), heights: Vec::new(), stats };
    let r1 = planet_generators::bake::tile::TileResult {
        id: id1,
        core: tile_heights(id1),
        deposited: Vec::new(),
        fill: Vec::new(),
        heights: Vec::new(),
        stats,
    };
    let mut writer = PackWriter::open(out2.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&tile_record(&w, RES, &s, &r).unwrap()).unwrap();
    writer.append(&tile_record(&w, RES, &s, &r1).unwrap()).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let report =
        run_fill_pass(out2.path(), scratch2.path(), &w, RES, &s, &[id, id1], 1, &|_| 0.0, &|_| true, "all", &mut |_, _| {}).unwrap();
    let store2 = TileStore::open(out2.path(), w.params()).unwrap();
    let codes = load_fill(&store2, &w, RES, &s, id).unwrap().unwrap();
    assert_eq!(codes[spire], 0, "the outcrop was lowered to the level");
    assert_eq!(codes[spire - 1], fill_code_of(50.0), "its neighbour is filled to the plateau's 200 m (50 m deep)");
    assert_eq!(report.clamped_cells, 1, "the clamped cell is counted");
}

fn fill_code_of(m: f64) -> u16 {
    planet_generators::bake::face::fill_code(m)
}

// spec: STRM-077
#[test]
fn an_inland_basin_below_the_sea_level_is_filled_unless_the_ocean_connectivity_calls_it_ocean() {
    // A plateau of 200 m with an 8 x 8 bowl whose floor is 5 m BELOW the sea level, in the middle of tile (0, 0): with the ocean mask false (an inland basin) the bowl is a pit of 205 m; with it true (the
    // sea) the bowl's cells are outlets and hold no depth; the report counts the inland cells.
    let w = world();
    let s = spec(&w);
    let id = face_tiles(0)[0];
    let heights: Vec<f32> =
        (0..T * T).map(|p| if (8..16).contains(&(p % T)) && (8..16).contains(&(p / T)) { -5.0 } else { 200.0 }).collect();
    let (scratch, out) = (Dir::new("scratch"), Dir::new("tiles"));
    let mut writer = PackWriter::open(scratch.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&heights_record_of(&w, RES, &s, id, &heights).unwrap()).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let stats = planet_generators::bake::tile::TileStats {
        land_cells: 0,
        thermal_passes: 0,
        thermal_moving_at_cap: 0,
        eroded_m3: 0.0,
        deposited_m3: 0.0,
        exported_m3: 0.0,
        clamped_cells: 0,
    };
    let r = planet_generators::bake::tile::TileResult {
        id,
        core: heights.clone(),
        deposited: Vec::new(),
        fill: Vec::new(),
        heights: Vec::new(),
        stats,
    };
    let mut writer = PackWriter::open(out.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&tile_record(&w, RES, &s, &r).unwrap()).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let bowl = 12 * T + 12;
    // Not ocean: filled to the rim (205 m deep), 64 inland cells counted.
    let inland = run_fill_pass(out.path(), scratch.path(), &w, RES, &s, &[id], 1, &|_| 0.0, &|_| false, "none", &mut |_, _| {}).unwrap();
    let store = TileStore::open(out.path(), w.params()).unwrap();
    let codes = load_fill(&store, &w, RES, &s, id).unwrap().unwrap();
    assert_eq!(inland.inland_low_cells, 64);
    assert_eq!(codes[bowl], planet_generators::bake::face::fill_code(205.0), "an inland basin below 0 m is filled to its rim");
    // Ocean: the same bowl is sea, an outlet, no depth.
    let (out2, _) = (Dir::new("tiles2"), ());
    let mut writer = PackWriter::open(out2.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    writer.append(&tile_record(&w, RES, &s, &r).unwrap()).unwrap();
    writer.write_index().unwrap();
    writer.finish().unwrap();
    let sea = run_fill_pass(out2.path(), scratch.path(), &w, RES, &s, &[id], 1, &|_| 0.0, &|_| true, "all", &mut |_, _| {}).unwrap();
    let store2 = TileStore::open(out2.path(), w.params()).unwrap();
    assert_eq!(sea.inland_low_cells, 0);
    assert_eq!(load_fill(&store2, &w, RES, &s, id).unwrap().unwrap()[bowl], 0, "an ocean cell is an outlet");
}

// spec: STRM-077
#[test]
fn a_mask_that_differs_between_tiles_gives_the_depth_of_a_global_flood_with_the_same_outlets_and_another_mask_is_refused() {
    // Half of the face's coast is called ocean and half is not (by the direction of the cell); the cells at or below 0 m of the other half are inland basins, filled like pits. Every code equals the oracle's.
    let (w, scratch, out) = (world(), Dir::new("scratch"), Dir::new("tiles"));
    let s = spec(&w);
    let tiles = face_tiles(0);
    write_scratch(scratch.path(), &w, &s, &tiles);
    ensure_tile_records(&w, &s, out.path(), &tiles);
    let ycut = s.grid.direction(Face(0), 48, 48).y;
    let ocean = move |d: Vec3| d.y < ycut;
    let report = run_fill_pass(out.path(), scratch.path(), &w, RES, &s, &tiles, 2, &|_| 0.0, &ocean, "half", &mut |_, _| {}).unwrap();
    assert!(report.inland_low_cells > 0, "no cell is called inland");
    let level = oracle_with(&|i, j| s.grid.direction(Face(0), i as i64, j as i64).y < ycut);
    let store = TileStore::open(out.path(), w.params()).unwrap();
    let mut filled_inland = 0;
    for &id in &tiles {
        let codes = load_fill(&store, &w, RES, &s, id).unwrap().unwrap();
        for (p, &c) in codes.iter().enumerate() {
            let (i, j) = (id.ti as usize * T + p % T, id.tj as usize * T + p / T);
            let want = fill_code((level[j * N + i] - surface(i, j)).max(0.0));
            assert_eq!(c, want, "tile {}/{}/{} cell ({i}, {j})", id.face.0, id.ti, id.tj);
            filled_inland += usize::from(c > 0 && surface(i, j) <= 0.0);
        }
    }
    assert!(filled_inland > 0, "no inland cell at or below the sea level was filled: the mask changed nothing");
    // Another outlet mask in the same directory is refused, naming both.
    let e = run_fill_pass(out.path(), scratch.path(), &w, RES, &s, &tiles, 2, &|_| 0.0, &|_| true, "all", &mut |_, _| {})
        .unwrap_err()
        .to_string();
    assert!(e.contains("another outlet mask") && e.contains("outlets=half") && e.contains("outlets=all"), "{e}");
}
