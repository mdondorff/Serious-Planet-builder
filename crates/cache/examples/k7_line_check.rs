//! Read-only check (#125): do the lines K7 burns (the bake's in-memory feed) and the lines the commit stores (the channel graph blocks, the viewer's feed) agree, and does K7's own surface rise along them?
//! Builds the R3a front exactly as `bake_region` does, runs K7 on one tile and compares, over the tile's core:
//! - the line cells of the in-memory channels against those of the stored graph's channels (cells in one set only);
//! - the rises over 0.25 m of K7's `z` along each set of lines;
//! - K7's `z` against the stored L (`B + L` at the cell, quantised) at the line cells.
//!
//! Usage: `k7_line_check <commit dir> <ti> <tj> [--threads T]` (R3a of face 1, seed 14, margin 200: the defaults of `bake_region`).
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
    let (ti, tj): (u32, u32) = (args[1].parse().expect("ti"), args[2].parse().expect("tj"));
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
    let id = TileId { face, ti, tj };
    let (t, h, w) = (spec.tile as i64, spec.halo as i64, grid.window() as i64);
    let (wi0, wj0) = (i64::from(ti) * t - h, i64::from(tj) * t - h);
    let mem: Vec<Channel> = front.feed.channels_near(face, wi0, wj0, w);
    let r = k7_tile(&grid, id, &base, &regional, &mem, &K7Params::standard(seed, 0.0), threads);

    let commit = Commit::read(dir)?.ok_or("no commit record")?;
    let store = Arc::new(TileStore::open(dir, world.params()).map_err(|e| format!("{e}"))?);
    let stored: Vec<Channel> = ChannelGraphFeed::new(store, commit)?.try_channels_near(face, wi0, wj0, w)?;

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
    let src: Arc<dyn planet_generators::erosion_layer::LayerSource> = Arc::new(planet_cache::l_layer::CommitLayer::open(dir, world.clone(), 16)?);
    let layer = planet_generators::erosion_layer::ErosionLayer::new(src)?;
    let n = spec.local() as f64;
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
    Ok(())
}
