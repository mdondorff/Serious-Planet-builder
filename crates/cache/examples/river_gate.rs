//! The river gate (BUILD-055): `river_gate [--window face,i0,i1,j0,j1,seed] [--margin M] [--store <commit dir> [--no-key-check]] [--min-cells N] [--d D] [--threads T]`, run by `cargo xtask river-gate`
//! under the heavy lock. It builds the region's start surface at L (the base's C1 read B plus the regional field, SPIKE-114) and its H1 channel graph, and prints [`planet_cache::river_gate`]'s report:
//! - without `--store`, on the start surface itself, counted over the region's box;
//! - with `--store`, on the stored ground B + L (an L tile is K7's ground minus B, so the regional change is in it) over the commit's tiles, the start surface around them, counted over the tiles;
//!   `--no-key-check` reads each tile under the key its commit names (a spike branch's store) and the report says so.
//!
//! - with `--grid file,w,h,sea_level_m`, on a raw height grid (little-endian f64, row-major, cells of 101.8 m as SPIKE-119 prepared them; a GLO-30 benchmark tile prepared by SPIKE-119's
//!   `dem_prep.py`): the grid's edge and every cell at or below the sea level are outlets, counted inside a 10-cell border, routing seed 7, no H1 (BUILD-059).
//!
//! The window is in regional cells (12,288 a face edge) as the spikes write it; the default is R1h (face 1, seed 14; the E1 region stores' window, lead 2026-11-14).
use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::Commit;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{
    border_inside, commit_differences, key_note, lay_store_ground, measure, raw_grid_arg, raw_grid_heights, Crossing, GateInput,
    GateParams, LWindow, RawGrid, WindowIdentity, GRID_BORDER, GRID_CELL_M,
};
use planet_cache::river_gate_graph::{
    h1_trunks, read_graph, read_stages, read_surface, G2Input, Window, CORRIDOR_MIN_PCT, CORRIDOR_MIN_PCT_MOUNTAIN,
};
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
/// The mountain window (SPIKE-119), whose corridor bar is 95 % (lead #168).
const M1: [usize; 6] = [4, 6600, 7219, 10200, 10819, 15];

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

/// The gate on a raw height grid (`--grid file,w,h,sea_level_m`; BUILD-059's GLO-30 benchmark input).
fn grid_gate(arg: &str, params: &GateParams) -> Result<(), String> {
    let RawGrid { file, w, h, sea_level_m: sea } = raw_grid_arg(arg)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("{file}: {e}"))?;
    let surface = raw_grid_heights(&bytes, w, h).map_err(|e| format!("{file}: {e}"))?;
    let grid = planet_generators::bake::Grid::new(w, h, GRID_CELL_M);
    let outlet = grid.outlets(&surface, sea);
    let inside = border_inside(w, h, GRID_BORDER);
    let region = vec![true; w * h];
    let report = measure(
        &GateInput {
            grid: &grid,
            start: &surface,
            surface: &surface,
            outlet: &outlet,
            inside: &inside,
            region: &region,
            h1: &[],
            seed: 7,
            sea_level_m: sea,
            outside_drains: false,
            trunks: &[],
            g2: None,
        },
        params,
    );
    println!("RIVER-GATE input: grid {file}, sea level {sea} m, counted inside a {GRID_BORDER}-cell border, no H1; {w} x {h} cells of {GRID_CELL_M} m");
    for line in report.lines() {
        println!("{line}");
    }
    println!("{}", report.fields_line());
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_key_check = args.iter().any(|a| a == "--no-key-check");
    let args: Vec<String> = args.into_iter().filter(|a| a != "--no-key-check").collect();
    let known = [
        "--window",
        "--margin",
        "--store",
        "--min-cells",
        "--d",
        "--threads",
        "--network-km2",
        "--grid",
        "--graph",
        "--stages",
        "--partition",
        "--surface",
        "--corridor-bar",
        "--start",
        "--dump-surface",
    ];
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
    let network_km2 = match value(&args, "--network-km2") {
        None => GateParams::default().network_km2,
        Some(v) => {
            let a: Vec<f64> = v
                .split(',')
                .map(|x| x.parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("--network-km2 a,b in km2, not '{v}'"))?;
            match a[..] {
                [x, y] if x > 0.0 && y > 0.0 => [x, y],
                _ => return Err(format!("--network-km2 takes two positive areas a,b in km2, not '{v}'")),
            }
        }
    };
    let params = GateParams {
        basin_cells: parsed(&args, "--min-cells", GateParams::default().basin_cells)?,
        lake_depth_m: parsed(&args, "--d", 20.0)?,
        network_km2,
        // lead #168: the corridor bar is 95 % on the mountain window M1, 97.5 % elsewhere
        corridor_min_pct: parsed(&args, "--corridor-bar", if win[..] == M1 { CORRIDOR_MIN_PCT_MOUNTAIN } else { CORRIDOR_MIN_PCT })?,
        ..GateParams::default()
    };
    if let Some(g) = value(&args, "--grid") {
        if ["--store", "--window", "--surface", "--start"].iter().any(|k| value(&args, k).is_some()) {
            return Err("--grid is its own input: it takes no --store, --window, --surface or --start".to_string());
        }
        return grid_gate(g, &params);
    }
    let store = value(&args, "--store").map(Path::new);
    if no_key_check && store.is_none() {
        return Err("--no-key-check reads a store: it goes with --store".to_string());
    }
    if value(&args, "--surface").is_some() && store.is_some() {
        return Err("--surface is the candidate's surface on the window: it takes no --store".to_string());
    }
    if value(&args, "--start").is_some() && store.is_some() {
        return Err("--start is a chain run's own start surface: a store's ground lies on main's base, so it takes no --store".to_string());
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
    // BUILD-060 `--start` (lead #171): the run's own start surface on the window's L grid (a chain that conditions its base, as A+, has another S0 than main's); every delta and the
    // corridor are then measured against it, and the trunks come from the run's own graph (`--graph`'s H1 reaches); the H1 edge-agreement check still reads main's H1
    let own_start = value(&args, "--start");
    if let Some(path) = own_start {
        start = read_surface(path, w * h)?;
    }
    let (bi0, bi1, bj0, bj1) = ((region.0 * l) as i64, ((region.1 + 1) * l) as i64, (region.2 * l) as i64, ((region.3 + 1) * l) as i64);
    let in_box: Vec<bool> = (0..w * h)
        .map(|c| {
            let (gx, gy) = (x0 + (c % w) as i64, y0 + (c / w) as i64);
            gx >= bi0 && gx < bi1 && gy >= bj0 && gy < bj1
        })
        .collect();
    let mut surface = start.clone();
    let mut inside = in_box.clone();
    let mut label = match own_start {
        Some(p) => format!("the run's own start surface {p} on the window {win:?}, margin {margin}, counted over the region's box"),
        None => format!("start surface of the window {win:?}, margin {margin}, counted over the region's box"),
    };
    let mut sea_level_m = 0.0;
    // BUILD-060: a candidate's surface on the window's L grid (the RG chain's final surface), measured against the window's start surface
    if let Some(path) = value(&args, "--surface") {
        surface = read_surface(path, w * h)?;
        label = format!("surface {path} on the window {win:?}, margin {margin}, against its start surface, counted over the region's box");
    }
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
    // spike/rg-chain (explorer #167, never merged): the measured surface as little-endian f64 on the window's L grid, for the stills
    if let Some(f) = value(&args, "--dump-surface") {
        let b: Vec<u8> = surface.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(f, b).map_err(|e| format!("{f}: {e}"))?;
        println!("RIVER-GATE surface written: {f} ({w} x {h} f64)");
    }
    let grid = planet_generators::bake::Grid::new(w, h, front.solve.grid.dx() / l as f64);
    let outlet = grid.outlets(&surface, sea_level_m);
    // The G2 inputs (BUILD-060): a canonical graph, the per-stage budget fields, a partition pair; global L ids against this window's origin.
    let graph = value(&args, "--graph").map(read_graph).transpose()?;
    if own_start.is_some() {
        // the run's own H1 trunks: its graph's H1 reaches; a run on its own start without its graph has no trunks of its own, so its corridor is not measured
        trunks = graph.as_ref().map_or_else(Vec::new, |g| h1_trunks(g, &Window { x0: x0 as u64, y0: y0 as u64, w, h }, l));
    }
    let partition = match value(&args, "--partition") {
        None => None,
        Some(v) => {
            let (a, b) = v.split_once(',').ok_or_else(|| format!("--partition one,two: two canonical graph files, not '{v}'"))?;
            Some((read_graph(a)?, read_graph(b)?))
        }
    };
    let (stages, residual_records) = match value(&args, "--stages") {
        None => (None, None),
        Some(dir) => {
            let (st, rec) = read_stages(Path::new(dir), w * h)?;
            (Some(st), rec)
        }
    };
    if stages.is_some() && (x0 % l as i64 != 0 || y0 % l as i64 != 0) {
        return Err(format!(
            "--stages: the budget field reads H1 blocks of {l} L cells, and the window's origin ({x0}, {y0}) is not on an H1 corner"
        ));
    }
    let g2_input = (graph.is_some() || partition.is_some() || stages.is_some()).then(|| G2Input {
        window: Window { x0: x0 as u64, y0: y0 as u64, w, h },
        graph: graph.as_ref(),
        stages: stages.as_deref(),
        residual_records,
        partition: partition.as_ref().map(|(a, b)| (a, b)),
    });
    let report = measure(
        &GateInput {
            grid: &grid,
            start: &start,
            surface: &surface,
            outlet: &outlet,
            inside: &inside,
            region: &in_box,
            h1: &h1,
            seed: front.solve.stream,
            sea_level_m,
            outside_drains: store.is_some(),
            trunks: &trunks,
            g2: g2_input.as_ref(),
        },
        &params,
    );
    println!("RIVER-GATE input: {label}; {w} x {h} L cells of {:.1} m", grid.dx());
    for line in report.lines() {
        println!("{line}");
    }
    println!("{}", report.fields_line());
    if let Some(c) = report.classes_line() {
        println!("{c}");
    }
    Ok(())
}
