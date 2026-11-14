//! SPIKE-116 (spike code, never merged): a base whose sub-km relief drains at L. Transforms B's lattice (16,384 nodes a face edge, 611 m) over a region window before the regional solve, then routes
//! the start surface `B + regional` at L (101.8 m) as SPIKE-114 does and reports its metrics on the region's core (no commit needed): river length over closed basins, cut over 50 m with every pit
//! breached, river cells in lakes when pits deeper than 20 m stay; relief at 200 m-1 km against today's B; the lattice change.
//!
//! Usage: `drain_base --face F --region i0,i1,j0,j1 --seed S [--margin 200] [--variant none|fill|breach|rivers] [--threads 8] [--out DIR]`
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io::Write;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
const NONE: u32 = u32::MAX;

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

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

#[derive(PartialEq, PartialOrd)]
struct Key(f64);
impl Eq for Key {}
impl Ord for Key {
    fn cmp(&self, o: &Key) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

const NB: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Complete breaching on a grid (priority flood keyed by height, without raising): a cell reached from a higher or equal cell lowers that cell's path to the outlet below itself, `eps` a step.
/// Returns how much each cell was lowered.
fn breach(w: usize, h: usize, z: &mut [f64], outlet: &[bool], eps: f64) -> Vec<f64> {
    breach_nb(w, h, z, outlet, eps, &NB, f64::NEG_INFINITY)
}

const NB4: [(i64, i64); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// [`breach`] over a chosen neighbourhood (`NB4`: every cell drains through an edge neighbour, so a bilinear surface over the grid has no closed basin).
/// `floor`: land (above it to begin with) is never lowered below it (the sea level plus a few millimetres, as the regional carve does).
fn breach_nb(w: usize, h: usize, z: &mut [f64], outlet: &[bool], eps: f64, nbs: &[(i64, i64)], floor: f64) -> Vec<f64> {
    let z0 = z.to_vec();
    let n = w * h;
    let mut parent = vec![NONE; n];
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
                // Water at q must leave through c: lower c's path below q.
                let mut lvl = z[q] - eps;
                let mut a = c;
                loop {
                    if outlet[a] || z[a] < lvl || (z0[a] > floor && z[a] <= floor) {
                        break;
                    }
                    z[a] = if z0[a] > floor { lvl.max(floor) } else { lvl };
                    lvl -= eps;
                    if parent[a] == NONE {
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

/// Box mean of radius `r` cells (separable, clamped at the edges).
fn box_mean(w: usize, h: usize, v: &[f64], r: i64) -> Vec<f64> {
    let mut t = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut k) = (0.0, 0.0);
            for d in -r..=r {
                let xx = x as i64 + d;
                if xx >= 0 && xx < w as i64 {
                    s += v[y * w + xx as usize];
                    k += 1.0;
                }
            }
            t[y * w + x] = s / k;
        }
    }
    let mut o = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut s, mut k) = (0.0, 0.0);
            for d in -r..=r {
                let yy = y as i64 + d;
                if yy >= 0 && yy < h as i64 {
                    s += t[yy as usize * w + x];
                    k += 1.0;
                }
            }
            o[y * w + x] = s / k;
        }
    }
    o
}

/// D8 steepest-descent receivers and the upstream cell count of a surface that drains (every non-outlet cell has a lower neighbour).
fn d8_area(w: usize, h: usize, z: &[f64], outlet: &[bool], cell_m: f64) -> (Vec<u32>, Vec<f64>) {
    let n = w * h;
    let mut recv = vec![NONE; n];
    for c in 0..n {
        if outlet[c] {
            continue;
        }
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        let mut best = (0.0, NONE);
        for (dx, dy) in NB {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let q = ny as usize * w + nx as usize;
            let s = (z[c] - z[q]) / (cell_m * ((dx * dx + dy * dy) as f64).sqrt());
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        recv[c] = best.1;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| z[b].total_cmp(&z[a]).then(a.cmp(&b)));
    let mut area = vec![cell_m * cell_m; n];
    for &c in &order {
        if recv[c] != NONE {
            let a = area[c];
            area[recv[c] as usize] += a;
        }
    }
    (recv, area)
}

/// Chamfer distance (metres) to the marked cells.
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

fn pct(v: &mut [f64], f: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * f) as usize]
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let face = Face(arg(&args, "--face").ok_or("--face F")?.parse().map_err(|e| format!("{e}"))?);
    let reg: Vec<usize> = arg(&args, "--region").ok_or("--region i0,i1,j0,j1")?.split(',').map(|v| v.parse().unwrap()).collect();
    let seed: u64 = arg(&args, "--seed").ok_or("--seed S")?.parse().map_err(|e| format!("{e}"))?;
    let margin: usize = arg(&args, "--margin").map_or(200, |v| v.parse().unwrap());
    let variant = arg(&args, "--variant").unwrap_or_else(|| "none".into());
    let threads: usize = arg(&args, "--threads").map_or(8, |v| v.parse().unwrap());
    let out = std::path::PathBuf::from(arg(&args, "--out").unwrap_or_else(|| "target/spike116".into()));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let tag = format!("f{}-{}-{}-{}", face.0, reg[0], reg[2], variant);
    let t0 = std::time::Instant::now();
    let def = WorldDefinition::single_planet(R, seed);
    let _world = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
    let inputs = class_inputs(&def, seed, threads);
    let region = (reg[0], reg[1], reg[2], reg[3]);
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |r: usize| r * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f64; nw * nh];
    let rows = nh.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in nodes.chunks_mut(rows * nw).enumerate() {
            let inputs = &inputs;
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * nw + c;
                    *v = f64::from(inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)) as f32);
                }
            });
        }
    });
    let t_nodes = t0.elapsed().as_secs_f64();
    // The lattice transform: every node of the window (the window's edge and the sea are outlets).
    let node_m = R * std::f64::consts::FRAC_PI_2 / LATTICE as f64;
    let original = nodes.clone();
    let lat_outlet: Vec<bool> = (0..nw * nh).map(|c| c % nw == 0 || c / nw == 0 || c % nw == nw - 1 || c / nw == nh - 1 || original[c] <= 0.0).collect();
    let mut notes = String::new();
    match variant.as_str() {
        "none" => {}
        "fill" => {
            let g = planet_generators::bake::Grid::new(nw, nh, node_m);
            nodes = planet_generators::bake::flood::priority_flood_with(&g, &nodes, &lat_outlet, 1.0e-3, false);
        }
        "breach" => {
            let _ = breach(nw, nh, &mut nodes, &lat_outlet, 1.0e-3);
        }
        "lowpass" => {
            // Diagnostic: B without its sub-2 km relief (a box mean of radius r nodes applied twice).
            let r: i64 = arg(&args, "--blur").map_or(2, |v| v.parse().unwrap());
            let m = box_mean(nw, nh, &box_mean(nw, nh, &original, r), r);
            for c in 0..nw * nh {
                if original[c] > 0.0 {
                    nodes[c] = m[c].max(0.5);
                }
            }
            notes = format!("lowpass: radius {r} nodes twice");
        }
        "msbreach" => {
            // Schott et al. 2024, multi-scale breaching: P_k(T) = T - (T - B(T)) * g_k with the kernel radius halving, the last step a plain breach (drainage exact, no one-node trench).
            let radii: Vec<i64> = arg(&args, "--radii").map_or(vec![4, 2], |v| v.split(',').map(|x| x.parse().unwrap()).collect());
            for &r in &radii {
                let mut b = nodes.clone();
                let _ = breach(nw, nh, &mut b, &lat_outlet, 1.0e-3);
                let diff: Vec<f64> = nodes.iter().zip(&b).map(|(t, bt)| t - bt).collect();
                // A box mean applied twice stands in for the Gaussian of radius r.
                let g = box_mean(nw, nh, &box_mean(nw, nh, &diff, r), r);
                for c in 0..nw * nh {
                    if original[c] > 0.0 {
                        nodes[c] -= g[c];
                    }
                }
            }
            let _ = breach(nw, nh, &mut nodes, &lat_outlet, 1.0e-3);
            notes = format!("msbreach: radii {radii:?} nodes, then a plain breach");
        }
        "hybrid" => {
            // Lindsay 2016, constrained breaching: breach at most D metres a node, fill the rest (the filled pits are flats that drain at L).
            let dmax: f64 = arg(&args, "--dmax").map_or(20.0, |v| v.parse().unwrap());
            let mut b = nodes.clone();
            let _ = breach(nw, nh, &mut b, &lat_outlet, 1.0e-3);
            for c in 0..nw * nh {
                nodes[c] = b[c].max(nodes[c] - dmax);
            }
            let g = planet_generators::bake::Grid::new(nw, nh, node_m);
            nodes = planet_generators::bake::flood::priority_flood_with(&g, &nodes, &lat_outlet, 1.0e-3, false);
            notes = format!("hybrid: breach at most {dmax} m, then fill");
        }
        "rivers" => {
            // (a) rivers first: the network of a low-passed B (radius 3 nodes, about 4.3 km box), breached; valleys along it, the relief damped toward it, the sub-km relief of B kept above the floor.
            let blur_r: i64 = arg(&args, "--blur").map_or(3, |v| v.parse().unwrap());
            let a0_km2: f64 = arg(&args, "--a0").map_or(16.0, |v| v.parse().unwrap());
            let hill_m: f64 = arg(&args, "--hill").map_or(1200.0, |v| v.parse().unwrap());
            let e = box_mean(nw, nh, &original, blur_r);
            let mut eb = e.clone();
            let _ = breach(nw, nh, &mut eb, &lat_outlet, 1.0e-3);
            let (_recv, area) = d8_area(nw, nh, &eb, &lat_outlet, node_m);
            let chan: Vec<bool> = (0..nw * nh).map(|c| area[c] >= a0_km2 * 1e6 && original[c] > 0.0).collect();
            let dist = chamfer(nw, nh, &chan, node_m);
            let r0: Vec<f64> = original.iter().zip(&e).map(|(b, m)| b - m).collect();
            let r2: Vec<f64> = r0.iter().map(|v| v * v).collect();
            let (m1, m2) = (box_mean(nw, nh, &r0, 3), box_mean(nw, nh, &r2, 3));
            for c in 0..nw * nh {
                if original[c] <= 0.0 {
                    continue;
                }
                let amp = 2.0 * (m2[c] - m1[c] * m1[c]).max(0.0).sqrt();
                let u = (dist[c] / hill_m).min(1.0);
                let s = u * u * (3.0 - 2.0 * u);
                // The floor at the breached low-pass, the rise toward the divides, B's own sub-km relief on the rise.
                nodes[c] = (eb[c] + amp * (s - 0.5) + r0[c] * s).max(eb[c].min(original[c]));
            }
            let _ = breach(nw, nh, &mut nodes, &lat_outlet, 1.0e-3);
            notes = format!(
                "rivers: blur radius {blur_r} nodes, channels at {a0_km2} km2 ({:.1} % of the window's nodes), hillslope {hill_m} m",
                100.0 * chan.iter().filter(|&&b| b).count() as f64 / (nw * nh) as f64
            );
        }
        other => return Err(format!("unknown variant {other}")),
    }
    // The lattice change.
    let mut dl: Vec<f64> = (0..nw * nh).filter(|&c| original[c] > 0.0).map(|c| nodes[c] - original[c]).collect();
    let changed = dl.iter().filter(|v| v.abs() > 0.01).count();
    let over50 = dl.iter().filter(|v| **v < -50.0).count();
    let land_nodes = dl.len();
    let (mut down, mut up): (Vec<f64>, Vec<f64>) = (dl.iter().filter(|v| **v < -0.01).map(|v| -v).collect(), dl.iter().filter(|v| **v > 0.01).copied().collect());
    let lat_line = format!(
        "LATTICE {tag}: {} x {} nodes ({:.0} m); land nodes changed over 1 cm {changed} of {land_nodes} ({:.2} %); lowered p50 {:.2} p95 {:.2} max {:.2} m, over 50 m on {:.2} % of land nodes; raised p50 {:.2} p95 {:.2} max {:.2} m; mean change {:.3} m; {notes}",
        nw,
        nh,
        node_m,
        100.0 * changed as f64 / land_nodes.max(1) as f64,
        pct(&mut down, 0.5),
        pct(&mut down, 0.95),
        down.last().copied().unwrap_or(0.0),
        100.0 * over50 as f64 / land_nodes.max(1) as f64,
        pct(&mut up, 0.5),
        pct(&mut up, 0.95),
        up.last().copied().unwrap_or(0.0),
        dl.iter().sum::<f64>() / land_nodes.max(1) as f64
    );
    let _ = pct(&mut dl, 0.5);
    println!("{lat_line}");
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes.iter().map(|&v| v as f32).collect() };
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
    let mut z0 = sample(&|d: Vec3| base(d) + regional.at(d));
    // PLANET_TRANSFER=d4: the start surface rebuilt from the solved surface breached over 4 neighbours (every regional cell drains through an edge), bilinear at L, plus B's sub-814 m residual
    // damped to 0 toward the channels of that surface over `--hill` m. Replaces `B + regional` for every metric below.
    let transfer = std::env::var("PLANET_TRANSFER").unwrap_or_default();
    let mut transfer_line = String::new();
    if transfer == "d4" {
        let (rw_, rh_) = (front.solve.rw, front.solve.rh);
        let mut zs4 = front.solve.zs.clone();
        let reg_eps: f64 = arg(&args, "--reg-eps").map_or(0.01, |v| v.parse().unwrap());
        let lowered = breach_nb(rw_, rh_, &mut zs4, &front.solve.outlet, reg_eps, &NB4, 0.005);
        let land_reg = front.solve.zs.iter().filter(|v| **v > 0.0).count();
        let mut low: Vec<f64> = lowered.iter().copied().filter(|v| *v > 0.01).collect();
        let over50 = low.iter().filter(|v| **v > 50.0).count();
        let nl = low.len();
        transfer_line = format!(
            "TRANSFER d4: the 814 m breach lowered {nl} of {land_reg} land cells: p50 {:.2} p95 {:.2} max {:.2} m; over 50 m {over50} ({:.2} % of land)",
            pct(&mut low, 0.5),
            pct(&mut low, 0.95),
            low.last().copied().unwrap_or(0.0),
            100.0 * over50 as f64 / land_reg.max(1) as f64
        );
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
        let reg_b: Vec<f64> = (0..rw_ * rh_).map(|c| base(front.solve.face_spec.halo_direction((front.solve.ri0 + c % rw_) as i64, (front.solve.rj0 + c / rw_) as i64))).collect();
        let zbr = bil(&reg_b);
        let zb = sample(&|d: Vec3| base(d));
        let gg = planet_generators::bake::Grid::new(w, h, 101.8);
        let o4 = gg.outlets(&zr4, 0.0);
        let rt4 = planet_generators::bake::route::route_rules(&gg, &zr4, &o4, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
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
        let hill: f64 = arg(&args, "--hill").map_or(400.0, |v| v.parse().unwrap());
        let dist = chamfer(w, h, &chan4, 101.8);
        for c in 0..w * h {
            let u = (dist[c] / hill).min(1.0);
            let sm = u * u * (3.0 - 2.0 * u);
            z0[c] = if zr4[c] > 0.0 { zr4[c] + sm * (zb[c] - zbr[c]) } else { zr4[c] };
        }
        transfer_line += &format!("; residual damped over {hill} m");
        println!("{transfer_line}");
    }
    let tag = if transfer.is_empty() { tag } else { format!("{tag}-t{transfer}") };
    let t_sample = t0.elapsed().as_secs_f64();
    let h1_runoff: Vec<f32> = (0..rw * rh)
        .map(|c| {
            let d = front.solve.face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64);
            if front.solve.zs[c] > 0.0 { climate(d).0 as f32 } else { 0.0 }
        })
        .collect();
    let g = planet_generators::bake::Grid::new(w, h, cell_m);
    let outlet = g.outlets(&z0, 0.0);
    let zr = z0.clone();
    let filled = planet_generators::bake::flood::priority_flood_with(&g, &zr, &outlet, 1.0e-3, false);
    let routing = planet_generators::bake::route::route_rules(&g, &zr, &outlet, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
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
    let t_route = t0.elapsed().as_secs_f64();
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
            for (dx, dy) in NB {
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
        depth_of.push(maxd);
    }
    // The region's core (without the margin).
    let (ci0, ci1, cj0, cj1) = ((region.0 * l) as i64, ((region.1 + 1) * l) as i64, (region.2 * l) as i64, ((region.3 + 1) * l) as i64);
    let inside = |c: usize| {
        let (x, y) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
        x >= ci0 && x < ci1 && y >= cj0 && y < cj1
    };
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(out.join("metrics.txt")).map_err(|e| e.to_string())?;
    writeln!(log, "{lat_line}").unwrap();
    if !transfer_line.is_empty() {
        writeln!(log, "{tag} {transfer_line}").unwrap();
    }
    let channel_inside = (0..w * h).filter(|&c| channel[c] && inside(c)).count();
    let pit_inside = (0..w * h).filter(|&c| channel[c] && inside(c) && label[c] != u32::MAX).count();
    let pits_core = {
        let mut seen = std::collections::HashSet::new();
        (0..w * h).filter(|&c| inside(c) && label[c] != u32::MAX).for_each(|c| {
            seen.insert(label[c]);
        });
        seen.len()
    };
    let l1 = format!(
        "RIVERS {tag}: core {} channel cells; {} on closed basins of the start surface ({:.2} % of river length); pits touching the core {pits_core}; time nodes {:.0} s, front {:.0} s, sample {:.0} s, route {:.0} s",
        channel_inside,
        pit_inside,
        100.0 * pit_inside as f64 / channel_inside.max(1) as f64,
        t_nodes,
        t_front - t_nodes,
        t_sample - t_front,
        t_route - t_sample
    );
    println!("{l1}");
    writeln!(log, "{l1}").unwrap();
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
        let mut lake_cells = 0usize;
        for c in 0..w * h {
            if !channel[c] || !inside(c) {
                continue;
            }
            cuts.push(z0[c] - level[c].min(z0[c]).max(level[c]));
            lake_cells += usize::from(lake(c));
        }
        let over = cuts.iter().filter(|&&v| v > 50.0).count();
        let n_c = cuts.len();
        let l2 = format!(
            "RIVERS {tag} D {d_max} m: cut p50 {:.2} p95 {:.2} p99 {:.2} max {:.2} m, over 50 m {:.2} %; channel cells in lakes {:.2} %",
            pct(&mut cuts, 0.5),
            pct(&mut cuts, 0.95),
            pct(&mut cuts, 0.99),
            cuts.last().copied().unwrap_or(0.0),
            100.0 * over as f64 / n_c.max(1) as f64,
            100.0 * lake_cells as f64 / n_c.max(1) as f64
        );
        println!("{l2}");
        writeln!(log, "{l2}").unwrap();
    }
    // Diagnosis (PLANET_DIAG): the same basin share on B alone and on the solved regional surface alone (bilinear at L), against B + regional above.
    if std::env::var("PLANET_DIAG").is_ok() {
        let share = |z: &[f64], name: &str| {
            let out = g.outlets(z, 0.0);
            let fl = planet_generators::bake::flood::priority_flood_with(&g, z, &out, 1.0e-3, false);
            let rt = planet_generators::bake::route::route_rules(&g, z, &out, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
            let mut a = vec![cell_m * cell_m; w * h];
            for &c in rt.order.iter().rev() {
                let c = c as usize;
                let r = rt.receiver[c];
                if r != none {
                    a[r as usize] += a[c];
                }
            }
            // Strict: a fill with an epsilon of 1e-7 m a cell (a flat is not a basin).
            let fl0 = planet_generators::bake::flood::priority_flood_with(&g, z, &out, 1.0e-7, false);
            let (mut ch, mut pit, mut deep, mut pit0, mut deep0) = (0usize, 0usize, 0usize, 0usize, 0usize);
            for c in 0..w * h {
                if a[c] >= threshold && z[c] > 0.0 && inside(c) {
                    ch += 1;
                    pit += usize::from(fl[c] - z[c] > 0.05);
                    deep += usize::from(fl[c] - z[c] > 5.0);
                    pit0 += usize::from(fl0[c] - z[c] > 0.05);
                    deep0 += usize::from(fl0[c] - z[c] > 5.0);
                }
            }
            let f = |v: usize| 100.0 * v as f64 / ch.max(1) as f64;
            let line = format!("DIAG {tag} {name}: {ch} channel cells in the core, on closed basins {:.2} % (over 5 m deep {:.2} %) [1 mm a cell fill, SPIKE-114]; strict fill {:.2} % (over 5 m {:.2} %)", f(pit), f(deep), f(pit0), f(deep0));
            println!("{line}");
            line
        };
        {
            // The solved surface at its own grid (814 m): channel cells in pits of its own flood.
            let rg = &front.solve.grid;
            let zs = &front.solve.zs;
            let ro = &front.solve.outlet;
            let fl = planet_generators::bake::flood::priority_flood_with(rg, zs, ro, 1.0e-3, false);
            let rt = planet_generators::bake::route::route_rules(rg, zs, ro, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
            let cm = R * std::f64::consts::FRAC_PI_2 / REGIONAL as f64;
            let mut a = vec![cm * cm; rw * rh];
            for &c in rt.order.iter().rev() {
                let c = c as usize;
                let r = rt.receiver[c];
                if r != none {
                    a[r as usize] += a[c];
                }
            }
            let (mut ch, mut pit, mut deep, mut cells_pit) = (0usize, 0usize, 0usize, 0usize);
            for c in 0..rw * rh {
                let (x, y) = ((ri0 + c % rw) * l, (rj0 + c / rw) * l);
                let core = (x as i64) >= ci0 && (x as i64) < ci1 && (y as i64) >= cj0 && (y as i64) < cj1;
                if !core || zs[c] <= 0.0 {
                    continue;
                }
                cells_pit += usize::from(fl[c] - zs[c] > 0.05);
                if a[c] >= threshold {
                    ch += 1;
                    pit += usize::from(fl[c] - zs[c] > 0.05);
                    deep += usize::from(fl[c] - zs[c] > 5.0);
                }
            }
            let line = format!("DIAG {tag} solved surface at 814 m: {ch} channel cells in the core, on closed basins {:.2} % (over 5 m {:.2} %); land cells in pits {cells_pit}", 100.0 * pit as f64 / ch.max(1) as f64, 100.0 * deep as f64 / ch.max(1) as f64);
            println!("{line}");
            writeln!(log, "{line}").unwrap();
        }
        let zb = sample(&|d: Vec3| base(d));
        let l_b = share(&zb, "B alone");
        // The solved regional surface, bilinear between the regional cell centres (local 8i + 3.5).
        let zsv = &front.solve.zs;
        let zreg: Vec<f64> = (0..w * h)
            .map(|c| {
                let (fx, fy) = (((c % w) as f64 - 3.5) / l as f64, ((c / w) as f64 - 3.5) / l as f64);
                let (ix, iy) = ((fx.floor().max(0.0) as usize).min(rw - 2), (fy.floor().max(0.0) as usize).min(rh - 2));
                let (u, v) = ((fx - ix as f64).clamp(0.0, 1.0), (fy - iy as f64).clamp(0.0, 1.0));
                let at = |x: usize, y: usize| zsv[y * rw + x];
                (at(ix, iy) * (1.0 - u) + at(ix + 1, iy) * u) * (1.0 - v) + (at(ix, iy + 1) * (1.0 - u) + at(ix + 1, iy + 1) * u) * v
            })
            .collect();
        let l_r = share(&zreg, "solved regional surface alone (bilinear)");
        let l_c = share(&z0, "B + regional (the start surface)");
        // The transfer test: the solved surface breached over 4 neighbours (every cell drains through an edge), then bilinear at L.
        let mut zs4 = zsv.clone();
        let reg_eps: f64 = arg(&args, "--reg-eps").map_or(0.01, |v| v.parse().unwrap());
        let lowered = breach_nb(rw, rh, &mut zs4, &front.solve.outlet, reg_eps, &NB4, 0.005);
        let mut low: Vec<f64> = lowered.iter().copied().filter(|v| *v > 0.01).collect();
        let n_low = low.len();
        let bil = |zz: &[f64]| -> Vec<f64> {
            (0..w * h)
                .map(|c| {
                    let (fx, fy) = (((c % w) as f64 - 3.5) / l as f64, ((c / w) as f64 - 3.5) / l as f64);
                    let (ix, iy) = ((fx.floor().max(0.0) as usize).min(rw - 2), (fy.floor().max(0.0) as usize).min(rh - 2));
                    let (u, v) = ((fx - ix as f64).clamp(0.0, 1.0), (fy - iy as f64).clamp(0.0, 1.0));
                    let at = |x: usize, y: usize| zz[y * rw + x];
                    (at(ix, iy) * (1.0 - u) + at(ix + 1, iy) * u) * (1.0 - v) + (at(ix, iy + 1) * (1.0 - u) + at(ix + 1, iy + 1) * u) * v
                })
                .collect()
        };
        let zr4 = bil(&zs4);
        let l_4 = share(&zr4, "solved surface breached over 4 neighbours, bilinear");
        let land_reg = zsv.iter().filter(|v| **v > 0.0).count();
        let over50 = low.iter().filter(|v| **v > 50.0).count();
        let l_4b = format!("DIAG {tag} the D4 breach (step {reg_eps} m) lowered {n_low} of {land_reg} land regional cells: p50 {:.2} p95 {:.2} max {:.2} m; over 50 m {over50} ({:.2} % of land)", pct(&mut low, 0.5), pct(&mut low, 0.95), low.last().copied().unwrap_or(0.0), 100.0 * over50 as f64 / land_reg.max(1) as f64);
        println!("{l_4b}");
        // B's sub-814 m residual (B at L minus the bilinear of B at the regional centres), damped to 0 toward the channels of the D4 surface over a hillslope of `--hill` m.
        let hill: f64 = arg(&args, "--hill").map_or(400.0, |v| v.parse().unwrap());
        let zbr: Vec<f64> = {
            let reg_b: Vec<f64> = (0..rw * rh).map(|c| base(front.solve.face_spec.halo_direction((ri0 + c % rw) as i64, (rj0 + c / rw) as i64))).collect();
            bil(&reg_b)
        };
        let out4 = g.outlets(&zr4, 0.0);
        let rt4 = planet_generators::bake::route::route_rules(&g, &zr4, &out4, 1.0e-3, front.solve.stream, 0.0, None, planet_generators::bake::route::RoundRules::default());
        let mut a4 = vec![cell_m * cell_m; w * h];
        for &c in rt4.order.iter().rev() {
            let c = c as usize;
            let r = rt4.receiver[c];
            if r != none {
                a4[r as usize] += a4[c];
            }
        }
        let chan4: Vec<bool> = (0..w * h).map(|c| a4[c] >= threshold && zr4[c] > 0.0).collect();
        drop(a4);
        let dist = chamfer(w, h, &chan4, cell_m);
        let sfin: Vec<f64> = (0..w * h)
            .map(|c| {
                let u = (dist[c] / hill).min(1.0);
                let sm = u * u * (3.0 - 2.0 * u);
                let r = zb[c] - zbr[c];
                if zr4[c] > 0.0 { zr4[c] + sm * r } else { zr4[c] }
            })
            .collect();
        let l_s = share(&sfin, &format!("D4 surface + B's sub-814 m residual damped over {hill} m toward its channels"));
        let (sb1, sb5) = (box_mean(w, h, &sfin, 1), box_mean(w, h, &sfin, 5));
        let mut sband: Vec<f64> = (0..w * h).filter(|&c| inside(c) && sfin[c] > 0.0).map(|c| sb1[c] - sb5[c]).collect();
        let sm_ = sband.iter().sum::<f64>() / sband.len().max(1) as f64;
        let sstd = (sband.iter().map(|v| (v - sm_) * (v - sm_)).sum::<f64>() / sband.len().max(1) as f64).sqrt();
        let _ = pct(&mut sband, 0.5);
        let l_rel = format!("DIAG {tag} relief of that surface: band 200 m-1 km std {sstd:.2} m (the start surface's is printed under RELIEF)");
        println!("{l_rel}");
        writeln!(log, "{l_4}
{l_4b}
{l_s}
{l_rel}").unwrap();
        writeln!(log, "{l_b}
{l_r}
{l_c}").unwrap();
    }
    // Relief at 200 m - 1 km on land in the core: the std of (box mean over 2 cells - box mean over 10 cells), and the range in 1 km blocks (10 x 10 L cells).
    let (b1, b5) = (box_mean(w, h, &z0, 1), box_mean(w, h, &z0, 5));
    let mut band: Vec<f64> = (0..w * h).filter(|&c| inside(c) && z0[c] > 0.0).map(|c| b1[c] - b5[c]).collect();
    let mean = band.iter().sum::<f64>() / band.len().max(1) as f64;
    let std = (band.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / band.len().max(1) as f64).sqrt();
    let mut blocks: Vec<f64> = Vec::new();
    for by in (0..h - 10).step_by(10) {
        for bx in (0..w - 10).step_by(10) {
            let c0 = by * w + bx;
            if !inside(c0) || z0[c0] <= 0.0 {
                continue;
            }
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for yy in 0..10 {
                for xx in 0..10 {
                    let v = z0[(by + yy) * w + bx + xx];
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            blocks.push(hi - lo);
        }
    }
    let l3 = format!(
        "RELIEF {tag}: band 200 m-1 km std {:.2} m (|.| p95 {:.2}); 1 km block relief p50 {:.1} p95 {:.1} m ({} blocks)",
        std,
        pct(&mut band.iter().map(|v| v.abs()).collect::<Vec<_>>(), 0.95),
        pct(&mut blocks, 0.5),
        pct(&mut blocks, 0.95),
        blocks.len()
    );
    let _ = pct(&mut band, 0.5);
    println!("{l3}");
    writeln!(log, "{l3}").unwrap();
    // The start surface of the core for pictures (f32, row-major, every 2nd L cell).
    let (cw, ch) = (((ci1 - ci0) / 2) as usize, ((cj1 - cj0) / 2) as usize);
    let mut bytes = Vec::with_capacity(cw * ch * 4);
    for yy in 0..ch {
        for xx in 0..cw {
            let (x, y) = (ci0 - x0 + 2 * xx as i64, cj0 - y0 + 2 * yy as i64);
            bytes.extend_from_slice(&(z0[y as usize * w + x as usize] as f32).to_le_bytes());
        }
    }
    std::fs::write(out.join(format!("{tag}-{cw}x{ch}.f32")), bytes).map_err(|e| e.to_string())?;
    println!("TOTAL {tag} {:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}
