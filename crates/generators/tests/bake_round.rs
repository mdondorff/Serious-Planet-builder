//! Tier A, GEN-141 (A3 sections 6.2 to 6.4): the two round kernels (the MFD area clipped to the D8 catchment, the flood visiting by level then cell id) and the round as a
//! pure function of its inputs. G-ROUND: a carve that visits one catchment after the other, in either catchment order, is the same bits as the region-order carve, under today's rules and
//! under the round rules. Closure: under the clip the area at a catchment's outlet is the sum of its members' areas. The default rules are untouched. Small grids; no store read.

use planet_generators::bake::carve::{carve, CarveParams, Schedule};
use planet_generators::bake::flood::{priority_flood, priority_flood_with};
use planet_generators::bake::route::{catchment_roots, route_rules, route_weighted, MfdClip, RoundRules};
use planet_generators::bake::Grid;

const N: usize = 72;

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

// spec: GEN-141
#[test]
fn the_default_rules_are_todays_kernels() {
    let g = Grid::new(N, N, 800.0);
    let z = terrain(2, true);
    let outlet = g.outlets(&z, 0.0);
    let a = route_weighted(&g, &z, &outlet, 1e-3, 2, 0.0, None);
    let b = route_rules(&g, &z, &outlet, 1e-3, 2, 0.0, None, RoundRules::default());
    assert_eq!(a, b);
    // A digest of the routing and a 20-step carve of this land, taken on the kernels before GEN-141 (origin/main at 70743083): the default path is bit for bit what it was.
    let mut d: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| d = (d ^ v).wrapping_mul(0x100_0000_01b3);
    for p in 0..N * N {
        mix(u64::from(a.receiver[p]));
        mix(u64::from(a.order[p]));
        mix(a.area[p].to_bits());
    }
    let mut zz = z.clone();
    carve(&g, &mut zz, &outlet, &CarveParams { steps: 20, ..CarveParams::default() }, None, 2);
    zz.iter().for_each(|v| mix(v.to_bits()));
    assert_eq!(d, 0xbbde_6e85_47d2_338a, "the default kernels moved: digest {d:016x}");
    assert_eq!(priority_flood(&g, &z, &outlet, 1e-3), priority_flood_with(&g, &z, &outlet, 1e-3, false));
    assert!(RoundRules::default().is_default() && !RoundRules::ROUND.is_default());
    assert_eq!(CarveParams::default().schedule, Schedule::Region);
    assert!(CarveParams::default().round_rules.is_default());
}

// spec: GEN-141
#[test]
fn the_canonical_flood_is_a_valid_fill_that_does_not_depend_on_anything_but_its_input() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(3, terraced);
        let outlet = g.outlets(&z, 0.0);
        let f = priority_flood_with(&g, &z, &outlet, 1e-3, true);
        assert_eq!(f, priority_flood_with(&g, &z, &outlet, 1e-3, true), "two runs differ");
        for p in 0..N * N {
            assert!(f[p] >= z[p], "cell {p} is below its own height");
            if !outlet[p] {
                assert!(
                    (0..8).filter_map(|d| g.neighbour(p, d)).any(|q| f[q] < f[p]),
                    "cell {p} has no lower neighbour on the filled surface"
                );
            }
        }
    }
}

// spec: GEN-141
#[test]
fn under_the_local_clip_a_catchments_outlet_holds_exactly_the_area_of_its_members() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(4, terraced);
        let outlet = g.outlets(&z, 0.0);
        let mut moved_by_off = false;
        for (rules, closed) in [
            (RoundRules::ROUND, true),
            (RoundRules { clip_mfd: MfdClip::ToReceiver, canonical_flood: true }, true),
            (RoundRules { canonical_flood: true, ..Default::default() }, false),
        ] {
            let r = route_rules(&g, &z, &outlet, 1e-3, 4, 0.0, None, rules);
            let root = catchment_roots(&r.receiver, &r.order);
            let mut members = vec![0.0f64; N * N];
            for p in 0..N * N {
                members[root[p] as usize] += g.cell_area(p);
            }
            for p in (0..N * N).filter(|&p| root[p] as usize == p) {
                let ok = (r.area[p] - members[p]).abs() <= 1e-9 * members[p];
                if closed {
                    assert!(ok, "{rules:?}: the outlet {p} holds {} of {} m2", r.area[p], members[p]);
                } else {
                    moved_by_off |= !ok;
                }
            }
        }
        assert!(moved_by_off, "terraced {terraced}: without the clip no area crosses a divide, the fixture does not test the clip");
    }
}

// spec: GEN-141
#[test]
fn on_a_land_without_flats_a_catchment_routed_alone_is_its_part_of_the_region_routing() {
    // The local clip makes the accumulation closed under the catchment; the flood's reach across a divide shows only on flats (the measured limit, design.md), so this fixture has none.
    let g = Grid::new(N, N, 800.0);
    let z = terrain(5, false);
    let outlet = g.outlets(&z, 0.0);
    let r = route_rules(&g, &z, &outlet, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    let root = catchment_roots(&r.receiver, &r.order);
    let mut roots = root.clone();
    roots.sort_unstable();
    roots.dedup();
    let mut checked = 0;
    for &rt in &roots {
        let active: Vec<bool> = root.iter().map(|&x| x == rt).collect();
        if active.iter().filter(|&&a| a).count() < 20 {
            continue;
        }
        let ga = g.clone().with_active(active.clone());
        let mut out = vec![false; N * N];
        out[rt as usize] = true;
        let a = route_rules(&ga, &z, &out, 1e-3, 5, 0.0, None, RoundRules::ROUND);
        for p in (0..N * N).filter(|&p| active[p]) {
            assert_eq!(a.receiver[p], r.receiver[p], "catchment {rt} cell {p}: the receiver");
            assert_eq!(a.area[p].to_bits(), r.area[p].to_bits(), "catchment {rt} cell {p}: the area");
        }
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} catchments checked");
}

// spec: GEN-141
#[test]
fn g_round_a_round_carved_one_catchment_after_the_other_is_the_region_carve() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(6, terraced);
        let outlet = g.outlets(&z, 0.0);
        for rules in [RoundRules::default(), RoundRules::ROUND] {
            let run = |schedule| {
                let mut zz = z.clone();
                carve(&g, &mut zz, &outlet, &CarveParams { steps: 20, round_rules: rules, schedule, ..CarveParams::default() }, None, 6);
                zz.iter().map(|v| v.to_bits()).collect::<Vec<u64>>()
            };
            let region = run(Schedule::Region);
            let moved = region.iter().zip(&z).filter(|(a, b)| **a != b.to_bits()).count();
            assert!(moved > N * N / 4, "the carve moved only {moved} cells");
            for descending in [false, true] {
                let by = run(Schedule::Catchments { descending });
                let differ = by.iter().zip(&region).filter(|(a, b)| a != b).count();
                assert_eq!(differ, 0, "terraced {terraced} {rules:?} descending {descending}: {differ} cells differ from the region carve");
            }
        }
    }
}

// spec: GEN-141
#[test]
fn a_masked_grid_shows_the_kernels_only_its_cells() {
    let g = Grid::new(6, 6, 100.0);
    let mut active = vec![true; 36];
    active[7] = false;
    let m = g.with_active(active);
    assert!(!m.is_active(7) && m.is_active(8));
    assert_eq!(m.neighbour(7, 0), None, "a masked cell has no neighbours");
    assert_eq!(m.neighbour(8, 0), None, "no cell has a masked cell as a neighbour");
    assert_eq!(m.neighbour(8, 1), Some(9));
}

/// A flat chain of cells beside a sea cell, ending next to a second sea cell: land `0.01` m, the sea at `0`, walls of 100 m. Row 4 of a 16 x 9 grid: the sea cell A at column 2 (the lower cell id), the chain on columns 3 to 9,
/// the cell `q` at column 10, the sea cell B at column 11.
fn chain_beside_two_outlets() -> (Grid, Vec<f64>, usize) {
    let g = Grid::new(16, 9, 100.0);
    // The walls rise inwards from the border (1 m a cell): no flat of walls for a FIFO to ramp across, so only the chain falsifies the plain flood.
    let mut z: Vec<f64> = (0..16 * 9).map(|p| 100.0 + (p % 16).min(p / 16).min(15 - p % 16).min(8 - p / 16) as f64).collect();
    let at = |x: usize| 4 * 16 + x;
    z[at(2)] = 0.0;
    z[at(11)] = 0.0;
    for x in 3..=10 {
        z[at(x)] = 0.01;
    }
    (g, z, at(10))
}

// spec: GEN-159
#[test]
fn the_canonical_flood_does_not_lift_a_cell_above_a_lower_outlet_beside_it() {
    // The plain flood drains the flat chain from A in its FIFO before the heap pops B (the same level, the higher id): the cell `q`, next to B, is lifted to the end of the chain's epsilon ramp (0.16 m).
    // The canonical flood visits by (level, id): `q` stands at the flat grade (GEN-173) above B, never above the chessboard count of one epsilon (GEN-159), and above the outlet beside it.
    let (g, z, q) = chain_beside_two_outlets();
    let eps = 0.02;
    let outlet = g.outlets(&z, 0.0);
    let canonical = priority_flood_with(&g, &z, &outlet, eps, true);
    assert!(
        (canonical[q] - eps / std::f64::consts::SQRT_2).abs() < 1e-12,
        "q is eps d / sqrt 2 above the outlet beside it (d = 1): {}",
        canonical[q]
    );
    assert!(canonical[q] > 0.0 && canonical[q] <= eps, "above the outlet beside it, never above the chessboard count: {}", canonical[q]);
    // The mechanism: the plain flood lifts it by the length of the chain, 8 epsilons (0.16 m), while B is still in its heap.
    let plain = priority_flood(&g, &z, &outlet, eps);
    assert!((plain[q] - 8.0 * eps).abs() < 1e-12, "the plain flood's FIFO runs the chain first: {}", plain[q]);
    // The ramp from both ends: each chain cell stands eps d / sqrt 2 above the nearer outlet (A at column 2, B at column 11), d the distance to it in cells (GEN-173), and never above the chessboard count.
    for x in 3..=10usize {
        let d = (x as f64 - 2.0).min(11.0 - x as f64);
        let want = eps * d / std::f64::consts::SQRT_2;
        assert!((canonical[4 * 16 + x] - want).abs() < 1e-12, "column {x}: {} against {want}", canonical[4 * 16 + x]);
        assert!(canonical[4 * 16 + x] <= eps * d, "column {x}: above the chessboard count");
    }
}

// spec: GEN-159
#[test]
fn the_pits_of_the_sediment_pass_are_the_canonical_flood_s_pits() {
    // On the surface above no cell is a depression of 0.1 m: the pit count the sediment pass reports must not count the plain flood's lift of the chain (the cells of columns 8 to 10 stand 0.11, 0.13 and 0.15 m deep on it).
    let (g, z, _) = chain_beside_two_outlets();
    let s = planet_generators::bake::sediment::pit_cells(&g, &z, 0.0, 0.02, 0.1);
    assert_eq!(s.pit_cells, 0, "no depression: {s:?}");
}

// spec: GEN-158
#[test]
fn the_route_that_keeps_its_filled_surface_is_the_route_and_the_flood_it_replaces() {
    use planet_generators::bake::route::route_rules_filled;
    let g = Grid::new(N, N, 800.0);
    let z = terrain(3, true);
    let outlet = g.outlets(&z, 0.0);
    let weights: Vec<f32> = (0..N * N).map(|p| 0.5 + (p % 7) as f32 * 0.25).collect();
    for rules in [RoundRules::ROUND, RoundRules { clip_mfd: MfdClip::Off, canonical_flood: true }, RoundRules::default()] {
        let plain = route_rules(&g, &z, &outlet, 1e-3, 9, 0.0, Some(&weights), rules);
        let (kept, filled) = route_rules_filled(&g, &z, &outlet, 1e-3, 9, 0.0, Some(&weights), rules);
        assert_eq!(kept.receiver, plain.receiver);
        assert_eq!(kept.order, plain.order);
        assert_eq!(kept.area.iter().map(|a| a.to_bits()).collect::<Vec<_>>(), plain.area.iter().map(|a| a.to_bits()).collect::<Vec<_>>());
        let flood = priority_flood_with(&g, &z, &outlet, 1e-3, rules.canonical_flood);
        assert_eq!(
            filled.iter().map(|a| a.to_bits()).collect::<Vec<_>>(),
            flood.iter().map(|a| a.to_bits()).collect::<Vec<_>>(),
            "the kept surface is the flood's bits"
        );
    }
}

/// A closed bowl of `n` cells a side with a noisy floor, its rim cells the only outlets at height `cut` (None: the rim's own height).
fn bowl(n: usize, cut: Option<f64>) -> (Grid, Vec<f64>, Vec<bool>) {
    let c = (n as f64 - 1.0) / 2.0;
    let mut z = vec![0.0f64; n * n];
    let mut outlet = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let r = libm::sqrt((i as f64 - c) * (i as f64 - c) + (j as f64 - c) * (j as f64 - c));
            let noise = planet_generators::bake::route::jitter(7, (j * n + i) as u64, 0);
            z[j * n + i] = 50.0 + 10.0 * r + 20.0 * noise;
            outlet[j * n + i] = i == 0 || j == 0 || i == n - 1 || j == n - 1;
            if let (Some(h), true) = (cut, outlet[j * n + i]) {
                z[j * n + i] = h;
            }
        }
    }
    (Grid::new(n, n, 814.0), z, outlet)
}

/// The median depth of the cells more than 1 m under the canonical fill (0 when there are none) and their number.
fn depth_under_fill(grid: &Grid, z: &[f64], outlet: &[bool]) -> (f64, usize) {
    let f = priority_flood_with(grid, z, outlet, 1e-3, true);
    let mut d: Vec<f64> = (0..z.len()).filter(|&p| !outlet[p] && f[p] - z[p] > 1.0).map(|p| f[p] - z[p]).collect();
    d.sort_by(f64::total_cmp);
    (d.get(d.len() / 2).copied().unwrap_or(0.0), d.len())
}

fn rounds_from(grid: &Grid, z0: &[f64], outlet: &[bool], rounds: usize) -> Vec<f64> {
    let mut z = z0.to_vec();
    let p = CarveParams { steps: 5 * rounds, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    planet_generators::bake::round::solve_rounds(grid, &mut z, outlet, &p, rounds, &1usize, 15);
    z
}

// spec: GEN-162 (A3 sections 5.3 and 6: the breach opens a depression whose sill it can lower)
#[test]
fn a_bowl_whose_outlet_is_at_the_sea_is_opened_by_the_first_round() {
    let (grid, z0, outlet) = bowl(32, Some(0.0));
    let z = rounds_from(&grid, &z0, &outlet, 1);
    let (_, cells) = depth_under_fill(&grid, &z, &outlet);
    assert_eq!(cells, 0, "a bowl whose rim is at the sea leaves {cells} cells under the fill after round 1");
}

// spec: GEN-162 (inbox #118), GEN-173: a depression behind an outlet the breach cannot lower must not get deeper round over round (the sweeps move the floor by up to 6 % of the depth in a round; the unfixed breach added 50 %).
// Two bounds (architect, 2026-11-12): the per-round one (round 1 may move the floor by 10 % of the base depth, each later round by at most 0.25 m over the one before) and a cumulative one with no slack, so drift over the rounds fails: rounds 2 to 4 are each at most the median of round 1, and round 4 is below round 1.
// Sequences: 42.7, 41.7, 41.4, 40.9 m, the same under the flat grade of GEN-173 (42.665, 41.745, 41.400, 40.855 m).
#[test]
fn a_lake_behind_a_cut_outlet_does_not_deepen_round_over_round() {
    let (grid, z0, outlet) = bowl(32, None);
    let (base, _) = depth_under_fill(&grid, &z0, &outlet);
    let (mut prev, mut first) = (base, f64::NAN);
    for round in 1..=4 {
        let z = rounds_from(&grid, &z0, &outlet, round);
        let (p50, cells) = depth_under_fill(&grid, &z, &outlet);
        // round 1: the incision of the sweeps moves the floor (a bowl without the breach reads 42.2 m against 40.3 m); after it the depth falls
        let allowed = if round == 1 { 0.1 * base } else { 0.25 };
        assert!(
            p50 <= prev + allowed,
            "round {round}: median depth under the fill {p50:.1} m against {prev:.1} m before it ({cells} cells, base {base:.1} m)"
        );
        if round == 1 {
            first = p50;
        } else {
            assert!(
                p50 <= first,
                "round {round}: median depth under the fill {p50:.3} m is deeper than round 1's {first:.3} m ({cells} cells)"
            );
        }
        if round == 4 {
            assert!(p50 < first, "round 4: {p50:.3} m is not below round 1's {first:.3} m");
        }
        prev = p50;
    }
}

// spec: GEN-162, GEN-173 (sea-outlet chains keep their bits through the base-level rule): a digest of three rounds on a bowl whose rim is at the sea level. Taken on the kernels before GEN-162 (origin/main at 82931e11: 0xb9e6_6858_0505_bf11, which GEN-162 left unchanged); declared change of GEN-173: the flats are graded by Euclidean distance, so the digest moves to the value below
#[test]
fn a_field_whose_outlets_are_at_the_sea_keeps_the_bits_it_had_before_the_base_level_rule() {
    let (grid, z0, outlet) = bowl(32, Some(0.0));
    let z = rounds_from(&grid, &z0, &outlet, 3);
    let mut d: u64 = 0xcbf2_9ce4_8422_2325;
    z.iter().for_each(|v| d = (d ^ v.to_bits()).wrapping_mul(0x100_0000_01b3));
    assert_eq!(d, 0x8686_f0d1_ac1d_02ec, "the sea-outlet rounds moved: digest {d:016x}");
}

/// The lowest height of the cells that stand beside a rim cell of the bowl and are not outlets, against the lowest rim cell: how far the breach cut below the exit.
fn cut_below_the_rim(grid: &Grid, z: &[f64], outlet: &[bool]) -> f64 {
    let rim = (0..z.len()).filter(|&p| outlet[p]).map(|p| z[p]).fold(f64::INFINITY, f64::min);
    let beside = (0..z.len())
        .filter(|&p| !outlet[p] && (0..8).any(|d| grid.neighbour(p, d).is_some_and(|q| outlet[q])))
        .map(|p| z[p])
        .fold(f64::INFINITY, f64::min);
    rim - beside
}

// spec: GEN-172
// spec: TEST-118
#[test]
fn a_window_cut_outlet_gives_the_breach_no_floor_and_a_basin_rim_keeps_it() {
    let (grid, z0, outlet) = bowl(32, None);
    let base = CarveParams { steps: 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let run = |window_cut: bool| {
        let mut z = z0.clone();
        planet_generators::bake::round::solve_rounds(&grid, &mut z, &outlet, &CarveParams { window_cut, ..base }, 1, &1usize, 15);
        z
    };
    let (rim_kept, cut_open) = (cut_below_the_rim(&grid, &run(false), &outlet), cut_below_the_rim(&grid, &run(true), &outlet));
    // the rim as a basin's rim (GEN-162): nothing beside it is cut below it; as a window cut the breach goes down toward the depression behind it
    assert!(rim_kept <= 3.0, "a basin rim is the floor of the breach: the cells beside it are at most {rim_kept:.2} m under it");
    assert!(cut_open > 100.0, "a window cut gives no floor: the cells beside it are {cut_open:.2} m under it");
    // the same through the region carve (`carve_rock`)
    let carve_run = |window_cut: bool| {
        let mut z = z0.clone();
        let p = CarveParams { window_cut, ..base };
        planet_generators::bake::carve::carve(&grid, &mut z, &outlet, &p, None, 15);
        z
    };
    assert!(cut_below_the_rim(&grid, &carve_run(false), &outlet) <= 3.0);
    assert!(cut_below_the_rim(&grid, &carve_run(true), &outlet) > 100.0);
}

// spec: GEN-172
// spec: TEST-118
#[test]
fn a_region_bounded_by_the_sea_has_no_window_cut_outlet_and_the_flag_changes_no_bit_on_it() {
    let (grid, z0, outlet) = bowl(32, Some(0.0));
    assert!(grid.window_cut(&z0, 0.0).iter().all(|&c| !c), "border cells at the sea level are sea outlets, not window cuts");
    let (_, land, _) = bowl(32, None);
    assert!(grid.window_cut(&land, 0.0).iter().filter(|&&c| c).count() >= 4 * 31, "a land border is window cut all around");
    let run = |window_cut: bool| {
        let mut z = z0.clone();
        let p = CarveParams { steps: 15, reroute_every: 5, round_rules: RoundRules::ROUND, window_cut, ..CarveParams::default() };
        planet_generators::bake::round::solve_rounds(&grid, &mut z, &outlet, &p, 3, &1usize, 15);
        z.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    };
    assert_eq!(run(false), run(true), "no window-cut outlet: the flag moves no bit");
}

// spec: GEN-146
// spec: GEN-172
// spec: TEST-118
/// The key of a catchment round holds every parameter `catchment_round` reads (inbox D28): flipping one changes the key, the same inputs keep it.
#[test]
fn catchment_key_holds_every_parameter_the_round_reads() {
    use planet_generators::bake::carve::CarveParams;
    use planet_generators::bake::round::{catchment_key, CatchmentInputs};
    let grid = Grid::new(4, 3, 100.0);
    let cells = [0u32, 1, 2, 3];
    let inp = CatchmentInputs {
        z: vec![10.0, 20.0, 30.0, 40.0],
        receiver: vec![u32::MAX, 0, 1, 2],
        area: vec![4e4, 3e4, 2e4, 1e4],
        dist: vec![100.0; 4],
        taper: vec![1.0; 4],
        receiver_is_outlet: vec![false; 4],
        scale: vec![1.0; 4],
        up_unit: vec![0.0; 4],
        exit_window_cut: false,
    };
    let p = CarveParams { round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let key = catchment_key(&grid, &cells, &inp, &p);
    assert_eq!(key, catchment_key(&grid, &cells, &inp.clone(), &p.clone()), "identical inputs keep the key");
    let changed: Vec<(&str, CarveParams)> = vec![
        ("clip_mfd", CarveParams { round_rules: RoundRules::default(), ..p }),
        ("k_dt", CarveParams { k_dt: p.k_dt * 1.5, ..p }),
        ("epsilon", CarveParams { epsilon: p.epsilon * 2.0, ..p }),
        ("breach_slope", CarveParams { breach_slope: p.breach_slope + 1e-3, ..p }),
        ("breach_max_m", CarveParams { breach_max_m: p.breach_max_m + 1.0, ..p }),
        ("breach_height_scale_m", CarveParams { breach_height_scale_m: p.breach_height_scale_m + 100.0, ..p }),
        ("sea_level", CarveParams { sea_level: p.sea_level + 1.0, ..p }),
        ("keep_pits", CarveParams { keep_pits: !p.keep_pits, ..p }),
        ("incise_only", CarveParams { incise_only: !p.incise_only, ..p }),
        ("reroute_every", CarveParams { reroute_every: p.reroute_every + 1, ..p }),
    ];
    for (name, q) in changed {
        assert_ne!(key, catchment_key(&grid, &cells, &inp, &q), "flipping {name} must change the catchment key");
    }
    // the outlet kind of the catchment is read by the round (GEN-172)
    let cut = CatchmentInputs { exit_window_cut: true, ..inp.clone() };
    assert_ne!(key, catchment_key(&grid, &cells, &cut, &p), "the outlet kind must change the catchment key");
}

// spec: GEN-146
/// D31: a catchment round visits its members in `Routing.order` (the depth-first walk of the donors from the outlet in cell-id order, the `cut` order), a function of the ids and the receivers, which are
/// key inputs. The key hashes the members in that order, and a round on the same members in another order is not a key match.
#[test]
fn the_catchment_key_is_the_members_in_the_routing_order_and_another_order_does_not_match() {
    use planet_generators::bake::carve::CarveParams;
    use planet_generators::bake::round::{catchment_key, cut_in, gather, CatchmentInputs};
    let z = terrain(5, true);
    let g = Grid::new(N, N, 1000.0);
    let outlet = g.outlets(&z, 0.0);
    let p = CarveParams { round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let r = route_rules(&g, &z, &outlet, p.epsilon, 5, 0.0, None, RoundRules::ROUND);
    let cuts = cut_in(Some(&g), &r);
    let taper: Vec<f32> = z.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect();
    let k = (0..cuts.members.len()).max_by_key(|&k| cuts.members[k].len()).unwrap();
    let cells = cuts.members[k].clone();
    assert!(cells.len() > 50, "a catchment with something to permute");
    // (a) the members are the catchment's cells in the order of the routing, the outlet first.
    let root = catchment_roots(&r.receiver, &r.order);
    let in_order: Vec<u32> = r.order.iter().copied().filter(|&c| root[c as usize] == root[cells[0] as usize]).collect();
    assert_eq!(cells, in_order, "the members of a catchment are in the order of `Routing.order`");
    let inp = gather(&g, &z, &outlet, &r, &cuts, &taper, &p, k);
    let key = catchment_key(&g, &cells, &inp, &p);
    assert_eq!(
        key,
        catchment_key(&g, &cells, &gather(&g, &z, &outlet, &r, &cuts, &taper, &p, k), &p),
        "the same members in the same order: the same key"
    );
    // (b) the same members and the same data in another order (two members past the outlet swapped; the receivers are positions, so they are re-pointed): not a key match.
    let n = cells.len();
    let mut perm: Vec<usize> = (0..n).collect();
    perm.swap(n / 3, n / 3 + 1);
    let mut inverse = vec![0usize; n];
    for (new, &old) in perm.iter().enumerate() {
        inverse[old] = new;
    }
    let pick = |v: &Vec<f64>| perm.iter().map(|&o| v[o]).collect::<Vec<_>>();
    let permuted = CatchmentInputs {
        z: pick(&inp.z),
        receiver: perm
            .iter()
            .map(|&o| if inp.receiver[o] == u32::MAX { u32::MAX } else { inverse[inp.receiver[o] as usize] as u32 })
            .collect(),
        area: pick(&inp.area),
        dist: pick(&inp.dist),
        taper: perm.iter().map(|&o| inp.taper[o]).collect(),
        receiver_is_outlet: perm.iter().map(|&o| inp.receiver_is_outlet[o]).collect(),
        scale: pick(&inp.scale),
        up_unit: pick(&inp.up_unit),
        exit_window_cut: inp.exit_window_cut,
    };
    let cells_permuted: Vec<u32> = perm.iter().map(|&o| cells[o]).collect();
    assert_ne!(key, catchment_key(&g, &cells_permuted, &permuted, &p), "a round on the same members in another order is not a key match");
    // The cell ids alone in another order, the data kept: not a match either (the key hashes the members in the order they are given).
    assert_ne!(key, catchment_key(&g, &cells_permuted, &inp, &p), "the member list is part of the key in its order");
}
