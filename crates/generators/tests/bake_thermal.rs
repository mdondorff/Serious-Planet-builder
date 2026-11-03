//! Tier A, GEN-101 (m9-bake PR 3b): thermal stabilisation on active cells and the deposition sweep on synthetic ground. The active-cell run is bit for bit the run over every
//! cell, the mass is kept, any thread count gives the same bytes, the border and the sea only receive, ground under the critical slope is left alone, a tower relaxes to
//! the critical slope, and the deposition fills a pit, keeps every receiver lower and loses no volume. Small grids; no store.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::deposit::{deposit, deposit_on_own_route, DepositParams, Kc};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::route::{route, NONE};
use planet_generators::bake::thermal::{thermal, thermal_all_cells, ThermalParams};
use planet_generators::bake::Grid;

const DX: f64 = 100.0;

fn bits(z: &[f64]) -> Vec<u64> {
    z.iter().map(|v| v.to_bits()).collect()
}

/// A plateau of 100 m with a tower of `height` m at the centre.
fn tower(n: usize, height: f64) -> Vec<f64> {
    let mut z = vec![100.0; n * n];
    z[(n / 2) * n + n / 2] += height;
    z
}

/// Heights from an integer hash: steep everywhere, 100 to 700 m.
fn noise(n: usize) -> Vec<f64> {
    (0..n * n)
        .map(|p| {
            let x = (p as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let x = (x ^ (x >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            100.0 + 600.0 * ((x >> 11) as f64 / (1u64 << 53) as f64)
        })
        .collect()
}

// spec: GEN-101
#[test]
fn the_active_cell_run_is_the_run_over_every_cell_and_costs_less() {
    let g = Grid::new(64, 64, DX);
    let p = ThermalParams { passes: 80, ..ThermalParams::default() };
    let (mut a, mut b) = (tower(64, 900.0), tower(64, 900.0));
    let ra = thermal(&g, &mut a, &p, 2);
    let rb = thermal_all_cells(&g, &mut b, &p, 2);
    assert_eq!(bits(&a), bits(&b), "the active-cell run differs from the full one");
    assert_eq!((ra.passes_run, ra.moved_last), (rb.passes_run, rb.moved_last));
    assert!(ra.evaluations * 4 < rb.evaluations, "{} evaluations against {}", ra.evaluations, rb.evaluations);
    let (mut a, mut b) = (noise(40), noise(40));
    let g = Grid::new(40, 40, DX);
    thermal(&g, &mut a, &p, 3);
    thermal_all_cells(&g, &mut b, &p, 3);
    assert_eq!(bits(&a), bits(&b), "the active-cell run differs from the full one on rough ground");
}

// spec: GEN-101
#[test]
fn thermal_keeps_the_mass_and_gives_the_same_bytes_on_1_and_8_threads() {
    let g = Grid::new(48, 48, DX);
    let p = ThermalParams { passes: 60, ..ThermalParams::default() };
    let z0 = noise(48);
    let (mut one, mut eight) = (z0.clone(), z0.clone());
    thermal(&g, &mut one, &p, 1);
    thermal(&g, &mut eight, &p, 8);
    assert_eq!(bits(&one), bits(&eight));
    let (before, after): (f64, f64) = (z0.iter().sum(), one.iter().sum());
    assert!((before - after).abs() < 1e-9 * before, "the mass changed from {before} to {after}");
    assert!(one != z0, "nothing moved on rough ground");
}

// spec: GEN-101
#[test]
fn ground_under_the_critical_slope_is_left_alone_and_a_tower_relaxes_to_it() {
    let g = Grid::new(48, 48, DX);
    let p = ThermalParams::default();
    // A ramp of 20 degrees (under 33 degrees less the perturbation's 25 %: 26 degrees).
    let mut ramp: Vec<f64> = (0..48 * 48).map(|q| 50.0 + (q % 48) as f64 * DX * libm::tan(20f64.to_radians())).collect();
    let r0 = ramp.clone();
    let r = thermal(&g, &mut ramp, &p, 2);
    assert_eq!((ramp, r.passes_run, r.moved_last), (r0, 1, 0));
    // A tower of 900 m settles: the run ends before its cap and no land pair stays over the largest critical slope.
    let mut z = tower(48, 900.0);
    let long = ThermalParams { passes: 20_000, ..p };
    let r = thermal(&g, &mut z, &long, 2);
    assert!(r.passes_run < 20_000 && r.moved_last == 0, "the tower did not settle in {} passes", r.passes_run);
    let crit = libm::tan(33f64.to_radians()) * 1.25;
    for q in 0..48 * 48 {
        for d in 0..8 {
            if let Some(o) = g.neighbour(q, d) {
                if !g.is_edge(q) {
                    let slope = (z[q] - z[o]) / g.distance(d);
                    assert!(slope <= crit + 1e-5, "cell {q} stands over its neighbour {o} at a slope of {slope}");
                }
            }
        }
    }
}

// spec: GEN-101
#[test]
fn the_border_and_the_sea_only_receive() {
    let g = Grid::new(32, 32, DX);
    // A tower next to the border and a sea cell (below 0) next to a tower.
    let mut z = vec![100.0; 32 * 32];
    z[5 * 32 + 1] += 900.0;
    z[20 * 32 + 20] += 900.0;
    z[20 * 32 + 21] = -10.0;
    let z0 = z.clone();
    thermal(&g, &mut z, &ThermalParams { passes: 300, ..ThermalParams::default() }, 2);
    for q in 0..32 * 32 {
        if g.is_edge(q) || z0[q] <= 0.0 {
            assert!(z[q] >= z0[q], "the border or sea cell {q} lost {} m", z0[q] - z[q]);
        }
    }
    assert!(z[5 * 32] > z0[5 * 32] && z[20 * 32 + 21] > z0[20 * 32 + 21], "the sinks received nothing");
}

// spec: GEN-101
#[test]
#[should_panic(expected = "share moved per pass")]
fn a_share_that_could_overshoot_is_refused() {
    let g = Grid::new(8, 8, DX);
    thermal(&g, &mut tower(8, 500.0), &ThermalParams { k: 0.5, ..ThermalParams::default() }, 1);
}

/// A ramp falling towards `x = 0` (the outlet column) with an eroded strip upstream and a pit in the middle.
fn valley() -> (Grid, Vec<f64>, Vec<f64>, usize) {
    let (n, g) = (48usize, Grid::new(48, 48, DX));
    let mut orig: Vec<f64> = (0..n * n).map(|p| 5.0 + (p % n) as f64 * 20.0).collect();
    let mut z = orig.clone();
    // The ground upstream (x from 30 on, rows 20 to 28) was 15 m higher before the carve.
    for j in 20..28 {
        for i in 30..n {
            orig[j * n + i] += 15.0;
        }
    }
    // A pit of 12 m at (15, 24) that the flood fills.
    let pit = 24 * n + 15;
    z[pit] -= 12.0;
    orig[pit] = z[pit] + 12.0;
    (g, orig, z, pit)
}

// spec: GEN-101
#[test]
fn deposition_fills_a_pit_loses_no_volume_and_keeps_every_receiver_lower() {
    let (g, orig, mut z, pit) = valley();
    let (z0, outlet) = (z.clone(), g.outlets(&z, 0.0));
    let filled = priority_flood(&g, &z, &outlet, 1e-3);
    let routing = route(&g, &z, &outlet, 1e-3, 7, 0.0);
    let params = DepositParams { kc: Kc::Uniform(1e5), zeta: 0.5, sea_level: 0.0 };
    let r = deposit(&g, &mut z, &orig, &filled, &routing, &params);
    assert!(z[pit] > z0[pit], "the pit was not filled");
    assert!(
        (r.eroded - (r.deposited + r.exported)).abs() < 1e-9 * r.eroded.max(1.0),
        "{r:?}: what was eroded is not what was deposited and exported"
    );
    assert!(r.deposited > 0.0);
    for p in 0..g.len() {
        let rec = routing.receiver[p];
        if rec != NONE {
            assert!(z[p] >= z0[p], "cell {p} was lowered");
            let drop = filled[p] - filled[rec as usize];
            assert!(z[p] <= filled[p] + 0.5 * drop.max(0.0) + 1e-9, "cell {p} rose above the filled level and half the drop");
        }
    }
}

// spec: GEN-101
#[test]
fn deposition_on_a_carved_surface_keeps_the_mass_and_a_zero_zeta_fills_only_depressions() {
    let g = Grid::new(40, 40, DX);
    let orig: Vec<f64> = (0..40 * 40)
        .map(|p| {
            let (i, j) = ((p % 40) as f64, (p / 40) as f64);
            20.0 + i * 25.0 + 150.0 * libm::sin(i * 0.4) * libm::cos(j * 0.3)
        })
        .collect();
    let mut z = orig.clone();
    let outlet = g.outlets(&z, 0.0);
    carve(&g, &mut z, &outlet, &CarveParams::default(), None, 3);
    let carved = z.clone();
    let on = |kc: f64, zeta: f64, z: &mut Vec<f64>| {
        deposit_on_own_route(&g, z, &orig, 1e-3, 3, &DepositParams { kc: Kc::Uniform(kc), zeta, sea_level: 0.0 })
    };
    let mut a = carved.clone();
    let r = on(1e5, 0.5, &mut a);
    assert!((r.eroded - (r.deposited + r.exported)).abs() < 1e-9 * r.eroded.max(1.0), "{r:?}");
    let mut b = carved.clone();
    let none = on(1e5, 0.0, &mut b);
    assert!(none.deposited <= r.deposited + 1e-9, "a zero zeta deposited more ({} against {})", none.deposited, r.deposited);
    // A capacity that nothing exceeds leaves only the filling of depressions.
    let mut c = carved.clone();
    let big = on(1e30, 0.5, &mut c);
    assert!(big.deposited <= none.deposited + 1e-9, "an unreachable capacity deposited more than a zero zeta");
    for q in 0..g.len() {
        assert!(a[q] >= carved[q] && b[q] >= carved[q] && c[q] >= carved[q], "cell {q} was lowered");
    }
}
