use planet::worldfield::WorldTerrain;
use planet_world_def::WorldDefinition;
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let store = std::path::PathBuf::from(arg("--store").expect("--store"));
    let out = std::path::PathBuf::from(arg("--out").expect("--out"));
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).unwrap().seed;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let world = Arc::new(WorldTerrain::open_store(def, &store, 16384).unwrap());
    if std::env::args().any(|x| x == "--candidates") {
        println!("{}", world.route_candidates().unwrap());
        return;
    }
    for site in arg("--sites").expect("--sites tag:lat,lon;...").split(';') {
        let (tag, ll) = site.split_once(':').unwrap();
        let (a, b) = ll.split_once(',').unwrap();
        println!("{}", world.route_dump(a.trim().parse().unwrap(), b.trim().parse().unwrap(), &out, tag).unwrap());
    }
}
