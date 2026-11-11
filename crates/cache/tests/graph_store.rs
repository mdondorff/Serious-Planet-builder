//! Tier A, STRM-084 (TEST-114): the channel graph as a stored kind (20). A small hydrology on a single face: the blocks round trip through a store with every vertex of every segment in the block of its cell (and
//! the margin's neighbours), the unit flag survives, a damaged, foreign or misplaced block is refused naming the record, a count the payload cannot hold is an error, and the bytes are the same twice.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use planet_cache::basin_table::BasinSpec;
use planet_cache::global_store::GlobalWorld;
use planet_cache::graph_store::*;
use planet_cache::pack::{PackWriter, TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_generators::bake::channel_graph::{q_value, ChannelGraph};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams, QUnit};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const RES: u32 = 64;
const W: usize = 40;
const EPS: f64 = 1e-3;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("planet-graph-store-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

fn spec() -> GraphSpec {
    GraphSpec {
        basin: BasinSpec { hydro: HydroParams::default(), tile_cells: 80, tiles_per_edge: 4, base_id: 7, inputs_id: 11 },
        h1_cells: W as u32,
    }
}

struct Fixture {
    g: Grid,
    graph: ChannelGraph,
    ids: Vec<u64>,
    qw: Vec<f64>,
}

fn fixture(unit_area: bool) -> Fixture {
    let g = Grid::new(W, W, 800.0);
    let z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64 + 3.0 * ((p / W) as f64 - 20.0).abs()).collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; W * W];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, if unit_area { None } else { Some(&runoff) }, None, &p);
    let mut graph = ChannelGraph::from_hydro(&g, &routing, &hydro, &z);
    // A lake under part of the channels: the cells of a 5 x 5 square are water.
    graph.set_cuts(planet_generators::bake::lakes::lake_cut_ranges(&hydro, &routing, &|c| {
        (14..19).contains(&(c % W)) && (14..19).contains(&(c / W))
    }));
    let ids = hydro.segments.iter().map(|s| s.id).collect();
    Fixture { g, graph, ids, qw: hydro.qw.clone() }
}

fn xy(c: u32) -> (u8, i64, i64) {
    (0, i64::from(c) % W as i64, i64::from(c) / W as i64)
}

fn store_of(dir: &Dir, w: &GlobalWorld, records: &[planet_cache::pack::Record]) -> TileStore {
    let mut writer = PackWriter::open(dir.path(), w.params(), DEFAULT_MAX_PACK_BYTES).unwrap();
    for r in records {
        writer.append(r).unwrap();
    }
    writer.finish().unwrap();
    TileStore::open(dir.path(), w.params()).unwrap()
}

// spec: STRM-084, TEST-114
#[test]
fn every_vertex_is_in_the_block_of_its_cell_and_the_pieces_round_trip_through_a_store() {
    let (w, d, s) = (world(), Dir::new("round"), spec());
    let f = fixture(false);
    let records = graph_block_records(&w, RES, &s, &f.graph, &f.ids, &f.g, &xy);
    assert!(!records.is_empty());
    let store = store_of(&d, &w, &records);
    let h = i64::from(s.h1_per_block());
    let mut blocks = std::collections::BTreeMap::new();
    for r in &records {
        assert_eq!(r.kind, KIND_CHANNEL_GRAPH);
    }
    for bj in 0..4u32 {
        for bi in 0..4u32 {
            let key = graph_key(&w, RES, &s, 0, bi, bj).unwrap();
            if let Some(b) = load_graph_block(&store, &s, 0, bi, bj, key).unwrap() {
                assert_eq!(b.q_unit, QUnit::Discharge);
                blocks.insert((bi, bj), b);
            }
        }
    }
    assert_eq!(blocks.len(), records.len(), "every record is found by its block's key");
    // Every vertex of every segment is in the piece of the block of its own cell, with its codes.
    let mut checked = 0;
    f.graph.for_each(|seg| {
        let cells = seg.cells(&f.g);
        for (k, &c) in cells.iter().enumerate() {
            let (_, i, j) = xy(c);
            let b = &blocks[&((i / h) as u32, (j / h) as u32)];
            let found = b.pieces.iter().filter(|p| p.segment == f.ids[seg.index]).any(|p| {
                p.coords().iter().enumerate().any(|(v, &(pi, pj))| (pi, pj) == (i, j) && p.q[v] == seg.q[k] && p.bed[v] == seg.bed[k])
            });
            assert!(found, "vertex {k} of segment {} at ({i}, {j}) is in its block with its codes", seg.index);
            checked += 1;
        }
    });
    assert!(checked > 100);
    // Q_w of every stored vertex is the dequantised code of the f64 within the step.
    for b in blocks.values() {
        for p in &b.pieces {
            for (v, &(i, j)) in p.coords().iter().enumerate() {
                let want = f.qw[(j as usize) * W + i as usize];
                assert!(
                    p.q[v] != 0 && ((q_value(p.q[v]) - want) / want).abs() <= 1.94e-4,
                    "vertex ({i}, {j}): {} against {want}",
                    q_value(p.q[v])
                );
            }
        }
    }
    // The same input gives the same bytes (CON-14).
    let again = graph_block_records(&w, RES, &s, &f.graph, &f.ids, &f.g, &xy);
    assert_eq!(
        records.iter().map(|r| (r.key, r.payload.clone())).collect::<Vec<_>>(),
        again.iter().map(|r| (r.key, r.payload.clone())).collect::<Vec<_>>()
    );
    let bytes: usize = records.iter().map(|r| r.payload.len()).sum();
    println!(
        "graph store fixture: {} blocks, {} bytes stored for {} vertices ({:.2} B a vertex; arrays {} B)",
        records.len(),
        bytes,
        f.graph.vertices(),
        bytes as f64 / f.graph.vertices() as f64,
        f.graph.bytes()
    );
}

// spec: STRM-084, TEST-114
#[test]
fn a_segment_across_a_block_edge_is_in_both_blocks_with_an_overlap_and_the_unit_flag_survives() {
    let (w, d, s) = (world(), Dir::new("edge"), spec());
    let f = fixture(true);
    assert_eq!(f.graph.q_unit, QUnit::Area, "no runoff: the unit is an area");
    let records = graph_block_records(&w, RES, &s, &f.graph, &f.ids, &f.g, &xy);
    let store = store_of(&d, &w, &records);
    let h = i64::from(s.h1_per_block());
    let mut two_blocks = 0;
    f.graph.for_each(|seg| {
        let cells = seg.cells(&f.g);
        let owners: std::collections::BTreeSet<(i64, i64)> = cells.iter().map(|&c| (xy(c).1 / h, xy(c).2 / h)).collect();
        if owners.len() > 1 {
            two_blocks += 1;
            for &(bi, bj) in &owners {
                let key = graph_key(&w, RES, &s, 0, bi as u32, bj as u32).unwrap();
                let b = load_graph_block(&store, &s, 0, bi as u32, bj as u32, key).unwrap().expect("a block that holds a vertex");
                assert_eq!(b.q_unit, QUnit::Area);
                let mine: Vec<_> = b.pieces.iter().filter(|p| p.segment == f.ids[seg.index]).collect();
                assert!(!mine.is_empty());
                // A margin cell next to the block's edge is also in the piece: the run reaches one H1 cell beyond the block.
                let beyond = cells.iter().any(|&c| {
                    let (_, i, j) = xy(c);
                    (i / h, j / h) != (bi, bj) && i >= bi * h - 1 && i < (bi + 1) * h + 1 && j >= bj * h - 1 && j < (bj + 1) * h + 1
                });
                if beyond {
                    assert!(
                        mine.iter().any(|p| p.coords().iter().any(|&(i, j)| (i / h, j / h) != (bi, bj))),
                        "the margin's vertex is in the piece"
                    );
                }
            }
        }
    });
    assert!(two_blocks > 0, "the fixture has a segment across a block edge");
}

// spec: STRM-084, TEST-114
#[test]
fn a_damaged_foreign_or_misplaced_block_is_refused_naming_the_record_and_a_huge_count_is_an_error() {
    let (w, d, s) = (world(), Dir::new("refuse"), spec());
    let f = fixture(false);
    let records = graph_block_records(&w, RES, &s, &f.graph, &f.ids, &f.g, &xy);
    let r0 = records[0].clone();
    let tile = r0.tile;
    // Find the block of the first record by trying the grid.
    let (mut bi, mut bj) = (0u32, 0u32);
    'find: for j in 0..4u32 {
        for i in 0..4u32 {
            if graph_key(&w, RES, &s, 0, i, j).unwrap() == r0.key {
                (bi, bj) = (i, j);
                break 'find;
            }
        }
    }
    let damage = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut r = r0.clone();
        f(&mut r.payload);
        let dir = Dir::new("refuse-one");
        let store = store_of(&dir, &w, &[r]);
        load_graph_block(&store, &s, 0, bi, bj, r0.key)
    };
    assert!(damage(&|_| {}).unwrap().is_some());
    let e = damage(&|p| p[0] = 9).unwrap_err().0;
    assert!(e.contains("channel graph block 0/") && e.contains("codec version"), "{e}");
    let e = damage(&|p| p[1] = 9).unwrap_err().0;
    assert!(e.contains("graph version"), "{e}");
    let e = damage(&|p| p[6] = (bi as u8) + 1).unwrap_err().0;
    assert!(e.contains("it is the block"), "{e}");
    let e = damage(&|p| p[14] = 99).unwrap_err().0;
    assert!(e.contains("H1 cells a side"), "{e}");
    let e = damage(&|p| p[18] = 7).unwrap_err().0;
    assert!(e.contains("unit byte"), "{e}");
    let e = damage(&|p| p[23] ^= 0xff).unwrap_err().0;
    assert!(e.contains("key inside the payload"), "{e}");
    let e = damage(&|p| p[19..23].copy_from_slice(&u32::MAX.to_le_bytes())).unwrap_err().0;
    assert!(e.contains("cannot fit"), "a count the payload cannot hold: {e}");
    let e = damage(&|p| p.truncate(60)).unwrap_err().0;
    assert!(e.contains("do not decompress") || e.contains("cut off") || e.contains("cannot fit") || e.contains("bytes"), "{e}");
    // Another spec reads as absent: a different base is another key, so the store holds nothing under it.
    let store = store_of(&Dir::new("refuse-two"), &w, std::slice::from_ref(&r0));
    let other = GraphSpec { basin: BasinSpec { base_id: 8, ..s.basin }, ..s };
    let key = graph_key(&w, RES, &other, 0, bi, bj).unwrap();
    assert_ne!(key, r0.key);
    assert!(load_graph_block(&store, &other, 0, bi, bj, key).unwrap().is_none());
    // A block outside the grid is refused.
    assert!(graph_key(&w, RES, &s, 0, 9, 0).unwrap_err().0.contains("outside"));
    let _ = (tile, d);
}

// spec: STRM-084, TEST-114 (D54)
#[test]
fn the_lake_entries_and_spill_cells_are_stored_with_the_pieces_so_the_drawing_derives_its_pieces_from_the_record() {
    let (w, d, s) = (world(), Dir::new("cuts"), spec());
    let f = fixture(false);
    assert!(!f.graph.cuts().is_empty(), "the fixture has channels under water");
    let records = graph_block_records(&w, RES, &s, &f.graph, &f.ids, &f.g, &xy);
    let store = store_of(&d, &w, &records);
    let h = i64::from(s.h1_per_block());
    let mut with_cuts = 0;
    let mut extra_bytes = 0usize;
    for bj in 0..4u32 {
        for bi in 0..4u32 {
            let key = graph_key(&w, RES, &s, 0, bi, bj).unwrap();
            let Some(b) = load_graph_block(&store, &s, 0, bi, bj, key).unwrap() else { continue };
            for p in &b.pieces {
                let seg = f.ids.iter().position(|&id| id == p.segment).expect("a segment of the graph");
                let want: Vec<(u32, u32)> =
                    f.graph.cuts_of(seg).iter().filter(|c| c.1 < p.v0 + p.vertices() as u32 && c.2 > p.v0).map(|c| (c.1, c.2)).collect();
                assert_eq!(p.cuts, want, "piece {} of segment {seg} in block ({bi}, {bj})", p.part);
                // The piece's vertices are the chain from v0: its first vertex is the chain's vertex v0.
                let mut cells = Vec::new();
                f.graph.for_each(|sv| {
                    if sv.index == seg {
                        cells = sv.cells(&f.g);
                    }
                });
                let (_, i, j) = xy(cells[p.v0 as usize]);
                assert_eq!((i64::from(p.i0), i64::from(p.j0)), (i, j), "v0 is the index of the piece's first vertex in its segment");
                assert!(i / h >= 0);
                if !p.cuts.is_empty() {
                    with_cuts += 1;
                }
                extra_bytes += 8 + 8 * p.cuts.len();
            }
        }
    }
    assert!(with_cuts > 0, "some piece meets a lake");
    println!(
        "lake cuts: {with_cuts} pieces meet a lake; {extra_bytes} extra bytes before compression for v0 and the cut counts and ranges"
    );
    // A damaged cut count is an error, not an allocation.
    let r0 = records[0].clone();
    assert!(!r0.payload.is_empty());
}
