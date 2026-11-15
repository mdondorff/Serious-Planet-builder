//! TEST-137: the river gate's measurement suite (BUILD-057) on small fixtures, and its store path on a small baked store.
use planet_cache::l_writer::read_l_tile;
use planet_cache::pack::TileStore;
use planet_cache::river_gate::{edge_pairs, lay_store_ground, measure, Crossing, GateInput, GateParams, GateReport, LWindow};
use planet_cache::river_suite::spectrum;
use planet_core::Face;
use planet_generators::bake::Grid;

const N: usize = 40;
const DX: f64 = 100.0;

/// A valley along y draining to the edge y = 0: the floor at x = 20 falls 1 m a cell (100 m + y); the side rises `left` m a cell below x = 20 and `right` m a cell above it.
fn valley(left: f64, right: f64) -> Vec<f64> {
    (0..N * N)
        .map(|c| {
            let dx = (c % N) as f64 - 20.0;
            100.0 + if dx < 0.0 { -dx * left } else { dx * right } + (c / N) as f64
        })
        .collect()
}

fn params() -> GateParams {
    // a river from 100 cells of area on: the floor only, not the flanks
    GateParams { channel_area_m2: 100.0 * DX * DX, ..GateParams::default() }
}

fn run_in(start: &[f64], surface: &[f64], region: &[bool], h1: &[Crossing]) -> GateReport {
    let g = Grid::new(N, N, DX);
    let outlet = g.outlets(surface, 0.0);
    let all = vec![true; N * N];
    measure(
        &GateInput {
            grid: &g,
            start,
            surface,
            outlet: &outlet,
            inside: &all,
            region,
            h1,
            seed: 7,
            sea_level_m: 0.0,
            outside_drains: false,
            trunks: &[],
        },
        &params(),
    )
}

fn run(z: &[f64]) -> GateReport {
    run_in(z, z, &vec![true; N * N], &[])
}

// spec: TEST-137, BUILD-057
#[test]
fn a_river_steps_uphill_only_where_the_surface_rises_in_its_pit() {
    let z = valley(2.0, 2.0);
    let r = run(&z);
    assert!(r.suite.steps > 20, "{:?}", r.suite);
    assert_eq!(r.suite.uphill_steps, 0, "a draining valley never rises: {:?}", r.suite);
    // A 3 m pit (x 18..=22, y 20..=24) is filled flat for the routing: the river leaves it over a 3 m rise.
    let mut p = valley(2.0, 2.0);
    for y in 20..=24 {
        for x in 18..=22 {
            p[y * N + x] = 116.0;
        }
    }
    let r = run(&p);
    assert!(r.suite.uphill_steps >= 1 && r.suite.uphill_max_m > 2.0, "{:?}", r.suite);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_valley_width_is_read_across_the_flow_at_a_fixed_depth_and_at_half_the_ring_depth() {
    let r = run(&valley(2.0, 2.0));
    // 2 m a cell: 10 m is 5 cells to each side: 1,000 m.
    assert!((r.suite.width_fixed.p50 - 1000.0).abs() < 50.0, "{:?}", r.suite.width_fixed);
    assert_eq!(r.suite.width_fixed_capped, 0);
    assert!(r.suite.width_half.n > 0 && r.suite.width_half.p50 > 100.0, "{:?}", r.suite.width_half);
    // Steeper sides, a narrower valley.
    let steep = run(&valley(4.0, 4.0));
    assert!(
        steep.suite.width_fixed.p50 < r.suite.width_fixed.p50 - 300.0,
        "{:?} against {:?}",
        steep.suite.width_fixed,
        r.suite.width_fixed
    );
    // A floor with no sides (flat across) is capped at 64 cells a side.
    let flat = run(&valley(0.0, 0.0));
    assert!(flat.suite.width_fixed_capped > 0 && flat.suite.width_fixed.p50 >= 3000.0, "{:?}", flat.suite.width_fixed);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_asymmetry_is_zero_for_a_symmetric_valley_and_a_third_for_sides_of_4_and_2() {
    let s = run(&valley(2.0, 2.0)).suite.asymmetry;
    assert!(s.n > 0 && s.p50 < 0.01 && s.p90 < 0.01, "{s:?}");
    // The rise 2 to 4 cells out: 3 x 4 = 12 m on one side and 3 x 2 = 6 m on the other: |12 - 6| / 18.
    let a = run(&valley(4.0, 2.0)).suite.asymmetry;
    assert!(a.n > 0 && (a.p50 - 1.0 / 3.0).abs() < 0.02, "{a:?}");
}

// spec: TEST-137, BUILD-057
#[test]
fn the_drainage_density_is_the_river_length_over_the_land_area() {
    let s = run(&valley(2.0, 2.0)).suite;
    assert!((s.land_km2 - 16.0).abs() < 1e-9, "{s:?}");
    assert!(s.river_km > 2.0 && s.river_km < 4.2, "one river down the middle, at most 39 cells of 100 m: {s:?}");
    assert!((s.density_km_per_km2 - s.river_km / 16.0).abs() < 1e-12);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_catchment_ratio_and_the_seam_read_the_region_edge() {
    let z = valley(2.0, 2.0);
    // The region is the rows y < 20: the river crosses its edge at (20, 20) going in (the cell outside).
    let region: Vec<bool> = (0..N * N).map(|c| c / N < 20).collect();
    let h1 = [Crossing { inside: false, x: 20.0, y: 20.0, q_m3s: 2.0 }, Crossing { inside: false, x: 20.0, y: 20.0, q_m3s: 0.5 }];
    let r = run_in(&z, &z, &region, &h1);
    // Only the crossing with Q >= 1 m3/s counts: one pair, so every ratio to the median is 1.
    assert_eq!(r.suite.conservation_pairs, 1, "{:?}", r.suite);
    assert!(r.suite.implied_runoff > 0.0);
    assert_eq!((r.suite.conservation_p10, r.suite.conservation_p90), (1.0, 1.0));
    // The seam: 40 vertical neighbour pairs across y 19 | 20, each a 1 m step; inside the region the p99 is the 2 m side step.
    assert_eq!(r.suite.seam_pairs, 40, "{:?}", r.suite);
    assert_eq!((r.suite.seam_p50_m, r.suite.seam_max_m), (1.0, 1.0));
    assert_eq!(r.suite.interior_p99_m, 2.0);
    // A 50 m step at the edge shows in the seam only.
    let mut cliff = z.clone();
    for (c, v) in cliff.iter_mut().enumerate() {
        if c / N >= 20 {
            *v += 50.0;
        }
    }
    let r = run_in(&cliff, &cliff, &region, &[]);
    assert!(r.suite.seam_p50_m > 50.0 && r.suite.interior_p99_m <= 2.0, "{:?}", r.suite);
}

// spec: TEST-137, BUILD-057
#[test]
fn edge_pairs_take_the_nearest_crossing_of_the_direction_within_the_distance() {
    let c = |inside: bool, x: f64, y: f64, q: f64| Crossing { inside, x, y, q_m3s: q };
    let l = [c(true, 10.0, 10.0, 0.0), c(true, 11.0, 10.0, 0.0), c(false, 10.0, 10.0, 0.0)];
    let area = [1.0e6, 2.0e6, 3.0e6];
    let p = GateParams::default();
    // Cells of 100 m: the crossing at (10.9, 10) is nearest to (11, 10); the 3 km away one is out of the 2 km, and a Q below 1 is not counted.
    let h1 = [c(true, 10.9, 10.0, 3.0), c(true, 40.0, 10.0, 3.0), c(false, 10.0, 10.0, 0.4)];
    assert_eq!(edge_pairs(&l, &area, &h1, 100.0, &p), vec![(2.0e6, 3.0)]);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_narrowness_ratio_is_one_for_a_trench_and_under_one_for_a_v() {
    let v = run(&valley(2.0, 2.0)).suite;
    assert!(v.narrowness.n > 0 && v.narrowness_of_p50 > 0.2 && v.narrowness_of_p50 < 0.7, "{v:?}");
    // A 60 m trench: the cells 1 to 2 out stand as high above the river as those 2 to 4 out.
    let start = valley(2.0, 2.0);
    let mut trench = start.clone();
    for y in 0..N {
        trench[y * N + 20] -= 60.0;
    }
    let t = run_in(&start, &trench, &vec![true; N * N], &[]).suite;
    assert!(t.narrowness_of_p50 > 0.85 && t.narrowness.p50 > 0.85, "{t:?}");
}

// spec: TEST-137, BUILD-057
#[test]
fn the_slope_area_exponent_is_zero_for_a_ramp_and_positive_for_a_profile_steeper_upstream() {
    let ramp: Vec<f64> = (0..N * N).map(|c| 100.0 + 5.0 * (c / N) as f64).collect();
    let r = run(&ramp).suite;
    assert!(r.slope_area.iter().any(|b| b.cells >= 30) && r.theta.abs() < 0.1, "{:?} theta {}", r.slope_area, r.theta);
    // Steeper where the area is small (the rows far from the outlet): the slope falls with the area, over the bins that hold 30 cells.
    let concave: Vec<f64> = (0..N * N).map(|c| 100.0 + 200.0 * ((N as f64).sqrt() - ((N - c / N) as f64).sqrt())).collect();
    let s = run(&concave).suite;
    let fitted: Vec<&planet_cache::river_suite::SlopeBin> = s.slope_area.iter().filter(|b| b.cells >= 30).collect();
    assert!(fitted.len() >= 3 && fitted.first().unwrap().slope_p50 > fitted.last().unwrap().slope_p50, "{:?}", s.slope_area);
    assert!(s.theta > 0.05 && s.theta > r.theta + 0.05, "theta {} against the ramp's {}", s.theta, r.theta);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_relief_spectrum_peaks_at_the_octave_of_a_sinusoid() {
    let g = Grid::new(N, N, DX);
    let inside = vec![true; N * N];
    let wave =
        |period: f64| -> Vec<f64> { (0..N * N).map(|c| 10.0 * libm::sin(2.0 * std::f64::consts::PI * (c % N) as f64 / period)).collect() };
    let peak =
        |s: &planet_cache::river_suite::Spectrum| s.power.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(o, _)| o).unwrap();
    // The window's square is 32 cells (the largest power of two in 40): a period of 8 cells is wavenumber 4 (octave 2), 16 cells wavenumber 2 (octave 1).
    let s8 = spectrum(&g, &wave(8.0), &inside);
    assert_eq!((s8.n, peak(&s8)), (32, 2), "{s8:?}");
    let s16 = spectrum(&g, &wave(16.0), &inside);
    assert_eq!(peak(&s16), 1, "{s16:?}");
    // A flat surface has no power.
    let flat = spectrum(&g, &vec![5.0; N * N], &inside);
    assert!(flat.power.iter().all(|&p| p < 1e-12) && flat.beta == 0.0, "{flat:?}");
}

// spec: TEST-137, BUILD-057
#[test]
fn the_suite_prints_one_line_per_metric_and_gives_the_same_bits_twice() {
    let z = valley(4.0, 2.0);
    let (a, b) = (run(&z), run(&z));
    assert_eq!(a, b);
    let lines = a.lines().join("\n");
    for key in ["gradient", "valley width", "asymmetry", "drainage density", "catchment", "seam", "narrowness", "slope-area", "spectrum"] {
        assert!(lines.contains(&format!("RIVER-GATE {key}:")), "no {key} line in\n{lines}");
    }
}

/// A B for the store fixture's window: any smooth height of the face's L cell (the laying adds it to the tile's change).
fn store_base(x: i64, y: i64) -> f64 {
    300.0 + 0.5 * x as f64 + 0.25 * y as f64
}

// spec: TEST-137, BUILD-057, BUILD-055
#[test]
fn a_baked_store_read_through_the_reader_gives_the_numbers_of_its_surface() {
    use crate::l_writer::{bake, lakes, spec, tiles, world, Dir, Feed, RES, T};
    let (w, d) = (world(), Dir::new("river-suite-store"));
    let commit = bake(&d, &w, spec(7), &Feed(true), &lakes(), 2).expect("the fixture bake");
    let store = TileStore::open(d.path(), w.params()).unwrap();
    // The window: face 2 from L cell (24, 24), 5 tiles a side; the commit's tiles (2..=4, 2..=4) lie inside it.
    let win = LWindow { face: Face(2), x0: T as i64, y0: T as i64, w: 5 * T, h: 5 * T };
    let n = win.w * win.h;
    let start: Vec<f64> = (0..n).map(|c| store_base(win.x0 + (c % win.w) as i64, win.y0 + (c / win.w) as i64)).collect();
    let (mut surface, mut inside) = (start.clone(), vec![false; n]);
    let (read, outside) = lay_store_ground(&store, &w, &commit, true, win, &store_base, 3, &mut surface, &mut inside).unwrap();
    assert_eq!((read, outside), (9, 0));
    // The same surface laid by hand from the reader's tiles: B plus each tile's change at its cells.
    let (mut by_hand, mut hand_inside) = (start.clone(), vec![false; n]);
    for id in tiles() {
        let t = read_l_tile(&store, &w, RES, &spec(7), &commit, id).unwrap().unwrap();
        for p in 0..T * T {
            let (x, y) = (id.ti as usize * T + p % T, id.tj as usize * T + p / T);
            let c = (y - T) * win.w + x - T;
            by_hand[c] = store_base(x as i64, y as i64) + f64::from(t.core[p]);
            hand_inside[c] = true;
        }
    }
    assert_eq!(surface, by_hand, "the store's ground is B + the tile's change at its cells");
    assert_eq!(inside, hand_inside);
    assert!(surface.iter().zip(&start).any(|(a, b)| a != b), "the bake changed the ground");
    // Without the key check, the commit's own keys read the same tiles; another thread count lays the same bits.
    let (mut s2, mut i2) = (start.clone(), vec![false; n]);
    lay_store_ground(&store, &w, &commit, false, win, &store_base, 1, &mut s2, &mut i2).unwrap();
    assert_eq!((s2, i2), (surface.clone(), inside.clone()));
    // The gate and the suite on the store read and on the hand-laid surface: the same report.
    let g = Grid::new(win.w, win.h, 1000.0);
    let outlet = g.outlets(&surface, 0.0);
    let p = GateParams { channel_area_m2: 20.0 * 1000.0 * 1000.0, ..GateParams::default() };
    let gate = |s: &[f64], ins: &[bool]| {
        measure(
            &GateInput {
                grid: &g,
                start: &start,
                surface: s,
                outlet: &outlet,
                inside: ins,
                region: ins,
                h1: &[],
                seed: 7,
                sea_level_m: 0.0,
                outside_drains: true,
                trunks: &[],
            },
            &p,
        )
    };
    let (a, b) = (gate(&surface, &inside), gate(&by_hand, &hand_inside));
    assert_eq!(a, b);
    assert!(a.river_cells > 0 && a.suite.steps > 0, "{:?}", a.suite);
    // A window short of the third column of tiles (window cells 72..96) leaves those 3 outside; a window on another face reads none and is refused.
    let short = LWindow { w: 4 * T - 1, ..win };
    let (mut s3, mut i3) = (vec![0.0; short.w * short.h], vec![false; short.w * short.h]);
    assert_eq!(lay_store_ground(&store, &w, &commit, true, short, &store_base, 2, &mut s3, &mut i3).unwrap(), (6, 3));
    let other = LWindow { face: Face(3), ..win };
    let err = lay_store_ground(&store, &w, &commit, true, other, &store_base, 2, &mut surface, &mut inside).unwrap_err();
    assert!(err.contains("none of the commit's 9 tiles"), "{err}");
}

// spec: TEST-137, BUILD-057
#[test]
fn the_fields_name_every_number_once_with_a_count_before_every_percentage() {
    let r = run(&valley(4.0, 2.0));
    let f = r.fields();
    let names: Vec<&str> = f.iter().map(|(k, _)| *k).collect();
    let mut uniq = names.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), names.len(), "a column name twice: {names:?}");
    for (i, (k, _)) in f.iter().enumerate() {
        if k.ends_with("_pct") {
            let before = f[i - 1].0;
            assert!(!before.ends_with("_pct") && !before.ends_with("_m"), "{k} follows {before}, not its count");
        }
    }
    assert!(f.iter().all(|(_, v)| !v.is_empty() && !v.contains(' ')), "{f:?}");
    let line = r.fields_line();
    assert_eq!(line.strip_prefix("RIVER-GATE fields: ").unwrap().split(' ').count(), f.len());
    assert_eq!(f.iter().find(|(k, _)| *k == "river_cells").unwrap().1, r.river_cells.to_string());
}

// spec: TEST-137, BUILD-057
#[test]
fn a_trunk_over_a_dam_needs_a_burn_and_one_down_a_draining_floor_none() {
    let g = Grid::new(N, N, DX);
    let inside = vec![true; N * N];
    let z = valley(2.0, 2.0);
    // The floor x = 20 from y = 39 (upstream) to y = 0: its surface falls 1 m a cell, so it is its own running minimum.
    let floor = vec![vec![(20.0, 39.0), (20.0, 0.0)]];
    let t = planet_cache::river_suite::trunk_burn(&g, &z, &inside, &floor, 50.0);
    assert_eq!((t.reaches, t.vertices, t.reaches_over, t.vertices_over), (1, 40, 0, 0), "{t:?}");
    // A 60 m dam across the floor at y = 10: the running minimum from upstream is 111 m (y = 11), the dam stands at 170 m: 59 m over it, more than 50.
    let mut dam = z.clone();
    for x in 0..N {
        dam[10 * N + x] += 60.0;
    }
    let d = planet_cache::river_suite::trunk_burn(&g, &dam, &inside, &floor, 50.0);
    assert_eq!((d.reaches_over, d.vertices_over, d.deepest_reach, d.deepest_at), (1, 1, 0, (20.0, 10.0)), "{d:?}");
    assert!((d.deepest_m - 59.0).abs() < 1e-9, "{d:?}");
    // At a bar of 60 m it is not counted; a reach outside the counted cells is not a reach.
    assert_eq!(planet_cache::river_suite::trunk_burn(&g, &dam, &inside, &floor, 60.0).vertices_over, 0);
    let none = vec![false; N * N];
    assert_eq!(planet_cache::river_suite::trunk_burn(&g, &dam, &none, &floor, 50.0).reaches, 0);
    // Through the gate: the trunk line and the fields; no reach in the input says so.
    let outlet = g.outlets(&dam, 0.0);
    let r = measure(
        &GateInput {
            grid: &g,
            start: &z,
            surface: &dam,
            outlet: &outlet,
            inside: &inside,
            region: &inside,
            h1: &[],
            seed: 7,
            sea_level_m: 0.0,
            outside_drains: false,
            trunks: &floor,
        },
        &params(),
    );
    assert!(
        r.trunk_line().contains("1 of 1 H1 trunk reaches") && r.trunk_line().contains("deepest 59.00 m on reach 0"),
        "{}",
        r.trunk_line()
    );
    assert!(r.fields().iter().any(|(k, v)| *k == "trunk_vertices_over" && v == "1"));
    assert_eq!(run(&z).trunk_line(), "RIVER-GATE trunks: no H1 trunk reach in the input");
}

// spec: TEST-137, BUILD-057
#[test]
fn a_corridor_routes_a_trunk_around_a_partial_dam_but_not_through_a_narrow_one() {
    let g = Grid::new(N, N, DX);
    let inside = vec![true; N * N];
    let floor = vec![vec![(20.0, 39.0), (20.0, 0.0)]];
    // A 60 m dam across the valley floor only (x 16..=24) at y = 10; the sides rise 2 m a cell.
    let mut dam = valley(2.0, 2.0);
    for x in 16..=24 {
        dam[10 * N + x] += 60.0;
    }
    let narrow = planet_cache::river_suite::trunk_corridor(&g, &dam, &inside, &floor, 2.0, 50.0, None);
    assert_eq!((narrow.reaches, narrow.reaches_over), (1, 1), "a 2-cell corridor cannot leave the dam: {narrow:?}");
    // The minimax route comes down at x = 18 (its minimum 115 m at y = 11: 104 + y) and steps diagonally onto the dam at (19, 10), 172 m: 57 m, less than the
    // 59 m of crossing straight down the floor (170 m over 111 m).
    assert!((narrow.deepest_m - 57.0).abs() <= 0.11, "{narrow:?}");
    // 6 cells reach x = 14 and 26, outside the dam: the route climbs the side about 10 m and passes.
    let wide = planet_cache::river_suite::trunk_corridor(&g, &dam, &inside, &floor, 6.0, 50.0, None);
    assert_eq!((wide.reaches, wide.reaches_over, wide.vertices_over, wide.deepest_m), (1, 0, 0, 0.0), "{wide:?}");
    assert!(wide.vertices >= 40, "the route's cells inside are counted: {wide:?}");
    // A dam across the whole valley stays over in any corridor.
    let mut full = valley(2.0, 2.0);
    for x in 0..N {
        full[10 * N + x] += 60.0;
    }
    assert_eq!(planet_cache::river_suite::trunk_corridor(&g, &full, &inside, &floor, 20.0, 50.0, None).reaches_over, 1);
    // Through the gate: k_c 1, 2 and 4 of 814 m on 100 m cells reach past the partial dam; the polyline itself does not.
    let outlet = g.outlets(&dam, 0.0);
    let p = params();
    let r = measure(
        &GateInput {
            grid: &g,
            start: &dam,
            surface: &dam,
            outlet: &outlet,
            inside: &inside,
            region: &inside,
            h1: &[],
            seed: 7,
            sea_level_m: 0.0,
            outside_drains: false,
            trunks: &floor,
        },
        &p,
    );
    assert_eq!(r.trunk.reaches_over, 1);
    assert!(r.trunk_corridors.iter().all(|t| t.reaches_over == 0), "{:?}", r.trunk_corridors);
    // The window's bar: half the start's 10 x 10 relief (27 m here), at least 10 m.
    assert_eq!(r.corridor_bar_m, planet_cache::river_suite::corridor_bar_m(r.relief_10_start_m));
    assert!(r.corridor_line().contains("k_c 1 (814 m) 100.00 % of 1 reaches pass"), "{}", r.corridor_line());
    assert!(r.fields().iter().any(|(k, v)| *k == "trunk_c4_reaches_over" && v == "0"));
}

// spec: TEST-137, BUILD-057
#[test]
fn a_corridor_route_bypasses_a_pit_whose_low_minimum_would_make_a_later_rise_a_burn() {
    // A plateau at 100 m. On the trunk's line (y = 10, from x = 2 to x = 37) a pit at 40 m (x 10..=12, y 9..=11) and, further down, a ridge at 145 m across the
    // whole grid (x = 25). Through the pit the ridge stands 105 m above the running minimum; around the pit (y 7 or 13) only 45 m above it.
    let g = Grid::new(N, N, DX);
    let inside = vec![true; N * N];
    let mut z = vec![100.0; N * N];
    for y in 9..=11 {
        for x in 10..=12 {
            z[y * N + x] = 40.0;
        }
    }
    for y in 0..N {
        z[y * N + 25] = 145.0;
    }
    let line = vec![vec![(2.0, 10.0), (37.0, 10.0)]];
    // Along the line itself: 105 m.
    let polyline = planet_cache::river_suite::trunk_burn(&g, &z, &inside, &line, 50.0);
    assert_eq!(polyline.reaches_over, 1);
    assert!((polyline.deepest_m - 105.0).abs() < 1e-9, "{polyline:?}");
    // A corridor of 4 cells lets the route pass beside the pit: the highest running minimum (100 m) keeps the ridge within 45 m.
    let wide = planet_cache::river_suite::trunk_corridor(&g, &z, &inside, &line, 4.0, 50.0, None);
    assert_eq!((wide.reaches, wide.reaches_over, wide.deepest_m), (1, 0, 0.0), "{wide:?}");
    // A corridor of 1 cell holds no way around (the pit spans y 9..=11): the route must cross it, 105 m.
    let narrow = planet_cache::river_suite::trunk_corridor(&g, &z, &inside, &line, 1.0, 50.0, None);
    assert_eq!(narrow.reaches_over, 1, "{narrow:?}");
    assert!((narrow.deepest_m - 105.0).abs() <= 0.11, "{narrow:?}");
    // The same input gives the same bits.
    assert_eq!(planet_cache::river_suite::trunk_corridor(&g, &z, &inside, &line, 4.0, 50.0, None), wide);
    assert_eq!(planet_cache::river_suite::trunk_corridor(&g, &z, &inside, &line, 1.0, 50.0, None), narrow);
}

// spec: TEST-137, BUILD-057
#[test]
fn the_corridor_bar_scales_with_relief_and_snapping_moves_ends_and_crossings_to_their_lowest_cells() {
    use planet_cache::river_suite::{corridor_bar_m, trunk_corridor, Snap};
    // The bar: R1h's start relief 54.9 m gives 27.45 m; a flat window 10 m; a mountain 50 m.
    assert_eq!((corridor_bar_m(54.9), corridor_bar_m(5.0), corridor_bar_m(400.0)), (27.45, 10.0, 50.0));
    let g = Grid::new(N, N, DX);
    let inside = vec![true; N * N];
    let all = vec![true; N * N];
    // A plateau at 100 m; the raw end of the reach, cell (30, 10), on a 170 m spike: the route must end 70 m above its minimum. Snapped, the node moves to the
    // lowest cell within 3 cells (the plateau, ties by cell id) and the reach passes.
    let mut z = vec![100.0; N * N];
    z[10 * N + 30] = 170.0;
    let line = vec![vec![(2.0, 10.0), (30.0, 10.0)]];
    let snap = Snap { node_radius: 3.0, region: &all };
    assert_eq!(trunk_corridor(&g, &z, &inside, &line, 6.0, 50.0, None).reaches_over, 1);
    let s = trunk_corridor(&g, &z, &inside, &line, 6.0, 50.0, Some(&snap));
    assert_eq!((s.reaches, s.reaches_over), (1, 0), "{s:?}");
    // A crossing: the region is x <= 19, its edge column x = 19 a 160 m wall but for a gap at (19, 15), 5 cells off the line. A 6-cell corridor finds the gap from
    // the raw ends; snapped, the reach must cross at the lowest edge cell within 3 cells of H1's crossing, which is the wall, and it fails.
    let mut wall = vec![100.0; N * N];
    for y in 0..N {
        wall[y * N + 19] = 160.0;
    }
    wall[15 * N + 19] = 100.0;
    let region: Vec<bool> = (0..N * N).map(|c| c % N <= 19).collect();
    let across = vec![vec![(4.0, 10.0), (12.0, 10.0), (20.0, 10.0), (28.0, 10.0), (34.0, 10.0)]];
    let raw = trunk_corridor(&g, &wall, &inside, &across, 6.0, 50.0, None);
    assert_eq!(raw.reaches_over, 0, "{raw:?}");
    let snapped = trunk_corridor(&g, &wall, &inside, &across, 6.0, 50.0, Some(&Snap { node_radius: 3.0, region: &region }));
    assert_eq!(snapped.reaches_over, 1, "{snapped:?}");
    assert!((snapped.deepest_m - 60.0).abs() <= 0.11, "{snapped:?}");
}

// spec: TEST-137, BUILD-057
#[test]
fn a_crossing_leg_carries_the_running_minimum_and_a_rise_outside_the_counted_cells_still_fails_the_reach() {
    use planet_cache::river_suite::{trunk_corridor, Snap};
    let g = Grid::new(N, N, DX);
    let all = vec![true; N * N];
    // A plateau at 100 m with a pit band at 60 m across the whole grid (x 6..=8; the plateau after it stands 40 m above its minimum, within the bar) and a ridge at 145 m across it (x = 28). The region is x <= 19, so the snapped
    // reach crosses at an edge cell between the pit and the ridge: carried over, the pit's minimum makes the ridge an 85 m rise; restarted at the crossing it would
    // be 45 m and pass.
    let mut z = vec![100.0; N * N];
    for y in 0..N {
        for x in 6..=8 {
            z[y * N + x] = 60.0;
        }
        z[y * N + 28] = 145.0;
    }
    let region: Vec<bool> = (0..N * N).map(|c| c % N <= 19).collect();
    let line = vec![vec![(2.0, 10.0), (12.0, 10.0), (20.0, 10.0), (28.0, 10.0), (36.0, 10.0)]];
    let s = trunk_corridor(&g, &z, &all, &line, 4.0, 50.0, Some(&Snap { node_radius: 3.0, region: &region }));
    assert_eq!(s.reaches_over, 1, "{s:?}");
    assert!((s.deepest_m - 85.0).abs() <= 0.11, "{s:?}");
    // Only the cells x <= 15 are counted: the ridge lies outside them, yet the reach has no route within the bar and is over; no counted vertex is over.
    let counted: Vec<bool> = (0..N * N).map(|c| c % N <= 15).collect();
    let t = trunk_corridor(&g, &z, &counted, &line, 4.0, 50.0, None);
    assert_eq!((t.reaches, t.reaches_over, t.vertices_over), (1, 1, 0), "{t:?}");
    assert!((t.deepest_m - 85.0).abs() <= 0.11, "{t:?}");
}
