//! GEN-147: the rounds on the whole prototype landmass at the working resolution (A3 section 6.6, G-SCALE; inbox #101): the landmass is found at 3,072 cells from the base cache of `landmass_region`,
//! refined by `k` (k = 4 is the erosion grid, 12,288 cells, 814 m): the base is sampled only in the fine cells whose coarse parent is in the coarse landmass or within a ring of it, the fine land is the cells
//! above the sea connected to the region's centre, its coast ring the outlets, and the landmass is a compact grid (`Grid::compact_cube`: only its cells, indexed by rank). Then 4 rounds of 5 steps on
//! the resident route, with the process's peak working set after each phase (PowerShell, Windows).
//! Usage: landmass_scale [k] [threads] [h1|part|both|sed|sedround|thdiag|dump]: `part` routes each round with the partitioned route on the compact grid (GEN-149, tiles of 256 cells; `both` also runs the resident rounds and compares the bits, small k only); with `h1` the hydrology pass follows (runoff of the orographic climate, weighted route, filled surface, H1) with the peak after it. Needs `target/landmass_base_3072.f32` (made by `landmass_region`).
#![allow(clippy::needless_range_loop, clippy::chunks_exact_to_as_chunks)]
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{hydrology, HydroParams};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::round::{solve_rounds, solve_rounds_memo, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use std::time::Instant;

/// Pit report (A3 6.7, #107 and #108): the cells deeper than `pit_min_m` on the flooded surface, their depth percentiles, the separate depressions, the cells inside the basins `hy` kept by class (a cell in none is an
/// artefact of A3 5.3), and how many were not pits in `pre`. Returns the pit mask.
fn pit_report(
    tag: &str,
    grid: &Grid,
    z: &[f64],
    q: &planet_generators::bake::tile::Pass2Params,
    hy: Option<&planet_generators::bake::hydro::Hydro>,
    pre: Option<&[bool]>,
) -> Vec<bool> {
    use planet_generators::bake::hydro::BasinClass;
    use planet_generators::bake::sediment::outlets;
    let outl = outlets(grid, z, q.sea_level);
    let filled = priority_flood_with(grid, z, &outl, q.carve.epsilon, true);
    let pit: Vec<bool> = (0..z.len()).map(|p| z[p] > q.sea_level && !outl[p] && filled[p] - z[p] >= q.pit_min_m).collect();
    let land = (0..z.len()).filter(|&p| z[p] > q.sea_level && !outl[p]).count();
    let mut depth: Vec<f64> = (0..z.len()).filter(|&p| pit[p]).map(|p| filled[p] - z[p]).collect();
    drop(filled);
    drop(outl);
    depth.sort_by(|a, b| a.total_cmp(b));
    if depth.is_empty() {
        println!("PITS {tag}: none");
        return pit;
    }
    let at = |f: f64| depth[((depth.len() - 1) as f64 * f) as usize];
    // classes: 0 none (artefact), 1 lake, 2 endorheic, 3 intentional
    let class: Vec<u8> = hy.map_or(Vec::new(), |h| {
        h.basins
            .iter()
            .map(|b| match b.class {
                BasinClass::Lake => 1,
                BasinClass::Endorheic => 2,
                BasinClass::Intentional => 3,
            })
            .collect()
    });
    let (mut seen, mut comps, mut by, mut fresh) = (vec![false; z.len()], 0usize, [0usize; 4], 0usize);
    for p0 in 0..z.len() {
        if !pit[p0] || seen[p0] {
            continue;
        }
        comps += 1;
        seen[p0] = true;
        let mut stack = vec![p0];
        while let Some(p) = stack.pop() {
            if let Some(h) = hy {
                let l = h.label[p];
                by[if l == u32::MAX { 0 } else { class[l as usize] as usize }] += 1;
            }
            if pre.is_some_and(|m| !m[p]) {
                fresh += 1;
            }
            for d in 0..8 {
                if let Some(nb) = grid.neighbour(p, d) {
                    if pit[nb] && !seen[nb] {
                        seen[nb] = true;
                        stack.push(nb);
                    }
                }
            }
        }
    }
    let pc = |v: usize| 100.0 * v as f64 / depth.len() as f64;
    println!(
        "PITS {tag}: threshold {} m; {} cells ({:.4} % of {land} land); p50 {:.2} m, p99 {:.2} m, max {:.1} m; {comps} separate depressions",
        q.pit_min_m,
        depth.len(),
        100.0 * depth.len() as f64 / land as f64,
        at(0.5),
        at(0.99),
        depth[depth.len() - 1]
    );
    if hy.is_some() {
        println!(
            "  by class of the basin H1 kept: lake {:.2} %, endorheic {:.2} %, intentional {:.2} %, in none (artefact) {:.2} % of the pit cells",
            pc(by[1]),
            pc(by[2]),
            pc(by[3]),
            pc(by[0])
        );
    }
    if pre.is_some() {
        println!(
            "  not a pit before the sediment pass (created by it): {} cells, {:.2} % of the pit cells; inherited {:.2} %",
            fresh,
            pc(fresh),
            100.0 - pc(fresh)
        );
    }
    pit
}

/// FNV-1a over a word stream: the result hashes the retention changes must not move.
fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    words.fold(0xcbf2_9ce4_8422_2325, |h, w| (h ^ w).wrapping_mul(0x0100_0000_01b3))
}

const R: f64 = 6_371_000.0;
const COARSE: usize = 3072;
const SEED_CELL: (usize, usize, usize) = (1, 2090, 267);
const REGION: (usize, usize, usize, usize) = (2014, 2168, 190, 344);

fn memory_mb() -> (f64, f64) {
    let (peak, now, _) = memory3();
    (peak, now)
}

/// The peak and current working set and the peak commit (paged memory) of this process in MiB.
fn memory3() -> (f64, f64, f64) {
    let ps = |field: &str| -> f64 {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).{field}", std::process::id())])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(f64::NAN) / 1048576.0
    };
    (ps("PeakWorkingSet64"), ps("WorkingSet64"), ps("PeakPagedMemorySize64"))
}

/// CPU seconds of this process against wall seconds since the last call (#110: cores used per phase). Report only.
fn cpu_phase(tag: &str) {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    use std::sync::OnceLock;
    static LAST: AtomicU64 = AtomicU64::new(0);
    static START: OnceLock<Instant> = OnceLock::new();
    static WALL: AtomicU64 = AtomicU64::new(0);
    let start = START.get_or_init(Instant::now);
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).TotalProcessorTime.TotalSeconds", std::process::id())])
        .output()
        .expect("powershell");
    let cpu: f64 = String::from_utf8_lossy(&out.stdout).trim().replace(',', ".").parse().unwrap_or(f64::NAN);
    let wall = start.elapsed().as_secs_f64();
    let (c0, w0) = (f64::from_bits(LAST.swap(cpu.to_bits(), Relaxed)), f64::from_bits(WALL.swap(wall.to_bits(), Relaxed)));
    let (dc, dw) = (cpu - c0, wall - w0);
    println!("CPU {tag}: {dc:.0} CPU s in {dw:.0} wall s ({:.2} cores busy)", dc / dw.max(1e-9));
}

fn set(bits: &mut [u64], g: usize) {
    bits[g >> 6] |= 1 << (g & 63);
}

fn get(bits: &[u64], g: usize) -> bool {
    bits[g >> 6] >> (g & 63) & 1 == 1
}

/// The probe of the plan mode: the sheet probe (when on) and, for the dam count, the surface "z" of three stages written to disk as it passes (the last round before thermal, after thermal, after the deposition), so
/// that the build holds no extra copy while it runs and the peaks it prints are the build's. The analysis reads the files back after the final peak line.
struct DamProbe {
    inner: Option<planet_generators::bake::probe::SheetProbe>,
    prefix: String,
    last_round: u32,
}

impl DamProbe {
    fn path(&self, tag: &str) -> String {
        format!("{}_{tag}.f64", self.prefix)
    }
}

impl planet_generators::bake::probe::StageProbe for DamProbe {
    fn stage(&self, view: &planet_generators::bake::probe::StageView<'_>) {
        use planet_generators::bake::probe::{FieldData, Stage};
        if let Some(inner) = &self.inner {
            inner.stage(view);
        }
        let tag = match view.stage {
            Stage::Round(r) if r == self.last_round => "before",
            Stage::Thermal => "thermal",
            Stage::Deposition => "deposition",
            _ => return,
        };
        let Some(z) = view.fields.iter().find(|f| f.name == "z").and_then(|f| if let FieldData::F64(d) = f.data { Some(d) } else { None })
        else {
            return;
        };
        write_f64s(&self.path(tag), z);
    }
}

fn write_f64s(path: &str, v: &[f64]) {
    use std::io::Write;
    // One reused buffer of 1 MiB: no allocation per chunk, so the spill adds 1 MiB to the build's memory and returns it.
    let mut f = std::fs::File::create(path).expect("the dam spill");
    let mut bytes = vec![0u8; 8 << 17];
    for c in v.chunks(1 << 17) {
        for (o, x) in bytes.chunks_exact_mut(8).zip(c) {
            o.copy_from_slice(&x.to_le_bytes());
        }
        f.write_all(&bytes[..c.len() * 8]).expect("the dam spill");
    }
    f.flush().expect("the dam spill");
}

fn read_f64s(path: &str, n: usize) -> Vec<f64> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).expect("the dam spill");
    let mut out = Vec::with_capacity(n);
    let mut buf = vec![0u8; 1 << 20];
    while out.len() < n {
        let want = ((n - out.len()) * 8).min(buf.len());
        f.read_exact(&mut buf[..want]).expect("the dam spill");
        out.extend(buf[..want].chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().expect("8 bytes"))));
    }
    out
}

/// The land pits of a surface: the canonical flood, 0.5 m under the fill (as G-LAND), and the depth of each cell under its fill.
fn pit_depths(grid: &Grid, z: &[f64], outlet: &[bool]) -> Vec<f32> {
    let filled = priority_flood_with(grid, z, outlet, 1e-3, true);
    (0..z.len()).map(|c| if z[c] > 0.0 { (filled[c] - z[c]) as f32 } else { 0.0 }).collect()
}

/// The dams of the final surface (inbox #124, decision D15): item 10's question is whether the remaining pits are made by the pass. The land pits after the pass (canonical flood, 0.5 m, as G-LAND) are 8-connected
/// components; a component is a DAM when the pass raised its spill cell (the lowest cell of its rim: `z_after - z_before > 0` there) and the pit is new or at least 0.5 m deeper than before the pass
/// (`z_before` is the surface after the last round, before thermal), and inherited otherwise. The cause of a dam is the part of the raise at the spill cell by thermal, by the deposition, by the pit spreading, or by
/// several of them. The one-cell wall (the outside cell beyond the spill along the line from the pit more than 5 m lower, both cells across the line higher) is an extra column, not the definition.
fn dam_report(grid: &Grid, zb: &[f64], zt: &[f64], zd: &[f64], z: &[f64], outlet: &[bool]) -> DamResult {
    use planet_generators::bake::NEIGHBOURS;
    let before = pit_depths(grid, zb, outlet);
    let after = pit_depths(grid, z, outlet);
    let land = z.iter().filter(|&&v| v > 0.0).count();
    let is_pit = |c: usize| after[c] > 0.5;
    let mut label = vec![u32::MAX; z.len()];
    let mut res = DamResult { land, ..DamResult::default() };
    let mut stack: Vec<usize> = Vec::new();
    let mut dam_depths: Vec<f64> = Vec::new();
    for s in 0..z.len() {
        if label[s] != u32::MAX || !is_pit(s) {
            continue;
        }
        let id = res.components as u32;
        res.components += 1;
        label[s] = id;
        stack.push(s);
        let (mut cells, mut depth_after, mut depth_before) = (Vec::new(), 0.0f64, 0.0f64);
        while let Some(c) = stack.pop() {
            cells.push(c);
            depth_after = depth_after.max(f64::from(after[c]));
            depth_before = depth_before.max(f64::from(before[c]));
            for d in 0..8 {
                if let Some(q) = grid.neighbour(c, d) {
                    if label[q] == u32::MAX && is_pit(q) {
                        label[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
        // The spill cell: the lowest cell of the rim (a neighbour outside the component); the pit cell it was found from gives the line for the wall test.
        let mut spill: Option<(usize, usize, usize)> = None;
        for &p in &cells {
            for d in 0..8 {
                let Some(q) = grid.neighbour(p, d) else { continue };
                if label[q] != id && spill.is_none_or(|(sq, _, _)| z[q] < z[sq]) {
                    spill = Some((q, p, d));
                }
            }
        }
        res.pit_cells += cells.len();
        let Some((q, _p, d)) = spill else {
            res.inherited_cells += cells.len();
            res.inherited += 1;
            continue;
        };
        let (rt, rd, rs) = (zt[q] - zb[q], zd[q] - zt[q], z[q] - zd[q]);
        let raise = z[q] - zb[q];
        let dam = raise > 1e-6 && depth_after - depth_before >= 0.5;
        if !dam {
            res.inherited_cells += cells.len();
            res.inherited += 1;
            continue;
        }
        res.dams += 1;
        res.dam_cells += cells.len();
        let cause = {
            let mut parts = Vec::new();
            for (name, v) in [("thermal", rt), ("deposition", rd), ("spreading", rs)] {
                if v > 1e-6 {
                    parts.push(name);
                }
            }
            if parts.is_empty() {
                "none".to_string()
            } else {
                parts.join("+")
            }
        };
        *res.causes.entry(cause.clone()).or_insert(0) += 1;
        dam_depths.push(depth_after);
        if depth_after >= 20.0 {
            res.dams_over_20 += 1;
        }
        // The wall column: the cell beyond the spill along the line from the pit (found as the direction from the pit cell to the spill) more than 5 m lower, both cells across it higher.
        let (di, dj) = NEIGHBOURS[d];
        let at = |a: i64, b: i64| NEIGHBOURS.iter().position(|&o| o == (a, b)).and_then(|k| grid.neighbour(q, k));
        if let (Some(r), Some(n1), Some(n2)) = (at(di, dj), at(-dj, di), at(dj, -di)) {
            if z[r] < z[q] - 5.0 && z[n1] > z[q] && z[n2] > z[q] {
                res.walls += 1;
            }
        }
        res.top.push((depth_after, cells[0], raise, cause));
    }
    dam_depths.sort_by(f64::total_cmp);
    let pct = |p: usize| dam_depths.get(dam_depths.len().saturating_sub(1) * p / 100).copied().unwrap_or(0.0);
    (res.p50, res.p95, res.max) = (pct(50), pct(95), dam_depths.last().copied().unwrap_or(0.0));
    res.top.sort_by(|a, b| b.0.total_cmp(&a.0));
    res.top.truncate(10);
    res
}

#[derive(Default)]
struct DamResult {
    land: usize,
    components: usize,
    pit_cells: usize,
    dams: usize,
    dam_cells: usize,
    inherited: usize,
    inherited_cells: usize,
    dams_over_20: usize,
    walls: usize,
    causes: std::collections::BTreeMap<String, usize>,
    p50: f64,
    p95: f64,
    max: f64,
    top: Vec<(f64, usize, f64, String)>,
}

fn print_dams(r: &DamResult, seconds: f64) {
    let pct = |n: usize| 100.0 * n as f64 / r.land.max(1) as f64;
    println!(
        "DAMS (D15, {seconds:.0} s): land pits after the pass {} cells ({:.4} % of {} land cells) in {} components: dams {} components, {} cells ({:.4} %); inherited {} components, {} cells ({:.4} %)",
        r.pit_cells,
        pct(r.pit_cells),
        r.land,
        r.components,
        r.dams,
        r.dam_cells,
        pct(r.dam_cells),
        r.inherited,
        r.inherited_cells,
        pct(r.inherited_cells)
    );
    println!("  dams by cause (the raise at the spill cell): {:?}", r.causes);
    println!("  dam depth under the fill: p50 {:.1} m, p95 {:.1} m, max {:.1} m; {} dams deeper than 20 m; {} of the dams are also a one-cell wall across a cut (extra column)", r.p50, r.p95, r.max, r.dams_over_20, r.walls);
    for (depth, cell, raise, cause) in &r.top {
        println!("  dam: depth {depth:.1} m, a pit cell {cell}, raise at the spill cell {raise:.2} m ({cause})");
    }
}

/// The dam report on a fixture: a plateau at 200 m with a sea on its west edge, a one-cell canyon (walls 200 m) whose floor rises inland, raised to 90 m at one cell by thermal in the "after" surface (a dam: the pit
/// behind it is new and 59 m deep), and a closed 5 x 5 basin 50 m deep in the plateau present before and after (inherited). `landmass_scale 1 8 damtest` expects 2 pits, 1 dam (cause thermal, also a wall), 1 inherited.
fn dam_selftest() {
    let (w, h) = (48usize, 48usize);
    let mut zb = vec![200.0f64; w * h];
    for j in 0..h {
        for i in 0..2 {
            zb[j * w + i] = -10.0;
        }
    }
    for i in 2..31 {
        zb[20 * w + i] = 20.0 + i as f64;
    }
    for j in 32..37 {
        for i in 24..29 {
            zb[j * w + i] = 150.0;
        }
    }
    let mut zt = zb.clone();
    zt[20 * w + 10] = 90.0;
    let (zd, z) = (zt.clone(), zt.clone());
    let grid = Grid::new(w, h, 100.0);
    let outlet = grid.outlets(&z, 0.0);
    let r = dam_report(&grid, &zb, &zt, &zd, &z, &outlet);
    print_dams(&r, 0.0);
    assert_eq!((r.components, r.dams, r.inherited, r.walls), (2, 1, 1, 1), "the dam report on its fixture");
    assert_eq!(r.causes.get("thermal"), Some(&1));
    assert_eq!(r.dams_over_20, 1);
    // The spill files round trip (a size that is not a multiple of the chunk).
    let v: Vec<f64> = (0..300_001).map(|i| f64::from(i) * 0.125 - 7.0).collect();
    let path = format!("target/plan_dam_selftest_{}.f64", std::process::id());
    write_f64s(&path, &v);
    assert_eq!(read_f64s(&path, v.len()), v);
    let _ = std::fs::remove_file(&path);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.get(3).is_some_and(|m| m == "damtest") {
        dam_selftest();
        return;
    }
    let k: usize = a.get(1).map_or(1, |s| s.parse().unwrap());
    let threads: usize = a.get(2).map_or(8, |s| s.parse().unwrap());
    let n = COARSE * k;
    let nn = n * n;
    let t0 = Instant::now();
    cpu_phase("start");
    let seed = 14u64;

    // ---- the coarse landmass and its ring, from the 3,072-cell base cache
    let bytes = std::fs::read(format!("target/landmass_base_{COARSE}.f32")).expect("run landmass_region first (it makes the base cache)");
    let cz = |g: usize| f32::from_le_bytes([bytes[4 * g], bytes[4 * g + 1], bytes[4 * g + 2], bytes[4 * g + 3]]);
    let coarse = Grid::cube(COARSE, R);
    let cid = |f: usize, i: usize, j: usize| f * COARSE * COARSE + j * COARSE + i;
    let mut land_c = vec![0u64; (6 * COARSE * COARSE).div_ceil(64)];
    let mut stack = vec![cid(SEED_CELL.0, SEED_CELL.1, SEED_CELL.2)];
    set(&mut land_c, stack[0]);
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = coarse.neighbour(p, d) {
                if !get(&land_c, q) && cz(q) > 0.0 {
                    set(&mut land_c, q);
                    stack.push(q);
                }
            }
        }
    }
    // The candidates: the coarse landmass and two rings of cells around it (the fine coast may differ from the coarse one).
    let mut cand_c = land_c.clone();
    for _ in 0..2 {
        let prev = cand_c.clone();
        for g in 0..6 * COARSE * COARSE {
            if get(&prev, g) {
                for d in 0..8 {
                    if let Some(q) = coarse.neighbour(g, d) {
                        set(&mut cand_c, q);
                    }
                }
            }
        }
    }
    drop((coarse, stack, land_c, cz));
    drop(bytes);
    let coarse_cand: usize = cand_c.iter().map(|w| w.count_ones() as usize).sum();
    println!("coarse candidates {coarse_cand} ({:.1} s)", t0.elapsed().as_secs_f64());

    // ---- the fine candidates, in order of their cube id, and the fine base heights there
    let mut bits = vec![0u64; (6 * nn).div_ceil(64)];
    let mut rows: Vec<(usize, usize)> = Vec::new(); // (face, j) of every row with a candidate
    for f in 0..6 {
        for j in 0..n {
            let mut any = false;
            for i in 0..n {
                if get(&cand_c, cid(f, i / k, j / k)) {
                    set(&mut bits, f * nn + j * n + i);
                    any = true;
                }
            }
            if any {
                rows.push((f, j));
            }
        }
    }
    let count: usize = bits.iter().map(|w| w.count_ones() as usize).sum();
    println!("fine candidates {count} of {} ({:.1} s)", 6 * nn, t0.elapsed().as_secs_f64());
    let cand = Grid::compact_cube(n, R, bits);
    let inputs = BakeInputs::new(seed, threads);
    let mut zf = vec![0f32; count];
    {
        // Rows in ascending id order own consecutive candidate ranges: split the heights by row.
        let mut starts: Vec<usize> = Vec::with_capacity(rows.len() + 1);
        let mut at = 0usize;
        for &(f, j) in &rows {
            starts.push(at);
            at += (0..n).filter(|&i| cand.compact_index_of(f * nn + j * n + i).is_some()).count();
        }
        starts.push(at);
        assert_eq!(at, count);
        let per = rows.len().div_ceil(threads);
        let mut rest: &mut [f32] = &mut zf;
        let mut parts: Vec<(usize, &mut [f32])> = Vec::new();
        for (t, chunk) in rows.chunks(per).enumerate() {
            let len = starts[(t * per + chunk.len()).min(rows.len())] - starts[t * per];
            let (head, tail) = rest.split_at_mut(len);
            parts.push((t, head));
            rest = tail;
        }
        std::thread::scope(|s| {
            for (t, part) in parts {
                let (rows, inputs, cand) = (&rows, &inputs, &cand);
                s.spawn(move || {
                    let mut at = 0;
                    for &(f, j) in &rows[t * per..(t * per + per).min(rows.len())] {
                        let spec = FaceSpec { face: Face(f as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                        for i in 0..n {
                            if cand.compact_index_of(f * nn + j * n + i).is_some() {
                                let d: Vec3 = spec.halo_direction(i as i64, j as i64);
                                part[at] = inputs.base(d) as f32;
                                at += 1;
                            }
                        }
                    }
                });
            }
        });
    }
    println!("fine base sampled ({:.1} s)", t0.elapsed().as_secs_f64());
    let (peak, now) = memory_mb();
    println!("memory after the sampling: peak {peak:.0} MiB, now {now:.0} MiB");

    // ---- the fine landmass: the land connected to the region's centre, and its coast ring
    let seed_g = SEED_CELL.0 * nn + (SEED_CELL.2 * k + k / 2) * n + SEED_CELL.1 * k + k / 2;
    let s0 = cand.compact_index_of(seed_g).expect("the seed cell is a candidate");
    assert!(zf[s0] > 0.0, "the region's centre is land");
    let mut land = vec![0u64; count.div_ceil(64)];
    let mut stack = vec![s0];
    set(&mut land, s0);
    while let Some(p) = stack.pop() {
        for d in 0..8 {
            if let Some(q) = cand.neighbour(p, d) {
                if !get(&land, q) && zf[q] > 0.0 {
                    set(&mut land, q);
                    stack.push(q);
                }
            }
        }
    }
    drop(stack);
    // The fine land must not reach the border of the candidates (the coarse landmass and two rings): a land cell with a cube neighbour outside them would be cut off from its outlets.
    {
        let full = Grid::cube(n, R);
        let mut border = 0usize;
        for p in 0..count {
            if get(&land, p) {
                let g = cand.cube_id_of(p);
                if (0..8).any(|d| full.neighbour(g, d).is_some_and(|q| cand.compact_index_of(q).is_none())) {
                    border += 1;
                }
            }
        }
        println!("fine land cells at the border of the candidates: {border}");
        assert_eq!(border, 0, "the fine landmass reaches the border of the candidates: more rings are needed");
    }
    let mut fin = vec![0u64; (6 * nn).div_ceil(64)];
    let mut land_cells = 0usize;
    for p in 0..count {
        if get(&land, p) {
            land_cells += 1;
            set(&mut fin, cand.cube_id_of(p));
            for d in 0..8 {
                if let Some(q) = cand.neighbour(p, d) {
                    set(&mut fin, cand.cube_id_of(q));
                }
            }
        }
    }
    let active: usize = fin.iter().map(|w| w.count_ones() as usize).sum();
    println!("landmass: {land_cells} land cells, {active} cells with the coast ring ({:.1} s)", t0.elapsed().as_secs_f64());
    // The heights and outlets of the landmass, by compact index.
    let grid = Grid::compact_cube(n, R, fin);
    let mut z: Vec<f64> = Vec::with_capacity(active);
    let mut outlet: Vec<bool> = Vec::with_capacity(active);
    for p in 0..count {
        let g = cand.cube_id_of(p);
        if grid.compact_index_of(g).is_some() {
            z.push(f64::from(zf[p]));
            outlet.push(zf[p] <= 0.0);
        }
    }
    assert_eq!(z.len(), active);
    // The region's cells, kept for the change.
    let region: Vec<(usize, f64)> = (REGION.2 * k..(REGION.3 + 1) * k)
        .flat_map(|j| (REGION.0 * k..(REGION.1 + 1) * k).map(move |i| (nn + j * n + i, 0.0)))
        .filter_map(|(g, _)| grid.compact_index_of(g).filter(|&p| z[p] > 0.0).map(|p| (p, z[p])))
        .collect();
    drop((cand, zf, land));
    cpu_phase("the landmass (sampling, ring, outlets)");
    let (peak, now) = memory_mb();
    println!(
        "memory after the landmass is built (heights f64, outlets): peak {peak:.0} MiB, now {now:.0} MiB; {:.1} s",
        t0.elapsed().as_secs_f64()
    );

    // ---- the rounds, resident
    let mut z_start =
        if a.get(3).is_some_and(|x| x == "sed" || x == "sedround" || x == "dump" || x == "full") { z.clone() } else { Vec::new() };
    let p = CarveParams { steps: 20, reroute_every: 5, round_rules: RoundRules::ROUND, round_f32: true, ..CarveParams::default() };
    let t = Instant::now();
    let mode = a.get(3).map_or("", |x| x.as_str());
    if mode == "plan" {
        // GEN-158: the production phase plan (`bake::build`): the same steps as `full`, with the pre-carve surface spilled, H1's discharges released and the stage probe called.
        use planet_generators::bake::build::{build_resident, PlanParams};
        let o = inputs.orography();
        let climate = |p: usize| -> (f64, f64) {
            let g = grid.cube_id_of(p);
            let spec = FaceSpec { face: Face((g / nn) as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
            let d: Vec3 = spec.halo_direction(((g % nn) % n) as i64, ((g % nn) / n) as i64);
            (o.precipitation_mm(d), o.temperature_c(d))
        };
        let mark = |name: &str| {
            let (peak, now, commit) = memory3();
            println!("MARK {name}: working set {now:.0} MiB now, peak {peak:.0} MiB, peak commit {commit:.0} MiB");
        };
        // The stage probe (as `region_scale`): sheets of at most `PLANET_STAGE_BINS` bins (default 4,194,304) in target/review/stages/plan-k<k>/; `PLANET_STAGES=0` switches it off. Its memory is inside the peaks below.
        // The probes: the sheet probe (as `region_scale`: sheets of at most `PLANET_STAGE_BINS` bins, default 4,194,304, in target/review/stages/plan-k<k>/; `PLANET_STAGES=0` switches it off; its memory is inside the printed
        // peaks) and the dam probe (`PLANET_DAMS=0` switches it off), which writes three surfaces to disk as they pass and holds nothing.
        let stages_on = std::env::var("PLANET_STAGES").map_or(true, |v| v != "0");
        let dams_on = std::env::var("PLANET_DAMS").map_or(true, |v| v != "0");
        let dam_prefix = format!("target/plan_dam_k{k}_{}", std::process::id());
        if stages_on || dams_on {
            let bins: usize = std::env::var("PLANET_STAGE_BINS").ok().and_then(|v| v.parse().ok()).unwrap_or(4_194_304);
            let dir = format!("target/review/stages/plan-k{k}");
            let inner = stages_on.then(|| planet_generators::bake::probe::SheetProbe::new(&dir, bins));
            planet_generators::bake::probe::install(std::sync::Arc::new(DamProbe { inner, prefix: dam_prefix.clone(), last_round: 3 }));
            println!("stage probe: sheets {} (at most {bins} bins, numbers in {dir}/; memory inside the peaks), dam surfaces {} (on disk, deleted after the count)", if stages_on { "ON" } else { "off" }, if dams_on { "ON" } else { "off" });
        }
        let params = PlanParams::standard(seed, threads, format!("target/plan_zstart_k{k}_{}.f64", std::process::id()).into());
        let t = Instant::now();
        let built = build_resident(&grid, z, &outlet, &params, &climate, &mark).expect("the build");
        let r = &built.report;
        println!("plan: {:.1} s; phases {:?}", t.elapsed().as_secs_f64(), r.phases);
        println!(
            "hash H1: label {:016x}, channel {:016x}, segments {:016x}",
            fnv(built.hydro.label.iter().map(|&v| u64::from(v))),
            fnv(built.hydro.channel.iter().map(|&c| u64::from(c))),
            fnv(built.hydro.segments.iter().flat_map(|g| [g.id, g.q_up.to_bits(), g.q_down.to_bits(), u64::from(g.cells)]))
        );
        println!(
            "hash lakes: {} lakes, {} pieces, {} dropped cells, {:016x}",
            built.lakes.lakes.len(),
            built.lakes.pieces.len(),
            built.lakes.dropped_cells,
            fnv(format!("{:?}", built.lakes).bytes().map(u64::from))
        );
        println!(
            "lake lists: water {} entries, ring {} entries ({} KiB at 8 B an entry, D37), basin cells {}",
            built.lakes.water.len(),
            built.lakes.ring.len(),
            (built.lakes.water.len() + built.lakes.ring.len()) * 8 / 1024,
            built.lakes.lakes.iter().map(|l| u64::from(l.cells)).sum::<u64>()
        );
        println!(
            "channel graph: {} segments, {} vertices, {} bytes ({:.1} MiB; {:.1} B a vertex), largest relative Q_w error {:.3e}",
            built.graph.segments(),
            built.graph.vertices(),
            built.graph.bytes(),
            built.graph.bytes() as f64 / 1_048_576.0,
            built.graph.bytes() as f64 / built.graph.vertices().max(1) as f64,
            built.graph.max_q_rel_error
        );
        println!("H1: {:?}", r.h1);
        println!("G-CON (hydro::verify) before the release: {:?}", r.g_con);
        println!("sums before the release: Q {:.6e} m3/s, Qw {:.6e} m3/s (not conservation figures)", r.sum_q, r.sum_qw);
        for (name, s) in &r.sediment.pits {
            println!("pits {name}: {} of {} land cells ({:.4} %), max {:.1} m", s.pit_cells, s.land_cells, 100.0 * s.share(), s.max_m);
        }
        let d = r.sediment.deposit;
        println!("deposition: eroded {:.4e} m3, deposited {:.4e}, exported {:.4e}", d.eroded, d.deposited, d.exported);
        println!("hash z after the sediment pass {:016x}", fnv(built.z.iter().map(|v| v.to_bits())));
        let (peak, now, commit) = memory3();
        println!(
            "THE COMPLETE BUILD (plan): peak working set {peak:.0} MiB ({:.0} bytes a cell), now {now:.0} MiB, peak commit {commit:.0} MiB ({:.2} GB of the 12 GB budget; the gate reads commit against 11,444 MiB); spill {} B on disk, deleted; H1 result kept: {}",
            peak * 1048576.0 / active as f64,
            peak.max(commit) * 1048576.0 / 1e9,
            r.spill_bytes,
            built.hydro.segments.len()
        );
        if dams_on {
            let t = Instant::now();
            let n = built.z.len();
            let paths = ["before", "thermal", "deposition"].map(|tag| format!("{dam_prefix}_{tag}.f64"));
            let [zb, zt, zd] = paths.clone().map(|p| read_f64s(&p, n));
            let r = dam_report(&grid, &zb, &zt, &zd, &built.z, &outlet);
            print_dams(&r, t.elapsed().as_secs_f64());
            let (peak, now, commit) = memory3();
            println!("memory after the dam count (the count holds four surfaces and two fills; after the build's own peaks above): peak working set {peak:.0} MiB, now {now:.0} MiB, peak commit {commit:.0} MiB");
            for p in paths {
                let _ = std::fs::remove_file(p);
            }
        }
        cpu_phase("the plan");
        return;
    }
    let rep = if mode == "part" || mode == "both" {
        // `part` keeps nothing between rounds (the memory of one solve); `both` keeps the memos like an edited build.
        let mut cache = if mode == "part" { RoundCache::without_memory() } else { RoundCache::default() };
        let mut zp = if mode == "both" { z.clone() } else { Vec::new() };
        let (rep, work) = if mode == "both" {
            solve_rounds_memo(&grid, &mut zp, &outlet, &p, 4, &threads, seed, &Drivers::default(), 256, &mut cache)
        } else {
            solve_rounds_memo(&grid, &mut z, &outlet, &p, 4, &threads, seed, &Drivers::default(), 256, &mut cache)
        };
        for (r, w) in work.iter().enumerate() {
            println!("  round {}: {} tiles, {} tile floods, {} memo hits", r + 1, w.tiles, w.tile_floods, w.tile_memo_hits);
        }
        println!("partitioned rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        let (peak, now) = memory_mb();
        println!(
            "memory after the partitioned rounds: peak {peak:.0} MiB, now {now:.0} MiB; bytes a cell at the peak {:.0}",
            peak * 1048576.0 / active as f64
        );
        if mode == "both" {
            let t = Instant::now();
            let _ = solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, seed);
            println!("resident rounds: {:.1} s", t.elapsed().as_secs_f64());
            let differ = z.iter().zip(&zp).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
            println!("cells whose height differs, partitioned against resident: {differ}");
            assert_eq!(differ, 0);
        }
        rep
    } else {
        let rep = solve_rounds(&grid, &mut z, &outlet, &p, 4, &threads, seed);
        println!("4 rounds, {threads} threads: {:.1} s", t.elapsed().as_secs_f64());
        rep
    };
    if let Some(b) = &rep.pre_breach {
        println!(
            "MARK round-0 breach: {} passes drained basins, {} drained, {} kept as lakes ({} cells), {} beside the border, cut {} cells; fixed point {}",
            b.passes_run, b.drained, b.kept_lake, b.cells_in_kept_lakes, b.kept_bound, b.cells_cut, if b.fixed_point { "reached" } else { "NOT reached (the cap of the passes was hit)" }
        );
    }
    for (r, s) in rep.rounds.iter().enumerate() {
        println!(
            "  round {}: {} catchments, largest {} cells, {} cells in catchments of 1,000 or more",
            r + 1,
            s.catchments,
            s.largest,
            s.in_large
        );
    }
    cpu_phase("the 4 rounds");
    let (peak, now) = memory_mb();
    println!(
        "memory after the 4 rounds: peak {peak:.0} MiB, now {now:.0} MiB; cells {active}, bytes a cell at the peak {:.0}",
        peak * 1048576.0 / active as f64
    );
    if mode == "full" {
        println!("hash z after the rounds {:016x}", fnv(z.iter().map(|v| v.to_bits())));
    }
    if a.get(3).is_some_and(|x| x == "sedround") {
        // GEN-152: the deposition's balance per catchment after each round (the surface after round r against the surface after round r - 1).
        use planet_generators::bake::sediment::catchment_balance;
        use planet_generators::bake::tile::Pass2Params;
        let mut q = Pass2Params::standard(seed, 0.0);
        q.carve.breach_max_m = CarveParams::default().breach_max_m;
        let mut prev = z_start.clone();
        for r in 1..=4usize {
            let t = Instant::now();
            let mut zr = if r == 4 { z.clone() } else { z_start.clone() };
            if r < 4 {
                let pr = CarveParams { steps: 5 * r, ..p };
                solve_rounds(&grid, &mut zr, &outlet, &pr, r, &threads, seed);
            }
            let mut work = zr.clone();
            let b = catchment_balance(&grid, &mut work, &prev, &q);
            println!(
                "round {r}: {} catchments, eroded {:.4e} m3, raised {:.4e}, exported {:.4e}; max relative error of a catchment {:.2e}, of the region {:.2e}; largest catchment's eroded column {:.1} % ({:.1} s)",
                b.catchments,
                b.eroded_m3,
                b.raised_m3,
                b.exported_m3,
                b.max_relative_error,
                b.total_relative_error,
                100.0 * b.largest_eroded_m3 / b.eroded_m3,
                t.elapsed().as_secs_f64()
            );
            prev = zr;
        }
        return;
    }
    if a.get(3).is_some_and(|x| x == "thdiag") {
        // The deepest pits thermal makes: where they are and what their neighbourhood did.
        use planet_generators::bake::flood::priority_flood;
        use planet_generators::bake::tile::Pass2Params;
        let q = Pass2Params::standard(seed, 0.0);
        let mut zt = z.clone();
        let th = planet_generators::bake::thermal::thermal(&grid, &mut zt, &q.thermal, threads);
        println!("thermal: {} passes", th.passes_run);
        // The same outlets as the sediment steps: the sea and the landmass's boundary cells.
        let outlet = planet_generators::bake::sediment::outlets(&grid, &zt, 0.0);
        let filled = priority_flood(&grid, &zt, &outlet, q.carve.epsilon);
        let before_out = planet_generators::bake::sediment::outlets(&grid, &z, 0.0);
        let filled0 = priority_flood(&grid, &z, &before_out, q.carve.epsilon);
        let mut deep: Vec<(f64, usize)> =
            (0..z.len()).filter(|&p| zt[p] > 0.0 && !outlet[p]).map(|p| (filled[p] - zt[p], p)).filter(|d| d.0 >= 0.5).collect();
        deep.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (mut ring, mut near_sea, mut all_pre) = (0, 0, 0);
        for &(_, p) in &deep {
            let nb: Vec<usize> = (0..8).filter_map(|k| grid.neighbour(p, k)).collect();
            let sea_nb = nb.iter().any(|&q| zt[q] <= 0.0);
            let was_pit = filled0[p] - z[p] >= 0.5;
            if nb.len() < 8 {
                ring += 1;
            }
            if sea_nb {
                near_sea += 1;
            }
            if was_pit {
                all_pre += 1;
            }
        }
        // How steep the surface was before thermal: land cells with a neighbour whose height difference exceeds the critical slope, and the deep pits' share of them.
        let tan = libm::tan(q.thermal.theta_deg.to_radians());
        let steep = |p: usize| (0..8).any(|k| grid.neighbour(p, k).is_some_and(|nb| (z[p] - z[nb]).abs() > tan * grid.distance_at(p, k)));
        let land: Vec<usize> = (0..z.len()).filter(|&p| z[p] > 0.0).collect();
        let steep_cells = land.iter().filter(|&&p| steep(p)).count();
        let deep50: Vec<usize> = deep.iter().filter(|d| d.0 >= 50.0).map(|d| d.1).collect();
        let deep_steep = deep50.iter().filter(|&&p| steep(p) || (0..8).filter_map(|k| grid.neighbour(p, k)).any(steep)).count();
        let deep_pre = deep50.iter().filter(|&&p| filled0[p] - z[p] >= 0.5).count();
        println!(
            "before thermal: {steep_cells} of {} land cells have a neighbour steeper than the critical slope ({:.3} %); pits of 50 m or deeper after thermal: {} ({} of them a pit before, {} with a steep cell in their 3x3)",
            land.len(),
            100.0 * steep_cells as f64 / land.len() as f64,
            deep50.len(),
            deep_pre,
            deep_steep
        );
        println!("{} pits; with fewer than 8 neighbours {ring}; next to sea {near_sea}; already pits before thermal {all_pre}", deep.len());
        for &(d, p) in deep.iter().take(8) {
            let g = grid.cube_id_of(p);
            let nb: Vec<usize> = (0..8).filter_map(|k| grid.neighbour(p, k)).collect();
            let fmt = |v: &[f64], idx: &[usize]| idx.iter().map(|&q| format!("{:.0}", v[q])).collect::<Vec<_>>().join(" ");
            println!(
                "pit {d:.1} m at cell {p} (face {}, i {}, j {}, {} neighbours): z {:.1} -> {:.1}, filled0 {:.1}; neighbours before [{}] after [{}]",
                g / nn,
                (g % nn) % n,
                (g % nn) / n,
                nb.len(),
                z[p],
                zt[p],
                filled0[p],
                fmt(&z, &nb),
                fmt(&zt, &nb)
            );
        }
        return;
    }
    if a.get(3).is_some_and(|x| x == "sed") {
        // GEN-151: the sediment steps on the whole compact landmass after the rounds (A3 6.7), with the breach-lowering share of the eroded column.
        use planet_generators::bake::sediment::{breach_lowered_m3, sediment_pass};
        use planet_generators::bake::tile::Pass2Params;
        let mut q = Pass2Params::standard(seed, 0.0);
        q.carve.breach_max_m = CarveParams::default().breach_max_m;
        let t = Instant::now();
        let breach = breach_lowered_m3(&grid, &z_start, &outlet, &CarveParams { max_area: 0.0, ..p }, seed);
        println!("breach alone on the starting surface: {:.3e} m3 ({:.1} s)", breach, t.elapsed().as_secs_f64());
        let t = Instant::now();
        let r = sediment_pass(&grid, &mut z, &z_start, &q, threads);
        println!("sediment steps: {:.1} s", t.elapsed().as_secs_f64());
        for (name, s) in &r.pits {
            println!(
                "pits {name}: {} of {} land cells ({:.4} %), p50 {:.2} m, p99 {:.2} m, max {:.1} m",
                s.pit_cells,
                s.land_cells,
                100.0 * s.share(),
                s.p50_m,
                s.p99_m,
                s.max_m
            );
        }
        println!(
            "thermal: {} passes, {} cells still gave in the last, volume change {:.2e}",
            r.thermal.passes_run, r.thermal.moved_last, r.thermal_volume_change
        );
        let d = r.deposit;
        println!(
            "deposition: eroded {:.4e} m3, deposited {:.4e}, exported {:.4e}; (eroded - deposited - exported) / eroded = {:.2e}",
            d.eroded,
            d.deposited,
            d.exported,
            (d.eroded - d.deposited - d.exported) / d.eroded
        );
        println!(
            "eroded column before thermal {:.4e} m3, after thermal {:.4e} (the deposition reads the latter)",
            r.eroded_before_thermal_m3, r.eroded_after_thermal_m3
        );
        println!("deposition, change of the height volume: {:.4e} m3 (deposited {:.4e})", r.deposit_height_volume_m3, d.deposited);
        println!(
            "pit spreading: {} passes, volume change {:.2e}; first-round breach alone is {:.1} % of the eroded column (a lower bound)",
            r.spread_passes,
            r.spread_volume_change,
            100.0 * breach / d.eroded
        );
        let (peak, now) = memory_mb();
        println!("memory after the sediment steps: peak {peak:.0} MiB, now {now:.0} MiB");
    }
    let mut hy_keep: Option<planet_generators::bake::hydro::Hydro> = None;
    let diag = a.get(4).is_some_and(|x| x == "diag");
    let mut pre_pits: Option<Vec<bool>> = None;
    if a.get(3).is_some_and(|x| x == "h1" || x == "dump" || x == "full" || x == "reconcile") {
        // `full`: the pre-carve surface is read again only by the sediment pass; it waits on disk while H1 holds its drivers, routing and filled surface (A3 6.6 rule 1).
        let spill = format!("target/full_zstart_k{k}.f64");
        let keep = a.get(5).is_some_and(|x| x == "keep"); // the old shape, to compare the hashes against
        if mode == "full" && !keep {
            use std::io::Write;
            let mut f = std::io::BufWriter::new(std::fs::File::create(&spill).expect("the spill file"));
            for v in &z_start {
                f.write_all(&v.to_le_bytes()).expect("the spill write");
            }
            f.flush().expect("the spill flush");
            z_start = Vec::new();
        }
        let o = inputs.orography();
        let mut weight = vec![1.0f32; active];
        let mut runoff = vec![0.0f32; active];
        let mut evap = vec![0.0f32; active];
        let per = active.div_ceil(threads);
        let t = Instant::now();
        std::thread::scope(|s| {
            for (((wc, rc), ec), t) in weight.chunks_mut(per).zip(runoff.chunks_mut(per)).zip(evap.chunks_mut(per)).zip(0..) {
                let (grid, z, o) = (&grid, &z, o);
                s.spawn(move || {
                    for c in 0..wc.len() {
                        let p = t * per + c;
                        if z[p] <= 0.0 {
                            continue;
                        }
                        let g = grid.cube_id_of(p);
                        let spec = FaceSpec { face: Face((g / nn) as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                        let d: Vec3 = spec.halo_direction(((g % nn) % n) as i64, ((g % nn) / n) as i64);
                        let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                        let ro = runoff_mm(pm, tc);
                        wc[c] = discharge_weight(ro) as f32;
                        rc[c] = (ro / 1000.0 / 31_557_600.0) as f32;
                        ec[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / 31_557_600.0) as f32;
                    }
                });
            }
        });
        println!("runoff, weights and evaporation: {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let routing = route_rules(&grid, &z, &outlet, 1e-3, seed, 0.0, Some(&weight), RoundRules::ROUND);
        let filled = priority_flood_with(&grid, &z, &outlet, 1e-3, true);
        println!("weighted route and filled surface: {:.1} s", t.elapsed().as_secs_f64());
        let weight_keep = if mode == "reconcile" {
            Some(weight)
        } else {
            drop(weight);
            None
        };
        let t = Instant::now();
        let hy = hydrology(&grid, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
        println!("H1: {:.1} s; {:?}", t.elapsed().as_secs_f64(), hy.stats);
        let (peak, now) = memory_mb();
        cpu_phase("H1 (drivers, weighted route, filled surface, hydrology)");
        println!("memory after H1: peak {peak:.0} MiB, now {now:.0} MiB; bytes a cell at the peak {:.0}", peak * 1048576.0 / active as f64);
        if mode == "reconcile" {
            // #111 (architect): the pit definition of the sediment pass against H1's basins on the same surface (the surface after the rounds), cell by cell.
            use planet_generators::bake::flood::priority_flood;
            use planet_generators::bake::sediment::outlets;
            let v = planet_generators::bake::hydro::verify(&grid, &hy, &routing, &filled, &outlet, Some(&runoff), weight_keep.as_deref());
            println!("G-CON (hydro::verify): {v:?}");
            let q = planet_generators::bake::tile::Pass2Params::standard(seed, 0.0);
            let outl_s = outlets(&grid, &z, q.sea_level);
            println!(
                "outlet sets: H1's {} cells, the sediment pass's {} cells, in both {}; sediment epsilon {}, H1 epsilon 1e-3",
                outlet.iter().filter(|&&o| o).count(),
                outl_s.iter().filter(|&&o| o).count(),
                (0..z.len()).filter(|&p| outlet[p] && outl_s[p]).count(),
                q.carve.epsilon
            );
            let mask = |fill: &[f64], o: &[bool]| -> Vec<bool> {
                (0..z.len()).map(|p| grid.is_active(p) && !o[p] && fill[p] - z[p] > 0.5).collect()
            };
            let count = |m: &[bool]| m.iter().filter(|&&b| b).count();
            let h = mask(&filled, &outlet);
            let f_s = priority_flood(&grid, &z, &outl_s, q.carve.epsilon);
            let sd = mask(&f_s, &outl_s);
            let f_x1 = priority_flood(&grid, &z, &outlet, 1e-3);
            let x1 = mask(&f_x1, &outlet);
            drop(f_x1);
            let f_x2 = priority_flood_with(&grid, &z, &outl_s, 1e-3, true);
            let x2 = mask(&f_x2, &outl_s);
            drop(f_x2);
            println!(
                "pit cells (filled minus ground over 0.5 m, active, not an outlet of the set): H1's flood and outlets {} ; the sediment pass's flood, epsilon and outlets {} ; both in both {} ; H1 outlets with the plain flood at 1e-3 {} ; sediment outlets with H1's canonical flood {} ; H1 basin_cells {}",
                count(&h),
                count(&sd),
                (0..z.len()).filter(|&p| h[p] && sd[p]).count(),
                count(&x1),
                count(&x2),
                hy.stats.basin_cells
            );
            let (mut differ, mut maxd, mut maxd_at) = (0usize, 0.0f64, 0usize);
            for p in 0..z.len() {
                if outlet[p] == outl_s[p] {
                    let d = libm::fabs(filled[p] - f_s[p]);
                    if d > 1e-6 {
                        differ += 1;
                    }
                    if d > maxd {
                        (maxd, maxd_at) = (d, p);
                    }
                }
            }
            println!("filled surfaces (H1's against the sediment pass's) on cells with the same outlet flag: {differ} differ by over 1e-6 m, the largest {maxd:.3} m at cell {maxd_at}");
            // H1's components of its pit cells and its rules: ramps (rise under pit_min), artefacts (few cells and shallow), kept.
            let (mut seen, mut comps) = (vec![false; z.len()], Vec::<(usize, f64, usize)>::new());
            for p0 in 0..z.len() {
                if !sd[p0] || seen[p0] {
                    continue;
                }
                seen[p0] = true;
                let (mut st, mut cells, mut deepest, mut edge) = (vec![p0], 0usize, 0.0f64, 0usize);
                while let Some(p) = st.pop() {
                    cells += 1;
                    deepest = deepest.max(f_s[p] - z[p]);
                    let mut touches = false;
                    for d in 0..8 {
                        match grid.neighbour(p, d) {
                            Some(nb) => {
                                if outl_s[nb] {
                                    touches = true;
                                }
                                if sd[nb] && !seen[nb] {
                                    seen[nb] = true;
                                    st.push(nb);
                                }
                            }
                            None => touches = true,
                        }
                    }
                    edge += usize::from(touches);
                }
                comps.push((cells, deepest, edge));
            }
            comps.sort_by_key(|a| std::cmp::Reverse(a.0));
            let total: usize = comps.iter().map(|c| c.0).sum();
            println!("components of the sediment pass's pit cells: {} with {total} cells; the largest ten:", comps.len());
            let mut run = 0usize;
            for c in comps.iter().take(10) {
                run += c.0;
                println!(
                    "  {} cells, deepest {:.1} m, {} of its cells touch an outlet cell or the grid's edge; cumulative share {:.1} %",
                    c.0,
                    c.1,
                    c.2,
                    100.0 * run as f64 / total as f64
                );
            }
            let in_h1 = (0..z.len()).filter(|&p| sd[p] && hy.label[p] != u32::MAX).count();
            println!(
                "sediment-pass pit cells that H1 labels as a kept basin: {in_h1}; H1 labelled cells in all: {}",
                hy.label.iter().filter(|&&l| l != u32::MAX).count()
            );
        }
        if a.get(3).is_some_and(|x| x == "dump") {
            // GEN: G-STORE (STRM-078): the raw state of the build, for `cache/examples/store_measure`, which encodes it as store records.
            use std::io::Write;
            let dir = format!("target/store_dump_k{k}");
            std::fs::create_dir_all(&dir).expect("the dump directory");
            let w = |name: &str, bytes: Vec<u8>| {
                std::fs::File::create(format!("{dir}/{name}")).and_then(|mut f| f.write_all(&bytes)).expect("a dump file");
            };
            let f32s = |v: &[f64]| v.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect::<Vec<u8>>();
            w(
                "meta.txt",
                format!(
                    "k {k}
cells {active}
face_cells {n}
"
                )
                .into_bytes(),
            );
            w("heights_r4.f32", f32s(&z));
            w("heights_r0.f32", f32s(&z_start));
            for r in 1..4usize {
                let mut zr = z_start.clone();
                solve_rounds(&grid, &mut zr, &outlet, &CarveParams { steps: 5 * r, ..p }, r, &threads, seed);
                w(&format!("heights_r{r}.f32"), f32s(&zr));
            }
            w("receiver.u32", routing.receiver.iter().flat_map(|x| x.to_le_bytes()).collect());
            w("area.f32", f32s(&routing.area));
            w("filled.f32", f32s(&filled));
            w("cube_id.u32", (0..z.len()).flat_map(|p| (grid.cube_id_of(p) as u32).to_le_bytes()).collect());
            w("land.u8", z.iter().map(|&h| u8::from(h > 0.0)).collect());
            let mut seg = Vec::new();
            for sg in &hy.segments {
                seg.extend(sg.id.to_le_bytes());
                for v in [sg.up, sg.down, sg.cells] {
                    seg.extend(v.to_le_bytes());
                }
                seg.extend(sg.q_up.to_bits().to_le_bytes());
                seg.extend(sg.q_down.to_bits().to_le_bytes());
                seg.extend(sg.downstream.to_le_bytes());
            }
            w("segments.bin", seg);
            let mut bas = Vec::new();
            for b in &hy.basins {
                bas.extend(b.id.to_le_bytes());
                bas.extend(b.floor_cell.to_le_bytes());
                bas.extend(b.floor_m.to_bits().to_le_bytes());
                bas.extend(b.spill_cell.to_le_bytes());
                bas.extend(b.spill_m.to_bits().to_le_bytes());
                bas.extend(b.cells.to_le_bytes());
                for v in b.area_m2.iter().chain(&b.volume_m3) {
                    bas.extend(v.to_bits().to_le_bytes());
                }
                bas.extend(b.q_in.to_bits().to_le_bytes());
                bas.push(b.class as u8);
                bas.push(u8::from(b.overflows));
                bas.extend(b.water_level_m.to_bits().to_le_bytes());
                bas.extend(b.q_out.to_bits().to_le_bytes());
            }
            w("basins.bin", bas);
            let mut bits = vec![0u8; hy.channel.len().div_ceil(8)];
            for (i, &c) in hy.channel.iter().enumerate() {
                bits[i >> 3] |= u8::from(c) << (i & 7);
            }
            w("channel.bits", bits);
            w("label.u32", hy.label.iter().flat_map(|x| x.to_le_bytes()).collect());
            println!("dump written to {dir}: {} segments, {} basins", hy.segments.len(), hy.basins.len());
        }
        if a.get(3).is_some_and(|x| x == "full") {
            // The build keeps H1's result (channel graph, basins, labels); the routing, filled surface and drivers are temporaries released here.
            println!(
                "hash H1: label {:016x}, channel {:016x}, segments {:016x}",
                fnv(hy.label.iter().map(|&v| u64::from(v))),
                fnv(hy.channel.iter().map(|&c| u64::from(c))),
                fnv(hy.segments.iter().flat_map(|g| [g.id, g.q_up.to_bits(), g.q_down.to_bits(), u64::from(g.cells)]))
            );
            // The kept result is the channel graph, basins, labels and channel flags; the per-cell potential and routed discharge (two f64 a cell) are derivable and segments hold Q_w, so they are released.
            let mut hy = hy;
            println!(
                "G-CON sums before the release: sum Q {:.6e} m3/s, sum Q_w {:.6e} m3/s",
                hy.q.iter().sum::<f64>(),
                hy.qw.iter().sum::<f64>()
            );
            if !keep {
                hy.q = Vec::new();
                hy.qw = Vec::new();
            }
            hy_keep = Some(hy);
        }
    }
    if a.get(3).is_some_and(|x| x == "full") {
        // The complete resident build in one process: rounds, H1 (above) and the sediment pass, against 12 GB (A3 6.6). H1's result stays resident.
        use planet_generators::bake::sediment::sediment_pass;
        use planet_generators::bake::tile::Pass2Params;
        if z_start.is_empty() {
            let bytes = std::fs::read(format!("target/full_zstart_k{k}.f64")).expect("the spill file");
            z_start = bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
            drop(bytes);
            std::fs::remove_file(format!("target/full_zstart_k{k}.f64")).expect("the spill file is deleted after the read-back");
        }
        let (peak, now, commit) = memory3();
        println!("memory before the sediment pass: peak working set {peak:.0} MiB, now {now:.0} MiB, peak commit {commit:.0} MiB");
        let mut q = Pass2Params::standard(seed, 0.0);
        q.carve.breach_max_m = CarveParams::default().breach_max_m;
        if diag {
            pre_pits = Some(pit_report("after the last round, before the sediment pass", &grid, &z, &q, None, None));
        }
        let t = Instant::now();
        let r = sediment_pass(&grid, &mut z, &z_start, &q, threads);
        println!("sediment pass: {:.1} s", t.elapsed().as_secs_f64());
        cpu_phase("the sediment pass");
        for (name, s) in &r.pits {
            println!("pits {name}: {} of {} land cells ({:.4} %), max {:.1} m", s.pit_cells, s.land_cells, 100.0 * s.share(), s.max_m);
        }
        let d = r.deposit;
        println!("deposition: eroded {:.4e} m3, deposited {:.4e}, exported {:.4e}", d.eroded, d.deposited, d.exported);
        let (peak, now, commit) = memory3();
        println!(
            "THE COMPLETE BUILD: peak working set {peak:.0} MiB ({:.0} bytes a cell), now {now:.0} MiB, peak commit {commit:.0} MiB ({:.2} GB of the 12 GB budget); H1 result kept: {}",
            peak * 1048576.0 / active as f64,
            peak.max(commit) * 1048576.0 / 1e9,
            hy_keep.as_ref().map_or(0, |h| h.segments.len())
        );
    }
    if mode == "full" {
        println!("hash z after the sediment pass {:016x}", fnv(z.iter().map(|v| v.to_bits())));
        // The pits left (A3 6.7 report, planner #107): H1 ran on the surface before the sediment pass.
        let mut q = planet_generators::bake::tile::Pass2Params::standard(seed, 0.0);
        q.carve.breach_max_m = CarveParams::default().breach_max_m;
        let after = pit_report(
            "after the sediment pass, classes of H1 on the surface before it",
            &grid,
            &z,
            &q,
            hy_keep.as_ref(),
            pre_pits.as_deref(),
        );
        if diag {
            // #108 (c): the post-sediment surface classed as if K6 had read it: H1 again on the final surface (the kept result and the pre-carve surface are released first).
            drop(hy_keep.take());
            drop(after);
            let o = inputs.orography();
            let mut weight = vec![1.0f32; active];
            let mut runoff = vec![0.0f32; active];
            let mut evap = vec![0.0f32; active];
            let per = active.div_ceil(threads);
            std::thread::scope(|s| {
                for (((wc, rc), ec), t) in weight.chunks_mut(per).zip(runoff.chunks_mut(per)).zip(evap.chunks_mut(per)).zip(0..) {
                    let (grid, z, o) = (&grid, &z, o);
                    s.spawn(move || {
                        for c in 0..wc.len() {
                            let p = t * per + c;
                            if z[p] <= 0.0 {
                                continue;
                            }
                            let g = grid.cube_id_of(p);
                            let spec = FaceSpec { face: Face((g / nn) as u8), cells: n, radius_m: R, sea_level: 0.0, seed, halo: 0 };
                            let d: Vec3 = spec.halo_direction(((g % nn) % n) as i64, ((g % nn) / n) as i64);
                            let (pm, tc) = (o.precipitation_mm(d), o.temperature_c(d));
                            let ro = runoff_mm(pm, tc);
                            wc[c] = discharge_weight(ro) as f32;
                            rc[c] = (ro / 1000.0 / 31_557_600.0) as f32;
                            ec[c] = ((pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / 31_557_600.0) as f32;
                        }
                    });
                }
            });
            let routing = route_rules(&grid, &z, &outlet, 1e-3, seed, 0.0, Some(&weight), RoundRules::ROUND);
            drop(weight);
            let filled = priority_flood_with(&grid, &z, &outlet, 1e-3, true);
            let hy2 = hydrology(&grid, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), &HydroParams::default());
            println!("H1 on the final surface: {:?}", hy2.stats);
            drop((routing, filled, runoff, evap));
            let _ = pit_report("after the sediment pass, classes of H1 on that surface", &grid, &z, &q, Some(&hy2), pre_pits.as_deref());
        }
    }
    let mut dz: Vec<f64> = region.iter().map(|&(p, z0)| z[p] - z0).collect();
    dz.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| dz[((dz.len() - 1) as f64 * f) as usize];
    println!("region land change (m): n {} p5 {:.1} p50 {:.1} p95 {:.1} max {:.1}", dz.len(), q(0.05), q(0.5), q(0.95), dz[dz.len() - 1]);
}
