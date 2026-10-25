//! M8-1 (c) timing: the coupled run made from a stored bake, timed. By default nothing is written (the store is only read); with `--write` the run
//! is loaded from the store when it is there and else made and stored beside the bake, as the viewer does (it takes the store's writer lock).
//! `cargo run --release -p planet --example time_coupled_make -- --store DIR --global-res N [--cells 2048] [--steps 80] [--write]`. Not production code.

use planet::coupledworld::{load_or_run, COUPLED_CELLS, COUPLED_STEPS};
use planet_cache::global_store::{GlobalWorld, StoreSource};
use planet_world_def::WorldDefinition;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() -> Result<(), String> {
    let dir = arg("--store").ok_or("--store DIR needed")?;
    let res: u32 = arg("--global-res").ok_or("--global-res N needed")?.parse().map_err(|e| format!("{e}"))?;
    let n: u32 = arg("--cells").map_or(Ok(COUPLED_CELLS.min(res)), |s| s.parse()).map_err(|e| format!("{e}"))?;
    let steps: u32 = arg("--steps").map_or(Ok(COUPLED_STEPS), |s| s.parse()).map_err(|e| format!("{e}"))?;
    let def = WorldDefinition::single_planet(6_371_000.0, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let src = StoreSource::new(&store, &world, res).map_err(|e| format!("{e}"))?;
    let t = std::time::Instant::now();
    let write = std::env::args().any(|a| a == "--write");
    let r = load_or_run(&def, write.then(|| std::path::Path::new(&dir)), &src, n, steps)?;
    println!(
        "{:?} {n} cells, {steps} steps from a bake of {res} in {:.1} s (stored: {}); block cache {:?}",
        r.origin,
        t.elapsed().as_secs_f64(),
        write && r.store_error.is_none(),
        src.stats()
    );
    if let Some(e) = &r.store_error {
        return Err(format!("the run is made but not stored: {e}"));
    }
    drop(r);
    Ok(())
}
