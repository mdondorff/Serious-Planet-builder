//! The preview of the carve's rock fields (GEN-127, plan PR 4): pass 1 of face 2 of seed 14 at the production grid with the lithology erodibility and the uplift field (`--no-rock`: today's carve, the
//! baseline), pass 2 on tile 2/12/10, then the relief per class on the spike's measure (p95 - p5 in blocks at least 95 % land and 90 % one class) and the elevation per class of the tile.
//! `--out DIR [--no-rock]` bakes and measures; `--stats-only TILEDIR` measures a stored tile. The tile core is also written to `DIR/tile-2-12-10.f32` (the export format of the stills tool).

use std::path::PathBuf;
use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::{load_face_change, run_pass1_rock};
use planet_cache::layer_tiles::{bake_specs, face_change_reader, load_tile, run_pass2, BakeShape, Pass2Inputs};
use planet_cache::pack::TileStore;
use planet_core::par::{self, Par};
use planet_core::{Face, Vec3};
use planet_generators::bake::face::{FaceChange, RockFields};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileId as PassTile;
use planet_generators::classes::{CLASS_COUNT, CLASS_NAMES};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;

fn quantile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.windows(2).find(|w| w[0] == k).map(|w| w[1].clone());
    let seed = 14u64;
    let (threads, _) = planet_streaming::topology::bake_threads(0).unwrap();
    let def = WorldDefinition::single_planet(RADIUS_M, seed);
    let inputs = Arc::new(BakeInputs::new(seed, threads));
    let world = GlobalWorld::from_definition(&def).unwrap().with_class_source(inputs.clone());
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    let shape = BakeShape::production(seed, RADIUS_M);
    let (layer, spec) = bake_specs(&world, &shape).unwrap();
    let (res, rp) = (shape.face_res as u32, shape.planet_res as u32);
    let id = PassTile { face: Face(2), ti: 12, tj: 10 };
    let tiles_dir: PathBuf = if let Some(d) = arg("--stats-only") {
        PathBuf::from(d)
    } else {
        let out = PathBuf::from(arg("--out").expect("--out DIR"));
        let no_rock = args.iter().any(|a| a == "--no-rock");
        let (rock_k, rock_u) = (|d: Vec3, z: f64| inputs.rock_k(d, z), |d: Vec3| inputs.uplift_at(d));
        let rock = if no_rock { RockFields::none() } else { RockFields { k: Some(&rock_k), uplift: Some(&rock_u) } };
        println!("rock fields: {}", if no_rock { "off (today's carve)" } else { "on (lithology K and the uplift field)" });
        let clock = std::time::Instant::now();
        run_pass1_rock(&out, &world, res, &layer, &base, Some(&factor), &rock, &[Face(2)], threads).unwrap();
        println!("pass 1 of face 2: {:.0} s", clock.elapsed().as_secs_f64());
        let p1 = TileStore::open(&out, world.params()).unwrap();
        let changes: Vec<FaceChange> = (0..6u8)
            .map(|f| {
                if f == 2 {
                    load_face_change(&p1, &world, res, &layer, Face(f)).unwrap().unwrap()
                } else {
                    let m = layer.cells + 2 * layer.halo;
                    FaceChange { face: Face(f), cells: layer.cells, halo: layer.halo, change: vec![0.0; m * m] }
                }
            })
            .collect();
        let changes: [FaceChange; 6] = changes.try_into().unwrap();
        let reader = face_change_reader(&changes);
        let tiles = out.join("tiles");
        run_pass2(
            &tiles,
            &world,
            rp,
            &spec,
            &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor), scratch: None, rock },
            &[id],
            threads,
            &mut |_| {},
        )
        .unwrap();
        println!("pass 2 done: {:.0} s", clock.elapsed().as_secs_f64());
        tiles
    };
    let store = TileStore::open(&tiles_dir, world.params()).unwrap();
    let core = load_tile(&store, &world, rp, &spec, id).unwrap().expect("the tile is in the store").core;
    if let Some(out) = arg("--out") {
        let raw: Vec<u8> = core.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(PathBuf::from(out).join("tile-2-12-10.f32"), raw).unwrap();
    }
    let g = spec.grid;
    let t = g.tile;
    let par: &dyn Par = &threads;
    let rows: Vec<Vec<(f64, u8)>> = par::map(par, t, &|j| {
        (0..t)
            .map(|i| {
                let d = g.direction(id.face, (id.ti as usize * t + i) as i64, (id.tj as usize * t + j) as i64);
                let w = inputs.weights_at(d);
                let c = (0..CLASS_COUNT).fold(0, |b, k| if w[k] > w[b] { k } else { b });
                (base(d) + f64::from(core[j * t + i]), c as u8)
            })
            .collect()
    });
    let z: Vec<f64> = rows.iter().flatten().map(|v| v.0).collect();
    let class: Vec<u8> = rows.iter().flatten().map(|v| v.1).collect();
    // The same relief on the base alone and on the filled surface (the pit depth added), to see what the layer and the fill add.
    let basez: Vec<f64> = z.iter().zip(core.iter()).map(|(s, c)| s - f64::from(*c)).collect();
    let fill = planet_cache::layer_fill::load_fill(&store, &world, rp, &spec, id).ok().flatten();
    let relief = |name: &str, surf: &[f64]| {
        for (km, b) in [("3 km", 30usize), ("10 km", 98usize)] {
            let mut per = spike_relief(surf, &class, t, b);
            for (k, v) in per.iter_mut().enumerate() {
                if v.len() >= 5 {
                    println!(
                        "  {km} relief (p95-p5, the spike's measure) on {name:<14} {:<16} blocks {:>4} p50 {:>7.1} m p90 {:>7.1} m",
                        CLASS_NAMES[k],
                        v.len(),
                        quantile(v, 0.5),
                        quantile(v, 0.9)
                    );
                }
            }
        }
    };
    println!(
        "
surface comparison (3 km blocks, class of the block's centre)"
    );
    relief("the base alone", &basez);
    relief("base + change", &z);
    if let Some(f) = &fill {
        let filled: Vec<f64> = z.iter().zip(f.iter()).map(|(s, c)| s + planet_generators::bake::face::fill_m(*c)).collect();
        relief("filled surface", &filled);
    }
    println!("\nrelief per class on 2/12/10 (surface = base + stored change, unfilled; blocks of 30 and 98 cells = 3 km and 10 km, the class of the block's centre)");
    for (name, b) in [("3 km", 30usize), ("10 km", 98usize)] {
        let mut per: Vec<Vec<f64>> = vec![Vec::new(); CLASS_COUNT];
        for bj in 0..t / b {
            for bi in 0..t / b {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for j in bj * b..(bj + 1) * b {
                    for i in bi * b..(bi + 1) * b {
                        let v = z[j * t + i];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                if lo > 0.0 {
                    per[class[(bj * b + b / 2) * t + bi * b + b / 2] as usize].push(hi - lo);
                }
            }
        }
        for (k, v) in per.iter_mut().enumerate() {
            if v.len() >= 5 {
                println!(
                    "  {name} relief {:<16} blocks {:>5}  p50 {:>7.1} m  p90 {:>7.1} m",
                    CLASS_NAMES[k],
                    v.len(),
                    quantile(v, 0.5),
                    quantile(v, 0.9)
                );
            }
        }
    }
    println!("\nelevation per class on 2/12/10 (land cells)");
    let mut per: Vec<Vec<f64>> = vec![Vec::new(); CLASS_COUNT];
    for p in 0..t * t {
        if z[p] > 0.0 {
            per[class[p] as usize].push(z[p]);
        }
    }
    for (k, v) in per.iter_mut().enumerate() {
        if v.len() > 1000 {
            println!(
                "  {:<16} cells {:>8}  p5 {:>7.1}  p50 {:>7.1}  p95 {:>7.1}  p99.9 {:>7.1}  max {:>7.1} m",
                CLASS_NAMES[k],
                v.len(),
                quantile(v, 0.05),
                quantile(v, 0.5),
                quantile(v, 0.95),
                quantile(v, 0.999),
                quantile(v, 1.0)
            );
        }
    }
}

/// The spike's measure (SPIKE-99 `relief_stats`): the local relief p95 - p5 of the heights in a block of `b` cells, over blocks that are at least 95 % land (above 1 m) and at least 90 % one class, per class.
fn spike_relief(surf: &[f64], class: &[u8], t: usize, b: usize) -> Vec<Vec<f64>> {
    let mut per: Vec<Vec<f64>> = vec![Vec::new(); CLASS_COUNT];
    for bj in 0..t / b {
        for bi in 0..t / b {
            let mut heights = Vec::with_capacity(b * b);
            let mut counts = [0usize; CLASS_COUNT];
            for j in bj * b..(bj + 1) * b {
                for i in bi * b..(bi + 1) * b {
                    heights.push(surf[j * t + i]);
                    counts[class[j * t + i] as usize] += 1;
                }
            }
            let land = heights.iter().filter(|h| **h > 1.0).count();
            let (k, top) = counts.iter().enumerate().max_by_key(|(_, c)| **c).map(|(k, c)| (k, *c)).unwrap();
            if land * 100 >= heights.len() * 95 && top * 10 >= heights.len() * 9 {
                per[k].push(quantile(&mut heights.clone(), 0.95) - quantile(&mut heights, 0.05));
            }
        }
    }
    per
}
