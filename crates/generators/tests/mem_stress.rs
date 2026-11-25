//! Spike only (crusader, G3-6 / R13, lead 2026-11-25 19:20): the planet-scale memory gate's byte model and its 1/16-scale measurement.
//! - `the_byte_model_at_the_largest_landmass` (tier A): every component of wave 1 and wave 2 and the per-tile L buffers, in bytes per cell from the arrays the code holds, at the largest
//!   landmass's cell count (8.4e7 km², terrain-target §6.6: 8.10e9 L cells, 1.27e8 H1 cells) with 16 worker `CubePatch` buffers; the components over the 10.8 GB gate are named.
//! - `wave_1_at_one_sixteenth_scale_against_the_model` (ignored, run by hand: `cargo test --release -p planet-generators --test mem_stress -- --ignored --nocapture`): wave 1's
//!   kernels on a cube of 1/16 the largest landmass's H1 cells, and band6's drain on one worker's L patch (1/16 of the 16 buffers), each stage's peak heap measured by a counting
//!   allocator (this binary's global allocator) and the process's peak RSS, against the model's prediction for the same cells.
//!
//! Lock-free: no store, no window run.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// Counts the live heap bytes and their peak.
struct Counting;
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call forwards to the system allocator with the same layout; the counters only observe sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: the caller's layout contract is passed through unchanged.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: as above.
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), Relaxed);
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: as above.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
        }
        p
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        // SAFETY: as above.
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            if new >= l.size() {
                let c = CUR.fetch_add(new - l.size(), Relaxed) + (new - l.size());
                PEAK.fetch_max(c, Relaxed);
            } else {
                CUR.fetch_sub(l.size() - new, Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Runs `f` and returns its value with the peak heap above the start (bytes).
fn peak<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = CUR.load(Relaxed);
    PEAK.store(base, Relaxed);
    let t = f();
    (t, PEAK.load(Relaxed).saturating_sub(base))
}

/// The process's peak working set and working set (MiB).
#[cfg(windows)]
fn rss_mib() -> (u64, u64) {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c =
        Counters { cb: core::mem::size_of::<Counters>() as u32, page_fault_count: 0, peak_working_set: 0, working_set: 0, rest: [0; 6] };
    // SAFETY: `c` is a writable PROCESS_MEMORY_COUNTERS of the size it declares; the pseudo handle needs no closing.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok == 0 {
        return (0, 0);
    }
    ((c.peak_working_set >> 20) as u64, (c.working_set >> 20) as u64)
}

#[cfg(not(windows))]
fn rss_mib() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).unwrap_or(0);
    (kb("VmHWM:") >> 10, kb("VmRSS:") >> 10)
}

// ---------------------------------------------------------------- the model

/// The gate (G3-6): 10.8 GB, 10 % headroom under the 12 GB world-build budget.
const GATE_B: f64 = 10.8e9;
/// The largest landmass (terrain-target §6.6) and the nominal cells (6,371 km x π/2 / 98,304 = 101.8 m at L; 8 x 8 L cells an H1 cell).
const LAND_KM2: f64 = 8.4e7;
const L_CELL_KM2: f64 = 0.010364;
const H1_PER_L: f64 = 64.0;
/// GEN-182's production tile and halo (L cells), and the workers in flight.
const TILE_L: f64 = 3072.0;
const HALO_L: f64 = 128.0;
const WORKERS: f64 = 16.0;
/// The H1 tile (GEN-143's partitioned route at H1: 3,072 / 8).
const TILE_H1: f64 = 384.0;
const HALO_H1: f64 = 16.0;

/// How a component's bytes grow.
#[derive(Clone, Copy, Debug)]
enum Basis {
    /// per H1 land cell, resident over the landmass
    H1Land,
    /// per cell of one worker's buffer, times the workers
    Patch { l: bool },
    /// a fixed total (bytes)
    Total,
}

/// A component: its wave, what it holds (bytes per cell, from the code's arrays), its basis and where the number comes from.
struct Component {
    wave: &'static str,
    name: &'static str,
    bytes_per_cell: f64,
    basis: Basis,
    source: &'static str,
}

fn cells() -> (f64, f64) {
    let l = LAND_KM2 / L_CELL_KM2;
    (l, l / H1_PER_L)
}

fn bytes(c: &Component) -> f64 {
    let (_, h1) = cells();
    match c.basis {
        Basis::H1Land => c.bytes_per_cell * h1,
        Basis::Patch { l: true } => c.bytes_per_cell * (TILE_L + 2.0 * HALO_L) * (TILE_L + 2.0 * HALO_L) * WORKERS,
        Basis::Patch { l: false } => c.bytes_per_cell * (TILE_H1 + 2.0 * HALO_H1) * (TILE_H1 + 2.0 * HALO_H1) * WORKERS,
        Basis::Total => c.bytes_per_cell,
    }
}

/// The components as the code holds them today (counted from the arrays; the 1/16 run measures the wave-1 rows).
fn components() -> Vec<Component> {
    let f64b = core::mem::size_of::<f64>() as f64;
    let u32b = core::mem::size_of::<u32>() as f64;
    let boolb = core::mem::size_of::<bool>() as f64;
    vec![
        Component { wave: "1", name: "z_b, z_s and the outlets, resident", bytes_per_cell: 2.0 * f64b + boolb, basis: Basis::H1Land, source: "the surfaces wave 1 reads and writes" },
        Component {
            wave: "1",
            name: "base1 / sel-post, global round input (level, label, depression cells, output copy)",
            bytes_per_cell: f64b + u32b + u32b + f64b,
            basis: Basis::H1Land,
            source: "drain::sel_round + sel_apply's copy",
        },
        Component {
            wave: "1",
            name: "K5, GEN-143's partitioned route (filled, receiver, sum, pending, ready, area, order)",
            bytes_per_cell: f64b + u32b + f64b + 1.0 + u32b + f64b + u32b,
            basis: Basis::H1Land,
            source: "partition.rs: resident arrays (\"this change gates the equality, not the memory\")",
        },
        Component {
            wave: "1",
            name: "K5, a round's catchment members",
            bytes_per_cell: u32b,
            basis: Basis::H1Land,
            source: "round::cut_in",
        },
        Component {
            wave: "1",
            name: "K5, gathered catchment inputs in flight (z, receiver, area, dist, taper, outlet flag, scale, up unit)",
            bytes_per_cell: 6.0 * f64b - f64b + u32b + 4.0 + boolb,
            basis: Basis::H1Land,
            source: "round::CatchmentInputs; the largest-first queue puts the 16 largest catchments in flight, which on one continent hold about all its cells (the 1/16 run measures the share)",
        },
        Component {
            wave: "1",
            name: "K6, routing (16), filled (8), hydrology (24), d8 (8)",
            bytes_per_cell: 2.0 * u32b + f64b + f64b + (2.0 * f64b + u32b + 4.0 * boolb) + f64b,
            basis: Basis::H1Land,
            source: "route::Routing, flood, hydro::hydrology's arrays, rg_h1::H1.d8",
        },
        Component {
            wave: "2",
            name: "landmass trunks, portal-only (explorer #195, exact; measured 11.6-19.2 GB)",
            bytes_per_cell: 8.8e9,
            basis: Basis::Total,
            source: "explorer #195 (R1h 5.2, R3a 8.8, M1 6.6 GB exact): the worst window",
        },
        Component { wave: "2", name: "portal records (80 B)", bytes_per_cell: 0.08e9, basis: Basis::Total, source: "explorer #195" },
        Component {
            wave: "L",
            name: "per-tile L stages, 16 worker buffers (3,328² cells each)",
            bytes_per_cell: 177.0,
            basis: Basis::Patch { l: true },
            source: "rg_chain's window peak, 10.3-11.7 GiB over 8,160² L cells (seam design §3): 177 B a cell at 11.0 GiB, an upper bound (it holds the window's H1 too)",
        },
        Component {
            wave: "L",
            name: "base1's tiled form instead of the global one (#207), 16 H1 buffers",
            bytes_per_cell: f64b + u32b + u32b + f64b,
            basis: Basis::Patch { l: false },
            source: "the same arrays per (384 + 32)² patch",
        },
    ]
}

// spike only (crusader, G3-6 / R13): the byte model
#[test]
fn the_byte_model_at_the_largest_landmass() {
    let (l, h1) = cells();
    eprintln!("G3-6 model: the largest landmass {LAND_KM2:.1e} km² = {:.3e} L cells, {:.3e} H1 cells; gate {:.1} GB", l, h1, GATE_B / 1e9);
    let comps = components();
    let mut over = Vec::new();
    let (mut w1, mut w2) = (0.0, 0.0);
    for c in &comps {
        let b = bytes(c);
        eprintln!(
            "G3-6 model | {} | {} | {:.1} B a cell, {:?} | {:.2} GB | {}",
            c.wave,
            c.name,
            c.bytes_per_cell,
            c.basis,
            b / 1e9,
            c.source
        );
        if b > GATE_B {
            over.push(c.name);
        }
        match c.wave {
            "1" => w1 += b,
            "2" => w2 += b,
            _ => {}
        }
    }
    // wave 1 resident at its peak (K6, with the surfaces); K5's route and gathers are freed before K6
    let peak = |names: &[&str]| comps.iter().filter(|c| names.iter().any(|n| c.name.starts_with(n))).map(bytes).sum::<f64>();
    let w1_k5 = peak(&["z_b, z_s", "K5, GEN-143", "K5, a round", "K5, gathered"]);
    let w1_k6 = peak(&["z_b, z_s", "K6"]);
    let l_tiles = peak(&["per-tile L stages"]);
    eprintln!(
        "G3-6 model: wave 1 at K5 {:.2} GB, at K6 {:.2} GB (all wave-1 rows summed {:.2} GB); wave 2 {:.2} GB; L buffers {:.2} GB; components over the gate alone: {over:?}",
        w1_k5 / 1e9,
        w1_k6 / 1e9,
        w1 / 1e9,
        w2 / 1e9,
        l_tiles / 1e9
    );
    // the verdicts the report names (a change to a layout changes them, and this test with them)
    assert!(w1_k5 > GATE_B, "wave 1's K5 with GEN-143's resident arrays does not fit");
    assert!(l_tiles > GATE_B, "16 L buffers of 3,328² at today's bytes a cell do not fit");
    assert!(w2 < GATE_B, "wave 2 portal-only fits alone");
    // what would fit: the L buffers' workers at today's bytes, and the H1 bytes a cell a resident wave 1 may hold
    let per_buffer = l_tiles / WORKERS;
    eprintln!(
        "G3-6 model: one L buffer {:.2} GB, so {:.0} workers fit the gate alone; a resident wave 1 must hold under {:.0} B an H1 land cell",
        per_buffer / 1e9,
        (GATE_B / per_buffer).floor(),
        GATE_B / h1
    );
}

// ---------------------------------------------------------------- the 1/16 run

/// A deterministic relief on the cube grid of `n` cells an edge: seven octaves of sines of the direction, about a fifth of it sea.
fn relief(n: usize) -> Vec<f64> {
    let gg = planet_generators::bake::h0::GlobalGrid::new(n, 6_371_000.0);
    let k =
        [(0.8, 0.5, 0.3), (-0.4, 0.9, 0.2), (0.3, -0.2, 0.93), (0.6, 0.6, -0.5), (-0.7, 0.1, 0.7), (0.2, -0.95, 0.24), (0.9, -0.3, -0.3)];
    (0..6 * n * n)
        .map(|p| {
            let d = gg.direction(p as u32);
            let mut z = -300.0;
            for (o, &(a, b, c)) in k.iter().enumerate() {
                let f = 3.0 * (1u32 << o) as f64;
                z += 1200.0 / (1u32 << o) as f64 * libm::sin(f * (a * d.x + b * d.y + c * d.z) + o as f64);
            }
            z
        })
        .collect()
}

// spike only (crusader, G3-6 / R13): wave 1 at 1/16 of the largest landmass, measured on today's full cube grid
#[test]
#[ignore = "a measurement run (minutes, about 2 GB): cargo test --release -p planet-generators --test mem_stress -- --ignored --nocapture"]
fn wave_1_at_one_sixteenth_scale_against_the_model() {
    wave_1_measured(false);
}

// spike only (crusader, G3-6 layout step 1): the same on the compact grid of the land and its coastal sea (GEN-147)
#[test]
#[ignore = "a measurement run (minutes, about 1 GB): cargo test --release -p planet-generators --test mem_stress -- --ignored --nocapture"]
fn wave_1_on_the_compact_grid_at_one_sixteenth_scale() {
    wave_1_measured(true);
}

fn wave_1_measured(compact: bool) {
    use planet_generators::bake::carve::CarveParams;
    use planet_generators::bake::drain::{drain_selective_floors_on, SelParams, SelSwitches};
    use planet_generators::bake::flood::priority_flood_with;
    use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
    use planet_generators::bake::round::{cut_in, solve_rounds_memo, Drivers, RoundCache};
    use planet_generators::bake::route::{RoundRules, NONE};
    use planet_generators::bake::Grid;
    let n = 1152;
    let full = Grid::cube(n, 6_371_000.0);
    let zf = relief(n);
    let of: Vec<bool> = zf.iter().map(|&h| h <= 0.0).collect();
    // the compact grid holds the land and the sea cells next to it (the outlets land drains to)
    let (g, z0, outlet) = if compact {
        let mut bits = vec![0u64; (6 * n * n).div_ceil(64)];
        for p in 0..6 * n * n {
            if !of[p] || (0..8).any(|d| full.neighbour(p, d).is_some_and(|q| !of[q])) {
                bits[p / 64] |= 1 << (p % 64);
            }
        }
        let g = Grid::compact_cube(n, 6_371_000.0, bits);
        let ids: Vec<usize> = (0..g.len()).map(|p| g.cube_id_of(p)).collect();
        let z: Vec<f64> = ids.iter().map(|&i| zf[i]).collect();
        let o: Vec<bool> = ids.iter().map(|&i| of[i]).collect();
        drop((zf, of));
        (g, z, o)
    } else {
        (full, zf, of)
    };
    let land = outlet.iter().filter(|&&o| !o).count();
    let (_, h1_full) = cells();
    eprintln!(
        "G3-6 1/16 ({}): {} cells on the grid, {land} land ({:.3} of the largest landmass's H1 land)",
        if compact { "compact grid" } else { "full cube" },
        z0.len(),
        land as f64 / h1_full
    );
    let model = |name: &str| components().iter().filter(|c| c.name.starts_with(name)).map(|c| c.bytes_per_cell).sum::<f64>();
    let row = |stage: &str, t: f64, peak_b: usize, model_bpc: f64| {
        let (hwm, rss) = rss_mib();
        eprintln!(
            "G3-6 1/16 | {stage} | {t:.1} s | peak heap {:.2} GB = {:.1} B a cell ({:.1} B a land cell) | model {model_bpc:.1} B a cell | process peak RSS {hwm} MiB, RSS {rss} MiB",
            peak_b as f64 / 1e9,
            peak_b as f64 / z0.len() as f64,
            peak_b as f64 / land as f64
        );
    };
    // base1: Drain::SELECTIVE_50 in its global form
    let keep = |_: usize| false;
    let area = |p: usize| g.cell_area(p);
    let cap = |_: usize| 50.0;
    let lo = |_: usize| f64::NEG_INFINITY;
    let p = SelParams {
        keep: &keep,
        area: &area,
        max_rounds: 12,
        search: 1 << 20,
        cut_cap: &cap,
        fill_cap: &cap,
        cell_lo: &lo,
        switches: SelSwitches::default(),
    };
    let t = std::time::Instant::now();
    let ((zb, st), pk) = peak(|| drain_selective_floors_on(&g, &z0, &outlet, &p));
    row(
        &format!("base1 ({} depressions, {} breached, {} filled)", st.depressions, st.breached, st.filled),
        t.elapsed().as_secs_f64(),
        pk,
        model("base1"),
    );
    // K5 through GEN-143 (tile 384, 16 workers), and the largest catchments of its first round
    let cp = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let r0 =
        partitioned_route_weighted(&g, &zb, &outlet, cp.epsilon, 7, 384, RoundRules::ROUND, None, &mut PartCache::without_memory()).routing;
    let cuts = cut_in(Some(&g), &r0);
    let mut sizes: Vec<usize> = cuts.members.iter().map(Vec::len).collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let top16: usize = sizes.iter().take(16).sum();
    eprintln!(
        "G3-6 1/16: {} catchments, the largest {} cells ({:.1} % of land), the 16 largest {} ({:.1} %)",
        sizes.len(),
        sizes[0],
        100.0 * sizes[0] as f64 / land as f64,
        top16,
        100.0 * top16 as f64 / land as f64
    );
    drop((r0, cuts));
    let mut zs = zb.clone();
    let t = std::time::Instant::now();
    let (_, pk) =
        peak(|| solve_rounds_memo(&g, &mut zs, &outlet, &cp, 4, &16usize, 7, &Drivers::default(), 384, &mut RoundCache::without_memory()));
    row("K5 (solve_rounds_memo, tile 384, 16 workers)", t.elapsed().as_secs_f64(), pk, model("K5"));
    // G3-6 layout step 2: the catchments in flight capped at 200,000 cells (the same bits)
    if compact {
        let mut zc = zb.clone();
        let t = std::time::Instant::now();
        let (_, pk) = peak(|| {
            solve_rounds_memo(
                &g,
                &mut zc,
                &outlet,
                &cp,
                4,
                &16usize,
                7,
                &Drivers::default(),
                384,
                &mut RoundCache::without_memory().with_inflight_cap(200_000),
            )
        });
        assert!(zc.iter().zip(&zs).all(|(a, b)| a.to_bits() == b.to_bits()), "the capped K5 is K5");
        row("K5 with the in-flight cap at 200,000 cells (the same bits)", t.elapsed().as_secs_f64(), pk, model("K5"));
    }
    // K6: the weighted route, the flood, hydrology and graph (planet_cache::rg_h1, the chain's code)
    let t = std::time::Instant::now();
    let (h1, pk) = peak(|| {
        let w = vec![1.0f32; g.len()];
        let r =
            partitioned_route_weighted(&g, &zs, &outlet, cp.epsilon, 7, 384, RoundRules::ROUND, Some(&w), &mut PartCache::without_memory())
                .routing;
        let f = priority_flood_with(&g, &zs, &outlet, cp.epsilon, true);
        let runoff = vec![1.0e-8f32; g.len()];
        let evap = vec![0.0f32; g.len()];
        planet_cache::rg_h1::h1_from(&g, &zs, &f, r, &outlet, &runoff, &evap, Some(&[]), false)
    });
    let trunks = h1.trunk_cells.len();
    let recv_set = h1.routing.receiver.iter().filter(|&&r| r != NONE).count();
    drop(h1);
    row(&format!("K6 ({trunks} trunk reaches, {recv_set} routed cells)"), t.elapsed().as_secs_f64(), pk, model("K6"));
    if compact {
        return;
    }
    // one worker's L buffer (1/16 of 16): band6's drain on a 3,328² plane grid of the same relief's character
    let side = 3328;
    let gl = Grid::new(side, side, 101.8);
    let zl: Vec<f64> = (0..side * side)
        .map(|p| {
            let (x, y) = ((p % side) as f64 / side as f64, (p / side) as f64 / side as f64);
            -100.0 + 400.0 * libm::sin(9.0 * x + 1.0) * libm::cos(7.0 * y) + 60.0 * libm::sin(83.0 * x * y + 2.0)
        })
        .collect();
    let ol: Vec<bool> = zl.iter().map(|&h| h <= 0.0).collect();
    let areal = |_: usize| 101.8 * 101.8;
    let pl = SelParams {
        keep: &keep,
        area: &areal,
        max_rounds: 8,
        search: 4096,
        cut_cap: &cap,
        fill_cap: &cap,
        cell_lo: &lo,
        switches: SelSwitches { outlet_floors: true, ..SelSwitches::default() },
    };
    let t = std::time::Instant::now();
    let (_, pk) = peak(|| drain_selective_floors_on(&gl, &zl, &ol, &pl));
    let (hwm, rss) = rss_mib();
    eprintln!(
        "G3-6 1/16 | band6 on one L buffer ({side}², the drain alone) | {:.1} s | peak heap {:.2} GB = {:.1} B a cell | the model's L rows hold 177 B a cell for all stages | RSS peak {hwm} MiB, now {rss} MiB",
        t.elapsed().as_secs_f64(),
        pk as f64 / 1e9,
        pk as f64 / (side * side) as f64
    );
}
