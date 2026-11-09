//! GEN-147: the rounds on the whole prototype landmass at the working resolution (A3 section 6.6, G-SCALE; inbox #101): the landmass is found at 3,072 cells from the base cache of `landmass_region`,
//! refined by `k` (k = 4 is the erosion grid, 12,288 cells, 814 m): the base is sampled only in the fine cells whose coarse parent is in the coarse landmass or within a ring of it, the fine land is the cells
//! above the sea connected to the region's centre, its coast ring the outlets, and the landmass is a compact grid (`Grid::compact_cube`: only its cells, indexed by rank). Then 4 rounds of 5 steps on
//! the resident route, with the process's peak working set after each phase (PowerShell, Windows).
//! Usage: landmass_scale [k] [threads] [h1|part|both]: `part` routes each round with the partitioned route on the compact grid (GEN-149, tiles of 256 cells; `both` also runs the resident rounds and compares the bits, small k only); with `h1` the hydrology pass follows (runoff of the orographic climate, weighted route, filled surface, H1) with the peak after it. Needs `target/landmass_base_3072.f32` (made by `landmass_region`).
#![allow(clippy::needless_range_loop, clippy::chunks_exact_to_as_chunks)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::round::{solve_rounds, solve_rounds_memo, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const R: f64 = 6_371_000.0;
const COARSE: usize = 3072;
const SEED_CELL: (usize, usize, usize) = (1, 2090, 267);
const REGION: (usize, usize, usize, usize) = (2014, 2168, 190, 344);

fn memory_mb() -> (f64, f64) {
    let (peak, now, _) = memory3();
    (peak, now)
}

/// The peak and current working set and the peak commit (paged memory) of this process in MiB.
fn memory3() -> (f64, f64, f64) {
    let ps = |field: &str| -> f64 {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).{field}", std::process::id())])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(f64::NAN) / 1048576.0
    };
    (ps("PeakWorkingSet64"), ps("WorkingSet64"), ps("PeakPagedMemorySize64"))
}

fn set(bits: &mut [u64], g: usize) {
    bits[g >> 6] |= 1 << (g & 63);
}

fn get(bits: &[u64], g: usize) -> bool {
    bits[g >> 6] >> (g & 63) & 1 == 1
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let k: usize = a.get(1).map_or(1, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let n = COARSE * k;
    let nn = n * n;
    let t0 = Instant::now();
    let seed = 14u64;

    // ---- the coarse landmass and its ring, from the 3,072-cell base cache
    let bytes = std::fs::read(format!("target/landmass_base_{COARSE}.f32")).expect("run landmass_region first (it makes the base cache)");
    let cz = |g: usize| f32::from_le_bytes([bytes[4 * g], bytes[4 * g + 1], bytes[4 * g + 2], bytes[4 * g + 3]]);
    let coarse = Grid::cube(COARSE, R);
    let cid = |f: usize, i: usize, j: usize| f * COARSE * COARSE + j * COARSE + i;
    let mut land_c = vec![0u64; (6 * COARSE * COARSE).div_ceil(64)];
    let mut stack = vec![cid(SEED_CELL.0, SEED_CELL.1, SEED_CELL.2)];
    set(&mut land_c, stack[0]);
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = coarse.neighbour(p, d) {
                if !get(&land_c, q) && cz(q) > 0.0 {
                    set(&mut land_c, q);
                    stack.push(q);
                }
            }
        }
    }
    // The candidates: the coarse landmass and two rings of cells around it (the fine coast may differ from the coarse one).
    let mut cand_c = land_c.clone();
    for _ in 0..2 {
        let prev = cand_c.clone();
        for g in 0..6 * COARSE * COARSE {
            if get(&prev, g) {
                for d in 0..8 {
                    if let Some(q) = coarse.neighbour(g, d) {
                        set(&mut cand_c, q);
                    }
                }
            }
        }
    }
    drop((coarse, stack, land_c, cz));
    drop(bytes);
    let coarse_cand: usize = cand_c.iter().map(|w| w.count_ones() as usize).sum();
    println!("coarse candidates {coarse_cand} ({:.1} s)", t0.elapsed().as_secs_f64());

    // ---- the fine candidates, in order of their cube id, and the fine base heights there
    let mut bits = vec![0u64; (6 * nn).div_ceil(64)];
    let mut rows: Vec<(usize, usize)> = Vec::new(); // (face, j) of every row with a candidate
    for f in 0..6 {
        for j in 0..n {
            let mut any = false;
            for i in 0..n {
                if get(&cand_c, cid(f, i / k, j / k)) {
                    set(&mut bits, f * nn + j * n + i);
                    any = true;
                }
            }
            if any {
                rows.push((f, j));
            }
        }
    }
    let count: usize = bits.iter().map(|w| w.count_ones() as usize).sum();
    println!("fine candidates {count} of {} ({:.1} s)", 6 * nn, t0.elapsed().as_secs_f64());
    let cand = Grid::compact_cube(n, R, bits);
    let inputs = BakeInputs::new(seed, threads);
    let mut zf = vec![0f32; count];
    {
        // Rows in ascending id order own consecutive candidate ranges: split the heights by row.
        let mut starts: Vec<usize> = Vec::with_capacity(rows.len() + 1);
        let mut at = 0usize;
        for &(f, j) in &rows {
            starts.push(at);
            at += (0..n).filter(|&i| cand.compact_index_of(f * nn + j * n + i).is_some()).count();
        }
        starts.push(at);
        assert_eq!(at, count);
        let per = rows.len().div_ceil(threads);
        let mut rest: &mut [f32] = &mut zf;
        let mut parts: Vec<(usize, &mut [f32])> = Vec::new();
        for (t, chunk) in rows.chunks(per).enumerate() {
            let len = starts[(t * per + chunk.len()).min(rows.len())] - starts[t * per];
            let (head, tail) = rest.split_at_mut(len);
            parts.push((t, head));
            rest = tail;
        }
        std::thread::scope(|s| {
            for (t, part) in parts {
                let (rows, inputs, cand) = (&rows, &inputs, &cand);
                s.spawn(move || {
                    let mut at = 0;
                    for &(f, j) in &rows[t * per..(t * per + per).min(rows.len())] {
                        let spec = FaceSpec { face: Face(f as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                        for i in 0..n {
                            if cand.compact_index_of(f * nn + j * n + i).is_some() {
                                let d: Vec3 = spec.halo_direction(i as i64, j as i64);
                                part[at] = inputs.base(d) as f32;
                                at += 1;
                            }
                        }
                    }
                });
            }
        });
    }
    println!("fine base sampled ({:.1} s)", t0.elapsed().as_secs_f64());
    let (peak, now) = memory_mb();
    println!("memory after the sampling: peak {peak:.0} MiB, now {now:.0} MiB");

    // ---- the fine landmass: the land connected to the region's centre, and its coast ring
    let seed_g = SEED_CELL.0 * nn + (SEED_CELL.2 * k + k / 2) * n + SEED_CELL.1 * k + k / 2;
    let s0 = cand.compact_index_of(seed_g).expect("the seed cell is a candidate");
    assert!(zf[s0] > 0.0, "the region's centre is land");
    let mut land = vec![0u64; count.div_ceil(64)];
    let mut stack = vec![s0];
    set(&mut land, s0);
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = cand.neighbour(p, d) {
                if !get(&land, q) && zf[q] > 0.0 {
                    set(&mut land, q);
                    stack.push(q);
                }
            }
        }
    }
    drop(stack);
    // The fine land must not reach the border of the candidates (the coarse landmass and two rings): a land cell with a cube neighbour outside them would be cut off from its outlets.
    {
        let full = Grid::cube(n, R);
        let mut border = 0usize;
        for p in 0..count {
            if get(&land, p) {
                let g = cand.cube_id_of(p);
                if (0..8).any(|d| full.neighbour(g, d).is_some_and(|q| cand.compact_index_of(q).is_none())) {
                    border += 1;
                }
            }
        }
        println!("fine land cells at the border of the candidates: {border}");
        assert_eq!(border, 0, "the fine landmass reaches the border of the candidates: more rings are needed");
    }
    let mut fin = vec![0u64; (6 * nn).div_ceil(64)];
    let mut land_cells = 0usize;
    for p in 0..count {
        if get(&land, p) {
            land_cells += 1;
            set(&mut fin, cand.cube_id_of(p));
            for d in 0..8 {
                if let Some(q) = cand.neighbour(p, d) {
                    set(&mut fin, cand.cube_id_of(q));
                }
            }
        }
    }
    let active: usize = fin.iter().map(|w| w.count_ones() as usize).sum();
    println!("landmass: {land_cells} land cells, {active} cells with the coast ring ({:.1} s)", t0.elapsed().as_secs_f64());
    // The heights and outlets of the landmass, by compact index.
    let grid = Grid::compact_cube(n, R, fin);
    let mut z: Vec<f64> = Vec::with_capacity(active);
    let mut outlet: Vec<bool> = Vec::with_capacity(active);
    for p in 0..count {
        let g = cand.cube_id_of(p);
        if grid.compact_index_of(g).is_some() {
            z.push(f64::from(zf[p]));
            outlet.push(zf[p] <= 0.0);
        }
    }
    assert_eq!(z.len(), active);
    // The region's cells, kept for the change.
    let region: Vec<(usize, f64)> = (REGION.2 * k..(REGION.3 + 1) * k)
        .flat_map(|j| (REGION.0 * k..(REGION.1 + 1) * k).map(move |i| (nn + j * n + i, 0.0)))
        .filter_map(|(g, _)| grid.compact_index_of(g).filter(|&p| z[p] > 0.0).map(|p| (p, z[p])))
        .collect();
    drop((cand, zf, land));
    let (peak, now) = memory_mb();
    println!(
        "memory after the landmass is built (heights f64, outlets): peak {peak:.0} MiB, now {now:.0} MiB; {:.1} s",
        t0.elapsed().as_secs_f64()
    );

    // ---- the rounds, resident
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let t = Instant::now();
    let mode = a.get(3).map_or("", |x| x.as_str());
    let rep = if mode == "part" || mode == "both" {
        // `part` keeps nothing between rounds (the memory of one solve); `both` keeps the memos like an edited build.
        let mut cache = if mode == "part" { RoundCache::without_memory() } else { RoundCache::default() };
        let mut zp = if mode == "both" { z.clone() } else { Vec::new() };
        let (rep, work) = if mode == "both" {
            solve_rounds_memo(&grid, &mut zp, &outlet, &p, 4, &threads, seed, &Drivers::default(), 256, &mut cache)
        } else {
            solve_rounds_memo(&grid, &mut z, &outlet, &p, 4, &threads, seed, &Drivers::default(), 256, &mut cache)
        };
        for (r, w) in work.iter().enumerate() {
            println!("  round {}: {} tiles, {} tile floods, {} memo hits", r + 1, w.tiles, w.tile_floods, w.tile_memo_hits);
        }
        println!("partitioned rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        let (peak, now) = memory_mb();
        println!(
            "memory after the partitioned rounds: peak {peak:.0} MiB, now {now:.0} MiB; bytes a cell at the peak {:.0}",
            peak * 1048576.0 / active as f64
        );
        if mode == "both" {
            let t = Instant::now();
            let _ = solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, seed);
            println!("resident rounds: {:.1} s", t.elapsed().as_secs_f64());
            let differ = z.iter().zip(&zp).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
            println!("cells whose height differs, partitioned against resident: {differ}");
            assert_eq!(differ, 0);
        }
        rep
    } else {
        let rep = solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, seed);
        println!("4 rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        rep
    };
    for (r, s) in rep.rounds.iter().enumerate() {
        println!(
            "  round {}: {} catchments, largest {} cells, {} cells in catchments of 1,000 or more",
            r + 1,
            s.catchments,
            s.largest,
            s.in_large
        );
    }
    let (peak, now) = memory_mb();
    println!(
        "memory after the 4 rounds: peak {peak:.0} MiB, now {now:.0} MiB; cells {active}, bytes a cell at the peak {:.0}",
        peak * 1048576.0 / active as f64
    );
    if a.get(3).is_some_and(|x| x == "h1") {
        let o = inputs.orography();
        let mut weight = vec![1.0f32; active];
        let mut runoff = vec![0.0f32; active];
        let mut evap = vec![0.0f32; active];
        let per = active.div_ceil(threads);
        let t = Instant::now();
        std::thread::scope(|s| {
            for (((wc, rc), ec), t) in weight.chunks_mut(per).zip(runoff.chunks_mut(per)).zip(evap.chunks_mut(per)).zip(0..) {
                let (grid, z, o) = (&grid, &z, o);
                s.spawn(move || {
                    for c in 0..wc.len() {
                        let p = t * per + c;
                        if z[p] <= 0.0 {
                            continue;
                        }
                        let g = grid.cube_id_of(p);
                        let spec = FaceSpec { face: Face((g / nn) as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                        let d: Vec3 = spec.halo_direction(((g % nn) % n) as i64, ((g % nn) / n) as i64);
                        let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                        let ro = runoff_mm(pm, tc);
                        wc[c] = discharge_weight(ro) as f32;
                        rc[c] = (ro / 1000.0 / 31_557_600.0) as f32;
                        ec[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / 31_557_600.0) as f32;
                    }
                });
            }
        });
        println!("runoff, weights and evaporation: {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let routing = route_rules(&grid, &z, &outlet, 1e-3, seed, 0.0, Some(&weight), RoundRules::ROUND);
        let filled = priority_flood_with(&grid, &z, &outlet, 1e-3, true);
        println!("weighted route and filled surface: {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let hy = hydrology(&grid, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
        println!("H1: {:.1} s; {:?}", t.elapsed().as_secs_f64(), hy.stats);
        let (peak, now) = memory_mb();
        println!("memory after H1: peak {peak:.0} MiB, now {now:.0} MiB; bytes a cell at the peak {:.0}", peak * 1048576.0 / active as f64);
    }
    let mut dz: Vec<f64> = region.iter().map(|&(p, z0)| z[p] - z0).collect();
    dz.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| dz[((dz.len() - 1) as f64 * f) as usize];
    println!("region land change (m): n {} p5 {:.1} p50 {:.1} p95 {:.1} max {:.1}", dz.len(), q(0.05), q(0.5), q(0.95), dz[dz.len() - 1]);
}
