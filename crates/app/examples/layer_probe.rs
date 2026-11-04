//! Where a tile of the erosion layer raises low ground: the cells of tile `FACE/TI/TJ` whose class-source base is within `MAX_E` metres above the sea and whose change is at least `MIN_C` metres,
//! with their count, the box they span and a few of them as latitude and longitude for the poses of a still. A dev tool for looking at a coastal band of the layer; not a test.
//!
//! `cargo run --release -p planet --example layer_probe -- STORE SEED FACE/TI/TJ [MAX_E MIN_C]`

use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_tiles::{BakeShape, ShapeOverride, StoreLayer};
use planet_core::Face;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileGrid;
use planet_generators::erosion_layer::LayerSource;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (dir, seed) = (std::path::PathBuf::from(&a[0]), a[1].parse::<u64>().unwrap());
    let t: Vec<usize> = a[2].split('/').map(|v| v.parse().unwrap()).collect();
    let max_e: f64 = a.get(3).map_or(10.0, |v| v.parse().unwrap());
    let min_c: f64 = a.get(4).map_or(1.0, |v| v.parse().unwrap());
    let def = planet_world_def::WorldDefinition::single_planet(6_371_000.0, seed);
    let world = GlobalWorld::for_store(&def, &dir, 16384, None, 8).expect("the store's world");
    let (layer, manifest, shape) = StoreLayer::open_recorded(
        &dir,
        Arc::new(world),
        &BakeShape::production(seed, 6_371_000.0),
        ShapeOverride::default(),
        4,
    )
    .expect("the layer");
    println!("layer: shape {shape:?}, manifest {}, range {:?}", manifest.is_some(), layer.global_range());
    let tile = layer.tile(Face(t[0] as u8), t[1] as u32, t[2] as u32).unwrap().expect("a tile");
    let inputs = BakeInputs::new(seed, 8);
    let grid = TileGrid { cells: shape.planet_res, tile: 3072, halo: 128, radius_m: 6_371_000.0 };
    let mut hits: Vec<(f64, f64, f64, f64)> = Vec::new();
    for j in (0..3072).step_by(6) {
        for i in (0..3072).step_by(6) {
            let c = f64::from(tile.cell(i, j));
            if c < min_c {
                continue;
            }
            let d = grid.direction(Face(t[0] as u8), (t[1] * 3072 + i) as i64, (t[2] * 3072 + j) as i64);
            let e = inputs.base(d);
            if e > 0.0 && e < max_e {
                hits.push((libm::asin(d.z).to_degrees(), libm::atan2(d.y, d.x).to_degrees(), e, c));
            }
        }
    }
    // The change over all low land of the tile by base height: what the layer does to a coastal plain.
    let mut bins: [Vec<f64>; 4] = Default::default();
    for j in (0..3072).step_by(6) {
        for i in (0..3072).step_by(6) {
            let d = grid.direction(Face(t[0] as u8), (t[1] * 3072 + i) as i64, (t[2] * 3072 + j) as i64);
            let e = inputs.base(d);
            let b = if e <= 0.0 { continue } else if e < 3.0 { 0 } else if e < 6.0 { 1 } else if e < 10.0 { 2 } else if e < 30.0 { 3 } else { continue };
            bins[b].push(f64::from(tile.cell(i, j)));
        }
    }
    for (name, v) in ["0-3 m", "3-6 m", "6-10 m", "10-30 m"].iter().zip(bins.iter_mut()) {
        if v.is_empty() {
            continue;
        }
        v.sort_by(f64::total_cmp);
        let q = |p: f64| v[((v.len() - 1) as f64 * p) as usize];
        println!("base {name}: {} cells, change p5 {:.1} p50 {:.1} p95 {:.1} m", v.len(), q(0.05), q(0.5), q(0.95));
    }
    println!("{} cells (every 6th) with the base in (0, {max_e}) m and a change of at least {min_c} m", hits.len());
    if hits.is_empty() {
        return;
    }
    let (la, lo): (Vec<f64>, Vec<f64>) = hits.iter().map(|h| (h.0, h.1)).unzip();
    let ext = |v: &[f64]| (v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    println!("box: lat {:?}, lon {:?}", ext(&la), ext(&lo));
    hits.sort_by(|x, y| (x.0 + x.1).total_cmp(&(y.0 + y.1)));
    for k in 0..8 {
        let h = hits[k * (hits.len() - 1) / 7];
        println!("sample {k}: lat {:.4} lon {:.4}, base {:.1} m, change {:.1} m", h.0, h.1, h.2, h.3);
    }
}
