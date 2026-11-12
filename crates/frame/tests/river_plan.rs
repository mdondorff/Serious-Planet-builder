//! Tier A, REND-059 (the plan's part): the river plan. Which terrain nodes carry a river coverage, where the node's samples lie in the face's cell-index space, that the coverage is the signed distance there, that a
//! channel read through two L tiles counts once, and that a failed read names the tile.

use std::sync::{Arc, Mutex};

use planet_core::{Face, TileId};
use planet_frame::river::{
    node_window, plan_rivers, river_distance_grid, RiverGrid, RiverRequest, RiverSource, FULL_RIVER_SAMPLES, MIN_RIVER_SAMPLES, RANGE_CELLS,
};
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

fn request<'a>(source: &'a dyn RiverSource, res: u32) -> RiverRequest<'a> {
    RiverRequest { source, law: &LAW, res, min_samples: MIN_RIVER_SAMPLES, full_samples: FULL_RIVER_SAMPLES, max_layers: 256 }
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
    let plan = plan_rivers(&request(&src, 32), &[(7, on, 0.0), (9, off, 0.0)]).expect("plans");
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
fn a_node_carries_a_river_from_half_a_sample_wide_and_fades_linearly_to_one_sample() {
    // A river 8 cells wide. A level 1 tile is 128 L cells over 32 samples: 4 cells a sample (two samples wide), level 0 is 8 cells a sample (one sample wide): both at full strength.
    let src = Source::new(vec![channel(&[(60.0, 100.0), (200.0, 120.0)], q_for(8.0))]);
    let at = |src: &Source, level: u8, x: u32, y: u32| {
        let t = tile(2, level, x, y);
        let (_, step) = node_window(t, 32, GRID.cells);
        (plan_rivers(&request(src, 32), &[(0, t, 0.0)]).unwrap(), step)
    };
    let (two, step) = at(&src, 1, 0, 0);
    assert_eq!(step, 4.0);
    assert_eq!((two.nodes.len(), two.nodes[0].fade), (1, 255), "two samples wide: full strength");
    let (one, step) = at(&src, 0, 0, 0);
    assert_eq!(step, 8.0);
    assert_eq!((one.nodes.len(), one.nodes[0].fade), (1, 255), "one sample wide: full strength");
    // Between half a sample and one: linear, 5 cells at 8 cells a sample is 0.625 samples: a quarter of the way (63.75 rounds to 64).
    let src5 = Source::new(vec![channel(&[(60.0, 100.0), (200.0, 120.0)], q_for(5.0))]);
    assert_eq!(at(&src5, 0, 0, 0).0.nodes[0].fade, 64, "0.625 samples wide: a quarter");
    // Half a sample or less: not drawn.
    let src4 = Source::new(vec![channel(&[(60.0, 100.0), (200.0, 120.0)], q_for(4.0))]);
    assert!(at(&src4, 0, 0, 0).0.nodes.is_empty(), "exactly half a sample wide: nothing");
    let src2 = Source::new(vec![channel(&[(60.0, 100.0), (200.0, 120.0)], q_for(2.0))]);
    assert!(at(&src2, 0, 0, 0).0.nodes.is_empty(), "a quarter of a sample wide: nothing");
    // A node whose samples are so far apart that even the widest river (32 cells) is under half a sample reads no tile.
    struct Big(Source);
    impl RiverSource for Big {
        fn grid(&self) -> RiverGrid {
            RiverGrid { cells: 16 * GRID.cells, tile: GRID.tile, cell_m: GRID.cell_m }
        }
        fn tile_channels(&self, f: Face, i: u32, j: u32) -> Result<Arc<Vec<Channel>>, String> {
            self.0.tile_channels(f, i, j)
        }
    }
    let big = Big(Source::new(vec![channel(&[(60.0, 100.0), (200.0, 120.0)], q_for(30.0))]));
    let (_, wide_step) = node_window(tile(2, 0, 0, 0), 32, 16 * GRID.cells);
    assert!(wide_step > 64.0, "{wide_step}");
    assert!(plan_rivers(&request(&big, 32), &[(0, tile(2, 0, 0, 0), 0.0)]).unwrap().nodes.is_empty());
    assert!(big.0.reads.lock().unwrap().is_empty(), "no tile is read for a node that cannot carry the widest river");
}

// spec: REND-059
#[test]
fn a_river_plan_with_a_strength_other_than_full_round_trips_through_its_text() {
    let plan = planet_frame::river::RiverPlan {
        nodes: vec![planet_frame::river::RiverNode { node: 4, size: 2, coverage: vec![0, 128, 255, 7], fade: 64 }],
        dropped: 2,
    };
    let text = plan.to_text();
    assert_eq!(
        text,
        "rivers 2
rnode 4 64 0080ff07"
    );
    let lines: Vec<Vec<&str>> = text.lines().map(|l| l.split_whitespace().collect()).collect();
    assert_eq!(planet_frame::river::RiverPlan::from_lines(&lines).unwrap(), plan);
}

// spec: REND-059
#[test]
fn a_channel_with_a_discharge_that_is_not_finite_is_an_error_naming_the_tile_at_every_distance() {
    for (level, x, y) in [(3u8, 1u32, 1u32), (0, 0, 0)] {
        for bad in [f64::NAN, f64::INFINITY] {
            let mut c = channel(&[(40.0, 40.0), (60.0, 40.0)], q_for(3.0));
            c.q[1] = bad;
            let src = Source::new(vec![c]);
            let e = plan_rivers(&request(&src, 32), &[(0, tile(2, level, x, y), 0.0)]).err();
            // A node too coarse for any river reads nothing, so it has nothing to refuse; the others name the tile.
            let (_, step) = node_window(tile(2, level, x, y), 32, GRID.cells);
            if step <= 64.0 {
                assert!(e.is_some_and(|e| e.contains("river tile 2/") && e.contains("not finite")), "level {level}, q {bad}");
            }
        }
    }
}

// spec: REND-059, CON-30
#[test]
fn a_channel_that_two_l_tiles_hand_out_counts_once_and_the_same_plan_twice_is_the_same_bytes() {
    // The river lies on the border of the L tiles 0 and 1 (cell 64): both hand it out to a node over the border.
    let river = channel(&[(50.0, 40.0), (80.0, 44.0)], q_for(6.0));
    let src = Source::new(vec![river.clone()]);
    let t = tile(2, 3, 1, 1); // cells 32..64: touches L tile 0 and (within reach) tile 1
    let plan = plan_rivers(&request(&src, 32), &[(3, t, 0.0)]).unwrap();
    let reads = src.reads.lock().unwrap().clone();
    assert!(reads.len() >= 2, "the node needs more than one L tile: {reads:?}");
    let (origin, step) = node_window(t, 32, GRID.cells);
    let once = river_coverage_grid(&[river], origin, step);
    assert_eq!(plan.nodes[0].coverage, once, "the duplicates change nothing");
    // The same plan twice gives the same bytes.
    let again = plan_rivers(&request(&src, 32), &[(3, t, 0.0)]).unwrap();
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
    let e = plan_rivers(&request(&src, 32), &[(7, tile(2, 3, 1, 1), 0.0)]).unwrap_err();
    assert!(e.contains("river tile 2/0/0"), "{e}");
    // A node at the corner of the face reads tiles 0 and up only: never an index outside 0..4.
    let src = Source::new(vec![channel(&[(2.0, 2.0), (20.0, 10.0)], q_for(3.0))]);
    plan_rivers(&request(&src, 32), &[(0, tile(2, 3, 0, 0), 0.0), (1, tile(2, 3, 7, 7), 0.0)]).unwrap();
    assert!(src.reads.lock().unwrap().iter().all(|&(_, ti, tj)| ti < 4 && tj < 4), "{:?}", src.reads.lock().unwrap());
    assert_eq!(RANGE_CELLS, 4.0);
}

// spec: REND-059, CON-30
#[test]
fn a_node_and_its_child_give_the_same_codes_at_the_positions_they_share() {
    // A river across the area of the level 3 node (1, 1); its level 4 child (2, 2) covers its lower-left quarter. The child's samples at even indices are the parent's samples: the codes agree bit for bit.
    let src = Source::new(vec![channel(&[(30.0, 36.0), (70.0, 52.0)], q_for(5.0))]);
    let (parent, child) = (tile(2, 3, 1, 1), tile(2, 4, 2, 2));
    let p = plan_rivers(&request(&src, 32), &[(0, parent, 0.0), (1, child, 0.0)]).unwrap();
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
    let both = plan_rivers(&request(&src, 32), &[(0, t, 0.0)]).unwrap();
    let (origin, step) = node_window(t, 32, GRID.cells);
    let want = river_coverage_grid(&[a.clone(), b.clone()], origin, step);
    assert_eq!(both.nodes[0].coverage, want, "the second piece is not dropped as a duplicate of the first");
}

// spec: REND-059
#[test]
fn a_plan_with_nothing_to_sample_or_a_bad_grid_is_an_error_not_a_panic() {
    let src = Source::new(Vec::new());
    assert!(plan_rivers(&request(&src, 0), &[(0, tile(2, 3, 1, 1), 0.0)]).is_err(), "res 0");
    for (min, full) in [(f64::NAN, 2.0), (1.0, f64::INFINITY), (0.0, 2.0), (2.0, 1.0), (1.0, 1.0)] {
        let mut req = request(&src, 32);
        (req.min_samples, req.full_samples) = (min, full);
        assert!(plan_rivers(&req, &[(0, tile(2, 3, 1, 1), 0.0)]).is_err(), "min {min}, full {full}");
    }
}

// spec: REND-059
#[test]
#[should_panic(expected = "points of a channel are finite")]
fn a_channel_point_that_is_not_finite_is_refused_with_a_message() {
    let ch = channel(&[(f64::NEG_INFINITY, 5.0), (5.0, 5.0)], 1.0);
    river_distance_grid(&[ch], (0.0, 0.0), 1.0, (8, 8), &LAW, GRID.cell_m);
}

// spec: REND-061
#[test]
fn more_nodes_with_a_river_than_layers_keep_the_nearest_and_count_the_rest_never_silently() {
    // Six nodes along one long river (level 3, tiles 0..5 of the row 1), each with a distance; three layers.
    let river = channel(&[(2.0, 40.0), (200.0, 44.0)], q_for(4.0));
    let src = Source::new(vec![river]);
    let nodes: Vec<(u32, TileId, f64)> =
        [(10u32, 0u32, 900.0), (11, 1, 300.0), (12, 2, 100.0), (13, 3, 100.0), (14, 4, 500.0), (15, 5, 50.0)]
            .iter()
            .map(|&(i, x, d)| (i, tile(2, 3, x, 1), d))
            .collect();
    let all = plan_rivers(&request(&src, 32), &nodes).unwrap();
    assert_eq!(all.nodes.len(), 6, "every node has the river in range");
    assert_eq!(all.dropped, 0);
    let mut req = request(&src, 32);
    req.max_layers = 3;
    let few = plan_rivers(&req, &nodes).unwrap();
    // The nearest are 15 (50 m), then 12 and 13 (both 100 m: the tie goes to the lower node index, both fit); in node order.
    let kept: Vec<u32> = few.nodes.iter().map(|n| n.node).collect();
    assert_eq!(kept, vec![12, 13, 15]);
    assert_eq!(few.dropped, 3, "the three that got no layer are counted");
    // A tie at the limit goes to the lower node index: two nodes at the same distance, one layer.
    let tied = vec![(21u32, tile(2, 3, 1, 1), 70.0), (20, tile(2, 3, 2, 1), 70.0)];
    req.max_layers = 1;
    let one = plan_rivers(&req, &tied).unwrap();
    assert_eq!(one.nodes.iter().map(|n| n.node).collect::<Vec<_>>(), vec![20]);
    assert_eq!(one.dropped, 1);
    // The input order changes nothing.
    let mut rev = nodes.clone();
    rev.reverse();
    req.max_layers = 3;
    assert_eq!(plan_rivers(&req, &rev).unwrap(), few);
    // The text carries the dropped count and reads back equal.
    let text = few.to_text();
    assert!(text.starts_with("rivers 3\n"), "{}", &text[..20]);
    let lines: Vec<Vec<&str>> = text.lines().map(|l| l.split_whitespace().collect()).collect();
    assert_eq!(planet_frame::river::RiverPlan::from_lines(&lines).unwrap(), few);
}

// spec: REND-061
#[test]
fn neighbouring_nodes_of_one_level_hold_the_same_codes_on_their_shared_edge() {
    let src = Source::new(vec![channel(&[(40.0, 20.0), (100.0, 100.0)], q_for(6.0))]);
    // Level 3 tiles (1, 1) and (2, 1): the first one's last column is the second one's first.
    let p = plan_rivers(&request(&src, 32), &[(0, tile(2, 3, 1, 1), 0.0), (1, tile(2, 3, 2, 1), 0.0), (2, tile(2, 3, 1, 0), 0.0)]).unwrap();
    assert_eq!(p.nodes.len(), 3);
    let (a, b, c) = (&p.nodes[0].coverage, &p.nodes[1].coverage, &p.nodes[2].coverage);
    for y in 0..33usize {
        assert_eq!(a[y * 33 + 32], b[y * 33], "row {y}: the shared column");
    }
    // The tile below shares its last row with this one's first.
    for x in 0..33usize {
        assert_eq!(c[32 * 33 + x], a[x], "column {x}: the shared row");
    }
}

// spec: REND-061
#[test]
fn across_a_level_boundary_the_shared_samples_agree_and_the_in_between_ones_are_reported() {
    let src = Source::new(vec![channel(&[(20.0, 40.0), (120.0, 52.0)], q_for(6.0))]);
    // The coarse tile (1, 1) of level 3 meets the finer tiles (4, 2) and (4, 3) of level 4 on its right edge (x = 64).
    let p = plan_rivers(&request(&src, 32), &[(0, tile(2, 3, 1, 1), 0.0), (1, tile(2, 4, 4, 2), 0.0), (2, tile(2, 4, 4, 3), 0.0)]).unwrap();
    let (coarse, fine_low) = (&p.nodes[0].coverage, &p.nodes[1].coverage);
    // The coarse right column (rows 0..=32 over 32 cells) against the lower fine tile's left column (rows 0..=32 over 16 cells): the fine sample at row 2j is the coarse sample at row j of the lower half... the
    // fine tile (4, 2) starts at the cell row 32 of the face, the coarse tile (1, 1) at 32 too, so the fine rows 0, 2, 4.. are the coarse rows 0, 1, 2.. at x = 64 only where the lattices coincide.
    let mut max_between = 0i32;
    let mut identical = 0;
    for j in 0..=16usize {
        let (cf, cc) = (fine_low[(2 * j) * 33], coarse[j * 33 + 32]);
        assert_eq!(cf, cc, "the sample at the same face position agrees (row {j})");
        identical += 1;
        if j < 16 {
            // The fine tile's in-between sample against the coarse linear interpolation of its two neighbours.
            let mid = i32::from(fine_low[(2 * j + 1) * 33]);
            let lin = (i32::from(coarse[j * 33 + 32]) + i32::from(coarse[(j + 1) * 33 + 32])) / 2;
            max_between = max_between.max((mid - lin).abs());
        }
    }
    assert_eq!(identical, 17);
    // Reported in the TEST-117 report; guarded here so that it cannot grow unnoticed (half a cell).
    assert!(max_between <= 16, "an in-between sample differs from the coarse interpolation by {max_between} codes");
    println!("level boundary: largest code difference of an in-between sample against the coarse interpolation: {max_between} codes ({:.2} cells)", f64::from(max_between) / 32.0);
}

// spec: REND-059
/// The thin-river thickening (Tracked, reported, not gated): the drawn width of a river over nodes whose samples are 1/r of its width apart, the codes interpolated bilinearly as the fragment does, against the river's own
/// width, over phases of the river's centre between sample rows and over its direction against the sample grid (0, 22.5 and 45 degrees). The wet area over a stretch of river, divided by the stretch and the river's width.
#[test]
fn the_wet_width_of_a_thin_river_over_coarse_samples_is_reported_against_its_own_width() {
    let width = 3.0f64;
    let q = q_for(width);
    println!("wet width / river width, node samples 1/r of the width apart (min, mean, max over 4 phases and 3 directions):");
    let mut rows: Vec<(f64, f64, f64, f64)> = Vec::new();
    for &r in &[0.5f64, 0.75, 1.0, 1.2, 1.5, 2.0, 3.0, 4.0] {
        let step = width / r;
        let (mut lo, mut hi, mut sum, mut n) = (f64::MAX, 0.0f64, 0.0f64, 0.0f64);
        for deg in [0.0f64, 22.5, 45.0] {
            let (ux, uy) = (libm::cos(deg.to_radians()), libm::sin(deg.to_radians()));
            for ph in 0..4 {
                // The grid has 33 x 33 samples; the river passes the grid's centre sample shifted by `ph / 4` of a step across.
                let size = 33usize;
                let c = 16.0 * step;
                let (cx, cy) = (c - uy * step * f64::from(ph) / 4.0, c + ux * step * f64::from(ph) / 4.0);
                let ch =
                    channel(&[(cx - 100.0 * width * ux, cy - 100.0 * width * uy), (cx + 100.0 * width * ux, cy + 100.0 * width * uy)], q);
                let d = river_distance_grid(&[ch], (0.0, 0.0), step, (size, size), &LAW, GRID.cell_m);
                let code = |i: usize, j: usize| f64::from(planet_frame::river::distance_code(d[j * size + i]));
                // A square of the grid's middle (8 samples a side), sampled at 16 points a sample; the wet area against the river's length in the square along its direction.
                let (span, sub) = (8.0f64, 16.0f64);
                let (mut wet, mut count) = (0.0f64, 0.0f64);
                let total = (span * sub) as usize;
                for a in 0..total {
                    for b in 0..total {
                        let (x, y) = (12.0 + (a as f64 + 0.5) / sub, 12.0 + (b as f64 + 0.5) / sub);
                        let (i, j) = (libm::floor(x) as usize, libm::floor(y) as usize);
                        let (fx, fy) = (x - libm::floor(x), y - libm::floor(y));
                        let v = code(i, j) * (1.0 - fx) * (1.0 - fy)
                            + code(i + 1, j) * fx * (1.0 - fy)
                            + code(i, j + 1) * (1.0 - fx) * fy
                            + code(i + 1, j + 1) * fx * fy;
                        wet += f64::from(u8::from(v <= 128.0));
                        count += 1.0;
                    }
                }
                // The length of river inside the square: the square's area over its width, the river passing through its middle: its extent along the direction is span / max(|cos|, |sin|) for the chord through the centre.
                let chord = span / libm::fmax(libm::fabs(ux), libm::fabs(uy));
                let area = wet / count * span * span * step * step;
                let h = area / (chord * step * width);
                lo = lo.min(h);
                hi = hi.max(h);
                sum += h;
                n += 1.0;
            }
        }
        rows.push((r, lo, sum / n, hi));
        println!("  r = {r:>5.2}: {lo:.2}, {:.2}, {hi:.2}", sum / n);
    }
    // Reported, not gated as a look value; what the test holds is the finding: the wet band is never wider than the river (1.1 of its width at most) whatever the phase and direction, and a river of 1.2 samples or more
    // is drawn at 0.8 of its width or more; below one sample it thins and breaks up instead of thickening.
    assert!(rows.iter().all(|x| x.3 <= 1.1), "no thickening: {rows:?}");
    assert!(rows.iter().filter(|x| x.0 >= 1.2).all(|x| x.1 >= 0.8), "a river of 1.2 samples or more keeps its width: {rows:?}");
}

/// A river source with several rivers of different widths over level 3 and 4 nodes, and the nodes of a small flight: `frames` sets of nodes that overlap from one frame to the next.
fn flight_source() -> Source {
    Source::new(vec![
        channel(&[(10.0, 60.0), (90.0, 70.0), (180.0, 90.0)], q_for(6.0)),
        channel(&[(100.0, 10.0), (110.0, 100.0), (120.0, 200.0)], q_for(3.0)),
        channel(&[(30.0, 200.0), (150.0, 190.0), (240.0, 180.0)], q_for(9.0)),
    ])
}

fn flight_frames() -> Vec<Vec<(u32, TileId, f64)>> {
    let mut frames = Vec::new();
    for f in 0..6u32 {
        let mut nodes: Vec<(u32, TileId, f64)> = Vec::new();
        for (k, (x, y)) in [(f, 1u32), (f + 1, 1), (f, 2), (f + 1, 2), (f + 2, 3), (7 - f, 6)].into_iter().enumerate() {
            nodes.push((k as u32, tile(2, 3, x.min(7), y), f64::from(k as u32) * 10.0 + f64::from(f)));
        }
        // Finer nodes over the first river, and one node of another face.
        nodes.push((6, tile(2, 4, 2 + f, 3), 1.0));
        nodes.push((7, tile(2, 5, 6 + f, 7), 2.0));
        nodes.push((8, tile(0, 3, 1, 1), 3.0));
        frames.push(nodes);
    }
    frames
}

// spec: REND-064
#[test]
fn a_cached_plan_is_the_fresh_plan_bit_for_bit_over_a_flight_and_the_second_pass_builds_nothing() {
    let src = flight_source();
    let req = request(&src, 32);
    let mut cache = planet_frame::river::RiverCache::new(1 << 20);
    let mut any_river = false;
    for nodes in flight_frames() {
        let fresh = plan_rivers(&req, &nodes).unwrap();
        let cached = planet_frame::river::plan_rivers_cached(&req, &nodes, &mut cache).unwrap();
        assert_eq!(cached, fresh, "the cached plan is the fresh plan");
        any_river |= !fresh.nodes.is_empty();
    }
    assert!(any_river, "the flight meets rivers");
    let built = cache.counters().builds;
    assert!(built > 0 && !cache.is_empty() && cache.bytes() <= 1 << 20);
    // The same flight again: every node is a hit, nothing is built.
    for nodes in flight_frames() {
        assert_eq!(planet_frame::river::plan_rivers_cached(&req, &nodes, &mut cache).unwrap(), plan_rivers(&req, &nodes).unwrap());
    }
    assert_eq!(cache.counters().builds, built, "the second pass builds no node");
    assert!(cache.counters().hits > 0);
}

// spec: REND-064
#[test]
fn a_cache_held_to_a_few_tiles_evicts_and_rebuilds_the_same_bits_within_its_byte_bound() {
    let src = flight_source();
    let req = request(&src, 32);
    // 33 x 33 codes and the overhead: room for about three entries.
    let cap = 3 * (33 * 33 + 64) + 10;
    let mut cache = planet_frame::river::RiverCache::new(cap);
    for _ in 0..2 {
        for nodes in flight_frames() {
            assert_eq!(planet_frame::river::plan_rivers_cached(&req, &nodes, &mut cache).unwrap(), plan_rivers(&req, &nodes).unwrap());
            assert!(cache.bytes() <= cap, "{} bytes against {cap}", cache.bytes());
        }
    }
    assert!(cache.counters().evictions > 0, "the bound was reached");
    // A cache smaller than one entry keeps nothing and still gives the fresh plan.
    let mut tiny = planet_frame::river::RiverCache::new(10);
    for nodes in flight_frames() {
        assert_eq!(planet_frame::river::plan_rivers_cached(&req, &nodes, &mut tiny).unwrap(), plan_rivers(&req, &nodes).unwrap());
    }
    assert_eq!((tiny.len(), tiny.bytes()), (0, 0));
}

// spec: REND-064
#[test]
fn a_lookup_plan_lists_what_is_missing_and_after_the_fill_it_is_the_fresh_plan() {
    use planet_frame::river::{fill_river_cache, plan_rivers_lookup, RiverCache};
    let src = flight_source();
    let req = request(&src, 32);
    let mut cache = RiverCache::new(1 << 20);
    let nodes = &flight_frames()[2];
    let first = plan_rivers_lookup(&req, nodes, &mut cache).unwrap();
    assert!(
        first.plan.nodes.is_empty() && first.not_ready > 0,
        "nothing is built: no river this frame, {} nodes not ready",
        first.not_ready
    );
    assert!(!first.missing.is_empty() && first.missing.len() <= nodes.len());
    let mut sorted = first.missing.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), first.missing.len(), "each missing tile once");
    fill_river_cache(&req, &mut cache, &first.missing).unwrap();
    let second = plan_rivers_lookup(&req, nodes, &mut cache).unwrap();
    assert_eq!((second.not_ready, second.missing.len()), (0, 0));
    assert_eq!(second.plan, plan_rivers(&req, nodes).unwrap(), "after the fill the lookup is the fresh plan");
    assert!(!second.plan.nodes.is_empty());
}

// spec: REND-064
#[test]
fn another_request_or_source_clears_the_cache() {
    use planet_frame::river::{plan_rivers_cached, RiverCache};
    struct Other(Source, u64);
    impl RiverSource for Other {
        fn grid(&self) -> RiverGrid {
            GRID
        }
        fn tile_channels(&self, f: Face, i: u32, j: u32) -> Result<Arc<Vec<Channel>>, String> {
            self.0.tile_channels(f, i, j)
        }
        fn id(&self) -> u64 {
            self.1
        }
    }
    let (a, b) = (Other(flight_source(), 1), Other(flight_source(), 2));
    let nodes = &flight_frames()[0];
    let mut cache = RiverCache::new(1 << 20);
    plan_rivers_cached(&request(&a, 32), nodes, &mut cache).unwrap();
    assert!(!cache.is_empty());
    // The same request over another commit identity starts empty; so do other limits, node size and law.
    let builds = cache.counters().builds;
    plan_rivers_cached(&request(&b, 32), nodes, &mut cache).unwrap();
    assert!(cache.counters().builds > builds, "another commit builds again");
    let mut tighter = request(&b, 32);
    tighter.min_samples = 0.6;
    let before = cache.counters().builds;
    plan_rivers_cached(&tighter, nodes, &mut cache).unwrap();
    assert!(cache.counters().builds > before, "other limits build again");
    // A bad request is refused by the cache's adoption too.
    assert!(plan_rivers_cached(&request(&b, 0), nodes, &mut cache).is_err());
    // The layer count is not part of the stamp: the same entries serve another layer cap, and the drops follow the cap.
    let mut few = request(&b, 32);
    few.max_layers = 1;
    let plan = plan_rivers_cached(&few, nodes, &mut RiverCache::new(1 << 20)).unwrap();
    assert!(plan.nodes.len() <= 1);
    assert_eq!(plan_rivers_cached(&few, nodes, &mut cache).unwrap(), plan);
}
