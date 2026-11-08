//! SPIKE-105 F3 (spike branch only): size of a persistent river network by channel threshold, from the face-2 pass-1 area channel. `f3_river_store DIR`.
use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_channels::load_area;
use planet_cache::layer_tiles::{bake_specs, BakeShape};
use planet_cache::pack::TileStore;
use planet_core::Face;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_world_def::WorldDefinition;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let seed = 14u64;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let inputs = std::sync::Arc::new(BakeInputs::new(seed, 4));
    let world = GlobalWorld::from_definition(&def).unwrap().with_class_source(inputs.clone());
    let n = 12_288usize;
    let shape = BakeShape { seed, radius_m: 6_371_000.0, face_res: n, planet_res: 98_304, tile: None, halo: None };
    let (layer, _spec) = bake_specs(&world, &shape).unwrap();
    let store = TileStore::open(&dir.join("layer"), world.params()).unwrap();
    let codes = match load_area(&store, &world, n as u32, &layer, Face(2)) {
        Ok(Some(c)) => c,
        Ok(None) => panic!("store holds no area for face 2 under these keys"),
        Err(e) => panic!("load_area failed: {e:?}"),
    };
    eprintln!("loaded {} codes", codes.len());
    let mut hist = vec![0u64; 256];
    for c in &codes {
        hist[*c as usize] += 1;
    }
    println!("code histogram (code: count) for codes 0..8 and 255: {:?} | 255: {}", &hist[..8], hist[255]);
    // land share by base sampling, stride 16
    let fs = FaceSpec { face: Face(2), cells: n, radius_m: layer.radius_m, sea_level: layer.sea_level, seed, halo: 0 };
    let (mut land, mut tot) = (0u64, 0u64);
    let mut i = 8;
    while i < n {
        let mut j = 8;
        while j < n {
            tot += 1;
            if inputs.base(fs.halo_direction(i as i64, j as i64)) > layer.sea_level {
                land += 1;
            }
            j += 16;
        }
        i += 16;
    }
    let st = 4usize;
    let m = n / st;
    let t0 = std::time::Instant::now();
    let mut mask = vec![0u8; m * m];
    std::thread::scope(|sc| {
        for (k, rows) in mask.chunks_mut(m * m / 4).enumerate() {
            let (inputs, fs, sea) = (&inputs, &fs, layer.sea_level);
            sc.spawn(move || {
                for (r, row) in rows.chunks_mut(m).enumerate() {
                    let jj = k * (m / 4) + r;
                    for (ii, v) in row.iter_mut().enumerate() {
                        *v = (inputs.base(fs.halo_direction((ii * st + st / 2) as i64, (jj * st + st / 2) as i64)) > sea) as u8;
                    }
                }
            });
        }
    });
    let land4 = mask.iter().filter(|v| **v == 1).count();
    eprintln!("mask: {:.0} s, land share stride4 {:.4}", t0.elapsed().as_secs_f64(), land4 as f64 / (m * m) as f64);
    let is_land = |i: usize, j: usize| mask[(j / st) * m + i / st] == 1;
    let land_share = land4 as f64 / (m * m) as f64;
    let _ = (land, tot);
    let face_km2 = 5.1e8 / 6.0;
    let face_land_km2 = land_share * face_km2;
    let planet_land_km2 = 0.29 * 5.1e8;
    let scale = planet_land_km2 / face_land_km2;
    let cell_m = std::f64::consts::FRAC_PI_2 * 6_371_000.0 / n as f64;
    let cell_km2 = cell_m * cell_m / 1e6;
    println!("land share face 2 {:.3} ({land}/{tot}), face land {:.3e} km2, scale to planet {:.2}, cell {:.1} m = {:.4} km2", land_share, face_land_km2, scale, cell_m, cell_km2);
    println!("T_km2 | code>= | cells | length_km (x1.1) | heads | mouths | confluences | segments | face B@102 | face B@814 | planet len km | planet MB@102 | planet MB@814");
    for t in [1.0f64, 10.0, 50.0, 100.0, 1000.0] {
        let thr = (8.0 * (t / cell_km2).log2()).round().max(1.0).min(254.0) as u8;
        let ch = |c: u8| c >= thr;
        let (mut cells, mut heads, mut heads_lo, mut mouths) = (0u64, 0u64, 0u64, 0u64);
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let c = codes[j * n + i];
                if !ch(c) || !is_land(i, j) {
                    continue;
                }
                cells += 1;
                let (mut up, mut up_le, mut zero) = (false, false, false);
                for dj in [-1isize, 0, 1] {
                    for di in [-1isize, 0, 1] {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        let nb = codes[(j as isize + dj) as usize * n + (i as isize + di) as usize];
                        if ch(nb) && nb <= c && is_land((i as isize + di) as usize, (j as isize + dj) as usize) {
                            up_le = true;
                        }
                        if ch(nb) && nb < c && is_land((i as isize + di) as usize, (j as isize + dj) as usize) {
                            up = true;
                        }
                        if !is_land((i as isize + di) as usize, (j as isize + dj) as usize) {
                            zero = true;
                        }
                    }
                }
                if !up {
                    heads += 1;
                }
                if !up_le {
                    heads_lo += 1;
                }
                if zero {
                    mouths += 1;
                }
            }
        }
        let len_km = cells as f64 * cell_m * 1.1 / 1000.0;
        let hb = (land_share * face_km2 / t) as u64;
        println!("T {t}: heads strict-lt (upper, tie-inflated) {heads}, heads nb<=c (lower, tie-deflated) {heads_lo}, basin-bound land/T {hb}");
        let heads = heads_lo;
        let conf = heads.saturating_sub(mouths);
        let segs = heads + conf;
        let b = |sp: f64| len_km * 1000.0 / sp * 24.0 + segs as f64 * 32.0;
        println!(
            "{t} | {thr} | {cells} | {len_km:.0} | {heads} | {mouths} | {conf} | {segs} | {:.3e} | {:.3e} | {:.3e} | {:.0} | {:.0}",
            b(101.8),
            b(814.0),
            len_km * scale,
            b(101.8) * scale / 1e6,
            b(814.0) * scale / 1e6
        );
    }
}
