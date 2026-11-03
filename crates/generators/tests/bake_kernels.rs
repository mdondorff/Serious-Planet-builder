//! Tier A, GEN-100 (m9-bake PR 3a): the routing and carve kernels on synthetic surfaces: a pit, a ramp and a cone, with the expected results, the order and the area
//! conserved, the outlets kept, and the same bits on every run. Small grids; no planet bake.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::route::{route, NONE};
use planet_generators::bake::Grid;

const DX: f64 = 800.0;

fn grid(n: usize) -> Grid {
    Grid::new(n, n, DX)
}

/// A tilted plane draining to the west: height `10 i` metres over a sea-level column at `i = 0`.
fn ramp(n: usize) -> Vec<f64> {
    (0..n * n).map(|p| 10.0 * (p % n) as f64).collect()
}

/// A bowl: a plateau at 100 m with a pit of 20 m in its middle, and a notch in the rim at 60 m on the west side that is its pour point.
fn bowl(n: usize) -> Vec<f64> {
    let c = n as f64 / 2.0;
    (0..n * n)
        .map(|p| {
            let (i, j) = ((p % n) as f64, (p / n) as f64);
            let r = libm::hypot(i - c, j - c);
            if r < 3.0 {
                20.0
            } else {
                100.0
            }
        })
        .collect()
}

// spec: GEN-100
#[test]
fn the_flood_fills_a_pit_to_its_pour_point_and_every_cell_drains() {
    let g = grid(24);
    let mut z = bowl(24);
    // The pour point: a channel from the pit's rim to the west edge at 60 m.
    for i in 0..10 {
        z[12 * 24 + i] = 60.0;
    }
    let outlet = g.outlets(&z, -1.0);
    let filled = priority_flood(&g, &z, &outlet, 1e-3);
    for p in 0..z.len() {
        assert!(filled[p] >= z[p], "the flood lowered cell {p}");
        if !outlet[p] {
            // Every cell has a neighbour lower by about epsilon or more on the filled surface: it drains.
            let lower = (0..8).filter_map(|d| g.neighbour(p, d)).any(|q| filled[q] < filled[p]);
            assert!(lower, "cell {p} at {} has no lower neighbour: a pit is left", filled[p]);
        }
    }
    // The pit fills to the channel's height (60 m) plus a little, not to the plateau's 100 m.
    let pit = 12 * 24 + 12;
    assert!(filled[pit] > 59.9 && filled[pit] < 61.0, "the pit fills to {}", filled[pit]);
}

// spec: GEN-100
#[test]
fn a_ramp_drains_downhill_the_order_follows_the_receivers_and_the_area_is_conserved() {
    let n = 20;
    let g = grid(n);
    let z = ramp(n);
    // The sea is the west column and the grid's edge cells are outlets too.
    let outlet = g.outlets(&z, 0.0);
    let r = route(&g, &z, &outlet, 1e-3, 7, 0.0);
    for (p, &is_outlet) in outlet.iter().enumerate() {
        if is_outlet {
            assert_eq!(r.receiver[p], NONE);
        } else {
            let rec = r.receiver[p];
            assert!(rec != NONE && r.filled[rec as usize] < r.filled[p], "cell {p} drains uphill or nowhere");
        }
    }
    // Outlets first, each cell after its receiver.
    let mut at = vec![0usize; n * n];
    for (k, &p) in r.order.iter().enumerate() {
        at[p as usize] = k;
    }
    assert_eq!(r.order.len(), n * n);
    for p in 0..n * n {
        if r.receiver[p] != NONE {
            assert!(at[r.receiver[p] as usize] < at[p], "cell {p} comes before its receiver");
        }
    }
    // The area leaving the outlets is the area of the grid (each cell counts once).
    let total: f64 = (0..n * n).filter(|&p| outlet[p]).map(|p| r.area[p]).sum();
    assert!((total - (n * n) as f64 * DX * DX).abs() < 1.0, "area {total} against {}", (n * n) as f64 * DX * DX);
    // A cap on the area holds.
    let capped = route(&g, &z, &outlet, 1e-3, 7, 4.0 * DX * DX);
    assert!(capped.area.iter().all(|&a| a <= 4.0 * DX * DX + 1e-9));
}

// spec: GEN-100
#[test]
fn the_carve_keeps_the_outlets_stays_above_the_receivers_is_the_same_bits_on_every_run_and_uplifts_the_land() {
    let n = 32;
    let g = grid(n);
    // A cone: 3000 m at the middle, falling to 0 at the edge.
    let c = (n as f64 - 1.0) / 2.0;
    let cone: Vec<f64> = (0..n * n)
        .map(|p| {
            let r = libm::hypot((p % n) as f64 - c, (p / n) as f64 - c);
            (3000.0 * (1.0 - r / c)).max(0.0)
        })
        .collect();
    let outlet = g.outlets(&cone, 0.0);
    let params = CarveParams::default();
    let run = || {
        let mut z = cone.clone();
        let r = carve(&g, &mut z, &outlet, &params, None, 11);
        (z, r)
    };
    let (z, r) = run();
    let (z2, r2) = run();
    assert_eq!(
        z.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        z2.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "the carve differs between runs"
    );
    assert_eq!(r, r2);
    let uplift = params.u_over_k * params.k_dt * params.steps as f64;
    for p in 0..n * n {
        if outlet[p] {
            assert_eq!(z[p].to_bits(), cone[p].to_bits(), "an outlet moved");
        } else {
            assert!(z[p] <= cone[p] + uplift + 1e-9, "cell {p} rose by more than the uplift");
            let rec = r.receiver[p];
            if rec != NONE {
                assert!(z[p] > z[rec as usize], "cell {p} is not above its receiver after the carve");
            }
        }
    }
    // The trunk (the cells of the largest area) is cut deeper than the flanks: the carve incises where the water runs.
    let (trunk, _) = (0..n * n).filter(|&p| !outlet[p]).map(|p| (p, r.area[p])).fold((0, 0.0), |b, x| if x.1 > b.1 { x } else { b });
    assert!(r.area[trunk] > 10.0 * DX * DX, "the cone has a trunk of {} cells", r.area[trunk] / (DX * DX));
    assert!(cone[trunk] - z[trunk] > 0.0, "the trunk was not incised");
}

// spec: GEN-100
#[test]
fn a_breach_lowers_a_dam_between_a_basin_and_the_sea_by_at_most_the_limit() {
    let n = 24;
    let g = grid(n);
    // A basin at 200 m east of a dam of 230 m, with the sea (0 m) beyond: the dam is 30 m above the basin floor.
    let mut z = vec![0.0f64; n * n];
    for j in 0..n {
        for i in 0..n {
            z[j * n + i] = if i < 3 {
                0.0
            } else if i == 5 {
                230.0
            } else if i < 5 {
                50.0
            } else {
                200.0
            };
        }
    }
    let outlet = g.outlets(&z, 0.0);
    let before = z.clone();
    let mut after = z;
    let params = CarveParams { steps: 1, ..CarveParams::default() };
    carve(&g, &mut after, &outlet, &params, None, 3);
    for p in 0..n * n {
        assert!(
            before[p] - after[p] <= params.breach_max_m + params.u_over_k * params.k_dt + 1e-9,
            "cell {p} was lowered by {}",
            before[p] - after[p]
        );
    }
    // The dam crest is cut towards the basin floor by the breach (up to the limit), not left as a wall.
    let crest = 12 * n + 5;
    assert!(after[crest] < before[crest], "the dam was not breached");
}
