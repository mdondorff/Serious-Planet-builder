//! spike/rg-prototype (lead #175 item 3, spike only, never merged): the flight store of step C. A prototype run's final surface (an `rg_chain` `<tag>-final-<w>x<h>.f64`, little-endian f64
//! over the window's L grid, margin included) written as the L layer of a store the viewer flies: each L tile's core is the surface minus the store's base `B` (no K7, no lake, an empty water
//! section), with the basin tables of no lake and a commit naming them.
//!
//! Usage: `flight_store --window W --surface FILE --out DIR [--graph FILE [--graph-level l|h1] [--graph-min-km2 A]] [--noise turned|axis] [--threads T]`. W is one of the gate's windows (r1h, r3a, r3b, m1, f1). DIR must not exist.
//! With `--graph` the run's graph is the store's channel graph: its L reaches (default) on the window's L grid, or its H1 reaches, where the drained area is at least A km2 (default 5).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use planet_cache::basin_table::{basin_table_records, BasinSpec};
use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::key::CacheKey;
use planet_cache::l_writer::{Commit, CommitSpec};
use planet_cache::layer_l::{l_tile_record, LTileSpec, LTileStats};
use planet_cache::pack::{PackWriter, DEFAULT_MAX_PACK_BYTES};
use planet_cache::region_bake::{lattice_direction, RegionSpec, LOCAL_PER_REGIONAL};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::bake::tile::TileGrid;
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const MARGIN: usize = 200;
/// A kept basin is a lake when its floor holds at least this much water on the final surface (metres).
const LAKE_MIN_DEPTH_M: f64 = 0.5;
/// A kept basin's recorded spill is valid while the final surface's depression level at its floor lies at most this below it (the gate's spill_mismatch rule, lead #176b).
const SPILL_MISMATCH_M: f64 = 0.5;

/// The gate's windows (BUILD-055): `face,i0,i1,j0,j1,seed` in regional cells.
const WINDOWS: [(&str, [usize; 6]); 5] = [
    ("r1h", [1, 8056, 8675, 760, 1379, 14]),
    ("r3a", [4, 368, 987, 4720, 5339, 15]),
    ("r3b", [2, 9968, 10587, 8304, 8923, 15]),
    ("m1", [4, 6600, 7219, 10200, 10819, 15]),
    ("f1", [4, 4800, 5419, 3300, 3919, 15]),
];

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

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).map(|k| args.get(k + 1).map(String::as_str).expect("an option takes a value"))
}

/// FNV-1a over bytes: the surface file's identity in the store's inputs id.
fn fnv(bytes: impl IntoIterator<Item = u8>, mut h: u64) -> u64 {
    for b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let threads: usize = opt(&args, "--threads").map_or(Ok(8), str::parse).map_err(|_| "--threads T")?;
    let noise = opt(&args, "--noise").unwrap_or("turned");
    if noise != "axis" && noise != "turned" {
        return Err(format!("--noise axis|turned, not '{noise}'"));
    }
    planet_generators::height::set_turned_noise(noise == "turned");
    // `--latlon FACE,GX,GY`: the latitude and longitude (degrees) of a face's local L cell, for the poses of a flight
    if let Some(v) = opt(&args, "--latlon") {
        let p: Vec<i64> = v
            .split(',')
            .map(|s| s.trim().parse::<f64>().map(|x| x.round() as i64))
            .collect::<Result<_, _>>()
            .map_err(|_| "--latlon FACE,GX,GY")?;
        let [f, gx, gy] = p[..] else { return Err("--latlon FACE,GX,GY".into()) };
        let g = TileGrid { cells: REGIONAL * LOCAL_PER_REGIONAL, tile: 768, halo: 64, radius_m: R };
        let (lat, lon) = g.direction(Face(f as u8), gx, gy).lat_lon();
        println!("FLIGHT-STORE latlon {f},{gx},{gy}: lat {:.5} lon {:.5}", lat.to_degrees(), lon.to_degrees());
        return Ok(());
    }
    let wname = opt(&args, "--window").ok_or("--window W (r1h, r3a, r3b, m1, f1)")?;
    let win = WINDOWS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(wname))
        .ok_or_else(|| format!("--window {wname}: one of r1h, r3a, r3b, m1, f1"))?
        .1;
    let surface_file = PathBuf::from(opt(&args, "--surface").ok_or("--surface FILE (the run's final f64 surface)")?);
    let out = PathBuf::from(opt(&args, "--out").ok_or("--out DIR")?);
    if out.exists() {
        return Err(format!("{} exists: a spike store never writes into an existing folder", out.display()));
    }
    let t0 = std::time::Instant::now();
    let (seed, face) = (win[5] as u64, Face(win[0] as u8));
    let (i0, j0, size) = (win[1], win[3], win[2] - win[1] + 1);
    let def = WorldDefinition::single_planet(R, seed);
    let inputs = class_inputs(&def, seed, threads);
    let region = (i0, i0 + size - 1, j0, j0 + size - 1);
    let (ri0, rj0) = (region.0 - MARGIN, region.2 - MARGIN);
    let (ri1, rj1) = (region.1 + MARGIN, region.3 + MARGIN);
    // the base B exactly as the spike bakes read it (bake_eroded): the global lattice's base read with C1 interpolation
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
    let spec = RegionSpec {
        seed,
        face,
        radius_m: R,
        regional: REGIONAL,
        region,
        margin: MARGIN,
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
    // the window's L grid: the region with its margin, cell (0, 0) at the face's local cell (x0, y0)
    let l = LOCAL_PER_REGIONAL;
    let (x0, y0) = ((ri0 * l) as i64, (rj0 * l) as i64);
    let (w, h) = ((ri1 - ri0 + 1) * l, (rj1 - rj0 + 1) * l);
    let bytes = std::fs::read(&surface_file).map_err(|e| format!("{}: {e}", surface_file.display()))?;
    if bytes.len() != w * h * 8 {
        return Err(format!(
            "{}: {} bytes, the window's L grid is {w} x {h} f64 ({} bytes)",
            surface_file.display(),
            bytes.len(),
            w * h * 8
        ));
    }
    let surface: Vec<f64> = bytes.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect();
    if let Some(c) = surface.iter().position(|z| !z.is_finite()) {
        return Err(format!("{}: cell ({}, {}) is not finite", surface_file.display(), c % w, c / w));
    }
    let inputs_id = spec.effective_inputs_id() ^ fnv(bytes.iter().copied(), 0xcbf2_9ce4_8422_2325) ^ 0x464c_4947_4854_0001;
    let graph = opt(&args, "--graph").map(planet_cache::river_gate_graph::read_graph).transpose()?;
    drop(bytes);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?;
    let basin = BasinSpec {
        hydro: planet_generators::bake::hydro::HydroParams::default(),
        tile_cells: spec.tile as u32,
        tiles_per_edge: spec.tiles_per_edge(),
        base_id: spec.base_id,
        inputs_id,
    };
    let grid = TileGrid { cells: spec.local(), tile: spec.tile, halo: spec.halo, radius_m: spec.radius_m };
    let l_spec = LTileSpec { grid, k7: planet_generators::bake::k7::K7Params::standard(spec.seed, 0.0), base_id: spec.base_id, inputs_id };
    let t = spec.tile;
    let mut tiles = spec.tiles();
    tiles.sort_unstable_by_key(|id| (id.face.0, id.ti, id.tj));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut writer = PackWriter::open(&out, world.params(), DEFAULT_MAX_PACK_BYTES).map_err(|e| e.to_string())?;
    // lead #176a: the graph's kept L basins (`protected_surface`) as lakes at their spill level, their water flooded on the final surface from the floor cell
    let (lakes, lake_of, invalid) = match (&graph, args.iter().any(|a| a == "--no-lakes")) {
        (Some(g), false) => {
            let spill = match opt(&args, "--lake-level").unwrap_or("spill") {
                "spill" => true,
                "depression" => false,
                o => return Err(format!("--lake-level spill|depression, not '{o}'")),
            };
            kept_lakes(g, &surface, w, h, x0, y0, spec.radius_m * std::f64::consts::FRAC_PI_2 / spec.local() as f64, face, t, spill)?
        }
        _ => (Vec::new(), Vec::new(), 0),
    };
    let mut tables = [CacheKey::NONE; 6];
    let mut by_id = lakes.clone();
    by_id.sort_by_key(|l| l.id);
    for (f, r) in basin_table_records(&world, spec.res, &basin, &by_id, &|_| face).iter().enumerate() {
        tables[f] = r.key;
        writer.append(r).map_err(|e| e.to_string())?;
    }
    let mut keys = BTreeMap::new();
    let (mut land, mut lowest, mut highest) = (0u64, f64::INFINITY, f64::NEG_INFINITY);
    for &id in &tiles {
        let mut core = vec![0f32; t * t];
        let mut tile_land = 0u64;
        let rows = t.div_ceil(threads);
        let mut bad: Option<String> = None;
        std::thread::scope(|s| {
            let handles: Vec<_> = core
                .chunks_mut(rows * t)
                .enumerate()
                .map(|(k, chunk)| {
                    let (base, surface, grid) = (&base, &surface, &grid);
                    s.spawn(move || -> Result<u64, String> {
                        let mut n = 0u64;
                        for (c, v) in chunk.iter_mut().enumerate() {
                            let p = k * rows * t + c;
                            let (gx, gy) = (i64::from(id.ti) * t as i64 + (p % t) as i64, i64::from(id.tj) * t as i64 + (p / t) as i64);
                            let (x, y) = (gx - x0, gy - y0);
                            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                                return Err(format!(
                                    "tile {}/{}/{}: cell ({gx}, {gy}) lies outside the surface's grid",
                                    id.face.0, id.ti, id.tj
                                ));
                            }
                            let z = surface[y as usize * w + x as usize];
                            n += u64::from(z > 0.0);
                            *v = (z - base(grid.direction(id.face, gx, gy))) as f32;
                        }
                        Ok(n)
                    })
                })
                .collect();
            for hd in handles {
                match hd.join().expect("a tile worker") {
                    Ok(n) => tile_land += n,
                    Err(e) => bad = Some(e),
                }
            }
        });
        if let Some(e) = bad {
            return Err(e);
        }
        for &v in &core {
            lowest = lowest.min(f64::from(v));
            highest = highest.max(f64::from(v));
        }
        land += tile_land;
        let stats = LTileStats { land_cells: tile_land, burned_cells: 0, pit_cells: 0, pit_p50_m: 0.0, pit_p99_m: 0.0, pit_max_m: 0.0 };
        let water = if lake_of.is_empty() {
            Vec::new()
        } else {
            let labels: Vec<u64> = (0..t * t)
                .map(|p| {
                    let (x, y) = (i64::from(id.ti) * t as i64 + (p % t) as i64 - x0, i64::from(id.tj) * t as i64 + (p / t) as i64 - y0);
                    match lake_of[y as usize * w + x as usize] {
                        u32::MAX => planet_generators::bake::water_mask::NO_LAKE,
                        k => lakes[k as usize].id,
                    }
                })
                .collect();
            planet_generators::bake::water_mask::encode(&labels, t).map_err(|e| format!("tile {}/{}/{}: {e}", id.face.0, id.ti, id.tj))?
        };
        let r = l_tile_record(&world, spec.res, &l_spec, id, &core, &stats, &water).map_err(|e| e.to_string())?;
        keys.insert((id.face.0, id.ti, id.tj), r.key);
        writer.append(&r).map_err(|e| e.to_string())?;
    }
    // the run's graph (GEN-189 canonical form) as the store's channel graph, cut into chains of 8-neighbour cells inside the window: `--graph-level l` (default; lead #176) the L reaches on the
    // window's L grid (one stored cell per L cell), `h1` the H1 reaches on the H1 grid
    let (mut graph_cells, mut graph_keys) = (0u32, BTreeMap::new());
    if let (Some(gpath), Some(g)) = (opt(&args, "--graph"), &graph) {
        let level_name = opt(&args, "--graph-level").unwrap_or("l");
        let (level, per_edge, gx0, gy0, rw, rh) = match level_name {
            "l" => (planet_generators::bake::river_graph::Level::L, spec.local(), x0 as usize, y0 as usize, w, h),
            "h1" => (planet_generators::bake::river_graph::Level::H1, spec.regional, ri0, rj0, ri1 - ri0 + 1, rj1 - rj0 + 1),
            o => return Err(format!("--graph-level l|h1, not '{o}'")),
        };
        // lead #176: a vertex is stored only where its drained area is at least `--graph-min-km2` (default 5 km2; the drawing has a width floor of about one stored cell)
        let min_m2: f64 = opt(&args, "--graph-min-km2").map_or(Ok(5.0), str::parse::<f64>).map_err(|_| "--graph-min-km2 KM2")? * 1.0e6;
        let h1 = planet_generators::bake::Grid::new(rw, rh, spec.radius_m * std::f64::consts::FRAC_PI_2 / per_edge as f64);
        let index = |id: u64| -> Option<u32> {
            let (gx, gy) = ((id & 0xffff_ffff) as usize, (id >> 32) as usize);
            (gx >= gx0 && gy >= gy0 && gx < gx0 + rw && gy < gy0 + rh).then(|| ((gy - gy0) * rw + (gx - gx0)) as u32)
        };
        let mut chains: Vec<(Vec<u32>, Vec<f64>, Vec<i32>)> = Vec::new();
        let mut ids: Vec<u64> = Vec::new();
        for r in g.reaches.iter().filter(|r| r.level == level) {
            let mut cur: (Vec<u32>, Vec<f64>, Vec<i32>) = (Vec::new(), Vec::new(), Vec::new());
            let mut part = 0u64;
            let mut flush = |cur: &mut (Vec<u32>, Vec<f64>, Vec<i32>), part: &mut u64| {
                if cur.0.len() >= 2 {
                    chains.push(std::mem::take(cur));
                    ids.push(r.id ^ part.wrapping_mul(0x9e37_79b9_7f4a_7c15));
                    *part += 1;
                }
                cur.0.clear();
                cur.1.clear();
                cur.2.clear();
            };
            for (v, &id) in r.vertices.iter().enumerate() {
                let Some(c) = index(id).filter(|_| r.attrs.a_geo[v] >= min_m2) else {
                    flush(&mut cur, &mut part);
                    continue;
                };
                let adjacent = cur.0.last().is_none_or(|&p| (0..8).any(|d| h1.neighbour(p as usize, d) == Some(c as usize)));
                if !adjacent {
                    flush(&mut cur, &mut part);
                }
                cur.0.push(c);
                cur.1.push(r.attrs.q_w[v]);
                cur.2.push(r.attrs.bed[v]);
            }
            flush(&mut cur, &mut part);
        }
        let chains_n = chains.len();
        let verts_n: usize = chains.iter().map(|c| c.0.len()).sum();
        let cg = planet_generators::bake::channel_graph::ChannelGraph::from_chains(&h1, &chains)?;
        let gspec = planet_cache::graph_store::GraphSpec { basin, h1_cells: per_edge as u32, margin_h1: 0 };
        let gspec = planet_cache::graph_store::GraphSpec { margin_h1: gspec.required_margin(spec.halo as u32), ..gspec };
        let cell_xy = |c: u32| (face.0, (gx0 + c as usize % rw) as i64, (gy0 + c as usize / rw) as i64);
        for r in planet_cache::graph_store::graph_block_records(&world, spec.res, &gspec, &cg, &ids, &h1, &cell_xy) {
            let (f, bi, bj) = planet_cache::graph_store::block_of(&gspec, &r)?;
            graph_keys.insert((f, bi, bj), r.key);
            writer.append(&r).map_err(|e| e.to_string())?;
        }
        graph_cells = per_edge as u32;
        println!("FLIGHT-STORE graph {gpath} ({level_name}, drained area >= {} km2): {chains_n} chains,", min_m2 / 1.0e6);
        println!("FLIGHT-STORE graph detail: {verts_n} vertices, {} blocks", graph_keys.len());
    }
    writer.finish().map_err(|e| e.to_string())?;
    let commit = Commit::new(CommitSpec::of(&l_spec, spec.res)?, tables, keys, graph_cells, graph_keys);
    commit.write(&out)?;
    write_readme(&out, wname, &win, &surface_file, commit.id, tiles.len(), inputs_id, noise)?;
    println!(
        "FLIGHT-STORE lakes: {} kept L basins as lakes, {} wet cells; {invalid} kept basins dry (spill mismatch: the final surface's depression at the floor, or a cell its water reaches, more than {SPILL_MISMATCH_M} m under the recorded spill)",
        lakes.len(),
        lake_of.iter().filter(|&&k| k != u32::MAX).count()
    );
    println!(
        "FLIGHT-STORE {wname}: {} tiles from {} into {} (commit {:016x}, inputs identity {inputs_id:016x}); {land} land cells; L from {lowest:.1} to {highest:.1} m; {:.0} s",
        tiles.len(),
        surface_file.display(),
        out.display(),
        commit.id,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_readme(
    out: &Path,
    wname: &str,
    win: &[usize; 6],
    surface: &Path,
    commit: u64,
    tiles: usize,
    inputs_id: u64,
    noise: &str,
) -> Result<(), String> {
    std::fs::write(
        out.join("README.txt"),
        format!(
            "SPIKE FLIGHT STORE, not the world bake (lead #175 item 3, step C): window {wname} {win:?} (face, i0, i1, j0, j1, seed); L = the prototype's final surface {} minus the store's base B (no K7, no lake; the channel graph as the store's commit names it). Commit {commit:016x}, {tiles} tiles, inputs identity {inputs_id:016x}, base noise {noise}. Branch spike/rg-prototype (flight_store).\n",
            surface.display()
        ),
    )
    .map_err(|e| e.to_string())
}

/// The kept L basins of `g` (`protected_surface`) as lakes on the final surface `z`: the depression of `z` (its priority flood with the window's edge and the sea as outlets) that holds a basin's floor
/// cell is its water, at the depression's level (at most the graph's spill: later stages may have lowered the rim); a basin whose floor holds less than [`LAKE_MIN_DEPTH_M`] of water is dry, and two kept
/// basins in one depression share it (the first by id names it). Returns the lakes and, per window cell, the index of its lake (`u32::MAX` for dry).
#[allow(clippy::too_many_arguments)]
fn kept_lakes(
    g: &planet_generators::bake::river_graph::Graph,
    z: &[f64],
    w: usize,
    h: usize,
    x0: i64,
    y0: i64,
    cell_m: f64,
    face: Face,
    tile: usize,
    spill: bool,
) -> Result<(Vec<planet_generators::bake::lakes::Lake>, Vec<u32>, usize), String> {
    use planet_generators::bake::river_graph::{Level, Outlet, Represented};
    let grid = planet_generators::bake::Grid::new(w, h, cell_m);
    let filled = planet_generators::bake::flood::priority_flood_with(&grid, z, &grid.outlets(z, 0.0), 1.0e-7, false);
    let mut kept: Vec<_> = g.basins.iter().filter(|b| b.level == Level::L && b.represented == Represented::ProtectedSurface).collect();
    // spill (lead #176b): increasing spill, so an enclosing basin's water takes the cells of the ones it holds; depression: by id
    if spill {
        kept.sort_by(|a, b| a.spill_m.total_cmp(&b.spill_m).then(a.id.cmp(&b.id)));
    } else {
        kept.sort_by_key(|b| b.id);
    }
    let (mut invalid, mut leaks) = (0usize, 0usize);
    let mut label = vec![u32::MAX; w * h];
    let mut lakes = Vec::new();
    let mut stack = Vec::new();
    for b in kept {
        if b.id == planet_generators::bake::water_mask::NO_LAKE {
            return Err(format!("basin {:016x}: the id of the dry label", b.id));
        }
        let (fx, fy) = ((b.floor_cell & 0xffff_ffff) as i64 - x0, (b.floor_cell >> 32) as i64 - y0);
        if fx < 0 || fy < 0 || fx >= w as i64 || fy >= h as i64 {
            continue;
        }
        let start = fy as usize * w + fx as usize;
        // spill: the graph's recorded spill is the level, valid while the final surface's depression at the floor reaches it within SPILL_MISMATCH_M (the gate's spill_mismatch)
        if spill && (filled[start] < b.spill_m - SPILL_MISMATCH_M || z[start] >= b.spill_m) {
            invalid += 1;
            continue;
        }
        if !spill && (label[start] != u32::MAX || filled[start] - z[start] < LAKE_MIN_DEPTH_M) {
            continue;
        }
        let level = if spill { b.spill_m } else { filled[start] };
        let k = lakes.len() as u32;
        let (mut cells, mut tiles) = (0u32, std::collections::BTreeSet::new());
        // the cells this lake takes with the label each had, so a lake that leaks over a lowered rim gives them back
        let mut taken: Vec<(usize, u32)> = vec![(start, label[start])];
        let mut leaked = false;
        label[start] = k;
        stack.push(start);
        while let Some(c) = stack.pop() {
            cells += 1;
            // spill: a cell whose depression stands more than SPILL_MISMATCH_M under the spill lies beyond the rim (the water would run out there)
            if spill && filled[c] < level - SPILL_MISMATCH_M {
                leaked = true;
                break;
            }
            let (x, y) = ((c % w) as i64, (c / w) as i64);
            tiles.insert((((x + x0) as usize / tile) as u32, ((y + y0) as usize / tile) as u32));
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (u, v) = (x + dx, y + dy);
                    if u < 0 || v < 0 || u >= w as i64 || v >= h as i64 {
                        continue;
                    }
                    let q = v as usize * w + u as usize;
                    // the same depression: water stands there, its flood within 1 mm of this level (the flood grades its flats at 1e-7 m a cell)
                    let wet = if spill {
                        label[q] != k && z[q] < level
                    } else {
                        label[q] == u32::MAX && filled[q] - z[q] > 1.0e-4 && (filled[q] - level).abs() < 1.0e-3
                    };
                    if wet {
                        taken.push((q, label[q]));
                        label[q] = k;
                        stack.push(q);
                    }
                }
            }
        }
        if leaked {
            for &(c, was) in taken.iter().rev() {
                label[c] = was;
            }
            stack.clear();
            leaks += 1;
            continue;
        }
        let active = b.outlet == Outlet::Active;
        lakes.push(planet_generators::bake::lakes::Lake {
            id: b.id,
            floor_id: b.floor_cell,
            spill_id: b.spill_cell,
            floor_m: b.floor_m,
            spill_m: b.spill_m,
            level_m: level,
            class: if active {
                planet_generators::bake::hydro::BasinClass::Lake
            } else {
                planet_generators::bake::hydro::BasinClass::Endorheic
            },
            overflows: active,
            wet: cells > 0,
            area_m2: f64::from(cells) * cell_m * cell_m,
            cells,
            q_in: 0.0,
            q_out: 0.0,
            tiles: tiles.into_iter().map(|(ti, tj)| planet_generators::bake::lakes::TileRef { face: face.0, ti, tj }).collect(),
        });
    }
    // the labels index this list in flooding order; the basin tables take a copy sorted by id
    Ok((lakes, label, invalid + leaks))
}
