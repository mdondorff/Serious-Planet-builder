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

const W: usize = 32;
const R: f64 = 6_371_000.0;

/// A ground well above the beds of the H1 channels (500-900 m), so that the burn-in lowers cells.
fn base(d: planet_core::Vec3) -> f64 {
    1500.0 + 100.0 * d.z
}

fn world() -> GlobalWorld {
    crate::l_writer::world()
}

fn spec() -> LTileSpec {
    LTileSpec { grid: TileGrid { cells: 256, tile: 64, halo: 24, radius_m: R }, k7: K7Params::standard(5, 0.0), base_id: 7, inputs_id: 11 }
}

fn basin() -> BasinSpec {
    BasinSpec { hydro: HydroParams::default(), tile_cells: 64, tiles_per_edge: 4, base_id: 7, inputs_id: 11 }
}

struct H1 {
    g: Grid,
    graph: ChannelGraph,
    ids: Vec<u64>,
}

fn h1(runoff: bool) -> H1 {
    let g = Grid::new(W, W, 800.0);
    let z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64 + 3.0 * ((p / W) as f64 - 16.0).abs()).collect();
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
    (0..4u32).flat_map(|tj| (0..4u32).map(move |ti| PassTile { face: Face(2), ti, tj })).collect()
}

fn run(
    dir: &Dir,
    w: &GlobalWorld,
    feed: &dyn planet_cache::l_writer::ChannelFeed,
    with_graph: Option<&H1>,
) -> Result<planet_cache::l_writer::Commit, String> {
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let gi = with_graph.map(|h| GraphInput {
        spec: GraphSpec { basin: basin(), h1_cells: W as u32, margin_h1: 4 },
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
            spec: GraphSpec { basin: basin(), h1_cells: 80, margin_h1: 4 },
            graph: &h.graph,
            segment_ids: &h.ids,
            grid: &h.g,
            cell_xy: &xy,
        }),
    };
    let e = bake_l(d.path(), &b, &tiles()).unwrap_err();
    assert!(e.contains("80 H1 cells") && e.contains("32 H1 cells"), "{e}");
}

// spec: STRM-084, GEN-169
#[test]
fn a_graph_whose_margin_is_smaller_than_the_tile_halo_needs_is_refused_naming_both_numbers() {
    let w = world();
    let h = h1(true);
    let d = Dir::new("graph-margin");
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
            spec: GraphSpec { basin: basin(), h1_cells: W as u32, margin_h1: 3 },
            graph: &h.graph,
            segment_ids: &h.ids,
            grid: &h.g,
            cell_xy: &xy,
        }),
    };
    let e = bake_l(d.path(), &b, &tiles()).unwrap_err();
    assert!(e.contains("reaches 3 H1 cells") && e.contains("halo of 24") && e.contains("needs 4"), "{e}");
}

fn baked(name: &str, regional: &(dyn Fn(planet_core::Vec3) -> f64 + Sync)) -> (Dir, planet_cache::l_writer::Commit, H1) {
    let w = world();
    let h = h1(true);
    let to_local = |c: u32| (8.0 * f64::from(c % W as u32) + 3.5, 8.0 * f64::from(c / W as u32) + 3.5);
    let memory = MemoryFeed { face: Face(2), channels: channels_from_graph(&h.graph, &h.g, &to_local), discharge: true };
    let d = Dir::new(name);
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let b = LBake {
        world: &w,
        res: RES,
        spec: spec(),
        basin: basin(),
        base: &base,
        regional,
        feed: &memory,
        lakes: &lakes,
        h1_cells: W,
        h1_cell: &|_, _, _| None,
        face_of: &|_| Face(2),
        threads: 2,
        graph: Some(GraphInput {
            spec: GraphSpec { basin: basin(), h1_cells: W as u32, margin_h1: 4 },
            graph: &h.graph,
            segment_ids: &h.ids,
            grid: &h.g,
            cell_xy: &xy,
        }),
    };
    let c = bake_l(d.path(), &b, &tiles()).expect("bake");
    (d, c, h)
}

// spec: STRM-085
#[test]
fn the_river_reader_gives_the_tile_its_channels_under_the_commit_of_the_layer_and_keeps_a_bounded_cache() {
    use planet_cache::l_layer::CommitLayer;
    use planet_cache::river_read::RiverReader;
    use planet_generators::erosion_layer::LayerSource;
    let w = world();
    let (d, c, _h) = baked("river-read", &zero);
    let layer = CommitLayer::open(d.path(), Arc::new(world()), 4).unwrap();
    // The size of all 16 tiles decoded, to set a bound of a quarter of it.
    let all = layer.rivers(planet_cache::river_read::DEFAULT_CACHE_BYTES).unwrap();
    for tj in 0..4u32 {
        for ti in 0..4u32 {
            all.tile_channels(Face(2), ti, tj).unwrap();
        }
    }
    assert_eq!(all.len(), 16, "the default bound holds the whole fixture");
    let bound = all.bytes() / 4;
    let rivers = layer.rivers(bound).expect("a reader over the commit of the layer");
    assert_eq!(rivers.commit_id(), layer.id(), "the rivers, the tiles and the tables are one commit");
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let feed = ChannelGraphFeed::new(store.clone(), c.clone()).unwrap();
    let mut seen = 0;
    for tj in 0..4u32 {
        for ti in 0..4u32 {
            let got = rivers.tile_channels(Face(2), ti, tj).expect("reads");
            let want = feed.try_channels_near(Face(2), i64::from(ti) * 64 - 16, i64::from(tj) * 64 - 16, 64 + 32).unwrap();
            assert_eq!(*got, want, "tile ({ti}, {tj}): the reader returns the feed's channels of the tile widened by the widest trench");
            seen += got.len();
            assert!(rivers.bytes() <= bound || rivers.len() == 1, "the cache holds at most its byte bound ({} of {bound})", rivers.bytes());
        }
    }
    assert!(seen > 100, "the fixture has rivers in its tiles");
    assert_eq!(rivers.decodes(), 16);
    assert!(rivers.len() < 16, "a quarter of the bytes cannot hold every tile");
    rivers.tile_channels(Face(2), 3, 3).unwrap();
    assert_eq!(rivers.decodes(), 16, "the newest tile is a hit");
    rivers.tile_channels(Face(2), 0, 0).unwrap();
    assert_eq!(rivers.decodes(), 17, "an evicted tile is decoded again, to the same channels");
    // A commit without a graph has no reader.
    let dn = Dir::new("river-none");
    let cn = run(&dn, &w, &Feed(true), None).unwrap();
    let e = RiverReader::new(Arc::new(TileStore::open(dn.path(), w.params()).unwrap()), cn, 1 << 20).err().unwrap();
    assert!(e.contains("holds no channel graph"), "{e}");
}

// spec: STRM-085
#[test]
fn a_graph_block_the_commit_names_and_the_store_lacks_is_an_error_naming_the_tile_not_a_stop() {
    use planet_cache::river_read::RiverReader;
    let w = world();
    let (d, mut c, _h) = baked("river-missing", &zero);
    let key = c.graph.get_mut(&(2, 1, 1)).expect("a block of the commit");
    key.0[0] ^= 1;
    let r = RiverReader::new(Arc::new(TileStore::open(d.path(), w.params()).unwrap()), c, 1 << 20).unwrap();
    let e = r.tile_channels(Face(2), 1, 1).unwrap_err();
    assert!(e.contains("river tile 2/1/1") && e.contains("no record"), "{e}");
    assert!(r.tile_channels(Face(2), 3, 3).is_ok(), "a tile away from the missing block still reads");
}

/// A ground that falls steeply along the flow of the fixture channels (they run toward low `x`; 3 m a metre of `x`), so that the pits stay within the carry of the profile and no cutoff binds: the shallow fixture of G-PROFILE.
fn shallow_base(d: planet_core::Vec3) -> f64 {
    700.0 - 3000.0 * d.x
}

fn regional_c(d: planet_core::Vec3) -> f64 {
    8.0 * d.x - 5.0 * d.y
}

fn bake_with(
    name: &str,
    ground: &(dyn Fn(planet_core::Vec3) -> f64 + Sync),
    regional: &(dyn Fn(planet_core::Vec3) -> f64 + Sync),
) -> (Dir, planet_cache::l_writer::Commit) {
    let w = world();
    let h = h1(true);
    let to_local = |c: u32| (8.0 * f64::from(c % W as u32) + 3.5, 8.0 * f64::from(c / W as u32) + 3.5);
    let memory = MemoryFeed { face: Face(2), channels: channels_from_graph(&h.graph, &h.g, &to_local), discharge: true };
    let d = Dir::new(name);
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let b = LBake {
        world: &w,
        res: RES,
        spec: spec(),
        basin: basin(),
        base: ground,
        regional,
        feed: &memory,
        lakes: &lakes,
        h1_cells: W,
        h1_cell: &|_, _, _| None,
        face_of: &|_| Face(2),
        threads: 2,
        graph: Some(GraphInput {
            spec: GraphSpec { basin: basin(), h1_cells: W as u32, margin_h1: 4 },
            graph: &h.graph,
            segment_ids: &h.ids,
            grid: &h.g,
            cell_xy: &xy,
        }),
    };
    let c = bake_l(d.path(), &b, &tiles()).expect("bake");
    (d, c)
}

/// The rises along the stored channels of a tile core in K7 own heights: the line steps with both cells in the core, the number over `step` and the largest, with the final pass report.
fn k7_rises(
    feed: &ChannelGraphFeed,
    ground: &(dyn Fn(planet_core::Vec3) -> f64 + Sync),
    regional: &(dyn Fn(planet_core::Vec3) -> f64 + Sync),
    id: PassTile,
    step: f64,
) -> (usize, usize, f64, planet_generators::bake::k7::FinalPass, usize, f64) {
    use planet_generators::bake::k7::{k7_tile, line_cells, upstream_first};
    let g = spec().grid;
    let (win, h, t) = (g.window() as i64, g.halo as i64, g.tile as i64);
    let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let chans = upstream_first(feed.try_channels_near(Face(2), i0, j0, win).unwrap());
    let r = k7_tile(&g, id, ground, regional, &chans, &spec().k7, 1);
    let at = |x: i64, y: i64| r.z[((y - j0) * win + (x - i0)) as usize];
    let core = |x: i64, y: i64| {
        x >= i64::from(id.ti) * t && y >= i64::from(id.tj) * t && x < i64::from(id.ti + 1) * t && y < i64::from(id.tj + 1) * t
    };
    let (mut n, mut over, mut worst) = (0, 0, 0.0f64);
    for ch in &chans {
        for p in line_cells(ch).windows(2) {
            let ((a, b), (c, e)) = (p[0], p[1]);
            if core(a, b) && core(c, e) {
                n += 1;
                let rise = at(c, e) - at(a, b);
                over += usize::from(rise > step);
                worst = worst.max(rise);
            }
        }
    }
    (n, over, worst, r.final_pass, r.burned, r.burn_cutoff_diff.max)
}

/// A lattice of deep holes along the valley of the H1 channels (GEN-176): 1,200 m deep, 5 cells wide every 20 cells of the face, over the rows the channels run in. A hole is lower than the beds
/// (500-900 m), so the burn carries it down the line only `carry` steps: the cells farther downstream stand higher than the hole, a genuine cutoff rise that the final pass cannot remove.
fn deep_holes(d: planet_core::Vec3) -> f64 {
    let face = Face(2);
    let n = 256.0;
    let w = d.dot(face.normal());
    let plane = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
    let (s, t) = (plane(d.dot(face.u_axis()) / w), plane(d.dot(face.v_axis()) / w));
    let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
    if (112.0..152.0).contains(&y) && x >= 0.0 && libm::fmod(x, 20.0) < 5.0 {
        -1200.0
    } else {
        0.0
    }
}

// spec: TEST-116, GEN-169
#[test]
fn g_profile_on_a_fixture_whose_burn_stays_within_the_carry_has_no_cutoff_and_no_rise_over_a_step() {
    use planet_cache::l_layer::CommitLayer;
    use planet_generators::bake::k7::line_cells;
    use planet_generators::erosion_layer::LayerSource;
    let step = 0.25;
    let g = spec().grid;
    let t = g.tile as i64;
    for (name, regional) in [("profile-off", &zero as &(dyn Fn(planet_core::Vec3) -> f64 + Sync)), ("profile-on", &regional_c)] {
        let (d, c) = bake_with(name, &shallow_base, regional);
        let w = world();
        let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
        let feed = ChannelGraphFeed::new(store, c).unwrap();
        // K7 own heights: the profile holds in every tile core and the cutoff never binds.
        // Non-vacuity (GEN-176, architect 2026-11-12): the burn now holds the profile by itself (the channel cells are fixed after it), so the final pass has no work on this fixture and "the pass works
        // in more than 4 tiles" no longer holds; the fixture must still have channel steps, burned channels in more than 4 tiles and line steps across a tile border. The deep fixture below makes the pass work.
        let (mut steps, mut binds, mut burned_tiles) = (0, 0, 0);
        for id in tiles() {
            let (n, over, worst, fp, burned, burn_diff) = k7_rises(&feed, &shallow_base, regional, id, step);
            assert!(burn_diff <= step, "{name}: tile ({}, {}): the burn's cutoff changes a height by {burn_diff} m", id.ti, id.tj);
            assert_eq!(over, 0, "{name}: tile ({}, {}) has {over} rises over {step} m (largest {worst} m)", id.ti, id.tj);
            // GATED (architect, 2026-11-12): wherever a cutoff binds, the bounded target is within the store's 0.25 m step of the unbounded one.
            assert!(
                fp.cutoff_diff.max <= step,
                "{name}: tile ({}, {}): a cutoff changes a height by {} m",
                id.ti,
                id.tj,
                fp.cutoff_diff.max
            );
            steps += n;
            binds += fp.cutoff_binds;
            burned_tiles += usize::from(burned > 0);
        }
        assert!(
            steps > 1000 && burned_tiles > 4,
            "{name}: the fixture has channel steps ({steps}) and burned channels in {burned_tiles} tiles"
        );
        assert_eq!(binds, 0, "{name}: the carry holds the profile everywhere: no cutoff binds");
        // Through the stored read (B + L): the same along every stored piece, tile borders included.
        let layer = CommitLayer::open(d.path(), Arc::new(world()), 16).unwrap();
        let rivers = layer.rivers(planet_cache::river_read::DEFAULT_CACHE_BYTES).unwrap();
        let mut bs = std::collections::HashMap::new();
        for id in tiles() {
            bs.insert((id.ti, id.tj), planet_generators::bake::k7::k7_tile(&g, id, &shallow_base, regional, &[], &spec().k7, 1).b);
        }
        let surf = |i: i64, j: i64| {
            let (ti, tj) = ((i / t) as u32, (j / t) as u32);
            let tile = layer.tile(Face(2), ti, tj).unwrap().expect("a tile of the commit");
            let (li, lj) = ((i % t) as usize, (j % t) as usize);
            bs[&(ti, tj)][(lj + g.halo) * g.window() + li + g.halo] + f64::from(tile.cell(li, lj))
        };
        let (mut cells, mut worst, mut border, mut border_steps) = (0usize, f64::NEG_INFINITY, f64::NEG_INFINITY, 0usize);
        for tj in 0..4u32 {
            for ti in 0..4u32 {
                for ch in rivers.tile_channels(Face(2), ti, tj).unwrap().iter() {
                    for pair in line_cells(ch).windows(2) {
                        let ((a, b), (c, e)) = (pair[0], pair[1]);
                        if [a, b, c, e].iter().any(|&v| !(0..256).contains(&v)) {
                            continue;
                        }
                        let rise = surf(c, e) - surf(a, b);
                        cells += 1;
                        worst = worst.max(rise);
                        if a / t != c / t || b / t != e / t {
                            border = border.max(rise);
                            border_steps += 1;
                        }
                    }
                }
            }
        }
        println!("G-PROFILE {name}: {cells} stored steps, largest rise {worst:.4} m, across a tile border {border:.4} m");
        assert!(cells > 1000 && border_steps > 0, "{name}: {cells} stored steps, {border_steps} across a tile border");
        assert!(worst <= step, "{name}: the stored surface rises {worst} m downstream along a piece (limit {step} m)");
        assert!(border <= step, "{name}: the step across a tile border is {border} m (limit {step} m)");
    }
}

// spec: TEST-116, GEN-169
#[test]
fn on_the_deep_fixture_every_rise_over_a_step_is_reported_by_the_cutoff_count() {
    let w = world();
    // Restated (GEN-176, architect 2026-11-12): the rises this fixture used to show came from the defects of K7 (a sub-sea trench under a land cell, trenches filled after the burn), not from the carry; with those fixed the
    // plain deep fixture has none. "Deep" now means a hole farther upstream than the carry (`deep_holes`), so a genuine carry cutoff and rise exist, the per-tile asserts below stay, and the final pass has work.
    let (d, c, _h) = baked("profile-deep", &deep_holes);
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let feed = ChannelGraphFeed::new(store, c).unwrap();
    let (mut rises, mut binds) = (0usize, 0usize);
    for id in tiles() {
        let (_, over, _, fp, _, _) = k7_rises(&feed, &base, &deep_holes, id, 0.25);
        // A rise exists only where the pass reports a cutoff, and the report counts at least every rise.
        assert!(over == 0 || fp.cutoff_binds > 0, "tile ({}, {}): {over} rises over 0.25 m and no cutoff reported", id.ti, id.tj);
        assert!(over <= fp.cutoff_binds, "tile ({}, {}): {over} rises over 0.25 m but {} cutoff cells", id.ti, id.tj, fp.cutoff_binds);
        rises += over;
        binds += fp.cutoff_binds;
    }
    assert!(rises > 0, "the deep fixture does show the cutoff rises (else it is not the deep case)");
    println!("deep fixture: {rises} rises over 0.25 m, {binds} cutoff cells reported");
}
