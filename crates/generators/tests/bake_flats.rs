#![allow(clippy::needless_range_loop)]
//! Tier A, GEN-173 (TEST-119; SPIKE-108): the flats of the canonical flood are graded by Euclidean distance from the lower rim cells, not by the chessboard count, so a flat drains
//! radially and has no straight diagonal run; every cell of a flat drains; the grade does not depend on anything but the cells. Small grids; no store read.

use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::route::{route_rules, MfdClip, RoundRules, NONE};
use planet_generators::bake::{Grid, NEIGHBOURS};

const N: usize = 61;
const EPS: f64 = 1e-3;

/// A square pit: a wall of 20 m three cells thick (the flat does not touch the grid border), a floor of 0 m inside, one outlet (the pour point) at 8 m in the middle of the right wall.
fn pit() -> (Vec<f64>, Vec<bool>) {
    let mut z = vec![0.0; N * N];
    let mut outlet = vec![false; N * N];
    for j in 0..N {
        for i in 0..N {
            if i.min(j).min(N - 1 - i).min(N - 1 - j) < 3 {
                z[j * N + i] = 20.0;
            }
        }
    }
    z[(N / 2) * N + N - 3] = 8.0;
    outlet[(N / 2) * N + N - 3] = true;
    (z, outlet)
}

/// The steepest descent of `f` from `p` over the 8 neighbours by drop per distance.
fn steepest(grid: &Grid, f: &[f64], p: usize) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for d in 0..8 {
        let Some(q) = grid.neighbour(p, d) else { continue };
        let slope = (f[p] - f[q]) / if d < 4 { 1.0 } else { std::f64::consts::SQRT_2 };
        if slope > 0.0 && best.is_none_or(|(s, _)| slope > s) {
            best = Some((slope, q));
        }
    }
    best.map(|(_, q)| q)
}

/// The longest run of equal diagonal steps along the D8 receivers (jittered steepest descent) of the flood's routing of `z`.
fn longest_diagonal_run(grid: &Grid, z: &[f64], outlet: &[bool], canonical: bool) -> usize {
    let r = route_rules(grid, z, outlet, EPS, 7, 0.0, None, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: canonical });
    let mut longest = 0;
    for start in 0..N * N {
        let (mut p, mut run, mut last) = (start, 0usize, (0i64, 0i64));
        while r.receiver[p] != NONE && r.receiver[p] as usize != p {
            let q = r.receiver[p] as usize;
            let step = ((q % N) as i64 - (p % N) as i64, (q / N) as i64 - (p / N) as i64);
            let diagonal = step.0 != 0 && step.1 != 0;
            run = if diagonal && step == last { run + 1 } else { usize::from(diagonal) };
            longest = longest.max(run);
            last = step;
            p = q;
        }
    }
    longest
}

// spec: GEN-173, TEST-119
#[test]
fn a_flat_has_no_long_straight_diagonal_run_where_the_chessboard_flat_has() {
    let grid = Grid::new(N, N, 30.0);
    let (z, outlet) = pit();
    let (chess, radial) = (longest_diagonal_run(&grid, &z, &outlet, false), longest_diagonal_run(&grid, &z, &outlet, true));
    eprintln!("longest diagonal run: chessboard {chess}, Euclidean {radial}");
    assert!(radial <= 12, "the Euclidean flat has no straight diagonal run over 12 cells: {radial} (chessboard {chess})");
    assert!(radial * 2 < chess, "and less than half the chessboard flat: {radial} against {chess}");
}

// spec: GEN-173, TEST-119
#[test]
fn the_grade_is_the_euclidean_distance_from_the_pour_point_and_every_cell_drains() {
    let grid = Grid::new(N, N, 30.0);
    let (z, outlet) = pit();
    let f = priority_flood_with(&grid, &z, &outlet, EPS, true);
    let (sx, sy) = (N as f64 - 3.0, (N / 2) as f64);
    let mut worst = 0.0f64;
    for j in 3..N - 3 {
        for i in 3..N - 4 {
            let p = j * N + i;
            let d = libm::hypot(i as f64 - sx, j as f64 - sy);
            // Within 5 % of the distance (and the step that keeps cells above their parents), never above the chessboard grade.
            let grade = (f[p] - 8.0) / EPS;
            worst = worst.max((grade - d / std::f64::consts::SQRT_2).abs() / (d / std::f64::consts::SQRT_2));
            let cheb = (i as f64 - sx).abs().max((j as f64 - sy).abs());
            assert!(grade <= cheb + 1e-9, "cell ({i},{j}): grade {grade} above the chessboard count {cheb}");
            assert!(steepest(&grid, &f, p).is_some(), "cell ({i},{j}) has no lower neighbour");
        }
    }
    assert!(worst < 0.05, "the grade follows the distance within 5 %: worst {worst}");
}

// spec: GEN-173, TEST-119
#[test]
fn the_flats_are_the_same_bits_on_a_window_of_the_same_cells_and_on_a_rerun() {
    let grid = Grid::new(N, N, 30.0);
    let (z, outlet) = pit();
    let a = priority_flood_with(&grid, &z, &outlet, EPS, true);
    let b = priority_flood_with(&grid, &z, &outlet, EPS, true);
    assert_eq!(a, b);
    // A translate of the whole pit inside a larger grid has the same grades (nothing but the cells decides).
    let big = N + 9;
    let mut zb = vec![0.0; big * big];
    let mut ob = vec![false; big * big];
    for j in 0..big {
        for i in 0..big {
            zb[j * big + i] = 20.0;
        }
    }
    for j in 0..N {
        for i in 0..N {
            zb[(j + 4) * big + i + 5] = z[j * N + i];
            ob[(j + 4) * big + i + 5] = outlet[j * N + i];
        }
    }
    let c = priority_flood_with(&Grid::new(big, big, 30.0), &zb, &ob, EPS, true);
    for j in 3..N - 3 {
        for i in 3..N - 3 {
            assert_eq!(c[(j + 4) * big + i + 5].to_bits(), a[j * N + i].to_bits(), "cell ({i},{j})");
        }
    }
    let _ = NEIGHBOURS;
}

/// A square pit of `s` cells a side (floor 0 m) in walls of 20 m, 6 cells of wall around it, one outlet (8 m) in the right wall in the middle of the pit's side.
fn big_pit(s: usize) -> (Grid, Vec<f64>, Vec<bool>, usize) {
    let g = s + 12;
    let mut z = vec![20.0; g * g];
    let mut outlet = vec![false; g * g];
    for j in 6..6 + s {
        for i in 6..6 + s {
            z[j * g + i] = 0.0;
        }
    }
    let o = (6 + s / 2) * g + 6 + s;
    z[o] = 8.0;
    outlet[o] = true;
    (Grid::new(g, g, 30.0), z, outlet, g)
}

// spec: GEN-173, TEST-119
#[test]
fn a_flat_up_to_the_bound_is_graded_and_a_larger_one_keeps_the_chessboard_and_no_tiling_changes_a_bit() {
    use planet_generators::bake::flood::FLAT_BOUND;
    use planet_generators::bake::partition::{partitioned_route, PartCache};
    for (s, graded) in [(FLAT_BOUND + 1, true), (FLAT_BOUND + 2, false)] {
        let (grid, z, outlet, g) = big_pit(s);
        let resident = priority_flood_with(&grid, &z, &outlet, EPS, true);
        let sample = (6 + s / 2) * g + 6 + s / 2;
        let chess_count = {
            let (cx, cy) = (6 + s / 2, 6 + s / 2);
            let (sx, sy) = (6 + s, 6 + s / 2);
            (cx.abs_diff(sx)).max(cy.abs_diff(sy)) as f64
        };
        let grade = (resident[sample] - 8.0) / EPS;
        if graded {
            assert!(
                grade < chess_count - 1.0,
                "a flat of diameter {FLAT_BOUND} is graded by Euclidean distance: {grade} against the chessboard count {chess_count}"
            );
        } else {
            assert!(
                (grade - chess_count).abs() < 1e-6,
                "a flat of diameter {} keeps the chessboard grade: {grade} against {chess_count}",
                FLAT_BOUND + 1
            );
        }
        // Two tilings (a tile corner inside the pit, in both) give the resident bits.
        for tile in [48usize, 70] {
            let part = partitioned_route(
                &grid,
                &z,
                &outlet,
                EPS,
                7,
                tile,
                RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true },
                &mut PartCache::default(),
            );
            assert!(
                part.filled.iter().zip(&resident).all(|(a, b)| a.to_bits() == b.to_bits()),
                "diameter {}: tile {tile}: the partitioned surface is the resident one",
                s - 1
            );
        }
    }
}

// spec: GEN-173, TEST-119 (numerics review: a lifted cell never stands below its own height)
#[test]
fn a_ramp_inside_a_flat_is_never_graded_below_its_own_height_and_the_partitioned_surface_agrees() {
    use planet_generators::bake::partition::{partitioned_route, PartCache};
    let eps = 0.02;
    let g = Grid::new(16, 9, 100.0);
    let mut z: Vec<f64> = (0..16 * 9).map(|p| 100.0 + (p % 16).min(p / 16).min(15 - p % 16).min(8 - p / 16) as f64).collect();
    let mut outlet = vec![false; 16 * 9];
    let at = |x: usize| 4 * 16 + x;
    z[at(2)] = 0.0;
    outlet[at(2)] = true;
    // A chain that rises 0.8 epsilon a cell from the outlet: each cell is lifted by less than the chessboard count.
    for x in 3..=12 {
        z[at(x)] = 0.8 * eps * (x as f64 - 2.0);
    }
    let f = priority_flood_with(&g, &z, &outlet, eps, true);
    for x in 3..=12 {
        assert!(f[at(x)] >= z[at(x)], "column {x}: filled {} below its height {}", f[at(x)], z[at(x)]);
        assert!(f[at(x)] <= eps * (x as f64 - 2.0) + 1e-9, "column {x}: above the chessboard count");
        assert!(f[at(x)] > f[at(x - 1)], "column {x} drains to {}", x - 1);
    }
    let part = partitioned_route(
        &g,
        &z,
        &outlet,
        eps,
        7,
        5,
        RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true },
        &mut PartCache::default(),
    );
    assert!(part.filled.iter().zip(&f).all(|(a, b)| a.to_bits() == b.to_bits()));
}

// spec: GEN-173, TEST-119 (numerics review: a flat is graded from its own rim, whatever its neighbours do)
#[test]
fn a_small_flat_has_the_same_bits_whether_or_not_the_flats_beside_it_are_graded() {
    use planet_generators::bake::flood::{priority_flood_ungraded, regrade_flats};
    let n = 70;
    let g = Grid::new(n, n, 30.0);
    let mut z = vec![0.0; n * n];
    for p in 0..n * n {
        z[p] = 10.0 * libm::floor(2.5 * (planet_generators::bake::route::jitter(5, p as u64, 0) + 1.0))
            + 3.0 * (((p % n) / 9 + (p / n) / 9) % 2) as f64;
    }
    let outlet: Vec<bool> = (0..n * n).map(|p| p % n == 0 || p / n == 0 || p % n == n - 1 || p / n == n - 1).collect();
    let base = priority_flood_ungraded(&g, &z, &outlet, 1.0, true);
    let (mut small, mut large) = (base.clone(), base.clone());
    let a = regrade_flats(&g, &z, &outlet, 1.0, 3, &mut small);
    let b = regrade_flats(&g, &z, &outlet, 1.0, 1000, &mut large);
    assert!(a.iter().any(|f| f.graded) && a.iter().any(|f| !f.graded), "the fixture has flats on both sides of the bound");
    assert_eq!(a.len(), b.len());
    let mut compared = 0;
    for p in 0..n * n {
        if small[p].to_bits() != base[p].to_bits() {
            compared += 1;
            assert_eq!(small[p].to_bits(), large[p].to_bits(), "cell {p}: a small flat's bits depend on whether its neighbours are graded");
        }
    }
    assert!(compared > 20, "{compared} regraded cells compared");
}

/// A rough low plain (heights up to `amp` epsilon, seeded) in walls of 1 m three cells thick, one outlet at 0 m in the right wall: its flats hold cells the fill lifted by less than their
/// chessboard count beside cells nearer the pour by count but farther from it by distance.
fn rough_plain(n: usize, amp: f64, seed: u64) -> (Grid, Vec<f64>, Vec<bool>) {
    let mut z = vec![0.0; n * n];
    let mut outlet = vec![false; n * n];
    for p in 0..n * n {
        let (x, y) = (p % n, p / n);
        z[p] = if x.min(y).min(n - 1 - x).min(n - 1 - y) < 3 {
            1.0
        } else {
            EPS * amp * (planet_generators::bake::route::jitter(seed, p as u64, 0) + 1.0) / 2.0
        };
    }
    let o = (n / 2) * n + n - 3;
    z[o] = 0.0;
    outlet[o] = true;
    (Grid::new(n, n, 30.0), z, outlet)
}

// spec: GEN-173, TEST-119 (f) (a flat with a hump: SPIKE-115's K7 refusal on the L-eroded surface, route.rs:236)
#[test]
fn a_flat_with_a_hump_is_never_graded_above_its_chessboard_level_and_the_cell_beside_it_still_drains() {
    use planet_generators::bake::flood::priority_flood_ungraded;
    let n = 48;
    // At the sea and near the highest summits (the rounding of `base + epsilon rank` there).
    for offset in [0.0, 8_999.0] {
        let (grid, mut z, outlet) = rough_plain(n, 64.0, 146);
        // A cell beside the flat, above its neighbours' chessboard levels plus epsilon, so it is not lifted and drains into the flat.
        z[19 * n + 24] = 0.040_132_38;
        for h in z.iter_mut() {
            *h += offset;
        }
        let chess = priority_flood_ungraded(&grid, &z, &outlet, EPS, true);
        let graded = priority_flood_with(&grid, &z, &outlet, EPS, true);
        assert!(graded.iter().zip(&chess).any(|(g, c)| g < c), "the fixture has a graded flat");
        let above: Vec<(usize, f64)> = (0..n * n).filter(|&p| graded[p] > chess[p]).map(|p| (p, (graded[p] - chess[p]) / EPS)).collect();
        assert!(
            above.is_empty(),
            "offset {offset} m: {} cells stand above their chessboard level (cell, excess in epsilon): {:?}",
            above.len(),
            &above[..above.len().min(6)]
        );
        let stuck: Vec<usize> =
            (0..n * n).filter(|&p| !outlet[p] && (0..8).filter_map(|d| grid.neighbour(p, d)).all(|q| graded[q] >= graded[p])).collect();
        assert!(stuck.is_empty(), "offset {offset} m: cells without a lower neighbour after the grade: {stuck:?}");
        let sunk: Vec<usize> = (0..n * n).filter(|&p| chess[p] > z[p] && graded[p] <= z[p]).collect();
        assert!(sunk.is_empty(), "offset {offset} m: lifted cells graded onto or below their ground: {sunk:?}");
        // The routing takes a receiver for every cell (it asserted at route.rs:236 before the cap).
        let _ = route_rules(&grid, &z, &outlet, EPS, 7, 0.0, None, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true });
    }
}
