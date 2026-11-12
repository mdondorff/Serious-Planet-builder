//! Tier A, TEST-123 and STRM-086/REND-062: a flight over a small one-face world that holds a channel graph. No real store has a graph yet, so the world is
//! baked here (the hydrology of a 32 x 32 H1 grid, its graph, the L tiles of one face beside it); the river source over the reader of the commit is asked
//! for the river plan of the nodes along the course of its longest channel, from a high level to a fine one. The flight's report is the reader's cache:
//! the bytes it holds at its peak against its bound, and the decodes. Nothing here needs a GPU.

use std::sync::Arc;

use planet::rivers::{plan_nodes_rivers, ReaderRiverSource};
use planet_cache::basin_table::BasinSpec;
use planet_cache::global_store::GlobalWorld;
use planet_cache::graph_store::GraphSpec;
use planet_cache::l_writer::{bake_l, ChannelFeed, GraphInput, LBake};
use planet_cache::layer_l::LTileSpec;
use planet_cache::pack::TileStore;
use planet_cache::river_read::RiverReader;
use planet_core::{Face, TileId, Vec3};
use planet_generators::bake::channel_graph::{channels_from_graph, ChannelGraph};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::k7::{Channel, K7Params};
use planet_generators::bake::lakes::Lakes;
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::tile::{TileGrid, TileId as PassTile};
use planet_generators::bake::Grid;
use planet_world_def::WorldDefinition;

const W: usize = 32;
const R: f64 = 6_371_000.0;
const RES: u32 = 64;
/// The node's cells an edge in the flight (the terrain's own `cells`).
const NODE_CELLS: u32 = 32;
/// A bound a little under the flight's unbounded peak (about 743 kB), so that the reader's eviction runs and the assertion can fail.
const CACHE_BOUND: usize = 400_000;

struct Memory(Vec<Channel>);

impl ChannelFeed for Memory {
    fn q_is_discharge(&self) -> bool {
        true
    }
    fn channels_near(&self, _: Face, _: i64, _: i64, _: i64) -> Vec<Channel> {
        self.0.clone()
    }
}

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base(d: Vec3) -> f64 {
    1500.0 + 100.0 * d.z
}

fn tile_at(face: Face, level: u8, s: f64, t: f64) -> TileId {
    let n = f64::from(1u32 << level);
    let ix = (libm::floor((s + 1.0) / 2.0 * n) as i64).clamp(0, (1i64 << level) - 1);
    let iy = (libm::floor((t + 1.0) / 2.0 * n) as i64).clamp(0, (1i64 << level) - 1);
    TileId::new(face, level, ix as u32, iy as u32).expect("a tile")
}

// spec: TEST-123, STRM-086, REND-062
#[test]
fn a_flight_over_a_one_face_world_with_a_channel_graph_plans_its_rivers_through_the_reader_within_the_cache_bound() {
    let world = GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 3)).unwrap().with_block_limit(4);
    let g = Grid::new(W, W, 800.0);
    let z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64 + 3.0 * ((p / W) as f64 - 16.0).abs()).collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, 1e-3, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, 1e-3, true);
    let runoff = vec![1.0e-8f32; W * W];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, Some(&runoff[..]), None, &p);
    let graph = ChannelGraph::from_hydro(&g, &routing, &hydro, &z);
    let ids: Vec<u64> = hydro.segments.iter().map(|s| s.id).collect();
    let to_local = |c: u32| (8.0 * f64::from(c % W as u32) + 3.5, 8.0 * f64::from(c / W as u32) + 3.5);
    let channels = channels_from_graph(&graph, &g, &to_local);
    assert!(channels.len() > 100, "the fixture has a river network ({} channels)", channels.len());

    let dir = Scratch(std::env::temp_dir().join(format!("planet-river-flight-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let basin = BasinSpec { hydro: HydroParams::default(), tile_cells: 64, tiles_per_edge: 4, base_id: 7, inputs_id: 11 };
    let spec = LTileSpec {
        grid: TileGrid { cells: 256, tile: 64, halo: 24, radius_m: R },
        k7: K7Params::standard(5, 0.0),
        base_id: 7,
        inputs_id: 11,
    };
    let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
    let xy = |c: u32| (2u8, i64::from(c) % W as i64, i64::from(c) / W as i64);
    let feed = Memory(channels.clone());
    let zero = |_: Vec3| 0.0;
    let b = LBake {
        world: &world,
        res: RES,
        spec,
        basin,
        base: &base,
        regional: &zero,
        feed: &feed,
        lakes: &lakes,
        h1_cells: W,
        h1_cell: &|_, _, _| None,
        face_of: &|_| Face(2),
        threads: 2,
        graph: Some(GraphInput {
            spec: GraphSpec { basin, h1_cells: W as u32, margin_h1: 4 },
            graph: &graph,
            segment_ids: &ids,
            grid: &g,
            cell_xy: &xy,
        }),
    };
    let tiles: Vec<PassTile> = (0..4u32).flat_map(|tj| (0..4u32).map(move |ti| PassTile { face: Face(2), ti, tj })).collect();
    let commit = bake_l(&dir.0, &b, &tiles).expect("the bake");
    assert!(!commit.graph.is_empty(), "the commit holds the graph");

    let store = Arc::new(TileStore::open(&dir.0, world.params()).unwrap());
    let reader = RiverReader::new(store, commit, CACHE_BOUND).expect("the reader of the commit");
    let src = ReaderRiverSource::new(Arc::new(reader));
    let cells = f64::from(src.reader().grid_dims().0);

    // The course: the points of the longest channel, as face coordinates.
    let course = channels.iter().max_by_key(|c| c.points.len()).expect("a channel");
    assert!(course.points.len() > 3);
    let to_face = |p: f64| 2.0 * (p + 0.5) / cells - 1.0;

    let (mut peak, mut with_river, mut asked, mut drawn_px_levels) = (0usize, 0usize, 0usize, std::collections::BTreeSet::new());
    for pass in 0..2 {
        for &(px, py) in &course.points {
            let (s, t) = (to_face(px), to_face(py));
            // The flight descends over the point: a node ring of 3 x 3 tiles at each level from 6 down to 14.
            for level in 6u8..=14 {
                let centre = tile_at(Face(2), level, s, t);
                let mut nodes = Vec::new();
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (x, y) = (i64::from(centre.x()) + dx, i64::from(centre.y()) + dy);
                        if (0..(1i64 << level)).contains(&x) && (0..(1i64 << level)).contains(&y) {
                            let id = TileId::new(Face(2), level, x as u32, y as u32).unwrap();
                            nodes.push((nodes.len() as u32, id, f64::from(level) * 100.0 + (dx.abs() + dy.abs()) as f64));
                        }
                    }
                }
                let plan = plan_nodes_rivers(&src, NODE_CELLS, &nodes).expect("the river plan");
                asked += nodes.len();
                with_river += plan.nodes.len();
                if !plan.nodes.is_empty() {
                    drawn_px_levels.insert(level);
                }
                peak = peak.max(src.reader().bytes());
            }
        }
        if pass == 0 {
            // The second pass over the same course asks for tiles the cache holds (the decodes of the first pass are the whole cost).
            let first = src.reader().decodes();
            assert!(first > 0, "the first pass decoded river tiles");
        }
    }
    let decodes = src.reader().decodes();
    println!(
        "river flight: {asked} nodes asked, {with_river} carry a river (levels {:?}); reader cache peak {} B of {} B, {decodes} decodes",
        drawn_px_levels, peak, CACHE_BOUND
    );
    // A negative control: the position farthest from every channel point (on a 32 x 32 sample grid) at the finest level carries no river.
    let far = (0..32)
        .flat_map(|i| (0..32).map(move |j| (8.0 * f64::from(i) + 4.0, 8.0 * f64::from(j) + 4.0)))
        .max_by(|a, b| {
            let d = |q: &(f64, f64)| {
                channels.iter().flat_map(|c| c.points.iter()).map(|p| libm::hypot(p.0 - q.0, p.1 - q.1)).fold(f64::MAX, f64::min)
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let control =
        plan_nodes_rivers(&src, NODE_CELLS, &[(0, tile_at(Face(2), 14, to_face(far.0), to_face(far.1)), 0.0)]).expect("the control plan");
    assert!(control.nodes.is_empty(), "a node at {far:?}, far from every channel, carries no river");
    assert!(with_river > 0, "some node of the flight carries a river");
    assert!(peak <= CACHE_BOUND, "the reader holds {peak} B, its bound is {CACHE_BOUND} B");
}

// spec: TEST-123, STRM-086, REND-062
/// The flight of TEST-123 over a REAL stored region: a commit folder baked by `cargo run --release -p planet-cache --example bake_region` (the G-RT fixture), named by `PLANET_RIVER_STORE`. Skips with a message when the variable
/// is unset or the folder holds no commit. Reports the reader's cache peak (against its 64 MiB default bound), the decodes and the time of the flight; light CPU (run it at BelowNormal).
#[test]
#[ignore = "needs PLANET_RIVER_STORE=<a commit folder baked by the bake_region example>"]
fn a_flight_over_a_stored_region_with_a_channel_graph_reports_the_readers_cache_peak() {
    use planet_cache::l_writer::Commit;
    use planet_cache::river_read::DEFAULT_CACHE_BYTES;
    let Ok(dir) = std::env::var("PLANET_RIVER_STORE") else {
        println!("skipped: PLANET_RIVER_STORE names no commit folder (the G-RT fixture: `bake_region`, output target/stores/g-rt-fixture)");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let Some(commit) = Commit::read(&dir).expect("a readable commit file") else {
        println!("skipped: {} holds no commit", dir.display());
        return;
    };
    assert!(!commit.graph.is_empty(), "the commit holds a channel graph");
    let world = GlobalWorld::from_definition(&WorldDefinition::single_planet(commit.spec.radius_m, commit.spec.seed))
        .expect("the world of the commit");
    let face = Face(commit.tiles.keys().next().expect("a tile").0);
    let store = Arc::new(TileStore::open(&dir, world.params()).unwrap());
    let reader = RiverReader::new(store, commit.clone(), DEFAULT_CACHE_BYTES).expect("the reader of the commit");
    let src = ReaderRiverSource::new(Arc::new(reader));
    let cells = f64::from(src.reader().grid_dims().0);
    let to_face = |p: f64| 2.0 * (p + 0.5) / cells - 1.0;

    // The course: the points of the longest channels of the commit's tiles, every 24th point.
    let t0 = std::time::Instant::now();
    let mut chans: Vec<Arc<Vec<planet_generators::bake::k7::Channel>>> = Vec::new();
    for &(f, ti, tj) in commit.tiles.keys() {
        chans.push(src.reader().tile_channels(Face(f), ti, tj).expect("a tile's channels"));
    }
    let mut longest: Vec<&planet_generators::bake::k7::Channel> = chans.iter().flat_map(|c| c.iter()).collect();
    longest.sort_by_key(|c| std::cmp::Reverse(c.points.len()));
    longest.truncate(4);
    assert!(!longest.is_empty(), "the store has channels");
    let course: Vec<(f64, f64)> = longest.iter().flat_map(|c| c.points.iter().step_by(24).copied()).collect();
    let (mut peak, mut asked, mut with_river) = (src.reader().bytes(), 0usize, 0usize);
    for &(px, py) in &course {
        let (s, t) = (to_face(px), to_face(py));
        for level in 6u8..=16 {
            let centre = tile_at(face, level, s, t);
            let mut nodes = Vec::new();
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (x, y) = (i64::from(centre.x()) + dx, i64::from(centre.y()) + dy);
                    if (0..(1i64 << level)).contains(&x) && (0..(1i64 << level)).contains(&y) {
                        let id = TileId::new(face, level, x as u32, y as u32).unwrap();
                        nodes.push((nodes.len() as u32, id, f64::from(level) * 100.0 + (dx.abs() + dy.abs()) as f64));
                    }
                }
            }
            let plan = plan_nodes_rivers(&src, NODE_CELLS, &nodes).expect("the river plan");
            asked += nodes.len();
            with_river += plan.nodes.len();
            peak = peak.max(src.reader().bytes());
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    println!(
        "river flight over {}: {} course points, {asked} nodes asked, {with_river} carry a river; reader cache peak {peak} B of {DEFAULT_CACHE_BYTES} B, {} decodes; {secs:.1} s (the read of every tile and the flight; not a measurement)",
        dir.display(),
        course.len(),
        src.reader().decodes()
    );
    assert!(with_river > 0, "some node of the flight carries a river");
    assert!(peak <= DEFAULT_CACHE_BYTES, "the reader holds {peak} B, its bound is {DEFAULT_CACHE_BYTES} B");
}
