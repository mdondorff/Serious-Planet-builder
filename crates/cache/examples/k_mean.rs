//! The land mean and median of the lithology's erodibility K (GEN-116) read at the surface, over the equal-area sample of a seed: the constant that normalises the carve's K to today's rate (GEN-127).
//! `k_mean SEED...` prints, for each seed, the land share, the mean, the median and the mean over every height (the surface plus 0 to 1500 m below it).

use planet_core::Vec3;
use planet_generators::bake::inputs::BakeInputs;

fn main() {
    let (threads, _) = planet_streaming::topology::bake_threads(0).unwrap();
    for seed in std::env::args().skip(1).map(|s| s.parse::<u64>().expect("a seed")) {
        let inputs = BakeInputs::new(seed, threads);
        let n = 600_000usize;
        let golden = core::f64::consts::PI * (3.0 - libm::sqrt(5.0));
        let (mut at_surface, mut at_depth) = (Vec::new(), Vec::new());
        for i in 0..n {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = libm::sqrt(1.0 - y * y);
            let (s, c) = libm::sincos(golden * i as f64);
            let d = Vec3::new(r * c, y, r * s);
            let z = inputs.base(d);
            if z > 0.0 {
                at_surface.push(inputs.rock_k(d, z).k);
                at_depth.push(inputs.rock_k(d, z - 750.0 * (0.5 + 0.5 * libm::sin(i as f64))).k);
            }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let median = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        println!(
            "seed {seed}: land {:.1} %, surface mean {:.4}, median {:.4}, mean at depth {:.4}",
            100.0 * at_surface.len() as f64 / n as f64,
            mean(&at_surface),
            median(&mut at_surface.clone()),
            mean(&at_depth)
        );
    }
}
