//! SPIKE-130 (spike branch spike/basins-g1e, never merged): the basins of `S0` at L on a window (the river gate's setup without `--store`: the start surface B plus the regional change), with the G0
//! contracts' kept rule (0.25 km2, 2 m), nesting, areas and the share on H1 trunk lines.
use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::Commit;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{commit_differences, key_note, lay_store_ground, Crossing, GateParams, LWindow, WindowIdentity};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
/// R1h: face 1, regional cells i 8,056 to 8,675, j 760 to 1,379, seed 14 (the E1 region stores' window: r3a-e1, r3a-p2, r3a-p2e).
const R1H: [usize; 6] = [1, 8056, 8675, 760, 1379, 14];

/// The base lattice's nodes over the region (the C1 read's footprint).
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

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).map(|k| args.get(k + 1).map(String::as_str).unwrap_or(""))
}

fn parsed<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    match value(args, name) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|_| format!("{name} takes a number, not '{v}'")),
    }
}

/// Fills `out` (`w` a row) by `f(cell)` over `threads` scoped threads, in rows: the same values on any thread count.
fn par_fill(out: &mut [f64], w: usize, threads: usize, f: &(dyn Fn(usize) -> f64 + Sync)) {
    let rows = (out.len() / w).div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    *v = f(k * rows * w + c);
                }
            });
        }
    });
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_key_check = args.iter().any(|a| a == "--no-key-check");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--no-key-check").collect();
    let known = ["--window", "--margin", "--store", "--min-cells", "--d", "--threads"];
    let mut k = 0;
    while k < args.len() {
        if !known.contains(&args[k].as_str()) {
            return Err(format!("unknown river-gate option '{}' (known: {})", args[k], known.join(", ")));
        }
        k += 2;
    }
    let win: Vec<usize> = match value(&args, "--window") {
        None => R1H.to_vec(),
        Some(v) => v
            .split(',')
            .map(|x| x.parse::<usize>().map_err(|_| format!("--window face,i0,i1,j0,j1,seed, not '{v}'")))
            .collect::<Result<_, _>>()?,
    };
    if win.len() != 6 || win[0] > 5 || win[1] > win[2] || win[3] > win[4] || win[2] >= REGIONAL || win[4] >= REGIONAL {
        return Err(format!("--window face,i0,i1,j0,j1,seed: a face 0 to 5 and an ordered box inside 0..{REGIONAL}, not {win:?}"));
    }
    let margin: usize = parsed(&args, "--margin", 200)?;
    let threads: usize = parsed(&args, "--threads", 8)?;
    let params = GateParams {
        basin_cells: parsed(&args, "--min-cells", GateParams::default().basin_cells)?,
        lake_depth_m: parsed(&args, "--d", 20.0)?,
        ..GateParams::default()
    };
    let store = value(&args, "--store").map(Path::new);
    if no_key_check && store.is_none() {
        return Err("--no-key-check reads a store: it goes with --store".to_string());
    }
    let (face, seed) = (Face(win[0] as u8), win[5] as u64);
    let region = (win[1], win[2], win[3], win[4]);
    // The C1 read meets base nodes 3 below and 4 above the margin's (to_node below): the window keeps 4 regional cells of the face beyond its margin on each side.
    if region.0 < margin + 4 || region.2 < margin + 4 || region.1 + margin + 4 >= REGIONAL || region.3 + margin + 4 >= REGIONAL {
        return Err(format!("the window {region:?} with the margin {margin} reaches within 4 regional cells of the face's edge"));
    }
    let def = WorldDefinition::single_planet(R, seed);
    let inputs = class_inputs(&def, seed, threads);
    // The base lattice's nodes the C1 read meets over the region and its margin.
    let (ri0, ri1, rj0, rj1) = (region.0 - margin, region.1 + margin, region.2 - margin, region.3 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let mut nodes = vec![0f64; nw * nh];
    par_fill(&mut nodes, nw, threads, &|p| inputs.base(lattice_direction(face, LATTICE, ni0 + p % nw, nj0 + p / nw)));
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
    // The L window: the regional solve's window, 8 L cells a regional cell.
    let l = LOCAL_PER_REGIONAL;
    let (sri0, srj0, srw, srh) = (front.solve.ri0, front.solve.rj0, front.solve.rw, front.solve.rh);
    let (x0, y0, w, h) = ((sri0 * l) as i64, (srj0 * l) as i64, srw * l, srh * l);
    let n = spec.local() as f64;
    let dir_of = |x: i64, y: i64| {
        planet_core::cube::face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0)
    };
    let mut start = vec![0f64; w * h];
    par_fill(&mut start, w, threads, &|p| {
        let d = dir_of(x0 + (p % w) as i64, y0 + (p / w) as i64);
        base(d) + front.regional_field.at(d)
    });
    let (bi0, bi1, bj0, bj1) = ((region.0 * l) as i64, ((region.1 + 1) * l) as i64, (region.2 * l) as i64, ((region.3 + 1) * l) as i64);
    let in_box: Vec<bool> = (0..w * h)
        .map(|c| {
            let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
        })
        .collect();
    let mut surface = start.clone();
    let mut inside = in_box.clone();
    let mut label = format!("start surface of the window {win:?}, margin {margin}, counted over the region's box");
    let mut sea_level_m = 0.0;
    if let Some(dir) = store {
        let commit = Commit::read(dir)?.ok_or_else(|| format!("{}: no commit record", dir.display()))?;
        let world = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
        let tiles = TileStore::open(dir, world.params()).map_err(|e| format!("{}: {e}", dir.display()))?;
        // The tiles are placed on this window's L grid and laid on its B: the commit's grid, radius, seed and base must be this window's. Its inputs identity names the window, the rounds and the
        // carve (RegionSpec::effective_inputs_id): checked with the keys, not with --no-key-check (a spike's carve differs by design).
        let cs = &commit.spec;
        let window =
            WindowIdentity { cells: spec.local() as u32, radius_m: R, seed, base_id: spec.base_id, inputs_id: spec.effective_inputs_id() };
        let differ = commit_differences(cs, &window, !no_key_check);
        if !differ.is_empty() {
            return Err(format!(
                "{}: the commit was not baked on the window {win:?} with margin {margin}: {}",
                dir.display(),
                differ.join(", ")
            ));
        }
        sea_level_m = cs.sea_level_m;
        inside = vec![false; w * h];
        let (read, outside) = lay_store_ground(
            &tiles,
            &world,
            &commit,
            !no_key_check,
            LWindow { face, x0, y0, w, h },
            &|x, y| base(dir_of(x, y)),
            threads,
            &mut surface,
            &mut inside,
        )
        .map_err(|e| format!("{}: {e} {win:?}", dir.display()))?;
        label = format!(
            "store {} (commit {:016x}, {read} tiles read, {outside} outside the window; {}): the stored ground B + L over its tiles, which drain to their border, window {win:?}, sea level {sea_level_m} m; the stored change's 0.25 m steps add pits to the 5 cm reference only",
            dir.display(),
            commit.id,
            key_note(!no_key_check)
        );
    }
    // H1's crossings of the region's box, in the L window's cells (an H1 cell's centre).
    let to_local = |c: u32| {
        let c = c as usize;
        (
            (l * (sri0 + c % srw)) as f64 + (l as f64 - 1.0) / 2.0 - x0 as f64,
            (l * (srj0 + c / srw)) as f64 + (l as f64 - 1.0) / 2.0 - y0 as f64,
        )
    };
    let in_region = |p: (f64, f64)| {
        let (gx, gy) = (p.0 as i64 + x0, p.1 as i64 + y0);
        gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
    };
    let mut h1: Vec<Crossing> = Vec::new();
    // H1's trunk reaches (BUILD-057): every segment of the channel graph whose discharge reaches the edge limit, its H1 cell centres in the L window's cells, upstream first.
    let mut trunks: Vec<Vec<(f64, f64)>> = Vec::new();
    front.graph.for_each(|s| {
        let cells = s.cells(&front.solve.grid);
        let q_max = s.q.iter().map(|&q| planet_generators::bake::channel_graph::q_value(q)).fold(0.0, f64::max);
        if cells.len() >= 2 && q_max >= params.edge_q_m3s {
            trunks.push(cells.iter().map(|&c| to_local(c)).collect());
        }
        for k in 1..cells.len() {
            let (p0, p1) = (to_local(cells[k - 1]), to_local(cells[k]));
            if in_region(p0) != in_region(p1) {
                h1.push(Crossing {
                    inside: in_region(p0),
                    x: p0.0,
                    y: p0.1,
                    q_m3s: planet_generators::bake::channel_graph::q_value(s.q[k - 1]),
                });
            }
        }
    });
    let grid = planet_generators::bake::Grid::new(w, h, front.solve.grid.dx() / l as f64);
    let outlet = grid.outlets(&surface, sea_level_m);
    // ---- SPIKE-130: the basins of S0 at L (the start surface: B plus the regional change), G0 contracts section 4; counted over the region's box (floor inside)
    let t0 = std::time::Instant::now();
    let hy = planet_generators::bake::basins_rg::hierarchy(&grid, &start, &outlet);
    let nn = hy.nodes.len();
    let cell_a = grid.dx() * grid.dx();
    // per node: subtree cells and whether it touches an H1 trunk line (the trunk polylines sampled every L cell), children before parents
    let mut on_trunk = vec![false; w * h];
    for line in &trunks {
        for pair in line.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let n = libm::hypot(bx - ax, by - ay).ceil().max(1.0) as usize;
            for s in 0..=n {
                let t = s as f64 / n as f64;
                let (x, y) = ((ax + t * (bx - ax)).round(), (ay + t * (by - ay)).round());
                if x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h {
                    on_trunk[y as usize * w + x as usize] = true;
                }
            }
        }
    }
    let mut cells = vec![0u64; nn];
    let mut trunk = vec![false; nn];
    for k in 0..nn {
        let nd = &hy.nodes[k];
        cells[k] = nd.own.len() as u64 + nd.kids.iter().map(|&c| cells[c as usize]).sum::<u64>();
        trunk[k] = nd.own.iter().any(|&c| on_trunk[c]) || nd.kids.iter().any(|&c| trunk[c as usize]);
    }
    let depth = |k: usize| hy.nodes[k].spill.map_or(f64::INFINITY, |s| s.1) - hy.nodes[k].floor_m;
    let kept = |k: usize| depth(k) >= 2.0 && cells[k] as f64 * cell_a >= 0.25e6;
    let reported = |k: usize| !kept(k) && depth(k) >= 0.5 && cells[k] >= 16;
    let inside = |k: usize| in_box[hy.nodes[k].floor_cell];
    let sel: Vec<usize> = (0..nn).filter(|&k| inside(k) && kept(k)).collect();
    let nested = sel.iter().filter(|&&k| {
        let mut a = hy.nodes[k].parent;
        while a != planet_generators::bake::basins_rg::DRAINED {
            if kept(a as usize) {
                return true;
            }
            a = hy.nodes[a as usize].parent;
        }
        false
    });
    let n_nested = nested.count();
    // the outermost kept basins: their areas do not double-count nested ones
    let outer: Vec<usize> = sel
        .iter()
        .copied()
        .filter(|&k| {
            let mut a = hy.nodes[k].parent;
            while a != planet_generators::bake::basins_rg::DRAINED {
                if kept(a as usize) {
                    return false;
                }
                a = hy.nodes[a as usize].parent;
            }
            true
        })
        .collect();
    let km2 = |k: usize| cells[k] as f64 * cell_a / 1e6;
    let mut areas: Vec<f64> = outer.iter().map(|&k| km2(k)).collect();
    areas.sort_by(f64::total_cmp);
    let mut depths: Vec<f64> = outer.iter().map(|&k| depth(k)).collect();
    depths.sort_by(f64::total_cmp);
    let at = |v: &[f64], f: f64| v.get(((v.len() as f64 * f) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(0.0);
    let land_km2 = (0..w * h).filter(|&c| in_box[c] && start[c] > sea_level_m).count() as f64 * cell_a / 1e6;
    let on_trunk_outer: Vec<usize> = outer.iter().copied().filter(|&k| trunk[k]).collect();
    let closed = (0..nn).filter(|&k| inside(k) && hy.nodes[k].spill.is_none()).count();
    let n_reported = (0..nn).filter(|&k| inside(k) && reported(k)).count();
    println!("BASINS input: {label}; {w} x {h} L cells of {:.1} m; hierarchy {nn} nodes in {:.1} s", grid.dx(), t0.elapsed().as_secs_f64());
    println!(
        "BASINS kept (>= 0.25 km2, >= 2 m) with the floor in the box: {} ({} nested in another, {} outermost); reported (>= 0.5 m, >= 16 cells, under the kept bar): {n_reported}; closed: {closed}",
        sel.len(),
        n_nested,
        outer.len()
    );
    println!(
        "BASINS outermost area: {:.2} km2 total ({:.3} % of the box's land {:.0} km2); p50 {:.3} p90 {:.3} max {:.2} km2; depth p50 {:.1} p90 {:.1} max {:.1} m",
        areas.iter().sum::<f64>(),
        100.0 * areas.iter().sum::<f64>() / land_km2.max(1e-9),
        land_km2,
        at(&areas, 0.5),
        at(&areas, 0.9),
        areas.last().copied().unwrap_or(0.0),
        at(&depths, 0.5),
        at(&depths, 0.9),
        depths.last().copied().unwrap_or(0.0)
    );
    println!(
        "BASINS on an H1 trunk line: {} of {} outermost ({:.2} km2); off the trunks {} ({:.2} km2)",
        on_trunk_outer.len(),
        outer.len(),
        on_trunk_outer.iter().map(|&k| km2(k)).sum::<f64>(),
        outer.len() - on_trunk_outer.len(),
        outer.iter().filter(|&&k| !trunk[k]).map(|&k| km2(k)).sum::<f64>()
    );
    let mut big: Vec<usize> = outer.clone();
    big.sort_by(|&a, &b| cells[b].cmp(&cells[a]).then(a.cmp(&b)));
    for &k in big.iter().take(5) {
        let nd = &hy.nodes[k];
        let (fx, fy) = (x0 + (nd.floor_cell % w) as i64, y0 + (nd.floor_cell / w) as i64);
        println!(
            "BASINS largest: floor L ({fx}, {fy}) {:.1} m, spill {:.1} m, {:.2} km2, depth {:.1} m, on a trunk {}",
            nd.floor_m,
            nd.spill.map_or(f64::NAN, |s| s.1),
            km2(k),
            depth(k),
            trunk[k]
        );
    }
    let _ = (&h1, &surface, store);
    Ok(())
}
