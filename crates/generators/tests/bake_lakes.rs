//! Tier A, GEN-163 (TEST-108): the lake producer. A bowl with a known sill, a river through it and a playa: the level of the balance in the three cases, the bounds, the records, the dropped channel cells
//! and the determinism. Small grids; no store read.

#![allow(clippy::needless_range_loop)]
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, BasinClass, Hydro, HydroParams};
use planet_generators::bake::lakes::{lakes_from_hydro, Lakes};
use planet_generators::bake::route::{route_rules, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;

const W: usize = 40;
const EPS: f64 = 1e-3;

/// A slope rising to the east with a bowl at (10..16, 10..16): the floor at (12, 12), 36 cells, rim 590 m on the west.
fn bowl_land() -> Vec<f64> {
    let mut z: Vec<f64> = (0..W * W).map(|p| 500.0 + 10.0 * (p % W) as f64).collect();
    for j in 10..16 {
        for i in 10..16 {
            z[j * W + i] = 300.0;
        }
    }
    z[12 * W + 12] = 280.0;
    z
}

struct Run {
    g: Grid,
    routing: Routing,
    hydro: Hydro,
}

fn run(z: &[f64], runoff: Option<f32>, evap: Option<f32>, p: &HydroParams) -> Run {
    let g = Grid::new(W, W, 800.0);
    let outlet = g.outlets(z, 0.0);
    let routing = route_rules(&g, z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, z, &outlet, EPS, true);
    let r = runoff.map(|v| vec![v; W * W]);
    let e = evap.map(|v| vec![v; W * W]);
    let hydro = hydrology(&g, z, &filled, &routing, &outlet, r.as_deref(), e.as_deref(), p);
    Run { g, routing, hydro }
}

fn small_t() -> HydroParams {
    HydroParams { initiation_m2: 3.0 * 800.0 * 800.0, ..HydroParams::default() }
}

fn lakes(r: &Run) -> Lakes {
    lakes_from_hydro(&r.g, &r.routing, &r.hydro)
}

// spec: GEN-163, TEST-108
#[test]
fn the_level_is_the_balance_in_the_three_cases() {
    let z = bowl_land();
    // Overflow: no evaporation, the level is the sill and everything continues.
    let r = run(&z, None, None, &small_t());
    let l = &lakes(&r).lakes[0];
    assert_eq!((l.class, l.overflows, l.wet), (BasinClass::Lake, true, true));
    assert_eq!(l.level_m, l.spill_m, "full to the sill");
    assert!((l.q_out - l.q_in).abs() <= 1e-9 * l.q_in);
    assert_eq!(l.area_m2, r.hydro.basins[0].area_m2[15], "the surface at the sill is the curve's last step");
    assert!((l.area_m2 - 36.0 * r.g.cell_area(0)).abs() < 1.0, "the surface at the sill is the bowl");
    // Below the sill: evaporation takes all of it; the level is where A(h) E meets the inflow, hand-computed between the 1st and 2nd step.
    let r = run(&z, Some(1.0e-8), Some(1.0e-6), &small_t());
    let (b, l) = (&r.hydro.basins[0], &lakes(&r).lakes[0]);
    let (h1, h2) = (b.floor_m + (b.spill_m - b.floor_m) / 16.0, b.floor_m + 2.0 * (b.spill_m - b.floor_m) / 16.0);
    let (w1, w2) = (b.area_m2[0] * 1.0e-6, b.area_m2[1] * 1.0e-6);
    let want = h1 + (b.q_in - w1) / (w2 - w1) * (h2 - h1);
    assert!(w1 < b.q_in && b.q_in <= w2, "between the 1st and 2nd step");
    assert_eq!((l.class, l.overflows, l.wet, l.q_out), (BasinClass::Endorheic, false, true, 0.0));
    assert!((l.level_m - want).abs() <= 1e-9 * want.abs(), "level {} against {want}", l.level_m);
    let a = l.area_m2;
    let e_open = f64::from(1.0e-6f32); // the evaporation field is f32
    assert!((a * e_open - b.q_in).abs() <= 1e-9 * b.q_in, "A(h) E meets the inflow: {} against {}", a * e_open, b.q_in);
    assert!(b.area_m2[0] < a && a <= b.area_m2[1], "the area at the level lies on the curve between the steps: {a}");
    // Q_in = 0: a playa, the floor and no water surface.
    let r = run(&z, Some(0.0), Some(1.0e-6), &small_t());
    let l = &lakes(&r).lakes[0];
    assert_eq!((l.q_in, l.wet, l.level_m, l.area_m2), (0.0, false, l.floor_m, 0.0));
}

// spec: GEN-163, TEST-108
#[test]
fn the_level_stays_between_the_floor_and_the_sill_and_the_outflow_follows_the_overflow() {
    let z = bowl_land();
    let cases = [
        (None, None),
        (Some(1.0e-8f32), Some(1.0e-6f32)),
        (Some(1.0e-8), Some(1.0e-9)),
        (Some(0.0), Some(1.0e-6)),
        (Some(1.0e-8), Some(3.0e-7)),
    ];
    for (runoff, evap) in cases {
        let r = run(&z, runoff, evap, &small_t());
        for l in &lakes(&r).lakes {
            assert!(
                l.floor_m <= l.level_m && l.level_m <= l.spill_m,
                "level {} outside [{}, {}] for {runoff:?} {evap:?}",
                l.level_m,
                l.floor_m,
                l.spill_m
            );
            assert_eq!(l.q_out > 0.0, l.overflows, "q_out is positive iff the basin overflows ({runoff:?} {evap:?})");
            assert!(l.q_out <= l.q_in + 1e-12 * l.q_in);
            assert!(l.area_m2 >= 0.0 && l.area_m2 <= 36.0 * r.g.cell_area(0) * (1.0 + 1e-9));
        }
    }
}

// spec: GEN-163, TEST-108
#[test]
fn one_lake_per_kept_basin_and_none_for_an_artefact_or_a_ramp() {
    let mut dent = bowl_land();
    dent[30 * W + 30] = 789.0; // a one-cell dent a metre deep: an artefact
    let r = run(&dent, None, None, &small_t());
    assert!(r.hydro.stats.artefacts >= 1);
    let ls = lakes(&r);
    let mut ids: Vec<u64> = r.hydro.basins.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    assert_eq!(ls.lakes.iter().map(|l| l.id).collect::<Vec<_>>(), ids, "the lakes are the kept basins, by id");
    assert_eq!(ls.lakes.len(), 1);
    // A flat plain ramps by the flood's epsilon: no basin, no lake.
    let g = Grid::new(1200, 1200, 800.0);
    let z = vec![10.0; 1200 * 1200];
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let hydro = hydrology(&g, &z, &filled, &routing, &outlet, None, None, &HydroParams::default());
    assert!(hydro.stats.ramps >= 1);
    assert!(lakes_from_hydro(&g, &routing, &hydro).lakes.is_empty());
}

// spec: GEN-163, TEST-108
#[test]
fn no_channel_cell_stays_inside_a_kept_lake_and_the_river_continues_at_the_spill() {
    let r = run(&bowl_land(), None, None, &small_t());
    let ls = lakes(&r);
    assert!(ls.dropped_cells > 0, "a channel passes through the bowl: {:?}", r.hydro.stats);
    let total: usize = r.hydro.segments.iter().map(|s| s.cells as usize).sum();
    let kept: usize = ls.pieces.iter().map(|p| p.cells.len()).sum();
    assert_eq!(kept + ls.dropped_cells, total, "every segment cell is in a piece or dropped");
    for p in &ls.pieces {
        for &c in &p.cells {
            assert_eq!(r.hydro.label[c as usize], NONE, "piece cell {c} of segment {} is inside a lake", p.segment);
            assert!(r.hydro.channel[c as usize]);
        }
        assert!(p.cells.windows(2).all(|w| r.routing.receiver[w[0] as usize] == w[1]), "a piece runs down the receivers");
    }
    // The river leaves the lake at its spill cell with the outflow added.
    let b = &r.hydro.basins[0];
    let after = ls.pieces.iter().find(|p| p.cells[0] == b.spill_cell).expect("a piece starts at the spill cell");
    assert!(after.q_first >= b.q_out * (1.0 - 1e-9), "Q_w at the spill cell {} carries q_out {}", after.q_first, b.q_out);
    // The piece before ends at the last cell outside the basin: its receiver is a member.
    let last = |p: &&planet_generators::bake::lakes::RiverPiece| *p.cells.last().unwrap() as usize;
    let before: Vec<_> = ls.pieces.iter().filter(|p| r.hydro.label[r.routing.receiver[last(p)] as usize] != NONE).collect();
    assert!(!before.is_empty(), "a piece ends where the river enters the lake");
    for p in before {
        assert_eq!(p.q_last, r.hydro.qw[last(&p)]);
    }
    // Parts of a segment are numbered from zero without gaps.
    for s in &r.hydro.segments {
        let parts: Vec<u32> = ls.pieces.iter().filter(|p| p.segment == s.id).map(|p| p.part).collect();
        assert!(parts.iter().enumerate().all(|(k, &q)| q as usize == k));
    }
}

// spec: GEN-163, CON-14
#[test]
fn the_result_does_not_depend_on_the_order_of_the_basins() {
    // Two bowls; the basin list of the hydrology is reversed (labels remapped): the lakes and the pieces are the same.
    let mut z = bowl_land();
    for j in 26..31 {
        for i in 24..30 {
            z[j * W + i] = 380.0;
        }
    }
    z[28 * W + 26] = 360.0;
    let r = run(&z, None, None, &small_t());
    assert_eq!(r.hydro.basins.len(), 2);
    let want = lakes(&r);
    let mut swapped = Run { g: Grid::new(W, W, 800.0), routing: r.routing.clone(), hydro: clone_hydro(&r.hydro) };
    let n = swapped.hydro.basins.len();
    swapped.hydro.basins.reverse();
    for l in &mut swapped.hydro.label {
        if *l != NONE {
            *l = (n as u32 - 1) - *l;
        }
    }
    assert_eq!(lakes(&swapped), want);
    assert!(want.lakes.windows(2).all(|p| p[0].id < p[1].id), "the lakes are in the order of their id");
}

fn clone_hydro(h: &Hydro) -> Hydro {
    Hydro {
        q: h.q.clone(),
        qw: h.qw.clone(),
        channel: h.channel.clone(),
        segments: h.segments.clone(),
        basins: h.basins.clone(),
        label: h.label.clone(),
        stats: h.stats.clone(),
    }
}

// spec: GEN-163
#[test]
#[should_panic(expected = "the producer needs Q_w")]
fn a_released_discharge_is_refused_with_a_clear_message() {
    let mut r = run(&bowl_land(), None, None, &small_t());
    r.hydro.qw = Vec::new();
    lakes(&r);
}
