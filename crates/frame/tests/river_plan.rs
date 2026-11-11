//! Tier A, REND-059 (the plan's part): the river plan. Which terrain nodes carry a river coverage, where the node's samples lie in the face's cell-index space, that the coverage is the signed distance there, that a
//! channel read through two L tiles counts once, and that a failed read names the tile.

use std::sync::{Arc, Mutex};

use planet_core::{Face, TileId};
use planet_frame::river::{node_window, plan_rivers, river_distance_grid, RiverGrid, RiverRequest, RiverSource, RANGE_CELLS};
use planet_generators::bake::k7::{Channel, QUnit, WidthLaw, DISC_RADIUS_MAX};

const LAW: WidthLaw = WidthLaw::STANDARD;
const GRID: RiverGrid = RiverGrid { cells: 256, tile: 64, cell_m: 100.0 };

fn q_for(cells: f64) -> f64 {
    let w = cells * GRID.cell_m;
    (w / LAW.a) * (w / LAW.a)
}

fn channel(points: &[(f64, f64)], q: f64) -> Channel {
    Channel { points: points.to_vec(), levels: vec![1.0; points.len()], q: vec![q; points.len()], q_unit: QUnit::Discharge }
}

/// Every L tile whose core widened by the widest trench meets a channel hands it out, as the runtime reader does.
struct Source {
    channels: Vec<Channel>,
    reads: Mutex<Vec<(u8, u32, u32)>>,
    fail: Option<(u8, u32, u32)>,
}

impl Source {
    fn new(channels: Vec<Channel>) -> Source {
        Source { channels, reads: Mutex::new(Vec::new()), fail: None }
    }
}

impl RiverSource for Source {
    fn grid(&self) -> RiverGrid {
        GRID
    }
    fn tile_channels(&self, face: Face, ti: u32, tj: u32) -> Result<Arc<Vec<Channel>>, String> {
        self.reads.lock().unwrap().push((face.0, ti, tj));
        if self.fail == Some((face.0, ti, tj)) {
            return Err(format!("river tile {}/{ti}/{tj}: no record", face.0));
        }
        let t = f64::from(GRID.tile);
        let reach = DISC_RADIUS_MAX as f64;
        let (x0, x1, y0, y1) =
            (f64::from(ti) * t - reach, f64::from(ti + 1) * t + reach, f64::from(tj) * t - reach, f64::from(tj + 1) * t + reach);
        Ok(Arc::new(
            self.channels.iter().filter(|c| c.points.iter().any(|&(x, y)| x >= x0 && x <= x1 && y >= y0 && y <= y1)).cloned().collect(),
        ))
    }
}

fn request<'a>(source: &'a dyn RiverSource, res: u32, max_step_cells: f64) -> RiverRequest<'a> {
    RiverRequest { source, law: &LAW, res, max_step_cells }
}

fn tile(face: u8, level: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(face), level, x, y).expect("a tile")
}

// spec: REND-059
#[test]
fn a_node_samples_lie_where_the_cell_centres_of_its_window_are_by_the_face_mapping() {
    // Level 3 over a 256-cell face: 32 L cells to a tile, 32 samples a tile edge: one sample a cell, on the cell corners (half a cell before the centres).
    let t = tile(2, 3, 5, 2);
    let (origin, step) = node_window(t, 32, GRID.cells);
    assert_eq!(step, 1.0);
    assert_eq!(origin, (5.0 * 32.0 - 0.5, 2.0 * 32.0 - 0.5));
    // A level 4 tile over the same face with 32 samples: two samples a cell.
    let (origin, step) = node_window(tile(2, 4, 9, 4), 32, GRID.cells);
    assert_eq!(step, 0.5);
    assert_eq!(origin, (9.0 * 16.0 - 0.5, 4.0 * 16.0 - 0.5));
    // The definition: an L cell's centre `i` is at the face coordinate `s = 2 (i + 1/2) / n - 1` (the bake's own mapping), so the sample of the node at face coordinate `s` is at `(s + 1) n / 2 - 1/2`.
    let t = tile(1, 5, 11, 20);
    let (origin, step) = node_window(t, 16, GRID.cells);
    let (s0, s1, _, _) = t.bounds();
    for k in [0u32, 5, 16] {
        let s = s0 + (s1 - s0) * f64::from(k) / 16.0;
        let x = (s + 1.0) * f64::from(GRID.cells) / 2.0 - 0.5;
        assert_eq!(origin.0 + f64::from(k) * step, x, "sample {k}: the mapping is exact");
    }
}

// spec: REND-059
#[test]
fn a_node_with_a_river_in_range_carries_the_coverage_of_its_samples_and_one_without_carries_none() {
    let river = channel(&[(40.0, 60.0), (80.0, 70.0)], q_for(5.0));
    let src = Source::new(vec![river.clone()]);
    let on = tile(2, 3, 1, 1); // cells 32..64 on each axis: the river passes through it
    let off = tile(2, 3, 6, 6); // far from it
    let plan = plan_rivers(&request(&src, 32, 2.0), &[(7, on), (9, off)]).expect("plans");
    assert_eq!(plan.nodes.len(), 1, "the node with no river within range carries none");
    let n = &plan.nodes[0];
    assert_eq!(n.node, 7, "the node index of the terrain plan");
    assert_eq!(n.size, 33, "samples a side: cells + 1");
    let (origin, step) = node_window(on, 32, GRID.cells);
    let want: Vec<u8> = river_distance_grid(std::slice::from_ref(&river), origin, step, (33, 33), &LAW, GRID.cell_m)
        .into_iter()
        .map(|d| if d.is_finite() { planet_frame::river::distance_code(d) } else { 255 })
        .collect();
    assert_eq!(n.coverage, want);
    assert!(n.coverage.iter().any(|&c| c <= 128), "some samples are water");
}

// spec: REND-059
#[test]
fn nodes_coarser_than_the_limit_carry_no_coverage_and_the_limit_is_a_parameter() {
    let src = Source::new(vec![channel(&[(100.0, 100.0), (150.0, 120.0)], q_for(8.0))]);
    // A level 1 tile is 128 L cells wide: with 32 samples that is 4 cells a sample.
    let coarse = tile(2, 1, 0, 0);
    let (_, step) = node_window(coarse, 32, GRID.cells);
    assert_eq!(step, 4.0);
    assert!(plan_rivers(&request(&src, 32, 2.0), &[(0, coarse)]).unwrap().nodes.is_empty());
    assert!(src.reads.lock().unwrap().is_empty(), "a refused node reads no tile");
    assert_eq!(plan_rivers(&request(&src, 32, 4.0), &[(0, coarse)]).unwrap().nodes.len(), 1, "with the limit at 4 cells it carries one");
}

// spec: REND-059, CON-30
#[test]
fn a_channel_that_two_l_tiles_hand_out_counts_once_and_the_same_plan_twice_is_the_same_bytes() {
    // The river lies on the border of the L tiles 0 and 1 (cell 64): both hand it out to a node over the border.
    let river = channel(&[(50.0, 40.0), (80.0, 44.0)], q_for(6.0));
    let src = Source::new(vec![river.clone()]);
    let t = tile(2, 3, 1, 1); // cells 32..64: touches L tile 0 and (within reach) tile 1
    let plan = plan_rivers(&request(&src, 32, 2.0), &[(3, t)]).unwrap();
    let reads = src.reads.lock().unwrap().clone();
    assert!(reads.len() >= 2, "the node needs more than one L tile: {reads:?}");
    let (origin, step) = node_window(t, 32, GRID.cells);
    let once = river_coverage_grid(&[river], origin, step);
    assert_eq!(plan.nodes[0].coverage, once, "the duplicates change nothing");
    // The same plan twice gives the same bytes.
    let again = plan_rivers(&request(&src, 32, 2.0), &[(3, t)]).unwrap();
    assert_eq!(again, plan);
}

fn river_coverage_grid(channels: &[Channel], origin: (f64, f64), step: f64) -> Vec<u8> {
    river_distance_grid(channels, origin, step, (33, 33), &LAW, GRID.cell_m)
        .into_iter()
        .map(|d| if d.is_finite() { planet_frame::river::distance_code(d) } else { 255 })
        .collect()
}

// spec: REND-059
#[test]
fn a_failed_read_is_an_error_naming_the_tile_and_a_node_at_a_face_edge_reads_only_tiles_of_the_face() {
    let mut src = Source::new(vec![channel(&[(40.0, 60.0), (80.0, 70.0)], q_for(5.0))]);
    src.fail = Some((2, 0, 0));
    let e = plan_rivers(&request(&src, 32, 2.0), &[(7, tile(2, 3, 1, 1))]).unwrap_err();
    assert!(e.contains("river tile 2/0/0"), "{e}");
    // A node at the corner of the face reads tiles 0 and up only: never an index outside 0..4.
    let src = Source::new(vec![channel(&[(2.0, 2.0), (20.0, 10.0)], q_for(3.0))]);
    plan_rivers(&request(&src, 32, 2.0), &[(0, tile(2, 3, 0, 0)), (1, tile(2, 3, 7, 7))]).unwrap();
    assert!(src.reads.lock().unwrap().iter().all(|&(_, ti, tj)| ti < 4 && tj < 4), "{:?}", src.reads.lock().unwrap());
    assert_eq!(RANGE_CELLS, 4.0);
}

// spec: REND-059, CON-30
#[test]
fn a_node_and_its_child_give_the_same_codes_at_the_positions_they_share() {
    // A river across the area of the level 3 node (1, 1); its level 4 child (2, 2) covers its lower-left quarter. The child's samples at even indices are the parent's samples: the codes agree bit for bit.
    let src = Source::new(vec![channel(&[(30.0, 36.0), (70.0, 52.0)], q_for(5.0))]);
    let (parent, child) = (tile(2, 3, 1, 1), tile(2, 4, 2, 2));
    let p = plan_rivers(&request(&src, 32, 2.0), &[(0, parent), (1, child)]).unwrap();
    let (pc, cc) = (&p.nodes[0].coverage, &p.nodes[1].coverage);
    let mut shared = 0;
    for l in 0..=16usize {
        for k in 0..=16usize {
            // The child's sample (2k, 2l) is at the parent's sample (k, l) of its lower-left quarter.
            assert_eq!(cc[(2 * l) * 33 + 2 * k], pc[l * 33 + k], "sample ({k}, {l})");
            shared += 1;
        }
    }
    assert_eq!(shared, 289);
}

// spec: REND-059
#[test]
fn two_rivers_that_share_a_start_and_a_length_are_both_drawn() {
    // A distributary: two pieces start at the same point with the same discharge and the same number of points, and part ways.
    let a = channel(&[(40.0, 40.0), (60.0, 40.0)], q_for(3.0));
    let b = channel(&[(40.0, 40.0), (40.0, 62.0)], q_for(3.0));
    let src = Source::new(vec![a.clone(), b.clone()]);
    let t = tile(2, 3, 1, 1);
    let both = plan_rivers(&request(&src, 32, 2.0), &[(0, t)]).unwrap();
    let (origin, step) = node_window(t, 32, GRID.cells);
    let want = river_coverage_grid(&[a.clone(), b.clone()], origin, step);
    assert_eq!(both.nodes[0].coverage, want, "the second piece is not dropped as a duplicate of the first");
}

// spec: REND-059
#[test]
fn a_plan_with_nothing_to_sample_or_a_bad_grid_is_an_error_not_a_panic() {
    let src = Source::new(Vec::new());
    for (res, limit) in [(0u32, 2.0), (32, f64::NAN), (32, f64::INFINITY), (32, 0.0)] {
        assert!(plan_rivers(&request(&src, res, limit), &[(0, tile(2, 3, 1, 1))]).is_err(), "res {res}, limit {limit}");
    }
}

// spec: REND-059
#[test]
#[should_panic(expected = "points of a channel are finite")]
fn a_channel_point_that_is_not_finite_is_refused_with_a_message() {
    let ch = channel(&[(f64::NEG_INFINITY, 5.0), (5.0, 5.0)], 1.0);
    river_distance_grid(&[ch], (0.0, 0.0), 1.0, (8, 8), &LAW, GRID.cell_m);
}
