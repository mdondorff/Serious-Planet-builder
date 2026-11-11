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
    // The relative error of a read is at most half a step, 1.9e-4; the width 7.2 sqrt(Q) half of it.
    let mut worst = 0.0f64;
    let mut q = 2.0e-4;
    while q < 5.0e6 {
        worst = worst.max(((q_value(q_code(q)) - q) / q).abs());
        q *= 1.0137;
    }
    assert!(worst <= 1.95e-4, "worst relative error {worst}");
    println!("q code: step {:.3e}, worst relative error {worst:.3e}", q_value(2) / q_value(1) - 1.0);
}

// spec: GEN-170, TEST-114
#[test]
fn the_bed_code_keeps_the_order_of_the_levels_and_is_within_half_a_step() {
    for z in [-11000.0, -3.2, 0.0, 0.2, 0.26, 8848.0, 12000.0] {
        let err = (bed_value(bed_code(z)) - z).abs();
        assert!(err <= BED_STEP_M / 2.0 + 1e-9 || z.abs() > 16000.0, "{z}: {err}");
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
            assert!(((got - want) / want).abs() <= 1.95e-4, "vertex {k} of segment {}: {got} against {want}", s.index);
            level = level.min(r.z[c as usize]);
            assert_eq!(s.bed[k], bed_code(level), "the bed is the running minimum");
        }
        assert!(s.bed.windows(2).all(|w| w[1] <= w[0]));
        vertices += cells.len();
    });
    assert_eq!(graph.vertices(), vertices);
    assert!(graph.max_q_rel_error <= 1.95e-4 && graph.max_q_rel_error > 0.0, "the stated error: {}", graph.max_q_rel_error);
    assert_eq!(graph.bytes(), graph.segments() * 8 + vertices * 5, "8 bytes a segment, 5 a vertex (direction, Q_w code, bed code)");
    println!("graph: {} segments, {} vertices, {} bytes", graph.segments(), vertices, graph.bytes());
}
