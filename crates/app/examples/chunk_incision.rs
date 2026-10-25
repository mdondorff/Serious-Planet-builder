//! Diagnostic, not production: the unclamped change of the A-net chunk solve at a point (see `WorldTerrain::chunk_incision_report`).
//! `cargo run --release -p planet --example chunk_incision -- --store DIR --lat DEG --lon DEG [--slump-rate R] [--steps N] [--route-off] [--river-area M2]`, or `--run-vs-base`.

use planet::worldfield::WorldTerrain;
use planet_world_def::WorldDefinition;
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let store = std::path::PathBuf::from(arg("--store").expect("--store"));
    let (lat, lon): (f64, f64) = (arg("--lat").map_or(0.0, |v| v.parse().unwrap()), arg("--lon").map_or(0.0, |v| v.parse().unwrap()));
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).unwrap().seed;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let world = Arc::new(WorldTerrain::open_store(def, &store, 16384).unwrap());
    if std::env::args().any(|x| x == "--run-vs-base") {
        println!("{}", world.run_against_base_report(8, 1e10).unwrap());
        return;
    }
    let slump_rate = arg("--slump-rate").map(|v| v.parse().expect("--slump-rate"));
    let steps = arg("--steps").map(|v| v.parse().expect("--steps"));
    println!(
        "{}",
        world
            .chunk_incision_report(
                lat,
                lon,
                slump_rate,
                steps,
                std::env::args().any(|x| x == "--route-off"),
                arg("--river-area").map(|v| v.parse().expect("--river-area"))
            )
            .unwrap()
    );
}
