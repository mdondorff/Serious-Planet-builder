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
