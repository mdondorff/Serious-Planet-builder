//! Diagnosis (inbox #118; not for merge): depression depth round over round on a window, and the spill path of the deepest depression through the breach and the sweeps.
//! Usage: [PLANET_REGION=seed,face,i0,j0,size] depth_trace [margin] [threads]. Default region: R3b (seed 15, face 2, 9968, 8304, 620).
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::round::{catchment_round, cut_in, gather_with, solve_rounds, Drivers};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;

fn stats(grid: &Grid, z: &[f64], outlet: &[bool], inside: &dyn Fn(usize) -> bool, tag: &str) -> Vec<f64> {
    let filled = priority_flood_with(grid, z, outlet, 1e-3, true);
    let mut d: Vec<f64> = (0..z.len()).filter(|&p| inside(p) && z[p] > 0.0 && filled[p] - z[p] > 1.0).map(|p| filled[p] - z[p]).collect();
    d.sort_by(f64::total_cmp);
    let land = (0..z.len()).filter(|&p| inside(p) && z[p] > 0.0).count();
    if d.is_empty() {
        println!("{tag}: no land cell deeper than 1 m under its fill");
    } else {
        let q = |f: f64| d[((d.len() - 1) as f64 * f) as usize];
        println!(
            "{tag}: depression cells (>1 m under the fill) {} ({:.3} % of land), depth p50 {:.1} p95 {:.1} max {:.1} m",
            d.len(),
            100.0 * d.len() as f64 / land as f64,
            q(0.5),
            q(0.95),
            d[d.len() - 1]
        );
    }
    filled
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let margin: usize = a.get(1).map_or(200, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let pick: usize = a.get(3).map_or(1, |s| s.parse().unwrap());
    let text = std::env::var("PLANET_REGION").unwrap_or_else(|_| "15,2,9968,8304,620".to_string());
    let v: Vec<usize> = text.split(',').map(|s| s.trim().parse().unwrap()).collect();
    let (seed, face, region) = (v[0] as u64, v[1] as u8, (v[2], v[2] + v[4] - 1, v[3], v[3] + v[4] - 1));
    let (i0, i1) = (region.0.saturating_sub(margin), (region.1 + margin).min(CELLS - 1));
    let (j0, j1) = (region.2.saturating_sub(margin), (region.3 + margin).min(CELLS - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let n = w * h;
    let spec = FaceSpec { face: Face(face), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let inputs = BakeInputs::new(seed, threads);
    let mut z = vec![0.0f64; n];
    let rows = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in z.chunks_mut(rows * w).enumerate() {
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
    let grid = Grid::new(w, h, spec.dx()).with_origin((i0 as u32, j0 as u32)).with_metric(Metric::cube_face(
        CELLS,
        R,
        (i0 as i64, j0 as i64),
        w,
        h,
    ));
    let outlet = grid.outlets(&z, 0.0);
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
    println!(
        "window {w} x {h}; breach_max_m {} slope {} keep_pits {} uplift_taper {} u_over_k {} k_dt {}",
        p.breach_max_m, p.breach_slope, p.keep_pits, p.uplift_taper_m, p.u_over_k, p.k_dt
    );

    let inside = |c: usize| {
        let (i, j) = (i0 + c % w, j0 + c / w);
        i >= region.0 && i <= region.1 && j >= region.2 && j <= region.3
    };
    let z0 = z.clone();
    let taper: Vec<f32> = z0.iter().map(|h| planet_core::smoothstep(0.0, p.uplift_taper_m, h - p.sea_level) as f32).collect();
    stats(&grid, &z, &outlet, &inside, "base");
    let mut target: Option<usize> = None;
    for round in 0..4usize {
        let filled = stats(&grid, &z, &outlet, &inside, &format!("before round {}", round + 1));
        if target.is_none() {
            // the deepest depression cell of the surface after round 1 is followed from round 1 on: pick it after the first round below
        }
        let r = route_rules(&grid, &z, &outlet, p.epsilon, seed, p.max_area, None, p.round_rules);
        let cuts = cut_in(Some(&grid), &r);
        let queue: Vec<usize> = (0..cuts.members.len()).filter(|&k| cuts.members[k].len() > 1).collect();
        // the cell to follow: the deepest land depression cell of this routing's surface, fixed after round 1 (the first round that has one)
        if target.is_none() && round >= pick {
            target = (0..n).filter(|&c| inside(c) && z[c] > 0.0).max_by(|&x, &y| (filled[x] - z[x]).total_cmp(&(filled[y] - z[y])));
        }
        let Some(c0) = target else {
            let zr: &[f64] = &z;
            let mut next = z.clone();
            for &k in &queue {
                let mut inp = gather_with(&grid, zr, &outlet, &r, &cuts, &taper, &Drivers::default(), &p, k);
                catchment_round(&mut inp, &p, p.reroute_every);
                for (&c, v) in cuts.members[k].iter().zip(inp.z) {
                    next[c as usize] = v;
                }
            }
            z = next;
            continue;
        };
        let zz = z.clone();
        let mut chain_before: Vec<(usize, f64, f64)> = Vec::new();
        // the spill path of c0: the receiver chain until the fill drops below the level of c0's depression
        let level = filled[c0];
        let mut c = c0;
        let mut steps_out = 0;
        loop {
            chain_before.push((c, zz[c], filled[c]));
            let rc = r.receiver[c];
            if rc == NONE || chain_before.len() > 4000 {
                break;
            }
            c = rc as usize;
            if filled[c] < level - 0.5 {
                steps_out += 1;
                if steps_out > 6 {
                    break;
                }
            }
        }
        let kc = (0..cuts.members.len()).find(|&k| cuts.members[k].contains(&(c0 as u32))).unwrap();
        let drivers = Drivers::default();
        let inp_full = gather_with(&grid, &zz, &outlet, &r, &cuts, &taper, &drivers, &p, kc);
        let mut inp_breach = inp_full.clone();
        catchment_round(&mut inp_breach, &p, 0);
        let mut inp_swept = inp_full.clone();
        catchment_round(&mut inp_swept, &p, p.reroute_every);
        println!(
            "round {}: following cell {} (i {}, j {}), depression level {:.2} m, bottom z {:.2} m, depth {:.2} m, catchment of {} cells, spill path {} cells",
            round + 1,
            c0,
            i0 + c0 % w,
            j0 + c0 / w,
            level,
            zz[c0],
            level - zz[c0],
            cuts.members[kc].len(),
            chain_before.len()
        );
        println!("   path cell  dist(m)   z_start  filled   z_after_breach  z_after_sweeps  area(km2)");
        let step = (chain_before.len() / 24).max(1);
        for (idx, &(c, z_s, f)) in chain_before.iter().enumerate() {
            if idx % step != 0 && idx + 1 != chain_before.len() {
                continue;
            }
            let l = cuts.local[c] as usize;
            let dist = inp_full.dist[l];
            println!(
                "   {idx:>4} {c:>9} {dist:>8.0} {z_s:>9.2} {f:>8.2} {:>15.2} {:>15.2} {:>10.1}",
                inp_breach.z[l],
                inp_swept.z[l],
                r.area[c] / 1e6
            );
        }
        // the whole round, as solve_rounds does it
        let zr: &[f64] = &z;
        let mut next = z.clone();
        for &k in &queue {
            let mut inp = gather_with(&grid, zr, &outlet, &r, &cuts, &taper, &drivers, &p, k);
            catchment_round(&mut inp, &p, p.reroute_every);
            for (&c, v) in cuts.members[k].iter().zip(inp.z) {
                next[c as usize] = v;
            }
        }
        z = next;
    }
    stats(&grid, &z, &outlet, &inside, "after round 4");
    by_outlet(&grid, &z, &outlet, &inside, seed, &p, "after round 4");
    by_outlet(&grid, &z0, &outlet, &inside, seed, &p, "base");
    // sanity: the same rounds through the library's solver
    let mut lib = z0.clone();
    solve_rounds(&grid, &mut lib, &outlet, &p, 4, &threads, seed);
    println!("check against solve_rounds: {} cells differ", lib.iter().zip(&z).filter(|(x, y)| x.to_bits() != y.to_bits()).count());
}

/// The depression cells of `z` (more than 1 m under the fill) by the outlet their route ends at: a land outlet (the window's cut at a height above the sea) or a sea outlet.
#[allow(dead_code)]
pub fn by_outlet(grid: &Grid, z: &[f64], outlet: &[bool], inside: &dyn Fn(usize) -> bool, seed: u64, p: &CarveParams, tag: &str) {
    let filled = priority_flood_with(grid, z, outlet, 1e-3, true);
    let r = route_rules(grid, z, outlet, p.epsilon, seed, p.max_area, None, p.round_rules);
    let n = z.len();
    let mut end = vec![u32::MAX; n];
    for &c in r.order.iter() {
        let c = c as usize;
        end[c] = if r.receiver[c] == NONE { c as u32 } else { end[r.receiver[c] as usize] };
    }
    let mut groups: [Vec<f64>; 4] = Default::default(); // [window land outlet, sea outlet] x [all window, region proper]
    for c in 0..n {
        if z[c] > 0.0 && filled[c] - z[c] > 1.0 {
            let land_outlet = z[end[c] as usize] > 1.0;
            let d = filled[c] - z[c];
            groups[usize::from(!land_outlet)].push(d);
            if inside(c) {
                groups[2 + usize::from(!land_outlet)].push(d);
            }
        }
    }
    for (name, g) in
        ["window, ends at a land outlet (cut)", "window, ends at a sea outlet", "region proper, land outlet", "region proper, sea outlet"]
            .iter()
            .zip(groups.iter_mut())
    {
        g.sort_by(f64::total_cmp);
        if g.is_empty() {
            println!("{tag}: {name}: none");
        } else {
            println!(
                "{tag}: {name}: {} cells, depth p50 {:.1} p95 {:.1} max {:.1} m",
                g.len(),
                g[g.len() / 2],
                g[(g.len() - 1) * 95 / 100],
                g[g.len() - 1]
            );
        }
    }
}
