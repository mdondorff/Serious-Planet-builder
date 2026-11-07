//! Pits of the raw base surface and of the surface after pass 1 (inbox #96, probe, not a product command): `pit_compare PASS1_DIR TI TJ` reads the face-2 pass-1 store of a `bake layer --class-source`
//! run and prints, for the footprint of pass-2 tile (2, TI, TJ) and for the whole face, the pits (cells more than 0.5 m under the flood) of the base and of base + change: share of land, depth p50/p95,
//! deepest and the volume.

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::load_face_change;
use planet_cache::layer_tiles::{bake_specs, BakeShape};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::face::{sample_base, FaceSpec};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::inputs::BakeInputs;
use planet_world_def::WorldDefinition;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let (ti, tj): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let seed = 14u64;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let inputs = std::sync::Arc::new(BakeInputs::new(seed, 10));
    let world = GlobalWorld::from_definition(&def).unwrap().with_class_source(inputs.clone());
    let shape = BakeShape { seed, radius_m: 6_371_000.0, face_res: 12_288, planet_res: 98_304, tile: None, halo: None };
    let (layer, _) = bake_specs(&world, &shape).unwrap();
    let spec = FaceSpec { face: Face(2), cells: layer.cells, radius_m: layer.radius_m, sea_level: layer.sea_level, seed: layer.seed, halo: layer.halo };
    let base = |d: Vec3| inputs.base(d);
    let z0 = sample_base(&spec, &base, 10);
    let store = TileStore::open(&dir.join("layer"), world.params()).unwrap();
    let change = load_face_change(&store, &world, 12_288, &layer, Face(2)).unwrap().expect("a pass-1 face");
    let carved: Vec<f64> = z0.iter().zip(&change.change).map(|(b, c)| b + f64::from(*c)).collect();
    let grid = planet_generators::bake::Grid::new(spec.cells + 2 * spec.halo, spec.cells + 2 * spec.halo, spec.dx());
    let m = spec.cells + 2 * spec.halo;
    let ratio = 98_304 / 12_288;
    let (tile_cells, h) = (3072 / ratio, spec.halo);
    let (i0, j0) = (ti * tile_cells + h, tj * tile_cells + h);
    for (name, z) in [("raw base", &z0), ("after pass 1 (G=0)", &carved)] {
        let outlet = grid.outlets(z, spec.sea_level);
        let filled = priority_flood(&grid, z, &outlet, 1e-3);
        for (scope, rows, cols) in [
            (format!("tile 2/{ti}/{tj}"), j0..j0 + tile_cells, i0..i0 + tile_cells),
            ("whole face".to_string(), h..h + spec.cells, h..h + spec.cells),
        ] {
            let (mut land, mut depths, mut vol) = (0usize, Vec::new(), 0.0f64);
            for j in rows.clone() {
                for i in cols.clone() {
                    let c = j * m + i;
                    if z[c] > spec.sea_level {
                        land += 1;
                    }
                    let d = filled[c] - z[c];
                    if d > 0.5 {
                        depths.push(d);
                        vol += d * grid.dx() * grid.dx();
                    }
                }
            }
            depths.sort_by(f64::total_cmp);
            let q = |p: usize| depths.get((depths.len().max(1) - 1) * p / 100).copied().unwrap_or(0.0);
            println!(
                "{name:20} {scope:14}: {} pit cells ({:.1} % of {land} land), depth p50 {:.1} m, p95 {:.1} m, deepest {:.1} m, volume {:.3e} m3",
                depths.len(),
                100.0 * depths.len() as f64 / land.max(1) as f64,
                q(50),
                q(95),
                depths.last().copied().unwrap_or(0.0),
                vol
            );
        }
    }
}
