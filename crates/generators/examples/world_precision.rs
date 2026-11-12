//! GEN-175 measurement: f32 rounds against f64 rounds on a synthetic world, in the frame of the sea level, at the same epsilon_w, after one round and after four; and the noise floor (f64 under a second
//! jitter seed: the start surface perturbed by 0.2 mm of noise).
//! Usage: cargo run --release -p planet-generators --example world_precision [N] [amp_m] [lo_bound_m] [hi_bound_m]
//! 60 km world: `384 40000 -10000 50000`; Earth control: `384 9000 -11000 9000`.
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::precision::{f32_spacing, WorldPrecision, EPSILON_MAX_M};
use planet_generators::bake::route::{jitter, RoundRules};
use planet_generators::bake::Grid;

fn pct(d: &mut [f64], p: f64) -> f64 {
    d.sort_by(f64::total_cmp);
    d[((d.len() - 1) as f64 * p) as usize]
}

/// The p95 of the local relief: the range of the heights in the 5 x 5 window of every land cell.
fn relief_p95(z: &[f64], n: usize) -> f64 {
    let mut r = Vec::new();
    for j in 2..n - 2 {
        for i in 2..n - 2 {
            if z[j * n + i] <= 0.0 {
                continue;
            }
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for dj in 0..5 {
                for di in 0..5 {
                    let v = z[(j + dj - 2) * n + i + di - 2];
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            r.push(hi - lo);
        }
    }
    pct(&mut r, 0.95)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let arg = |k: usize, d: f64| a.get(k).and_then(|s| s.parse().ok()).unwrap_or(d);
    let n = arg(1, 384.0) as usize;
    let amp = arg(2, 40_000.0);
    let (lo, hi) = (arg(3, -10_000.0), arg(4, 50_000.0));
    let g = Grid::new(n, n, 800.0);
    let c = n as f64 / 2.0;
    let mut z0 = vec![0.0; n * n];
    for j in 0..n {
        for i in 0..n {
            let r = ((i as f64 - c).powi(2) + (j as f64 - c).powi(2)).sqrt() / (c * 0.9);
            let cone = amp * (1.0 - r).max(0.0).powf(1.3);
            let ridge = 600.0 * libm::sin(i as f64 * 0.09) * libm::cos(j as f64 * 0.07) + 150.0 * jitter(5, (j * n + i) as u64, 0);
            let edge = (i.min(j).min(n - 1 - i).min(n - 1 - j) as f64 / 10.0).min(1.0);
            z0[j * n + i] = (cone + ridge * (cone / 4000.0).min(1.0)) * edge + 2.0 + -10_000.0 * (1.0 - edge);
        }
    }
    let w = WorldPrecision::new(lo, hi, 1e-3, EPSILON_MAX_M);
    println!("{} ; spacing(R) {:.5} m; 2 x spacing {:.5} m", w.mark_line(), f32_spacing(w.radius), 2.0 * f32_spacing(w.radius));
    let outlet = g.outlets(&z0, 0.0);
    let noisy = {
        let mut z = z0.clone();
        for (q, v) in z.iter_mut().enumerate() {
            *v += 2e-4 * jitter(77, q as u64, 0);
        }
        z
    };
    for rounds in [1usize, 4] {
        let base = CarveParams {
            steps: rounds * 5,
            reroute_every: 5,
            round_rules: RoundRules::ROUND,
            epsilon: w.epsilon_w,
            ..CarveParams::default()
        };
        let run = |start: &[f64], f32r: bool| {
            let mut z = start.to_vec();
            carve(&g, &mut z, &outlet, &CarveParams { round_f32: f32r, ..base }, None, 4);
            z
        };
        let (z32, z64, z64s) = (run(&z0, true), run(&z0, false), run(&noisy, false));
        for (label, z) in [("f32 vs f64          ", &z32), ("f64 seed2 vs f64    ", &z64s)] {
            let mut d: Vec<f64> = z.iter().zip(&z64).map(|(x, y)| (x - y).abs()).collect();
            let (p95, p99) = (pct(&mut d, 0.95), pct(&mut d, 0.99));
            println!("rounds {rounds}: {label} |dz| p95 {p95:.5}  p99 {p99:.5}  max {:.5}  m", d[d.len() - 1]);
        }
        println!(
            "rounds {rounds}: relief p95 (5x5, land): f64 {:.3}  f32 {:.3}  f64 seed2 {:.3} m",
            relief_p95(&z64, n),
            relief_p95(&z32, n),
            relief_p95(&z64s, n)
        );
    }
}
