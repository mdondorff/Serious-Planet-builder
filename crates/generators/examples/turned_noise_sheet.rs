//! The lattice-lines sheet of GEN-140 (A2): the crease noise and the hill band at the site of the owner's frames 1162 (1.6 km) and 1510 (143 m), 7.437 N 29.627 W, hillshaded from above with the
//! untouched lattice (left) and the turned lattice (right). Rows: the crease octaves 4 to 11 over 1.6 km and over 300 m (900 m of relief, the amplitude of the fine octaves in the frames), the hill
//! band over 6 km (200 m). A probe, never a measurement of the pipeline: the heights are the noise alone, no class, no carve, no erosion.
//!
//! `cargo run --release -p planet-generators --example turned_noise_sheet [OUT.png]` (default `target/review/turned-noise/sheet.png`).

use planet_core::Vec3;
use planet_generators::global::hill_octaves_at;
use planet_generators::height::{layer_noise_t, turned_noise};

const R: f64 = 6_371_000.0;
const SEED: u64 = 14;
const LAYER_CREASE: u64 = 0x4552_0001;
const PX: usize = 384;

fn crease(n: f64) -> f64 {
    let t = (1.0 - n.abs()).max(0.0);
    0.25 - t * t * t
}

/// The fine crease octaves (4 to 11) as one height in `[-0.75, 0.25]`, normalised like the erosion field's (amplitude 0.55 per octave, base frequency 256).
fn crease_sum(turned: bool, d: Vec3) -> f64 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, 256.0);
    for o in 0..12u32 {
        if o >= 4 {
            sum += amp * crease(layer_noise_t(turned, SEED, LAYER_CREASE, o, d * freq));
        }
        norm += amp;
        amp *= 0.55;
        freq *= 2.0;
    }
    sum / norm
}

type NoiseFn = fn(bool, Vec3) -> f64;

fn hill_noise(turned: bool, d: Vec3) -> f64 {
    hill_octaves_at(SEED, d, turned)
}

fn main() {
    let (lat, lon) = (7.437f64.to_radians(), (-29.627f64).to_radians());
    let up = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    assert!(!turned_noise());
    let rows: [(f64, f64, NoiseFn); 3] = [(1600.0, 900.0, crease_sum), (300.0, 900.0, crease_sum), (6000.0, 200.0, hill_noise)];
    let (w, h) = (2 * PX, 3 * PX);
    let mut img = vec![0u8; w * h];
    for (r, (size_m, relief_m, f)) in rows.iter().enumerate() {
        for (c, turned) in [false, true].into_iter().enumerate() {
            let cell = size_m / PX as f64;
            let height = |ix: i64, iy: i64| -> f64 {
                let (x, y) = ((ix as f64 - PX as f64 / 2.0) * cell, (iy as f64 - PX as f64 / 2.0) * cell);
                f(turned, (up + east * (x / R) + north * (y / R)).normalized()) * relief_m
            };
            // Hillshade from the north-west at 35 degrees: the normal from central differences.
            for iy in 0..PX {
                for ix in 0..PX {
                    let (dx, dy) = (
                        (height(ix as i64 + 1, iy as i64) - height(ix as i64 - 1, iy as i64)) / (2.0 * cell),
                        (height(ix as i64, iy as i64 + 1) - height(ix as i64, iy as i64 - 1)) / (2.0 * cell),
                    );
                    let n = Vec3::new(-dx, -dy, 1.0).normalized();
                    let l = Vec3::new(-0.5, 0.5, 0.7).normalized();
                    let v = (n.dot(l).max(0.0) * 255.0).clamp(0.0, 255.0) as u8;
                    img[(r * PX + iy) * w + c * PX + ix] = v;
                }
            }
        }
    }
    let out = std::env::args().nth(1).unwrap_or_else(|| "target/review/turned-noise/sheet.png".into());
    let path = std::path::Path::new(&out);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path).unwrap()), w as u32, h as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
    println!("wrote {out}");
}
