//! Tier A, GEN-147 (A3 section 6.6): a landmass of the cube as a compact grid (only its cells, indexed by rank) gives the bits of the same solve on the masked cube grid: neighbours, areas, distances, the
//! routing and the rounds. Small cubes; no store read.

use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const N: usize = 24;
const R: f64 = 6_371_000.0;

#[test]
// spec: GEN-147
fn a_compact_landmass_is_the_masked_cube_bit_for_bit() {
    let full = Grid::cube(N, R);
    let cube = planet_generators::bake::h0::GlobalGrid::new(N, R);
    let z_all: Vec<f64> = (0..full.len())
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
    let land: Vec<bool> = z_all.iter().map(|&v| v > 0.0).collect();
    let active: Vec<bool> = (0..z_all.len()).map(|p| land[p] || (0..8).filter_map(|d| full.neighbour(p, d)).any(|q| land[q])).collect();
    assert!(active.iter().any(|&a| !a) && land.iter().any(|&l| l));
    let masked = Grid::cube(N, R).with_active(active.clone());
    let mut bits = vec![0u64; (6 * N * N).div_ceil(64)];
    for (g, &a) in active.iter().enumerate() {
        if a {
            bits[g >> 6] |= 1 << (g & 63);
        }
    }
    let compact = Grid::compact_cube(N, R, bits);
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    assert_eq!(compact.len(), ids.len());
    // The grid: neighbours, areas, distances, global ids.
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(compact.cell_area(p).to_bits(), masked.cell_area(g).to_bits());
        assert_eq!(compact.global_id(p), masked.global_id(g));
        for d in 0..8 {
            let want = masked.neighbour(g, d);
            let got = compact.neighbour(p, d).map(|q| ids[q]);
            assert_eq!(got, want, "neighbour {d} of {g}");
            if want.is_some() {
                assert_eq!(compact.distance_at(p, d).to_bits(), masked.distance_at(g, d).to_bits());
            }
        }
    }
    // The routing and the rounds.
    let z: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet_masked: Vec<bool> = (0..z_all.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    let outlet: Vec<bool> = ids.iter().map(|&g| outlet_masked[g]).collect();
    let (rm, rc) = (
        route_rules(&masked, &z_all, &outlet_masked, 1e-3, 5, 0.0, None, RoundRules::ROUND),
        route_rules(&compact, &z, &outlet, 1e-3, 5, 0.0, None, RoundRules::ROUND),
    );
    for (p, &g) in ids.iter().enumerate() {
        let want = rm.receiver[g];
        let got = if rc.receiver[p] == NONE { NONE } else { ids[rc.receiver[p] as usize] as u32 };
        assert_eq!(got, want, "receiver of {g}");
        assert_eq!(rc.area[p].to_bits(), rm.area[g].to_bits(), "A_d of {g}");
    }
    let params = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let (mut zm, mut zc) = (z_all.clone(), z.clone());
    solve_rounds(&masked, &mut zm, &outlet_masked, &params, 2, &3usize, 5);
    solve_rounds(&compact, &mut zc, &outlet, &params, 2, &3usize, 5);
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(zc[p].to_bits(), zm[g].to_bits(), "the rounds at {g}");
    }
}

/// A landmass that is a cap of the sphere around a cube corner (under a third of the cells), on a cube whose cell count is not a multiple of 64.
fn corner_cap(n: usize, cos_limit: f64) -> (Vec<bool>, Vec<u64>) {
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let c = (1.0f64, 1.0f64, 1.0f64);
    let norm = libm::sqrt(c.0 * c.0 + c.1 * c.1 + c.2 * c.2);
    let active: Vec<bool> = (0..6 * n * n)
        .map(|g| {
            let d = cube.direction(g as u32);
            (d.x * c.0 + d.y * c.1 + d.z * c.2) / norm > cos_limit
        })
        .collect();
    let mut bits = vec![0u64; (6 * n * n).div_ceil(64)];
    for (g, &a) in active.iter().enumerate() {
        if a {
            bits[g >> 6] |= 1 << (g & 63);
        }
    }
    (active, bits)
}

// spec: GEN-147
#[test]
fn a_cap_over_a_cube_corner_with_empty_words_is_the_masked_cube_and_the_misuse_is_refused() {
    let n = 25; // 3,750 cells: not a multiple of 64
    let (active, bits) = corner_cap(n, 0.8);
    let share = active.iter().filter(|&&a| a).count() as f64 / active.len() as f64;
    assert!(share > 0.02 && share < 0.3, "a minority landmass: {share}");
    assert!(bits.iter().filter(|&&w| w == 0).count() > 3, "words with no active cell");
    let masked = Grid::cube(n, R).with_active(active.clone());
    let compact = Grid::compact_cube(n, R, bits);
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let mut crossings = 0;
    for (p, &g) in ids.iter().enumerate() {
        for d in 0..8 {
            let want = masked.neighbour(g, d);
            assert_eq!(compact.neighbour(p, d).map(|q| ids[q]), want, "neighbour {d} of {g}");
            if let Some(q) = want {
                crossings += usize::from(q / (n * n) != g / (n * n));
            }
        }
    }
    assert!(crossings > 0, "the cap crosses cube edges");
    // The routing, resident, on the cap.
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (1.0 - (d.x + d.y + d.z) / 1.7320508) + 5.0 * planet_generators::bake::route::jitter(2, g as u64, 0) - 30.0
        })
        .collect();
    let outlet_m: Vec<bool> = (0..active.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    assert!(outlet_m.iter().any(|&o| o));
    let z: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet: Vec<bool> = ids.iter().map(|&g| outlet_m[g]).collect();
    let rm = route_rules(&masked, &z_all, &outlet_m, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    let rc = route_rules(&compact, &z, &outlet, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(rc.area[p].to_bits(), rm.area[g].to_bits(), "A_d of {g}");
    }
}

// spec: GEN-147
#[test]
#[should_panic(expected = "past the last cube cell")]
fn a_bit_past_the_last_cube_cell_is_refused() {
    let n = 4; // 96 cells in two words
    let mut bits = vec![0u64; 2];
    bits[0] = u64::MAX;
    bits[1] = 1 << 40; // bit 104
    Grid::compact_cube(n, R, bits);
}

// spec: GEN-149
#[test]
fn the_partitioned_route_on_a_compact_landmass_is_the_resident_route_and_the_masked_cube_for_any_tile_size() {
    use planet_generators::bake::partition::{partitioned_route, PartCache};
    use planet_generators::bake::round::{solve_rounds_memo, Drivers, RoundCache};
    let n = 24;
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, R, bits);
    let masked = Grid::cube(n, R).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    // The landmass spans at least 2 faces, so tiles straddle a face seam of the stacked layout.
    let faces: std::collections::BTreeSet<usize> = (0..compact.len()).map(|p| compact.xy(p).1 / n).collect();
    assert!(faces.len() >= 2, "the landmass spans {} face(s)", faces.len());
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let z: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet_masked: Vec<bool> = (0..active.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    let outlet: Vec<bool> = ids.iter().map(|&g| outlet_masked[g]).collect();
    assert!(outlet.iter().any(|&o| o) && outlet.iter().any(|&o| !o));
    let want = route_rules(&compact, &z, &outlet, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    let want_masked = route_rules(&masked, &z_all, &outlet_masked, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    for tile in [4usize, 6, 8, 12, 24] {
        let got = partitioned_route(&compact, &z, &outlet, 1e-3, 5, tile, RoundRules::ROUND, &mut PartCache::default());
        assert_eq!(got.routing.receiver, want.receiver, "receivers, tile {tile}");
        assert!(got.routing.area.iter().zip(&want.area).all(|(a, b)| a.to_bits() == b.to_bits()), "A_d, tile {tile}");
        // The masked cube: the same bits, by cube id.
        let masked_part = partitioned_route(&masked, &z_all, &outlet_masked, 1e-3, 5, tile, RoundRules::ROUND, &mut PartCache::default());
        for (p, &g) in ids.iter().enumerate() {
            assert_eq!(got.routing.area[p].to_bits(), masked_part.routing.area[g].to_bits(), "tile {tile}, cell {g}");
            assert_eq!(got.routing.area[p].to_bits(), want_masked.area[g].to_bits(), "tile {tile}, cell {g}");
        }
    }
    // The memoised rounds on the compact grid are the rounds without a cache, and a second solve of the same surface floods nothing.
    let params = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let mut plain = z.clone();
    solve_rounds(&compact, &mut plain, &outlet, &params, 2, &3usize, 5);
    let mut cache = RoundCache::default();
    let mut memo = z.clone();
    solve_rounds_memo(&compact, &mut memo, &outlet, &params, 2, &3usize, 5, &Drivers::default(), 8, &mut cache);
    assert!(plain.iter().zip(&memo).all(|(a, b)| a.to_bits() == b.to_bits()), "the memoised rounds on a compact grid");
    let mut again = z.clone();
    let (_, work) = solve_rounds_memo(&compact, &mut again, &outlet, &params, 2, &3usize, 5, &Drivers::default(), 8, &mut cache);
    assert!(again.iter().zip(&memo).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert!(work.iter().all(|w| w.tile_floods == 0), "the same surface again floods no tile");
    // A cache without memory keeps nothing and gives the same bits.
    let mut forgetful = RoundCache::without_memory();
    let mut none = z.clone();
    solve_rounds_memo(&compact, &mut none, &outlet, &params, 2, &3usize, 5, &Drivers::default(), 8, &mut forgetful);
    assert!(none.iter().zip(&memo).all(|(a, b)| a.to_bits() == b.to_bits()), "a cache without memory");
    assert!(forgetful.is_empty(), "no catchment is kept");
}

// spec: GEN-149
#[test]
fn one_partition_cache_across_landmasses_of_one_face_edge_gives_the_fresh_bits() {
    use planet_generators::bake::partition::{partitioned_route, PartCache};
    let n = 24;
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let mut cache = PartCache::default();
    for cos in [0.2, 0.5, 0.1, 0.2] {
        let (active, bits) = corner_cap(n, cos);
        let grid = Grid::compact_cube(n, R, bits);
        let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
        let z: Vec<f64> = ids
            .iter()
            .map(|&g| {
                let d = cube.direction(g as u32);
                900.0 * (d.x + d.y + d.z) + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0) - 500.0
            })
            .collect();
        let lo = z.iter().cloned().fold(f64::INFINITY, f64::min);
        let outlet: Vec<bool> = z.iter().map(|&h| h <= 0.0 || h == lo).collect();
        let fresh = partitioned_route(&grid, &z, &outlet, 1e-3, 5, 8, RoundRules::ROUND, &mut PartCache::default());
        let shared = partitioned_route(&grid, &z, &outlet, 1e-3, 5, 8, RoundRules::ROUND, &mut cache);
        assert_eq!(shared.routing.receiver, fresh.routing.receiver, "cap {cos}");
        assert!(shared.routing.area.iter().zip(&fresh.routing.area).all(|(a, b)| a.to_bits() == b.to_bits()), "cap {cos}");
        assert_eq!(shared.stats.tiles_filled_changed, fresh.stats.tiles_filled_changed, "cap {cos}");
    }
}

// spec: GEN-147
#[test]
#[should_panic(expected = "no mask")]
fn a_compact_grid_takes_no_mask() {
    let n = 8;
    let (_, bits) = corner_cap(n, 0.2);
    let g = Grid::compact_cube(n, R, bits);
    let len = g.len();
    g.with_active(vec![true; len]);
}

// spec: GEN-150
#[test]
fn deposition_and_the_pit_spreading_on_a_cube_grid_keep_the_volume_and_are_the_masked_cube_on_a_compact_one() {
    use planet_generators::bake::deposit::{deposit_on_own_route, spread_pits, DepositParams, Kc};
    let n = 24;
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, R, bits);
    let masked = Grid::cube(n, R).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    // The ground before the carve: up to 12 m above `z` on land.
    let original_all: Vec<f64> =
        (0..active.len()).map(|g| z_all[g] + 12.0 * (0.5 + 0.5 * planet_generators::bake::route::jitter(9, g as u64, 1))).collect();
    let params = DepositParams { kc: Kc::Uniform(1e3), zeta: 0.5, sea_level: 0.0 };
    let (mut zm, mut zc) = (z_all.clone(), ids.iter().map(|&g| z_all[g]).collect::<Vec<f64>>());
    let orig_c: Vec<f64> = ids.iter().map(|&g| original_all[g]).collect();
    let rm = deposit_on_own_route(&masked, &mut zm, &original_all, 1e-3, 5, &params);
    let rc = deposit_on_own_route(&compact, &mut zc, &orig_c, 1e-3, 5, &params);
    assert!(rc.eroded > 0.0 && rc.deposited > 0.0, "{rc:?}");
    assert!((rc.eroded - (rc.deposited + rc.exported)).abs() < 1e-9 * rc.eroded, "eroded = deposited + exported: {rc:?}");
    assert_eq!(rc.eroded.to_bits(), rm.eroded.to_bits());
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(zc[p].to_bits(), zm[g].to_bits(), "deposition at {g}");
    }
    // The pits: land cells dug 500 m below their ring.
    let pits: Vec<usize> =
        (0..ids.len()).filter(|&p| p % 5 == 0 && zc[p] > 900.0 && (0..8).all(|d| compact.neighbour(p, d).is_some())).collect();
    assert!(pits.len() > 5);
    // Pits on the cube's edges and corner: fewer than 8 neighbours, as many as the cube gives them.
    let full = Grid::cube(n, R);
    let count = |g: &Grid, p: usize| (0..8).filter(|&d| g.neighbour(p, d).is_some()).count();
    let edge: Vec<usize> =
        (0..ids.len()).filter(|&p| zc[p] > 900.0 && count(&compact, p) < 8 && count(&compact, p) == count(&full, ids[p])).collect();
    assert!(!edge.is_empty(), "pits on a cube edge");
    let pits: Vec<usize> = pits.into_iter().chain(edge.iter().copied()).collect();
    for &p in &pits {
        zc[p] -= 500.0;
        zm[ids[p]] -= 500.0;
    }
    let volume = |g: &Grid, z: &[f64]| (0..g.len()).map(|p| z[p] * g.cell_area(p)).sum::<f64>();
    let (v0, before) = (volume(&compact, &zc), zc.clone());
    let passes = spread_pits(&compact, &mut zc, 0.0, 0.5, 32);
    assert!(passes > 0 && zc.iter().zip(&before).any(|(a, b)| a != b), "the pits were spread: {passes} passes, {} pits", pits.len());
    assert!((volume(&compact, &zc) - v0).abs() < 1e-9 * v0.abs().max(1.0), "the volume is kept");
    let moved = edge.iter().filter(|&&p| zc[p] > before[p]).count();
    assert!(moved > 0, "a pit on a cube edge was spread: {moved} of {}", edge.len());
    assert_eq!(spread_pits(&masked, &mut zm, 0.0, 0.5, 32), passes);
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(zc[p].to_bits(), zm[g].to_bits(), "pit spreading at {g}");
    }
}

// spec: GEN-150
#[test]
fn thermal_on_a_cube_grid_keeps_the_volume_and_is_the_masked_cube_on_a_compact_one() {
    use planet_generators::bake::thermal::{thermal, thermal_all_cells, ThermalParams};
    let n = 24;
    let small = 4000.0; // a small body: cells of about 260 m, so towers of tens of metres are steep
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, small, bits);
    let masked = Grid::cube(n, small).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| 50.0 + 40.0 * planet_generators::bake::route::jitter(3, g as u64, 0) + if g % 29 == 0 { 400.0 } else { 0.0 })
        .collect();
    let z0: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let params = ThermalParams { passes: 40, ..ThermalParams::default() };
    let volume = |g: &Grid, z: &[f64]| (0..g.len()).map(|p| z[p] * g.cell_area(p)).sum::<f64>();
    let (mut zc, mut za, mut zm) = (z0.clone(), z0.clone(), z_all.clone());
    let rc = thermal(&compact, &mut zc, &params, 3);
    thermal_all_cells(&compact, &mut za, &params, 1);
    thermal(&masked, &mut zm, &params, 5);
    assert!(rc.moved_volume_first > 0.0 && zc.iter().zip(&z0).any(|(a, b)| a != b), "thermal moved nothing: {rc:?}");
    let (v0, v1) = (volume(&compact, &z0), volume(&compact, &zc));
    assert!((v1 - v0).abs() < 1e-9 * v0.abs(), "the volume is kept: {v0} against {v1}");
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(zc[p].to_bits(), za[p].to_bits(), "the active set against every cell at {g}");
        assert_eq!(zc[p].to_bits(), zm[g].to_bits(), "the compact grid against the masked cube at {g}");
    }
}

// spec: GEN-151
#[test]
fn the_sediment_pass_on_a_compact_landmass_after_the_rounds_conserves_and_is_the_masked_cube() {
    use planet_generators::bake::round::solve_rounds;
    use planet_generators::bake::sediment::sediment_pass;
    use planet_generators::bake::tile::Pass2Params;
    let n = 24;
    let small = 4000.0; // a small body: cells of about 260 m, steep enough for thermal
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, small, bits);
    let masked = Grid::cube(n, small).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let z0: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= 0.0).collect();
    let outlet_m: Vec<bool> = (0..active.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    let rounds = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let (mut zc, mut zm) = (z0.clone(), z_all.clone());
    solve_rounds(&compact, &mut zc, &outlet, &rounds, 2, &3usize, 5);
    solve_rounds(&masked, &mut zm, &outlet_m, &rounds, 2, &3usize, 5);
    let (start_c, start_m) = (z0.clone(), z_all.clone());
    let mut q = Pass2Params::standard(5, 0.0);
    q.carve.breach_max_m = CarveParams::default().breach_max_m;
    let rc = sediment_pass(&compact, &mut zc, &start_c, &q, 3);
    let rm = sediment_pass(&masked, &mut zm, &start_m, &q, 1);
    // G-SED: what was eroded is deposited or leaves; thermal and the pit spreading keep the volume.
    let d = rc.deposit;
    assert!(d.eroded > 0.0 && (d.eroded - d.deposited - d.exported).abs() < 1e-9 * d.eroded, "{d:?}");
    // The heights moved by what the sweep reports (not only its bookkeeping).
    assert!(
        (rc.deposit_height_volume_m3 - d.deposited).abs() < 1e-9 * d.deposited,
        "{} against {}",
        rc.deposit_height_volume_m3,
        d.deposited
    );
    assert!(rc.thermal_volume_change.abs() < 1e-9 && rc.spread_volume_change.abs() < 1e-9, "{rc:?}");
    let names: Vec<&str> = rc.pits.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["start", "after thermal", "after deposition", "after pit spreading"]);
    for (p, &g) in ids.iter().enumerate() {
        assert_eq!(zc[p].to_bits(), zm[g].to_bits(), "the sediment pass at {g}");
    }
    assert_eq!(rc.deposit.eroded.to_bits(), rm.deposit.eroded.to_bits());
    assert!(rc.thermal.moved_volume_first > 0.0, "thermal moved nothing");
    // The breach alone is a finite, non-negative volume (its share of the eroded column is measured by `landmass_scale sed`).
    // The boundary of the landmass is outlets for the steps: the coast ring.
    let boundary = planet_generators::bake::sediment::boundary(&compact);
    assert!(boundary.iter().any(|&b| b) && boundary.iter().any(|&b| !b));
    // The deposition reads the eroded column after thermal: thermal's moves count as load where the receiver is above `original` (K7 does the same).
    let flat: Vec<f64> = zc.clone();
    let mut steep = flat.clone();
    let r0 = sediment_pass(&compact, &mut steep, &flat, &q, 1);
    assert_eq!(r0.eroded_before_thermal_m3, 0.0);
    assert!(r0.eroded_after_thermal_m3 > 0.0, "thermal's moves are counted as eroded load: {r0:?}");
}

// spec: GEN-151
#[test]
fn the_breach_alone_lowers_a_ridge_to_the_spill_and_nothing_on_a_surface_without_pits() {
    use planet_generators::bake::sediment::breach_lowered_m3;
    let (n, dx) = (16usize, 100.0);
    let g = Grid::new(n, n, dx);
    // A slope falling to the column x = 0 (the outlets), 5 m a cell.
    let slope: Vec<f64> = (0..n * n).map(|p| 10.0 + (p % n) as f64 * 5.0).collect();
    let outlet = g.outlets(&slope, 0.0);
    let carve = CarveParams { steps: 1, reroute_every: 1, sea_level: 0.0, ..CarveParams::default() };
    assert_eq!(breach_lowered_m3(&g, &slope, &outlet, &carve, 5), 0.0, "no pit, no breach");
    // A bowl: the cell (8, 8) is 30 m under its slope and ringed by a ridge 40 m over it.
    let mut bowl = slope.clone();
    bowl[8 * n + 8] -= 30.0;
    for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        bowl[((8 + dj) as usize) * n + (8 + di) as usize] += 40.0;
    }
    let lowered = breach_lowered_m3(&g, &bowl, &outlet, &carve, 5);
    assert!(lowered > 0.0 && lowered < 40.0 * 8.0 * dx * dx, "the breach lowers part of the ridge: {lowered}");
    // With no breach allowed nothing is lowered.
    assert_eq!(breach_lowered_m3(&g, &bowl, &outlet, &CarveParams { breach_max_m: 0.0, ..carve }, 5), 0.0);
}

// spec: GEN-152
#[test]
fn the_deposition_balances_per_catchment_and_per_round_on_a_compact_landmass() {
    use planet_generators::bake::round::solve_rounds;
    use planet_generators::bake::sediment::catchment_balance;
    use planet_generators::bake::tile::Pass2Params;
    let n = 24;
    let small = 4000.0;
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, small, bits);
    let masked = Grid::cube(n, small).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let z0: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= 0.0).collect();
    let outlet_m: Vec<bool> = (0..active.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    let mut q = Pass2Params::standard(5, 0.0);
    q.carve.breach_max_m = CarveParams::default().breach_max_m;
    let mut checked = 0;
    for rounds in 1..=3usize {
        let p = CarveParams { steps: 5 * rounds, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
        let prev = CarveParams { steps: 5 * (rounds - 1).max(1), ..p };
        let (mut zr, mut zp) = (z0.clone(), z0.clone());
        solve_rounds(&compact, &mut zr, &outlet, &p, rounds, &3usize, 5);
        if rounds > 1 {
            solve_rounds(&compact, &mut zp, &outlet, &prev, rounds - 1, &3usize, 5);
        }
        let (mut zm, mut zpm) = (z_all.clone(), z_all.clone());
        solve_rounds(&masked, &mut zm, &outlet_m, &p, rounds, &1usize, 5);
        if rounds > 1 {
            solve_rounds(&masked, &mut zpm, &outlet_m, &prev, rounds - 1, &1usize, 5);
        }
        let b = catchment_balance(&compact, &mut zr, &zp, &q);
        let bm = catchment_balance(&masked, &mut zm, &zpm, &q);
        assert!(b.catchments > 1 && b.eroded_m3 > 0.0, "round {rounds}: {b:?}");
        assert!(b.max_relative_error < 1e-9 && b.total_relative_error < 1e-9, "round {rounds}: {b:?}");
        assert_eq!(b.eroded_m3.to_bits(), bm.eroded_m3.to_bits(), "compact against masked, round {rounds}");
        assert_eq!(b.catchments, bm.catchments);
        assert!(bm.max_relative_error < 1e-9 && bm.total_relative_error < 1e-9, "masked, round {rounds}: {bm:?}");
        assert_eq!(b.raised_m3.to_bits(), bm.raised_m3.to_bits(), "raised, round {rounds}");
        assert_eq!(b.exported_m3.to_bits(), bm.exported_m3.to_bits(), "exported, round {rounds}");
        checked += 1;
    }
    assert_eq!(checked, 3);
}

// spec: GEN-158, GEN-178, TEST-126
#[test]
fn the_phase_plan_is_the_steps_in_order_releases_what_it_should_and_does_not_move_a_bit_with_the_probe_on() {
    use planet_generators::bake::build::{build_resident, PlanParams};
    use planet_generators::bake::hydro::{hydrology, verify, HydroParams};
    use planet_generators::bake::probe::{self, Stage, StageProbe, StageView};
    use planet_generators::bake::sediment::sediment_pass;
    use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
    use std::sync::{Arc, Mutex};
    struct Sink(Mutex<Vec<String>>);
    impl StageProbe for Sink {
        fn stage(&self, view: &StageView<'_>) {
            self.0.lock().unwrap().push(view.stage.label());
        }
    }
    let n = 24;
    let small = 4000.0;
    let (active, bits) = corner_cap(n, 0.2);
    let grid = Grid::compact_cube(n, small, bits);
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let z0: Vec<f64> = ids
        .iter()
        .map(|&g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= 0.0).collect();
    // A climate that varies over the cap: wetter and warmer towards the corner.
    let climate_of = |p: usize| -> (f64, f64) {
        let d = cube.direction(ids[p] as u32);
        (900.0 + 700.0 * (d.x + d.y + d.z) / 1.7, 8.0 + 14.0 * d.z)
    };
    let spill = std::env::temp_dir().join(format!("planet-plan-{}.f64", std::process::id()));
    let mut params = PlanParams::standard(5, 3, spill.clone());
    params.rounds = 2;
    params.carve.steps = 10;
    let marks = Mutex::new(Vec::<String>::new());
    let mark = |name: &str| marks.lock().unwrap().push(name.to_string());

    let off = build_resident(&grid, z0.clone(), &outlet, &params, &climate_of, &mark).expect("the plan runs");
    assert!(!spill.exists(), "the spill file is deleted after the read-back");
    assert_eq!(off.report.spill_bytes, z0.len() as u64 * 8);
    assert!(off.hydro.q.is_empty() && off.hydro.qw.is_empty(), "the per-cell discharges are released");
    assert_eq!(off.hydro.label.len(), z0.len(), "the labels are kept");
    // spec: GEN-163, TEST-108: the lake producer is a phase of the plan and gives one lake per kept basin, without a channel cell inside one.
    assert!(off.report.phases.iter().any(|(name, _)| *name == "lakes"), "the plan lists the lake step: {:?}", off.report.phases);
    assert_eq!(off.lakes.lakes.len(), off.hydro.basins.len());
    assert!(off.lakes.pieces.iter().all(|p| p.cells > 0 && off.hydro.label[p.first as usize] == planet_generators::bake::route::NONE));
    for name in [
        "pre-carve surface spilled",
        "after the rounds",
        "drivers",
        "routing and filled surface",
        "weight dropped",
        "hydrology",
        "lakes",
        "H1 result kept, discharges released",
        "pre-carve surface read back",
        "after the sediment pass",
    ] {
        assert!(marks.lock().unwrap().iter().any(|m| m == name), "the allocation boundary {name} is marked");
    }
    // G-CON: the conservation of H1 holds to rounding, checked before the release.
    let v = &off.report.g_con;
    assert!(
        v.max_confluence_error < 1e-9
            && v.q_outlets_error < 1e-9
            && v.qw_closure_error < 1e-9
            && v.qw_confluence_error < 1e-9
            && v.area_outlets_error < 1e-9,
        "{v:?}"
    );
    assert_eq!((v.disconnected_segments, v.broken_channel_cells, v.miscovered_cells), (0, 0, 0));

    // The steps by hand, in order: the rounds, H1 on that surface, the sediment pass against the pre-carve surface.
    let mut z = z0.clone();
    solve_rounds(&grid, &mut z, &outlet, &params.carve, 2, &3usize, 5);
    let z_rounds = z.clone();
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; z.len()], vec![0.0f32; z.len()], vec![0.0f32; z.len()]);
    for p in 0..z.len() {
        if z[p] > 0.0 {
            let (pm, tc) = climate_of(p);
            let ro = runoff_mm(pm, tc);
            weight[p] = discharge_weight(ro) as f32;
            runoff[p] = (ro / 1000.0 / 31_557_600.0) as f32;
            evap[p] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / 31_557_600.0) as f32;
        }
    }
    let routing = route_rules(&grid, &z, &outlet, 1e-3, 5, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = planet_generators::bake::flood::priority_flood_with(&grid, &z, &outlet, 1e-3, true);
    let hy = hydrology(&grid, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    let v_hand = verify(&grid, &hy, &routing, &filled, &outlet, Some(&runoff), Some(&weight));
    assert_eq!(
        format!("{:?}", off.report.g_con),
        format!("{v_hand:?}"),
        "the area reference taken before the weights are dropped is the one verify sums"
    );
    assert_eq!(off.hydro.label, hy.label);
    assert_eq!(off.hydro.channel, hy.channel);
    assert_eq!(off.hydro.segments, hy.segments);
    assert_eq!(format!("{:?}", off.report.h1), format!("{:?}", hy.stats));
    assert_eq!(off.hydro.basins, hy.basins);
    let mut q = params.sediment;
    q.carve.breach_max_m = CarveParams::default().breach_max_m;
    sediment_pass(&grid, &mut z, &z0, &q, 3);
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&off.z), bits(&z), "the plan's surface is the steps in order (the spill reads back the same bits)");
    assert_ne!(bits(&z), bits(&z_rounds), "the sediment pass moved the surface");

    // The probe on: every stage of the build is emitted, and no bit moves.
    let sink = Arc::new(Sink(Mutex::new(Vec::new())));
    probe::install_local(sink.clone());
    let on = build_resident(&grid, z0.clone(), &outlet, &params, &climate_of, &|_| {}).expect("the plan runs with the probe");
    probe::uninstall_local();
    assert_eq!(bits(&on.z), bits(&off.z), "the probe moves no bit");
    // One thread gives the bits of three: no chunk boundary enters the plan.
    let mut one_thread = params.clone();
    one_thread.threads = 1;
    let one = build_resident(&grid, z0.clone(), &outlet, &one_thread, &climate_of, &|_| {}).expect("the plan runs on one thread");
    assert_eq!(bits(&one.z), bits(&off.z), "the plan does not depend on the thread count");
    assert_eq!(one.hydro.label, off.hydro.label);
    assert_eq!(one.hydro.segments, off.hydro.segments);
    assert_eq!(on.hydro.label, off.hydro.label);
    let seen = sink.0.lock().unwrap().clone();
    let want = [Stage::Landmass, Stage::Round(0), Stage::Round(1), Stage::H1, Stage::Thermal, Stage::Deposition, Stage::PitSpreading]
        .map(|s| s.label());
    assert_eq!(seen, want, "every stage of the build is emitted, once, in order");
}

// spec: GEN-155
#[test]
fn the_sediment_balance_is_the_same_under_every_partition_the_pass_has_and_no_load_is_lost_or_counted_twice() {
    use planet_generators::bake::round::solve_rounds;
    use planet_generators::bake::sediment::{catchment_terms, sediment_pass};
    use planet_generators::bake::tile::Pass2Params;
    let n = 64; // about 9,800 cap cells: three pieces of the thermal step's 4,096
    let small = 4000.0;
    let (active, bits) = corner_cap(n, 0.2);
    let compact = Grid::compact_cube(n, small, bits);
    let masked = Grid::cube(n, small).with_active(active.clone());
    let ids: Vec<usize> = (0..active.len()).filter(|&g| active[g]).collect();
    let cube = planet_generators::bake::h0::GlobalGrid::new(n, R);
    let z_all: Vec<f64> = (0..active.len())
        .map(|g| {
            let d = cube.direction(g as u32);
            900.0 * (d.x + d.y + d.z)
                + 90.0 * libm::sin(11.0 * d.x + 5.0 * d.y) * libm::cos(8.0 * d.z)
                + 2.0 * planet_generators::bake::route::jitter(5, g as u64, 0)
                - 500.0
        })
        .collect();
    let z0: Vec<f64> = ids.iter().map(|&g| z_all[g]).collect();
    let outlet: Vec<bool> = z0.iter().map(|&h| h <= 0.0).collect();
    let outlet_m: Vec<bool> = (0..active.len()).map(|g| active[g] && z_all[g] <= 0.0).collect();
    let mut q = Pass2Params::standard(5, 0.0);
    q.carve.breach_max_m = CarveParams::default().breach_max_m;
    let p = CarveParams { steps: 10, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let (mut zr, mut zm) = (z0.clone(), z_all.clone());
    solve_rounds(&compact, &mut zr, &outlet, &p, 2, &3usize, 5);
    solve_rounds(&masked, &mut zm, &outlet_m, &p, 2, &3usize, 5);
    let bits_of = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();

    // Partition 1, the representation (the cell order is the same, the ranks follow the cube id): the compact grid and the masked cube give the same catchments (keyed by the cube id of the outlet), the same terms bit for bit and the same heights.
    let (mut zc, mut zmm) = (zr.clone(), zm.clone());
    let whole = catchment_terms(&compact, &mut zc, &z0, &q);
    let by_mask = catchment_terms(&masked, &mut zmm, &z_all, &q);
    assert!(whole.len() > 3, "several catchments: {}", whole.len());
    assert_eq!(whole.len(), by_mask.len(), "the same catchments");
    for (a, b) in whole.iter().zip(&by_mask) {
        assert_eq!(ids[a.outlet], b.outlet, "the same outlet, in the same order");
        assert_eq!(
            (a.cells, a.eroded_m3.to_bits(), a.raised_m3.to_bits(), a.exported_m3.to_bits(), a.relative_error.to_bits()),
            (b.cells, b.eroded_m3.to_bits(), b.raised_m3.to_bits(), b.exported_m3.to_bits(), b.relative_error.to_bits()),
            "catchment {}",
            b.outlet
        );
    }
    let on_cap: Vec<f64> = ids.iter().map(|&g| zmm[g]).collect();
    assert_eq!(bits_of(&zc), bits_of(&on_cap), "the heights after the sweep");
    for g in (0..active.len()).filter(|&g| !active[g]) {
        assert_eq!(zmm[g].to_bits(), zm[g].to_bits(), "the masked sweep leaves the inactive cell {g} alone");
    }

    // Partition 2, the bands of the thermal step (the thread count): the whole sediment pass gives the same bits and report on 1, 3 and 8 threads.
    let run = |threads: usize| {
        let mut z = zr.clone();
        let r = sediment_pass(&compact, &mut z, &z0, &q, threads);
        (bits_of(&z), format!("{r:?}"))
    };
    assert!(compact.len() > 2 * 4096, "more than two pieces of thermal's work: {}", compact.len());
    let one = run(1);
    let moved = sediment_pass(&compact, &mut zr.clone(), &z0, &q, 1).thermal;
    assert!(moved.moved_volume_first > 0.0 && moved.passes_run > 0, "thermal moved something: {moved:?}");
    for t in [3usize, 8] {
        assert!(one.0 == run(t).0, "the heights on {t} threads differ from one thread");
        assert!(one.1 == run(t).1, "the report on {t} threads differs from one thread");
    }

    // Partition 3, groups of whole catchments (the accounting a cut along the divides would hand to separate domains; the pass itself is not cut): the sums of the terms match the heights, every catchment is in exactly one group, the groups add up to the whole and the identity holds in every group.
    let (we, wr, wx): (f64, f64, f64) =
        whole.iter().fold((0.0, 0.0, 0.0), |s, t| (s.0 + t.eroded_m3, s.1 + t.raised_m3, s.2 + t.exported_m3));
    assert!(we > 0.0 && wx > 0.0, "the scenario erodes and exports: {we} {wx}");
    // The sums of the terms against the same quantities taken another way: the eroded column read from the heights before the sweep, the rise read from the volume of the heights before and after (not from the sweep's accounting).
    let eroded_direct = planet_generators::bake::sediment::eroded_m3(&compact, &zr, &z0, q.sea_level);
    let rise_direct =
        planet_generators::bake::sediment::volume_m3(&compact, &zc) - planet_generators::bake::sediment::volume_m3(&compact, &zr);
    assert!((we - eroded_direct).abs() <= 1e-9 * we, "eroded: the terms {we} against the heights {eroded_direct}");
    assert!((wr - rise_direct).abs() <= 1e-9 * we, "raised: the terms {wr} against the volume change {rise_direct}");
    let mut worst = (0.0f64, 0usize);
    for t in &whole {
        if t.relative_error > worst.0 {
            worst = (t.relative_error, ids[t.outlet]);
        }
    }
    assert!(worst.0 < 1e-9, "the residual of the worst catchment (cube id {}): {:e}", worst.1, worst.0);
    for k in [2usize, 3, 5] {
        let mut groups = vec![(0.0f64, 0.0f64, 0.0f64, 0usize); k];
        let mut seen = 0usize;
        for t in &whole {
            let g = &mut groups[ids[t.outlet] % k];
            g.0 += t.eroded_m3;
            g.1 += t.raised_m3;
            g.2 += t.exported_m3;
            g.3 += 1;
            seen += 1;
        }
        assert_eq!(groups.iter().map(|g| g.3).sum::<usize>(), whole.len(), "k = {k}: every catchment in one group");
        assert_eq!(seen, whole.len());
        let sum = groups.iter().fold((0.0, 0.0, 0.0), |s, g| (s.0 + g.0, s.1 + g.1, s.2 + g.2));
        for (name, a, b) in [("eroded", sum.0, we), ("raised", sum.1, wr), ("exported", sum.2, wx)] {
            assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "k = {k}: {name} {a} against {b}");
        }
        assert!(groups.iter().filter(|g| g.3 > 0).count() >= 2, "k = {k}: the catchments fall into at least two groups");
        for (i, g) in groups.iter().enumerate().filter(|(_, g)| g.3 > 0) {
            let scale = g.0.max(g.1.abs()).max(g.2).max(f64::MIN_POSITIVE);
            assert!((g.0 - g.1 - g.2).abs() / scale < 1e-9, "k = {k}, group {i}: {g:?}");
        }
    }
}
