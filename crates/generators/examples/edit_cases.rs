//! The edit cases E1-E6 of the prototype region (spikes/prototype-region/results/results.md) at the erosion grid (12,288 cells, 814 m), GEN-146: for each edit the memoised re-solve (G-INC: the tile
//! floods, the catchments carved, the tiles changed) against the solve of the edited definition without a cache (G-EQ, bit equality), and the structural gates after the edit (G-PART, G-CON, the
//! conservation of `Q` and `A_d`) with the case's own checks (basin ids and the continuity map for E3, the discharge at the outlet for E4, initiation for E5).
//! The window is the region plus `margin` cells (its border an outlet). Usage: edit_cases [margin_cells] [threads] [tile] [only]  (`only`: a case name prefix such as E3; `AUX`: the base solve, one edit and the store rows of A3 section 10 that were to measure, G-STORE item 14, then stop)
#![allow(clippy::needless_range_loop)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{basin_continuity, hydrology_with, verify, BasinClass, Hydro, HydroParams, Verify};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
use planet_generators::bake::round::{solve_rounds_memo, solve_rounds_with, Drivers, RoundCache, RoundWork};
use planet_generators::bake::route::{route_rules, RoundRules, Routing, NONE};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

const CELLS: usize = 12_288;
const R: f64 = 6_371_000.0;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;
const REGION: (usize, usize, usize, usize) = (2014 * 4, 2168 * 4 + 3, 190 * 4, 344 * 4 + 3);

/// The state one solve reads: the surface, the climate (precipitation mm a year, temperature), the rock and the cells named by items.
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
        evap[p] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR) as f32;
    }
    Derived { weight, runoff, evap }
}

#[derive(Clone, Copy)]
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
    fn inside(&self, i: usize, j: usize) -> bool {
        i >= self.i0 && j >= self.j0 && i < self.i0 + self.w && j < self.j0 + self.grid.h()
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
            z[c.at(i, j)] += m * (1.0 - smooth((dx * dx + dy * dy).sqrt() / edge));
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

fn h1(c: &Ctx, w: &World, d: &Derived, z: &[f64]) -> (Hydro, Routing, Verify) {
    let routing = route_rules(c.grid, z, c.outlet, 1e-3, c.stream, 0.0, Some(&d.weight), RoundRules::ROUND);
    let filled = priority_flood_with(c.grid, z, c.outlet, 1e-3, true);
    let hy =
        hydrology_with(c.grid, z, &filled, &routing, c.outlet, Some(&d.runoff), Some(&d.evap), Some(&w.flags), &HydroParams::default());
    let v = verify(c.grid, &hy, &routing, &filled, c.outlet, Some(&d.runoff), Some(&d.weight));
    (hy, routing, v)
}

/// The structural gates of the world after an edit: G-PART on its surface, G-CON and the conservation on the solved surface.
fn gates(c: &Ctx, w: &World, d: &Derived, solved: &[f64]) -> (Hydro, Routing) {
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
    let (hy, routing, v) = h1(c, w, d, solved);
    // G-EQ on the complete result: H1 from the partitioned route of the solved surface against H1 from the resident one.
    let part = partitioned_route_weighted(
        c.grid,
        solved,
        c.outlet,
        1e-3,
        c.stream,
        c.tile,
        RoundRules::ROUND,
        Some(&d.weight),
        &mut PartCache::default(),
    );
    let hp = hydrology_with(
        c.grid,
        solved,
        &part.filled,
        &part.routing,
        c.outlet,
        Some(&d.runoff),
        Some(&d.evap),
        Some(&w.flags),
        &HydroParams::default(),
    );
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let whole = usize::from(bits(&hp.q) != bits(&hy.q))
        + usize::from(bits(&hp.qw) != bits(&hy.qw))
        + usize::from(hp.channel != hy.channel)
        + usize::from(hp.segments != hy.segments)
        + usize::from(hp.basins != hy.basins)
        + usize::from(hp.label != hy.label);
    println!("  G-EQ whole result (rounds above; H1 channels, Q, Q_w, segments, basins and levels from the partitioned route): {whole} of 6 parts differ");
    println!("  Q_w closure {:.1e}, Q_w at confluences {:.1e}", v.qw_closure_error, v.qw_confluence_error);
    println!(
        "  G-PART {dr} receivers, {da} A_d differ; G-CON {} disconnected, {} broken, {} miscovered; Q confluences {:.1e}, outlets {:.1e}; A_d outlets {:.1e}; channel cells {} ({} heads), basins {}",
        v.disconnected_segments, v.broken_channel_cells, v.miscovered_cells, v.max_confluence_error, v.q_outlets_error, v.area_outlets_error, hy.stats.channel_cells, hy.stats.heads, hy.stats.basins
    );
    (hy, routing)
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
    edited_outlet: &[bool],
) -> (Vec<f64>, Hydro, Routing) {
    let base_outlet = c.outlet;
    let ce = Ctx { outlet: edited_outlet, ..*c };
    let c = &ce;
    let t = Instant::now();
    // The base definition through the cache again, so that the change counts read against it.
    let d0 = derive(base_w);
    let mut z = base_w.z.clone();
    solve_rounds_memo(c.grid, &mut z, base_outlet, c.p, 4, &c.threads, c.stream, &drivers(base_w, &d0), c.tile, cache);
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
    let (hy, routing) = gates(c, edited, &d, &z);
    (z, hy, routing)
}

fn find_basin(hy: &Hydro, c: &Ctx, i: usize, j: usize) -> Option<usize> {
    for r in 0..=8i64 {
        for dj in -r..=r {
            for di in -r..=r {
                let (x, y) = ((i as i64 + di) as usize, (j as i64 + dj) as usize);
                if !c.inside(x, y) {
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

fn outlet_of(r: &Routing, from: usize) -> usize {
    let mut m = from;
    while r.receiver[m] != NONE {
        m = r.receiver[m] as usize;
    }
    m
}

/// G-STORE (item 14, inbox #123): the four rows of A3 section 10 that were "to measure", from a memoised base solve and one edit of this window, in bytes at stated layouts and scaled to Earth
/// by the land cells (2.25e8 at 12,288 cells a face edge). `edit_cases <margin> <threads> <tile> AUX` runs this and stops.
fn aux_report(c: &Ctx, base: &World, d_base: &Derived, z_base: &[f64], hy_base: &Hydro, cache: &mut RoundCache) {
    const EARTH_LAND: f64 = 2.25e8;
    let n = z_base.len();
    let land = z_base.iter().filter(|&&v| v > 0.0).count();
    let scale = EARTH_LAND / land as f64;
    let gb = |b: f64| b * scale / 1e9;
    println!("AUX window {n} cells, {land} land cells; Earth is {scale:.1} times the land of this window");
    // 1. The catchment-round key table: one entry a catchment round the memo holds (4 rounds): the key (128 bit), the outlet cell (u32) and the member count (u32).
    let entries = cache.len();
    println!(
        "AUX 1 catchment-round key table: {entries} entries over 4 rounds ({:.0} a round); 24 B an entry (key 16, outlet 4, members 4) = {} B here, {:.4} GB at Earth; the memo that holds the results as well is a cache of {:.1} MB here ({:.2} GB at Earth, evictable, GEN-153)",
        entries as f64 / 4.0,
        entries * 24,
        gb((entries * 24) as f64),
        cache.bytes() as f64 / 1e6,
        gb(cache.bytes() as f64)
    );
    // 2. Per-tile border summaries: for every tile with land, the border cells (a cell with a neighbour in another tile) carry the lowest value the neighbour offers (f32) and the area share that crosses (f64).
    let (tile, w) = (c.tile, c.w);
    let (tw, th) = (w.div_ceil(tile), c.grid.h().div_ceil(tile));
    let mut border_cells = 0usize;
    let mut land_tiles = 0usize;
    for tj in 0..th {
        for ti in 0..tw {
            let (x0, y0) = (ti * tile, tj * tile);
            let (x1, y1) = ((x0 + tile).min(w), (y0 + tile).min(c.grid.h()));
            let has_land = (y0..y1).any(|y| (x0..x1).any(|x| z_base[y * w + x] > 0.0));
            if !has_land {
                continue;
            }
            land_tiles += 1;
            let (bw, bh) = (x1 - x0, y1 - y0);
            border_cells += if bw >= 2 && bh >= 2 { 2 * bw + 2 * bh - 4 } else { bw * bh };
        }
    }
    let bytes = border_cells * 12;
    println!(
        "AUX 2 per-tile border summaries (tile {tile}): {land_tiles} tiles with land of {}, {border_cells} border cells; 12 B a border cell (offer f32 4, crossing area f64 8) = {} B here, {:.4} GB at Earth",
        tw * th,
        bytes,
        gb(bytes as f64)
    );
    // 3. Edit overrides: the authored items (TOML, brotli) of STRM-079 (a 64-stroke brush item 36,443 B, 2,936 B brotli) and the changed records of an edit (the replacement table, STRM-079: 0.9 to 2.3 MB).
    println!("AUX 3 edit overrides: an item is 0.29 to 2.9 KB brotli (STRM-079); 10,000 items of 64 strokes: {:.3} GB brotli, {:.3} GB TOML; the changed records of an edit are the replacement table's 'added' bytes (0.9 to 2.3 MB), kept only while the edit is the latest of its tiles", 10_000.0 * 2936.0 / 1e9, 10_000.0 * 36443.0 / 1e9);
    // 4. The continuity map of an edit (basin_continuity): for each basin of the old result, its pairs (new id u64, shared cells u32): 8 B a basin + 12 B a pair; one edit (E1) is measured.
    let mut e1 = base.clone();
    raise_block(c, &mut e1.z, (8612, 8675, 780, 843), 1000.0, 8.0);
    let d1 = derive(&e1);
    let mut z1 = e1.z.clone();
    let _ = solve_rounds_memo(c.grid, &mut z1, c.outlet, c.p, 4, &c.threads, c.stream, &drivers(&e1, &d1), c.tile, cache);
    let (hy1, _, _) = h1(c, &e1, &d1, &z1);
    let map = basin_continuity(hy_base, &hy1);
    let pairs: usize = map.iter().map(|(_, v)| v.len()).sum();
    let bytes = map.len() * 8 + pairs * 12;
    println!(
        "AUX 4 continuity map of E1: {} basins before, {} after; {} old basins with {pairs} pairs = {bytes} B (8 B a basin + 12 B a pair); the basins of the window scaled to Earth: {:.0} basins, {:.4} GB for one edit's map; kept per undo step (1 or {} steps)",
        hy_base.basins.len(),
        hy1.basins.len(),
        map.len(),
        hy_base.basins.len() as f64 * scale,
        gb(bytes as f64),
        64
    );
    let _ = d_base;
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
    let p = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        round_f32: true,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let c = Ctx { grid: &grid, outlet: &outlet, p: &p, stream: spec.stream(), threads, tile, i0, j0, w };
    let run = |name: &str| only.as_ref().is_none_or(|o| name.starts_with(o.as_str()));

    // E3d: the basin of the fixture's lowest cell (2155, 205) of the 3,072 grid, named by an item: its floor cell is flagged, on the unsolved surface where the fixtures were found.
    let d_base = derive(&base);
    let (hy0, _, _) = h1(&c, &base, &d_base, &base.z);
    let (fix_i, fix_j) = (2155 * 4 + 2, 205 * 4 + 2);
    let named_id = match find_basin(&hy0, &c, fix_i, fix_j) {
        Some(k) => {
            base.flags[hy0.basins[k].floor_cell as usize] = true;
            println!("E3d the basin named by an item: {}", describe(&hy0, k, &c));
            Some(hy0.basins[k].id)
        }
        None => {
            println!("E3d: no basin within 8 cells of the fixture's lowest cell ({fix_i}, {fix_j}) at this grid; not exercised");
            None
        }
    };
    let named = |tag: &str, w: &World| {
        let Some(id) = named_id else { return };
        let (hy, _, _) = h1(&c, w, &derive(w), &w.z);
        match hy.basins.iter().find(|b| b.id == id) {
            Some(b) => println!(
                "  E3d after {tag}: the named basin is kept (id {id:016x}), class {:?}, {} cells, floor {:.0} m",
                b.class, b.cells, b.floor_m
            ),
            None => println!(
                "  E3d after {tag}: the named basin is GONE ({} intentional basin(s) now)",
                hy.basins.iter().filter(|b| b.class == BasinClass::Intentional).count()
            ),
        }
    };

    // ---- the base solve
    let mut cache = RoundCache::default();
    let d_base = derive(&base);
    let mut z_base = base.z.clone();
    let t = Instant::now();
    let (_, work_base) =
        solve_rounds_memo(&grid, &mut z_base, &outlet, &p, 4, &threads, c.stream, &drivers(&base, &d_base), tile, &mut cache);
    println!("base solve (memoised, 4 rounds): {:.1} s; {} catchment rounds held", t.elapsed().as_secs_f64(), cache.len());
    let (hy_base, r_base) = gates(&c, &base, &d_base, &z_base);
    named("the base", &base);
    if only.as_deref() == Some("AUX") {
        aux_report(&c, &base, &d_base, &z_base, &hy_base, &mut cache);
        return;
    }

    // ---- E1: raise a range
    if run("E1") {
        let mut e1 = base.clone();
        raise_block(&c, &mut e1.z, (8612, 8675, 780, 843), 1000.0, 8.0);
        run_edit(&c, "E1 raise a range by 1,000 m", &base, &z_base, &work_base, &mut cache, &e1, c.outlet);
        named("E1", &e1);
    }
    // ---- E2: move a divide
    if run("E2") {
        let mut e2 = base.clone();
        for j in 1240..=1247 {
            for i in 8120..=8127 {
                e2.z[c.at(i, j)] -= 100.0;
            }
        }
        run_edit(&c, "E2a lower the pass by 100 m", &base, &z_base, &work_base, &mut cache, &e2, c.outlet);
        named("E2a", &e2);
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
        run_edit(&c, "E2b raise a ridge line by 200 m", &base, &z_base, &work_base, &mut cache, &e2, c.outlet);
        named("E2b", &e2);
    }
    // ---- E3: basins (on the surfaces of the world, before erosion)
    if run("E3") {
        let (ai, aj, bi, bj) = (2082 * 4 + 2, 216 * 4 + 2, 2095 * 4 + 2, 224 * 4 + 2);
        match (find_basin(&hy0, &c, ai, aj), find_basin(&hy0, &c, bi, bj)) {
            (Some(ka), Some(kb)) if ka != kb => {
                println!("E3 basins at the base surface:\n  A {}\n  B {}", describe(&hy0, ka, &c), describe(&hy0, kb, &c));
                let (fa, fb) = (c.cell_of(hy0.basins[ka].floor_cell as usize), c.cell_of(hy0.basins[kb].floor_cell as usize));
                let (sa, ss) = (hy0.basins[ka].spill_cell as usize, hy0.basins[ka].spill_m);
                let id_a = hy0.basins[ka].id;
                // E3a: cut 25 m at the spill of A.
                let mut e3a = base.clone();
                let (si, sj) = c.cell_of(sa);
                for j in sj - 2..=sj + 2 {
                    for i in si - 2..=si + 2 {
                        e3a.z[c.at(i, j)] -= 25.0;
                    }
                }
                let (hy3a, _, _) = h1(&c, &e3a, &derive(&e3a), &e3a.z);
                println!("E3a cut 25 m at the spill ({si}, {sj}, spill level {ss:.0} m):");
                match find_basin(&hy3a, &c, fa.0, fa.1) {
                    Some(k) => println!(
                        "  A after: {}; id {}; class {:?} -> {:?}",
                        describe(&hy3a, k, &c),
                        if hy3a.basins[k].id == id_a { "stays" } else { "CHANGED" },
                        hy0.basins[ka].class,
                        hy3a.basins[k].class
                    ),
                    None => println!("  A is no longer a basin (the cut drained it)"),
                }
                run_edit(&c, "E3a (rounds)", &base, &z_base, &work_base, &mut cache, &e3a, c.outlet);
                named("E3a", &e3a);
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
                let (hy3b, _, _) = h1(&c, &e3b, &derive(&e3b), &e3b.z);
                println!("E3b a trench between the floors ({steps} steps):");
                match find_basin(&hy3b, &c, fa.0, fa.1) {
                    Some(k) => {
                        println!("  merged: {}", describe(&hy3b, k, &c));
                        for (old, news) in basin_continuity(&hy0, &hy3b).iter().filter(|(id, _)| *id == id_a || *id == hy0.basins[kb].id) {
                            println!(
                                "  continuity: {old:016x} -> {:?}",
                                news.iter().take(3).map(|(id, c)| format!("{id:016x} ({c} cells)")).collect::<Vec<_>>()
                            );
                        }
                        let (ba, bb) = (&hy0.basins[ka], &hy0.basins[kb]);
                        let expect = if (ba.floor_m, ba.floor_cell) <= (bb.floor_m, bb.floor_cell) { ba.id } else { bb.id };
                        println!(
                            "  the merged id is the id of the basin with the lower floor (ties by the cell id): {}",
                            hy3b.basins[k].id == expect
                        );
                    }
                    None => println!("  the trench left no basin at A's floor"),
                }
                run_edit(&c, "E3b (rounds)", &base, &z_base, &work_base, &mut cache, &e3b, c.outlet);
                named("E3b", &e3b);
                // E3c: the reverse of a merge: a ridge across basin A, above its spill level, splits it. The part with the old floor keeps the id; the other part is new.
                let mut e3c = base.clone();
                let (mut bi0, mut bi1, mut bj0, mut bj1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (cell, &l) in hy0.label.iter().enumerate() {
                    if l as usize == ka {
                        let (x, y) = c.cell_of(cell);
                        (bi0, bi1, bj0, bj1) = (bi0.min(x), bi1.max(x), bj0.min(y), bj1.max(y));
                    }
                }
                let cut = if bi1 > fa.0 + 10 { (fa.0 + bi1) / 2 } else { (fa.0 + bi0) / 2 };
                for y in bj0.saturating_sub(2)..=(bj1 + 2).min(c.j0 + h - 1) {
                    for x in cut - 1..=cut + 1 {
                        let idx = c.at(x, y);
                        e3c.z[idx] = e3c.z[idx].max(hy0.basins[ka].spill_m + 20.0);
                    }
                }
                let (hy3c, _, _) = h1(&c, &e3c, &derive(&e3c), &e3c.z);
                println!("E3c a ridge across A at i = {cut} (A spans i {bi0}-{bi1}, j {bj0}-{bj1}):");
                match find_basin(&hy3c, &c, fa.0, fa.1) {
                    Some(k) => println!(
                        "  the part with A's floor: {}; old id {}",
                        describe(&hy3c, k, &c),
                        if hy3c.basins[k].id == id_a { "kept" } else { "NEW" }
                    ),
                    None => println!("  no basin at A's floor"),
                }
                for (old, news) in basin_continuity(&hy0, &hy3c).iter().filter(|(id, _)| *id == id_a) {
                    println!(
                        "  continuity of A {old:016x} -> {:?}",
                        news.iter().take(4).map(|(id, c)| format!("{id:016x} ({c} cells)")).collect::<Vec<_>>()
                    );
                }
                run_edit(&c, "E3c (rounds)", &base, &z_base, &work_base, &mut cache, &e3c, c.outlet);
                named("E3c", &e3c);
            }
            (a, b) => println!("E3: the fixture basins are not both present at this grid (A {a:?}, B {b:?}); skipped"),
        }
    }
    // ---- E4: far downstream
    if run("E4") {
        let trunk = c.at(2097 * 4 + 2, 239 * 4 + 2);
        let mouth = outlet_of(&r_base, trunk);
        let mut up = trunk;
        for _ in 0..120 {
            let mut best: Option<usize> = None;
            for d in 0..8 {
                if let Some(q) = grid.neighbour(up, d) {
                    if r_base.receiver[q] == up as u32 && best.is_none_or(|b| r_base.area[q] > r_base.area[b]) {
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
            "E4 trunk ({}, {}) A_d {:.0} km2; the block is 120 cells upstream at ({ui}, {uj}); the outlet its water reaches ({}, {}) holds A_d {:.0} km2, Q {:.0} m3/s",
            c.cell_of(trunk).0,
            c.cell_of(trunk).1,
            r_base.area[trunk] / 1e6,
            c.cell_of(mouth).0,
            c.cell_of(mouth).1,
            r_base.area[mouth] / 1e6,
            hy_base.q[mouth]
        );
        let mut e4 = base.clone();
        raise_block(&c, &mut e4.z, (ui - 30, ui + 30, uj - 30, uj + 30), 500.0, 8.0);
        let (_, hy4, r4) = run_edit(&c, "E4a raise a 50 km block upstream by 500 m", &base, &z_base, &work_base, &mut cache, &e4, c.outlet);
        let m4 = outlet_of(&r4, trunk);
        println!(
            "  Q at the trunk cell {:.0} -> {:.0} m3/s; the outlet of the trunk cell ({}, {}) -> ({}, {}); Q there {:.0} -> {:.0} m3/s, A_d {:.0} -> {:.0} km2",
            hy_base.q[trunk],
            hy4.q[trunk],
            c.cell_of(mouth).0,
            c.cell_of(mouth).1,
            c.cell_of(m4).0,
            c.cell_of(m4).1,
            hy_base.q[mouth],
            hy4.q[m4],
            r_base.area[mouth] / 1e6,
            r4.area[m4] / 1e6
        );
        named("E4a", &e4);
        let mut e4b = base.clone();
        for j in uj - 30..=uj + 30 {
            for i in ui - 30..=ui + 30 {
                let k = c.at(i, j);
                e4b.pm[k] *= 1.5;
            }
        }
        let (_, hy4b, r4b) =
            run_edit(&c, "E4b raise the rainfall of the block by 50 %", &base, &z_base, &work_base, &mut cache, &e4b, c.outlet);
        let m4b = outlet_of(&r4b, trunk);
        println!(
            "  Q at the trunk cell {:.0} -> {:.0} m3/s; at the outlet ({}, {}) {:.0} -> {:.0} m3/s",
            hy_base.q[trunk],
            hy4b.q[trunk],
            c.cell_of(m4b).0,
            c.cell_of(m4b).1,
            hy_base.q[mouth],
            hy4b.q[m4b]
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
        let (_, hy, _) =
            run_edit(&c, "E5 double the rainfall of a window (no height change)", &base, &z_base, &work_base, &mut cache, &e5, c.outlet);
        named("E5", &e5);
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
    // ---- E7: a land bridge between two landmasses, then taken away again
    if run("E7") {
        let mut comp = vec![u32::MAX; n];
        let mut sizes: Vec<(usize, usize)> = Vec::new();
        for s0 in 0..n {
            if comp[s0] != u32::MAX || base.z[s0] <= 0.0 {
                continue;
            }
            let id = sizes.len() as u32;
            let mut stack = vec![s0];
            comp[s0] = id;
            let mut size = 0;
            while let Some(p) = stack.pop() {
                size += 1;
                for d in 0..8 {
                    if let Some(q) = grid.neighbour(p, d) {
                        if comp[q] == u32::MAX && base.z[q] > 0.0 {
                            comp[q] = id;
                            stack.push(q);
                        }
                    }
                }
            }
            sizes.push((size, s0));
        }
        let mut by_size: Vec<usize> = (0..sizes.len()).collect();
        by_size.sort_by_key(|&k| std::cmp::Reverse(sizes[k].0));
        let big = by_size[0] as u32;
        println!("E7 land components of the window (cells): {:?}", by_size.iter().take(6).map(|&k| sizes[k].0).collect::<Vec<_>>());
        match by_size.iter().skip(1).find(|&&k| sizes[k].0 >= 30) {
            None => println!("E7: no second landmass of 30 cells in this window; skipped"),
            Some(&k) => {
                let target = k as u32;
                // The shortest crossing from the large landmass to the other one, through the sea.
                let mut parent = vec![usize::MAX; n];
                let mut frontier: Vec<usize> = (0..n).filter(|&p| comp[p] == big).collect();
                let mut seen = vec![false; n];
                frontier.iter().for_each(|&p| seen[p] = true);
                let mut hit = None;
                'bfs: while !frontier.is_empty() {
                    let mut next = Vec::new();
                    for &p in &frontier {
                        for d in 0..8 {
                            if let Some(q) = grid.neighbour(p, d) {
                                if !seen[q] {
                                    seen[q] = true;
                                    parent[q] = p;
                                    if comp[q] == target {
                                        hit = Some(q);
                                        break 'bfs;
                                    }
                                    next.push(q);
                                }
                            }
                        }
                    }
                    frontier = next;
                }
                let mut e7 = base.clone();
                let mut bridge = 0;
                let mut p = hit.expect("the second landmass is reachable");
                while comp[p] != big {
                    if base.z[p] <= 0.0 {
                        bridge += 1;
                        for d in 0..4 {
                            if let Some(q) = grid.neighbour(p, d) {
                                if e7.z[q] <= 0.0 {
                                    e7.z[q] = 25.0;
                                }
                            }
                        }
                        e7.z[p] = 25.0;
                    }
                    p = parent[p];
                }
                let (i, j) = c.cell_of(sizes[k].1);
                println!(
                    "E7 the second landmass ({} cells, near ({i}, {j})) and the large one ({} cells): a bridge of {bridge} sea cells, 3 wide, raised to 25 m",
                    sizes[k].0, sizes[big as usize].0
                );
                let outlet2 = grid.outlets(&e7.z, 0.0);
                let (_, hy7, _) = run_edit(&c, "E7a join the landmasses", &base, &z_base, &work_base, &mut cache, &e7, &outlet2);
                println!(
                    "  channel cells {} -> {}, basins {} -> {}",
                    hy_base.stats.channel_cells, hy7.stats.channel_cells, hy_base.stats.basins, hy7.stats.basins
                );
                let _ = run_edit(&c, "E7b take the bridge away again", &base, &z_base, &work_base, &mut cache, &base, c.outlet);
            }
        }
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
        run_edit(&c, "E6a a softer lithology (hardness 0.29 -> 0.1) over a block", &base, &z_base, &work_base, &mut cache, &e6, c.outlet);
        let mut e6 = base.clone();
        for j in 840..=967 {
            for i in 8544..=8671 {
                let k = c.at(i, j);
                e6.up[k] *= 2.0;
            }
        }
        run_edit(&c, "E6b uplift x 2 over a block", &base, &z_base, &work_base, &mut cache, &e6, c.outlet);
    }
}
