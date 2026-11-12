//! GEN-148 (briefing item 11, A3 section 3.3, G-SURF): K7 in the new chain on a tile of the prototype region at the erosion and the local grid. The base is the store lattice's C1 read
//! (`read_elevation_c1`) of a lattice sampled from `BakeInputs::base` at the nodes (611 m, k = 14; stored as f32); the regional solve (12,288 cells, 814 m, 4 rounds) starts from the same read; the
//! channels of H1 on the solved surface are burned into a 1,024-cell tile of the local grid (102 m, 98,304 cells a face edge, 8 local cells to a regional cell); then the breach, the incision, thermal,
//! the deposition and the pit spreading. Reports the pit share and depth at 102 m after each stage, with and without the channels, and G-SURF at the lattice nodes.
//! Usage: k7_region [margin_regional_cells] [threads] [store|compare]: `compare` runs K7 with the canonical and with the plain flood (GEN-160) on the tile and reports how much the stored change L moves (cells changed, largest change, share over 0.25 m, pits); `store` runs K7 with the channels over every 1,024-cell tile that meets the prototype region and writes each tile's stored change L (f32 metres, core cells) and its land mask to `target/k7_l/` for `cache/examples/l_measure` (G-STORE, kind 13 on the new chain).
#![allow(clippy::needless_range_loop)]
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::k7::{k7_tile, surface_gate, Channel, K7Params, K7Result, RegionField};
use planet_generators::bake::metric::Metric;
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::tile::{TileGrid, TileId};
use planet_generators::bake::Grid;
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const K: u32 = 14;
const LATTICE: usize = 1 << K; // 16,384 nodes a face edge
const REGIONAL: usize = 12_288;
const LOCAL: usize = 6 << K; // 98,304
const R: f64 = 6_371_000.0;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
/// The default region: seed 14, face 1, the prototype region (inclusive cell ranges at 12,288 cells a face edge). `PLANET_REGION=seed,face,i0,j0,size` names another (item 14: L on a second region); `PLANET_L_DIR` names the directory `store` writes (default `target/k7_l`).
const DEFAULT_REGION: (u64, u8, (usize, usize, usize, usize)) = (14, 1, (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3));

fn region() -> (u64, u8, (usize, usize, usize, usize)) {
    let Ok(text) = std::env::var("PLANET_REGION") else { return DEFAULT_REGION };
    let v: Vec<usize> = text.split(',').map(|s| s.trim().parse().expect("PLANET_REGION=seed,face,i0,j0,size")).collect();
    assert_eq!(v.len(), 5, "PLANET_REGION=seed,face,i0,j0,size");
    (v[0] as u64, v[1] as u8, (v[2], v[2] + v[4] - 1, v[3], v[3] + v[4] - 1))
}

/// The lattice nodes of a rectangle of one face, sampled from the base field (f32, as stored).
struct Lattice {
    face: Face,
    i0: usize,
    j0: usize,
    w: usize,
    h: usize,
    z: Vec<f32>,
}

impl LatticeSource for Lattice {
    fn res(&self) -> u32 {
        LATTICE as u32
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let (i, j) = (i as usize, j as usize);
        if face != self.face || i < self.i0 || j < self.j0 || i >= self.i0 + self.w || j >= self.j0 + self.h {
            return Err(ReadError::Source(format!("node ({}, {i}, {j}) is outside the sampled rectangle", face.0)));
        }
        Ok(GlobalSample {
            continent: 0.0,
            elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)],
            temperature_c: 0.0,
            precipitation_mm: 0.0,
        })
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let margin: usize = a.get(1).map_or(200, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let (seed, face_n, region) = region();
    let face = Face(face_n);
    let l_dir = std::env::var("PLANET_L_DIR").unwrap_or_else(|_| "target/k7_l".to_string());
    let t0 = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    // ---- the regional window and its lattice rectangle (a regional cell is 8 local cells, a node every 6 local cells)
    let (ri0, ri1) = (region.0 - margin, region.1 + margin);
    let (rj0, rj1) = (region.2 - margin, region.3 + margin);
    let (rw, rh) = (ri1 - ri0 + 1, rj1 - rj0 + 1);
    let to_node = |reg: usize| reg * 8 / 6;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let spec_l = FaceSpec { face, cells: LATTICE, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let mut nodes = vec![0f32; nw * nh];
    let rows = nh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in nodes.chunks_mut(rows * nw).enumerate() {
            let (inputs, spec) = (&inputs, &spec_l);
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * nw + c;
                    let (vi, vj) = ((ni0 + p % nw) as f64, (nj0 + p / nw) as f64);
                    let nn = LATTICE as f64;
                    // The store lattice is vertex-based: vertex k lies at 2k/res - 1 on the face.
                    *v = inputs.base(face_to_direction(&TangentWarp, spec.face, 2.0 * vi / nn - 1.0, 2.0 * vj / nn - 1.0)) as f32;
                }
            });
        }
    });
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes };
    let base = |d: Vec3| -> f64 { f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle")) };
    println!("lattice {nw} x {nh} nodes sampled ({:.1} s)", t0.elapsed().as_secs_f64());

    // ---- the regional solve from B read at the regional cells' centres
    let spec_r = FaceSpec { face, cells: REGIONAL, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let n = rw * rh;
    let mut zb = vec![0.0f64; n];
    let rrows = rh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in zb.chunks_mut(rrows * rw).enumerate() {
            let (spec, base) = (&spec_r, &base);
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rrows * rw + c;
                    *v = base(spec.halo_direction((ri0 + p % rw) as i64, (rj0 + p / rw) as i64));
                }
            });
        }
    });
    let grid = Grid::new(rw, rh, spec_r.dx()).with_origin((ri0 as u32, rj0 as u32)).with_metric(Metric::cube_face(
        REGIONAL,
        R,
        (ri0 as i64, rj0 as i64),
        rw,
        rh,
    ));
    let outlet = grid.outlets(&zb, 0.0);
    let p = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let stream = spec_r.stream();
    let mut zs = zb.clone();
    solve_rounds(&grid, &mut zs, &outlet, &p, 4, &threads, stream);
    println!("regional window {rw} x {rh} cells solved from the C1 read of the lattice ({:.1} s)", t0.elapsed().as_secs_f64());
    let change: Vec<f32> = zs.iter().zip(&zb).map(|(s, b)| (s - b) as f32).collect();
    let regional_field = RegionField::new(face, REGIONAL, (ri0 as i64, rj0 as i64), rw, rh, change);
    let regional = |d: Vec3| regional_field.at(d);

    // ---- H1 on the solved surface: the channel segments
    let o = inputs.orography();
    // The runoff in m/s and the net open-water evaporation as the build gives them to H1 (region_scale, build.rs): without the runoff Q is an area in m2, not a discharge (the units bug of 2026-11-10).
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for c in (0..n).filter(|&c| zs[c] > 0.0) {
        let d = spec_r.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
        let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
        let ro = runoff_mm(pm, tc);
        weight[c] = discharge_weight(ro) as f32;
        runoff[c] = (ro / 1000.0 / SECONDS_PER_YEAR) as f32;
        evap[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
    }
    let routing = route_rules(&grid, &zs, &outlet, 1e-3, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = priority_flood_with(&grid, &zs, &outlet, 1e-3, true);
    let hy = hydrology(&grid, &zs, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
    println!("H1: {} channel cells, {} segments", hy.stats.channel_cells, hy.stats.segments);

    // ---- the tile
    let g = TileGrid { cells: LOCAL, tile: 1024, halo: 64, radius_m: R };
    let store = a.get(3).is_some_and(|s| s == "store");
    let compare = a.get(3).is_some_and(|s| s == "compare");
    let ids: Vec<TileId> = if store {
        let (i0, i1, j0, j1) = (region.0 * 8 / 1024, (region.1 * 8 + 7) / 1024, region.2 * 8 / 1024, (region.3 * 8 + 7) / 1024);
        (j0..=j1).flat_map(|tj| (i0..=i1).map(move |ti| TileId { face, ti: ti as u32, tj: tj as u32 })).collect()
    } else {
        vec![TileId { face, ti: (8390 * 8 + 4) as u32 / 1024, tj: (958 * 8 + 4) as u32 / 1024 }]
    };
    for id in ids {
        let (t, h) = (g.tile as i64, g.halo as i64);
        let (wi0, wj0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
        let ww = g.window() as i64;
        println!(
            "tile ({}, {}) of face {}: local cells i {}..{}, j {}..{} ({:.1} km a side)",
            id.ti,
            id.tj,
            face_n,
            wi0,
            wi0 + ww,
            wj0,
            wj0 + ww,
            ww as f64 * g.dx() / 1000.0
        );
        let mut channels: Vec<Channel> = Vec::new();
        for sg in &hy.segments {
            let mut cells = vec![sg.up as usize];
            while *cells.last().unwrap() != sg.down as usize {
                let r = routing.receiver[*cells.last().unwrap()];
                assert!(r != NONE);
                cells.push(r as usize);
            }
            let to_local = |c: usize| (8.0 * (ri0 + c % rw) as f64 + 3.5, 8.0 * (rj0 + c / rw) as f64 + 3.5);
            // Only the segments with a point within a regional cell of the window.
            let near = cells.iter().any(|&c| {
                let (x, y) = to_local(c);
                x >= (wi0 - 16) as f64 && x <= (wi0 + ww + 16) as f64 && y >= (wj0 - 16) as f64 && y <= (wj0 + ww + 16) as f64
            });
            if !near {
                continue;
            }
            let mut level = f64::INFINITY;
            let levels: Vec<f64> = cells
                .iter()
                .map(|&c| {
                    level = level.min(zs[c]);
                    level
                })
                .collect();
            channels.push(Channel {
                points: cells.iter().map(|&c| to_local(c)).collect(),
                levels,
                q: cells.iter().map(|&c| hy.qw[c]).collect(),
                q_unit: hy.q_unit,
            });
        }
        println!("{} channel segments meet the window", channels.len());
        // The trench widths of the width law in metres and in local cells (a local cell is about pi R / 2 / LOCAL metres): the check that Q is a discharge.
        let cell_m = std::f64::consts::FRAC_PI_2 * R / LOCAL as f64;
        let mut widths: Vec<f64> = channels
            .iter()
            .flat_map(|ch| ch.q.iter().filter(|&&q| q > 0.0).map(|&q| K7Params::standard(seed, 0.0).width.width_m(q)))
            .collect();
        widths.sort_by(f64::total_cmp);
        if let Some(&max) = widths.last() {
            let at = |f: f64| widths[((widths.len() - 1) as f64 * f) as usize];
            println!(
                "  trench widths (w = 7.2 Q^0.5, Q in {:?}): p50 {:.1} m ({:.2} cells), p99 {:.1} m ({:.2} cells), max {:.1} m ({:.2} cells); a local cell is {:.1} m",
                hy.q_unit,
                at(0.5),
                at(0.5) / cell_m,
                at(0.99),
                at(0.99) / cell_m,
                max,
                max / cell_m,
                cell_m
            );
        }

        let params = K7Params::standard(seed, 0.0);
        let runs: Vec<(&str, &[Channel])> = if store {
            vec![("with the channels burned in", &channels[..])]
        } else {
            vec![("with the channels burned in", &channels[..]), ("without the channels", &[][..])]
        };
        for (label, ch) in runs {
            let t = Instant::now();
            let r: K7Result = k7_tile(&g, id, &base, &regional, ch, &params, threads);
            println!("K7 {label}: {:.1} s, {} cells burned", t.elapsed().as_secs_f64(), r.burned);
            println!("MARK {}", r.mark_line());
            if compare {
                let plain = K7Params { canonical_flood: false, ..params };
                let r0: K7Result = k7_tile(&g, id, &base, &regional, ch, &plain, threads);
                let d: Vec<f64> = r.change.iter().zip(&r0.change).map(|(a, b)| f64::from(*a) - f64::from(*b)).collect();
                let changed = d.iter().filter(|v| **v != 0.0).count();
                let over = d.iter().filter(|v| v.abs() > 0.25).count();
                let max = d.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                let n = d.len();
                println!(
                "  stored change L, canonical against plain flood ({label}): {changed} of {n} core cells differ ({:.3} %), largest {max:.3} m, {over} over 0.25 m ({:.4} %)",
                100.0 * changed as f64 / n as f64,
                100.0 * over as f64 / n as f64
            );
                for (name, pc, p0) in r.pits.iter().zip(&r0.pits).map(|(c, p)| (c.0, c.1, p.1)) {
                    println!(
                        "    pits after {name}: canonical {} ({:.4} %, max {:.1} m), plain {} ({:.4} %, max {:.1} m)",
                        pc.pit_cells,
                        100.0 * pc.share(),
                        pc.max_m,
                        p0.pit_cells,
                        100.0 * p0.share(),
                        p0.max_m
                    );
                }
                continue;
            }
            for (stage, s) in &r.pits {
                println!(
                    "  pits at 102 m after {stage}: {} of {} land cells ({:.3} %), depth p50 {:.1} m, p99 {:.1} m, max {:.1} m",
                    s.pit_cells,
                    s.land_cells,
                    100.0 * s.share(),
                    s.p50_m,
                    s.p99_m,
                    s.max_m
                );
            }
            if store {
                std::fs::create_dir_all(&l_dir).expect("the output directory");
                let name = format!("{l_dir}/{}_{}", id.ti, id.tj);
                let (t, h, w) = (g.tile, g.halo, g.window());
                let land: Vec<u8> = (0..t * t).map(|c| u8::from(r.z[(c / t + h) * w + c % t + h] > 0.0)).collect();
                std::fs::write(format!("{name}.l.f32"), r.change.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())
                    .expect("the change");
                std::fs::write(format!("{name}.land"), land).expect("the land mask");
                continue;
            }
            let (lo, hi) = r.change.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, u), v| (l.min(*v), u.max(*v)));
            println!(
                "  stored change L against B: from {lo:.1} m to {hi:.1} m; |L| over 8 km: {}",
                r.change.iter().filter(|v| v.abs() > 8000.0).count()
            );
            let gate = surface_gate(&g, id, &base, &r, 6);
            println!(
            "  G-SURF at {} lattice nodes: runtime minus bake max {:.4} m unquantised, {:.4} m with L in steps of 0.25 m (of which B re-read from the local cells: {:.4} m); at the core's cell centres {:.1e} m and {:.4} m",
            gate.nodes, gate.max_unquantised_m, gate.max_quantised_m, gate.max_base_resample_m, gate.max_centres_unquantised_m, gate.max_centres_quantised_m
        );
            println!("  other readings at the nodes (L quantised): B plus the unclamped read of L: max {:.4} m; the clamped read of the composed B + L: max {:.4} m; the unclamped read of L overshoots its clamped read by up to {:.2} m", gate.max_unclamped_l_quantised_m, gate.max_compose_quantised_m, gate.max_l_overshoot_m);
            spatial(&g, &r, &gate);
            println!(
                "  G-SURF-I at the nodes: by the bake's local step height (up to 5 m, to 20 m, over 20 m) nodes {:?}, over 0.25 m {:?}; largest difference over step height {:.3}; the nodes over 0.25 m fall in {} of {} bins of 64 cells with nodes, at most {} in one bin; p95 {:.4} m",
                gate.nodes_by_step, gate.over_by_step, gate.max_error_over_step, gate.bins_over, gate.bins, gate.busiest_bin_over, gate.p95_quantised_m
            );
            println!(
                "  G-SURF-I error against the local base slope: {} nodes with a base rise of at least 1 m across one cell; difference over that rise: p99 {:.3}, max {:.3}; overshoot of the runtime read beyond the four composed cells: max {:.4} m, {} nodes over 0.25 m",
                gate.nodes_with_rise, gate.p99_error_over_rise, gate.max_error_over_rise, gate.max_runtime_overshoot_m, gate.runtime_overshoot_nodes
            );
            println!(
                "  quantised difference at the nodes: p50 {:.4} m, p99 {:.4} m, {} nodes over 0.25 m ({:.2} %)",
                gate.p50_quantised_m,
                gate.p99_quantised_m,
                gate.over_bound,
                100.0 * gate.over_bound as f64 / gate.nodes as f64
            );
        }
    }
    let _ = spec_l;
}

/// Where the nodes over 0.25 m are: the bounding box, the 64-cell bins they fall in, those near a burnt channel cell or a step in the surface, the worst five.
fn spatial(g: &TileGrid, r: &K7Result, gate: &planet_generators::bake::k7::SurfaceGate) {
    let w = g.window();
    if gate.failing.is_empty() {
        println!("  no node over 0.25 m");
        return;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    let mut bins = std::collections::BTreeMap::new();
    let (mut near_channel, mut near_step) = (0usize, 0usize);
    for &(x, y, _) in &gate.failing {
        (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
        *bins.entry(((x / 64.0) as i64, (y / 64.0) as i64)).or_insert(0usize) += 1;
        let (cx, cy) = (x as i64, y as i64);
        let cell = |a: i64, b: i64| (a.clamp(0, w as i64 - 1) as usize, b.clamp(0, w as i64 - 1) as usize);
        if (-3..=3i64).any(|dy| {
            (-3..=3i64).any(|dx| {
                r.burn_mask[{
                    let (a, b) = cell(cx + dx, cy + dy);
                    b * w + a
                }]
            })
        }) {
            near_channel += 1;
        }
        let here = {
            let (a, b) = cell(cx, cy);
            r.z[b * w + a]
        };
        let step = (-2..=2i64)
            .flat_map(|dy| (-2..=2i64).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| {
                let (a, b) = cell(cx + dx, cy + dy);
                libm::fabs(r.z[b * w + a] - here)
            })
            .fold(0.0f64, f64::max);
        if step > 20.0 {
            near_step += 1;
        }
    }
    println!(
        "  the {} nodes over 0.25 m: window x {:.0}..{:.0}, y {:.0}..{:.0} of {w}; in {} of {} bins of 64 cells; {near_channel} within 3 cells of a burnt channel cell; {near_step} beside a step of over 20 m within 2 cells",
        gate.failing.len(), x0, x1, y0, y1, bins.len(), (w / 64 + 1) * (w / 64 + 1)
    );
    let mut worst = gate.failing.clone();
    worst.sort_by(|a, b| b.2.total_cmp(&a.2));
    for &(x, y, d) in worst.iter().take(5) {
        let c = (y as usize) * w + x as usize;
        println!("    worst: x {x:.1}, y {y:.1}: {d:.2} m; z there {:.1} m, B {:.1} m", r.z[c], r.b[c]);
    }
}
