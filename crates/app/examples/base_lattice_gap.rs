//! What a lattice of the bake's own base surface loses below its cells: `BakeInputs::base` (the class source, which the erosion layer was carved on) sampled at the vertices of a
//! lattice of `RES` cells a face edge and read back with the C1 interpolant, against `BakeInputs::base` itself, on land of a tile of the layer. A dev tool for the question
//! "how close is a runtime base read from a lattice to the surface the layer was carved on?"; not a test.
//!
//! `cargo run --release -p planet --example base_lattice_gap -- SEED FACE TILE_I TILE_J [RES ...]`

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::Face;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileGrid;
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};

struct Lattice<'a> {
    inputs: &'a BakeInputs,
    res: u32,
}

impl LatticeSource for Lattice<'_> {
    fn res(&self) -> u32 {
        self.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let n = f64::from(self.res);
        let d = face_to_direction(&TangentWarp, face, 2.0 * f64::from(i) / n - 1.0, 2.0 * f64::from(j) / n - 1.0);
        Ok(GlobalSample { continent: 0.0, elevation_m: self.inputs.base(d) as f32, temperature_c: 0.0, precipitation_mm: 0.0 })
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seed: u64 = a[0].parse().unwrap();
    let (face, ti, tj): (u8, usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap(), a[3].parse().unwrap());
    let resolutions: Vec<u32> = if a.len() > 4 { a[4..].iter().map(|r| r.parse().unwrap()).collect() } else { vec![8192, 16384, 32768] };
    let grid = TileGrid { cells: 98_304, tile: 3072, halo: 128, radius_m: 6_371_000.0 };
    let inputs = BakeInputs::new(seed, 8);
    for res in resolutions {
        let lattice = Lattice { inputs: &inputs, res };
        let mut gap = Vec::new();
        for j in (0..3072).step_by(24) {
            for i in (0..3072).step_by(24) {
                let d = grid.direction(Face(face), (ti * 3072 + i) as i64, (tj * 3072 + j) as i64);
                let exact = inputs.base(d);
                if exact <= 0.0 {
                    continue;
                }
                gap.push(f64::from(read_elevation_c1(&lattice, &TangentWarp, d).unwrap()) - exact);
            }
        }
        let rms = (gap.iter().map(|x| x * x).sum::<f64>() / gap.len() as f64).sqrt();
        let mut sorted = gap.clone();
        sorted.sort_by(f64::total_cmp);
        let q = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
        println!(
            "lattice {res} cells ({:.0} m): C1 read - BakeInputs::base over {} land samples: rms {rms:.2} m, p5 {:.2}, p50 {:.2}, p95 {:.2}, max |x| {:.1}",
            6_371_000.0 * core::f64::consts::FRAC_PI_2 / f64::from(res),
            gap.len(),
            q(0.05),
            q(0.5),
            q(0.95),
            gap.iter().fold(0.0f64, |m, x| m.max(x.abs()))
        );
    }
}
