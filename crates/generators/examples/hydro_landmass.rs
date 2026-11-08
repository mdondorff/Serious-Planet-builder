//! GEN-145: hydrology H1 on the prototype landmass (A3 section 5) and the peak memory of the resident round (G-SCALE input, the proposed 12 GB). The landmass is `landmass_region`'s (the land cells connected
//! to the region's centre through the cube graph, the coast ring as outlets; base heights from `target/landmass_base_3072.f32`, made by that example).
//! Usage: hydro_landmass [h1|mem] [threads]. `h1`: the runoff of the orographic climate, the routing weighted by it, H1 and its gates. `mem`: the resident route and the 4 rounds only, with the peak working set
//! of the process after each phase (read from Windows by PowerShell, so a phase's peak is the peak so far).
#![allow(clippy::needless_range_loop, clippy::chunks_exact_to_as_chunks)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, verify, BasinClass, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const R: f64 = 6_371_000.0;
const N: usize = 3072;
const SEED_CELL: (usize, usize, usize) = (1, 2090, 267);
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

/// The peak and current working set of this process in MiB, read by PowerShell.
fn memory_mb() -> (f64, f64) {
    let ps = |field: &str| -> f64 {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).{field}", std::process::id())])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(f64::NAN) / 1048576.0
    };
    (ps("PeakWorkingSet64"), ps("WorkingSet64"))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = a.get(1).map_or("h1", String::as_str).to_string();
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let total = 6 * N * N;
    let seed = 14u64;
    let t0 = Instant::now();
    let bytes = std::fs::read(format!("target/landmass_base_{N}.f32")).expect("run landmass_region first (it makes the base cache)");
    assert_eq!(bytes.len(), total * 4);
    let z0: Vec<f64> = bytes.chunks_exact(4).map(|c| f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect();
    drop(bytes);
    let full = Grid::cube(N, R);
    let id = |f: usize, i: usize, j: usize| f * N * N + j * N + i;
    let start = id(SEED_CELL.0, SEED_CELL.1, SEED_CELL.2);
    let mut land = vec![false; total];
    let mut stack = vec![start as u32];
    land[start] = true;
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = full.neighbour(p as usize, d) {
                if !land[q] && z0[q] > 0.0 {
                    land[q] = true;
                    stack.push(q as u32);
                }
            }
        }
    }
    let mut active = land.clone();
    for p in 0..total {
        if land[p] {
            for d in 0..8 {
                if let Some(q) = full.neighbour(p, d) {
                    active[q] = true;
                }
            }
        }
    }
    drop((full, stack));
    let grid = Grid::cube(N, R).with_active(active.clone());
    let outlet: Vec<bool> = (0..total).map(|p| active[p] && z0[p] <= 0.0).collect();
    let land_cells = land.iter().filter(|&&l| l).count();
    println!("landmass: {land_cells} land cells, set up in {:.1} s", t0.elapsed().as_secs_f64());
    let eps = 1e-3;
    let stream = seed;
    let rules = RoundRules::ROUND;

    if mode == "mem" {
        drop(land);
        let (peak, now) = memory_mb();
        println!("memory after the set-up (heights f64, active, outlet): peak {peak:.0} MiB, now {now:.0} MiB");
        let t = Instant::now();
        let res = route_rules(&grid, &z0, &outlet, eps, stream, 0.0, None, rules);
        println!("resident route: {:.1} s", t.elapsed().as_secs_f64());
        let (peak, now) = memory_mb();
        println!("memory after the resident route (kept): peak {peak:.0} MiB, now {now:.0} MiB");
        let (mut ad, mut cells) = (0.0f64, 0.0f64);
        for p in 0..total {
            if active[p] {
                cells += grid.cell_area(p);
                if res.receiver[p] == NONE {
                    ad += res.area[p];
                }
            }
        }
        println!("conservation across the cube edges: A_d at the outlets {:.6e} m2, true areas of the active cells {:.6e} m2, relative error {:.2e}", ad, cells, (ad - cells).abs() / cells);
        drop(res);
        let p = CarveParams { steps: 20, reroute_every: 5, round_rules: rules, ..CarveParams::default() };
        let mut z = z0.clone();
        let (peak_before, _) = memory_mb();
        let t = Instant::now();
        solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, stream);
        println!("4 rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        let (peak, now) = memory_mb();
        println!("memory after the 4 rounds (heights copy z held): peak {peak:.0} MiB (before the rounds {peak_before:.0}), now {now:.0} MiB; cells {total}, bytes a cell at peak {:.0}", peak * 1048576.0 / total as f64);
        return;
    }

    // ---- the runoff of the orographic climate at every land cell
    let t = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    let o = inputs.orography();
    let mut weight = vec![1.0f32; total];
    let mut runoff = vec![0.0f32; total];
    let mut evap = vec![0.0f32; total];
    let rows = total.div_ceil(threads);
    std::thread::scope(|s| {
        for (((w, r), e), k) in weight.chunks_mut(rows).zip(runoff.chunks_mut(rows)).zip(evap.chunks_mut(rows)).zip(0..) {
            let (active, grid_ref) = (&active, &grid);
            let _ = grid_ref;
            s.spawn(move || {
                for c in 0..w.len() {
                    let p = k * rows + c;
                    if !active[p] {
                        continue;
                    }
                    let (f, i, j) = (p / (N * N), p % N, (p % (N * N)) / N);
                    let spec = FaceSpec { face: Face(f as u8), cells: N, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                    let d: Vec3 = spec.halo_direction(i as i64, j as i64);
                    let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                    let ro = runoff_mm(pm, tc);
                    w[c] = discharge_weight(ro) as f32;
                    r[c] = (ro / 1000.0 / SECONDS_PER_YEAR) as f32;
                    e[c] = ((pet_mm(tc) - pm).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
                }
            });
        }
    });
    println!("runoff, weights and net evaporation: {:.1} s", t.elapsed().as_secs_f64());

    // ---- the routing weighted by the runoff, then H1
    let t = Instant::now();
    let routing = route_rules(&grid, &z0, &outlet, eps, stream, 0.0, Some(&weight), rules);
    let filled = priority_flood_with(&grid, &z0, &outlet, eps, true);
    println!("weighted route and filled surface: {:.1} s", t.elapsed().as_secs_f64());
    let params = HydroParams::default();
    let t = Instant::now();
    let h = hydrology(&grid, &z0, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &params);
    println!("H1: {:.1} s; {:?}", t.elapsed().as_secs_f64(), h.stats);
    let v = verify(&grid, &h, &routing, &filled, &outlet, Some(&runoff), Some(&weight));
    println!("G-CON and conservation: {v:?}");
    let crossing = h.segments.iter().filter(|s| s.up as usize / (N * N) != s.down as usize / (N * N)).count();
    let longest = h.segments.iter().map(|s| s.cells).max().unwrap_or(0);
    let (mut best, mut best_q) = (0usize, 0.0f64);
    for p in 0..total {
        if h.channel[p] && routing.receiver[p] == NONE && h.q[p] > best_q {
            (best, best_q) = (p, h.q[p]);
        }
    }
    println!(
        "segments {}: {crossing} span a cube edge, longest {longest} cells; the largest mouth (face {} cell ({}, {})) carries {:.0} m3/s",
        h.segments.len(),
        best / (N * N),
        best % N,
        (best % (N * N)) / N,
        best_q
    );
    let ramps = h.basins.iter().filter(|b| b.spill_m - b.floor_m < 1.0).count();
    println!("kept basins shallower than 1 m (the epsilon ramp couples to the pit limit): {ramps}");
    let lakes = h.basins.iter().filter(|b| b.class == BasinClass::Lake).count();
    let largest = h.basins.iter().max_by(|a, b| a.area_m2[15].total_cmp(&b.area_m2[15]));
    println!(
        "basins: {} kept ({lakes} lakes, {} endorheic), {} artefacts, {} cells in basins",
        h.basins.len(),
        h.basins.len() - lakes,
        h.stats.artefacts,
        h.stats.basin_cells
    );
    if let Some(b) = largest {
        println!(
            "  largest: floor {:.0} m, spill {:.0} m, area at the spill {:.0} km2, volume {:.0} km3, inflow {:.0} m3/s, {:?}, level {:.0} m",
            b.floor_m,
            b.spill_m,
            b.area_m2[15] / 1e6,
            b.volume_m3[15] / 1e9,
            b.q_in,
            b.class,
            b.water_level_m
        );
    }
    drop(z0);
}
