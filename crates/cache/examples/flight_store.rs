//! spike/rg-prototype (lead #175 item 3, spike only, never merged): the flight store of step C. A prototype run's final surface (an `rg_chain` `<tag>-final-<w>x<h>.f64`, little-endian f64
//! over the window's L grid, margin included) written as the L layer of a store the viewer flies: each L tile's core is the surface minus the store's base `B` (no K7, no lake, an empty water
//! section), with the basin tables of no lake and a commit naming them.
//!
//! Usage: `flight_store --window W --surface FILE --out DIR [--noise turned|axis] [--threads T]`. W is one of the gate's windows (r1h, r3a, r3b, m1, f1). DIR must not exist.
//! Version 1 stores no channel graph (the store format holds H1 lines only; the graph's channels follow in a second step).
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
    let mut tables = [CacheKey::NONE; 6];
    for (f, r) in basin_table_records(&world, spec.res, &basin, &[], &|_| face).iter().enumerate() {
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
        let r = l_tile_record(&world, spec.res, &l_spec, id, &core, &stats, &[]).map_err(|e| e.to_string())?;
        keys.insert((id.face.0, id.ti, id.tj), r.key);
        writer.append(&r).map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    let commit = Commit::new(CommitSpec::of(&l_spec, spec.res)?, tables, keys, 0, BTreeMap::new());
    commit.write(&out)?;
    write_readme(&out, wname, &win, &surface_file, commit.id, tiles.len(), inputs_id, noise)?;
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
            "SPIKE FLIGHT STORE, not the world bake (lead #175 item 3, step C): window {wname} {win:?} (face, i0, i1, j0, j1, seed); L = the prototype's final surface {} minus the store's base B (no K7, no lake, no channel graph yet). Commit {commit:016x}, {tiles} tiles, inputs identity {inputs_id:016x}, base noise {noise}. Branch spike/rg-prototype (flight_store).\n",
            surface.display()
        ),
    )
    .map_err(|e| e.to_string())
}
