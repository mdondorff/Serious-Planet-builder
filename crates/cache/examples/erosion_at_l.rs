//! SPIKE-115 (spike branch, never merged): erosion rounds at L inside the region R3a (the regional solve's kernel over the 101.8 m window, N = 1, 3, 10), then routing at L with
//! SPIKE-114's metrics, the relief kept, and the cost of re-routing the dirty catchments after a ridge edit against a full L route.
//!
//! Usage: `erosion_at_l <commit dir> [--threads T]` (R3a of face 1, seed 14, margin 200); PLANET_ROUNDS_MAX stops after that many rounds.
use std::cmp::Reverse;
use std::collections::BinaryHeap;
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
        Ok(GlobalSample {
            continent: 0.0,
            elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)],
            temperature_c: 0.0,
            precipitation_mm: 0.0,
        })
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(&args[0]);
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    // `--window face,i0,i1,j0,j1,seed` and `--margin M` choose another region (the spikes lane's notation); the default is the r3a stores' window (face 1, seed 14).
    let win: Option<Vec<usize>> = args
        .iter()
        .position(|a| a == "--window")
        .map(|k| args[k + 1].split(',').map(|x| x.parse().expect("--window face,i0,i1,j0,j1,seed")).collect());
    let margin: usize = args.iter().position(|a| a == "--margin").map_or(200, |k| args[k + 1].parse().expect("--margin M"));
    let (seed, face) = win.as_ref().map_or((14u64, Face(1)), |v| (v[5] as u64, Face(v[0] as u8)));
    let (i0, j0, size) = win.as_ref().map_or((2014 * 4, 190 * 4, (2168 * 4 + 3) - 2014 * 4 + 1), |v| (v[1], v[3], v[2] - v[1] + 1));
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
    // Without a commit (`-`), "inside" is the region's box of tiles (every tile the region meets).
    let commit_tiles: Option<std::collections::BTreeSet<(u8, u32, u32)>> =
        if dir.to_str() == Some("-") { None } else { Some(Commit::read(dir)?.ok_or("no commit record")?.tiles.keys().copied().collect()) };
    let l = planet_cache::region_bake::LOCAL_PER_REGIONAL;
    let (ri0, rj0, rw, rh) = (front.solve.ri0, front.solve.rj0, front.solve.rw, front.solve.rh);
    let (x0, y0, w, h) = ((ri0 * l) as i64, (rj0 * l) as i64, rw * l, rh * l);
    let n = spec.local() as f64;
    let cell_m = 101.8f64;
    let dir_of = |x: i64, y: i64| {
        planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0)
    };
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
    // SPIKE-117: PLANET_TRANSFER=d4 replaces the start surface with SPIKE-116's hand-over: the solved 814 m surface breached over 4 neighbours (0.01 m a step, floor 0.005 m), read bilinearly
    // at L, plus B's sub-814 m residual damped to 0 toward the channels of that surface over 400 m; a land cell of the D4 surface stays at or above 0.005 m (the sea floor of the residual).
    let mut zb_l: Option<Vec<f64>> = None;
    if std::env::var("PLANET_TRANSFER").ok().as_deref() == Some("d4") {
        let td = t0.elapsed().as_secs_f64();
        let (rw_, rh_) = (front.solve.rw, front.solve.rh);
        let mut zs4 = front.solve.zs.clone();
        let _ = breach_nb(rw_, rh_, &mut zs4, &front.solve.outlet, 0.01, &NB4, 0.005);
        let lr = l as f64;
        let bil = |zz: &[f64]| -> Vec<f64> {
            (0..w * h)
                .map(|c| {
                    let (fx, fy) = (((c % w) as f64 - (lr - 1.0) / 2.0) / lr, ((c / w) as f64 - (lr - 1.0) / 2.0) / lr);
                    let (ix, iy) = ((fx.floor().max(0.0) as usize).min(rw_ - 2), (fy.floor().max(0.0) as usize).min(rh_ - 2));
                    let (u, v) = ((fx - ix as f64).clamp(0.0, 1.0), (fy - iy as f64).clamp(0.0, 1.0));
                    let at = |x: usize, y: usize| zz[y * rw_ + x];
                    (at(ix, iy) * (1.0 - u) + at(ix + 1, iy) * u) * (1.0 - v) + (at(ix, iy + 1) * (1.0 - u) + at(ix + 1, iy + 1) * u) * v
                })
                .collect()
        };
        let zr4 = bil(&zs4);
        let reg_b: Vec<f64> = (0..rw_ * rh_)
            .map(|c| base(front.solve.face_spec.halo_direction((front.solve.ri0 + c % rw_) as i64, (front.solve.rj0 + c / rw_) as i64)))
            .collect();
        let zbr = bil(&reg_b);
        let mut zb = vec![0f64; w * h];
        {
            let rows = h.div_ceil(threads);
            let base = &base;
            std::thread::scope(|sc| {
                for (k, chunk) in zb.chunks_mut(rows * w).enumerate() {
                    sc.spawn(move || {
                        for (c, v) in chunk.iter_mut().enumerate() {
                            let p = k * rows * w + c;
                            *v = base(dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64));
                        }
                    });
                }
            });
        }
        let gg = planet_generators::bake::Grid::new(w, h, 101.8);
        let o4 = gg.outlets(&zr4, 0.0);
        let rt4 = planet_generators::bake::route::route_rules(
            &gg,
            &zr4,
            &o4,
            1.0e-3,
            front.solve.stream,
            0.0,
            None,
            planet_generators::bake::route::RoundRules::default(),
        );
        let mut a4 = vec![101.8 * 101.8; w * h];
        for &c in rt4.order.iter().rev() {
            let c = c as usize;
            let r = rt4.receiver[c];
            if r != planet_generators::bake::route::NONE {
                a4[r as usize] += a4[c];
            }
        }
        let thr = planet_generators::bake::hydro::HydroParams::default().initiation_m2;
        let chan4: Vec<bool> = (0..w * h).map(|c| a4[c] >= thr && zr4[c] > 0.0).collect();
        drop(a4);
        let dist = chamfer(w, h, &chan4, 101.8);
        for c in 0..w * h {
            let u = (dist[c] / 400.0).min(1.0);
            let sm = u * u * (3.0 - 2.0 * u);
            z0[c] = if zr4[c] > 0.0 { (zr4[c] + sm * (zb[c] - zbr[c])).max(0.005) } else { zr4[c] };
        }
        zb_l = Some(zb);
        println!(
            "EROSION D4 hand-over (4-neighbour breach at 814 m, residual damped over 400 m, sea floor) in {:.0} s",
            t0.elapsed().as_secs_f64() - td
        );
    }
    let h1_runoff: Vec<f32> = (0..rw * rh)
        .map(|c| {
            let d = front.solve.face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
            if front.solve.zs[c] > 0.0 {
                climate(d).0 as f32
            } else {
                0.0
            }
        })
        .collect();
    let g = planet_generators::bake::Grid::new(w, h, cell_m);
    let outlet = g.outlets(&z0, 0.0);
    let none = planet_generators::bake::route::NONE;
    let tile = spec.tile as i64;
    let (bi0, bi1, bj0, bj1) =
        ((spec.region.0 * l) as i64, ((spec.region.1 + 1) * l) as i64, (spec.region.2 * l) as i64, ((spec.region.3 + 1) * l) as i64);
    let inside = |c: usize| {
        let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
        match &commit_tiles {
            Some(t) => t.contains(&(face.0, (gx / tile) as u32, (gy / tile) as u32)),
            None => gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1,
        }
    };
    let threshold = planet_generators::bake::hydro::HydroParams::default().initiation_m2;
    let rules = if std::env::var("PLANET_CANONICAL").is_ok() {
        planet_generators::bake::route::RoundRules::ROUND
    } else {
        planet_generators::bake::route::RoundRules::default()
    };
    // Relief kept: the p50 over the land windows of the height range in windows of 2 x 2 cells (about 200 m) and 10 x 10 cells (about 1 km).
    let relief = |z: &[f64], k: usize| -> f64 {
        let mut r: Vec<f64> = Vec::new();
        for by in (0..h - k).step_by(k) {
            for bx in (0..w - k).step_by(k) {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for y in by..by + k {
                    for x in bx..bx + k {
                        let v = z[y * w + x];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                if lo > 0.0 {
                    r.push(hi - lo);
                }
            }
        }
        r.sort_by(f64::total_cmp);
        r.get(r.len() / 2).copied().unwrap_or(0.0)
    };
    let (r200_0, r1k_0) = (relief(&z0, 2), relief(&z0, 10));
    println!("EROSION start surface: relief p50 over 2 x 2 cells {r200_0:.2} m, over 10 x 10 cells {r1k_0:.2} m; front {t_front:.0} s");
    // The analysis of a surface: route at L, channels by area, pits, the network levels for D = inf and 20 m, the cut against the start surface, the lakes, the rises and the edge.
    // River review (#138): PLANET_DUMP_DIR writes each surface (the start before the rounds, n0, and after each round target) as raw little-endian f64 of the w x h L window, PLANET_TAG names it.
    let dump = |zr: &[f64], k: usize| {
        if let Ok(d) = std::env::var("PLANET_DUMP_DIR") {
            let tag = std::env::var("PLANET_TAG").unwrap_or_else(|_| "surface".into());
            let _ = std::fs::create_dir_all(&d);
            let bytes: Vec<u8> = zr.iter().flat_map(|v| v.to_le_bytes()).collect();
            let path = std::path::Path::new(&d).join(format!("{tag}-n{k}.f64"));
            std::fs::write(&path, bytes).expect("the dump is written");
            println!("EROSION dump {} ({w} x {h} f64)", path.display());
        }
    };
    let analyse = |zr: &[f64], label_n: usize| -> Result<planet_generators::bake::route::Routing, String> {
        dump(zr, label_n);
        let ta = t0.elapsed().as_secs_f64();
        let filled = planet_generators::bake::flood::priority_flood_with(&g, zr, &outlet, 1.0e-3, false);
        let routing = planet_generators::bake::route::route_rules(&g, zr, &outlet, 1.0e-3, front.solve.stream, 0.0, None, rules);
        let t_route = t0.elapsed().as_secs_f64() - ta;
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
        let channel: Vec<bool> = area.iter().zip(zr).map(|(a, z)| *a >= threshold && *z > 0.0).collect();
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
        {
            // The basin metric agreed with spikes (SPIKE-117/118): a basin is at least 0.5 m deep and 16 L cells in area, under a fill with epsilon 1e-7 m a cell; the river cells inside the tiles in one.
            let fb = planet_generators::bake::flood::priority_flood_with(&g, zr, &outlet, 1.0e-7, false);
            let mut lab = vec![u32::MAX; w * h];
            let mut big: Vec<bool> = Vec::new();
            for s0 in 0..w * h {
                if fb[s0] - zr[s0] <= 1.0e-6 || lab[s0] != u32::MAX {
                    continue;
                }
                let id = big.len() as u32;
                let (mut maxd, mut area_c) = (0.0f64, 0usize);
                let mut stack = vec![s0];
                lab[s0] = id;
                while let Some(c) = stack.pop() {
                    maxd = maxd.max(fb[c] - zr[c]);
                    area_c += 1;
                    let (cx, cy) = ((c % w) as i64, (c / w) as i64);
                    for dy in -1..=1i64 {
                        for dx in -1..=1i64 {
                            let (nx, ny) = (cx + dx, cy + dy);
                            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                                continue;
                            }
                            let qq = ny as usize * w + nx as usize;
                            if fb[qq] - zr[qq] > 1.0e-6 && lab[qq] == u32::MAX {
                                lab[qq] = id;
                                stack.push(qq);
                            }
                        }
                    }
                }
                big.push(maxd >= 0.5 && area_c >= 16);
            }
            let (mut n_ch, mut in_big, mut in_5cm, mut steps_ch, mut axis) = (0usize, 0usize, 0usize, 0usize, 0usize);
            for c in 0..w * h {
                if !channel[c] || !inside(c) {
                    continue;
                }
                n_ch += 1;
                in_big += usize::from(lab[c] != u32::MAX && big[lab[c] as usize]);
                in_5cm += usize::from(label[c] != u32::MAX);
                let r = routing.receiver[c];
                if r != none {
                    let (dx, dy) = ((r as usize % w) as i64 - (c % w) as i64, (r as usize / w) as i64 - (c / w) as i64);
                    steps_ch += 1;
                    axis += usize::from(dx == 0 || dy == 0);
                }
            }
            println!(
                "EROSION N {label_n}: basins (>= 0.5 m, >= 16 cells, fill eps 1e-7) on {:.2} % of the river cells inside the tiles ({in_big} of {n_ch}); 5 cm count {:.2} %; {} such basins in the window; channel steps along a grid axis {:.2} % ({axis} of {steps_ch})",
                100.0 * in_big as f64 / n_ch.max(1) as f64,
                100.0 * in_5cm as f64 / n_ch.max(1) as f64,
                big.iter().filter(|&&b| b).count(),
                100.0 * axis as f64 / steps_ch.max(1) as f64
            );
        }
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
            let ground = |c: usize| if lake(c) { zr[c] } else { zr[c].min(level[c]) };
            let water = |c: usize| if lake(c) { filled[c] } else { ground(c) };
            let mut cuts: Vec<f64> = Vec::new();
            let mut burns: Vec<f64> = Vec::new();
            let (mut steps, mut rises, mut water_rises, mut lake_cells, mut pit_cells) = (0usize, 0usize, 0usize, 0usize, 0usize);
            for c in 0..w * h {
                if !channel[c] || !inside(c) {
                    continue;
                }
                cuts.push(z0[c] - level[c]);
                burns.push(zr[c] - level[c]);
                lake_cells += usize::from(lake(c));
                pit_cells += usize::from(label[c] != u32::MAX);
                let r = routing.receiver[c];
                if r != none && channel[r as usize] && zr[r as usize] > 0.0 {
                    steps += 1;
                    rises += usize::from(ground(r as usize) - ground(c) > 0.25);
                    water_rises += usize::from(water(r as usize) - water(c) > 0.25);
                }
            }
            cuts.sort_by(f64::total_cmp);
            burns.sort_by(f64::total_cmp);
            let burn_over = burns.iter().filter(|&&v| v > 50.0).count();
            let bp = |f: f64| burns[((burns.len() as f64 * f) as usize).min(burns.len().saturating_sub(1))];
            println!(
                "EROSION N {label_n} D {d_max} m: the burn under the eroded surface (the canyon): p50 {:.2} p99 {:.2} max {:.2} m, {burn_over} over 50 m ({:.2} %)",
                bp(0.5),
                bp(0.99),
                burns.last().copied().unwrap_or(0.0),
                100.0 * burn_over as f64 / burns.len().max(1) as f64
            );
            let p = |f: f64| cuts[((cuts.len() as f64 * f) as usize).min(cuts.len().saturating_sub(1))];
            let over = cuts.iter().filter(|&&v| v > 50.0).count();
            println!(
                "EROSION N {label_n} D {d_max} m: {} channel cells inside the tiles ({pit_cells} in a pit, {:.1} %; {lake_cells} in lakes, {:.1} %); cut under the start surface p50 {:.2} p95 {:.2} p99 {:.2} max {:.2} m, {over} over 50 m ({:.2} %); ground rises {rises} of {steps}, water rises {water_rises}; {} pits",
                cuts.len(),
                100.0 * pit_cells as f64 / cuts.len().max(1) as f64,
                100.0 * lake_cells as f64 / cuts.len().max(1) as f64,
                p(0.5),
                p(0.95),
                p(0.99),
                cuts.last().copied().unwrap_or(0.0),
                100.0 * over as f64 / cuts.len().max(1) as f64,
                depth_of.len()
            );
        }
        // the edge against H1
        let (ei0, ei1, ej0, ej1) =
            ((spec.region.0 * l) as i64, ((spec.region.1 + 1) * l) as i64, (spec.region.2 * l) as i64, ((spec.region.3 + 1) * l) as i64);
        let in_region = |x: i64, y: i64| x >= ei0 && x < ei1 && y >= ej0 && y < ej1;
        let mut l_cross: Vec<(bool, i64, i64, f64)> = Vec::new();
        for c in 0..w * h {
            let r = routing.receiver[c];
            if !channel[c] || r == none {
                continue;
            }
            let (cx, cy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            let (rx, ry) = (x0 + (r as usize % w) as i64, y0 + (r as usize / w) as i64);
            if in_region(cx, cy) != in_region(rx, ry) {
                l_cross.push((in_region(cx, cy), cx, cy, q[c]));
            }
        }
        let to_local = |c: u32| {
            let c = c as usize;
            ((l * (ri0 + c % rw)) as f64 + (l as f64 - 1.0) / 2.0, (l * (rj0 + c / rw)) as f64 + (l as f64 - 1.0) / 2.0)
        };
        let mut h_cross: Vec<(bool, i64, i64, f64)> = Vec::new();
        front.graph.for_each(|s| {
            let cells = s.cells(&front.solve.grid);
            for k in 1..cells.len() {
                let (p0, p1) = (to_local(cells[k - 1]), to_local(cells[k]));
                if in_region(p0.0 as i64, p0.1 as i64) != in_region(p1.0 as i64, p1.1 as i64) {
                    h_cross.push((
                        in_region(p0.0 as i64, p0.1 as i64),
                        p0.0 as i64,
                        p0.1 as i64,
                        planet_generators::bake::channel_graph::q_value(s.q[k - 1]),
                    ));
                }
            }
        });
        let big: Vec<_> = h_cross.iter().filter(|c| c.3 >= 1.0).collect();
        let matched = big
            .iter()
            .filter(|hc| {
                l_cross
                    .iter()
                    .filter(|c| c.0 == hc.0)
                    .any(|c| ((((c.1 - hc.1).pow(2) + (c.2 - hc.2).pow(2)) as f64).sqrt() * cell_m) <= 2000.0)
            })
            .count();
        println!(
            "EROSION N {label_n}: edge: {matched} of {} H1 crossings (Q >= 1 m3/s) have an L crossing of the same kind within 2 km; relief p50 2 x 2 cells {:.2} m ({:.0} % of the start), 10 x 10 cells {:.2} m ({:.0} %); flood + route {t_route:.0} s",
            big.len(),
            relief(zr, 2),
            100.0 * relief(zr, 2) / r200_0,
            relief(zr, 10),
            100.0 * relief(zr, 10) / r1k_0
        );
        Ok(routing)
    };
    // Rounds at L: the regional solve's kernel (stream-power carve with the breach and the deposition of the round solver) over the region's L window, N = 1, 3, 10 cumulative.
    let mut carve = planet_generators::bake::carve::CarveParams {
        steps: 5,
        reroute_every: 5,
        round_rules: rules,
        cell_metric: false,
        window_cut: true,
        ..planet_generators::bake::carve::CarveParams::default()
    };
    front.solve.precision.apply(&mut carve);
    carve.round_f32 = false;
    // SPIKE-120 item 2: PLANET_ROUND_PART=breach keeps only the round's breach (no incision and no uplift: `k_dt` 0); =carve keeps only the incision (no breach: `breach_max_m` 0).
    match std::env::var("PLANET_ROUND_PART").ok().as_deref() {
        Some("breach") => carve.k_dt = 0.0,
        Some("carve") => carve.breach_max_m = 0.0,
        _ => {}
    }
    dump(&z0, 0);
    if std::env::var("PLANET_ROUNDS_MAX").ok().as_deref() == Some("0") {
        return Ok(());
    }
    let mut z = z0.clone();
    let mut last_routing = None;
    if std::env::var("PLANET_MULTISCALE").is_ok() {
        // Schott et al. 2024 (lanes/digest/2026-11-14-rivers-literature.md): rounds at 407, 204 and 101.8 m, each scale seeded with the change of the coarser one upsampled (bilinear), then the
        // multi-scale breach P_k(T) = T - blur_rk(T - breach(T)) with the radius halving to one cell.
        let per = std::env::var("PLANET_ROUNDS_PER_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(3usize);
        let mut change: Option<(Vec<f64>, usize, usize, usize)> = None;
        for f in [4usize, 2, 1] {
            let ts = t0.elapsed().as_secs_f64();
            let (ws, hs) = (w / f, h / f);
            let at_scale = |i: usize, j: usize| -> (i64, i64) { (x0 + (i * f + f / 2) as i64, y0 + (j * f + f / 2) as i64) };
            let mut zs_start = vec![0f64; ws * hs];
            {
                let rows = hs.div_ceil(threads);
                let regional = &front.regional_field;
                let base = &base;
                std::thread::scope(|sc| {
                    for (k, chunk) in zs_start.chunks_mut(rows * ws).enumerate() {
                        sc.spawn(move || {
                            for (c, v) in chunk.iter_mut().enumerate() {
                                let p = k * rows * ws + c;
                                let (x, y) = at_scale(p % ws, p / ws);
                                let d = dir_of(x, y);
                                *v = base(d) + regional.at(d);
                            }
                        });
                    }
                });
            }
            let mut zs = zs_start.clone();
            if let Some((ch, cw, chh, cf)) = &change {
                for j in 0..hs {
                    for i in 0..ws {
                        let (gx, gy) = ((i * f) as f64 + f as f64 / 2.0, (j * f) as f64 + f as f64 / 2.0);
                        let (u, v) =
                            ((gx / *cf as f64 - 0.5).clamp(0.0, (*cw - 1) as f64), (gy / *cf as f64 - 0.5).clamp(0.0, (*chh - 1) as f64));
                        let (i0, j0) = ((u as usize).min(cw - 2), (v as usize).min(chh - 2));
                        let (fu, fv) = (u - i0 as f64, v - j0 as f64);
                        let a = |ii: usize, jj: usize| ch[jj * cw + ii];
                        zs[j * ws + i] += a(i0, j0) * (1.0 - fu) * (1.0 - fv)
                            + a(i0 + 1, j0) * fu * (1.0 - fv)
                            + a(i0, j0 + 1) * (1.0 - fu) * fv
                            + a(i0 + 1, j0 + 1) * fu * fv;
                    }
                }
            }
            let gs = planet_generators::bake::Grid::new(ws, hs, cell_m * f as f64);
            let outs = gs.outlets(&zs, 0.0);
            carve.steps = 5 * per;
            let _ = planet_generators::bake::round::solve_rounds(&gs, &mut zs, &outs, &carve, per, &threads, front.solve.stream);
            let ch: Vec<f64> = zs.iter().zip(&zs_start).map(|(a, b)| a - b).collect();
            println!(
                "EROSION multi-scale: {per} rounds at {:.0} m ({ws} x {hs}) in {:.0} s",
                cell_m * f as f64,
                t0.elapsed().as_secs_f64() - ts
            );
            if f == 1 {
                z = zs;
            }
            change = Some((ch, ws, hs, f));
        }
        let tb = t0.elapsed().as_secs_f64();
        let breach_only = planet_generators::bake::carve::CarveParams {
            k_dt: 0.0,
            steps: 1,
            reroute_every: 1,
            round_rules: rules,
            ..planet_generators::bake::k7::K7Params::standard(seed, 0.0).pass2.carve
        };
        for r in [8usize, 4, 2, 1] {
            let mut bz = z.clone();
            let _ = planet_generators::bake::carve::carve(&g, &mut bz, &outlet, &breach_only, None, front.solve.stream);
            let mut d: Vec<f64> = z.iter().zip(&bz).map(|(a, b)| a - b).collect();
            if r > 1 {
                let mut tmp = vec![0f64; w * h];
                for y in 0..h {
                    let mut acc = 0.0;
                    for x in 0..w + r {
                        if x < w {
                            acc += d[y * w + x];
                        }
                        if x >= 2 * r + 1 {
                            acc -= d[y * w + x - 2 * r - 1];
                        }
                        if x >= r && x - r < w {
                            tmp[y * w + x - r] = acc / (2 * r + 1) as f64;
                        }
                    }
                }
                for x in 0..w {
                    let mut acc = 0.0;
                    for y in 0..h + r {
                        if y < h {
                            acc += tmp[y * w + x];
                        }
                        if y >= 2 * r + 1 {
                            acc -= tmp[(y - 2 * r - 1) * w + x];
                        }
                        if y >= r && y - r < h {
                            d[(y - r) * w + x] = acc / (2 * r + 1) as f64;
                        }
                    }
                }
            }
            for c in 0..w * h {
                z[c] -= d[c];
            }
        }
        println!("EROSION multi-scale breach (radii 8, 4, 2, 1) in {:.0} s", t0.elapsed().as_secs_f64() - tb);
        last_routing = Some(analyse(&z, 99)?);
    } else {
        let mut done = 0usize;
        for target in [1usize, 3, 10] {
            let tr = t0.elapsed().as_secs_f64();
            let more = target - done;
            carve.steps = 5 * more;
            let report = planet_generators::bake::round::solve_rounds(&g, &mut z, &outlet, &carve, more, &threads, front.solve.stream);
            done = target;
            println!("EROSION N {target}: {more} more rounds at L in {:.0} s ({report:?})", t0.elapsed().as_secs_f64() - tr);
            last_routing = Some(analyse(&z, target)?);
            if std::env::var("PLANET_ROUNDS_MAX").ok().and_then(|v| v.parse::<usize>().ok()).is_some_and(|m| target >= m) {
                break;
            }
        }
    }
    if let Some(zb) = &zb_l {
        let tb = t0.elapsed().as_secs_f64();
        let ob = g.outlets(zb, 0.0);
        let rb = planet_generators::bake::route::route_rules(&g, zb, &ob, 1.0e-3, front.solve.stream, 0.0, None, rules);
        let mut ab = vec![cell_m * cell_m; w * h];
        for &c in rb.order.iter().rev() {
            let c = c as usize;
            let r = rb.receiver[c];
            if r != none {
                ab[r as usize] += ab[c];
            }
        }
        let (mut st, mut ax) = (0usize, 0usize);
        for c in 0..w * h {
            let r = rb.receiver[c];
            if ab[c] < threshold || zb[c] <= 0.0 || !inside(c) || r == none {
                continue;
            }
            let (dx, dy) = ((r as usize % w) as i64 - (c % w) as i64, (r as usize / w) as i64 - (c / w) as i64);
            st += 1;
            ax += usize::from(dx == 0 || dy == 0);
        }
        println!(
            "EROSION baseline B alone: channel steps along a grid axis {:.2} % ({ax} of {st}) ({:.0} s)",
            100.0 * ax as f64 / st.max(1) as f64,
            t0.elapsed().as_secs_f64() - tb
        );
    }
    // The edit sits on the r3a window (lake 30): only there.
    if win.is_none() {
        // Edits: a ridge (+60 m, 16 km E-W, 1.2 km wide, blended 0.6 km) south of lake 30 (36.75 S 162.28 W); the full L route against a re-route of the dirty catchments only.
        let base_routing = last_routing.clone().expect("a routing");
        let (anchor_lat, anchor_lon) = (-36.75f64.to_radians() - (4.0 * 814.0 / 6_371_000.0), -162.28f64.to_radians());
        let anchor = Vec3::from_lat_lon(anchor_lat, anchor_lon);
        let (_, s, t) = planet_core::cube::direction_to_face(&TangentWarp, anchor);
        let (mut ax, mut ay) = (((s + 1.0) * 0.5 * n) as i64 - x0, ((t + 1.0) * 0.5 * n) as i64 - y0);
        // SPIKE-120 item 4: PLANET_EDIT=mainstem moves the ridge onto the main stem of the largest catchment inside the region (the outlet root with the most cells), at the point halfway up the
        // stem (the upstream walk along the largest donor), turned across the river's local direction.
        let mut across_ns = false;
        if std::env::var("PLANET_EDIT").ok().as_deref() == Some("mainstem") {
            let r = &base_routing;
            let mut area = vec![1u64; w * h];
            for &c in r.order.iter().rev() {
                let rc = r.receiver[c as usize];
                if rc != none {
                    area[rc as usize] += area[c as usize];
                }
            }
            let mut best_root = (0u64, usize::MAX);
            for c in 0..w * h {
                if r.receiver[c] == none && area[c] > best_root.0 {
                    best_root = (area[c], c);
                }
            }
            // the donors of each cell: walk up along the largest
            let mut up: Vec<u32> = vec![none; w * h];
            for c in 0..w * h {
                let rc = r.receiver[c];
                if rc != none && (up[rc as usize] == none || area[c] > area[up[rc as usize] as usize]) {
                    up[rc as usize] = c as u32;
                }
            }
            let mut stem = vec![best_root.1];
            while up[*stem.last().unwrap()] != none {
                stem.push(up[*stem.last().unwrap()] as usize);
            }
            let mid = stem[stem.len() / 2];
            let nxt = stem[(stem.len() / 2 + 1).min(stem.len() - 1)];
            let (dx, dy) = ((nxt % w) as i64 - (mid % w) as i64, (nxt / w) as i64 - (mid / w) as i64);
            across_ns = dx.abs() > dy.abs();
            (ax, ay) = ((mid % w) as i64, (mid / w) as i64);
            println!(
                "EROSION edit mainstem: the largest catchment drains {} cells ({:.0} km2) to ({}, {}); its stem is {} cells; the ridge crosses it at ({}, {}), {} cells upstream of the ridge, {}",
                best_root.0,
                best_root.0 as f64 * cell_m * cell_m / 1e6,
                best_root.1 % w,
                best_root.1 / w,
                stem.len(),
                ax,
                ay,
                area[mid],
                if across_ns { "north-south" } else { "east-west" }
            );
        }
        let mut ze = z.clone();
        let mut edited: Vec<usize> = Vec::new();
        let half_len = (8000.0 / cell_m) as i64;
        let half_w = (600.0 / cell_m) as i64;
        let blend = (600.0 / cell_m) as i64;
        for dy in -(half_len + blend)..=(half_len + blend) {
            for dx in -(half_len + blend)..=(half_len + blend) {
                let (x, y) = (ax + dx, ay + dy);
                if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                    continue;
                }
                // the ridge's long axis: east-west, or north-south across a river that runs east-west (SPIKE-120 item 4)
                let (lx, ly) = if across_ns { (dy, dx) } else { (dx, dy) };
                let ex = ((lx.abs() - half_len).max(0) as f64) / blend as f64;
                let ey = ((ly.abs() - half_w).max(0) as f64) / blend as f64;
                let f = (1.0 - ex.max(ey)).clamp(0.0, 1.0);
                if f > 0.0 {
                    let c = y as usize * w + x as usize;
                    ze[c] += 60.0 * f;
                    edited.push(c);
                }
            }
        }
        let tf = t0.elapsed().as_secs_f64();
        let full = planet_generators::bake::route::route_rules(&g, &ze, &outlet, 1.0e-3, front.solve.stream, 0.0, None, rules);
        let t_full = t0.elapsed().as_secs_f64() - tf;
        // Dirty catchments: the outlet roots of the edited cells before and after; the incremental re-route recomputes the cells of those catchments only (here: the routing of their bounding box with the
        // box's border as outlets, then compared with the full re-route inside the catchments).
        let root_of = |r: &planet_generators::bake::route::Routing| -> Vec<u32> {
            let mut root = vec![u32::MAX; w * h];
            for &c in &r.order {
                let c = c as usize;
                let rc = r.receiver[c];
                root[c] = if rc == none { c as u32 } else { root[rc as usize] };
            }
            root
        };
        let td = t0.elapsed().as_secs_f64();
        let (root_a, root_b) = (root_of(&base_routing), root_of(&full));
        println!("EROSION edit discovery: the roots before and after in {:.2} s, after the full re-route of {t_full:.1} s it needs", t0.elapsed().as_secs_f64() - td);
        let mut dirty_roots: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for &c in &edited {
            dirty_roots.insert(root_a[c]);
            dirty_roots.insert(root_b[c]);
        }
        let dirty: Vec<usize> = (0..w * h).filter(|&c| dirty_roots.contains(&root_a[c]) || dirty_roots.contains(&root_b[c])).collect();
        let (mut bx0, mut bx1, mut by0, mut by1) = (usize::MAX, 0usize, usize::MAX, 0usize);
        for &c in &dirty {
            (bx0, bx1, by0, by1) = (bx0.min(c % w), bx1.max(c % w), by0.min(c / w), by1.max(c / w));
        }
        let (bw, bh) = (bx1 - bx0 + 1, by1 - by0 + 1);
        {
            // SPIKE-117: the edit path's first part, the L round re-run over the dirty catchments' box (the start surface plus the ridge; the box's border and the sea as outlets), against the full round.
            let tr = t0.elapsed().as_secs_f64();
            let mut sub: Vec<f64> = (0..bw * bh)
                .map(|k| {
                    z0[(by0 + k / bw) * w + bx0 + k % bw] + (ze[(by0 + k / bw) * w + bx0 + k % bw] - z[(by0 + k / bw) * w + bx0 + k % bw])
                })
                .collect();
            let gs = planet_generators::bake::Grid::new(bw, bh, cell_m);
            let os = gs.outlets(&sub, 0.0);
            carve.steps = 5;
            let _ = planet_generators::bake::round::solve_rounds(&gs, &mut sub, &os, &carve, 1, &threads, front.solve.stream);
            println!(
                "EROSION edit: the L round over the dirty catchments' box ({bw} x {bh} cells) in {:.2} s",
                t0.elapsed().as_secs_f64() - tr
            );
        }

        let ti = t0.elapsed().as_secs_f64();
        let sub: Vec<f64> = (0..bw * bh).map(|k| ze[(by0 + k / bw) * w + bx0 + k % bw]).collect();
        let gs = planet_generators::bake::Grid::new(bw, bh, cell_m);
        let mut sub_outlet = gs.outlets(&sub, 0.0);
        // cells of the box outside the dirty catchments drain as before: they are outlets of the sub-problem
        for k in 0..bw * bh {
            let c = (by0 + k / bw) * w + bx0 + k % bw;
            if !(dirty_roots.contains(&root_a[c]) || dirty_roots.contains(&root_b[c])) {
                sub_outlet[k] = true;
            }
        }
        let part = planet_generators::bake::route::route_rules(&gs, &sub, &sub_outlet, 1.0e-3, front.solve.stream, 0.0, None, rules);
        let t_inc = t0.elapsed().as_secs_f64() - ti;
        let mut same = 0usize;
        let mut differ = 0usize;
        for &c in &dirty {
            let (x, y) = (c % w - bx0, c / w - by0);
            let k = y * bw + x;
            let rp = part.receiver[k];
            let mapped = if rp == none { none } else { ((by0 + rp as usize / bw) * w + bx0 + rp as usize % bw) as u32 };
            if mapped == full.receiver[c] || (rp == none && sub_outlet[k]) {
                same += 1;
            } else {
                differ += 1;
            }
        }
        println!(
        "EROSION edit: ridge +60 m at L cell ({}, {}), {} cells edited; dirty catchments {} with {} cells ({:.1} % of the window), their box {} x {}; full L route {t_full:.1} s, re-route of the dirty catchments {t_inc:.1} s; receivers inside them equal to the full re-route: {same}, different: {differ}",
        ax + x0,
        ay + y0,
        edited.len(),
        dirty_roots.len(),
        dirty.len(),
        100.0 * dirty.len() as f64 / (w * h) as f64,
        bw,
        bh
    );
        {
            // Barnes 2016's tile scheme, its local part: each tile that holds an edited cell flooded alone (its border as outlets); the perimeter exchange and the global spillover graph are small (a lower bound).
            let tb = t0.elapsed().as_secs_f64();
            let t = spec.tile;
            let mut tiles: std::collections::BTreeSet<(usize, usize)> = std::collections::BTreeSet::new();
            for &c in &edited {
                tiles.insert((((x0 as usize) + c % w) / t, ((y0 as usize) + c / w) / t));
            }
            for &(tx, ty) in &tiles {
                let (lx, ly) = ((tx * t) as i64 - x0, (ty * t) as i64 - y0);
                if lx < 0 || ly < 0 || lx as usize + t > w || ly as usize + t > h {
                    continue;
                }
                let sub: Vec<f64> = (0..t * t).map(|k| ze[(ly as usize + k / t) * w + lx as usize + k % t]).collect();
                let gt = planet_generators::bake::Grid::new(t, t, cell_m);
                let o = gt.outlets(&sub, 0.0);
                let _ = planet_generators::bake::flood::priority_flood_with(&gt, &sub, &o, 1.0e-3, false);
            }
            println!(
                "EROSION edit, Barnes 2016 local part: {} tiles of {t} cells flooded alone in {:.2} s (plus the perimeter graph)",
                tiles.len(),
                t0.elapsed().as_secs_f64() - tb
            );
        }
    }
    let _ = &last_routing;
    println!("EROSION total {:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}

#[derive(PartialEq, PartialOrd)]
struct Key(f64);
impl Eq for Key {}
impl Ord for Key {
    fn cmp(&self, o: &Key) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

const NB4: [(i64, i64); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// SPIKE-116's breach over a chosen neighbourhood (copied from spikes' spike/drain-base): every cell drains through an edge neighbour with NB4; land is never lowered below `floor`.
fn breach_nb(w: usize, h: usize, z: &mut [f64], outlet: &[bool], eps: f64, nbs: &[(i64, i64)], floor: f64) -> Vec<f64> {
    let none = planet_generators::bake::route::NONE;
    let z0 = z.to_vec();
    let n = w * h;
    let mut parent = vec![none; n];
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    for c in 0..n {
        if outlet[c] {
            done[c] = true;
            heap.push(Reverse((Key(z[c]), c)));
        }
    }
    while let Some(Reverse((_, c))) = heap.pop() {
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        for &(dx, dy) in nbs {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let q = ny as usize * w + nx as usize;
            if done[q] {
                continue;
            }
            done[q] = true;
            parent[q] = c as u32;
            if z[q] <= z[c] {
                let mut lvl = z[q] - eps;
                let mut a = c;
                loop {
                    if outlet[a] || z[a] < lvl || (z0[a] > floor && z[a] <= floor) {
                        break;
                    }
                    z[a] = if z0[a] > floor { lvl.max(floor) } else { lvl };
                    lvl -= eps;
                    if parent[a] == none {
                        break;
                    }
                    a = parent[a] as usize;
                }
            }
            heap.push(Reverse((Key(z[q]), q)));
        }
    }
    z.iter().zip(&z0).map(|(a, b)| b - a).collect()
}

/// Chamfer distance (metres) to the marked cells (copied from spikes' spike/drain-base).
fn chamfer(w: usize, h: usize, mark: &[bool], cell_m: f64) -> Vec<f64> {
    let (a, b) = (cell_m, cell_m * std::f64::consts::SQRT_2);
    let mut d: Vec<f64> = mark.iter().map(|&m| if m { 0.0 } else { f64::INFINITY }).collect();
    for y in 0..h {
        for x in 0..w {
            let c = y * w + x;
            let mut v = d[c];
            if x > 0 {
                v = v.min(d[c - 1] + a);
            }
            if y > 0 {
                v = v.min(d[c - w] + a);
                if x > 0 {
                    v = v.min(d[c - w - 1] + b);
                }
                if x + 1 < w {
                    v = v.min(d[c - w + 1] + b);
                }
            }
            d[c] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let c = y * w + x;
            let mut v = d[c];
            if x + 1 < w {
                v = v.min(d[c + 1] + a);
            }
            if y + 1 < h {
                v = v.min(d[c + w] + a);
                if x + 1 < w {
                    v = v.min(d[c + w + 1] + b);
                }
                if x > 0 {
                    v = v.min(d[c + w - 1] + b);
                }
            }
            d[c] = v;
        }
    }
    d
}
