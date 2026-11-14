//! TEST-136: the river gate's measures (BUILD-055) on small fixtures.
use planet_cache::river_gate::{edge_agreement, measure, Crossing, GateInput, GateParams};
use planet_generators::bake::Grid;

const N: usize = 40;
const DX: f64 = 100.0;

/// A valley along y draining to the edge y = 0: the floor at x = 20 falls 1 m a cell (100 m + y), the sides rise 2 m a cell.
fn valley() -> Vec<f64> {
    (0..N * N).map(|c| 100.0 + 2.0 * ((c % N) as f64 - 20.0).abs() + (c / N) as f64).collect()
}

/// `z` with the block x 18..=22, y 20..=24 (25 cells) flat at `depth` metres below the floor's spill cell (20, 19) at 119 m: a pit of 25 cells, `depth` deep.
fn with_pit(mut z: Vec<f64>, depth: f64) -> Vec<f64> {
    for y in 20..=24 {
        for x in 18..=22 {
            z[y * N + x] = 119.0 - depth;
        }
    }
    z
}

fn params() -> GateParams {
    // a river from 10 cells of area on
    GateParams { channel_area_m2: 10.0 * DX * DX, ..GateParams::default() }
}

fn run(start: &[f64], surface: &[f64], p: &GateParams) -> planet_cache::river_gate::GateReport {
    let g = Grid::new(N, N, DX);
    let outlet = g.outlets(surface, 0.0);
    let all = vec![true; N * N];
    measure(&GateInput { grid: &g, start, surface, outlet: &outlet, inside: &all, region: &all, h1: &[], seed: 7 }, p)
}

// spec: TEST-136, BUILD-055
#[test]
fn a_draining_valley_has_no_basin_no_cut_no_lake_and_keeps_its_relief() {
    let z = valley();
    let r = run(&z, &z, &params());
    assert!(r.river_cells > 20, "the valley floor carries a river: {} cells", r.river_cells);
    assert_eq!(
        (r.basins, r.basin_river_cells, r.lake_river_cells, r.canyon.count_over, r.canyon_breached.count_over, r.erosion.count_over),
        (0, 0, 0, 0, 0, 0),
        "{r:?}"
    );
    assert_eq!(r.relief_2_m, r.relief_2_start_m);
    assert_eq!(r.relief_10_m, r.relief_10_start_m);
    assert!(r.relief_10_m > 0.0);
}

// spec: TEST-136, BUILD-055
#[test]
fn a_pit_on_the_river_is_a_closed_basin_only_within_the_depth_and_cell_limits() {
    let z = with_pit(valley(), 3.0);
    let r = run(&z, &z, &params());
    assert_eq!(r.basins, 1, "{r:?}");
    assert!(r.basin_river_cells > 0 && r.basin_share() > 0.0, "the river runs through the pit: {r:?}");
    assert_eq!(r.lake_river_cells, 0, "3 m is below D = 20 m");
    // Fewer cells than the limit: not a basin.
    let few = run(&z, &z, &GateParams { basin_cells: 26, ..params() });
    assert_eq!((few.basins, few.basin_river_cells), (0, 0), "the pit has at most 25 wet cells: {few:?}");
    let enough = run(&z, &z, &GateParams { basin_cells: 4, ..params() });
    assert_eq!(enough.basins, 1);
    // Shallower than 0.5 m: not a basin.
    let shallow = with_pit(valley(), 0.3);
    let s = run(&shallow, &shallow, &params());
    assert_eq!((s.basins, s.basin_river_cells), (0, 0), "{s:?}");
    assert!(s.reference_river_cells > 0, "SPIKE-114's reference counts any 5 cm pit: {s:?}");
    // The default cell minimum is 16: the 25-cell pit counts, a 9-cell one (x 19..=21, y 20..=22) does not.
    assert_eq!(GateParams::default().basin_cells, 16);
    let mut small = valley();
    for y in 20..=22 {
        for x in 19..=21 {
            small[y * N + x] = 116.0;
        }
    }
    let m = run(&small, &small, &params());
    assert_eq!((m.basins, m.basin_river_cells), (0, 0), "{m:?}");
}

// spec: TEST-136, BUILD-055
#[test]
fn a_trench_below_its_valley_is_a_canyon_and_a_uniform_lowering_only_erosion_depth() {
    let start = valley();
    // The whole surface 60 m lower: every river cell 60 m under the start, none under its valley.
    let lower: Vec<f64> = start.iter().map(|z| z - 60.0).collect();
    let e = run(&start, &lower, &params());
    assert!(e.erosion.count_over > 0 && e.erosion.p50_m > 50.0, "{e:?}");
    assert_eq!(e.canyon.count_over, 0, "{e:?}");
    // A trench 60 m deep along the floor (x = 20, y 0..=38, open to the outlet edge): the river runs 60 m under its valley ring.
    let mut trench = start.clone();
    for y in 0..=38 {
        trench[y * N + 20] -= 60.0;
    }
    let t = run(&start, &trench, &params());
    assert!(t.canyon.count_over > 0 && t.canyon.count_over >= 30, "the trench is the river: {t:?}");
    assert!(t.canyon.max_m > 50.0);
}

// spec: TEST-136, BUILD-055
#[test]
fn a_deep_basin_is_a_lake_at_d_and_a_canyon_when_breached() {
    let z = with_pit(valley(), 60.0);
    let r = run(&z, &z, &params());
    assert!(r.lake_river_cells > 0 && r.lake_share() > 0.0, "60 m is over D = 20 m: {r:?}");
    assert_eq!(r.canyon.count_over, 0, "the lake's water is the level downstream: no cut");
    assert!(r.canyon_breached.count_over > 0, "breached, the river runs 60 m under the floor below the pit: {r:?}");
    let deep_d = run(&z, &z, &GateParams { lake_depth_m: 100.0, ..params() });
    assert_eq!(deep_d.lake_river_cells, 0);
    assert!(deep_d.canyon.count_over > 0 && deep_d.canyon_share() > 0.0, "no lake at D = 100 m: the pit is breached");
    assert!(deep_d.canyon.max_m > 50.0, "{deep_d:?}");
}

// spec: TEST-136, BUILD-055
#[test]
fn relief_is_measured_against_the_start_surface() {
    let start = valley();
    let half: Vec<f64> = start.iter().map(|z| 5.0 + 0.5 * (z - 5.0)).collect();
    let r = run(&start, &half, &params());
    assert!((r.relief_10_m - 0.5 * r.relief_10_start_m).abs() < 1e-9, "{r:?}");
    assert!((r.relief_2_m - 0.5 * r.relief_2_start_m).abs() < 1e-9, "{r:?}");
    assert!(r.lines().iter().any(|l| l.starts_with("RIVER-GATE relief") && l.contains("(50 %)")), "{:?}", r.lines());
}

// spec: TEST-136, BUILD-055
#[test]
fn an_h1_crossing_is_matched_by_an_l_crossing_of_its_direction_within_the_distance() {
    let p = GateParams::default();
    let c = |inside: bool, x: f64, y: f64, q: f64| Crossing { inside, x, y, q_m3s: q };
    let l = [c(true, 0.0, 0.0, 0.0), c(false, 100.0, 0.0, 0.0)];
    // 19 cells of 101.8 m is 1,934 m: matched; 20 cells is 2,036 m: not.
    let h1 = [c(true, 19.0, 0.0, 5.0), c(true, 0.0, 20.0, 5.0), c(false, 0.0, 1.0, 5.0), c(true, 0.0, 1.0, 0.5)];
    assert_eq!(
        edge_agreement(&l, &h1, 101.8, &p),
        (1, 3),
        "the near one of its direction; the far one, the other direction, and the small river not counted"
    );
    assert_eq!(edge_agreement(&[], &h1, 101.8, &p), (0, 3));
}

// spec: TEST-136, BUILD-055
#[test]
fn the_report_is_the_same_for_the_same_input_and_names_every_measure() {
    let z = with_pit(valley(), 3.0);
    let (a, b) = (run(&z, &z, &params()), run(&z, &z, &params()));
    assert_eq!(a, b);
    let lines = a.lines();
    assert_eq!(lines, b.lines());
    for m in ["rivers", "basins", "canyon", "erosion depth", "lakes", "relief", "edge"] {
        assert!(lines.iter().any(|l| l.starts_with(&format!("RIVER-GATE {m}"))), "no {m} line in {lines:?}");
    }
    assert!(lines.iter().any(|l| l.contains("no H1 crossing")), "an input without H1 says so");
}
