//! Wall time of the erosion field's height with and without the landform term (GEN-061), on the same samples: a plain timing, not a perf measurement.
//! `cargo run --release -p planet-generators --example erosion_cost -- [--res N] [--samples N]`.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::Vec3;
use planet_generators::erosion::ErosionGenerator;
use planet_generators::global::{bake, GridSource};
use planet_generators::height::HeightField;
use planet_world_def::{BodyId, GenContext, WorldDefinition};
use std::time::Instant;

fn arg(name: &str, default: u64) -> u64 {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).map_or(default, |v| v.parse().expect("a number"))
}

fn main() {
    let (res, n) = (arg("--res", 512) as u32, arg("--samples", 200_000) as usize);
    let def = WorldDefinition::single_planet(6_371_000.0, 7);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    let started = Instant::now();
    let grid = bake(seed, &TangentWarp, res, 8).unwrap();
    println!("bake of {res} cells per face edge in {:.1} s", started.elapsed().as_secs_f64());
    let src = GridSource(&grid);
    let field = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [9; 16]).unwrap();
    let mut rng = SplitMix64(31);
    let dirs: Vec<Vec3> = (0..n)
        .map(|_| Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized())
        .collect();
    // Warm up, then three alternating passes each.
    let sum = |f: &dyn Fn(Vec3) -> f64| dirs.iter().map(|&d| f(d)).sum::<f64>();
    let _ = (sum(&|d| field.height(d)), sum(&|d| field.height_without_landform(d)));
    let (mut with, mut without) = (f64::MAX, f64::MAX);
    for _ in 0..3 {
        let t = Instant::now();
        let a = sum(&|d| field.height(d));
        with = with.min(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = sum(&|d| field.height_without_landform(d));
        without = without.min(t.elapsed().as_secs_f64());
        std::hint::black_box((a, b));
    }
    println!(
        "{n} heights: with the landform {:.3} s ({:.2} us each), without {:.3} s ({:.2} us each): +{:.1} %",
        with,
        1e6 * with / n as f64,
        without,
        1e6 * without / n as f64,
        100.0 * (with / without - 1.0)
    );
}
