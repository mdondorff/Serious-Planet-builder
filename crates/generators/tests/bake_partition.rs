//! Tier A, GEN-143 (A3 sections 6.5 and 6.6, gate G-PART at kernel scale): the partitioned route (tile floods from the neighbours' offers, per-tile receivers, the area by a topological sweep over the tiles)
//! is the resident route under the round rules, bit for bit, for any tile size; an edited surface reuses the floods of the tiles it did not touch and is again the resident route. Small grids; no store read.

use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::route::{route_rules, MfdClip, RoundRules};
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

fn grids() -> Vec<(&'static str, Grid)> {
    let dx = R * core::f64::consts::FRAC_PI_2 / 3072.0;
    vec![
        ("uniform", Grid::new(N, N, 800.0)),
        ("metric", Grid::new(N, N, dx).with_origin((2014, 190)).with_metric(Metric::cube_face(3072, R, (2014, 190), N, N))),
    ]
}

fn differ<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

// spec: GEN-143
#[test]
fn the_partitioned_route_is_the_resident_route_for_any_tile_size() {
    for (name, g) in grids() {
        for terraced in [false, true] {
            let z = terrain(4, terraced);
            let outlet = g.outlets(&z, 0.0);
            for rules in [RoundRules::ROUND, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true }] {
                let want = route_rules(&g, &z, &outlet, EPS, 4, 0.0, None, rules);
                let want_filled = priority_flood_with(&g, &z, &outlet, EPS, true);
                for tile in [8usize, 13, 24, 72, 100] {
                    let got = partitioned_route(&g, &z, &outlet, EPS, 4, tile, rules, &mut PartCache::default());
                    let ctx = format!("{name}, terraced {terraced}, clip {:?}, tile {tile}", rules.clip_mfd);
                    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                    assert_eq!(differ(&bits(&got.filled), &bits(&want_filled)), 0, "G-PART filled levels differ ({ctx})");
                    assert_eq!(differ(&got.routing.receiver, &want.receiver), 0, "G-PART receivers differ ({ctx})");
                    assert_eq!(differ(&bits(&got.routing.area), &bits(&want.area)), 0, "G-PART A_d differs ({ctx})");
                    assert_eq!(got.routing.order, want.order, "G-PART order differs ({ctx})");
                }
            }
        }
    }
}

// spec: GEN-143
#[test]
fn an_edit_reuses_the_untouched_tiles_and_is_again_the_resident_route() {
    for (name, g) in grids() {
        let z = terrain(6, true);
        let outlet = g.outlets(&z, 0.0);
        let tile = 12;
        let mut cache = PartCache::default();
        let first = partitioned_route(&g, &z, &outlet, EPS, 6, tile, RoundRules::ROUND, &mut cache);
        // The same surface again: every tile evaluation is a memo hit.
        let again = partitioned_route(&g, &z, &outlet, EPS, 6, tile, RoundRules::ROUND, &mut cache);
        assert_eq!(again.stats.floods, 0, "no flood on an unchanged surface ({name})");
        assert!(again.stats.memo_hits > 0);
        assert_eq!(again.stats.tiles_filled_changed + again.stats.tiles_receiver_changed + again.stats.tiles_area_changed, 0);
        // An edit: a ridge cell raised by 120 m and a pass lowered by 40 m.
        let mut z2 = z.clone();
        z2[30 * N + 30] += 120.0;
        z2[45 * N + 20] -= 40.0;
        let edited = partitioned_route(&g, &z2, &outlet, EPS, 6, tile, RoundRules::ROUND, &mut cache);
        let want = route_rules(&g, &z2, &outlet, EPS, 6, 0.0, None, RoundRules::ROUND);
        assert_eq!(differ(&edited.routing.receiver, &want.receiver), 0, "receivers after the edit ({name})");
        assert_eq!(
            differ(
                &edited.routing.area.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                &want.area.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            ),
            0,
            "A_d after the edit ({name})"
        );
        assert!(
            edited.stats.floods < first.stats.floods,
            "the edit floods fewer tiles than a fresh build ({} against {}, {name})",
            edited.stats.floods,
            first.stats.floods
        );
        assert!(edited.stats.tiles_filled_changed >= 1 && edited.stats.tiles_filled_changed < edited.stats.tiles);
    }
}

// spec: GEN-143
#[test]
fn a_cache_never_answers_with_a_flood_of_other_inputs() {
    let (_, g) = grids().remove(0);
    let z = terrain(7, true);
    let outlet = g.outlets(&z, 0.0);
    let mut cache = PartCache::default();
    for (eps, tile) in [(1e-3, 12usize), (5e-3, 12), (5e-3, 17), (1e-3, 17)] {
        let got = partitioned_route(&g, &z, &outlet, eps, 7, tile, RoundRules::ROUND, &mut cache);
        let want = route_rules(&g, &z, &outlet, eps, 7, 0.0, None, RoundRules::ROUND);
        assert_eq!(differ(&got.routing.receiver, &want.receiver), 0, "receivers (epsilon {eps}, tile {tile})");
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(differ(&bits(&got.routing.area), &bits(&want.area)), 0, "A_d (epsilon {eps}, tile {tile})");
    }
}

/// The land of `terrain` with a lake of sea cells (z = -3) inside it.
fn with_sea(salt: u64, h: usize) -> Vec<f64> {
    let mut z = terrain(salt, true);
    z.truncate(N * h);
    for j in 30..36.min(h - 2) {
        for i in 28..34 {
            z[j * N + i] = -3.0;
        }
    }
    z
}

// spec: GEN-143
#[test]
fn interior_sea_other_grid_shapes_and_a_changed_outlet_mask_are_the_resident_route() {
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    // One cache across a square and a wide grid, interior sea, and a sea cell turned to land with the outlets recomputed.
    let mut cache = PartCache::default();
    for h in [N, N - 9, N] {
        let g = Grid::new(N, h, 800.0);
        let mut z = with_sea(8, h);
        for step in 0..2 {
            if step == 1 {
                z[32 * N + 30] = 3.0; // a sea cell raised to land (the mirror of its height)
            }
            let outlet = g.outlets(&z, 0.0);
            let want = route_rules(&g, &z, &outlet, EPS, 8, 0.0, None, RoundRules::ROUND);
            let got = partitioned_route(&g, &z, &outlet, EPS, 8, 8, RoundRules::ROUND, &mut cache);
            let ctx = format!("height {h}, step {step}");
            assert_eq!(differ(&got.routing.receiver, &want.receiver), 0, "receivers ({ctx})");
            assert_eq!(differ(&bits(&got.routing.area), &bits(&want.area)), 0, "A_d ({ctx})");
            assert!(got.stats.sweeps > 1 || got.stats.floods == 0, "offers across tile borders were used ({ctx})");
        }
    }
}
