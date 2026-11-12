//! GEN-142: the round solver on the prototype region (spikes/prototype-region/results/results.md, region 1: face 1, i 2014-2168, j 190-344 of the 3,072-cell grid), 4 rounds of 5
//! steps, the true cell metric, the round rules. Prints the gates: G-ROUND (against the whole-region carve with the same rules), G-DET (1 and N threads), the cost split of a round.
//! The window is the region plus a margin; its border is an outlet (an artificial cut: the landmass is not routed whole here). Usage: rounds_region [margin_cells] [threads] [skip_reference]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::round::{cut, solve_rounds};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;
use std::time::Instant;

const CELLS: usize = 3072;
const R: f64 = 6_371_000.0;
const REGION: (usize, usize, usize, usize) = (2014, 2168, 190, 344);

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let margin: usize = a.get(1).map_or(200, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let skip_ref = a.get(3).is_some_and(|s| s == "skip_reference");
    let (i0, i1) = (REGION.0.saturating_sub(margin), (REGION.1 + margin).min(CELLS - 1));
    let (j0, j1) = (REGION.2.saturating_sub(margin), (REGION.3 + margin).min(CELLS - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let seed = 14u64;
    let spec = FaceSpec { face: Face(1), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let t = Instant::now();
    let inputs = BakeInputs::new(seed, threads);
    let mut z0 = vec![0.0f64; w * h];
    let rows = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in z0.chunks_mut(rows * w).enumerate() {
            let (inputs, spec) = (&inputs, &spec);
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * w + c;
                    let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    println!("window i {i0}-{i1} j {j0}-{j1}: {w} x {h} = {} cells, base sampled in {:.1} s", w * h, t.elapsed().as_secs_f64());
    let grid = Grid::new(w, h, spec.dx()).with_origin((i0 as u32, j0 as u32)).with_metric(Metric::cube_face(
        CELLS,
        R,
        (i0 as i64, j0 as i64),
        w,
        h,
    ));
    let outlet = grid.outlets(&z0, 0.0);
    let land = z0.iter().filter(|v| **v > 0.0).count();
    println!("land {:.1} %", 100.0 * land as f64 / z0.len() as f64);
    let p = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let stream = spec.stream();

    let mut one = z0.clone();
    let t = Instant::now();
    let rep = solve_rounds(&grid, &mut one, &outlet, &p, 4, &1usize, stream);
    println!("solver, 1 thread: {:.2} s", t.elapsed().as_secs_f64());
    for (r, s) in rep.rounds.iter().enumerate() {
        println!(
            "  round {}: {} catchments, largest {} cells, {} cells in catchments of 1,000 or more",
            r + 1,
            s.catchments,
            s.largest,
            s.in_large
        );
    }
    // The cost split of the first round: the routing and the cut.
    let t = Instant::now();
    let r = route_rules(&grid, &z0, &outlet, p.epsilon, stream, 0.0, None, RoundRules::ROUND);
    let route_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let _ = cut(&r);
    println!("  first round: routing {route_s:.2} s, cut {:.2} s", t.elapsed().as_secs_f64());
    let mut many = z0.clone();
    let t = Instant::now();
    let _ = solve_rounds(&grid, &mut many, &outlet, &p, 4, &threads, stream);
    println!("solver, {threads} threads: {:.2} s", t.elapsed().as_secs_f64());
    let det = one.iter().zip(&many).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    println!("G-DET: 1 thread against {threads} threads, {det} cells differ");
    if !skip_ref {
        let mut want = z0.clone();
        let t = Instant::now();
        carve(&grid, &mut want, &outlet, &p, None, stream);
        println!("whole-region carve (same rules): {:.2} s", t.elapsed().as_secs_f64());
        let d = one.iter().zip(&want).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        println!("G-ROUND: solver against the whole-region carve, {d} cells differ");
    }
    // The change in the region box (without the margin).
    let mut dz: Vec<f64> = Vec::new();
    for j in REGION.2..=REGION.3 {
        for i in REGION.0..=REGION.1 {
            let k = (j - j0) * w + (i - i0);
            if z0[k] > 0.0 {
                dz.push(one[k] - z0[k]);
            }
        }
    }
    dz.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| dz[((dz.len() - 1) as f64 * f) as usize];
    println!("region land change (m): n {} p5 {:.1} p50 {:.1} p95 {:.1} max {:.1}", dz.len(), q(0.05), q(0.5), q(0.95), dz[dz.len() - 1]);
}
