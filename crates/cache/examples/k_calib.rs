//! The calibration of the carve's K scale `ROCK_K_SCALE` (the constant is the geometric mean over seeds of what this prints) (GEN-127): the factor c for which the land mean of g(c K f) equals the land mean of g(f), g(f) = f / (1 + f) (the share of the drop to the
//! receiver that one implicit step removes), K the default lithology's raw erodibility at the surface and f = k_dt sqrt(A) / dx the step's stream-power number of a cell of drainage area A.
//! `k_calib [--store DIR] SEED...`: for each seed the c over the parametric area distribution of the spec (P(A > a) = a^-0.43, 1 to 1e7 cells), and with `--store` (a pass-1 store of face 2 of seed 14
//! baked without the rock fields) the c over the face's real area codes.

use std::path::PathBuf;
use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_tiles::{bake_specs, BakeShape};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::face::area_cells;
use planet_generators::bake::inputs::BakeInputs;
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
const K_DT: f64 = 0.05;

fn g(f: f64) -> f64 {
    f / (1.0 + f)
}

fn solve(pairs: &[(f64, f64)]) -> f64 {
    planet_generators::lithology::k_scale_for(pairs)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let store = args.windows(2).find(|w| w[0] == "--store").map(|w| PathBuf::from(&w[1]));
    let seeds: Vec<u64> = args.iter().filter_map(|a| a.parse().ok()).collect();
    let (threads, _) = planet_streaming::topology::bake_threads(0).unwrap();
    for seed in seeds {
        let inputs = Arc::new(BakeInputs::new(seed, threads));
        let n = 200_000usize;
        let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
        let mut pairs = Vec::new();
        for i in 0..n {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt(1.0 - y * y);
            let (s, c) = libm::sincos(golden * i as f64);
            let d = Vec3::new(r * c, y, r * s);
            let z = inputs.base(d);
            if z > 0.0 {
                let u = (i as f64 * 0.618_033_988_749_895).fract();
                let area = libm::pow(1.0 - u, -1.0 / 0.43).min(1e7);
                pairs.push((inputs.lithology().k_at(d, z), K_DT * libm::sqrt(area)));
            }
        }
        let mean_k = pairs.iter().map(|p| p.0).sum::<f64>() / pairs.len() as f64;
        println!("seed {seed}: parametric areas: raw mean K {mean_k:.4}, c = {:.4} ({} land points)", solve(&pairs), pairs.len());
        if let (Some(dir), 14) = (&store, seed) {
            let def = WorldDefinition::single_planet(RADIUS_M, seed);
            let world = GlobalWorld::from_definition(&def).unwrap().with_class_source(inputs.clone());
            let shape = BakeShape::production(seed, RADIUS_M);
            let (layer, _) = bake_specs(&world, &shape).unwrap();
            let layer = planet_cache::layer_store::LayerSpec { rock: 0, ..layer };
            let st = TileStore::open(dir, world.params()).unwrap();
            // The store was made under the keys of another build: its record is read by whatever key it holds (the codes do not depend on the key).
            let tile = planet_cache::relief_store::face_tile(Face(2));
            let key =
                st.keys_for(planet_cache::layer_channels::KIND_LAYER_AREA, tile).into_iter().next().expect("an area record of face 2");
            let rec = st.get(planet_cache::layer_channels::KIND_LAYER_AREA, tile, key).unwrap().expect("the record");
            let codes = planet_cache::layer_channels::diagnostic_decode_unchecked(&rec, layer.cells).unwrap();
            let fs = planet_generators::bake::face::FaceSpec {
                face: Face(2),
                cells: layer.cells,
                radius_m: layer.radius_m,
                sea_level: layer.sea_level,
                seed: layer.seed,
                halo: 0,
            };
            let cells = layer.cells;
            let mut real = Vec::new();
            for j in (0..cells).step_by(12) {
                for i in (0..cells).step_by(12) {
                    let d = fs.direction(i, j);
                    let z = inputs.base(d);
                    if z > 0.0 {
                        real.push((inputs.lithology().k_at(d, z), K_DT * libm::sqrt(area_cells(codes[j * cells + i]))));
                    }
                }
            }
            let mean_g = real.iter().map(|p| g(p.1)).sum::<f64>() / real.len() as f64;
            let mean_k = real.iter().map(|p| p.0).sum::<f64>() / real.len() as f64;
            println!(
                "seed {seed}: real area codes of face 2: raw mean K {mean_k:.4}, mean g(f) {mean_g:.4}, c = {:.4} ({} land points)",
                solve(&real),
                real.len()
            );
        }
    }
}
