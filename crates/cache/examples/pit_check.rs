//! Scratch (not committed): one-cell pits of the final surface (exact base + stored change) for the pass-2 core of a tile and for a reference core file.
//! `pit_check --face-file F --tile FACE,TI,TJ --ref A.f32 [--threads N]`
use planet_cache::layer_tiles::face_change_reader;
use planet_core::{Face, Vec3};
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::{pass2_tile, Pass2Params, TileGrid, TileId};

fn pits(z: &[f64], n: usize, thr: f64) -> (usize, usize, f64) {
    let (mut c2, mut c05, mut worst) = (0, 0, 0.0f64);
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let p = j * n + i;
            if z[p] <= 0.0 {
                continue;
            }
            let m = [-1i64, 0, 1].iter().flat_map(|dj| [-1i64, 0, 1].iter().map(move |di| (*di, *dj))).filter(|d| *d != (0, 0)).map(|(di, dj)| z[(p as i64 + dj * n as i64 + di) as usize]).fold(f64::INFINITY, f64::min);
            let depth = m - z[p];
            if depth > thr {
                c2 += 1;
            }
            if depth > 0.5 {
                c05 += 1;
            }
            worst = worst.max(depth);
        }
    }
    (c2, c05, worst)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let val = |name: &str| args.iter().position(|a| a == name).map(|i| args[i + 1].clone());
    let p: Vec<u32> = val("--tile").unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    let (face_no, ti, tj) = (p[0] as u8, p[1], p[2]);
    let threads: usize = val("--threads").map_or(4, |v| v.parse().unwrap());
    let raw = std::fs::read(val("--face-file").unwrap()).unwrap();
    let word = |i: usize| u64::from_le_bytes(raw[i * 8..i * 8 + 8].try_into().unwrap()) as usize;
    let (cells, halo, len) = (word(0), word(1), word(2));
    let change: Vec<f32> = raw[24..24 + len * 4].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let faces: Vec<FaceChange> = (0..6u8).map(|f| FaceChange { face: Face(f), cells, halo, change: if f == face_no { change.clone() } else { vec![0.0; len] } }).collect();
    let faces: [FaceChange; 6] = faces.try_into().unwrap();
    let reader = face_change_reader(&faces);
    let inputs = BakeInputs::new(14, threads);
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    let grid = TileGrid { cells: 98_304, tile: 3072, halo: 128, radius_m: 6_371_000.0 };
    let r = pass2_tile(&grid, TileId { face: Face(face_no), ti, tj }, &base, &reader, Some(&factor), &Pass2Params::standard(14, 0.0), threads);
    let n = 3072usize;
    let (i0, j0) = (ti as i64 * 3072, tj as i64 * 3072);
    let exact: Vec<f64> = planet_core::par::map(&threads, n, &|j| (0..n).map(|i| base(grid.direction(Face(face_no), i0 + i as i64, j0 + j as i64))).collect::<Vec<f64>>()).into_iter().flatten().collect();
    let fin_b: Vec<f64> = exact.iter().zip(&r.core).map(|(e, c)| e + f64::from(*c)).collect();
    let a: Vec<f32> = std::fs::read(val("--ref").unwrap()).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let fin_a: Vec<f64> = exact.iter().zip(&a).map(|(e, c)| e + f64::from(*c)).collect();
    println!("final surface of the tile, one-cell pits (land cells lower than all 8 neighbours):");
    for (name, z) in [("A (exact base, 100 x 1/16, halo 256)", &fin_a), ("B (sparse base, 300 x 1/8, halo 128)", &fin_b)] {
        let (c2, c05, w) = pits(z, n, 2.0);
        println!("  {name}: over 2 m {c2}, over 0.5 m {c05}, deepest {w:.2} m");
    }
    let d: Vec<f64> = fin_b.iter().zip(&fin_a).map(|(b, a)| b - a).collect();
    let rms = (d.iter().map(|v| v * v).sum::<f64>() / d.len() as f64).sqrt();
    println!("B - A final surface: rms {rms:.2} m, thermal passes {}, cells still moving {}", r.stats.thermal_passes, r.stats.thermal_moving_at_cap);
}
