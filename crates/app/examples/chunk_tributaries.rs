//! SPIKE-47 (spike branch only): tributary variants of the A-net chunk solve at one site.
//! `cargo run --release -p planet --example chunk_tributaries -- --store DIR --lat DEG --lon DEG [--variant name:key=val,...]... [--threads N] [--out DIR] [--crop N]`

use planet::worldfield::WorldTerrain;
use planet_world_def::WorldDefinition;
use std::sync::Arc;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn args(name: &str) -> Vec<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().enumerate().filter(|(_, x)| *x == name).filter_map(|(i, _)| a.get(i + 1).cloned()).collect()
}

/// Hillshade of the centre `crop` x `crop` cells of an `n` x `n` raster: sun from the north-west at 45 degrees, heights exaggerated by `ve`.
fn hillshade(z: &[f64], n: usize, crop: usize, cell_m: f64, ve: f64) -> Vec<u8> {
    let o = (n - crop) / 2;
    let at = |i: usize, j: usize| z[(o + j) * n + o + i] * ve;
    let (az, alt) = (315.0f64.to_radians(), 45.0f64.to_radians());
    let mut out = vec![0u8; crop * crop];
    for j in 1..crop - 1 {
        for i in 1..crop - 1 {
            let dzdx = (at(i + 1, j) - at(i - 1, j)) / (2.0 * cell_m);
            let dzdy = (at(i, j + 1) - at(i, j - 1)) / (2.0 * cell_m);
            let slope = (dzdx * dzdx + dzdy * dzdy).sqrt().atan();
            let aspect = (-dzdy).atan2(-dzdx);
            let s = alt.sin() * slope.cos() + alt.cos() * slope.sin() * (az - aspect).cos();
            out[j * crop + i] = (s.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    out
}

fn main() {
    let store = std::path::PathBuf::from(arg("--store").expect("--store"));
    let lat: f64 = arg("--lat").expect("--lat").parse().unwrap();
    let lon: f64 = arg("--lon").expect("--lon").parse().unwrap();
    let threads: usize = arg("--threads").map_or(1, |v| v.parse().unwrap());
    let crop: usize = arg("--crop").map_or(400, |v| v.parse().unwrap());
    let seed = planet::parse_args(&["test-render".to_string(), "--scene".to_string(), "lit".to_string()]).unwrap().seed;
    let def = WorldDefinition::single_planet(6_371_000.0, seed);
    let world = Arc::new(WorldTerrain::open_store(def, &store, 16384).unwrap());
    let mut variants = args("--variant");
    if variants.is_empty() {
        variants = vec!["fill".to_string()];
    }
    let run = world.tributary_spike(lat, lon, &variants, threads).unwrap();
    println!("{}", run.report);
    if let Some(out) = arg("--out") {
        std::fs::create_dir_all(&out).unwrap();
        let cell_m = 8.04;
        let panels: Vec<Vec<u8>> = run.cores.iter().map(|(_, z)| hillshade(z, run.n, crop, cell_m, 2.0)).collect();
        let cols = panels.len().min(3);
        let rows = panels.len().div_ceil(cols);
        let (w, h) = (cols * crop, rows * crop);
        let mut sheet = vec![0u8; w * h];
        for (p, panel) in panels.iter().enumerate() {
            let (cx, cy) = ((p % cols) * crop, (p / cols) * crop);
            for j in 0..crop {
                sheet[(cy + j) * w + cx..(cy + j) * w + cx + crop].copy_from_slice(&panel[j * crop..(j + 1) * crop]);
            }
        }
        let path = std::path::Path::new(&out).join(format!("hillshade_{lat:.3}_{lon:.3}.png"));
        let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let mut enc = png::Encoder::new(file, w as u32, h as u32);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&sheet).unwrap();
        let names: Vec<&str> = run.cores.iter().map(|(n, _)| n.as_str()).collect();
        println!("sheet {} ({} x {}), panels in order: {}", path.display(), w, h, names.join(", "));
    }
}
