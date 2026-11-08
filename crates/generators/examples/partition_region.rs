//! GEN-143: the partitioned route on the prototype region (G-PART) and E2-type edits; the window as in `rounds_region`. Usage: partition_region [margin_cells] [tile_cells] [threads]
//! (the sampling below is `rounds_region`'s)
//! GEN-142 (rounds_region): the round solver on the prototype region (spikes/prototype-region/results/results.md, region 1: face 1, i 2014-2168, j 190-344 of the 3,072-cell grid), 4 rounds of 5
//! steps, the true cell metric, the round rules. Prints the gates: G-ROUND (against the whole-region carve with the same rules), G-DET (1 and N threads), the cost split of a round.
//! The window is the region plus a margin; its border is an outlet (an artificial cut: the landmass is not routed whole here). Usage: rounds_region [margin_cells] [threads] [skip_reference]
use planet_core::{Face, Vec3};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;
use std::time::Instant;

const CELLS: usize = 3072;
const R: f64 = 6_371_000.0;
const REGION: (usize, usize, usize, usize) = (2014, 2168, 190, 344);

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let margin: usize = a.get(1).map_or(200, |s| s.parse().unwrap());
    let tile: usize = a.get(2).map_or(256, |s| s.parse().unwrap());
    let threads: usize = a.get(3).map_or(8, |s| s.parse().unwrap());
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
    let stream = spec.stream();
    let eps = 1e-3;
    let rules = RoundRules::ROUND;
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let diff = |a: &planet_generators::bake::route::Routing, b: &planet_generators::bake::route::Routing| {
        (
            a.receiver.iter().zip(&b.receiver).filter(|(x, y)| x != y).count(),
            a.area.iter().zip(&b.area).filter(|(x, y)| x.to_bits() != y.to_bits()).count(),
        )
    };
    let t = Instant::now();
    let resident = route_rules(&grid, &z0, &outlet, eps, stream, 0.0, None, rules);
    println!("resident route: {:.2} s", t.elapsed().as_secs_f64());
    let mut cache = PartCache::default();
    let t = Instant::now();
    let fresh = partitioned_route(&grid, &z0, &outlet, eps, stream, tile, rules, &mut cache);
    let fresh_s = t.elapsed().as_secs_f64();
    let (dr, da) = diff(&fresh.routing, &resident);
    let df = bits(&fresh.filled).iter().zip(bits(&priority_flood_with(&grid, &z0, &outlet, eps, true))).filter(|(x, y)| *x != y).count();
    println!(
        "G-PART fresh, tile {tile}: {} tiles, {} sweeps, {} tile floods ({} memo hits), {fresh_s:.2} s; against resident: {df} filled levels, {dr} receivers, {da} A_d differ",
        fresh.stats.tiles, fresh.stats.sweeps, fresh.stats.floods, fresh.stats.memo_hits
    );
    // E2-type edits (results.md): the pass between the two mouths' catchments lowered by 100 m; a ridge line raised by 200 m.
    let at = |i: usize, j: usize| (j - j0) * w + (i - i0);
    let mut edits: Vec<(&str, Vec<f64>)> = Vec::new();
    let mut e = z0.clone();
    e[at(2031, 311)] -= 100.0;
    edits.push(("E2 pass lowered by 100 m", e));
    let mut e = z0.clone();
    let (x0, y0, x1, y1) = (2012i64, 298i64, 2037i64, 316i64);
    for s in 0..=25 {
        let (x, y) = (x0 + (x1 - x0) * s / 25, y0 + (y1 - y0) * s / 25);
        e[at(x as usize, y as usize)] += 200.0;
    }
    edits.push(("E2 ridge line raised by 200 m", e));
    for (name, e) in &edits {
        let mut c = PartCache::default();
        let _ = partitioned_route(&grid, &z0, &outlet, eps, stream, tile, rules, &mut c);
        let t = Instant::now();
        let ed = partitioned_route(&grid, e, &outlet, eps, stream, tile, rules, &mut c);
        let ed_s = t.elapsed().as_secs_f64();
        let want = route_rules(&grid, e, &outlet, eps, stream, 0.0, None, rules);
        let (dr, da) = diff(&ed.routing, &want);
        println!(
            "{name}: {} of {} tile floods run ({:.1} % of a fresh build; {} memo hits, {} sweeps), tiles changed: filled {}, receivers {}, A_d {}; {ed_s:.2} s; against the resident route of the edited surface: {dr} receivers, {da} A_d differ",
            ed.stats.floods,
            fresh.stats.floods,
            100.0 * ed.stats.floods as f64 / fresh.stats.floods as f64,
            ed.stats.memo_hits,
            ed.stats.sweeps,
            ed.stats.tiles_filled_changed,
            ed.stats.tiles_receiver_changed,
            ed.stats.tiles_area_changed
        );
    }
}
