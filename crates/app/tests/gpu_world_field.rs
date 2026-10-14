//! Tier C: `test-render --scene terrain --world` renders the world's field (GEN-018).

use planet_testkit::read_png;
use std::path::PathBuf;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("planet-world-field-gpu-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn world(name: &str, extra: &str) -> PathBuf {
    let p = tmp(name);
    std::fs::write(&p, format!("schema_version = 1\n[body.0]\nradius_m = 6371000.0\n[world]\n{extra}seed = 5\n")).unwrap();
    p
}

// spec: GEN-018
#[test]
fn the_terrain_scene_renders_the_field_the_world_names_and_the_fields_differ() {
    let mut frames = Vec::new();
    for (name, extra, id) in
        [("default.toml", "", "cpu-erosion-v1"), ("terrain.toml", "height_field = \"cpu-terrain-v1\"\n", "cpu-terrain-v1")]
    {
        let w = world(name, extra);
        let png = tmp(&format!("{name}.png"));
        let log = run(&[
            "test-render",
            "--scene",
            "terrain",
            "--script",
            "low-2km",
            "--view",
            "height",
            "--adapter",
            "software",
            "--world",
            w.to_str().unwrap(),
            "--global-res",
            "64",
            "--out",
            png.to_str().unwrap(),
        ])
        .unwrap();
        assert!(log.contains(&format!("height field {id} over a 64-cell bake")), "{log}");
        frames.push(read_png(&png).unwrap());
    }
    assert_ne!(frames[0], frames[1], "the eroded field and the terrain field are different terrain");
}
