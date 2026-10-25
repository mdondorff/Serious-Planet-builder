//! SPIKE-40 throwaway (copy to crates/app/examples/ to build; never merged). Read-only river statistics of the coupled run and of A-net chunks.
//! `cargo run --release -p planet --example hydro_targets -- --store DIR [--sites "lat,lon;lat,lon"]`
use planet::worldfield::WorldTerrain;
use planet_world_def::WorldDefinition;
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}
fn pct(v: &mut Vec<f64>, q: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() { f64::NAN } else { v[((v.len() as f64 - 1.0) * q).round() as usize] }
}
/// Least-squares slope and intercept of y on x, with the point count.
fn fit(xs: &[f64], ys: &[f64]) -> (f64, f64, usize) {
    let n = xs.len() as f64;
    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
    let sxy: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    (sxy / sxx, my - sxy / sxx * mx, xs.len())
}

fn main() {
    let store = std::path::PathBuf::from(arg("--store").expect("--store"));
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).unwrap().seed;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let world = Arc::new(WorldTerrain::open_store(def, &store, 16384).unwrap());
    if arg("--sites").is_none() || std::env::args().any(|x| x == "--coupled") {
        println!("{}", world.hydro_coupled_report().unwrap());
    }
    if let Some(s) = arg("--sites") {
        for site in s.split(';') {
            let (a, b) = site.split_once(',').unwrap();
            println!("{}", world.hydro_chunk_report(a.trim().parse().unwrap(), b.trim().parse().unwrap()).unwrap());
        }
    }
}
