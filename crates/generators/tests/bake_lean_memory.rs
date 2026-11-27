//! Tier A, GEN-192 (TEST-157): the lean wave-1 memory paths give the bits of today's paths. K5 with its catchments in flight capped (any cap, with and without the memo) is K5
//! uncapped; hydrology with the channel initiation as bits and the MFD area freed is hydrology on the area. Small grids: a compact landmass of the cube and a plane; no store read.

use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, hydrology_initiated, initiation_bits, Hydro, HydroParams};
use planet_generators::bake::round::{solve_rounds_memo, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;

const N: usize = 24;
const R: f64 = 6_371_000.0;
const TILE: usize = 8;

/// A landmass of the cube (a tilted dome with ridges, closed pits and noise, sea around it) as a compact grid with its coastal ring, its heights and outlets.
fn landmass() -> (Grid, Vec<f64>, Vec<bool>) {
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
                - 90.0 * (libm::sin(23.0 * d.x) * libm::sin(19.0 * d.y)).max(0.0)
                - 20.0
        })
        .collect();
    let land: Vec<bool> = z_all.iter().map(|&v| v > 0.0).collect();
    let active: Vec<bool> = (0..z_all.len()).map(|p| land[p] || (0..8).filter_map(|d| full.neighbour(p, d)).any(|q| land[q])).collect();
    let mut bits = vec![0u64; (6 * N * N).div_ceil(64)];
    for (g, &a) in active.iter().enumerate() {
        if a {
            bits[g >> 6] |= 1 << (g & 63);
        }
    }
    let grid = Grid::compact_cube(N, R, bits);
    let z: Vec<f64> = (0..active.len()).filter(|&g| active[g]).map(|g| z_all[g]).collect();
    let outlet: Vec<bool> = z.iter().map(|&v| v <= 0.0).collect();
    assert!(outlet.iter().any(|&o| o) && outlet.iter().any(|&o| !o), "the landmass has land and sea");
    (grid, z, outlet)
}

/// A plane relief: two valleys, a ridge, closed pits and noise, drained at the west edge.
fn plane() -> (Grid, Vec<f64>, Vec<bool>) {
    let (w, h) = (64usize, 48usize);
    let g = Grid::new(w, h, 90.0);
    let z: Vec<f64> = (0..w * h)
        .map(|p| {
            let (x, y) = ((p % w) as f64, (p / w) as f64);
            6.0 * x
                + 40.0 * libm::sin(y / 6.0) * libm::cos(x / 9.0)
                + 15.0 * libm::sin(x * y / 50.0)
                + 2.0 * planet_generators::bake::route::jitter(9, p as u64, 0)
        })
        .collect();
    let outlet: Vec<bool> = (0..w * h).map(|p| p % w == 0).collect();
    (g, z, outlet)
}

fn params() -> CarveParams {
    CarveParams {
        steps: 15,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    }
}

fn same(a: &[f64], b: &[f64]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

#[test]
// spec: GEN-192, TEST-157
fn k5_with_the_in_flight_cap_is_k5() {
    for (name, (g, z0, outlet), tile) in [("compact cube", landmass(), TILE), ("plane", plane(), 16)] {
        let runoff: Vec<f32> = (0..g.len()).map(|p| 0.5 + (p % 7) as f32 * 0.1).collect();
        for drivers in [Drivers::default(), Drivers { runoff: Some(&runoff), ..Drivers::default() }] {
            // the reference: uncapped, on one thread
            let mut want = z0.clone();
            solve_rounds_memo(&g, &mut want, &outlet, &params(), 3, &1usize, 5, &drivers, tile, &mut RoundCache::without_memory());
            assert!(same(&want, &z0) > 0, "{name}: the rounds change the surface");
            let land = outlet.iter().filter(|&&o| !o).count();
            let runoff_on = drivers.runoff.is_some();
            for cap in [1, 7, land / 16, land / 3] {
                // the cache that keeps nothing (the chunk's heights dropped once written back) and the memoising cache, on 1 and 8 threads
                for (forget, threads) in [(true, 1usize), (true, 8), (false, 1), (false, 8)] {
                    let mut z = z0.clone();
                    let cache = if forget { RoundCache::without_memory() } else { RoundCache::default() };
                    let mut cache = cache.with_inflight_cap(cap.max(1));
                    solve_rounds_memo(&g, &mut z, &outlet, &params(), 3, &threads, 5, &drivers, tile, &mut cache);
                    let differ = same(&z, &want);
                    let at = format!("{name}, runoff {runoff_on}, cap {cap} cells, forget {forget}, {threads} threads");
                    assert_eq!(differ, 0, "{at}: {differ} cells differ in bits");
                    if !forget {
                        // the same surface again on the same capped cache: the catchments from the memo, the same bits
                        let mut again = z0.clone();
                        let (_, work) = solve_rounds_memo(&g, &mut again, &outlet, &params(), 3, &threads, 5, &drivers, tile, &mut cache);
                        assert!(work.iter().map(|w| w.memo_hits).sum::<usize>() > 0, "{at}: the second solve hits the memo");
                        assert_eq!(same(&again, &want), 0, "{at}: the memo's second solve differs in bits");
                    }
                }
            }
        }
    }
}

fn assert_same_hydro(name: &str, a: &Hydro, b: &Hydro) {
    assert_eq!(same(&a.q, &b.q), 0, "{name}: q");
    assert_eq!(same(&a.qw, &b.qw), 0, "{name}: q_w");
    assert_eq!(a.channel, b.channel, "{name}: channel cells");
    assert_eq!(a.label, b.label, "{name}: basin labels");
    assert_eq!(a.segments, b.segments, "{name}: segments");
    assert_eq!(a.basins, b.basins, "{name}: basins");
    assert_eq!(a.stats, b.stats, "{name}: stats");
}

#[test]
// spec: GEN-192, TEST-157
fn hydrology_on_initiation_bits_with_the_area_freed_is_hydrology() {
    for (name, (g, z, outlet)) in [("compact cube", landmass()), ("plane", plane())] {
        let runoff: Vec<f32> = (0..g.len()).map(|p| 1.0e-8 * (1.0 + (p % 5) as f32)).collect();
        let evap: Vec<f32> = (0..g.len()).map(|p| 2.0e-9 * (p % 3) as f32).collect();
        let weight: Vec<f32> = (0..g.len()).map(|p| 0.8 + (p % 4) as f32 * 0.1).collect();
        let routing = route_rules(&g, &z, &outlet, 1e-3, 5, 0.0, Some(&weight), RoundRules::ROUND);
        let filled = priority_flood_with(&g, &z, &outlet, 1e-3, true);
        let hp = HydroParams { initiation_m2: 4.0 * g.cell_area(0), ..HydroParams::default() };
        for (tag, ev) in [("with evaporation", Some(evap.as_slice())), ("without", None)] {
            let want = hydrology(&g, &z, &filled, &routing, &outlet, Some(&runoff), ev, &hp);
            assert!(want.stats.channel_cells > 0 && !want.basins.is_empty(), "{name}: channels and basins to compare: {:?}", want.stats);
            let bits = initiation_bits(&routing, &hp);
            let mut freed = routing.clone();
            freed.area = Vec::new();
            let got = hydrology_initiated(&g, &z, &filled, &freed, &outlet, Some(&runoff), ev, &bits, &hp);
            assert_same_hydro(&format!("{name}, {tag}"), &got, &want);
        }
    }
}

#[test]
// spec: GEN-192, TEST-157
fn the_partitioned_route_s_filled_surface_and_weighted_routing_are_the_resident_ones_on_b_s_grid() {
    use planet_generators::bake::metric::Metric;
    use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
    // B's grid form: a window of a cube face of 2,048 cells an edge, with its origin and the face's metric (region_bake::solve_region), much wider than the tiles
    let (w, h, origin) = (72usize, 60usize, (996u32, 1003u32));
    let (_, z, outlet) = plane();
    let z: Vec<f64> = (0..w * h).map(|p| z[(p / w) % 48 * 64 + (p % w) % 64] + 0.25 * (p % 11) as f64).collect();
    let outlet: Vec<bool> = (0..w * h)
        .map(|p| p % w == 0 || p % w == w - 1 || p / w == 0 || p / w == h - 1 || outlet[(p / w) % 48 * 64 + (p % w) % 64] && p % 5 == 0)
        .collect();
    let g = Grid::new(w, h, 2_445.0).with_origin(origin).with_metric(Metric::cube_face(
        2048,
        R,
        (i64::from(origin.0), i64::from(origin.1)),
        w,
        h,
    ));
    let weight: Vec<f32> = (0..w * h).map(|p| 0.3 + (p % 9) as f32 * 0.2).collect();
    let eps = 2e-3;
    let want = route_rules(&g, &z, &outlet, eps, 5, 0.0, Some(&weight), RoundRules::ROUND);
    let flood = priority_flood_with(&g, &z, &outlet, eps, true);
    for tile in [8, 13, 25] {
        let got =
            partitioned_route_weighted(&g, &z, &outlet, eps, 5, tile, RoundRules::ROUND, Some(&weight), &mut PartCache::without_memory());
        assert!(got.stats.tiles >= 9, "tile {tile}: many tiles ({})", got.stats.tiles);
        assert_eq!(same(&got.filled, &flood), 0, "tile {tile}: the filled surface is the canonical flood's");
        assert_eq!(got.routing.receiver, want.receiver, "tile {tile}: the receivers");
        assert_eq!(got.routing.order, want.order, "tile {tile}: the order");
        assert_eq!(same(&got.routing.area, &want.area), 0, "tile {tile}: the weighted area");
    }
}
