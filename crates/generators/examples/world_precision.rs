//! GEN-175 measurement: f32 rounds (datum, epsilon_w) against f64 rounds on a synthetic 60 km world with a 40 km volcano. Prints the displacement of the f32 result against f64.
//! Usage: cargo run --release -p planet-generators --example world_precision [N] [rounds]
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::precision::{WorldPrecision, EPSILON_MAX_M};
use planet_generators::bake::route::{jitter, RoundRules};
use planet_generators::bake::Grid;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(384);
    let rounds: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let amp: f64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(40_000.0);
    let g = Grid::new(n, n, 800.0);
    let c = n as f64 / 2.0;
    let mut z0 = vec![0.0; n * n];
    for j in 0..n {
        for i in 0..n {
            let r = ((i as f64 - c).powi(2) + (j as f64 - c).powi(2)).sqrt() / (c * 0.9);
            let cone = amp * (1.0 - r).max(0.0).powf(1.3);
            let ridge = 600.0 * libm::sin(i as f64 * 0.09) * libm::cos(j as f64 * 0.07) + 150.0 * jitter(5, (j * n + i) as u64, 0);
            let edge = (i.min(j).min(n - 1 - i).min(n - 1 - j) as f64 / 10.0).min(1.0);
            z0[j * n + i] = (cone + ridge * (cone / 4000.0).min(1.0)) * edge + 2.0 - 10_000.0 * (1.0 - edge);
        }
    }
    let (lo, hi) = z0.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let w = WorldPrecision::new(-10_000.0, amp + 10_000.0, 1e-3, EPSILON_MAX_M);
    println!("heights {lo:.1} .. {hi:.1} m; {}", w.mark_line());
    let base = CarveParams { steps: rounds * 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() };
    let outlet = g.outlets(&z0, 0.0);
    let run = |label: &str, p: &CarveParams| {
        let mut z = z0.clone();
        carve(&g, &mut z, &outlet, p, None, 4);
        println!("{label}: done");
        z
    };
    let z64 = run("f64", &CarveParams { round_f32: false, ..base });
    let zp = {
        let mut z = z0.clone();
        for (q, v) in z.iter_mut().enumerate() {
            *v += 2e-4 * jitter(77, q as u64, 0);
        }
        let mut z2 = z;
        carve(&g, &mut z2, &outlet, &CarveParams { round_f32: false, ..base }, None, 4);
        z2
    };
    let z64w = run("f64 epsilon_w", &CarveParams { round_f32: false, epsilon: w.epsilon_w, ..base });
    let mut pw = base;
    w.apply(&mut pw);
    let zd1 = run("f32 datum eps 1e-3", &CarveParams { epsilon: 1e-3, ..pw });
    let zn2 = run("f32 naive eps_w", &CarveParams { round_f32: true, epsilon: w.epsilon_w, ..base });
    let zw = run("f32 datum epsilon_w", &pw);
    let naive = CarveParams { round_f32: true, ..base };
    let zn = run("f32 no datum, epsilon 1e-3", &naive);
    {
        let bad: Vec<usize> = (0..z64.len()).filter(|&q| (zw[q] - z64[q]).abs() > 0.25).collect();
        let near = bad.iter().filter(|&&q| z64[q] < 5.0).count();
        let lowc = bad.iter().filter(|&&q| z64[q] < 100.0).count();
        let land = z64.iter().filter(|&&v| v > 0.0).count();
        println!("cells over 0.25 m: {} of {} land; of them below 5 m: {near}, below 100 m: {lowc}", bad.len(), land);
    }
    for (label, z, zr) in [
        ("datum+epsilon_w vs f64 same epsilon", &zw, &z64w),
        ("datum+epsilon_w vs f64 eps 1e-3", &zw, &z64),
        ("f64 epsilon_w vs f64 eps 1e-3", &z64w, &z64),
        ("f64 start perturbed by 0.2 mm vs f64", &zp, &z64),
        ("naive f32 vs f64 eps 1e-3", &zn, &z64),
        ("f32 datum eps 1e-3 vs f64 eps 1e-3", &zd1, &z64),
        ("f32 naive eps_w vs f64 eps_w", &zn2, &z64w),
    ] {
        let mut d: Vec<f64> = z.iter().zip(zr).map(|(a, b)| (a - b).abs()).collect();
        let mean = d.iter().sum::<f64>() / d.len() as f64;
        d.sort_by(f64::total_cmp);
        let q = |p: f64| d[((d.len() - 1) as f64 * p) as usize];
        let hs = z.iter().zip(zr).filter(|(_, b)| **b > 20_000.0).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        println!(
            "{label}: |f32-f64| m  mean {mean:.4}  p50 {:.4}  p99 {:.4}  p99.9 {:.4}  max {:.4}  max above 20 km {hs:.4}",
            q(0.5),
            q(0.99),
            q(0.999),
            q(1.0)
        );
    }
    println!(
        "noise floor refs: store quantisation step 0.25 m; f32 spacing at R=30 km {:.5} m",
        planet_generators::bake::precision::f32_spacing(30_000.0)
    );
}
