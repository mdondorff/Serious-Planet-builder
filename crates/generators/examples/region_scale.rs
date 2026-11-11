//! The prototype region at the erosion grid (inbox #100 step 1): face 1, the region of results.md (i 2014-2168, j 190-344 of the 3,072-cell grid) at 12,288 cells a face edge (814 m cells, 0.66 km^2),
//! with a margin of `margin` cells on every side (its border an outlet: an artificial cut). `gates`: the round solver against the whole-region carve (G-ROUND), 1 and N threads (G-DET), the
//! partitioned route against the resident route (G-PART), H1 on the carved surface with the runoff of the orographic climate (G-CON, conservation, the channel density) and the process memory.
//! The stage probe is ON by default (sheets in `target/review/stages/region-s<seed>-f<face>-i<i0>-j<j0>/`; `PLANET_STAGES=0` switches it off); `gates` ends with the plains block. `mem`: only the set-up, the 4 rounds and the H1 pass, with the peak working set after each (PowerShell, Windows). Usage: [PLANET_REGION=seed,face,i0,j0,size] region_scale [gates|mem] [margin_cells] [threads] [tile]; without PLANET_REGION the region above (seed 14).
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, verify, BasinClass, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::probe::{self, SheetProbe};
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::sediment::sediment_pass;
use planet_generators::bake::tile::Pass2Params;
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::sync::Arc;
use std::time::Instant;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;
/// The default region: the region of results.md at 3,072 cells, times 4 (seed 14, face 1, start (8056, 760), 620 cells). `PLANET_REGION=seed,face,i0,j0,size` (cells at 12,288 a face edge) names another.
const DEFAULT_REGION: &str = "14,1,8056,760,620";

/// The region: seed, face, and the inclusive cell ranges `i0..=i1`, `j0..=j1`.
fn region() -> (u64, u8, (usize, usize, usize, usize)) {
    let text = std::env::var("PLANET_REGION").unwrap_or_else(|_| DEFAULT_REGION.to_string());
    let v: Vec<usize> = text
        .split(',')
        .map(|s| s.trim().parse().unwrap_or_else(|_| panic!("PLANET_REGION={text}: seed,face,i0,j0,size, all integers")))
        .collect();
    assert!(
        v.len() == 5 && v[1] < 6 && v[4] > 0 && v[2] + v[4] <= CELLS && v[3] + v[4] <= CELLS,
        "PLANET_REGION={text}: seed,face (0-5),i0,j0,size inside the face of {CELLS} cells"
    );
    (v[0] as u64, v[1] as u8, (v[2], v[2] + v[4] - 1, v[3], v[3] + v[4] - 1))
}
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
    let (seed, face, region) = region();
    let run = Instant::now();
    let stages = std::env::var("PLANET_STAGES").map_or(true, |v| v != "0");
    if stages {
        // One bin a cell (1 cell a pixel) at the window: the sheets are not committed, so the probe's bound is the window's cell count (item 13); the library's own bound is the caller's argument and is not changed.
        let bins = (region.1 - region.0 + 1 + 2 * margin) * (region.3 - region.2 + 1 + 2 * margin);
        let dir = format!("target/review/stages/region-s{seed}-f{face}-i{}-j{}", region.0, region.2);
        probe::install(Arc::new(SheetProbe::new(&dir, bins)));
        println!("stage probe ON: sheets and numbers in {dir}/ (its cost is inside the times below; PLANET_STAGES=0 switches it off)");
    }
    let (i0, i1) = (region.0.saturating_sub(margin), (region.1 + margin).min(CELLS - 1));
    let (j0, j1) = (region.2.saturating_sub(margin), (region.3 + margin).min(CELLS - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let n = w * h;
    let spec = FaceSpec { face: Face(face), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 };
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
    let p = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let stream = spec.stream();
    let eps = p.epsilon;
    let rules = RoundRules::ROUND;
    let (peak, now) = memory_mb();
    println!("memory after the set-up: peak {peak:.0} MiB, now {now:.0} MiB");
    {
        // The relief and the precipitation of the land of the region proper, from the current base (item 13).
        let (mut zs, mut ps) = (Vec::new(), Vec::new());
        for j in region.2..=region.3 {
            for i in region.0..=region.1 {
                let k = (j - j0) * w + (i - i0);
                if z0[k] > 0.0 {
                    zs.push(z0[k]);
                    ps.push(inputs.orography().precipitation_mm(spec.halo_direction(i as i64, j as i64)));
                }
            }
        }
        zs.sort_by(|a, b| a.total_cmp(b));
        ps.sort_by(|a, b| a.total_cmp(b));
        let q = |v: &[f64], f: f64| v[((v.len() - 1) as f64 * f) as usize];
        println!(
            "region land (base): {} cells, z p5 {:.0} p50 {:.0} p95 {:.0} max {:.0} m; precipitation p5 {:.0} p50 {:.0} p95 {:.0} mm",
            zs.len(),
            q(&zs, 0.05),
            q(&zs, 0.5),
            q(&zs, 0.95),
            zs[zs.len() - 1],
            q(&ps, 0.05),
            q(&ps, 0.5),
            q(&ps, 0.95)
        );
    }

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
        h1(&grid, &one, &outlet, &spec, &inputs, stream, threads, &z0, region);
        let mut dz: Vec<f64> = Vec::new();
        for j in region.2..=region.3 {
            for i in region.0..=region.1 {
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
        h1(&grid, &z, &outlet, &spec, &inputs, stream, threads, &z0, region);
        let (peak, now) = memory_mb();
        println!("memory after H1: peak {peak:.0} MiB, now {now:.0} MiB; {:.0} bytes a cell at the peak", peak * 1048576.0 / n as f64);
    }
    println!("total {:.1} s{}", run.elapsed().as_secs_f64(), if stages { ", stage probe ON" } else { "" });
}

/// The runoff of the orographic climate, the weighted route and H1 on the surface `z`, with the gates.
#[allow(clippy::too_many_arguments)]
fn h1(
    grid: &Grid,
    z: &[f64],
    outlet: &[bool],
    spec: &FaceSpec,
    inputs: &BakeInputs,
    stream: u64,
    threads: usize,
    z0: &[f64],
    region: (usize, usize, usize, usize),
) {
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
    let route_s = t.elapsed().as_secs_f64();
    let tf = Instant::now();
    let filled = priority_flood_with(grid, z, outlet, 1e-3, true);
    let flood_s = tf.elapsed().as_secs_f64();
    println!(
        "weighted route {route_s:.1} s, canonical flood {flood_s:.1} s ({:.0} % of the route-and-flood step)",
        100.0 * flood_s / (route_s + flood_s)
    );
    plains(grid, z, &filled, &routing.receiver, outlet, region);
    let params = HydroParams::default();
    let t = Instant::now();
    let hy = hydrology(grid, z, &filled, &routing, outlet, Some(&runoff), Some(&evap), &params);
    println!("H1: {:.1} s; {:?}", t.elapsed().as_secs_f64(), hy.stats);
    probe::emit(probe::Stage::H1, |s| {
        probe::StageView::new(s, grid)
            .f64("area", probe::FieldKind::Area, &routing.area)
            .f64("z", probe::FieldKind::Height, z)
            .f64("filled", probe::FieldKind::Height, &filled)
            .number("segments", hy.stats.segments as f64)
            .number("basins", hy.stats.basins as f64)
            .number("channel cells", hy.stats.channel_cells as f64)
    });
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
    // The river width law (PR 6, D19/D20): the width `w = a Q_w^b` of every channel cell in metres and in local cells (the local grid is 8 times finer than this grid).
    {
        let law = planet_generators::bake::k7::WidthLaw::STANDARD;
        let local_m = spec.dx() / 8.0;
        let mut w_m: Vec<f64> = (0..n).filter(|&p| hy.channel[p] && hy.qw[p] > 0.0).map(|p| law.width_m(hy.qw[p])).collect();
        w_m.sort_by(|a, b| a.total_cmp(b));
        if !w_m.is_empty() {
            let at = |q: f64| w_m[((w_m.len() - 1) as f64 * q) as usize];
            let wider = w_m.iter().filter(|&&w| w > local_m).count();
            println!(
                "river width (a = {}, b = {}, local cell {local_m:.0} m): {} channel cells, p50 {:.0} m ({:.2} cells), p95 {:.0} m ({:.2} cells), max {:.0} m ({:.1} cells); wider than one local cell: {:.1} % of the channel cells",
                law.a,
                law.b,
                w_m.len(),
                at(0.5),
                at(0.5) / local_m,
                at(0.95),
                at(0.95) / local_m,
                w_m[w_m.len() - 1],
                w_m[w_m.len() - 1] / local_m,
                100.0 * wider as f64 / w_m.len() as f64
            );
        }
    }
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
    land_and_sediment(grid, z, &filled, outlet, &routing.receiver, &routing.area, z0, &hy, region, spec.seed, stream, threads);
}

/// The plains block (item 13): over the land cells of the region proper, on the carved surface `z` and its canonical filled surface.
/// A *false flat* is a land cell with a receiver whose filled surface drops to the receiver by exactly 0 or by less than one f32 step at that height (a drop the f32 tile surface cannot hold);
/// a *sink* is a land cell the fill raised (filled above z) and a *pit* a land cell without a receiver that is not an outlet. Counts and shares of the land cells of the region.
fn plains(grid: &Grid, z: &[f64], filled: &[f64], receiver: &[u32], outlet: &[bool], region: (usize, usize, usize, usize)) {
    let w = grid.w();
    let (i0, j0) = ((grid.global_id(0) & 0xffff_ffff) as usize, (grid.global_id(0) >> 32) as usize);
    let (mut land, mut zero, mut sub_ulp, mut raised, mut pits) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut raise_m = 0.0f64;
    for j in region.2..=region.3 {
        for i in region.0..=region.1 {
            let p = (j - j0) * w + (i - i0);
            if z[p] <= 0.0 {
                continue;
            }
            land += 1;
            if filled[p] > z[p] {
                raised += 1;
                raise_m += filled[p] - z[p];
            }
            if receiver[p] == NONE {
                if !outlet[p] {
                    pits += 1;
                }
                continue;
            }
            let drop = filled[p] - filled[receiver[p] as usize];
            let h = filled[p] as f32;
            let ulp = f64::from(f32::from_bits(h.abs().to_bits() + 1) - h.abs());
            if drop == 0.0 {
                zero += 1;
            } else if drop < ulp {
                sub_ulp += 1;
            }
        }
    }
    let pc = |c: usize| 100.0 * c as f64 / land.max(1) as f64;
    println!(
        "plains: {land} land cells in the region; false flats {} ({:.2} %: {zero} with a drop of exactly 0, {sub_ulp} below one f32 step); sinks {raised} ({:.2} %, mean fill {:.2} m); pits {pits} ({:.2} %)",
        zero + sub_ulp,
        pc(zero + sub_ulp),
        pc(raised),
        raise_m / raised.max(1) as f64,
        pc(pits)
    );
}

/// G-LAND's unclassified sinks and G-SED's balance on the carved surface of the region (item 13, step 4). The unclassified sinks are the cells of the pits H1 keeps as no basin (artefacts and ramps: its pit cells less the cells of the kept basins), as a share of the land of the region proper;
/// G-SED is the sediment pass on a copy of `z` with the original ground `z0` (pass 2's parameters, pass 1's breach limit): eroded = deposited + exported as the report states it, thermal and the pit spreading keeping the volume.
#[allow(clippy::too_many_arguments)]
fn land_and_sediment(
    grid: &Grid,
    z: &[f64],
    filled: &[f64],
    outlet: &[bool],
    receiver: &[u32],
    area: &[f64],
    z0: &[f64],
    hy: &planet_generators::bake::hydro::Hydro,
    region: (usize, usize, usize, usize),
    seed: u64,
    stream: u64,
    threads: usize,
) {
    let w = grid.w();
    let (i0, j0) = ((grid.global_id(0) & 0xffff_ffff) as usize, (grid.global_id(0) >> 32) as usize);
    let (mut land, mut unclassified, mut kept) = (0usize, 0usize, 0usize);
    for j in region.2..=region.3 {
        for i in region.0..=region.1 {
            let p = (j - j0) * w + (i - i0);
            if z[p] > 0.0 {
                land += 1;
            }
        }
    }
    let in_basins = hy.label.iter().filter(|&&l| l != NONE).count();
    let unclassified_window = hy.stats.basin_cells - in_basins;
    for p in 0..grid.len() {
        let (i, j) = (i0 + p % w, j0 + p / w);
        if i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3 && z[p] > 0.0 {
            if hy.label[p] != NONE {
                kept += 1;
            } else if !outlet[p] && filled[p] - z[p] > HydroParams::default().pit_min_m {
                unclassified += 1;
            }
        }
    }
    println!(
        "G-LAND: pit cells of the window {} = {} in kept basins + {unclassified_window} unclassified (artefacts and ramps: {} and {}); of the region's {land} land cells {unclassified} unclassified ({:.3} %; rule: at most 1 %), {kept} in kept basins",
        hy.stats.basin_cells,
        in_basins,
        hy.stats.artefacts,
        hy.stats.ramps,
        100.0 * unclassified as f64 / land.max(1) as f64
    );
    let mut q2 = Pass2Params::standard(seed, 0.0);
    q2.carve.breach_max_m = CarveParams::default().breach_max_m;
    let mut zs = z.to_vec();
    let t = Instant::now();
    let r = sediment_pass(grid, &mut zs, z0, &q2, threads);
    let rel = |a: f64, b: f64| (a - b).abs() / a.abs().max(1.0);
    let path = stem(grid, z, &zs, filled, outlet, receiver, area, hy, region);
    depth_along(grid, z0, z, &zs, outlet, &path, region, stream, threads);
    println!(
        "G-SED: sediment pass {:.1} s; eroded {:.4e} m3 = deposited {:.4e} + exported {:.4e} (residual {:.2e} relative); the heights moved by {:.4e} m3 (against deposited {:.2e}); eroded before thermal {:.4e}, after {:.4e}; volume change thermal {:.2e}, pit spreading {:.2e}; pits {:?}",
        t.elapsed().as_secs_f64(),
        r.deposit.eroded,
        r.deposit.deposited,
        r.deposit.exported,
        rel(r.deposit.eroded, r.deposit.deposited + r.deposit.exported),
        r.deposit_height_volume_m3,
        rel(r.deposit.deposited, r.deposit_height_volume_m3),
        r.eroded_before_thermal_m3,
        r.eroded_after_thermal_m3,
        r.thermal_volume_change,
        r.spread_volume_change,
        r.pits.iter().map(|(n, p)| format!("{n} {} ({:.3} %)", p.pit_cells, 100.0 * p.share())).collect::<Vec<_>>()
    );
}

/// The deposited thickness along the main stem (item 13, finding 3): from the mouth with the largest discharge among channel cells without a receiver, upstream through the donor of the largest drainage area
/// to a head; the thickness is the sediment pass's change of height (`after - z`) at each cell; p50 and max per tenth of the stem's cells, from the source (decile 1) to the mouth (decile 10).
#[allow(clippy::too_many_arguments)]
fn stem(
    grid: &Grid,
    z: &[f64],
    after: &[f64],
    filled: &[f64],
    outlet: &[bool],
    receiver: &[u32],
    area: &[f64],
    hy: &planet_generators::bake::hydro::Hydro,
    region: (usize, usize, usize, usize),
) -> Vec<usize> {
    let n = grid.len();
    let (w, h) = (grid.w(), grid.len() / grid.w());
    let mut donor_of = vec![usize::MAX; n];
    for p in 0..n {
        let r = receiver[p];
        if r != NONE && (donor_of[r as usize] == usize::MAX || area[p] > area[donor_of[r as usize]]) {
            donor_of[r as usize] = p;
        }
    }
    // The mouths by discharge; the first whose stem neither starts nor ends on the window's border (a cut, not a source or a mouth).
    let mut mouths: Vec<usize> = (0..n).filter(|&p| hy.channel[p] && receiver[p] == NONE).collect();
    mouths.sort_by(|&a, &b| hy.q[b].total_cmp(&hy.q[a]));
    let (mut path, mut mouth, mut rank) = (Vec::new(), usize::MAX, 0usize);
    for (k, &m) in mouths.iter().enumerate() {
        let mut p = vec![m];
        while donor_of[*p.last().unwrap()] != usize::MAX {
            p.push(donor_of[*p.last().unwrap()]);
        }
        let head = *p.last().unwrap();
        let (x, y) = (head % w, head / w);
        let (mx, my) = (m % w, m / w);
        // Neither end on the window's border: a river entering from the margin or leaving into the cut is no river of the region.
        let inside = |x: usize, y: usize| x >= 3 && y >= 3 && x + 3 < w && y + 3 < h;
        if inside(x, y) && inside(mx, my) && p.len() >= 30 {
            (path, mouth, rank) = (p, m, k + 1);
            break;
        }
    }
    if mouth == usize::MAX {
        println!("stem: no stem of 30 cells or more that starts inside the window");
        return Vec::new();
    }
    let best_q = hy.q[mouth];
    let (mx, my) = (mouth % w, mouth / w);
    let border = mx.min(w - 1 - mx).min(my).min(h - 1 - my);
    path.reverse();
    let len = path.len();
    // The split: a cell is in a lake when it is in a kept basin or the fill raised it; the others are open channel and hillslope.
    let in_lake = |p: usize| hy.label[p] != NONE || filled[p] > z[p];
    let mut out = String::new();
    for d in 0..10 {
        let cells = &path[d * len / 10..(d + 1) * len / 10];
        let mut part = String::new();
        for (name, want) in [("lake", true), ("open", false)] {
            let mut v: Vec<f64> = cells.iter().filter(|&&p| in_lake(p) == want).map(|&p| after[p] - z[p]).collect();
            v.sort_by(|a, b| a.total_cmp(b));
            if v.is_empty() {
                part += &format!(" {name} -");
            } else {
                part += &format!(" {name} {:.2}/{:.2} (n {})", v[v.len() / 2], v[v.len() - 1], v.len());
            }
        }
        out += &format!("\n  {}:{part}", d + 1);
    }
    let km = len as f64 * grid.dx() / 1000.0;
    println!("stem (mouth number {rank} by discharge): {len} cells (about {km:.0} km of cells, straight-line steps), mouth discharge {best_q:.0} m3/s, drainage area at the mouth {:.3e} m2, at the source {:.3e} m2; the mouth cell is at {:.0} m, {} cells from the window border, {}; deposited thickness p50/max (m) by tenth from the source, in lakes and outside them:{out}", area[mouth], area[path[0]], z[mouth], border, if outlet[mouth] { "an outlet cell" } else { "no outlet cell" });
    // The same split over the window's land and over the region proper (the margin's cut, an outlet at land height, is only in the window).
    let (i0, j0) = ((grid.global_id(0) & 0xffff_ffff) as usize, (grid.global_id(0) >> 32) as usize);
    for (name, only_region) in [("the window's land", false), ("the region proper", true)] {
        let (mut vin, mut vout, mut cin, mut cout) = (0.0f64, 0.0f64, 0usize, 0usize);
        for p in 0..grid.len() {
            let (i, j) = (i0 + p % w, j0 + p / w);
            if z[p] > 0.0 && (!only_region || (i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3)) {
                let dv = (after[p] - z[p]) * grid.cell_area(p);
                if in_lake(p) {
                    (vin, cin) = (vin + dv, cin + 1);
                } else {
                    (vout, cout) = (vout + dv, cout + 1);
                }
            }
        }
        println!("deposit by place over {name}: in lakes (kept basin or filled above z) {vin:.4e} m3 on {cin} cells, outside {vout:.4e} m3 on {cout} cells ({:.1} % of the change in lakes)", 100.0 * vin / (vin + vout));
    }
    path
}

/// Where the depressions along the main stem come from (item 13, finding 3): the depression depth (canonical filled surface minus the surface) at the stem's cells, p50 and max per tenth from the source,
/// on the base (round 0), after 1, 2, 3 and 4 rounds (the surface H1 and the sediment pass start from) and after the sediment pass, and the share of the region's land that is under the fill (deeper than 1.5 eps)
/// at each. It re-runs the rounds (1, 2, 3 of the 4) on the same inputs, so it adds about 6 rounds to the run.
#[allow(clippy::too_many_arguments)]
fn depth_along(
    grid: &Grid,
    z0: &[f64],
    z4: &[f64],
    after_pass: &[f64],
    outlet: &[bool],
    path: &[usize],
    region: (usize, usize, usize, usize),
    stream: u64,
    threads: usize,
) {
    if path.is_empty() {
        return;
    }
    let w = grid.w();
    let (i0, j0) = ((grid.global_id(0) & 0xffff_ffff) as usize, (grid.global_id(0) >> 32) as usize);
    let p = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let eps = 1e-3;
    let mut stages: Vec<(String, Vec<f64>)> = vec![("base".to_string(), z0.to_vec())];
    for k in 1..=3usize {
        let mut zk = z0.to_vec();
        // The carve is the rounds times the steps of a round (5 of the 20 steps of the 4 rounds).
        solve_rounds(grid, &mut zk, outlet, &CarveParams { steps: 5 * k, ..p }, k, &threads, stream);
        stages.push((format!("after round {k}"), zk));
    }
    stages.push(("after round 4 (before the pass)".to_string(), z4.to_vec()));
    stages.push(("after the pass".to_string(), after_pass.to_vec()));
    let len = path.len();
    for (name, zz) in &stages {
        let filled = priority_flood_with(grid, zz, outlet, eps, true);
        let mut out = String::new();
        for d in 0..10 {
            let mut v: Vec<f64> = path[d * len / 10..(d + 1) * len / 10].iter().map(|&c| filled[c] - zz[c]).collect();
            v.sort_by(|a, b| a.total_cmp(b));
            if !v.is_empty() {
                out += &format!(" {:.1}/{:.1}", v[v.len() / 2], v[v.len() - 1]);
            }
        }
        let (mut land, mut under) = (0usize, 0usize);
        for j in region.2..=region.3 {
            for i in region.0..=region.1 {
                let c = (j - j0) * w + (i - i0);
                if zz[c] > 0.0 {
                    land += 1;
                    under += usize::from(filled[c] - zz[c] > 1.5 * eps);
                }
            }
        }
        println!("depth {name}: region land under the fill {under} of {land} ({:.2} %); along the stem p50/max (m) by tenth from the source:{out}", 100.0 * under as f64 / land.max(1) as f64);
    }
}
