//! GEN-144: the landmass of the prototype region routed whole across the cube edges (A3 section 6.1): the six faces as one grid (`Grid::cube`), the landmass found from the region's centre by
//! the land cells connected through the cube graph, its coast ring as outlets, then the resident route, the partitioned route (G-PART), 4 rounds of 5 steps (the round solver) and, with
//! `reference`, the whole-region carve (G-ROUND). The region's change is compared with the window runs of `rounds_region` (an artificial border).
//! Usage: landmass_region [cells_a_face_edge] [tile] [threads] [reference]. The base heights are cached in `target/landmass_base_<n>.f32`.
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::partition::{partitioned_route, PartCache};
use planet_generators::bake::round::{cut_in, solve_rounds};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use std::time::Instant;

const R: f64 = 6_371_000.0;
const SEED_CELL: (usize, usize, usize) = (1, 2090, 267);
const REGION: (usize, usize, usize, usize) = (2014, 2168, 190, 344);

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).map_or(3072, |s| s.parse().unwrap());
    let tile: usize = a.get(2).map_or(256, |s| s.parse().unwrap());
    let threads: usize = a.get(3).map_or(8, |s| s.parse().unwrap());
    let reference = a.get(4).is_some_and(|s| s == "reference");
    let total = 6 * n * n;
    let seed = 14u64;
    let t0 = Instant::now();

    // ---- the base heights of the whole sphere (cached)
    let cache = std::path::PathBuf::from(format!("target/landmass_base_{n}.f32"));
    let mut base = vec![0f32; total];
    if let Ok(bytes) = std::fs::read(&cache) {
        assert_eq!(bytes.len(), total * 4, "the cache holds another size");
        for (k, c) in bytes.chunks_exact(4).enumerate() {
            base[k] = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        }
        println!("base heights from the cache in {:.1} s", t0.elapsed().as_secs_f64());
    } else {
        let inputs = BakeInputs::new(seed, threads);
        for f in 0..6u8 {
            let spec = FaceSpec { face: Face(f), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
            let rows = n.div_ceil(threads);
            let face = &mut base[f as usize * n * n..(f as usize + 1) * n * n];
            std::thread::scope(|s| {
                for (k, chunk) in face.chunks_mut(rows * n).enumerate() {
                    let (inputs, spec) = (&inputs, &spec);
                    s.spawn(move || {
                        for (c, v) in chunk.iter_mut().enumerate() {
                            let p = k * rows * n + c;
                            let d: Vec3 = spec.halo_direction((p % n) as i64, (p / n) as i64);
                            *v = inputs.base(d) as f32;
                        }
                    });
                }
            });
            println!("  face {f} sampled ({:.1} s)", t0.elapsed().as_secs_f64());
        }
        let mut bytes = Vec::with_capacity(total * 4);
        base.iter().for_each(|v| bytes.extend_from_slice(&v.to_le_bytes()));
        std::fs::write(&cache, bytes).unwrap();
    }

    // ---- the landmass of the region's centre, through the cube graph
    let full = Grid::cube(n, R);
    let id = |f: usize, i: usize, j: usize| f * n * n + j * n + i;
    let start = id(SEED_CELL.0, SEED_CELL.1, SEED_CELL.2);
    assert!(base[start] > 0.0, "the region's centre is land");
    let mut land = vec![false; total];
    let mut stack = vec![start as u32];
    land[start] = true;
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = full.neighbour(p as usize, d) {
                if !land[q] && base[q] > 0.0 {
                    land[q] = true;
                    stack.push(q as u32);
                }
            }
        }
    }
    let mut active = land.clone();
    for p in 0..total {
        if land[p] {
            for d in 0..8 {
                if let Some(q) = full.neighbour(p, d) {
                    active[q] = true;
                }
            }
        }
    }
    drop(full);
    let land_cells = land.iter().filter(|&&l| l).count();
    let active_cells = active.iter().filter(|&&a| a).count();
    let grid = Grid::cube(n, R).with_active(active.clone());
    let land_km2: f64 = (0..total).filter(|&p| land[p]).map(|p| grid.cell_area(p)).sum::<f64>() / 1e6;
    let mut faces = [0usize; 6];
    (0..total).filter(|&p| land[p]).for_each(|p| faces[p / (n * n)] += 1);
    println!(
        "landmass: {land_cells} land cells ({land_km2:.0} km2), {active_cells} cells with the coast ring, land cells a face {faces:?}"
    );
    let z0: Vec<f64> = base.iter().map(|&v| f64::from(v)).collect();
    drop(base);
    let outlet: Vec<bool> = (0..total).map(|p| active[p] && z0[p] <= 0.0).collect();
    let eps = 1e-3;
    let stream = seed; // one stream for the whole sphere
    let rules = RoundRules::ROUND;

    // ---- the resident route
    let t = Instant::now();
    let res = route_rules(&grid, &z0, &outlet, eps, stream, 0.0, None, rules);
    println!("resident route: {:.1} s", t.elapsed().as_secs_f64());
    let crossings = (0..total).filter(|&p| land[p] && res.receiver[p] != NONE && p / (n * n) != res.receiver[p] as usize / (n * n)).count();
    let cuts = cut_in(Some(&grid), &res);
    let largest = cuts.members.iter().map(Vec::len).max().unwrap_or(0);
    let held: f64 = (0..total).filter(|&p| outlet[p]).map(|p| res.area[p]).sum::<f64>() / 1e6;
    println!(
        "  {} catchments, largest {largest} cells; {crossings} land receivers cross a cube edge; A_d at the outlets {held:.0} km2 (land {land_km2:.0} km2 + outlet cells' own area)",
        cuts.members.len()
    );
    drop(cuts);

    // ---- G-PART on the landmass
    let mut pc = PartCache::default();
    let t = Instant::now();
    let part = partitioned_route(&grid, &z0, &outlet, eps, stream, tile, rules, &mut pc);
    let part_s = t.elapsed().as_secs_f64();
    let dr = (0..total).filter(|&p| active[p] && part.routing.receiver[p] != res.receiver[p]).count();
    let da = (0..total).filter(|&p| active[p] && part.routing.area[p].to_bits() != res.area[p].to_bits()).count();
    println!(
        "G-PART, tile {tile}: {} tiles, {} sweeps, {} tile floods, {part_s:.1} s; against the resident route {dr} receivers, {da} A_d differ",
        part.stats.tiles, part.stats.sweeps, part.stats.floods
    );
    let fresh_floods = part.stats.floods;
    drop(part);
    // E2 on the landmass: the pass between the two mouths' catchments lowered by 100 m.
    let mut e = z0.clone();
    e[id(1, 2031, 311)] -= 100.0;
    let t = Instant::now();
    let ed = partitioned_route(&grid, &e, &outlet, eps, stream, tile, rules, &mut pc);
    let want = route_rules(&grid, &e, &outlet, eps, stream, 0.0, None, rules);
    let dr = (0..total).filter(|&p| active[p] && ed.routing.receiver[p] != want.receiver[p]).count();
    let da = (0..total).filter(|&p| active[p] && ed.routing.area[p].to_bits() != want.area[p].to_bits()).count();
    println!(
        "E2 pass lowered by 100 m: {} of {fresh_floods} tile floods ({:.1} %), tiles changed filled {} receivers {} A_d {}; {:.1} s; against the resident route {dr} receivers, {da} A_d differ",
        ed.stats.floods,
        100.0 * ed.stats.floods as f64 / fresh_floods as f64,
        ed.stats.tiles_filled_changed,
        ed.stats.tiles_receiver_changed,
        ed.stats.tiles_area_changed,
        t.elapsed().as_secs_f64()
    );
    drop((ed, want, e, pc, res));

    // ---- the rounds
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: rules, ..CarveParams::default() };
    let mut z = z0.clone();
    let t = Instant::now();
    let rep = solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, stream);
    println!("round solver, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
    for (r, s) in rep.rounds.iter().enumerate() {
        println!(
            "  round {}: {} catchments, largest {} cells, {} cells in catchments of 1,000 or more",
            r + 1,
            s.catchments,
            s.largest,
            s.in_large
        );
    }
    let mut dz: Vec<f64> = Vec::new();
    for j in REGION.2..=REGION.3 {
        for i in REGION.0..=REGION.1 {
            let k = id(1, i, j);
            if z0[k] > 0.0 {
                dz.push(z[k] - z0[k]);
            }
        }
    }
    dz.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| dz[((dz.len() - 1) as f64 * f) as usize];
    println!(
        "region land change (m): n {} p5 {:.1} p50 {:.1} p95 {:.1} max {:.1} (the window runs: p5 -565.4 p50 -132.4 p95 -1.7 max 16.0)",
        dz.len(),
        q(0.05),
        q(0.5),
        q(0.95),
        dz[dz.len() - 1]
    );
    if reference {
        let mut want = z0.clone();
        let t = Instant::now();
        carve(&grid, &mut want, &outlet, &p, None, stream);
        let d = (0..total).filter(|&k| active[k] && want[k].to_bits() != z[k].to_bits()).count();
        println!("whole-region carve (same rules): {:.1} s; G-ROUND: {d} cells differ", t.elapsed().as_secs_f64());
    }
}
