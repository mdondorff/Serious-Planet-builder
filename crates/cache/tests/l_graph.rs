//! Tier A, STRM-084 and GEN-170 (TEST-114): the channel graph through the writer. A graph of a small hydrology is baked beside the L tiles of a face; a second bake reads the channels back through the commit
//! (`ChannelGraphFeed`) and gives the same L tiles byte for byte as the bake that had them in memory: the stored graph is what K7 burns. A commit without a graph has no feed, and a graph in area units is refused.

use std::sync::Arc;

use crate::l_writer::{zero, Dir, Feed, RES};
use planet_cache::basin_table::BasinSpec;
use planet_cache::global_store::GlobalWorld;
use planet_cache::graph_feed::ChannelGraphFeed;
use planet_cache::graph_store::GraphSpec;
use planet_cache::l_writer::{bake_l, GraphInput, LBake, MemoryFeed};
use planet_cache::layer_l::LTileSpec;
use planet_cache::layer_tiles::{layer_tile_id, KIND_LAYER_TILE};
use planet_cache::pack::TileStore;
use planet_core::Face;
use planet_generators::bake::channel_graph::{channels_from_graph, ChannelGraph};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::k7::K7Params;
use planet_generators::bake::lakes::Lakes;
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::tile::{TileGrid, TileId as PassTile};
use planet_generators::bake::Grid;

const W: usize = 40;
const R: f64 = 6_371_000.0;

/// A ground well above the beds of the H1 channels (500-900 m), so that the burn-in lowers cells.
fn base(d: planet_core::Vec3) -> f64 {
    1500.0 + 100.0 * d.z
}

fn world() -> GlobalWorld {
    crate::l_writer::world()
}

fn spec() -> LTileSpec {
    LTileSpec { grid: TileGrid { cells: 320, tile: 40, halo: 8, radius_m: R }, k7: K7Params::standard(5, 0.0), base_id: 7, inputs_id: 11 }
}

fn basin() -> BasinSpec {
    BasinSpec { hydro: HydroParams::default(), tile_cells: 40, tiles_per_edge: 8, base_id: 7, inputs_id: 11 }
}

struct H1 {
    g: Grid,
    graph: ChannelGraph,
    ids: Vec<u64>,
}

fn h1(runoff: bool) -> H1 {
    let g = Grid::new(W, W, 800.0);
    let z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64 + 3.0 * ((p / W) as f64 - 20.0).abs()).collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, 1e-3, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, 1e-3, true);
    let r = vec![1.0e-8f32; W * W];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, runoff.then_some(&r[..]), None, &p);
    let graph = ChannelGraph::from_hydro(&g, &routing, &hydro, &z);
    let ids = hydro.segments.iter().map(|s| s.id).collect();
    H1 { g, graph, ids }
}

fn xy(c: u32) -> (u8, i64, i64) {
    (2, i64::from(c) % W as i64, i64::from(c) / W as i64)
}

fn tiles() -> Vec<PassTile> {
    (0..8u32).flat_map(|tj| (0..8u32).map(move |ti| PassTile { face: Face(2), ti, tj })).collect()
}

fn run(
    dir: &Dir,
    w: &GlobalWorld,
    feed: &dyn planet_cache::l_writer::ChannelFeed,
    with_graph: Option<&H1>,
) -> Result<planet_cache::l_writer::Commit, String> {
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let gi = with_graph.map(|h| GraphInput {
        spec: GraphSpec { basin: basin(), h1_cells: W as u32 },
        graph: &h.graph,
        segment_ids: &h.ids,
        grid: &h.g,
        cell_xy: &xy,
    });
    let b = LBake {
        world: w,
        res: RES,
        spec: spec(),
        basin: basin(),
        base: &base,
        regional: &zero,
        feed,
        lakes: &lakes,
        h1_cells: W,
        h1_cell: &|_, _, _| None,
        face_of: &|_| Face(2),
        threads: 2,
        graph: gi,
    };
    bake_l(dir.path(), &b, &tiles())
}

// spec: STRM-084, GEN-170, TEST-114
#[test]
fn the_channels_read_back_through_the_commit_give_the_l_tiles_of_the_bake_that_had_them_in_memory() {
    let w = world();
    let h = h1(true);
    assert!(h.graph.segments() > 100);
    let to_local = |c: u32| (8.0 * f64::from(c % W as u32) + 3.5, 8.0 * f64::from(c / W as u32) + 3.5);
    let memory = MemoryFeed { face: Face(2), channels: channels_from_graph(&h.graph, &h.g, &to_local), discharge: true };
    let (da, db) = (Dir::new("graph-a"), Dir::new("graph-b"));
    let a = run(&da, &w, &memory, Some(&h)).expect("bake A");
    assert!(!a.graph.is_empty() && a.graph_h1_cells == W as u32, "the commit names the graph blocks");
    let store_a = Arc::new(TileStore::open(da.path(), w.params()).unwrap());
    let feed = ChannelGraphFeed::new(store_a.clone(), a.clone()).expect("a feed over commit A");
    let b = run(&db, &w, &feed, None).expect("bake B");
    let (sa, sb) = (store_a, TileStore::open(db.path(), w.params()).unwrap());
    let mut burned = 0;
    for id in tiles() {
        let tile = layer_tile_id(&spec().grid, id).unwrap();
        let (ka, kb) = (a.tiles[&(id.face.0, id.ti, id.tj)], b.tiles[&(id.face.0, id.ti, id.tj)]);
        assert_eq!(ka, kb, "the same key: the graph is not part of an L tile's key");
        let (ra, rb) = (sa.get(KIND_LAYER_TILE, tile, ka).unwrap().unwrap(), sb.get(KIND_LAYER_TILE, tile, kb).unwrap().unwrap());
        assert_eq!(ra.payload, rb.payload, "tile ({}, {}): the stored graph is what K7 burns", id.ti, id.tj);
        // The burn counts are in the stats block (bytes 13..21 of the payload header: burned cells).
        burned += u64::from_le_bytes(ra.payload[13..21].try_into().unwrap());
    }
    assert!(burned > 0, "the channels burned some cells");
    println!("graph feed: {} cells burned over {} tiles, the same bytes through the commit", burned, tiles().len());
}

// spec: STRM-084
#[test]
fn a_commit_without_a_graph_has_no_feed_and_a_graph_in_area_units_is_refused() {
    let w = world();
    let (d, d2) = (Dir::new("graph-none"), Dir::new("graph-area"));
    let c = run(&d, &w, &Feed(true), None).unwrap();
    assert!(c.graph.is_empty() && c.graph_h1_cells == 0);
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let e = ChannelGraphFeed::new(store, c).err().unwrap();
    assert!(e.contains("holds no channel graph"), "{e}");
    let area = h1(false);
    let e = run(&d2, &w, &Feed(true), Some(&area)).unwrap_err();
    assert!(e.contains("area") && e.contains("m3/s"), "{e}");
    assert!(planet_cache::l_writer::Commit::read(d2.path()).unwrap().is_none(), "no commit after the refusal");
}

// spec: STRM-084
#[test]
fn a_graph_made_for_another_grid_than_the_bake_is_refused_naming_both() {
    let w = world();
    let h = h1(true);
    let d = Dir::new("graph-mismatch");
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let b = LBake {
        world: &w,
        res: RES,
        spec: spec(),
        basin: basin(),
        base: &base,
        regional: &zero,
        feed: &Feed(true),
        lakes: &lakes,
        h1_cells: W,
        h1_cell: &|_, _, _| None,
        face_of: &|_| Face(2),
        threads: 1,
        graph: Some(GraphInput {
            spec: GraphSpec { basin: basin(), h1_cells: 80 },
            graph: &h.graph,
            segment_ids: &h.ids,
            grid: &h.g,
            cell_xy: &xy,
        }),
    };
    let e = bake_l(d.path(), &b, &tiles()).unwrap_err();
    assert!(e.contains("80 H1 cells") && e.contains("40 H1 cells"), "{e}");
}
