//! The cost of reading the baked channels at runtime (ground-channels-2b PR 1): the decode time and memory of a kind-14 face (the drainage area, pass 1's cells a face edge) and of a kind-15 tile (the
//! deposited depth, pass 2's tile) of a store, and of a windowed decode of the area (the runtime reader's). The records are read without the key check
//! (`diagnostic_decode_unchecked`), so a store made under the keys of another build can be measured; the codes do not depend on the key. A dev tool, the figures indicative (not a perf window).
//!
//! `cargo run --release -p planet --example channels_probe -- STORE FACE_CELLS PLANET_RES TILE [FACE [TI TJ]]` (the numbers are in the store's `layer-manifest.txt`: face_res, planet_res, tile)

use std::time::Instant;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_channels::{diagnostic_decode_unchecked, stream_window, KIND_LAYER_AREA, KIND_LAYER_DEPTH};
use planet_cache::layer_tiles::layer_tile_id;
use planet_cache::pack::TileStore;
use planet_cache::relief_store::face_tile;
use planet_core::Face;
use planet_generators::bake::tile::{TileGrid, TileId as PassTile};

fn peak_mb() -> f64 {
    planet_perf::streaming::query_process_peak_bytes().unwrap_or(0) as f64 / (1 << 20) as f64
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let (face_cells, planet_res, tile): (usize, usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap(), a[3].parse().unwrap());
    let face = Face(a.get(4).map_or(0, |v| v.parse().unwrap()));
    let per_face = planet_res / tile;
    let (ti, tj) = (a.get(5).map_or(per_face / 2, |v| v.parse().unwrap()), a.get(6).map_or(per_face / 2, |v| v.parse().unwrap()));
    let def = planet_world_def::WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 14);
    let legacy = GlobalWorld::from_definition(&def).expect("a world");
    let store = TileStore::open(&dir, legacy.params()).expect("the store");
    println!(
        "store {}: face {} of {face_cells} cells (a decoded face is {:.0} MB), tile ({ti}, {tj}) of {tile} cells ({:.1} MB)",
        dir.display(),
        face.0,
        (face_cells * face_cells) as f64 / 1e6,
        (tile * tile) as f64 / 1e6
    );

    // The face's area (kind 14).
    let keys = store.keys_for(KIND_LAYER_AREA, face_tile(face));
    println!("kind 14 keys for the face: {}", keys.len());
    let Some(&key) = keys.first() else { return println!("no area for this face") };
    let rec = store.get(KIND_LAYER_AREA, face_tile(face), key).expect("a read").expect("a record");
    println!("area record: {:.1} MB compressed", rec.payload.len() as f64 / 1e6);
    // The windowed decode (what the runtime reader does): a 257 x 257 window (256 cells and the one past) at the first, the middle and the last rows of the face, streamed from the record's start.
    let stream = &rec.payload[21..];
    for (name, y0) in [("first rows", 0), ("middle rows", face_cells / 2), ("last rows", face_cells - 257)] {
        let before = peak_mb();
        let t = Instant::now();
        let win = stream_window(stream, face_cells, y0..y0 + 257, 4000..4257).expect("a window");
        println!(
            "window at the {name}: {:.0} ms, {} bytes; process peak {before:.0} -> {:.0} MB",
            t.elapsed().as_secs_f64() * 1e3,
            win.len(),
            peak_mb()
        );
    }
    let mut plane;
    for run in 0..3 {
        let before = peak_mb();
        let t = Instant::now();
        plane = diagnostic_decode_unchecked(&rec, face_cells).expect("a decode");
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "area decode {run}: {ms:.0} ms ({:.0} MB/s decoded); process peak {before:.0} -> {:.0} MB",
            plane.len() as f64 / 1e6 / (ms / 1e3),
            peak_mb()
        );
    }

    // The tile's depth (kind 15).
    let grid = TileGrid { cells: planet_res, tile, halo: 128, radius_m: planet_core::EARTH_RADIUS_M };
    let id = PassTile { face, ti: ti as u32, tj: tj as u32 };
    // Which tiles of the face the store holds depth for (the baked region): the first one found stands in for a missing (ti, tj).
    let held: Vec<(usize, usize)> = (0..per_face)
        .flat_map(|j| (0..per_face).map(move |i| (i, j)))
        .filter(|&(i, j)| {
            layer_tile_id(&grid, PassTile { face, ti: i as u32, tj: j as u32 })
                .is_ok_and(|t| !store.keys_for(KIND_LAYER_DEPTH, t).is_empty())
        })
        .collect();
    println!("kind 15 tiles held on face {}: {} of {}", face.0, held.len(), per_face * per_face);
    let (ti, tj, id) = match (held.contains(&(ti, tj)), held.first()) {
        (false, Some(&(i, j))) => (i, j, PassTile { face, ti: i as u32, tj: j as u32 }),
        _ => (ti, tj, id),
    };
    println!("measuring tile ({ti}, {tj})");
    let tid = layer_tile_id(&grid, id).expect("a tile id");
    let keys = store.keys_for(KIND_LAYER_DEPTH, tid);
    println!("kind 15 keys for the tile: {}", keys.len());
    if let Some(&key) = keys.first() {
        let rec = store.get(KIND_LAYER_DEPTH, tid, key).expect("a read").expect("a record");
        println!("depth record: {:.2} MB compressed", rec.payload.len() as f64 / 1e6);
        let mut depth = Vec::new();
        for run in 0..3 {
            let t = Instant::now();
            depth = diagnostic_decode_unchecked(&rec, tile).expect("a decode");
            println!("depth decode {run}: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        }
        let nonzero = depth.iter().filter(|c| **c > 0).count();
        println!("depth tile: {nonzero} of {} cells deposited, max code {}", depth.len(), depth.iter().max().unwrap());
    }
}
