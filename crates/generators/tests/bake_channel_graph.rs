//! Tier A, GEN-170 (TEST-114): the channel graph as compact arrays. The chains equal the routing's, `Q_w` and the bed level are within the step of their codes, the codes are monotone and round trip, the
//! arrays state their bytes. Small grids; no store.

use planet_generators::bake::channel_graph::{bed_code, bed_value, q_code, q_value, ChannelGraph, BED_STEP_M, Q_CODE_MAX, Q_CODE_MIN};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, Hydro, HydroParams, QUnit};
use planet_generators::bake::route::{route_rules, RoundRules, Routing};
use planet_generators::bake::Grid;

const W: usize = 40;
const EPS: f64 = 1e-3;

fn land() -> Vec<f64> {
    let mut z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64 + 3.0 * ((p / W) as f64 - 20.0).abs()).collect();
    for j in 10..16 {
        for i in 10..16 {
            z[j * W + i] = 300.0;
        }
    }
    z
}

struct Run {
    g: Grid,
    z: Vec<f64>,
    routing: Routing,
    hydro: Hydro,
}

fn run() -> Run {
    let g = Grid::new(W, W, 800.0);
    let z = land();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; W * W];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, Some(&runoff), None, &p);
    Run { g, z, routing, hydro }
}

// spec: GEN-170, TEST-114
#[test]
fn the_q_code_is_monotone_round_trips_and_is_within_the_step() {
    assert_eq!(q_code(0.0), 0);
    assert_eq!(q_code(-3.0), 0);
    assert_eq!(q_code(f64::NAN), 0);
    assert_eq!(q_value(0), 0.0);
    assert_eq!(q_code(Q_CODE_MIN), 1);
    assert_eq!(q_code(Q_CODE_MAX), 65535);
    assert_eq!(q_code(1.0e-9), 1, "below the range reads as the smallest");
    assert_eq!(q_code(1.0e12), 65535, "above the range reads as the largest");
    let mut prev = 0.0;
    for c in 1..=65535u16 {
        let v = q_value(c);
        assert!(v > prev, "code {c}");
        assert_eq!(q_code(v), c, "code {c} round trips");
        prev = v;
    }
    // The relative error of a read is at most half a step, 1.94e-4; the width 7.2 sqrt(Q) half of it.
    let mut worst = 0.0f64;
    let mut q = 2.0e-4;
    while q < 5.0e6 {
        worst = worst.max(((q_value(q_code(q)) - q) / q).abs());
        q *= 1.0137;
    }
    assert!(worst <= 1.94e-4, "worst relative error {worst}");
    println!("q code: step {:.3e}, worst relative error {worst:.3e}", q_value(2) / q_value(1) - 1.0);
}

// spec: GEN-170, TEST-114
#[test]
fn the_bed_code_keeps_the_order_of_the_levels_and_is_within_half_a_step() {
    for z in [-11000.0, -3.2, 0.0, 0.2, 0.26, 100.3, 100.4, 8848.0, 12000.0] {
        let below = z - bed_value(bed_code(z));
        assert!((0.0..BED_STEP_M).contains(&below), "the stored bed is at or below the level, less than a step: {z}: {below}");
    }
    assert!(bed_code(100.0) <= bed_code(100.3), "monotone");
    assert_eq!(bed_code(1.0e9), i16::MAX);
    assert_eq!(bed_code(-1.0e9), i16::MIN);
}

// spec: GEN-170, TEST-114
#[test]
fn the_graph_holds_the_routings_chains_q_w_and_a_bed_that_does_not_rise() {
    let r = run();
    assert!(!r.hydro.segments.is_empty());
    let graph = ChannelGraph::from_hydro(&r.g, &r.routing, &r.hydro, &r.z);
    assert_eq!(graph.segments(), r.hydro.segments.len());
    assert_eq!(graph.q_unit, QUnit::Discharge);
    let mut vertices = 0;
    graph.for_each(|s| {
        let seg = &r.hydro.segments[s.index];
        let cells = s.cells(&r.g);
        assert_eq!(cells.len(), seg.cells as usize + 1, "up to down, both included");
        assert_eq!((cells[0], *cells.last().unwrap()), (seg.up, seg.down));
        let mut level = f64::INFINITY;
        for (k, &c) in cells.iter().enumerate() {
            if k > 0 {
                assert_eq!(r.routing.receiver[cells[k - 1] as usize], c, "the chain follows the receivers");
            }
            let want = r.hydro.qw[c as usize];
            let got = q_value(s.q[k]);
            assert!(((got - want) / want).abs() <= 1.94e-4, "vertex {k} of segment {}: {got} against {want}", s.index);
            level = level.min(r.z[c as usize]);
            assert_eq!(s.bed[k], bed_code(level), "the bed is the running minimum");
        }
        assert!(s.bed.windows(2).all(|w| w[1] <= w[0]));
        assert!(
            s.bed
                .iter()
                .enumerate()
                .all(|(k, &b)| bed_value(b) <= cells[..=k].iter().map(|&c| r.z[c as usize]).fold(f64::INFINITY, f64::min)),
            "the bed is never above the surface it came from"
        );
        vertices += cells.len();
    });
    assert_eq!(graph.vertices(), vertices);
    assert!(graph.max_q_rel_error <= 1.94e-4 && graph.max_q_rel_error > 0.0, "the stated error: {}", graph.max_q_rel_error);
    assert_eq!(graph.bytes(), graph.segments() * 8 + vertices * 5, "8 bytes a segment, 5 a vertex (direction, Q_w code, bed code)");
    println!("graph: {} segments, {} vertices, {} bytes", graph.segments(), vertices, graph.bytes());
}

// spec: GEN-170, TEST-114
#[test]
fn a_cell_one_step_outside_a_face_resolves_across_the_cube_edge_as_the_graph_joins_it() {
    let n = 12usize;
    let g = Grid::cube(n, 6_371_000.0);
    let id = |f: u8, i: usize, j: usize| usize::from(f) * n * n + j * n + i;
    for f in 0..6u8 {
        // Inside: the identity.
        assert_eq!(g.cell_at_face(f, 3, 4), Some(id(f, 3, 4)));
        for k in 0..n {
            // One step outside along each axis is the neighbour across the edge, i.e. the graph's own neighbour of the edge cell.
            for (o, (a, b), d) in
                [(0usize, (-1i64, k as i64), 0usize), (1, (n as i64, k as i64), 1), (2, (k as i64, -1), 2), (3, (k as i64, n as i64), 3)]
            {
                let edge_cell = match o {
                    0 => id(f, 0, k),
                    1 => id(f, n - 1, k),
                    2 => id(f, k, 0),
                    _ => id(f, k, n - 1),
                };
                let got = g.cell_at_face(f, a, b).expect("a cell across the edge");
                assert_eq!(Some(got), g.neighbour(edge_cell, d), "face {f}, side {o}, k {k}");
                assert_ne!(got / (n * n), usize::from(f), "it is on another face");
            }
        }
        // A corner quadrant, two steps out and a face above 5 have no cell.
        assert_eq!(g.cell_at_face(f, -1, -1), None);
        assert_eq!(g.cell_at_face(f, n as i64, n as i64), None);
        assert_eq!(g.cell_at_face(f, -2, 3), None);
        assert_eq!(g.cell_at_face(f, 3, n as i64 + 1), None);
    }
    assert_eq!(g.cell_at_face(6, 1, 1), None);
    // A plane grid has face 0 and its own cells only.
    let p = Grid::new(10, 10, 800.0);
    assert_eq!(p.cell_at_face(0, 2, 3), Some(32));
    assert_eq!(p.cell_at_face(0, -1, 3), None);
    assert_eq!(p.cell_at_face(1, 2, 3), None);
}

// spec: GEN-170, TEST-114
#[test]
fn the_lake_cuts_of_the_graph_are_the_producers_pieces_and_a_lake_entry_and_spill_cell_are_in_the_chain() {
    use planet_generators::bake::lakes::{lake_cut_ranges, lakes_from_hydro, river_pieces, water_cells};
    // A bowl with a river through it (as the lake producer's tests): the producer cuts the segments that go under water into pieces.
    let g = Grid::new(W, W, 800.0);
    let mut z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64).collect();
    for j in 10..16 {
        for i in 10..16 {
            z[j * W + i] = 300.0;
        }
    }
    z[12 * W + 12] = 280.0;
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; W * W];
    let p = HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() };
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, Some(&runoff), None, &p);
    let lakes = lakes_from_hydro(&g, &routing, &hydro, &z);
    assert!(!lakes.pieces.is_empty() && lakes.dropped_cells > 0, "the fixture has a river under a lake");
    let pre = river_pieces(&g, &routing, &hydro, &z);
    let mut graph = ChannelGraph::from_hydro(&g, &routing, &hydro, &z);
    graph.set_cuts(pre.cuts.clone());
    // The dropped cells are the cut vertices; the pieces are the runs between the cuts.
    assert_eq!(pre.cuts.iter().map(|c| (c.2 - c.1) as usize).sum::<usize>(), lakes.dropped_cells);
    let water = water_cells(&g, &routing, &hydro, &z);
    let mut pieces_seen = 0;
    graph.for_each(|s| {
        let cells = s.cells(&g);
        let seg = &hydro.segments[s.index];
        let cuts = graph.cuts_of(s.index);
        // Every cut vertex is under water, the vertex before it (the lake entry) and the one at its end (the spill cell) are not, and they are in the chain.
        for &(_, a, b) in cuts {
            assert!((a..b).all(|k| water.contains(cells[k as usize] as usize)));
            if a > 0 {
                assert!(!water.contains(cells[a as usize - 1] as usize), "the lake entry is out of the water");
            }
            assert!((b as usize) < cells.len());
            if b < seg.cells {
                assert!(!water.contains(cells[b as usize] as usize), "the spill cell is out of the water");
            } else if water.contains(cells[b as usize] as usize) {
                // The water runs on into the next segment (it starts under water): its cut begins at vertex 0.
                assert!(
                    graph.cuts_of(seg.downstream as usize).first().is_some_and(|c| c.1 == 0),
                    "the river continues under water in the downstream segment"
                );
            }
        }
        // The runs between the cuts (over the segment's cells, its end cell excluded) are the producer's pieces, in order and numbered from 0.
        let mut runs: Vec<(u32, u32)> = Vec::new();
        let mut start = 0u32;
        for &(_, a, b) in cuts {
            if a > start {
                runs.push((start, a));
            }
            start = b;
        }
        if start < seg.cells {
            runs.push((start, seg.cells));
        }
        let mine: Vec<_> = lakes.pieces.iter().filter(|p| p.segment == seg.id).collect();
        if cuts.is_empty() {
            assert!(mine.is_empty(), "a segment with no cut has no piece");
        } else {
            assert_eq!(mine.len(), runs.len(), "segment {}", s.index);
            for (part, (m, r)) in mine.iter().zip(&runs).enumerate() {
                assert_eq!(
                    (m.part as usize, m.first, m.cells),
                    (part, cells[r.0 as usize], r.1 - r.0),
                    "piece {part} of segment {}",
                    s.index
                );
                pieces_seen += 1;
            }
        }
    });
    assert_eq!(pieces_seen, lakes.pieces.len());
    // The cuts are part of the stated bytes.
    assert_eq!(graph.bytes(), graph.segments() * 8 + graph.vertices() * 5 + graph.cuts().len() * 12);
    assert_eq!(lake_cut_ranges(&hydro, &routing, &|c| water.contains(c)), pre.cuts);
}
