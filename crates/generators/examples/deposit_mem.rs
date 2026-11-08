//! Peak-memory probe for the carve with deposition (GEN-131): `deposit_mem N G` carves an N x N cone with routing; run it under a process that reports the peak working set.
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::Grid;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(2048);
    let g: f64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let grid = Grid::new(n, n, 814.0);
    let c = (n as f64 - 1.0) / 2.0;
    let mut z: Vec<f64> = (0..n * n)
        .map(|p| {
            let r = libm::hypot((p % n) as f64 - c, (p / n) as f64 - c);
            (3000.0 * (1.0 - r / c)).max(0.0) + 5.0 * libm::sin(0.37 * (p % n) as f64) * libm::cos(0.23 * (p / n) as f64)
        })
        .collect();
    let outlet = grid.outlets(&z, -1.0);
    let p = CarveParams { steps: 10, deposit_g: g, ..CarveParams::default() };
    let t = std::time::Instant::now();
    carve(&grid, &mut z, &outlet, &p, None, 1);
    println!("n {n} G {g} cells {} carve {:.1} s", n * n, t.elapsed().as_secs_f64());
}
