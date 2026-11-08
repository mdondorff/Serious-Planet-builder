//! The edit cases E1-E6 of the prototype region (spikes/prototype-region/results/results.md) at the erosion grid (12,288 cells, 814 m), GEN-146: for each edit the memoised re-solve (G-INC: the tile
//! floods, the catchments carved, the tiles changed) against the solve of the edited definition without a cache (G-EQ, bit equality), and the structural gates after the edit (G-PART, G-CON, the
//! conservation of `Q` and `A_d`) with the case's own checks (basin ids and the continuity map for E3, the discharge and the dirty set at the mouth for E4, initiation for E5).
//! The window is the region plus `margin` cells (its border an outlet). Usage: edit_cases [margin_cells] [threads] [tile] [only]  (`only`: a case name such as E3b)
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{basin_continuity, basin_id, hydrology_with, verify, BasinClass, Hydro, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
use planet_generators::bake::round::{solve_rounds_memo, solve_rounds_with, Drivers, RoundCache, RoundWork};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
const REGION: (usize, usize, usize, usize) = (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3);

/// The state one solve reads: the surface, the climate (precipitation mm a year, temperature) and the rock.
#[derive(Clone)]
struct World {
    z: Vec<f64>,
    pm: Vec<f32>,
    tc: Vec<f32>,
    k: Vec<f64>,
    up: Vec<f32>,
    flags: Vec<bool>,
}

struct Derived {
    weight: Vec<f32>,
    runoff: Vec<f32>,
    evap: Vec<f32>,
}

fn derive(w: &World) -> Derived {
    let n = w.z.len();
    let (mut weight, mut runoff, mut evap) = (vec![1.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    for p in 0..n {
        if w.z[p] <= 0.0 {
            continue;
        }
        let (pm, tc) = (f64::from(w.pm[p]), f64::from(w.tc[p]));
        let ro = runoff_mm(pm, tc);
        weight[p] = discharge_weight(ro) as f32;
        runoff[p] = (ro / 1000.0 / SECONDS_PER_YEAR) as f32;
        evap[p] = ((pet_mm(tc) - pm).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
    }
    Derived { weight, runoff, evap }
}

struct Ctx<'a> {
    grid: &'a Grid,
    outlet: &'a [bool],
    p: &'a CarveParams,
    stream: u64,
    threads: usize,
    tile: usize,
    i0: usize,
    j0: usize,
    w: usize,
}

impl Ctx<'_> {
    fn at(&self, i: usize, j: usize) -> usize {
        (j - self.j0) * self.w + (i - self.i0)
    }
    fn cell_of(&self, c: usize) -> (usize, usize) {
        (self.i0 + c % self.w, self.j0 + c / self.w)
    }
}

fn drivers<'a>(w: &'a World, d: &'a Derived) -> Drivers<'a> {
    Drivers { k_scale: Some(&w.k), uplift: Some(&w.up), runoff: Some(&d.weight) }
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A block raised by `m` metres with a smooth edge of `edge` cells.
fn raise_block(c: &Ctx, z: &mut [f64], (i0, i1, j0, j1): (usize, usize, usize, usize), m: f64, edge: f64) {
    for j in j0.saturating_sub(8)..=(j1 + 8).min(c.j0 + c.grid.h() - 1) {
        for i in i0.saturating_sub(8)..=(i1 + 8).min(c.i0 + c.w - 1) {
            let dx = (i0 as f64 - i as f64).max(i as f64 - i1 as f64).max(0.0);
            let dy = (j0 as f64 - j as f64).max(j as f64 - j1 as f64).max(0.0);
            let d = (dx * dx + dy * dy).sqrt();
            z[c.at(i, j)] += m * (1.0 - smooth(d / edge));
        }
    }
}

fn stats_line(label: &str, work: &[RoundWork], base: &[RoundWork]) {
    let fl: usize = work.iter().map(|w| w.tile_floods).sum();
    let fl0: usize = base.iter().map(|w| w.tile_floods).sum();
    let ca: usize = work.iter().map(|w| w.carved).sum();
    let ca0: usize = base.iter().map(|w| w.carved).sum();
    let cc: usize = work.iter().map(|w| w.cells_carved).sum();
    let cc0: usize = base.iter().map(|w| w.cells_carved).sum();
    println!(
        "  G-INC {label}: tile floods {fl} of {fl0} ({:.1} %); catchments carved {ca} of {ca0} ({:.2} %); cells carved {cc} of {cc0} ({:.2} %)",
        100.0 * fl as f64 / fl0.max(1) as f64,
        100.0 * ca as f64 / ca0.max(1) as f64,
        100.0 * cc as f64 / cc0.max(1) as f64
    );
    for (r, w) in work.iter().enumerate() {
        println!(
            "    round {}: tile floods {} of {} tiles, tiles changed (filled {}, receivers {}, A_d {}); catchments carved {} of {}, cells {}",
            r + 1,
            w.tile_floods,
            w.tiles,
            w.tiles_filled_changed,
            w.tiles_receiver_changed,
            w.tiles_area_changed,
            w.carved,
            w.catchments,
            w.cells_carved
        );
    }
}

fn h1(
    c: &Ctx,
    w: &World,
    d: &Derived,
    z: &[f64],
) -> (Hydro, Vec<f64>, planet_generators::bake::route::Routing, planet_generators::bake::hydro::Verify) {
    let routing = route_rules(c.grid, z, c.outlet, 1e-3, c.stream, 0.0, Some(&d.weight), RoundRules::ROUND);
    let filled = priority_flood_with(c.grid, z, c.outlet, 1e-3, true);
    let hy =
        hydrology_with(c.grid, z, &filled, &routing, c.outlet, Some(&d.runoff), Some(&d.evap), Some(&w.flags), &HydroParams::default());
    let v = verify(c.grid, &hy, &routing, &filled, c.outlet, Some(&d.runoff), Some(&d.weight));
    (hy, filled, routing, v)
}

/// The structural gates of the world after an edit: G-PART on its surface, G-CON and the conservation on the solved surface.
fn gates(c: &Ctx, w: &World, d: &Derived, solved: &[f64]) -> Hydro {
    let want = route_rules(c.grid, &w.z, c.outlet, 1e-3, c.stream, 0.0, Some(&d.weight), RoundRules::ROUND);
    let got = partitioned_route_weighted(
        c.grid,
        &w.z,
        c.outlet,
        1e-3,
        c.stream,
        c.tile,
        RoundRules::ROUND,
        Some(&d.weight),
        &mut PartCache::default(),
    );
    let dr = got.routing.receiver.iter().zip(&want.receiver).filter(|(a, b)| a != b).count();
    let da = got.routing.area.iter().zip(&want.area).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    let (hy, _, _, v) = h1(c, w, d, solved);
    println!(
        "  G-PART {dr} receivers, {da} A_d differ; G-CON {} disconnected, {} broken, {} miscovered; Q confluences {:.1e}, outlets {:.1e}; A_d outlets {:.1e}; channel cells {} ({} heads), basins {}",
        v.disconnected_segments, v.broken_channel_cells, v.miscovered_cells, v.max_confluence_error, v.q_outlets_error, v.area_outlets_error, hy.stats.channel_cells, hy.stats.heads, hy.stats.basins
    );
    hy
}

#[allow(clippy::too_many_arguments)]
fn run_edit(
    c: &Ctx,
    name: &str,
    base_w: &World,
    base_z: &[f64],
    base_work: &[RoundWork],
    cache: &mut RoundCache,
    edited: &World,
) -> (Vec<f64>, Vec<RoundWork>, Hydro) {
    let t = Instant::now();
    // The base definition through the cache again, so that the change counts read against it.
    let d0 = derive(base_w);
    let mut z = base_w.z.clone();
    solve_rounds_memo(c.grid, &mut z, c.outlet, c.p, 4, &c.threads, c.stream, &drivers(base_w, &d0), c.tile, cache);
    let d = derive(edited);
    let mut z = edited.z.clone();
    let (_, work) = solve_rounds_memo(c.grid, &mut z, c.outlet, c.p, 4, &c.threads, c.stream, &drivers(edited, &d), c.tile, cache);
    let memo_s = t.elapsed().as_secs_f64();
    let mut f = edited.z.clone();
    let t = Instant::now();
    solve_rounds_with(c.grid, &mut f, c.outlet, c.p, 4, &c.threads, c.stream, &drivers(edited, &d));
    let fresh_s = t.elapsed().as_secs_f64();
    let differ = z.iter().zip(&f).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    let changed = z.iter().zip(base_z).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    println!("{name}: G-EQ {differ} cells differ (memoised {memo_s:.1} s incl. the base pass, fresh solve {fresh_s:.1} s); {changed} cells differ from the base result");
    stats_line(name, &work, base_work);
    let hy = gates(c, edited, &d, &z);
    (z, work, hy)
}

fn find_basin(hy: &Hydro, c: &Ctx, i: usize, j: usize) -> Option<usize> {
    for r in 0..=8usize {
        for dj in -(r as i64)..=(r as i64) {
            for di in -(r as i64)..=(r as i64) {
                let (x, y) = ((i as i64 + di) as usize, (j as i64 + dj) as usize);
                if x < c.i0 || y < c.j0 || x >= c.i0 + c.w || y >= c.j0 + c.grid.h() {
                    continue;
                }
                let l = hy.label[c.at(x, y)];
                if l != NONE {
                    return Some(l as usize);
                }
            }
        }
    }
    None
}

fn describe(hy: &Hydro, k: usize, c: &Ctx) -> String {
    let b = &hy.basins[k];
    let (fi, fj) = c.cell_of(b.floor_cell as usize);
    let (si, sj) = c.cell_of(b.spill_cell as usize);
    format!(
        "id {:016x}, floor {:.0} m at ({fi}, {fj}), spill {:.0} m at ({si}, {sj}), {} cells, {:?}, level {:.0} m, q_in {:.2} m3/s, q_out {:.2}",
        b.id, b.floor_m, b.spill_m, b.cells, b.class, b.water_level_m, b.q_in, b.q_out
    )
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let margin: usize = a.get(1).map_or(400, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let tile: usize = a.get(3).map_or(256, |s| s.parse().unwrap());
    let only = a.get(4).cloned();
    let (i0, i1) = (REGION.0.saturating_sub(margin), (REGION.1 + margin).min(CELLS - 1));
    let (j0, j1) = (REGION.2.saturating_sub(margin), (REGION.3 + margin).min(CELLS - 1));
    let (w, h) = (i1 - i0 + 1, j1 - j0 + 1);
    let n = w * h;
    let seed = 14u64;
    let spec = FaceSpec { face: Face(1), cells: CELLS, radius_m: R, sea_level: 0.0, seed, halo: 0 };
    let inputs = BakeInputs::new(seed, threads);
    let o = inputs.orography();
    let t0 = Instant::now();
    let mut base = World { z: vec![0.0; n], pm: vec![0.0; n], tc: vec![0.0; n], k: vec![1.0; n], up: vec![20.0; n], flags: vec![false; n] };
    let rows = h.div_ceil(threads);
    std::thread::scope(|s| {
        let chunks = base.z.chunks_mut(rows * w).zip(base.pm.chunks_mut(rows * w)).zip(base.tc.chunks_mut(rows * w));
        for (k, ((zc, pc), tcc)) in chunks.enumerate() {
            let (inputs, spec) = (&inputs, &spec);
            s.spawn(move || {
                for c in 0..zc.len() {
                    let p = k * rows * w + c;
                    let d: Vec3 = spec.halo_direction((i0 + p % w) as i64, (j0 + p / w) as i64);
                    zc[c] = inputs.base(d);
                    pc[c] = o.precipitation_mm(d) as f32;
                    tcc[c] = o.temperature_c(d) as f32;
                }
            });
        }
    });
    println!("window i {i0}-{i1} j {j0}-{j1}: {w} x {h} = {n} cells, sampled in {:.1} s", t0.elapsed().as_secs_f64());
    let grid = Grid::new(w, h, spec.dx()).with_origin((i0 as u32, j0 as u32)).with_metric(Metric::cube_face(
        CELLS,
        R,
        (i0 as i64, j0 as i64),
        w,
        h,
    ));
    let outlet = grid.outlets(&base.z, 0.0);
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, cell_metric: true, ..CarveParams::default() };
    let c = Ctx { grid: &grid, outlet: &outlet, p: &p, stream: spec.stream(), threads, tile, i0, j0, w };
    // E3d: a basin named by an item at the 3,072-grid cell (2155, 205).
    for j in 818..=826 {
        for i in 8618..=8626 {
            base.flags[c.at(i, j)] = true;
        }
    }

    // ---- the base solve and the base basins (on the unsolved surface, where the E3 fixtures were found)
    let mut cache = RoundCache::default();
    let d_base = derive(&base);
    let mut z_base = base.z.clone();
    let t = Instant::now();
    let (_, work_base) =
        solve_rounds_memo(&grid, &mut z_base, &outlet, &p, 4, &threads, c.stream, &drivers(&base, &d_base), tile, &mut cache);
    println!("base solve (memoised, 4 rounds): {:.1} s; {} catchment rounds held", t.elapsed().as_secs_f64(), cache.len());
    let hy_base = gates(&c, &base, &d_base, &z_base);
    let (hy0, _, _, _) = h1(&c, &base, &d_base, &base.z);
    let run = |name: &str| only.as_ref().is_none_or(|o| name.starts_with(o.as_str()));

    // ---- E1: raise a range
    let mut e1 = base.clone();
    if run("E1") {
        raise_block(&c, &mut e1.z, (8612, 8675, 780, 843), 1000.0, 8.0);
        let (_, _, hy) = run_edit(&c, "E1 raise a range by 1,000 m", &base, &z_base, &work_base, &mut cache, &e1);
        let named = hy.basins.iter().filter(|b| b.class == BasinClass::Intentional).count();
        println!(
            "  E3d: {named} intentional basin(s) kept through E1 (base: {})",
            hy_base.basins.iter().filter(|b| b.class == BasinClass::Intentional).count()
        );
    }
    // ---- E2: move a divide
    if run("E2") {
        let mut e2 = base.clone();
        for j in 1240..=1247 {
            for i in 8120..=8127 {
                e2.z[c.at(i, j)] -= 100.0;
            }
        }
        run_edit(&c, "E2a lower the pass by 100 m", &base, &z_base, &work_base, &mut cache, &e2);
        let mut e2 = base.clone();
        let (x0, y0, x1, y1) = (8048i64, 1192i64, 8148i64, 1264i64);
        for s in 0..=100 {
            let (x, y) = (x0 + (x1 - x0) * s / 100, y0 + (y1 - y0) * s / 100);
            for dj in -2..=2 {
                for di in -2..=2 {
                    e2.z[c.at((x + di) as usize, (y + dj) as usize)] += 200.0;
                }
            }
        }
        run_edit(&c, "E2b raise a ridge line by 200 m", &base, &z_base, &work_base, &mut cache, &e2);
    }
    // ---- E3: basins (on the surfaces of the world, before erosion)
    if run("E3") {
        let (ai, aj, bi, bj) = (2082 * 4 + 2, 216 * 4 + 2, 2095 * 4 + 2, 224 * 4 + 2);
        match (find_basin(&hy0, &c, ai, aj), find_basin(&hy0, &c, bi, bj)) {
            (Some(ka), Some(kb)) if ka != kb => {
                println!("E3 basins at the base surface:\n  A {}\n  B {}", describe(&hy0, ka, &c), describe(&hy0, kb, &c));
                let (fa, fb) = (c.cell_of(hy0.basins[ka].floor_cell as usize), c.cell_of(hy0.basins[kb].floor_cell as usize));
                let (sa, ss) = (hy0.basins[ka].spill_cell as usize, hy0.basins[ka].spill_m);
                // E3a: cut 25 m at the spill of A.
                let mut e3a = base.clone();
                let (si, sj) = c.cell_of(sa);
                for j in sj - 2..=sj + 2 {
                    for i in si - 2..=si + 2 {
                        e3a.z[c.at(i, j)] -= 25.0;
                    }
                }
                let (hy3a, _, _, _) = h1(&c, &e3a, &derive(&e3a), &e3a.z);
                let ka2 = find_basin(&hy3a, &c, fa.0, fa.1);
                println!("E3a cut 25 m at the spill ({si}, {sj}, spill level {ss:.0} m):");
                match ka2 {
                    Some(k) => println!(
                        "  A after: {}; id {}; class {:?} -> {:?}",
                        describe(&hy3a, k, &c),
                        if hy3a.basins[k].id == hy0.basins[ka].id { "stays" } else { "CHANGED" },
                        hy0.basins[ka].class,
                        hy3a.basins[k].class
                    ),
                    None => println!("  A is no longer a basin (the cut drained it)"),
                }
                run_edit(&c, "E3a (rounds)", &base, &z_base, &work_base, &mut cache, &e3a);
                // E3b: a trench joins A and B.
                let mut e3b = base.clone();
                let target = hy0.basins[ka].floor_m.max(hy0.basins[kb].floor_m) + 2.0;
                let steps = 400;
                for s in 0..=steps {
                    let x = fa.0 as i64 + (fb.0 as i64 - fa.0 as i64) * s / steps;
                    let y = fa.1 as i64 + (fb.1 as i64 - fa.1 as i64) * s / steps;
                    for dj in -1..=1 {
                        for di in -1..=1 {
                            let idx = c.at((x + di) as usize, (y + dj) as usize);
                            e3b.z[idx] = e3b.z[idx].min(target);
                        }
                    }
                }
                let d3b = derive(&e3b);
                let (hy3b, _, _, _) = h1(&c, &e3b, &d3b, &e3b.z);
                let km = find_basin(&hy3b, &c, fa.0, fa.1);
                println!("E3b trench between the floors ({} cells long):", steps);
                if let Some(k) = km {
                    println!("  merged: {}", describe(&hy3b, k, &c));
                    let map = basin_continuity(&hy0, &hy3b);
                    for (old, news) in map.iter().filter(|(id, _)| *id == hy0.basins[ka].id || *id == hy0.basins[kb].id) {
                        println!(
                            "  continuity: {old:016x} -> {:?}",
                            news.iter().take(3).map(|(id, c)| format!("{id:016x} ({c} cells)")).collect::<Vec<_>>()
                        );
                    }
                    let deeper = if hy0.basins[ka].floor_m <= hy0.basins[kb].floor_m { ka } else { kb };
                    println!("  the merged id is the deeper basin's id: {}", hy3b.basins[k].id == hy0.basins[deeper].id);
                    // E3c: raise a sill across the middle of the trench: the reverse.
                    let mut e3c = e3b.clone();
                    for s in steps * 2 / 5..=steps * 3 / 5 {
                        let x = fa.0 as i64 + (fb.0 as i64 - fa.0 as i64) * s / steps;
                        let y = fa.1 as i64 + (fb.1 as i64 - fa.1 as i64) * s / steps;
                        for dj in -3..=3 {
                            for di in -3..=3 {
                                let idx = c.at((x + di) as usize, (y + dj) as usize);
                                e3c.z[idx] = e3c.z[idx].max(base.z[idx] + 30.0);
                            }
                        }
                    }
                    let (hy3c, _, _, _) = h1(&c, &e3c, &derive(&e3c), &e3c.z);
                    let (pa, pb) = (find_basin(&hy3c, &c, fa.0, fa.1), find_basin(&hy3c, &c, fb.0, fb.1));
                    println!("E3c a sill across the merged basin:");
                    for (lab, kk, old) in [("A", pa, ka), ("B", pb, kb)] {
                        match kk {
                            Some(k) => println!(
                                "  {lab}: {}; old id {}",
                                describe(&hy3c, k, &c),
                                if hy3c.basins[k].id == hy0.basins[old].id { "kept" } else { "NEW" }
                            ),
                            None => println!("  {lab}: no basin"),
                        }
                    }
                    for (old, news) in basin_continuity(&hy3b, &hy3c) {
                        println!(
                            "  continuity of the merged basin {old:016x} -> {:?}",
                            news.iter().take(3).map(|(id, c)| format!("{id:016x} ({c} cells)")).collect::<Vec<_>>()
                        );
                    }
                    run_edit(&c, "E3b (rounds)", &base, &z_base, &work_base, &mut cache, &e3b);
                    run_edit(&c, "E3c (rounds)", &base, &z_base, &work_base, &mut cache, &e3c);
                } else {
                    println!("  the trench left no basin at A's floor");
                }
            }
            (a, b) => println!("E3: the fixture basins are not both present at this grid (A {a:?}, B {b:?}); skipped"),
        }
    }
    // ---- E4: far downstream
    if run("E4") {
        let (_, _, routing, _) = h1(&c, &base, &d_base, &z_base);
        let trunk = c.at(2097 * 4 + 2, 239 * 4 + 2);
        let mouth = c.at(2095 * 4 + 2, 271 * 4 + 2);
        let mut up = trunk;
        for _ in 0..120 {
            let mut best: Option<usize> = None;
            for d in 0..8 {
                if let Some(q) = grid.neighbour(up, d) {
                    if routing.receiver[q] == up as u32 && best.is_none_or(|b| routing.area[q] > routing.area[b]) {
                        best = Some(q);
                    }
                }
            }
            match best {
                Some(q) => up = q,
                None => break,
            }
        }
        let (ui, uj) = c.cell_of(up);
        println!(
            "E4 trunk ({}, {}) A_d {:.0} km2; the block is 120 cells upstream at ({ui}, {uj}); the mouth cell ({}, {}) A_d {:.0} km2",
            c.cell_of(trunk).0,
            c.cell_of(trunk).1,
            routing.area[trunk] / 1e6,
            c.cell_of(mouth).0,
            c.cell_of(mouth).1,
            routing.area[mouth] / 1e6
        );
        let (_, _, _, _) = (0, 0, 0, 0);
        let (hy_b, _, _, _) = h1(&c, &base, &d_base, &z_base);
        let mut e4 = base.clone();
        raise_block(&c, &mut e4.z, (ui - 30, ui + 30, uj - 30, uj + 30), 500.0, 8.0);
        let (z4, _, hy4) = run_edit(&c, "E4a raise a 50 km block upstream by 500 m", &base, &z_base, &work_base, &mut cache, &e4);
        let d4 = derive(&e4);
        let (_, _, r4, _) = h1(&c, &e4, &d4, &z4);
        println!(
            "  Q at the trunk cell {:.0} -> {:.0} m3/s; at the mouth cell {:.0} -> {:.0} m3/s",
            hy_b.q[trunk], hy4.q[trunk], hy_b.q[mouth], hy4.q[mouth]
        );
        let _ = r4;
        let mut e4b = base.clone();
        for j in uj - 30..=uj + 30 {
            for i in ui - 30..=ui + 30 {
                let k = c.at(i, j);
                e4b.pm[k] *= 1.5;
            }
        }
        let (_, _, hy4b) = run_edit(&c, "E4b raise the rainfall of the block by 50 %", &base, &z_base, &work_base, &mut cache, &e4b);
        println!(
            "  Q at the trunk cell {:.0} -> {:.0} m3/s; at the mouth cell {:.0} -> {:.0} m3/s",
            hy_b.q[trunk], hy4b.q[trunk], hy_b.q[mouth], hy4b.q[mouth]
        );
    }
    // ---- E5: rainfall only
    if run("E5") {
        let mut e5 = base.clone();
        for j in 984..=1111 {
            for i in 8144..=8271 {
                let k = c.at(i, j);
                e5.pm[k] *= 2.0;
            }
        }
        let (_, _, hy) = run_edit(&c, "E5 double the rainfall of a window (no height change)", &base, &z_base, &work_base, &mut cache, &e5);
        println!(
            "  initiation: channel cells {} -> {}, heads {} -> {}, segments {} -> {}",
            hy_base.stats.channel_cells,
            hy.stats.channel_cells,
            hy_base.stats.heads,
            hy.stats.heads,
            hy_base.stats.segments,
            hy.stats.segments
        );
    }
    // ---- E6: rock and uplift
    if run("E6") {
        let mut e6 = base.clone();
        for j in 840..=967 {
            for i in 8544..=8671 {
                let k = c.at(i, j);
                e6.k[k] *= 0.29 / 0.1;
            }
        }
        run_edit(&c, "E6a a softer lithology (hardness 0.29 -> 0.1) over a block", &base, &z_base, &work_base, &mut cache, &e6);
        let mut e6 = base.clone();
        for j in 840..=967 {
            for i in 8544..=8671 {
                let k = c.at(i, j);
                e6.up[k] *= 2.0;
            }
        }
        run_edit(&c, "E6b uplift x 2 over a block", &base, &z_base, &work_base, &mut cache, &e6);
    }
    let _ = basin_id;
}
