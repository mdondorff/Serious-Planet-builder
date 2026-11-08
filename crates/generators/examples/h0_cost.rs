//! Cost probe of H0 (GEN-139): `h0_cost N` routes the whole planet at N cells a face edge over a synthetic surface and prints the time.
use planet_core::Vec3;
use planet_generators::bake::h0::GlobalGrid;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|v| v.parse().ok()).unwrap_or(512);
    let g = GlobalGrid::new(n, 6_371_000.0);
    let axis = Vec3::new(1.0, 0.0, 0.0);
    let ang: Vec<f64> = (0..g.len() as u32).map(|c| libm::acos(g.direction(c).dot(axis).clamp(-1.0, 1.0))).collect();
    let z: Vec<f64> = ang.iter().map(|a| 100.0 + 5000.0 * a + 40.0 * libm::sin(37.0 * a)).collect();
    let outlet: Vec<bool> = ang.iter().map(|a| a.to_degrees() < 25.0).collect();
    let t = std::time::Instant::now();
    let r = g.route(&z, &outlet, 1e-3);
    println!(
        "n {n} cells {} route {:.1} s, largest area {:.3e} m2",
        g.len(),
        t.elapsed().as_secs_f64(),
        r.area.iter().cloned().fold(0.0, f64::max)
    );
}
