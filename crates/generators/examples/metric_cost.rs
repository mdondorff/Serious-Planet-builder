//! Cost probe of the physical cell metric (GEN-138): `metric_cost N [metric]` routes and carves an N x N face window with or without the metric and prints the time.
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::metric::Metric;
use planet_generators::bake::Grid;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(2048);
    let metric = a.get(2).is_some();
    let r = 6_371_000.0;
    let mut grid = Grid::new(n, n, r * core::f64::consts::FRAC_PI_2 / n as f64);
    if metric {
        grid = grid.with_metric(Metric::cube_face(n, r, (0, 0), n, n));
    }
    let c = (n as f64 - 1.0) / 2.0;
    let mut z: Vec<f64> = (0..n * n)
        .map(|p| {
            (3000.0 * (1.0 - libm::hypot((p % n) as f64 - c, (p / n) as f64 - c) / c)).max(0.0)
                + 5.0 * libm::sin(0.37 * (p % n) as f64) * libm::cos(0.23 * (p / n) as f64)
        })
        .collect();
    let outlet = grid.outlets(&z, -1.0);
    let t = std::time::Instant::now();
    carve(&grid, &mut z, &outlet, &CarveParams { steps: 10, ..CarveParams::default() }, None, 1);
    println!("n {n} metric {metric}: carve {:.1} s", t.elapsed().as_secs_f64());
}
