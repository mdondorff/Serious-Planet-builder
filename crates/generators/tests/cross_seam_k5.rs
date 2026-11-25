//! Spike only (crusader #208, step 1, the lead's risk checkpoint, 2026-11-25): wave 1's K5 per step through GEN-143's partitioned route on TEST-139's fixture.
//! K5 (`solve_rounds_with`, 4 rounds of 5 steps, as `RegionSpec`) is, each round, one routing of the whole surface, then each catchment's round from its own inputs. Its partitioned form
//! (`solve_rounds_memo`) routes the round tile by tile (GEN-143: tile floods from the neighbours' offers, receivers per tile, the area by a topological sweep), on the cube grid of the six faces
//! (GEN-144). A catchment's round reads only its own gathered inputs, so it is decided whole by one owner, whichever region that is; only its cells outside the owner travel.
//! The checkpoint:
//! - the partitioned K5 is the resident K5 bit for bit, at the layouts' tile (8 H1 cells) and at every other tile that divides a face, on 1 and 4 threads;
//! - per TEST-151 layout, what crosses a region per round (the catchments that span regions and their cells outside the owner);
//! - the wall time, here and on the fixture's L cube (6 x 192²).
//!
//! The input is base1's output on the fixture (#207, the whole H1 cube), so the stage order of wave 1 holds. Drivers off (no uplift field, no class factor, no runoff weight).

use super::cross_seam_band6::{input_h1, run};
use super::g0_fixture::fixture::*;
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::round::{cut_in, solve_rounds_memo, solve_rounds_with, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use std::collections::BTreeMap;
use std::time::Instant;

const ROUNDS: usize = 4;
const SEED: u64 = 0x6b35;

fn params() -> CarveParams {
    CarveParams {
        steps: 5 * ROUNDS,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    }
}

fn resident(g: &Grid, z0: &[f64], outlet: &[bool], threads: usize) -> (Vec<f64>, f64) {
    let mut z = z0.to_vec();
    let t = Instant::now();
    solve_rounds_with(g, &mut z, outlet, &params(), ROUNDS, &threads, SEED, &Drivers::default());
    (z, t.elapsed().as_secs_f64())
}

fn partitioned(g: &Grid, z0: &[f64], outlet: &[bool], threads: usize, tile: usize) -> (Vec<f64>, f64) {
    let mut z = z0.to_vec();
    let mut cache = RoundCache::without_memory();
    let t = Instant::now();
    solve_rounds_memo(g, &mut z, outlet, &params(), ROUNDS, &threads, SEED, &Drivers::default(), tile, &mut cache);
    (z, t.elapsed().as_secs_f64())
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// What one routing's catchments make cross regions: (catchments spanning more than one region, their cells outside the owner, the largest such count). The owner is the region of the
/// catchment's outlet.
fn crossing(g: &Grid, z: &[f64], outlet: &[bool], l: &PartitionLayout) -> (usize, usize, usize) {
    let n = g.layout().0;
    let tile = l.tile / L_PER_H1;
    let region_of = |p: usize| {
        let (f, r) = (p / (n * n), p % (n * n));
        l.region(f, (r % n) / tile, (r / n) / tile)
    };
    let r = route_rules(g, z, outlet, params().epsilon, SEED, 0.0, None, RoundRules::ROUND);
    let cuts = cut_in(Some(g), &r);
    let (mut spanning, mut away, mut largest) = (0, 0, 0);
    for m in &cuts.members {
        let root = m.iter().map(|&c| c as usize).find(|&c| r.receiver[c] == NONE).unwrap_or(m[0] as usize);
        let owner = region_of(root);
        let out = m.iter().filter(|&&c| region_of(c as usize) != owner).count();
        if out > 0 {
            spanning += 1;
            away += out;
            largest = largest.max(out);
        }
    }
    (spanning, away, largest)
}

// spike only (crusader #208 step 1): the K5 checkpoint
#[test]
fn k5_through_the_partitioned_route_is_the_resident_k5_on_the_cube() {
    // wave 1's order: base1 (the whole H1 cube, #207), then K5
    let inp = input_h1();
    let z0 = run(&inp, None, false).z;
    let g = Grid::cube(H1_N, RADIUS_M);
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let (zr, tr) = resident(&g, &z0, &outlet, 1);
    let moved = zr.iter().zip(&z0).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    eprintln!("#208 K5 H1 cube (6 x {H1_N}², {} cells): resident {tr:.3} s, {moved} cells carved", z0.len());
    assert!(moved > 0, "K5 carves the fixture");
    let mut times = BTreeMap::new();
    for tile in [3, 4, 6, 8, 12, 24] {
        for threads in [1, 4] {
            let (zp, tp) = partitioned(&g, &z0, &outlet, threads, tile);
            times.insert((tile, threads), tp);
            assert!(same(&zp, &zr), "tile {tile}, {threads} threads: the partitioned K5 is the resident K5");
        }
    }
    eprintln!("#208 K5 H1: partitioned = resident on tiles 3, 4, 6, 8, 12, 24 and 1 and 4 threads; seconds by (tile, threads): {times:?}");
    let (_, t4) = resident(&g, &z0, &outlet, 4);
    assert!(same(&resident(&g, &z0, &outlet, 4).0, &zr), "the resident K5 on 4 threads");
    eprintln!("#208 K5 H1: resident on 4 threads {t4:.3} s");
    for l in partition_layouts() {
        let (s0, a0, m0) = crossing(&g, &z0, &outlet, &l);
        let (s1, a1, m1) = crossing(&g, &zr, &outlet, &l);
        eprintln!(
            "#208 K5 {}: catchments spanning regions (round 1 / after K5) {s0} / {s1}, their cells outside the owner {a0} / {a1} (largest {m0} / {m1}) of {} H1 cells",
            l.name,
            z0.len()
        );
    }
    // the wall time at L scale: the fixture's base on the L cube (no K5 runs at L in the chain; this is the per-cell cost of the partitioned route against the resident one)
    let f = build(4);
    let gl = Grid::cube(L_N, RADIUS_M);
    let ol: Vec<bool> = f.b.iter().map(|&h| h <= SEA_LEVEL_M).collect();
    let (zl, tl) = resident(&gl, &f.b, &ol, 4);
    let (zlp, tlp) = partitioned(&gl, &f.b, &ol, 4, TILE_L);
    assert!(same(&zl, &zlp), "the L cube: partitioned = resident");
    eprintln!(
        "#208 K5 L cube (6 x {L_N}², {} cells), 4 threads: resident {tl:.2} s, partitioned (tile {TILE_L}) {tlp:.2} s, ratio {:.2}; {:.0} ns a cell a round partitioned",
        f.b.len(),
        tlp / tl,
        tlp / (f.b.len() * ROUNDS) as f64 * 1e9
    );
}
