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

// spec: GEN-147
#[test]
#[should_panic(expected = "not for a compact grid")]
fn the_partitioned_route_and_a_layered_carve_refuse_a_compact_grid() {
    use planet_generators::bake::partition::{partitioned_route, PartCache};
    let n = 8;
    let (_, bits) = corner_cap(n, 0.2);
    let g = Grid::compact_cube(n, R, bits);
    let z: Vec<f64> = (0..g.len()).map(|p| 10.0 + (p % 7) as f64).collect();
    let mut outlet = vec![false; g.len()];
    outlet[0] = true;
    partitioned_route(&g, &z, &outlet, 1e-3, 1, 4, RoundRules::ROUND, &mut PartCache::default());
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
