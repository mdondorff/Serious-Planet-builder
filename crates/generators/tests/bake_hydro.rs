//! Tier A, GEN-145 (A3 sections 5.2 to 5.5, gate G-CON): the channel graph (initiation, continuation downstream, segments, confluences, the discharge `Q` conserved), the basins (floor, spill point, area
//! curve, class, water level, identity) and the conservation of the area, on plane grids and on the cube sphere. Small grids; no store read.

#![allow(clippy::needless_range_loop)]
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{basin_id, hydrology, segment_id, verify, BasinClass, HydroParams};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const N: usize = 72;
const R: f64 = 6_371_000.0;
const EPS: f64 = 1e-3;

/// A land of ridges with a coast plain; `terraced` quantises it into flats and basins.
fn terrain(salt: u64, terraced: bool) -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let h = planet_generators::bake::route::jitter(salt, (j * N + i) as u64, 0);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 12.0).min(1.0);
            let v = (350.0
                + 500.0 * libm::sin(x * 0.13 + salt as f64) * libm::cos(y * 0.11)
                + 250.0 * libm::sin(0.37 * x + 0.21 * y)
                + 30.0 * h)
                * edge
                + 2.0;
            z[j * N + i] = if terraced { libm::floor(v / 60.0) * 60.0 + 2.0 + 0.3 * h.abs() } else { v };
        }
    }
    z
}

fn small_t(g: &Grid) -> HydroParams {
    HydroParams { initiation_m2: 25.0 * g.cell_area(0), ..HydroParams::default() }
}

// spec: GEN-145
#[test]
fn the_channel_graph_is_connected_downhill_and_conserves_discharge() {
    for terraced in [false, true] {
        for with_runoff in [false, true] {
            let g = Grid::new(N, N, 800.0);
            let z = terrain(9, terraced);
            let outlet = g.outlets(&z, 0.0);
            let runoff: Vec<f32> = (0..z.len()).map(|p| 0.5 + (p % 7) as f32 * 0.25).collect();
            let rr = with_runoff.then_some(&runoff[..]);
            let routing = route_rules(&g, &z, &outlet, EPS, 9, 0.0, None, RoundRules::ROUND);
            let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
            let params = small_t(&g);
            let h = hydrology(&g, &z, &filled, &routing, &outlet, rr, None, &params);
            let v = verify(&g, &h, &routing, &filled, &outlet, rr, None);
            let ctx = format!("terraced {terraced}, runoff {with_runoff}");
            assert!(h.stats.segments > 3 && h.stats.confluences > 0 && h.stats.heads > 3, "a river network ({ctx}): {:?}", h.stats);
            assert_eq!(v.disconnected_segments, 0, "G-CON disconnected segments ({ctx})");
            assert_eq!(v.broken_channel_cells, 0, "G-CON channel cells without a downhill channel receiver ({ctx})");
            assert_eq!(v.unaccounted_cells, 0, "every channel cell is in one segment or is a mouth ({ctx})");
            assert!(v.max_confluence_error <= 1e-9, "Q at confluences {} ({ctx})", v.max_confluence_error);
            assert!(v.q_outlets_error <= 1e-9, "Q at the outlets {} ({ctx})", v.q_outlets_error);
            assert!(v.area_outlets_error <= 1e-9, "A_d at the outlets {} ({ctx})", v.area_outlets_error);
            // Initiation: every cell at or above the threshold is a channel cell and every head is initiated.
            for p in 0..z.len() {
                if !outlet[p] && routing.area[p] >= params.initiation_m2 {
                    assert!(h.channel[p], "an initiated cell {p} is a channel cell");
                }
            }
            // A channel continues where the diffuse area falls below the threshold, and every head is an initiated cell.
            let continues =
                (0..z.len()).filter(|&p| h.channel[p] && routing.receiver[p] != NONE && routing.area[p] < params.initiation_m2).count();
            assert!(continues > 0, "a channel continues below the threshold ({ctx})");
            for s in &h.segments {
                assert!(s.q_down >= s.q_up, "Q does not fall along a segment");
                let head = (0..z.len()).filter(|&p| p == s.up as usize).all(|p| routing.area[p] >= params.initiation_m2 || h.q[p] > 0.0);
                assert!(head);
            }
            let heads = (0..z.len()).filter(|&p| h.channel[p] && routing.area[p] >= params.initiation_m2).count();
            assert!(heads >= h.stats.heads, "every head is an initiated cell ({ctx})");
            let again = hydrology(&g, &z, &filled, &routing, &outlet, rr, None, &params);
            assert_eq!(again.segments, h.segments, "the ids and the graph are the same on a second run ({ctx})");
        }
    }
}

// spec: GEN-145
#[test]
fn rivers_cross_the_cube_edges_and_the_area_is_conserved() {
    let n = 24;
    let g = Grid::cube(n, R);
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z: Vec<f64> = (0..g.len())
        .map(|p| {
            let d = cube.direction(p as u32);
            let s = d.x * 0.2 - d.y * 0.3 - d.z * 0.93;
            1800.0 * (1.0 - s)
                + 120.0 * libm::sin(9.0 * d.x + 4.0 * d.y) * libm::cos(7.0 * d.z)
                + 3.0 * planet_generators::bake::route::jitter(5, p as u64, 0)
                - 400.0 * (s - 0.85).max(0.0) * 8.0
                - 20.0
        })
        .collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 5, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let params = HydroParams { initiation_m2: 30.0 * g.cell_area(0), ..HydroParams::default() };
    let h = hydrology(&g, &z, &filled, &routing, &outlet, None, None, &params);
    let v = verify(&g, &h, &routing, &filled, &outlet, None, None);
    let crossing = (0..g.len())
        .filter(|&p| h.channel[p] && routing.receiver[p] != NONE && p / (n * n) != routing.receiver[p] as usize / (n * n))
        .count();
    assert!(crossing > 0, "a channel crosses a cube edge");
    assert!(h.segments.iter().any(|s| s.up as usize / (n * n) != s.down as usize / (n * n)), "a segment spans a cube edge");
    assert_eq!((v.disconnected_segments, v.broken_channel_cells, v.unaccounted_cells), (0, 0, 0));
    assert!(v.max_confluence_error <= 1e-9 && v.q_outlets_error <= 1e-9 && v.area_outlets_error <= 1e-9, "{v:?}");
}

/// A 40 x 40 slope rising to the east with a bowl at (10..16, 10..16): the floor at (12, 12), 36 cells.
fn bowl_land() -> Vec<f64> {
    let w = 40;
    let mut z: Vec<f64> = (0..w * w).map(|p| 500.0 + 10.0 * (p % w) as f64).collect();
    for j in 10..16 {
        for i in 10..16 {
            z[j * w + i] = 300.0;
        }
    }
    z[12 * w + 12] = 280.0;
    z
}

// spec: GEN-145
#[test]
fn a_basin_has_floor_spill_curve_class_and_an_identity_of_its_lowest_cell() {
    let w = 40;
    let g = Grid::new(w, w, 800.0);
    let run = |z: &[f64], e: Option<&[f32]>| {
        let outlet = g.outlets(z, 0.0);
        let routing = route_rules(&g, z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
        let filled = priority_flood_with(&g, z, &outlet, EPS, true);
        hydrology(&g, z, &filled, &routing, &outlet, None, e, &HydroParams::default())
    };
    let z = bowl_land();
    let h = run(&z, None);
    assert_eq!(h.basins.len(), 1);
    let b = &h.basins[0];
    assert_eq!(b.floor_cell as usize, 12 * w + 12);
    assert_eq!(b.floor_m, 280.0);
    assert!((b.spill_m - 590.0).abs() < 0.5, "the spill level is the lowest cell of the rim: {}", b.spill_m);
    assert_eq!(b.cells, 36);
    let rim = [(9usize, 9usize), (9, 10), (9, 11), (9, 12), (9, 13), (9, 14), (9, 15), (9, 16)];
    assert!(
        rim.iter().any(|&(i, j)| b.spill_cell as usize == j * w + i),
        "the spill point is a cell of the west rim, cell {}",
        b.spill_cell
    );
    assert!(b.area_m2.windows(2).all(|p| p[0] <= p[1]) && b.volume_m3.windows(2).all(|p| p[0] < p[1]), "the curve rises");
    assert!((b.area_m2[15] - 36.0 * g.cell_area(0)).abs() < 1.0, "the area at the spill is the bowl");
    assert_eq!(b.class, BasinClass::Lake, "no evaporation: it overflows");
    assert!(b.q_in > 0.0 && (b.q_out - b.q_in).abs() <= 1e-9 * b.q_in);
    assert_eq!(b.water_level_m, b.spill_m);
    // Strong evaporation: no active outlet, the water level sits between the floor and the spill, nothing continues downstream.
    let evap: Vec<f32> = vec![1.0e3; w * w];
    let dry = run(&z, Some(&evap));
    let d = &dry.basins[0];
    assert_eq!(d.class, BasinClass::Endorheic);
    assert!(d.water_level_m >= d.floor_m && d.water_level_m < d.spill_m && d.q_out == 0.0);
    // Identity: a pure function of the lowest cell. An edit elsewhere, or deeper in the same cell, keeps it; a new lowest cell moves it.
    let mut far = z.clone();
    far[30 * w + 30] += 7.0;
    assert_eq!(run(&far, None).basins[0].id, b.id);
    let mut deeper = z.clone();
    deeper[12 * w + 12] -= 5.0;
    assert_eq!(run(&deeper, None).basins[0].id, b.id);
    let mut moved = z.clone();
    moved[11 * w + 11] = 270.0;
    let m = run(&moved, None);
    assert_ne!(m.basins[0].id, b.id);
    assert_eq!(m.basins[0].id, basin_id(0, g.global_id(11 * w + 11)));
    assert_ne!(basin_id(0, 5), segment_id(0, 5, 0), "the kinds of identity are apart");
    // A one-cell dent of a metre is an artefact: no object, counted.
    let mut dent = bowl_land();
    dent[30 * w + 30] = 789.0;
    let a = run(&dent, None);
    assert_eq!(a.basins.len(), 1);
    assert!(a.stats.artefacts >= 1, "the dent is an artefact");
}
