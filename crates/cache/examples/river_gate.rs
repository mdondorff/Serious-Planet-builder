//! The river gate (BUILD-055): `river_gate [--window face,i0,i1,j0,j1,seed] [--margin M] [--store <commit dir> [--no-key-check]] [--min-cells N] [--d D] [--threads T]`, run by `cargo xtask river-gate`
//! under the heavy lock. It builds the region's start surface at L (the base's C1 read B plus the regional field, SPIKE-114) and its H1 channel graph, and prints [`planet_cache::river_gate`]'s report:
//! - without `--store`, on the start surface itself, counted over the region's box;
//! - with `--store`, on the stored ground B + L (an L tile is K7's ground minus B, so the regional change is in it) over the commit's tiles, the start surface around them, counted over the tiles;
//!   `--no-key-check` reads each tile under the key its commit names (a spike branch's store) and the report says so.
//!
//! The window is in regional cells (12,288 a face edge) as the spikes write it; the default is R3a's (face 1, seed 14).
use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::l_writer::{read_l_tile, Commit};
use planet_cache::layer_l::load_l_tile_keyed;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{lattice_direction, region_front, RegionSpec, LOCAL_PER_REGIONAL};
use planet_cache::river_gate::{measure, Crossing, GateInput, GateParams};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::tile::TileId as PassTile;
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
/// R3a: face 1, regional cells 8,056 to 8,675 on both axes, seed 14 (the r3a stores' window).
const R3A: [usize; 6] = [1, 8056, 8675, 760, 1379, 14];

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
        None => R3A.to_vec(),
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
        let mut differ: Vec<String> = Vec::new();
        if cs.cells as usize != spec.local() {
            differ.push(format!("cells {} (the window's {})", cs.cells, spec.local()));
        }
        if cs.radius_m != R {
            differ.push(format!("radius {} m (the window's {R} m)", cs.radius_m));
        }
        if cs.seed != seed {
            differ.push(format!("seed {} (the window's {seed})", cs.seed));
        }
        if cs.base_id != spec.base_id {
            differ.push(format!("base {:016x} (the window's {:016x})", cs.base_id, spec.base_id));
        }
        if !no_key_check && cs.inputs_id != spec.effective_inputs_id() {
            differ.push(format!(
                "inputs {:016x} (the window's {:016x}: another window, margin, rounds or carve)",
                cs.inputs_id,
                spec.effective_inputs_id()
            ));
        }
        if !differ.is_empty() {
            return Err(format!(
                "{}: the commit was not baked on the window {win:?} with margin {margin}: {}",
                dir.display(),
                differ.join(", ")
            ));
        }
        sea_level_m = cs.sea_level_m;
        let l_spec = commit.spec.l_spec();
        let t = l_spec.grid.tile;
        inside = vec![false; w * h];
        let (mut read, mut outside) = (0usize, 0usize);
        for &(f, ti, tj) in commit.tiles.keys() {
            let (tx0, ty0) = (i64::from(ti) * t as i64 - x0, i64::from(tj) * t as i64 - y0);
            if f != face.0 || tx0 < 0 || ty0 < 0 || tx0 as usize + t > w || ty0 as usize + t > h {
                outside += 1;
                continue;
            }
            let id = PassTile { face, ti, tj };
            let tile = if no_key_check {
                load_l_tile_keyed(&tiles, &l_spec, id, commit.tiles[&(f, ti, tj)])
            } else {
                read_l_tile(&tiles, &world, commit.spec.res, &l_spec, &commit, id)
            }
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("the commit names tile {f}/{ti}/{tj} but the store does not hold it"))?;
            // The stored ground: B plus the tile's change (K7's ground minus B, which holds the regional change already).
            let mut ground = vec![0f64; t * t];
            par_fill(&mut ground, t, threads, &|p| {
                let (lx, ly) = (p % t, p / t);
                base(dir_of(x0 + tx0 + lx as i64, y0 + ty0 + ly as i64)) + f64::from(tile.core[p])
            });
            for (p, g) in ground.iter().enumerate() {
                let c = (ty0 as usize + p / t) * w + tx0 as usize + p % t;
                surface[c] = *g;
                inside[c] = true;
            }
            read += 1;
        }
        if read == 0 {
            return Err(format!("{}: none of the commit's {} tiles lies inside the window {win:?}", dir.display(), commit.tiles.len()));
        }
        label = format!(
            "store {} (commit {:016x}, {read} tiles read, {outside} outside the window; {}): the stored ground B + L over its tiles, which drain to their border, window {win:?}, sea level {sea_level_m} m; the stored change's 0.25 m steps add pits to the 5 cm reference only",
            dir.display(),
            commit.id,
            if no_key_check { "keys not checked: spike store" } else { "keys checked" }
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
    front.graph.for_each(|s| {
        let cells = s.cells(&front.solve.grid);
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
        },
        &params,
    );
    println!("RIVER-GATE input: {label}; {w} x {h} L cells of {:.1} m", grid.dx());
    for line in report.lines() {
        println!("{line}");
    }
    Ok(())
}
