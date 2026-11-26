//! Spike only (crusader, G3-6 / R13): wave 1 at 1/16 of the largest landmass, each stage's peak heap measured by a counting allocator (this binary's global allocator) and the process's peak RSS,
//! against the byte model (`tests/mem_model.rs`). `cargo run --release -p planet-generators --example mem_stress -- full|compact`. Lock-free: no store, no window run.

#[path = "../tests/mem_model.rs"]
mod mem_model;
use mem_model::*;

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

/// `full`: wave 1 at 1/16 on today's full cube grid; `compact`: on the compact grid of the land and its coastal sea (GEN-147, layout step 1), K5 also with its in-flight cap (step 2a).
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("full") => wave_1_measured(false),
        Some("compact") => wave_1_measured(true),
        Some("lean") => wave_1_lean(std::env::args().nth(2).as_deref().unwrap_or(""), std::env::args().nth(3).as_deref().unwrap_or("8160")),
        Some("real") => {
            k6_on_a_real_window(std::env::args().nth(2).as_deref().unwrap_or(""), std::env::args().nth(3).as_deref().unwrap_or("8160"))
        }
        _ => eprintln!("mem_stress full | compact"),
    }
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
    // G3-6 layout step 3: K6's parts, each peak measured on its own with the others' results held (which part holds the bytes)
    if compact {
        use planet_generators::bake::channel_graph::ChannelGraph;
        use planet_generators::bake::hydro::{hydrology, HydroParams};
        let w = vec![1.0f32; g.len()];
        let (r, pr) = peak(|| {
            partitioned_route_weighted(&g, &zs, &outlet, cp.epsilon, 7, 384, RoundRules::ROUND, Some(&w), &mut PartCache::without_memory())
                .routing
        });
        let (f, pf) = peak(|| priority_flood_with(&g, &zs, &outlet, cp.epsilon, true));
        let runoff = vec![1.0e-8f32; g.len()];
        let zeros = vec![0.0f32; g.len()];
        let (hy, ph) = peak(|| hydrology(&g, &zs, &f, &r, &outlet, Some(&runoff), Some(&zeros), &HydroParams::default()));
        let channel = hy.channel.iter().filter(|&&c| c).count();
        let (gr, pg) = peak(|| ChannelGraph::from_hydro(&g, &r, &hy, &zs));
        let mut segs = 0usize;
        gr.for_each(|_| segs += 1);
        let n = g.len() as f64;
        eprintln!(
            "G3-6 1/16 | K6 parts, peak heap a cell: weighted route {:.1} B, flood {:.1} B, hydrology {:.1} B ({} basins, {channel} channel cells = {:.1} %), graph {:.1} B ({segs} segments)",
            pr as f64 / n,
            pf as f64 / n,
            ph as f64 / n,
            hy.basins.len(),
            100.0 * channel as f64 / land as f64,
            pg as f64 / n
        );
    }
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

/// G3-6 layout step 3: K6's parts on a real window's S0 (an `exit-s0` dump of 8,160² L cells), downsampled to H1 by 8 x 8 means, outlets the sea and the window edge
/// (`Grid::outlets`), today's hydrology defaults (initiation 75 km²): the channel share, the segments and each part's peak heap a cell.
fn k6_on_a_real_window(path: &str, side: &str) {
    use planet_generators::bake::carve::CarveParams;
    use planet_generators::bake::channel_graph::ChannelGraph;
    use planet_generators::bake::flood::priority_flood_with;
    use planet_generators::bake::hydro::{hydrology, HydroParams};
    use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
    use planet_generators::bake::route::RoundRules;
    use planet_generators::bake::Grid;
    let lw: usize = side.parse().expect("the dump's side");
    // read 8 L rows at a time, so the dump does not raise the process's peak RSS (the gate judges RSS)
    use std::io::Read;
    let mut file = std::fs::File::open(path).expect("an exit-s0 dump");
    assert_eq!(file.metadata().unwrap().len() as usize, lw * lw * 8, "{path}: {lw}² f64");
    let w = lw / 8;
    let mut z = vec![0.0f64; w * w];
    let mut band = vec![0u8; 8 * lw * 8];
    for y in 0..w {
        file.read_exact(&mut band).expect("a band of 8 rows");
        let l = |r: usize, x: usize| f64::from_le_bytes(band[8 * (r * lw + x)..8 * (r * lw + x) + 8].try_into().unwrap());
        for x in 0..w {
            let mut sum = 0.0;
            for dy in 0..8 {
                for dx in 0..8 {
                    sum += l(dy, 8 * x + dx);
                }
            }
            z[y * w + x] = sum / 64.0;
        }
    }
    drop(band);
    let g = Grid::new(w, w, 814.0);
    let outlet = g.outlets(&z, 0.0);
    let land = outlet.iter().filter(|&&o| !o).count();
    let eps = CarveParams::default().epsilon;
    let tile = (1..=w).rev().find(|t| w.is_multiple_of(*t) && *t <= 384).unwrap();
    // wave 1 in its order: base1 (Drain::SELECTIVE_50), K5 (4 rounds of 5 steps, GEN-143 at the tile; uncapped, then capped at 6 % of the land), then K6's parts on K5's output
    use planet_generators::bake::drain::{drain_selective_floors_on, SelParams, SelSwitches};
    use planet_generators::bake::round::{solve_rounds_memo, Drivers, RoundCache};
    let keep = |_: usize| false;
    let area = |p: usize| g.cell_area(p);
    let cap = |_: usize| 50.0;
    let lo = |_: usize| f64::NEG_INFINITY;
    let sp = SelParams {
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
    let ((zb, st), pb) = peak(|| drain_selective_floors_on(&g, &z, &outlet, &sp));
    let tb = t.elapsed().as_secs_f64();
    let cp = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let mut zs = zb.clone();
    let t = std::time::Instant::now();
    let (_, pk) =
        peak(|| solve_rounds_memo(&g, &mut zs, &outlet, &cp, 4, &16usize, 7, &Drivers::default(), tile, &mut RoundCache::without_memory()));
    let tk = t.elapsed().as_secs_f64();
    let mut zc = zb.clone();
    let capc = (land * 6 / 100).max(1);
    let (_, pkc) = peak(|| {
        solve_rounds_memo(
            &g,
            &mut zc,
            &outlet,
            &cp,
            4,
            &16usize,
            7,
            &Drivers::default(),
            tile,
            &mut RoundCache::without_memory().with_inflight_cap(capc),
        )
    });
    assert!(zc.iter().zip(&zs).all(|(a, b)| a.to_bits() == b.to_bits()), "the capped K5 is K5");
    let n = g.len() as f64;
    eprintln!(
        "G3-6 real {path}: base1 {} depressions ({} breached, {} filled) {tb:.0} s, peak {:.1} B a cell; K5 {tk:.0} s, peak {:.1} B a cell, capped at {capc} cells {:.1} B a cell (the same bits)",
        st.depressions,
        st.breached,
        st.filled,
        pb as f64 / n,
        pk as f64 / n,
        pkc as f64 / n
    );
    let z = zs;
    // G3-6 step 3 (a): the partitioned route's filled surface against priority_flood_with's canonical flood, bit for bit (weighted and unweighted)
    {
        let wt = vec![1.0f32; g.len()];
        let pf = partitioned_route_weighted(&g, &z, &outlet, eps, 7, tile, RoundRules::ROUND, Some(&wt), &mut PartCache::without_memory())
            .filled;
        let ff = priority_flood_with(&g, &z, &outlet, eps, true);
        let differ = pf.iter().zip(&ff).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        eprintln!("G3-6 real {path}: (a) the route's filled against the canonical flood: {differ} cells differ of {}", ff.len());
    }
    // K6 whole, as the chain runs it (h1_from with its inputs allocated inside the one peak): the stage's true peak, the parts below being measured each with the earlier ones held
    let (h1w, pw) = peak(|| {
        let wt = vec![1.0f32; g.len()];
        // (a): the route's filled surface is the canonical flood bit for bit, so hydrology takes it (no second flood)
        let pr = partitioned_route_weighted(&g, &z, &outlet, eps, 7, tile, RoundRules::ROUND, Some(&wt), &mut PartCache::without_memory());
        drop(wt);
        let (r, f) = (pr.routing, pr.filled);
        let runoff = vec![1.0e-8f32; g.len()];
        // (d) no evaporation field, (f) the filled buffer becomes d8
        planet_cache::rg_h1::h1_from_owned(&g, &z, f, r, &outlet, &runoff, &[], Some(&[]), false)
    });
    let (hwm, _) = rss_mib();
    eprintln!(
        "G3-6 real {path}: K6 whole with (a)-(f), h1_from and its inputs: peak heap {:.1} B a cell, {} trunk reaches; process peak RSS so far {hwm} MiB ({:.1} B a cell)",
        pw as f64 / g.len() as f64,
        h1w.trunk_cells.len(),
        (hwm as f64 * 1048576.0) / g.len() as f64
    );
    drop(h1w);
    let wt = vec![1.0f32; g.len()];
    let (r, pr) = peak(|| {
        partitioned_route_weighted(&g, &z, &outlet, eps, 7, tile, RoundRules::ROUND, Some(&wt), &mut PartCache::without_memory()).routing
    });
    let (f, pf) = peak(|| priority_flood_with(&g, &z, &outlet, eps, true));
    let runoff = vec![1.0e-8f32; g.len()];
    let zeros = vec![0.0f32; g.len()];
    let (hy, ph) = peak(|| hydrology(&g, &z, &f, &r, &outlet, Some(&runoff), Some(&zeros), &HydroParams::default()));
    let channel = hy.channel.iter().filter(|&&c| c).count();
    let (gr, pg) = peak(|| ChannelGraph::from_hydro(&g, &r, &hy, &z));
    let mut segs = 0usize;
    gr.for_each(|_| segs += 1);
    let (hwm, _) = rss_mib();
    eprintln!(
        "G3-6 real {path}: H1 {w}², {land} land; channel {channel} cells = {:.2} % of land, {} segments ({segs} in the graph), {} basins; peak heap a cell: route {:.1} B, flood {:.1} B, hydrology {:.1} B, graph {:.1} B; peak RSS {hwm} MiB",
        100.0 * channel as f64 / land as f64,
        hy.segments.len(),
        hy.basins.len(),
        pr as f64 / n,
        pf as f64 / n,
        ph as f64 / n,
        pg as f64 / n
    );
}

/// An exit-s0 dump of `lw`² L cells as H1 by 8 x 8 means, read 8 L rows at a time (the dump does not raise the peak RSS).
fn h1_of_dump(path: &str, lw: usize) -> Vec<f64> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).expect("an exit-s0 dump");
    assert_eq!(file.metadata().unwrap().len() as usize, lw * lw * 8, "{path}: {lw}² f64");
    let w = lw / 8;
    let mut z = vec![0.0f64; w * w];
    let mut band = vec![0u8; 8 * lw * 8];
    for y in 0..w {
        file.read_exact(&mut band).expect("a band of 8 rows");
        let l = |r: usize, x: usize| f64::from_le_bytes(band[8 * (r * lw + x)..8 * (r * lw + x) + 8].try_into().unwrap());
        for x in 0..w {
            let mut sum = 0.0;
            for dy in 0..8 {
                for dx in 0..8 {
                    sum += l(dy, 8 * x + dx);
                }
            }
            z[y * w + x] = sum / 64.0;
        }
    }
    z
}

/// G3-6 (the lead, 2026-11-26: the gate judges RSS, with K5 and K6 in the same process as wave 1 runs them): wave 1 lean in a fresh process, as a build would run it:
/// base1 (its input freed after), K5 with the in-flight cap at 6 % of the land in place, then K6 with levers (a)-(c). Prints each stage's peak heap and, at the end, the
/// process's peak RSS, all a cell; the landmass figure is the figure x 1.266e8 (the compact grid holds the land and its coastal ring).
fn wave_1_lean(path: &str, side: &str) {
    use planet_generators::bake::carve::CarveParams;
    use planet_generators::bake::drain::{drain_selective_floors_on, SelParams, SelSwitches};
    use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
    use planet_generators::bake::round::{solve_rounds_memo, Drivers, RoundCache};
    use planet_generators::bake::route::RoundRules;
    use planet_generators::bake::Grid;
    let lw: usize = side.parse().expect("the dump's side");
    // the process's fixed RSS (binary, runtime, the workers' stacks once started): it does not grow with the landmass, so the per-cell figure counts only what lies above it
    let warm: usize = (0..16usize).map(|i| std::thread::spawn(move || i).join().unwrap()).sum();
    let (base, _) = rss_mib();
    let z0 = h1_of_dump(path, lw);
    let w = lw / 8;
    let g = Grid::new(w, w, 814.0);
    let outlet = g.outlets(&z0, 0.0);
    let land = outlet.iter().filter(|&&o| !o).count();
    let n = g.len() as f64;
    let tile = (1..=w).rev().find(|t| w.is_multiple_of(*t) && *t <= 384).unwrap();
    let keep = |_: usize| false;
    let area = |p: usize| g.cell_area(p);
    let cap = |_: usize| 50.0;
    let lo = |_: usize| f64::NEG_INFINITY;
    let sp = SelParams {
        keep: &keep,
        area: &area,
        max_rounds: 12,
        search: 1 << 20,
        cut_cap: &cap,
        fill_cap: &cap,
        cell_lo: &lo,
        switches: SelSwitches::default(),
    };
    let ((mut z, _), pb) = peak(|| drain_selective_floors_on(&g, &z0, &outlet, &sp));
    drop(z0);
    let cp = CarveParams {
        steps: 20,
        reroute_every: 5,
        round_rules: RoundRules::ROUND,
        cell_metric: true,
        window_cut: true,
        ..CarveParams::default()
    };
    let capc = (land * 6 / 100).max(1);
    let (_, pk) = peak(|| {
        solve_rounds_memo(
            &g,
            &mut z,
            &outlet,
            &cp,
            4,
            &16usize,
            7,
            &Drivers::default(),
            tile,
            &mut RoundCache::without_memory().with_inflight_cap(capc),
        )
    });
    let (h1, p6) = peak(|| {
        let wt = vec![1.0f32; g.len()];
        let pr = partitioned_route_weighted(
            &g,
            &z,
            &outlet,
            cp.epsilon,
            7,
            tile,
            RoundRules::ROUND,
            Some(&wt),
            &mut PartCache::without_memory(),
        );
        drop(wt);
        let (r, f) = (pr.routing, pr.filled);
        let runoff = vec![1.0e-8f32; g.len()];
        // (d) no evaporation field, (f) the filled buffer becomes d8
        planet_cache::rg_h1::h1_from_owned(&g, &z, f, r, &outlet, &runoff, &[], Some(&[]), false)
    });
    let trunks = h1.trunk_cells.len();
    drop(h1);
    let (hwm, _) = rss_mib();
    let rss = (hwm - base) as f64 * 1048576.0 / n;
    eprintln!(
        "G3-6 lean {path}: H1 {w}², {land} land; peak heap a cell above the held surface (z, outlets: 9 B): base1 {:.1} B, K5 capped {:.1} B, K6 (a)-(f) {:.1} B ({trunks} trunks); process peak RSS {hwm} MiB, fixed {base} MiB ({warm} warm), the rest {rss:.1} B a cell; at the landmass {:.1} GB plus the fixed part",
        pb as f64 / n,
        pk as f64 / n,
        p6 as f64 / n,
        rss * 1.266e8 / 1e9
    );
}
