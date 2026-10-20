fn main() {
    let out = std::env::args().nth(1).expect("path");
    let def = planet::worldfield::legacy_world(1, 6_371_000.0);
    std::fs::write(&out, planet_world_def::toml_io::to_toml(&def).expect("toml")).expect("write");
}
