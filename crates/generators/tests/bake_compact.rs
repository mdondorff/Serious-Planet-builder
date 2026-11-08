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
