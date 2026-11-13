//! SPIKE-114 (spike branch, never merged): rivers routed at L inside the region R3a. Routes K7's start surface `B + regional` at L, takes the channel cells by drainage area, runs the
//! network levels on the drained surface (D = infinity: all breach; D = 20 m: deeper pits stay lakes), and compares the region's edge with H1, the cost and a rebuild from B alone.
//!
//! Usage: `rivers_at_l <commit dir> [--threads T]` (R3a of face 1, seed 14, margin 200).
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
    let cell_m = 101.8f64;
    let dir_of = |x: i64, y: i64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
    let sample = |f: &(dyn Fn(Vec3) -> f64 + Sync)| -> Vec<f64> {
        let mut out = vec![0f64; w * h];
        let rows = h.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
                s.spawn(move || {
                    for (c, v) in chunk.iter_mut().enumerate() {
                        let p = k * rows * w + c;
                        *v = f(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64));
                    }
                });
            }
        });
        out
    };
    let regional = &front.regional_field;
    let z0 = sample(&|d: Vec3| base(d) + regional.at(d));
    let t_sample = t0.elapsed().as_secs_f64();
    // GEN-188 (lead 2026-11-14): the clamped C1 read of the regional change may step at the regional cell-centre lines (local x = 8i + 3.5): the jump across the pair (8i+3, 8i+4) against the mean of
    // the two neighbouring differences, over 0.1 m, counted for both axes.
    let (steps_over, line_pairs) = {
        let r = sample(&|d: Vec3| regional.at(d));
        let (mut over, mut pairs) = (0usize, 0usize);
        let half = (l / 2 - 1) as i64; // 3: the cell before the centre line within a regional cell
        for y in 1..h - 2 {
            for x in 1..w - 2 {
                let (gx, gy) = (x0 + x as i64, y0 + y as i64);
                if gx.rem_euclid(l as i64) == half {
                    let c = y * w + x;
                    let jump = r[c + 1] - r[c];
                    let around = 0.5 * ((r[c] - r[c - 1]) + (r[c + 2] - r[c + 1]));
                    pairs += 1;
                    over += usize::from((jump - around).abs() > 0.1);
                }
                if gy.rem_euclid(l as i64) == half {
                    let c = y * w + x;
                    let jump = r[c + w] - r[c];
                    let around = 0.5 * ((r[c] - r[c - w]) + (r[c + 2 * w] - r[c + w]));
                    pairs += 1;
                    over += usize::from((jump - around).abs() > 0.1);
                }
            }
        }
        (over, pairs)
    };
    println!("RIVERS GEN-188 steps of the regional read at its cell-centre lines: {steps_over} of {line_pairs} line-crossing L cell pairs jump over 0.1 m beyond their neighbours");
    // The runoff of each L cell: its H1 cell's (m/s), as H1 reads it.
    let h1_runoff: Vec<f32> = (0..rw * rh)
        .map(|c| {
            let d = front.solve.face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
            if front.solve.zs[c] > 0.0 { climate(d).0 as f32 } else { 0.0 }
        })
        .collect();
    let g = planet_generators::bake::Grid::new(w, h, cell_m);
    let outlet = g.outlets(&z0, 0.0);
    // Step 2 (PLANET_CONDITION): K7's local carve (pass 1's breach, incision with the area capped at about 1 km2) run once over the whole region before the routing; z0 stays the reference of the cut.
    let t_cond = t0.elapsed().as_secs_f64();
    let zr: Vec<f64> = if std::env::var("PLANET_CONDITION").is_ok() {
        let q = planet_generators::bake::k7::K7Params::standard(seed, 0.0).pass2;
        let local = planet_generators::bake::carve::CarveParams {
            max_area: q.local_area_cells * cell_m * cell_m,
            round_rules: planet_generators::bake::route::RoundRules::default(),
            ..q.carve
        };
        let mut z = z0.clone();
        let _ = planet_generators::bake::carve::carve(&g, &mut z, &outlet, &local, None, front.solve.stream);
        z
    } else {
        z0.clone()
    };
    println!("RIVERS conditioning {:.0} s (on: {})", t0.elapsed().as_secs_f64() - t_cond, std::env::var("PLANET_CONDITION").is_ok());
    let filled = planet_generators::bake::flood::priority_flood_with(&g, &zr, &outlet, 1.0e-3, false);
    let t_fill = t0.elapsed().as_secs_f64();
    // Routing at L on the start surface (the canonical flood drains every pit over its spill: the routes of a breach and of a lake are the same; only the levels differ).
    let route = |z: &[f64]| planet_generators::bake::route::route_rules(&g, z, &outlet, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
    let routing = route(&zr);
    let t_route = t0.elapsed().as_secs_f64();
    // D8 accumulation of the area and of the discharge (the H1 cell's runoff over each L cell) down the receivers.
    let none = planet_generators::bake::route::NONE;
    let (mut area, mut q) = (vec![cell_m * cell_m; w * h], vec![0f64; w * h]);
    for c in 0..w * h {
        let (hx, hy) = (c % w / l, c / w / l);
        q[c] = f64::from(h1_runoff[hy * rw + hx]) * cell_m * cell_m;
    }
    for &c in routing.order.iter().rev() {
        let c = c as usize;
        let r = routing.receiver[c];
        if r != none {
            area[r as usize] += area[c];
            q[r as usize] += q[c];
        }
    }
    let threshold = planet_generators::bake::hydro::HydroParams::default().initiation_m2;
    let channel: Vec<bool> = area.iter().zip(&z0).map(|(a, z)| *a >= threshold && *z > 0.0).collect();
    let t_acc = t0.elapsed().as_secs_f64();
    // Pits (for the lakes of D = 20 m): cells over 5 cm under the fill, 8-connected, with their depth.
    let mut label = vec![u32::MAX; w * h];
    let mut depth_of: Vec<f64> = Vec::new();
    for s0 in 0..w * h {
        if filled[s0] - zr[s0] <= 0.05 || label[s0] != u32::MAX {
            continue;
        }
        let id = depth_of.len() as u32;
        let mut maxd = 0.0f64;
        let mut stack = vec![s0];
        label[s0] = id;
        while let Some(c) = stack.pop() {
            maxd = maxd.max(filled[c] - zr[c]);
            let (cx, cy) = ((c % w) as i64, (c / w) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let qq = ny as usize * w + nx as usize;
                    if filled[qq] - zr[qq] > 0.05 && label[qq] == u32::MAX {
                        label[qq] = id;
                        stack.push(qq);
                    }
                }
            }
        }
        depth_of.push(maxd);
    }
    let tile = spec.tile as i64;
    let inside = |c: usize| commit.tiles.contains_key(&(face.0, ((x0 + (c % w) as i64) / tile) as u32, ((y0 + (c / w) as i64) / tile) as u32));
    let channel_inside = (0..w * h).filter(|&c| channel[c] && inside(c)).count();
    println!(
        "RIVERS region {w} x {h} L cells ({:.1} M); front {:.0} s, start surface {:.0} s, fill {:.0} s, route {:.0} s, accumulation {:.0} s; channel cells (area >= {:.0} km2) {} in the window, {} inside the commit's tiles; {} pits",
        (w * h) as f64 / 1e6,
        t_front,
        t_sample - t_front,
        t_fill - t_sample,
        t_route - t_fill,
        t_acc - t_route,
        threshold / 1e6,
        channel.iter().filter(|&&b| b).count(),
        channel_inside,
        depth_of.len()
    );
    // Per variant: the network levels (the running minimum of the drained surface down the channel cells, upstream first), the cut against the start surface and the rises.
    for d_max in [f64::INFINITY, 20.0] {
        let lake = |c: usize| label[c] != u32::MAX && depth_of[label[c] as usize] > d_max;
        let drained = |c: usize| if lake(c) { filled[c] } else { zr[c] };
        let mut level = vec![f64::INFINITY; w * h];
        for &c in routing.order.iter().rev() {
            let c = c as usize;
            if !channel[c] {
                continue;
            }
            let v = level[c].min(drained(c));
            level[c] = v;
            let r = routing.receiver[c];
            if r != none && channel[r as usize] {
                level[r as usize] = level[r as usize].min(v);
            }
        }
        let mut cuts: Vec<f64> = Vec::new();
        let (mut steps, mut rises, mut water_rises, mut lake_cells, mut pit_cells) = (0usize, 0usize, 0usize, 0usize, 0usize);
        let ground = |c: usize| if lake(c) { zr[c] } else { zr[c].min(level[c]) };
        let water = |c: usize| if lake(c) { filled[c] } else { ground(c) };
        for c in 0..w * h {
            if !channel[c] || !inside(c) {
                continue;
            }
            cuts.push(z0[c] - level[c].min(z0[c]).max(level[c]));
            lake_cells += usize::from(lake(c));
            pit_cells += usize::from(label[c] != u32::MAX);
            let r = routing.receiver[c];
            if r != none && channel[r as usize] && z0[r as usize] > 0.0 {
                steps += 1;
                rises += usize::from(ground(r as usize) - ground(c) > 0.25);
                water_rises += usize::from(water(r as usize) - water(c) > 0.25);
            }
        }
        cuts.sort_by(f64::total_cmp);
        let p = |f: f64| cuts[((cuts.len() as f64 * f) as usize).min(cuts.len() - 1)];
        let over = cuts.iter().filter(|&&v| v > 50.0).count();
        println!(
            "RIVERS D {d_max} m: {} channel cells inside the tiles ({pit_cells} in a pit of the start surface, {lake_cells} in lakes); cut under the start surface p50 {:.2} p95 {:.2} p99 {:.2} max {:.2} m, {over} over 50 m ({:.2} %); ground rises over 0.25 m {rises} of {steps} steps, water rises {water_rises}",
            cuts.len(),
            p(0.5),
            p(0.95),
            p(0.99),
            cuts.last().copied().unwrap_or(0.0),
            100.0 * over as f64 / cuts.len().max(1) as f64
        );
    }
    // The region's edge: where H1's network and the L channels cross the boundary of the region (its regional cells without the margin), in and out, with the discharge.
    let (ei0, ei1, ej0, ej1) = ((spec.region.0 * l) as i64, ((spec.region.1 + 1) * l) as i64, (spec.region.2 * l) as i64, ((spec.region.3 + 1) * l) as i64);
    let in_region = |x: i64, y: i64| x >= ei0 && x < ei1 && y >= ej0 && y < ej1;
    // L crossings: a channel cell and its receiver on the two sides of the boundary.
    let mut l_cross: Vec<(bool, i64, i64, f64)> = Vec::new(); // (outflow?, x, y, Q m3/s)
    for c in 0..w * h {
        let r = routing.receiver[c];
        if !channel[c] || r == none {
            continue;
        }
        let (cx, cy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
        let (rx, ry) = (x0 + (r as usize % w) as i64, y0 + (r as usize / w) as i64);
        let (a, b) = (in_region(cx, cy), in_region(rx, ry));
        if a != b {
            l_cross.push((a, cx, cy, q[c]));
        }
    }
    // H1 crossings: consecutive vertices of a segment on the two sides.
    let to_local = |c: u32| {
        let c = c as usize;
        ((l * (ri0 + c % rw)) as f64 + (l as f64 - 1.0) / 2.0, (l * (rj0 + c / rw)) as f64 + (l as f64 - 1.0) / 2.0)
    };
    let mut h_cross: Vec<(bool, i64, i64, f64)> = Vec::new();
    front.graph_h1.for_each(|s| {
        let cells = s.cells(&front.solve.grid);
        for k in 1..cells.len() {
            let (p0, p1) = (to_local(cells[k - 1]), to_local(cells[k]));
            let (a, b) = (in_region(p0.0 as i64, p0.1 as i64), in_region(p1.0 as i64, p1.1 as i64));
            if a != b {
                h_cross.push((a, p0.0 as i64, p0.1 as i64, planet_generators::bake::channel_graph::q_value(s.q[k - 1])));
            }
        }
    });
    let mut f = String::from("# kind x y q_h1_m3s | nearest L crossing of the same kind: distance_km q_l_m3s ratio\n");
    let (mut matched, mut far) = (0usize, Vec::new());
    for &(out, hx, hy, hq) in h_cross.iter().filter(|c| c.3 >= 1.0) {
        let best = l_cross
            .iter()
            .filter(|c| c.0 == out)
            .map(|c| (((c.1 - hx).pow(2) + (c.2 - hy).pow(2)) as f64).sqrt() * cell_m / 1000.0)
            .zip(l_cross.iter().filter(|c| c.0 == out))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, lc)) = best {
            f += &format!("{} {hx} {hy} {hq:.1} | {d:.2} {:.1} {:.2}\n", if out { "out" } else { "in" }, lc.3, lc.3 / hq);
            if d <= 2.0 {
                matched += 1;
            } else {
                far.push((out, hx, hy, hq, d));
            }
        }
    }
    std::fs::write("target/review/k7-beds/rivers-edge.txt", &f).map_err(|e| e.to_string())?;
    let big = h_cross.iter().filter(|c| c.3 >= 1.0).count();
    println!(
        "RIVERS edge: H1 crossings with Q >= 1 m3/s {big} ({} out), L crossings {} ({} out); {matched} H1 crossings have an L crossing of the same kind within 2 km; farther: {:?}",
        h_cross.iter().filter(|c| c.3 >= 1.0 && c.0).count(),
        l_cross.len(),
        l_cross.iter().filter(|c| c.0).count(),
        far.iter().take(12).map(|c| format!("{} ({},{}) Q {:.0} at {:.1} km", if c.0 { "out" } else { "in" }, c.1, c.2, c.3, c.4)).collect::<Vec<_>>()
    );
    // Rebuild at read time from B alone (bake_base(), VIEW-072): the channel cells of a routing on B against those on B + regional.
    let t_b = t0.elapsed().as_secs_f64();
    let zb = sample(&|d: Vec3| base(d));
    let outlet_b = g.outlets(&zb, 0.0);
    let routing_b = planet_generators::bake::route::route_rules(&g, &zb, &outlet_b, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
    let mut area_b = vec![cell_m * cell_m; w * h];
    for &c in routing_b.order.iter().rev() {
        let c = c as usize;
        let r = routing_b.receiver[c];
        if r != none {
            area_b[r as usize] += area_b[c];
        }
    }
    let t_b_done = t0.elapsed().as_secs_f64();
    // The tell of rivers following the regional cell-centre lines: channel steps whose two cells sit on the same side pair of one line (columns 8i+3 / 8i+4, or rows) and that run parallel to it.
    let along = |recv: &[u32], chan: &dyn Fn(usize) -> bool| -> (usize, usize) {
        let (mut n_steps, mut n_along) = (0usize, 0usize);
        let on_line = |v: i64| { let m = v.rem_euclid(l as i64); m == (l / 2 - 1) as i64 || m == (l / 2) as i64 };
        for c in 0..w * h {
            let r = recv[c];
            if r == none || !chan(c) || !inside(c) {
                continue;
            }
            let (cx, cy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            let (rx, ry) = (x0 + (r as usize % w) as i64, y0 + (r as usize / w) as i64);
            n_steps += 1;
            let vertical_along = cx == rx && on_line(cx);
            let horizontal_along = cy == ry && on_line(cy);
            n_along += usize::from(vertical_along || horizontal_along);
        }
        (n_steps, n_along)
    };
    let (sa, aa) = along(&routing.receiver, &|c| channel[c]);
    let (sb, ab) = along(&routing_b.receiver, &|c| area_b[c] >= threshold && zb[c] > 0.0);
    println!(
        "RIVERS along the regional cell-centre lines: B + regional {aa} of {sa} channel steps ({:.2} %), B alone {ab} of {sb} ({:.2} %) (a straight step on one of the 2 of 8 columns or rows next to a line)",
        100.0 * aa as f64 / sa.max(1) as f64,
        100.0 * ab as f64 / sb.max(1) as f64
    );
    let (mut both, mut only_a, mut only_b) = (0usize, 0usize, 0usize);
    for c in 0..w * h {
        if !inside(c) {
            continue;
        }
        let (a, b) = (channel[c], area_b[c] >= threshold && zb[c] > 0.0);
        both += usize::from(a && b);
        only_a += usize::from(a && !b);
        only_b += usize::from(b && !a);
    }
    println!(
        "RIVERS store or rebuild: stored L geometry ~ {:.1} MB for the commit's tiles (1 direction byte + 2 bytes Q + 2 bytes level a channel cell); rebuild from B alone {:.0} s for the region window (sample, flood, route, accumulate); channel cells inside the tiles on B + regional {}, on B alone {}, in both {both} ({:.1} % of B + regional's)",
        (channel_inside * 5) as f64 / 1e6,
        t_b_done - t_b,
        both + only_a,
        both + only_b,
        100.0 * both as f64 / (both + only_a).max(1) as f64
    );
    println!("RIVERS total {:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}
