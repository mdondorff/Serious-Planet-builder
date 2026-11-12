//! GEN-177: bakes a region of one face to a commit with a channel graph, over the real inputs of the default world: the base read C1 from a lattice sampled at the nodes (`BakeInputs::base`), the climate from
//! the orography (runoff, open-water evaporation and discharge weight as the build gives them to H1). A G-RT FRAME-TIME FIXTURE, not the world bake: a small stored region for the river viewer and its flight,
//! rebuilt when the start-surface fix lands (architect, 2026-11-12). Writes only under the output folder, never into a store; the folder gets a README.txt that says so.
//!
//! Usage: `bake_region [--seed N] [--face F] [--region i0,j0,size] [--margin M] [--threads T] [--out DIR]`
//! The region is in cells of the 12,288-cell regional grid (default: the prototype region R3a of face 1 of seed 14, as `k7_region`); the output defaults to `target/stores/g-rt-fixture`.
//! Open the result with the viewer's `--erosion` pointed at the folder over a store of the same world.
#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;
use std::time::Instant;

use planet_cache::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use planet_cache::region_bake::{bake_region, lattice_direction, RegionSpec};
use planet_core::{Face, TangentWarp, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

/// The lattice nodes of a rectangle of one face, sampled from the base field (f32, as stored).
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get =
        |name: &str| args.iter().position(|a| a == name).map(|k| args.get(k + 1).unwrap_or_else(|| panic!("{name} takes a value")).clone());
    let seed: u64 = get("--seed").map_or(14, |s| s.parse().expect("--seed N"));
    let face = Face(get("--face").map_or(1, |s| s.parse().expect("--face F")));
    let (i0, j0, size) = get("--region").map_or((2014 * 4, 190 * 4, (2168 * 4 + 3) - 2014 * 4 + 1), |s| {
        let v: Vec<usize> = s.split(',').map(|x| x.trim().parse().expect("--region i0,j0,size")).collect();
        assert_eq!(v.len(), 3, "--region i0,j0,size");
        (v[0], v[1], v[2])
    });
    let margin: usize = get("--margin").map_or(200, |s| s.parse().expect("--margin M"));
    let threads: usize = get("--threads").map_or(4, |s| s.parse().expect("--threads T"));
    let out = PathBuf::from(get("--out").unwrap_or_else(|| "target/stores/g-rt-fixture".to_string()));
    let t0 = Instant::now();

    let def = WorldDefinition::single_planet(R, seed);
    let world = GlobalWorld::from_definition(&def).expect("the world of the definition");
    let inputs = class_inputs(&def, seed, threads);
    let region = (i0, i0 + size - 1, j0, j0 + size - 1);

    // The lattice rectangle of the window (a regional cell is 8 local cells, a node every 6 local cells).
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
    println!("lattice {nw} x {nh} nodes sampled ({:.1} s)", t0.elapsed().as_secs_f64());

    // The climate as the build gives it to H1: the runoff in m/s (without it Q is an area, not a discharge).
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
        // 98,304 local cells are 128 tiles of 768: a power of two, so the tiles and the graph blocks (96 H1 cells each) are filed under quadtree ids; 1,024 would be 96 tiles.
        tile: 768,
        halo: 64,
        rounds: 4,
        threads,
        res: LATTICE as u32,
        base_id: 0x6c61_7474_6963_6531 ^ (LATTICE as u64),
        inputs_id: class_inputs_key(&def, seed),
        // Earth's span (the default world's base heights plus the profile's largest uplift): its flood epsilon is the profile's, so the bake is the one it was.
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: false,
    };
    std::fs::create_dir_all(&out).expect("the output folder");
    let (commit, report) = bake_region(&world, &spec, &base, &climate, &out).unwrap_or_else(|e| panic!("the region bake: {e}"));
    println!(
        "MARK region bake: window {:?}, H1 {} channel cells in {} segments, {} lakes, {} tiles, commit {:016x}, graph blocks {} ({:.1} s)",
        report.window,
        report.channel_cells,
        report.segments,
        report.lakes,
        report.tiles,
        commit.id,
        commit.graph.len(),
        t0.elapsed().as_secs_f64()
    );
    std::fs::write(
        out.join("README.txt"),
        format!(
            "A G-RT FRAME-TIME FIXTURE, not the world bake (architect, 2026-11-12; GEN-177).\n\
             World: default single planet, seed {seed}, face {}, regional cells {:?} of {REGIONAL}, margin {margin}.\n\
             Commit {:016x}: {} L tiles, {} channel graph blocks. Rebuilt when the start-surface fix lands; never opened for writing by the viewer.\n\
             Made by `cargo run --release -p planet-cache --example bake_region`; the kernels are those of the commit that built it.\n",
            face.0, region, commit.id, report.tiles, commit.graph.len()
        ),
    )
    .expect("the README");
}
