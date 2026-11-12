//! Tier A, GEN-166 PR 2 (TEST-120): the f32-stored round surface. The readers of the plan take heights through `Heights`: on f32-representable heights the f32 slice and the f64 slice give the same bits through the
//! flood, the weighted route, H1, the river pieces and the channel graph; under `round_f32` the plan stores the heights f32 from the rounds to the sediment pass and widens them for the sediment pass, and the
//! result is the f64 plan's. Small grids; no store read.

use planet_generators::bake::build::{build_resident, PlanParams};
use planet_generators::bake::channel_graph::ChannelGraph;
use planet_generators::bake::flood::{priority_flood_ungraded, priority_flood_with};
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::lakes::river_pieces;
use planet_generators::bake::route::{jitter, route_rules_filled, RoundRules};
use planet_generators::bake::{Grid, Heights};
use std::sync::Mutex;

const N: usize = 48;

/// A rolling land with pits and flats, every height an f32 value (round-trips through f32).
fn surface() -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 6.0).min(1.0);
            let h = 400.0 + 150.0 * libm::sin(x * 0.21) * libm::cos(y * 0.17) + 40.0 * jitter(5, (j * N + i) as u64, 0) - 3.0 * x;
            z[j * N + i] = f64::from(((h * edge) as f32 + 1.0) as f32);
        }
    }
    // a flat terrace and a pit
    for j in 20..26 {
        for i in 20..26 {
            z[j * N + i] = 200.0;
        }
    }
    z[30 * N + 30] = 40.0;
    z
}

// spec: GEN-166, GEN-178, TEST-126
#[test]
fn heights_reads_f32_and_f64_alike() {
    let z = surface();
    let z32: Vec<f32> = z.iter().map(|&h| h as f32).collect();
    assert!(z.iter().zip(&z32).all(|(a, b)| *a == f64::from(*b)), "the fixture is f32-representable");
    assert_eq!(Heights::len(&z[..]), Heights::len(&z32[..]));
    assert!((0..z.len()).all(|c| z.at(c).to_bits() == z32.at(c).to_bits()), "the f32 read is the exact widening");
    assert_eq!(Heights::len(&[1.0f64; 3]), 3);
    assert_eq!([1.5f32; 3].at(2), 1.5);
}

// spec: GEN-166, GEN-178, TEST-126
#[test]
fn the_readers_of_the_plan_give_the_same_bits_on_f32_and_f64_heights() {
    let g = Grid::new(N, N, 800.0);
    let z = surface();
    let z32: Vec<f32> = z.iter().map(|&h| h as f32).collect();
    let outlet = g.outlets(&z, 0.0);
    for canonical in [false, true] {
        let (a, b) = (priority_flood_with(&g, &z, &outlet, 1e-3, canonical), priority_flood_with(&g, &z32[..], &outlet, 1e-3, canonical));
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "flood, canonical {canonical}"
        );
    }
    let (a, b) = (priority_flood_ungraded(&g, &z, &outlet, 1e-3, true), priority_flood_ungraded(&g, &z32, &outlet, 1e-3, true));
    assert_eq!(a, b);
    let weight: Vec<f32> = (0..N * N).map(|c| 1.0 + (c % 7) as f32 * 0.1).collect();
    let (r64, f64s) = route_rules_filled(&g, &z, &outlet, 1e-3, 5, 0.0, Some(&weight), RoundRules::ROUND);
    let (r32, f32s) = route_rules_filled(&g, &z32[..], &outlet, 1e-3, 5, 0.0, Some(&weight), RoundRules::ROUND);
    assert_eq!((r64.receiver.clone(), r64.order.clone()), (r32.receiver.clone(), r32.order.clone()));
    assert_eq!(r64.area.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), r32.area.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    assert_eq!(f64s.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), f32s.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let runoff = vec![1e-8f32; N * N];
    let evap = vec![1e-9f32; N * N];
    let h64 = hydrology(&g, &z, &f64s, &r64, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    let h32 = hydrology(&g, &z32[..], &f32s, &r32, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    assert_eq!(format!("{:?}", h64.stats), format!("{:?}", h32.stats));
    assert_eq!(h64.label, h32.label);
    assert_eq!(h64.q.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), h32.q.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    let (p64, p32) = (river_pieces(&g, &r64, &h64, &z), river_pieces(&g, &r32, &h32, &z32[..]));
    assert_eq!(format!("{:?}", p64.pieces), format!("{:?}", p32.pieces));
    assert_eq!((p64.dropped_cells, p64.cuts.len()), (p32.dropped_cells, p32.cuts.len()));
    let (c64, c32) = (ChannelGraph::from_hydro(&g, &r64, &h64, &z), ChannelGraph::from_hydro(&g, &r32, &h32, &z32[..]));
    assert_eq!(format!("{:?}", c64), format!("{:?}", c32));
}

// spec: GEN-166, GEN-178, TEST-126
#[test]
fn the_plan_stores_the_heights_f32_between_the_rounds_and_the_sediment_pass_and_gives_the_f64_plans_bits() {
    let g = Grid::new(N, N, 800.0);
    let z0 = surface();
    let outlet = g.outlets(&z0, 0.0);
    let run = |f32_rounds: bool| {
        let spill = std::env::temp_dir().join(format!("planet-heights-{}-{f32_rounds}.f64", std::process::id()));
        let mut p = PlanParams::standard_for(5, 2, spill, &earth_span());
        p.rounds = 2;
        p.carve.steps = 10;
        p.carve.round_f32 = f32_rounds;
        let marks = Mutex::new(Vec::<String>::new());
        let built = build_resident(&g, z0.clone(), &outlet, &p, &|c| (900.0 + (c % 5) as f64 * 50.0, 12.0), &|m| {
            marks.lock().unwrap().push(m.to_string())
        })
        .expect("the plan runs");
        (built, marks.into_inner().unwrap())
    };
    let (on, marks_on) = run(true);
    let (off, marks_off) = run(false);
    let pos = |marks: &[String], name: &str| marks.iter().position(|m| m == name);
    let at = |name: &str| pos(&marks_on, name).unwrap_or_else(|| panic!("the mark {name} is missing: {marks_on:?}"));
    let (a, b, c, d) = (at("after the rounds"), at("heights stored f32"), at("heights widened"), at("pre-carve surface read back"));
    assert!(a < b && b < c && c < d, "the f64 heights are dropped after the rounds and widened before the sediment pass: {marks_on:?}");
    assert!(pos(&marks_off, "heights stored f32").is_none(), "with f64 rounds nothing is stored f32: {marks_off:?}");
    // the bits: `the_phase_plan_is_the_steps_in_order_...` (bake_compact) compares this plan, with the flag on, against the steps run by hand on f64 heights
    assert_eq!(on.z.len(), off.z.len());
    assert!(on.hydro.q.is_empty() && on.hydro.qw.is_empty());
    assert_eq!(on.hydro.label.len(), z0.len());
}

/// Earth's height span, written out (GEN-175): the plan refuses a surface outside it.
fn earth_span() -> planet_generators::bake::precision::WorldPrecision {
    planet_generators::bake::precision::WorldPrecision::new(-11_000.0, 9_000.0, 1e-3, planet_generators::bake::precision::EPSILON_MAX_M)
}
