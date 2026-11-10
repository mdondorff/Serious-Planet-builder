//! Diagnosis (inbox #118; not for merge): a synthetic window whose only outlet is a high cut, depth under the fill round over round.
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::round::solve_rounds;
use planet_generators::bake::route::RoundRules;
use planet_generators::bake::Grid;

fn hash(i: usize, j: usize) -> f64 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).map_or(48, |s| s.parse().unwrap());
    let noise: f64 = a.get(2).map_or(15.0, |s| s.parse().unwrap());
    let floor: f64 = a.get(3).map_or(50.0, |s| s.parse().unwrap());
    let rim: f64 = a.get(4).map_or(10.0, |s| s.parse().unwrap());
    let cut: Option<f64> = a.get(5).and_then(|s| s.parse().ok());
    let breach: f64 = a.get(6).map_or(500.0, |s| s.parse().unwrap());
    let c = (n as f64 - 1.0) / 2.0;
    let mut z = vec![0.0f64; n * n];
    let mut outlet = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let r = ((i as f64 - c).powi(2) + (j as f64 - c).powi(2)).sqrt();
            z[j * n + i] = floor + rim * r + noise * (hash(i, j) - 0.5);
            outlet[j * n + i] = i == 0 || j == 0 || i == n - 1 || j == n - 1;
            if let (Some(h), true) = (cut, outlet[j * n + i]) {
                z[j * n + i] = h;
            }
        }
    }
    let grid = Grid::new(n, n, 814.0);
    let z0 = z.clone();
    let depth = |z: &[f64]| {
        let f = priority_flood_with(&grid, z, &outlet, 1e-3, true);
        let mut d: Vec<f64> = (0..n * n).filter(|&p| !outlet[p] && f[p] - z[p] > 1.0).map(|p| f[p] - z[p]).collect();
        d.sort_by(f64::total_cmp);
        let bottom = z.iter().cloned().fold(f64::MAX, f64::min);
        (d.len(), if d.is_empty() { 0.0 } else { d[d.len() / 2] }, d.last().copied().unwrap_or(0.0), bottom)
    };
    let (cnt, p50, max, bot) = depth(&z0);
    println!(
        "n {n} noise {noise} floor {floor} rim {rim}: base: {cnt} cells under the fill, depth p50 {p50:.1} max {max:.1}, lowest z {bot:.1}"
    );
    for r in 1..=4usize {
        let mut zz = z0.clone();
        let p =
            CarveParams { breach_max_m: breach, steps: 5 * r, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
        solve_rounds(&grid, &mut zz, &outlet, &p, r, &1usize, 15);
        let (cnt, p50, max, bot) = depth(&zz);
        println!("after round {r}: {cnt} cells under the fill, depth p50 {p50:.1} max {max:.1}, lowest z {bot:.1}");
    }
}
