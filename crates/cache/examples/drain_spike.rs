//! Read-only cost spike (#125, lead 2026-11-13): a region-wide drain of K7's start surface `B + regional` at L before K7, as a breach/fill hybrid. The start surface is filled by one
//! priority flood over the whole region window; a pit (cells over 5 cm under the fill, 8-connected) whose depth below its spill is at most D is breached (it keeps its heights and the network
//! levels carry its bottom downstream), a deeper one is a lake (its cells read at the spill). For D = 5, 20, 50 m and infinity it reports the lakes left (count, area), and, along the placed lines with
//! the network levels (GEN-185) on that surface, the line cells cut over 50 m under the start surface and the steps whose ground rises over 0.25 m, inside the commit's tiles.
//!
//! Usage: `drain_spike <commit dir> [--threads T]` (R3a of face 1, seed 14, margin 200).
use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::Commit;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

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
        Ok(GlobalSample { continent: 0.0, elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)], temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(&args[0]);
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    let (seed, face, margin) = (14u64, Face(1), 200usize);
    let (i0, j0, size) = (2014 * 4, 190 * 4, (2168 * 4 + 3) - 2014 * 4 + 1);
    let t0 = std::time::Instant::now();
    let def = WorldDefinition::single_planet(R, seed);
    let _world = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
    let inputs = class_inputs(&def, seed, threads);
    let region = (i0, i0 + size - 1, j0, j0 + size - 1);
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f32; nw * nh];
    let rows = nh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in nodes.chunks_mut(rows * nw).enumerate() {
            let inputs = &inputs;
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * nw + c;
                    *v = inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)) as f32;
                }
            });
        }
    });
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes };
    let base = |d: Vec3| -> f64 { f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle")) };
    let orography = inputs.orography();
    let climate = |d: Vec3| {
        let (pm, tc) = (orography.precipitation_mm(d), orography.temperature_c(d));
        let ro = runoff_mm(pm, tc);
        (ro / 1000.0 / SECONDS_PER_YEAR, (pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR, discharge_weight(ro))
    };
    let spec = RegionSpec {
        seed,
        face,
        radius_m: R,
        regional: REGIONAL,
        region,
        margin,
        tile: 768,
        halo: 64,
        rounds: 4,
        threads,
        res: LATTICE as u32,
        base_id: 0x6c61_7474_6963_6531 ^ (LATTICE as u64),
        inputs_id: class_inputs_key(&def, seed),
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    };
    let front = region_front(&spec, &base, &climate)?;
    let t_front = t0.elapsed().as_secs_f64();
    let commit = Commit::read(dir)?.ok_or("no commit record")?;
    let l = planet_cache::region_bake::LOCAL_PER_REGIONAL;
    let (ri0, rj0, rw, rh) = (front.solve.ri0, front.solve.rj0, front.solve.rw, front.solve.rh);
    let (x0, y0, w, h) = ((ri0 * l) as i64, (rj0 * l) as i64, rw * l, rh * l);
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
    // 1. The start surface at every local cell of the region window.
    let mut z0 = vec![0f64; w * h];
    {
        let rows = h.div_ceil(threads);
        let regional = &front.regional_field;
        let base = &base;
        std::thread::scope(|s| {
            for (k, chunk) in z0.chunks_mut(rows * w).enumerate() {
                s.spawn(move || {
                    for (c, v) in chunk.iter_mut().enumerate() {
                        let p = k * rows * w + c;
                        let d = dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
                        *v = base(d) + regional.at(d);
                    }
                });
            }
        });
    }
    let t_sample = t0.elapsed().as_secs_f64();
    // 2. The fill (one priority flood over the region, the window's border and the sea as outlets), the pits and their depth, area and extent.
    let g = planet_generators::bake::Grid::new(w, h, 101.8);
    let outlet = g.outlets(&z0, 0.0);
    let filled = planet_generators::bake::flood::priority_flood_with(&g, &z0, &outlet, 1.0e-3, true);
    drop(outlet);
    let t_fill = t0.elapsed().as_secs_f64();
    let mut label = vec![u32::MAX; w * h];
    // depth below the spill, area, extent (cells: the larger side of the bounding box)
    let mut pits: Vec<(f64, usize, i64)> = Vec::new();
    for start in 0..w * h {
        if filled[start] - z0[start] <= 0.05 || label[start] != u32::MAX {
            continue;
        }
        let id = pits.len() as u32;
        let (mut maxd, mut area) = (0.0f64, 0usize);
        let (mut bx0, mut bx1, mut by0, mut by1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        let mut stack = vec![start];
        label[start] = id;
        while let Some(c) = stack.pop() {
            maxd = maxd.max(filled[c] - z0[c]);
            area += 1;
            let (cx, cy) = ((c % w) as i64, (c / w) as i64);
            (bx0, bx1, by0, by1) = (bx0.min(cx), bx1.max(cx), by0.min(cy), by1.max(cy));
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let q = ny as usize * w + nx as usize;
                    if filled[q] - z0[q] > 0.05 && label[q] == u32::MAX {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
        pits.push((maxd, area, (bx1 - bx0 + 1).max(by1 - by0 + 1)));
    }
    let t_label = t0.elapsed().as_secs_f64();
    let widest = pits.iter().map(|p| p.2).max().unwrap_or(0);
    println!(
        "SPIKE region {} x {} local cells ({:.1} M); front {:.0} s, start surface {:.0} s, fill {:.0} s, pits {:.0} s; {} pits, the widest {} cells across",
        w,
        h,
        (w * h) as f64 / 1e6,
        t_front,
        t_sample - t_front,
        t_fill - t_sample,
        t_label - t_fill,
        pits.len(),
        widest
    );
    let at = |x: i64, y: i64| ((y - y0) as usize) * w + (x - x0) as usize;
    let inside_window = |x: i64, y: i64| x >= x0 && y >= y0 && x < x0 + w as i64 && y < y0 + h as i64;
    let tile = spec.tile as i64;
    let keep = |x: i64, y: i64| commit.tiles.contains_key(&(face.0, (x / tile) as u32, (y / tile) as u32));
    let to_local = |c: u32| {
        let c = c as usize;
        ((l * (ri0 + c % rw)) as f64 + (l as f64 - 1.0) / 2.0, (l * (rj0 + c / rw)) as f64 + (l as f64 - 1.0) / 2.0)
    };
    let base_at = |x: i64, y: i64| base(dir_of(x, y));
    let placement = planet_generators::bake::placement::Placement::new(&base_at);
    let start0 = |x: i64, y: i64| if inside_window(x, y) { z0[at(x, y)] } else { base(dir_of(x, y)) + front.regional_field.at(dir_of(x, y)) };
    for d_max in [5.0, 20.0, 50.0, f64::INFINITY] {
        let t_d = t0.elapsed().as_secs_f64();
        let lake = |k: u32| k != u32::MAX && pits[k as usize].0 > d_max;
        let mut areas: Vec<usize> = pits.iter().filter(|p| p.0 > d_max).map(|p| p.1).collect();
        areas.sort();
        let lake_cells: usize = areas.iter().sum();
        // the drained surface: a lake reads at its spill, a breached pit at its own heights
        let drained = |x: i64, y: i64| {
            if inside_window(x, y) {
                let c = at(x, y);
                if lake(label[c]) {
                    filled[c]
                } else {
                    z0[c]
                }
            } else {
                start0(x, y)
            }
        };
        let mut graph = front.graph_h1.clone();
        graph.deepen_beds(&front.solve.grid, &to_local, &drained, Some(&placement));
        let cut = planet_cache::region_bake::BedCut::of(&graph, &front.solve.grid, &to_local, &start0, &keep, Some(&placement));
        // The ground and the water along the placed lines inside the tiles, as K7's burn makes them: the burn carries the lowest of the level and the found height (the drained surface: a lake
        // cell is found at its spill) down the line; a cell outside a lake is burnt to that, a lake cell keeps its start height under the water at its spill. Steps rising over 0.25 m.
        let (mut steps, mut rises, mut water_rises) = (0usize, 0usize, 0usize);
        graph.for_each(|s| {
            let cells = s.cells(&front.solve.grid);
            let mut prev: Option<(f64, f64)> = None;
            let mut run = f64::INFINITY;
            for k in 0..cells.len() {
                let p1 = to_local(cells[k]);
                let p0 = if k == 0 { p1 } else { to_local(cells[k - 1]) };
                let (l0, l1) = (
                    planet_generators::bake::channel_graph::bed_value(s.bed[if k == 0 { 0 } else { k - 1 }]),
                    planet_generators::bake::channel_graph::bed_value(s.bed[k]),
                );
                let line = if k == 0 { vec![placement.vertex(p1)] } else { placement.step(placement.vertex(p0), placement.vertex(p1), p0, p1) };
                let m = line.len();
                for (i, (x, y)) in line.into_iter().enumerate() {
                    if k > 0 && i == 0 {
                        continue;
                    }
                    let level = if m > 1 { l0 + (l1 - l0) * i as f64 / (m - 1) as f64 } else { l1 };
                    let in_lake = inside_window(x, y) && lake(label[at(x, y)]);
                    run = run.min(level.min(drained(x, y)));
                    let z = start0(x, y);
                    let ground = if in_lake { z } else { z.min(run) };
                    let water = if in_lake { filled[at(x, y)] } else { ground };
                    if let Some((pg, pw)) = prev {
                        if keep(x, y) && z > 0.0 {
                            steps += 1;
                            rises += usize::from(ground - pg > 0.25);
                            water_rises += usize::from(water - pw > 0.25);
                        }
                    }
                    prev = Some((ground, water));
                }
            }
        });
        println!(
            "SPIKE D {d_max} m: lakes {} ({} cells, {:.2} % of the window), area p50 {} max {} cells; inside the tiles {}; ground rises over 0.25 m {rises} of {steps} steps, water-surface rises {water_rises} ({:.0} s)",
            areas.len(),
            lake_cells,
            100.0 * lake_cells as f64 / (w * h) as f64,
            areas.get(areas.len() / 2).copied().unwrap_or(0),
            areas.last().copied().unwrap_or(0),
            cut.line(),
            t0.elapsed().as_secs_f64() - t_d
        );
    }
    println!("SPIKE total {:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}
