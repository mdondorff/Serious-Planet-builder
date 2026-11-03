//! Staircase diagnosis (m9-bake preview, diagnosis branch only): pass 1 of face 2 at `--face-res` cells with the experiment switches, then pass 2 of tile 2/12/10 on top of it, the
//! core written as raw f32 for `stair_metric`. No store. `stair_exp --face-res N --out FILE [--mfd-area] [--jitter F] [--c1-face-read] [--smooth-flow N] [--smooth-iso N] [--halo N]`.

use planet_cache::layer_tiles::face_change_reader;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::experiment as ex;
use planet_generators::bake::face::{pass1_face, FaceChange, FaceSpec};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::{pass2_tile, Pass2Params, TileGrid, TileId};

const R: f64 = 6_371_000.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let val = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let flag = |name: &str| args.iter().any(|a| a == name);
    let res: usize = val("--face-res").map_or(4096, |v| v.parse().unwrap());
    let out = val("--out").expect("--out FILE (with --tiles, a directory: tile-F-I-J.f32 files)");
    let face_no: u8 = val("--face").map_or(2, |v| v.parse().unwrap());
    let tiles: Vec<(u32, u32)> = val("--tiles").map_or(vec![(12, 10)], |v| v.split(';').map(|t| { let (a, b) = t.split_once(',').expect("TI,TJ"); (a.parse().unwrap(), b.parse().unwrap()) }).collect());
    if let Some(j) = val("--jitter") {
        ex::set_jitter_amp(j.parse().unwrap());
    }
    ex::set_mfd_area(flag("--mfd-area"));
    ex::set_c1_face_read(flag("--c1-face-read"));
    ex::set_smooth_flow(val("--smooth-flow").map_or(0, |v| v.parse().unwrap()));
    ex::set_smooth_iso(val("--smooth-iso").map_or(0, |v| v.parse().unwrap()));
    let threads = planet_streaming::topology::bake_threads(0).expect("topology").0;
    let inputs = BakeInputs::new(14, threads);
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    let halo = val("--halo").map_or(res.div_ceil(128).max(4), |v| v.parse().unwrap());
    let t0 = std::time::Instant::now();
    // Pass 1 with the experiment switches ON for face 2 only (the other faces are zero: the tile is far from the edges).
    let spec = FaceSpec { face: Face(face_no), cells: res, radius_m: R, sea_level: 0.0, seed: 14, halo };
    let face2 = pass1_face(&spec, &base, Some(&factor), &CarveParams::default(), threads);
    println!("pass 1 face {face_no} at {res} cells ({:.0} m): {:.1} s", R * core::f64::consts::FRAC_PI_2 / res as f64, t0.elapsed().as_secs_f64());
    let faces: Vec<FaceChange> = (0..6u8)
        .map(|f| if f == face_no { face2.clone() } else { FaceChange { face: Face(f), cells: res, halo, change: vec![0.0; face2.change.len()] } })
        .collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    let reader = face_change_reader(&faces);
    let grid = TileGrid { cells: 98_304, tile: 3072, halo: 256, radius_m: R };
    std::fs::create_dir_all(&out).expect("output directory");
    for (ti, tj) in tiles {
        let t1 = std::time::Instant::now();
        let r = pass2_tile(&grid, TileId { face: Face(face_no), ti, tj }, &base, &reader, Some(&factor), &Pass2Params::standard(14, 0.0), threads);
        println!(
            "pass 2 tile {face_no}/{ti}/{tj}: {:.1} s, thermal {} passes, {} still moving at the cap",
            t1.elapsed().as_secs_f64(),
            r.stats.thermal_passes,
            r.stats.thermal_moving_at_cap
        );
        let bytes: Vec<u8> = r.core.iter().flat_map(|v| v.to_le_bytes()).collect();
        let file = format!("{out}/tile-{face_no}-{ti}-{tj}.f32");
        std::fs::write(&file, bytes).expect("write");
        println!("wrote {file}");
    }
}
