//! GEN-148 (briefing item 11, A3 section 3.3, G-SURF): K7 in the new chain on a tile of the prototype region at the erosion and the local grid. The base is the store lattice's C1 read
//! (`read_elevation_c1`) of a lattice sampled from `BakeInputs::base` at the nodes (611 m, k = 14; stored as f32); the regional solve (12,288 cells, 814 m, 4 rounds) starts from the same read; the
//! channels of H1 on the solved surface are burned into a 1,024-cell tile of the local grid (102 m, 98,304 cells a face edge, 8 local cells to a regional cell); then the breach, the incision, thermal,
//! the deposition and the pit spreading. Reports the pit share and depth at 102 m after each stage, with and without the channels, and G-SURF at the lattice nodes.
//! Usage: k7_region [margin_regional_cells] [threads]
#![allow(clippy::needless_range_loop)]
use planet_core::cube::TangentWarp;
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
use planet_generators::runoff::{discharge_weight, runoff_mm};
use std::time::Instant;

const K: u32 = 14;
const LATTICE: usize = 1 << K; // 16,384 nodes a face edge
const REGIONAL: usize = 12_288;
const LOCAL: usize = 6 << K; // 98,304
const R: f64 = 6_371_000.0;
const REGION: (usize, usize, usize, usize) = (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3);

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
    let seed = 14u64;
    let face = Face(1);
    let t0 = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    // ---- the regional window and its lattice rectangle (a regional cell is 8 local cells, a node every 6 local cells)
    let (ri0, ri1) = (REGION.0 - margin, REGION.1 + margin);
    let (rj0, rj1) = (REGION.2 - margin, REGION.3 + margin);
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
                    *v = inputs.base(spec.halo_direction((ni0 + p % nw) as i64, (nj0 + p / nw) as i64)) as f32;
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
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
    let stream = spec_r.stream();
    let mut zs = zb.clone();
    solve_rounds(&grid, &mut zs, &outlet, &p, 4, &threads, stream);
    println!("regional window {rw} x {rh} cells solved from the C1 read of the lattice ({:.1} s)", t0.elapsed().as_secs_f64());
    let change: Vec<f32> = zs.iter().zip(&zb).map(|(s, b)| (s - b) as f32).collect();
    let regional_field = RegionField::new(face, REGIONAL, (ri0 as i64, rj0 as i64), rw, rh, change);
    let regional = |d: Vec3| regional_field.at(d);

    // ---- H1 on the solved surface: the channel segments
    let o = inputs.orography();
    let weight: Vec<f32> = (0..n)
        .map(|c| {
            if zs[c] <= 0.0 {
                return 1.0;
            }
            let d = spec_r.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
            discharge_weight(runoff_mm(o.precipitation_mm(d), o.temperature_c(d))) as f32
        })
        .collect();
    let routing = route_rules(&grid, &zs, &outlet, 1e-3, stream, 0.0, Some(&weight), RoundRules::ROUND);
    let filled = priority_flood_with(&grid, &zs, &outlet, 1e-3, true);
    let hy = hydrology(&grid, &zs, &filled, &routing, &outlet, None, None, &HydroParams::default());
    println!("H1: {} channel cells, {} segments", hy.stats.channel_cells, hy.stats.segments);

    // ---- the tile
    let g = TileGrid { cells: LOCAL, tile: 1024, halo: 64, radius_m: R };
    let id = TileId { face, ti: (8390 * 8 + 4) as u32 / 1024, tj: (958 * 8 + 4) as u32 / 1024 };
    let (t, h) = (g.tile as i64, g.halo as i64);
    let (wi0, wj0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
    let ww = g.window() as i64;
    println!(
        "tile ({}, {}) of face 1: local cells i {}..{}, j {}..{} ({:.1} km a side)",
        id.ti,
        id.tj,
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
        channels.push(Channel { points: cells.iter().map(|&c| to_local(c)).collect(), levels });
    }
    println!("{} channel segments meet the window", channels.len());

    let params = K7Params::standard(seed, 0.0);
    for (label, ch) in [("with the channels burned in", &channels[..]), ("without the channels", &[][..])] {
        let t = Instant::now();
        let r: K7Result = k7_tile(&g, id, &base, &regional, ch, &params, threads);
        println!("K7 {label}: {:.1} s, {} cells burned", t.elapsed().as_secs_f64(), r.burned);
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
        let (lo, hi) = r.change.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, u), v| (l.min(*v), u.max(*v)));
        println!(
            "  stored change L against B: from {lo:.1} m to {hi:.1} m; |L| over 8 km: {}",
            r.change.iter().filter(|v| v.abs() > 8000.0).count()
        );
        let gate = surface_gate(&g, id, &base, &r, 6);
        println!(
            "  G-SURF at {} lattice nodes: runtime minus bake max {:.4} m unquantised, {:.4} m with L in steps of 0.25 m; at the core's cell centres {:.1e} m and {:.4} m",
            gate.nodes, gate.max_unquantised_m, gate.max_quantised_m, gate.max_centres_unquantised_m, gate.max_centres_quantised_m
        );
    }
    let _ = spec_l;
}
