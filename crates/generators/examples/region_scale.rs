//! The prototype region at the erosion grid (inbox #100 step 1): face 1, the region of results.md (i 2014-2168, j 190-344 of the 3,072-cell grid) at 12,288 cells a face edge (814 m cells, 0.66 km^2),
//! with a margin of `margin` cells on every side (its border an outlet: an artificial cut). `gates`: the round solver against the whole-region carve (G-ROUND), 1 and N threads (G-DET), the
//! partitioned route against the resident route (G-PART), H1 on the carved surface with the runoff of the orographic climate (G-CON, conservation, the channel density) and the process memory.
//! `mem`: only the set-up, the 4 rounds and the H1 pass, with the peak working set after each (PowerShell, Windows). Usage: region_scale [gates|mem] [margin_cells] [threads] [tile]
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, verify, BasinClass, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;
/// The region of results.md at 3,072 cells, times 4.
const REGION: (usize, usize, usize, usize) = (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3);
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

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
    let mode = a.get(1).map_or("gates", String::as_str).to_string();
    let margin: usize = a.get(2).map_or(400, |s| s.parse().unwrap());
    let threads: usize = a.get(3).map_or(8, |s| s.parse().unwrap());
    let tile: usize = a.get(4).map_or(256, |s| s.parse().unwrap());
    let (i0, i1) = (REGION.0.saturating_sub(margin), (REGION.1 + margin).min(CELLS - 1));
    let (j0, j1) = (REGION.2.saturating_sub(margin), (REGION.3 + margin).min(CELLS - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let n = w * h;
    let seed = 14u64;
    let spec = FaceSpec { face: Face(1), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let t = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    let mut z0 = vec![0.0f64; n];
    let rows = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in z0.chunks_mut(rows * w).enumerate() {
            let (inputs, spec) = (&inputs, &spec);
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * w + c;
                    let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    let land = z0.iter().filter(|v| **v > 0.0).count();
    println!(
        "window i {i0}-{i1} j {j0}-{j1}: {w} x {h} = {n} cells ({:.1} x {:.1} km), land {:.1} %, base sampled in {:.1} s",
        w as f64 * spec.dx() / 1000.0,
        h as f64 * spec.dx() / 1000.0,
        100.0 * land as f64 / n as f64,
        t.elapsed().as_secs_f64()
    );
    let grid = Grid::new(w, h, spec.dx()).with_origin((i0 as u32, j0 as u32)).with_metric(Metric::cube_face(
        CELLS,
        R,
        (i0 as i64, j0 as i64),
        w,
        h,
    ));
    let outlet = grid.outlets(&z0, 0.0);
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
    let stream = spec.stream();
    let eps = p.epsilon;
    let rules = RoundRules::ROUND;
    let (peak, now) = memory_mb();
    println!("memory after the set-up: peak {peak:.0} MiB, now {now:.0} MiB");

    if mode == "gates" {
        let mut one = z0.clone();
        let t = Instant::now();
        solve_rounds(&grid, &mut one, &outlet, &p, 4, &1usize, stream);
        println!("round solver, 1 thread: {:.1} s", t.elapsed().as_secs_f64());
        let mut many = z0.clone();
        let t = Instant::now();
        solve_rounds(&grid, &mut many, &outlet, &p, 4, &threads, stream);
        println!("round solver, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        println!(
            "G-DET: {} cells differ between 1 and {threads} threads",
            one.iter().zip(&many).filter(|(a, b)| a.to_bits() != b.to_bits()).count()
        );
        drop(many);
        let mut want = z0.clone();
        let t = Instant::now();
        carve(&grid, &mut want, &outlet, &p, None, stream);
        println!(
            "whole-region carve (same rules): {:.1} s; G-ROUND: {} cells differ",
            t.elapsed().as_secs_f64(),
            one.iter().zip(&want).filter(|(a, b)| a.to_bits() != b.to_bits()).count()
        );
        drop(want);

        let t = Instant::now();
        let res = route_rules(&grid, &z0, &outlet, eps, stream, 0.0, None, rules);
        let res_s = t.elapsed().as_secs_f64();
        let mut cache = PartCache::default();
        let t = Instant::now();
        let part = partitioned_route(&grid, &z0, &outlet, eps, stream, tile, rules, &mut cache);
        println!(
            "G-PART, tile {tile}: {} tiles, {} sweeps, {} tile floods, {:.1} s (resident {res_s:.1} s); {} receivers, {} A_d differ",
            part.stats.tiles,
            part.stats.sweeps,
            part.stats.floods,
            t.elapsed().as_secs_f64(),
            part.routing.receiver.iter().zip(&res.receiver).filter(|(a, b)| a != b).count(),
            part.routing.area.iter().zip(&res.area).filter(|(a, b)| a.to_bits() != b.to_bits()).count()
        );
        drop((part, cache, res));
        h1(&grid, &one, &outlet, &spec, &inputs, stream, threads, &z0);
        let mut dz: Vec<f64> = Vec::new();
        for j in REGION.2..=REGION.3 {
            for i in REGION.0..=REGION.1 {
                let k = (j - j0) * w + (i - i0);
                if z0[k] > 0.0 {
                    dz.push(one[k] - z0[k]);
                }
            }
        }
        dz.sort_by(|a, b| a.total_cmp(b));
        let q = |f: f64| dz[((dz.len() - 1) as f64 * f) as usize];
        println!(
            "region land change (m): n {} p5 {:.1} p50 {:.1} p95 {:.1} max {:.1}",
            dz.len(),
            q(0.05),
            q(0.5),
            q(0.95),
            dz[dz.len() - 1]
        );
    } else {
        let t = Instant::now();
        let mut z = z0.clone();
        solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, stream);
        println!("4 rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        let (peak_rounds, now) = memory_mb();
        println!(
            "memory after the 4 rounds (z0 and z held): peak {peak_rounds:.0} MiB, now {now:.0} MiB; {:.0} bytes a cell at the peak",
            peak_rounds * 1048576.0 / n as f64
        );
        h1(&grid, &z, &outlet, &spec, &inputs, stream, threads, &z0);
        let (peak, now) = memory_mb();
        println!("memory after H1: peak {peak:.0} MiB, now {now:.0} MiB; {:.0} bytes a cell at the peak", peak * 1048576.0 / n as f64);
    }
}

/// The runoff of the orographic climate, the weighted route and H1 on the surface `z`, with the gates.
#[allow(clippy::too_many_arguments)]
fn h1(grid: &Grid, z: &[f64], outlet: &[bool], spec: &FaceSpec, inputs: &BakeInputs, stream: u64, threads: usize, z0: &[f64]) {
    let (w, n) = (grid.w(), grid.len());
    let (i0, j0) = ((grid.global_id(0) & 0xffff_ffff) as usize, (grid.global_id(0) >> 32) as usize);
    let o = inputs.orography();
    let mut weight = vec![1.0f32; n];
    let mut runoff = vec![0.0f32; n];
    let mut evap = vec![0.0f32; n];
    let rows = n.div_ceil(threads);
    let t = Instant::now();
    std::thread::scope(|s| {
        for (((wc, rc), ec), k) in weight.chunks_mut(rows).zip(runoff.chunks_mut(rows)).zip(evap.chunks_mut(rows)).zip(0..) {
            s.spawn(move || {
                for c in 0..wc.len() {
                    let p = k * rows + c;
                    if z0[p] <= 0.0 {
                        continue;
                    }
                    let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                    let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                    let ro = runoff_mm(pm, tc);
                    wc[c] = discharge_weight(ro) as f32;
                    rc[c] = (ro / 1000.0 / SECONDS_PER_YEAR) as f32;
                    ec[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
                }
            });
        }
    });
    println!("runoff, weights and net evaporation: {:.1} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let routing = route_rules(grid, z, outlet, 1e-3, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = priority_flood_with(grid, z, outlet, 1e-3, true);
    println!("weighted route and filled surface: {:.1} s", t.elapsed().as_secs_f64());
    let params = HydroParams::default();
    let t = Instant::now();
    let hy = hydrology(grid, z, &filled, &routing, outlet, Some(&runoff), Some(&evap), &params);
    println!("H1: {:.1} s; {:?}", t.elapsed().as_secs_f64(), hy.stats);
    let v = verify(grid, &hy, &routing, &filled, outlet, Some(&runoff), Some(&weight));
    println!("G-CON and conservation: {v:?}");
    let land = (0..n).filter(|&p| z[p] > 0.0).count();
    let land_km2: f64 = (0..n).filter(|&p| z[p] > 0.0).map(|p| grid.cell_area(p)).sum::<f64>() / 1e6;
    println!(
        "channel density: {:.1} % of land cells are channel cells; {:.2} heads and {:.2} segments per 1,000 km2 of land ({land} land cells, {land_km2:.0} km2); T = {:.0} km2 = {:.0} cells",
        100.0 * hy.stats.channel_cells as f64 / land as f64,
        1000.0 * hy.stats.heads as f64 / land_km2,
        1000.0 * hy.stats.segments as f64 / land_km2,
        params.initiation_m2 / 1e6,
        params.initiation_m2 / grid.cell_area(0)
    );
    let (mut best, mut best_q) = (0usize, 0.0f64);
    for p in 0..n {
        if hy.channel[p] && routing.receiver[p] == NONE && hy.q[p] > best_q {
            (best, best_q) = (p, hy.q[p]);
        }
    }
    let lakes = hy.basins.iter().filter(|b| b.class == BasinClass::Lake).count();
    let shallow = hy.basins.iter().filter(|b| b.spill_m - b.floor_m < 1.0).count();
    println!(
        "largest mouth ({}, {}) {:.0} m3/s; basins {} kept ({lakes} lakes, {} endorheic), {} artefacts, {} ramps, {shallow} shallower than 1 m",
        i0 + best % w,
        j0 + best / w,
        best_q,
        hy.basins.len(),
        hy.basins.len() - lakes,
        hy.stats.artefacts,
        hy.stats.ramps
    );
}
