//! Read-only check (#125): do the lines K7 burns (the bake's in-memory feed) and the lines the commit stores (the channel graph blocks, the viewer's feed) agree, and does K7's own surface rise along them?
//! Builds the R3a front exactly as `bake_region` does, runs K7 on one tile and compares, over the tile's core:
//! - the line cells of the in-memory channels against those of the stored graph's channels (cells in one set only);
//! - the rises over 0.25 m of K7's `z` along each set of lines;
//! - K7's `z` against the stored L (`B + L` at the cell, quantised) at the line cells.
//!
//! Usage: `k7_line_check <commit dir> <ti,tj,x,y>... [--threads T]` (a tile and a face cell near a line in it) (R3a of face 1, seed 14, margin 200: the defaults of `bake_region`).
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::graph_feed::ChannelGraphFeed;
use planet_cache::l_writer::{ChannelFeed, Commit};
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::k7::{k7_tile, line_cells, Channel, K7Params};
use planet_generators::bake::tile::{TileGrid, TileId};
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
    let spots: Vec<(u32, u32, i64, i64)> = args[1..].iter().take_while(|a| !a.starts_with("--")).map(|s| { let v: Vec<i64> = s.split(',').map(|x| x.parse().expect("ti,tj,x,y")).collect(); (v[0] as u32, v[1] as u32, v[2], v[3]) }).collect();
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    let (seed, face, margin) = (14u64, Face(1), 200usize);
    let (i0, j0, size) = (2014 * 4, 190 * 4, (2168 * 4 + 3) - 2014 * 4 + 1);
    let def = WorldDefinition::single_planet(R, seed);
    let world = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
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
    let regional = |d: Vec3| front.regional_field.at(d);
    let grid = TileGrid { cells: spec.local(), tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m };
    let commit = Commit::read(dir)?.ok_or("no commit record")?;
    println!("{}", front.bed_cut.line());
    {
        // The depth inside the commit's tiles (the graph's margin blocks lie over ground the commit did not carve).
        let (ri0, rj0, rw) = (front.solve.ri0, front.solve.rj0, front.solve.rw);
        let l = planet_cache::region_bake::LOCAL_PER_REGIONAL;
        let to_local = |c: u32| {
            let c = c as usize;
            ((l * (ri0 + c % rw)) as f64 + (l as f64 - 1.0) / 2.0, (l * (rj0 + c / rw)) as f64 + (l as f64 - 1.0) / 2.0)
        };
        let n_local = spec.local() as f64;
        let start = |x: i64, y: i64| {
            let d = planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n_local - 1.0, 2.0 * (y as f64 + 0.5) / n_local - 1.0);
            base(d) + front.regional_field.at(d)
        };
        let t = spec.tile as i64;
        let tiles = commit.tiles.clone();
        let keep = |x: i64, y: i64| tiles.contains_key(&(face.0, (x / t) as u32, (y / t) as u32));
        let base_at = |x: i64, y: i64| base(planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n_local - 1.0, 2.0 * (y as f64 + 0.5) / n_local - 1.0));
        let placement = planet_generators::bake::placement::Placement::new(&base_at);
        let inside = planet_cache::region_bake::BedCut::of(&front.graph, &front.solve.grid, &to_local, &start, &keep, if std::env::var("K7_STRAIGHT").is_ok() { None } else { Some(&placement) });
        println!("inside the commit's tiles: {}", inside.line());
        for (name, (x, y)) in [("deepest", inside.deepest), ("p95", inside.p95_cell)] {
            let d = planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n_local - 1.0, 2.0 * (y as f64 + 0.5) / n_local - 1.0);
            let (la, lo) = d.lat_lon();
            println!("{name} cut at local cell ({x}, {y}): lat {:.5} lon {:.5}, start {:.1} m", la.to_degrees(), lo.to_degrees(), start(x, y));
        }
    }
    let store = Arc::new(TileStore::open(dir, world.params()).map_err(|e| format!("{e}"))?);
    let feed = ChannelGraphFeed::new(store, commit)?;
    let src: Arc<dyn planet_generators::erosion_layer::LayerSource> = Arc::new(planet_cache::l_layer::CommitLayer::open(dir, world.clone(), 16)?);
    let layer = planet_generators::erosion_layer::ErosionLayer::new(src)?;
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
    // `--profile name,x,y`: the line through the local cell (x, y) (the in-memory channel with the largest discharge there): the start surface, the burn level and the stored ground along it (60 cells
    // each way) and across it (1 km each way, 20 m steps), as CSV under target/review/k7-beds/, and the heading of the line there.
    let profiles: Vec<(String, i64, i64)> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--profile")
        .map(|(k, _)| {
            let v: Vec<&str> = args[k + 1].split(',').collect();
            (v[0].to_string(), v[1].parse().expect("x"), v[2].parse().expect("y"))
        })
        .collect();
    for (name, px, py) in &profiles {
        let (px, py) = (*px, *py);
        let dirf = |x: f64, y: f64| planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x + 0.5) / n - 1.0, 2.0 * (y + 0.5) / n - 1.0);
        let start_at = |x: f64, y: f64| {
            let d = dirf(x, y);
            base(d) + regional(d)
        };
        let ground_at = |x: f64, y: f64| -> Result<f64, String> {
            let d = dirf(x, y);
            Ok(base(d) + layer.change_at(d)?)
        };
        let near = front.feed.channels_near(face, px - 4, py - 4, 9);
        let mut best: Option<(f64, Vec<((i64, i64), f64)>)> = None;
        for ch in &near {
            // the line cells with the burn level interpolated between the points
            let mut cells: Vec<((i64, i64), f64)> = Vec::new();
            for k in 0..ch.points.len() {
                let p0 = if k == 0 { ch.points[0] } else { ch.points[k - 1] };
                let (l0, l1) = (ch.levels[if k == 0 { 0 } else { k - 1 }], ch.levels[k]);
                let line = line_cells(&Channel { points: vec![p0, ch.points[k]], levels: vec![l0, l1], q: Vec::new(), q_unit: ch.q_unit });
                let m = line.len();
                for (i, c) in line.into_iter().enumerate() {
                    let lv = if m > 1 { l0 + (l1 - l0) * i as f64 / (m - 1) as f64 } else { l1 };
                    match cells.last_mut() {
                        Some(last) if last.0 == c => last.1 = last.1.min(lv),
                        _ => cells.push((c, lv)),
                    }
                }
            }
            if cells.iter().any(|(c, _)| *c == (px, py)) {
                let q = ch.q.iter().copied().fold(0.0, f64::max);
                if best.as_ref().is_none_or(|b| q > b.0) {
                    best = Some((q, cells));
                }
            }
        }
        let Some((q, cells)) = best else {
            println!("profile {name}: no line through ({px}, {py})");
            continue;
        };
        let k = cells.iter().position(|(c, _)| *c == (px, py)).expect("the cell");
        let mut f = String::from("step,offset_m,x,y,start_m,burn_level_m,stored_ground_m
");
        for j in k.saturating_sub(60)..(k + 61).min(cells.len()) {
            let ((x, y), lv) = cells[j];
            f += &format!("{},{:.0},{x},{y},{:.2},{:.2},{:.2}
", j as i64 - k as i64, (j as f64 - k as f64) * 101.8, start_at(x as f64, y as f64), lv, ground_at(x as f64, y as f64)?);
        }
        std::fs::write(format!("target/review/k7-beds/profile-{name}-along.csv"), f).map_err(|e| e.to_string())?;
        let (a, b) = (cells[k.saturating_sub(5)].0, cells[(k + 5).min(cells.len() - 1)].0);
        let (tx, ty) = ((b.0 - a.0) as f64, (b.1 - a.1) as f64);
        let len = (tx * tx + ty * ty).sqrt().max(1e-9);
        let (nx, ny) = (-ty / len, tx / len);
        let mut g = String::from("offset_m,start_m,stored_ground_m
");
        let cell_m = 101.8;
        for m in -50..=50 {
            let o = f64::from(m) * 20.0 / cell_m;
            let (x, y) = (px as f64 + nx * o, py as f64 + ny * o);
            g += &format!("{},{:.2},{:.2}
", m * 20, start_at(x, y), ground_at(x, y)?);
        }
        std::fs::write(format!("target/review/k7-beds/profile-{name}-across.csv"), g).map_err(|e| e.to_string())?;
        // the heading of the line (downstream), degrees from north clockwise
        let (d0, d1) = (dirf(a.0 as f64, a.1 as f64), dirf(b.0 as f64, b.1 as f64));
        let up = dirf(px as f64, py as f64);
        let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
        let north = up.cross(east);
        let v = d1 - d0;
        let heading = libm::atan2(v.dot(east), v.dot(north)).to_degrees().rem_euclid(360.0);
        let (la, lo) = up.lat_lon();
        println!(
            "profile {name}: ({px}, {py}) lat {:.5} lon {:.5}, line max q {q:.1} m3/s, heading downstream {heading:.1} deg; start {:.1} m, burn level {:.1} m, stored ground {:.1} m",
            la.to_degrees(),
            lo.to_degrees(),
            start_at(px as f64, py as f64),
            cells[k].1,
            ground_at(px as f64, py as f64)?
        );
    }
    for &(ti, tj, sx, sy) in &spots {
    let id = TileId { face, ti, tj };
    let (t, h, w) = (spec.tile as i64, spec.halo as i64, grid.window() as i64);
    let (wi0, wj0) = (i64::from(ti) * t - h, i64::from(tj) * t - h);
    let mem: Vec<Channel> = front.feed.channels_near(face, wi0, wj0, w);
    let r = k7_tile(&grid, id, &base, &regional, &mem, &K7Params::standard(seed, 0.0), threads);
    // The pits of the start surface `B + regional` on the tile's window (flood with the window's border and the sea as outlets: window-local, a halo of 64 cells), and the line cells in them.
    {
        let field = |d: Vec3| base(d) + regional(d);
        let z0 = planet_generators::bake::tile::sample_window_coarse(&grid, id, &field, threads, 1);
        let wn = grid.window();
        let g = planet_generators::bake::Grid::new(wn, wn, grid.dx());
        let outlet = g.outlets(&z0, 0.0);
        let filled = planet_generators::bake::flood::priority_flood_with(&g, &z0, &outlet, 1.0e-3, true);
        let depth: Vec<f64> = filled.iter().zip(&z0).map(|(f, z)| f - z).collect();
        // label the pits: 8-connected cells with a depth over 0.01 m
        let mut label = vec![u32::MAX; wn * wn];
        let mut pits: Vec<(f64, usize)> = Vec::new();
        for start in 0..wn * wn {
            if depth[start] <= 0.05 || label[start] != u32::MAX {
                continue;
            }
            let id_l = pits.len() as u32;
            let (mut maxd, mut area) = (0.0f64, 0usize);
            let mut stack = vec![start];
            label[start] = id_l;
            while let Some(c) = stack.pop() {
                maxd = maxd.max(depth[c]);
                area += 1;
                let (cx, cy) = ((c % wn) as i64, (c / wn) as i64);
                for dy in -1..=1i64 {
                    for dx in -1..=1i64 {
                        let (nx, ny) = (cx + dx, cy + dy);
                        if nx < 0 || ny < 0 || nx >= wn as i64 || ny >= wn as i64 {
                            continue;
                        }
                        let q = ny as usize * wn + nx as usize;
                        if depth[q] > 0.05 && label[q] == u32::MAX {
                            label[q] = id_l;
                            stack.push(q);
                        }
                    }
                }
            }
            pits.push((maxd, area));
        }
        let (h, t) = (spec.halo as i64, spec.tile as i64);
        let (wi0, wj0) = (i64::from(ti) * t - h, i64::from(tj) * t - h);
        let straight: Vec<Channel> = {
            let (ri0, rj0, rw) = (front.solve.ri0, front.solve.rj0, front.solve.rw);
            let l = planet_cache::region_bake::LOCAL_PER_REGIONAL;
            let to_local = |c: u32| {
                let c = c as usize;
                ((l * (ri0 + c % rw)) as f64 + (l as f64 - 1.0) / 2.0, (l * (rj0 + c / rw)) as f64 + (l as f64 - 1.0) / 2.0)
            };
            planet_generators::bake::channel_graph::channels_from_graph(&front.graph, &front.solve.grid, &to_local)
                .into_iter()
                .filter(|c| c.points.iter().any(|&(x, y)| x >= (wi0 - 16) as f64 && y >= (wj0 - 16) as f64 && x <= (wi0 + wn as i64 + 16) as f64 && y <= (wj0 + wn as i64 + 16) as f64))
                .collect()
        };
        for (name, chs) in [("straight", &straight), ("placed", &mem)] {
            let (mut cells, mut in_pit) = (0usize, 0usize);
            let mut touched: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
            for ch in chs.iter() {
                for (x, y) in line_cells(ch) {
                    if x < i64::from(ti) * t || y < i64::from(tj) * t || x >= (i64::from(ti) + 1) * t || y >= (i64::from(tj) + 1) * t {
                        continue;
                    }
                    let c = ((y - wj0) as usize) * wn + (x - wi0) as usize;
                    if z0[c] <= 0.0 {
                        continue;
                    }
                    cells += 1;
                    if label[c] != u32::MAX {
                        in_pit += 1;
                        touched.insert(label[c]);
                    }
                }
            }
            let mut d: Vec<f64> = touched.iter().map(|&k| pits[k as usize].0).collect();
            let mut a: Vec<usize> = touched.iter().map(|&k| pits[k as usize].1).collect();
            d.sort_by(f64::total_cmp);
            a.sort();
            let pf = |v: &[f64], q: f64| v.get(((v.len() as f64 * q) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
            let pu = |v: &[usize], q: f64| v.get(((v.len() as f64 * q) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0);
            let over = |m: f64| d.iter().filter(|&&v| v > m).count();
            println!(
                "PITS tile {ti}/{tj} {name}: {} line cells on land, {} in a pit ({:.2} %); {} pits touched, depth below spill p50 {:.2} p90 {:.2} max {:.2} m, area p50 {} p90 {} max {} cells; pits deeper than 1/5/20/50 m: {}/{}/{}/{}; all pits of the window {}",
                cells,
                in_pit,
                100.0 * in_pit as f64 / cells.max(1) as f64,
                d.len(),
                pf(&d, 0.5),
                pf(&d, 0.9),
                d.last().copied().unwrap_or(0.0),
                pu(&a, 0.5),
                pu(&a, 0.9),
                a.last().copied().unwrap_or(0),
                over(1.0),
                over(5.0),
                over(20.0),
                over(50.0),
                pits.len()
            );
        }
    }
    println!("{}", r.mark_line());
    let stored: Vec<Channel> = feed.try_channels_near(face, wi0, wj0, w)?;

    let (x0, y0) = (i64::from(ti) * t, i64::from(tj) * t);
    let core = |x: i64, y: i64| x >= x0 && y >= y0 && x < x0 + t && y < y0 + t;
    let z_at = |x: i64, y: i64| r.z[((y - wj0) * w + (x - wi0)) as usize];
    let cells_of = |chs: &[Channel]| -> BTreeSet<(i64, i64)> { chs.iter().flat_map(line_cells).filter(|&(x, y)| core(x, y)).collect() };
    let (a, b) = (cells_of(&mem), cells_of(&stored));
    println!(
        "tile {ti}/{tj}: in-memory channels {}, stored channels {}; core line cells: in-memory {}, stored {}, only in-memory {}, only stored {}",
        mem.len(),
        stored.len(),
        a.len(),
        b.len(),
        a.difference(&b).count(),
        b.difference(&a).count()
    );
    for (name, chs) in [("in-memory", &mem), ("stored", &stored)] {
        let (mut steps, mut rises, mut worst) = (0usize, 0usize, 0.0f64);
        for ch in chs.iter() {
            let line = line_cells(ch);
            for k in 1..line.len() {
                let ((px, py), (x, y)) = (line[k - 1], line[k]);
                if !(core(x, y) && core(px, py)) {
                    continue;
                }
                let (p, v) = (z_at(px, py), z_at(x, y));
                if p <= 0.0 || v <= 0.0 {
                    continue;
                }
                steps += 1;
                if v - p > 0.25 {
                    rises += 1;
                    worst = worst.max(v - p);
                }
            }
        }
        println!("K7 z along the {name} lines: {steps} land steps, {rises} rise over 0.25 m, largest {worst:.2} m");
    }
    // K7's z against the stored L at the stored line cells: B + L as stored.
    let mut d: Vec<f64> = Vec::new();
    for &(x, y) in b.iter().step_by(7) {
        let dir_v = planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
        let l = layer.change_at(dir_v)?;
        let zb = r.b[((y - wj0) * w + (x - wi0)) as usize];
        d.push((zb + l - z_at(x, y)).abs());
    }
    d.sort_by(f64::total_cmp);
    println!(
        "stored B + L against K7 z at {} stored line cells: p50 {:.3} p99 {:.3} max {:.3} m",
        d.len(),
        d.get(d.len() / 2).copied().unwrap_or(0.0),
        d.get(d.len() * 99 / 100).copied().unwrap_or(0.0),
        d.last().copied().unwrap_or(0.0)
    );
    // Crossings without a join: a core cell on the lines of two channels where neither ends at the other's first cell there. Rises: the steps of the in-memory lines that rise, and whether the cell before is such a crossing.
    {
        let mut on: std::collections::BTreeMap<(i64, i64), Vec<(usize, usize, usize)>> = std::collections::BTreeMap::new();
        let lines: Vec<Vec<(i64, i64)>> = mem.iter().map(line_cells).collect();
        for (ci, line) in lines.iter().enumerate() {
            for (k, &c) in line.iter().enumerate() {
                if core(c.0, c.1) {
                    on.entry(c).or_default().push((ci, k, line.len()));
                }
            }
        }
        let is_join = |a: (usize, usize, usize), b: (usize, usize, usize)| (a.1 + 1 == a.2 && b.1 == 0) || (b.1 + 1 == b.2 && a.1 == 0) || (a.1 + 1 == a.2 && b.1 + 1 == b.2) || (a.1 == 0 && b.1 == 0);
        let crossing: BTreeSet<(i64, i64)> = on
            .iter()
            .filter(|(_, v)| v.iter().enumerate().any(|(i, &a)| v[i + 1..].iter().any(|&b| a.0 != b.0 && !is_join(a, b))))
            .map(|(c, _)| *c)
            .collect();
        let shared = on.values().filter(|v| v.len() > 1).count();
        let (mut rises, mut at_cross, mut big, mut big_cross) = (0, 0, 0, 0);
        for line in &lines {
            for k in 1..line.len() {
                let ((px, py), (x, y)) = (line[k - 1], line[k]);
                if !(core(x, y) && core(px, py)) {
                    continue;
                }
                let (p, v) = (z_at(px, py), z_at(x, y));
                if p > 0.0 && v > 0.0 && v - p > 0.25 {
                    rises += 1;
                    let c = crossing.contains(&(px, py)) || crossing.contains(&(x, y));
                    at_cross += usize::from(c);
                    big += usize::from(v - p > 5.0);
                    big_cross += usize::from(c && v - p > 5.0);
                }
            }
        }
        println!("tile {ti}/{tj}: shared core line cells {shared}, crossings without a join {}; rises {rises} of which at a crossing {at_cross}; over 5 m {big} of which at a crossing {big_cross}", crossing.len());
    }
    // The in-memory channels through the spot: K7's start, z and burn flag, and the stored B + L, along the line around the spot.
    let win = |x: i64, y: i64| ((y - wj0) * w + (x - wi0)) as usize;
    for (ci, ch) in mem.iter().enumerate() {
        let line = line_cells(ch);
        let Some(k) = line.iter().position(|&(x, y)| (x - sx).abs() <= 1 && (y - sy).abs() <= 1) else { continue };
        println!("spot ({sx},{sy}) in-memory channel {ci}: {} points, levels {:.1}..{:.1}, line cell {k} of {}", ch.points.len(), ch.levels[0], ch.levels.last().unwrap(), line.len());
        for j in k.saturating_sub(10)..(k + 16).min(line.len()) {
            let (x, y) = line[j];
            if !core(x, y) { continue; }
            let c = win(x, y);
            let start = r.b[c] + regional(dir_of(x, y));
            let stored_g = r.b[c] + layer.change_at(dir_of(x, y))?;
            let lo3 = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| r.z[win(x + dx, y + dy)]).fold(f64::INFINITY, f64::min);
            println!("  {j} ({x},{y}) start {start:.2} k7 {:.2} burnt {} stored {stored_g:.2} min3x3 {lo3:.2}", r.z[c], r.burn_mask[c]);
        }
    }
    }
    Ok(())
}