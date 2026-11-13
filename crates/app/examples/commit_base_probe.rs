//! Where does the viewer's ground under `--commit` differ from the bake's base B plus the stored change L (VIEW-072)? Read-only over a commit folder.
//!
//! For the cells of a square around a local cell of face 1 it prints, against B(bake) (the C1 read of the 16,384-node lattice of `BakeInputs::base`, exactly `bake_region`'s `base`):
//! the viewer's base (`WorldTerrain::base_elevation_c1`), the exact `BakeInputs::base`, the viewer's drawn ground (`field.height`) and B + L. A dev tool; not a test.
//!
//! `cargo run --release -p planet --example commit_base_probe -- COMMIT_DIR [X Y [RADIUS]] [--threads T]`

use std::path::Path;
use std::sync::Arc;

use planet_cache::global_store::{class_inputs, GlobalWorld};
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;

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

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p) as usize]
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(&args[0]);
    let nums: Vec<i64> = args[1..].iter().take_while(|a| !a.starts_with("--")).map(|s| s.parse().expect("X Y [RADIUS]")).collect();
    let (sx, sy, radius) =
        (nums.first().copied().unwrap_or(68_728), nums.get(1).copied().unwrap_or(5_536), nums.get(2).copied().unwrap_or(12));
    let threads: usize = args.iter().position(|a| a == "--threads").map_or(8, |k| args[k + 1].parse().expect("--threads T"));
    let (seed, face, margin) = (14u64, Face(1), 200usize);
    let def = WorldDefinition::single_planet(R, seed);
    let inputs = class_inputs(&def, seed, threads);

    // The bake's lattice rectangle (examples/bake_region.rs): a node every 4/3 regional cells, three nodes before and four after the window.
    let (i0, j0, size) = (2014 * 4, 190 * 4, (2168 * 4 + 3) - 2014 * 4 + 1);
    let (ri0, ri1, rj0, rj1) = (i0 - margin, i0 + size - 1 + margin, j0 - margin, j0 + size - 1 + margin);
    let to_node = |reg: usize| reg * LATTICE / REGIONAL;
    let (ni0, nj0) = (to_node(ri0) - 3, to_node(rj0) - 3);
    let (nw, nh) = (to_node(ri1) + 4 - ni0, to_node(rj1) + 4 - nj0);
    let dir_of_node = |i: usize, j: usize| planet_cache::region_bake::lattice_direction(face, LATTICE, i, j);
    let z: Vec<f32> = planet_core::par::map(&threads, nw * nh, &|c| inputs.base(dir_of_node(ni0 + c % nw, nj0 + c / nw)) as f32);
    let lattice = Lattice { face, i0: ni0, j0: nj0, w: nw, h: nh, z };
    let bake_b = |d: Vec3| f64::from(read_elevation_c1(&lattice, &TangentWarp, d).expect("a node of the sampled rectangle"));

    // The stored change L of the commit.
    let commit = planet_cache::l_writer::Commit::read(dir)?.ok_or("no commit record")?;
    let n_local = commit.spec.cells as f64;
    let stored = Arc::new(GlobalWorld::from_definition(&def).map_err(|e| format!("{e:?}"))?);
    let layer_src: Arc<dyn planet_generators::erosion_layer::LayerSource> =
        Arc::new(planet_cache::l_layer::CommitLayer::open(dir, stored, 16)?);
    let layer = planet_generators::erosion_layer::ErosionLayer::new(layer_src)?;

    // The viewer's world and field.
    let world = planet::worldfield::WorldTerrain::open_commit_in_memory(def.clone(), dir, threads)?;
    println!("{}", world.base_note());
    let field = world.field()?;

    let (mut d_base, mut d_exact, mut d_ground, mut d_band) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for y in sy - radius..=sy + radius {
        for x in sx - radius..=sx + radius {
            let d = face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n_local - 1.0, 2.0 * (y as f64 + 0.5) / n_local - 1.0);
            let b = bake_b(d);
            let l = layer.change_at(d)?;
            let b_viewer = world.base_elevation_c1(d).ok_or("no base read")?;
            let exact = inputs.base(d);
            let ground = field.height(d);
            d_base.push(b_viewer - b);
            d_exact.push(exact - b);
            d_ground.push(ground - (b + l));
            d_band.push(ground - (b_viewer + l));
            if (x - sx).abs() <= 1 && (y - sy).abs() <= 1 {
                println!("({x},{y}) B bake {b:.3}  B viewer {b_viewer:.3}  exact base {exact:.3}  L {l:.3}  B+L {:.3}  viewer ground {ground:.3}", b + l);
            }
        }
    }
    for (name, v) in [
        ("viewer base - bake B", &mut d_base),
        ("exact BakeInputs::base - bake B (what the C1 lattice read loses)", &mut d_exact),
        ("viewer ground - (bake B + L)", &mut d_ground),
        ("viewer ground - (viewer base + L)  (the terms on top of the base)", &mut d_band),
    ] {
        let (min, p50, p99, max) = (pct(v, 0.0), pct(v, 0.5), pct(v, 0.99), pct(v, 1.0));
        let abs_max = v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        println!("{name}: min {min:.3} p50 {p50:.3} p99 {p99:.3} max {max:.3} m (largest magnitude {abs_max:.3}) over {} cells", v.len());
    }
    Ok(())
}
