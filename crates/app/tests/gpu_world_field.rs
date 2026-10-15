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

fn view_args<'a>(w: &'a str, png: &'a str) -> Vec<&'a str> {
    vec![
        "test-render",
        "--scene",
        "terrain",
        "--script",
        "aerial-200km",
        "--view",
        "tile-id",
        "--adapter",
        "software",
        "--world",
        w,
        "--global-res",
        "32",
        "--out",
        png,
    ]
}

// spec: GEN-018
#[test]
fn a_streamed_world_run_draws_the_frame_of_the_synchronous_world_run() {
    let w = world("stream.toml", "");
    let (a, b) = (tmp("world-sync.png"), tmp("world-stream.png"));
    run(&view_args(w.to_str().unwrap(), a.to_str().unwrap())).unwrap();
    let mut args = view_args(w.to_str().unwrap(), b.to_str().unwrap());
    args.push("--stream");
    let log = run(&args).unwrap();
    assert!(log.contains("height field cpu-erosion-v1 over a 32-cell bake"), "{log}");
    assert_eq!(read_png(&a).unwrap(), read_png(&b).unwrap(), "the streaming path builds its meshes from the world's field");
    // And it is the eroded field, not the reference the seed-only run draws.
    let c = tmp("world-seed-only.png");
    run(&[
        "test-render",
        "--scene",
        "terrain",
        "--script",
        "aerial-200km",
        "--view",
        "tile-id",
        "--adapter",
        "software",
        "--out",
        c.to_str().unwrap(),
    ])
    .unwrap();
    assert_ne!(read_png(&a).unwrap(), read_png(&c).unwrap());
}

// spec: TEST-008, GEN-018
#[test]
fn a_world_bundle_replays_from_the_bundle_alone_and_refuses_an_edited_world() {
    let w = world("bundle.toml", "height_field = \"cpu-terrain-v1\"\n");
    let (png, bundle, again) = (tmp("wb1.png"), tmp("wb.repro"), tmp("wb2.png"));
    let mut args = view_args(w.to_str().unwrap(), png.to_str().unwrap());
    args.extend(["--bundle", bundle.to_str().unwrap()]);
    run(&args).unwrap();
    let text = std::fs::read_to_string(&bundle).unwrap();
    assert!(text.starts_with("planet-terrain-repro 2\n"), "{text}");
    assert!(text.contains("height_field cpu-terrain-v1\nglobal_res 32\nworld_toml "), "the bundle records the field and the bake");
    // The replay needs no world file: the bundle carries it.
    std::fs::remove_file(&w).unwrap();
    let log =
        run(&["test-render", "--adapter", "software", "--repro", bundle.to_str().unwrap(), "--out", again.to_str().unwrap()]).unwrap();
    assert!(log.contains("terrain plan identical"), "{log}");
    assert_eq!(read_png(&png).unwrap(), read_png(&again).unwrap(), "the replay must draw the recorded pixels");

    let replay_of = |name: &str, edited: String| {
        let bad = tmp(name);
        std::fs::write(&bad, edited).unwrap();
        run(&["test-render", "--adapter", "software", "--repro", bad.to_str().unwrap()]).unwrap_err()
    };
    // The recorded world is hashed against the recorded definition hash.
    let e = replay_of("wb-seed.repro", text.replacen("seed = 5", "seed = 6", 1));
    assert!(e.contains("its recorded world hashes to"), "{e}");
    // The recorded field id is checked against what the recorded world resolves to.
    let e = replay_of("wb-field.repro", text.replacen("height_field cpu-terrain-v1", "height_field cpu-erosion-v1", 1));
    assert!(e.contains("bundle records height field cpu-erosion-v1") && e.contains("cpu-terrain-v1"), "{e}");
    // The bake size is part of what is replayed: another size is another field, so the plan differs and says so.
    let e = replay_of("wb-res.repro", text.replacen("global_res 32", "global_res 16", 1));
    assert!(e.contains("differs from the recorded plan"), "{e}");
}
