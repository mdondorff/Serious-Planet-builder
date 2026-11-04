//! The gap between the surface the bake carved (`BakeInputs::base`, the class source) and the surface the runtime field reads (the global bake lattice, `global::sample`, read C1) on a
//! tile of the layer: rms and percentiles over land cells, for a lattice of `RES` cells a face edge computed on demand (a pure function of the direction). A dev tool for the question
//! "what does the erosion layer add to?"; not a test.
//!
//! `cargo run --release -p planet --example base_gap -- SEED FACE TILE_I TILE_J [RES]`

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::Face;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileGrid;
use planet_generators::classes::{ClassLattice, ClassSource, CLASS_LATTICE_N};
use planet_generators::global::{read_elevation_c1, sample, GlobalSample, LatticeSource, ReadError};
use planet_generators::relief_channels::ReliefChannels;
use planet_generators::relief_lattice::ReliefLattice;

struct OnDemand {
    seed: u64,
    res: u32,
}

impl LatticeSource for OnDemand {
    fn res(&self) -> u32 {
        self.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let n = f64::from(self.res);
        Ok(sample(self.seed, face_to_direction(&TangentWarp, face, 2.0 * f64::from(i) / n - 1.0, 2.0 * f64::from(j) / n - 1.0)))
    }
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p) as usize]
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seed: u64 = a[0].parse().unwrap();
    let (face, ti, tj): (u8, usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap(), a[3].parse().unwrap());
    let res: u32 = a.get(4).map_or(16384, |r| r.parse().unwrap());
    let grid = TileGrid { cells: 98_304, tile: 3072, halo: 128, radius_m: 6_371_000.0 };
    let inputs = BakeInputs::new(seed, 8);
    let lattice = OnDemand { seed, res };
    // The relief channels `k` and `m` on a lattice of RELIEF_CELLS a face edge (GEN-099 wants 2048; they are smooth, so a coarser lattice is an estimate), from the bake's elevation and the class weights.
    let cells: usize = std::env::var("RELIEF_CELLS").ok().and_then(|v| v.parse().ok()).unwrap_or(512);
    let classes = ClassLattice::build_with(&ClassSource::new(seed), CLASS_LATTICE_N, 8);
    let surface = |d: planet_core::Vec3| f64::from(sample(seed, d).elevation_m);
    let relief = ReliefLattice::new(ReliefChannels::compute(&classes, &surface, 6_371_000.0, cells, 8)).unwrap();
    let mut stretched = Vec::new();
    let (mut gen, mut lat, mut tot, mut tot_abs) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let step = 24;
    for j in (0..3072).step_by(step) {
        for i in (0..3072).step_by(step) {
            let d = grid.direction(Face(face), (ti * 3072 + i) as i64, (tj * 3072 + j) as i64);
            let bake_base = inputs.base(d);
            if bake_base <= 0.0 {
                continue;
            }
            let exact = f64::from(sample(seed, d).elevation_m);
            let runtime = f64::from(read_elevation_c1(&lattice, &TangentWarp, d).unwrap());
            gen.push(exact - bake_base);
            lat.push(runtime - exact);
            tot.push(runtime - bake_base);
            tot_abs.push((runtime - bake_base).abs());
            let (k, m) = relief.at(d);
            stretched.push(m + k * (runtime - m) - bake_base);
        }
    }
    let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
    println!(
        "seed {seed} tile {face}/{ti}/{tj}, lattice {res} cells ({:.0} m), {} land samples",
        6_371_000.0 * core::f64::consts::FRAC_PI_2 / f64::from(res),
        tot.len()
    );
    for (name, v) in [
        ("generator gap  (global::sample - BakeInputs::base)", &mut gen),
        ("lattice gap    (C1 read - global::sample)", &mut lat),
        ("runtime base - BakeInputs::base", &mut tot),
    ] {
        println!(
            "{name}: rms {:.1} m, p5 {:.1}, p50 {:.1}, p95 {:.1}, max |x| {:.1}",
            rms(v),
            pct(v, 0.05),
            pct(v, 0.5),
            pct(v, 0.95),
            v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
        );
    }
    let mut st = stretched.clone();
    println!(
        "m + k (e - m) - BakeInputs::base ({cells}-cell channels): rms {:.1} m, p5 {:.1}, p50 {:.1}, p95 {:.1}, max |x| {:.1}",
        rms(&st),
        pct(&mut st, 0.05),
        pct(&mut st, 0.5),
        pct(&mut st, 0.95),
        stretched.iter().fold(0.0f64, |m, x| m.max(x.abs()))
    );
    println!("mean |runtime - bake base|: {:.1} m", tot_abs.iter().sum::<f64>() / tot_abs.len() as f64);
}
