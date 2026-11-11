//! Tier A, STRM-082 and GEN-167 (TEST-109): the bake writer of the new chain. K7 over a few tiles of a small face writes the L records with the lake water mask, the basin tables and the commit record; the
//! commit is read back through, a commit for another spec is refused, a bake with no commit leaves none, a feed in area units is refused and the bytes do not depend on the thread count.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::basin_table::BasinSpec;
use planet_cache::global_store::GlobalWorld;
use planet_cache::l_writer::{bake_l, read_basin_table, read_l_tile, ChannelFeed, Commit, LBake, COMMIT_FILE};
use planet_cache::layer_l::{l_tile_key, LTileSpec};
use planet_cache::layer_tiles::{layer_tile_id, KIND_LAYER_TILE};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::hydro::{BasinClass, HydroParams};
use planet_generators::bake::k7::{Channel, K7Params};
use planet_generators::bake::lakes::{Lake, Lakes};
use planet_generators::bake::tile::{TileGrid, TileId as PassTile};
use planet_generators::bake::water_mask::{decode, NO_LAKE};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;
const T: usize = 24;
const H1: usize = 24;
const LAKE: u64 = 42;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-l-writer-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

struct Feed(bool);

impl ChannelFeed for Feed {
    fn q_is_discharge(&self) -> bool {
        self.0
    }
    fn channels_near(&self, _: Face, _: i64, _: i64, _: i64) -> Vec<Channel> {
        Vec::new()
    }
}

fn world() -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4)
}

fn spec(base_id: u64) -> LTileSpec {
    LTileSpec { grid: TileGrid { cells: 192, tile: T, halo: 8, radius_m: R }, k7: K7Params::standard(5, 0.0), base_id, inputs_id: 11 }
}

fn basin() -> BasinSpec {
    BasinSpec { hydro: HydroParams::default(), tile_cells: T as u32, tiles_per_edge: 8, base_id: 7, inputs_id: 11 }
}

fn lake() -> Lake {
    Lake {
        id: LAKE,
        floor_id: 1,
        spill_id: 2,
        floor_m: 250.0,
        spill_m: 590.5,
        level_m: 450.0,
        class: BasinClass::Lake,
        overflows: true,
        wet: true,
        area_m2: 2.0e7,
        cells: 5,
        q_in: 1.0e-3,
        q_out: 1.0e-3,
        tiles: Vec::new(),
    }
}

/// H1 cells (i, j) of face 2: the water is the block 10..12 x 10..12 and the ring one cell west of it at (9, 10).
fn lakes() -> Lakes {
    let cell = |i: usize, j: usize| (j * H1 + i) as u32;
    let mut water: Vec<(u32, u32)> = [(10, 10), (11, 10), (10, 11), (11, 11)].iter().map(|&(i, j)| (cell(i, j), 0)).collect();
    water.sort_unstable();
    Lakes { lakes: vec![lake()], pieces: Vec::new(), dropped_cells: 0, water, ring: vec![(cell(9, 10), 0)] }
}

fn h1_cell(face: u8, i: i64, j: i64) -> Option<usize> {
    (face == 2 && (0..H1 as i64).contains(&i) && (0..H1 as i64).contains(&j)).then(|| (j * H1 as i64 + i) as usize)
}

fn base(d: Vec3) -> f64 {
    300.0 + 100.0 * d.z
}

fn zero(_: Vec3) -> f64 {
    0.0
}

fn tiles() -> Vec<PassTile> {
    (2..=4u32).flat_map(|tj| (2..=4u32).map(move |ti| PassTile { face: Face(2), ti, tj })).collect()
}

fn bake(dir: &Dir, w: &GlobalWorld, s: LTileSpec, feed: &Feed, l: &Lakes, threads: usize) -> Result<Commit, String> {
    let b = LBake {
        world: w,
        res: RES,
        spec: s,
        basin: basin(),
        base: &base,
        regional: &zero,
        feed,
        lakes: l,
        h1_cells: H1,
        h1_cell: &h1_cell,
        face_of: &|_| Face(2),
        threads,
    };
    bake_l(dir.path(), &b, &tiles())
}

/// The mask the rule gives on this fixture: every level is above the surface, so a cell is wet when its H1 parent is a water cell, or a ring cell within 8 steps of one (the ring cell at (9, 10) is next to the block).
fn expected(ti: u32, tj: u32) -> Vec<u64> {
    let in_set =
        |i: i64, j: i64| h1_cell(2, i, j).is_some_and(|c| lakes().water.iter().chain(lakes().ring.iter()).any(|e| e.0 as usize == c));
    (0..T * T)
        .map(|k| {
            let (x, y) = (i64::from(ti) * T as i64 + (k % T) as i64, i64::from(tj) * T as i64 + (k / T) as i64);
            if in_set(x.div_euclid(8), y.div_euclid(8)) {
                LAKE
            } else {
                NO_LAKE
            }
        })
        .collect()
}

// spec: STRM-082, GEN-167, TEST-109
#[test]
fn a_bake_writes_tiles_with_their_masks_the_tables_and_a_commit_that_reads_back() {
    let (w, d) = (world(), Dir::new("round"));
    assert_eq!(Commit::read(d.path()).unwrap(), None, "no bake, no commit");
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 2).expect("the bake");
    assert_eq!(Commit::read(d.path()).unwrap(), Some(c.clone()), "the commit on disk is the one returned");
    assert_eq!(c.tiles.len(), 9);
    let store = TileStore::open(d.path(), w.params()).unwrap();
    for id in tiles() {
        let t = read_l_tile(&store, &w, RES, &spec(7), &c, id).unwrap().expect("a tile of the commit");
        let want = expected(id.ti, id.tj);
        assert_eq!(decode(&t.water, T).unwrap(), want, "tile ({}, {}): the water section decodes to the mask", id.ti, id.tj);
        assert_eq!(t.water.is_empty(), want.iter().all(|&l| l == NO_LAKE), "a tile no lake touches has the empty section");
        assert_eq!(t.core.len(), T * T);
    }
    // The lake is in tile (3, 3) and not in its neighbours' cores.
    let t33 = read_l_tile(&store, &w, RES, &spec(7), &c, PassTile { face: Face(2), ti: 3, tj: 3 }).unwrap().unwrap();
    assert!(!t33.water.is_empty());
    // A tile the commit does not hold reads as absent through it.
    assert_eq!(read_l_tile(&store, &w, RES, &spec(7), &c, PassTile { face: Face(2), ti: 6, tj: 6 }).unwrap(), None);
    // The tables: the producer's lake on its face, the other faces empty.
    for f in 0..6u8 {
        let got = read_basin_table(&store, &w, RES, &basin(), &c, Face(f)).unwrap().expect("a table for every face");
        assert_eq!(got, if f == 2 { vec![lake()] } else { Vec::new() }, "face {f}");
    }
}

// spec: STRM-082
#[test]
fn a_commit_of_another_spec_is_refused_naming_both_keys_and_a_damaged_one_is_an_error() {
    let (w, d) = (world(), Dir::new("other"));
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let id = PassTile { face: Face(2), ti: 3, tj: 3 };
    let e = read_l_tile(&store, &w, RES, &spec(8), &c, id).unwrap_err().0;
    assert!(
        e.contains("names key") && e.contains(&l_tile_key(&w, RES, &spec(8), id).unwrap().to_hex()) && e.contains("another base"),
        "{e}"
    );
    let other = BasinSpec { base_id: 9, ..basin() };
    assert!(read_basin_table(&store, &w, RES, &other, &c, Face(2)).unwrap_err().contains("another spec"));
    // The records are content-keyed: read without the commit under the other spec they are refused, not mixed (STRM-081).
    let tile = layer_tile_id(&spec(8).grid, id).unwrap();
    assert!(store.keys_for(KIND_LAYER_TILE, tile).len() == 1);
    // A damaged commit: a changed key breaks the id.
    let text = c.to_text();
    let bad = text.replacen("tile 2 3 3 ", "tile 2 3 3 00", 1);
    assert!(Commit::parse(&bad).is_err());
    let swapped = text.replacen(&c.tables[2].to_hex(), &c.tables[3].to_hex(), 1);
    assert!(Commit::parse(&swapped).unwrap_err().contains("does not match its keys"));
    assert_eq!(Commit::parse(&text).unwrap(), c);
}

// spec: STRM-082
#[test]
fn a_bake_that_fails_before_its_commit_leaves_no_commit_and_the_previous_one_stays() {
    let (w, d) = (world(), Dir::new("killed"));
    let first = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    // A second bake that is refused (a feed in area units) writes nothing and leaves the first commit current.
    let e = bake(&d, &w, spec(8), &Feed(false), &lakes(), 1).unwrap_err();
    assert!(e.contains("area") && e.contains("m3/s"), "{e}");
    assert_eq!(Commit::read(d.path()).unwrap(), Some(first));
    // A bake with a window halo under the ring's depth is refused before any record.
    let mut s = spec(9);
    s.grid.halo = 6;
    let e = bake(&d, &w, s, &Feed(true), &lakes(), 1).unwrap_err();
    assert!(e.contains("halo"), "{e}");
    // Lists that are not what the mask reads are refused.
    let mut l = lakes();
    l.ring = l.water[..1].to_vec();
    assert!(bake(&d, &w, spec(10), &Feed(true), &l, 1).unwrap_err().contains("water list and in its ring"));
    assert!(d.path().join(COMMIT_FILE).exists());
}

// spec: STRM-082, GEN-167, CON-14
#[test]
fn the_records_do_not_depend_on_the_thread_count() {
    let w = world();
    let (d1, d8) = (Dir::new("t1"), Dir::new("t8"));
    let c1 = bake(&d1, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let c8 = bake(&d8, &w, spec(7), &Feed(true), &lakes(), 8).unwrap();
    assert_eq!(c1, c8);
    let (s1, s8) = (TileStore::open(d1.path(), w.params()).unwrap(), TileStore::open(d8.path(), w.params()).unwrap());
    for id in tiles() {
        let tile = layer_tile_id(&spec(7).grid, id).unwrap();
        let key = c1.tiles[&(id.face.0, id.ti, id.tj)];
        let (a, b) = (s1.get(KIND_LAYER_TILE, tile, key).unwrap().unwrap(), s8.get(KIND_LAYER_TILE, tile, key).unwrap().unwrap());
        assert_eq!(a.payload, b.payload, "tile ({}, {}) is the same bytes", id.ti, id.tj);
    }
}

// spec: STRM-082
#[test]
fn the_memory_feed_hands_out_the_segments_near_a_window() {
    use planet_cache::l_writer::MemoryFeed;
    let ch = |x: f64, y: f64| Channel { points: vec![(x, y), (x + 1.0, y)], levels: vec![1.0, 0.5], q: vec![2.0, 3.0] };
    let feed =
        MemoryFeed { face: Face(2), channels: vec![ch(50.0, 50.0), ch(400.0, 400.0), ch(30.0, 60.0), ch(-10.0, 60.0)], discharge: true };
    let got = feed.channels_near(Face(2), 40, 40, 40);
    assert_eq!(
        got.len(),
        2,
        "the one at (50, 50) and the one 10 cells outside the window edge, within the margin; the one far outside is left"
    );
    assert!(feed.q_is_discharge());
    assert!(feed.channels_near(Face(3), 40, 40, 40).is_empty(), "the channels of face 2 are not burned into face 3");
    assert!(feed.channels_near(Face(2), 1000, 1000, 40).is_empty());
}

// spec: STRM-082
#[test]
fn a_grid_with_more_than_eight_local_cells_to_an_h1_cell_and_a_commit_naming_a_seventh_face_are_refused() {
    let (w, d) = (world(), Dir::new("ratio"));
    let b = LBake {
        world: &w,
        res: RES,
        spec: spec(7),
        basin: basin(),
        base: &base,
        regional: &zero,
        feed: &Feed(true),
        lakes: &lakes(),
        h1_cells: 12,
        h1_cell: &h1_cell,
        face_of: &|_| Face(2),
        threads: 1,
    };
    let e = bake_l(d.path(), &b, &tiles()).unwrap_err();
    assert!(e.contains("16 local cells to an H1 cell"), "{e}");
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let bad = c.to_text().replacen("tile 2 ", "tile 258 ", 1);
    assert!(Commit::parse(&bad).unwrap_err().contains("face outside"));
}
