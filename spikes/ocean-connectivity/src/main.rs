//! SPIKE-08 harness: ocean connectivity over the stored 16384 bake (ADR 0041 point 2). Throwaway.
//!
//! `extract --store DIR --out FILE [--res 16384] [--threads N]`: read the bake's continentalness block by block (never the pack's
//! tile records) and write the sea mask (`continent <= SEA_LEVEL_F32`, one bit per vertex, six faces of (res+1)^2) to FILE.
//! `run --mask FILE [--threshold-km2 X] [--configs T:THREADS,...] [--serial yes|no] [--crop FILE.png]`: components by a serial
//! explicit-queue fill and by tiled union-find (min-id roots); hashes, cost, memory, area histogram, dry basins.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Instant;

use planet_cache::global_store::{decode_block, GlobalWorld, KIND_GLOBAL_BLOCK};
use planet_core::{Face, TileId};
use planet_generators::global::SEA_LEVEL_F32;
use planet_world_def::WorldDefinition;

// ---------------------------------------------------------------- heap accounting (live and peak bytes)

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(now, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(now, Relaxed);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            if new >= l.size() {
                let now = LIVE.fetch_add(new - l.size(), Relaxed) + new - l.size();
                PEAK.fetch_max(now, Relaxed);
            } else {
                LIVE.fetch_sub(l.size() - new, Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn reset_peak() {
    PEAK.store(LIVE.load(Relaxed), Relaxed);
}

fn mb(b: usize) -> f64 {
    b as f64 / 1e6
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

const NONE: u32 = u32::MAX;
const RADIUS_KM: f64 = 6371.0;
const AREA_SCALE: f64 = 1e5; // fixed point: 1 unit = 1e-5 km^2

type Bits = Vec<Vec<u64>>;

fn get(b: &[u64], idx: u64) -> bool {
    b[(idx >> 6) as usize] >> (idx & 63) & 1 == 1
}

fn fnv(h: &mut u64, x: u64) {
    *h = (*h ^ x).wrapping_mul(0x100_0000_01b3);
}

// ---------------------------------------------------------------- extract

fn extract() -> Result<(), String> {
    let dir = arg("--store").ok_or("--store DIR")?;
    let out = arg("--out").ok_or("--out FILE")?;
    let res: u32 = arg("--res").map(|s| s.parse().unwrap()).unwrap_or(16384);
    let threads: usize = arg("--threads").map(|s| s.parse().unwrap()).unwrap_or(8);
    let def = WorldDefinition::single_planet(6_371_000.0, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let store = planet_cache::pack::TileStore::open(std::path::Path::new(&dir), world.params()).map_err(|e| e.to_string())?;
    let (level, cells) = world.layout(res).map_err(|e| format!("{e:?}"))?;
    let side = 1u32 << level;
    let n = u64::from(res) + 1;
    let words = (n * n).div_ceil(64) as usize;
    let mask: Vec<Vec<AtomicU64>> = (0..6).map(|_| (0..words).map(|_| AtomicU64::new(0)).collect()).collect();
    let jobs: Vec<(u8, u32, u32)> = (0..6u8).flat_map(|f| (0..side).flat_map(move |by| (0..side).map(move |bx| (f, bx, by)))).collect();
    let next = AtomicUsize::new(0);
    let (sea_count, nan_count, missing) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    let (minc, maxc) = (AtomicU64::new(u64::MAX), AtomicU64::new(0));
    let t = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Relaxed);
                if k >= jobs.len() {
                    break;
                }
                let (f, bx, by) = jobs[k];
                let tile = TileId::new(Face(f), level, bx, by).expect("tile");
                let key = world.block_key(tile, cells);
                let Ok(Some(rec)) = store.get(KIND_GLOBAL_BLOCK, tile, key) else {
                    missing.fetch_add(1, Relaxed);
                    continue;
                };
                let (c, samples) = decode_block(&rec.payload).expect("decode");
                assert_eq!(c, cells);
                for l in 0..=cells {
                    // A vertex belongs to the block that starts at it; the far edge belongs to the last block.
                    if l == cells && by != side - 1 {
                        continue;
                    }
                    for kx in 0..=cells {
                        if kx == cells && bx != side - 1 {
                            continue;
                        }
                        let s = samples[(l * (cells + 1) + kx) as usize];
                        if s.continent.is_nan() {
                            nan_count.fetch_add(1, Relaxed);
                        }
                        minc.fetch_min(u64::from(s.continent.to_bits() ^ if s.continent < 0.0 { u32::MAX } else { 1 << 31 }), Relaxed);
                        maxc.fetch_max(u64::from(s.continent.to_bits() ^ if s.continent < 0.0 { u32::MAX } else { 1 << 31 }), Relaxed);
                        if s.continent <= SEA_LEVEL_F32 {
                            sea_count.fetch_add(1, Relaxed);
                            let idx = (u64::from(by * cells + l)) * n + u64::from(bx * cells + kx);
                            mask[f as usize][(idx >> 6) as usize].fetch_or(1 << (idx & 63), Relaxed);
                        }
                    }
                }
            });
        }
    });
    println!(
        "extract: {} blocks, {} missing, {} NaN, sea vertices {} of {} ({:.2} %), {:.1} s on {threads} threads, peak heap {:.0} MB",
        jobs.len(),
        missing.load(Relaxed),
        nan_count.load(Relaxed),
        sea_count.load(Relaxed),
        6 * n * n,
        100.0 * sea_count.load(Relaxed) as f64 / (6 * n * n) as f64,
        t.elapsed().as_secs_f64(),
        mb(PEAK.load(Relaxed))
    );
    if missing.load(Relaxed) > 0 {
        return Err("missing blocks".into());
    }
    let mut bytes = Vec::with_capacity(8 + 6 * words * 8);
    bytes.extend_from_slice(&u64::from(res).to_le_bytes());
    for f in &mask {
        for w in f {
            bytes.extend_from_slice(&w.load(Relaxed).to_le_bytes());
        }
    }
    std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}

fn load_mask(path: &str) -> Result<(u32, Bits), String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    let res = u64::from_le_bytes(b[0..8].try_into().unwrap()) as u32;
    let n = u64::from(res) + 1;
    let words = (n * n).div_ceil(64) as usize;
    let mut faces = Vec::new();
    for f in 0..6 {
        let at = 8 + f * words * 8;
        faces.push((0..words).map(|w| u64::from_le_bytes(b[at + w * 8..at + w * 8 + 8].try_into().unwrap())).collect());
    }
    Ok((res, faces))
}

// ---------------------------------------------------------------- the graph: vertices, shared nodes, areas

struct Ctx {
    res: u32,
    n: u64,
    /// Groups of raw vertex ids that are one node (face-edge copies; 2, or 3 at cube corners), each sorted ascending.
    groups: Vec<Vec<u32>>,
    group_of: HashMap<u32, u32>,
    /// Plane coordinate tan(s pi/4) per lattice index.
    xs: Vec<f64>,
}

impl Ctx {
    fn new(res: u32) -> Ctx {
        let n = u64::from(res) + 1;
        // Integer cube points identify the shared nodes: the same point on two faces is the same vertex.
        let mut by_point: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
        let r = i64::from(res);
        let round = |v: planet_core::Vec3| [v.x.round() as i64, v.y.round() as i64, v.z.round() as i64];
        for f in 0..6u8 {
            let (nv, uv, vv) = (round(Face(f).normal()), round(Face(f).u_axis()), round(Face(f).v_axis()));
            let mut visit = |i: i64, j: i64| {
                let p = [0, 1, 2].map(|k| nv[k] * r + (2 * i - r) * uv[k] + (2 * j - r) * vv[k]);
                let raw = (u64::from(f) * n * n + j as u64 * n + i as u64) as u32;
                by_point.entry(p).or_default().push(raw);
            };
            for k in 0..=r {
                visit(k, 0);
                visit(k, r);
                if k != 0 && k != r {
                    visit(0, k);
                    visit(r, k);
                }
            }
        }
        let mut groups: Vec<Vec<u32>> = by_point.into_values().filter(|g| g.len() > 1).collect();
        for g in &mut groups {
            g.sort_unstable();
            g.dedup();
        }
        groups.sort();
        let mut group_of = HashMap::new();
        for (gi, g) in groups.iter().enumerate() {
            for &r in g {
                group_of.insert(r, gi as u32);
            }
        }
        let xs = (0..=res).map(|i| (f64::from(i) / f64::from(res) * 2.0 - 1.0).mul_add(std::f64::consts::FRAC_PI_4, 0.0).tan()).collect();
        Ctx { res, n, groups, group_of, xs }
    }

    fn raw(&self, f: u8, i: u32, j: u32) -> u32 {
        (u64::from(f) * self.n * self.n + u64::from(j) * self.n + u64::from(i)) as u32
    }

    fn unraw(&self, r: u32) -> (u8, u32, u32) {
        let r = u64::from(r);
        let nn = self.n * self.n;
        let (f, rem) = (r / nn, r % nn);
        (f as u8, (rem % self.n) as u32, (rem / self.n) as u32)
    }

    fn is_edge(&self, i: u32, j: u32) -> bool {
        i == 0 || j == 0 || i == self.res || j == self.res
    }

    /// Counted once per shared node: the copy with the smallest raw id.
    fn canonical(&self, i: u32, j: u32, raw: u32) -> bool {
        !self.is_edge(i, j) || self.groups[self.group_of[&raw] as usize][0] == raw
    }

    /// Area of the vertex's dual cell on its face, in 1e-5 km^2 (a half cell on an edge, a quarter at a corner).
    fn weight(&self, i: u32, j: u32) -> u64 {
        let (x, y) = (self.xs[i as usize], self.xs[j as usize]);
        let q = std::f64::consts::FRAC_PI_4;
        let ds = 2.0 / f64::from(self.res);
        let d_omega = (q * (1.0 + x * x) * ds) * (q * (1.0 + y * y) * ds) / (1.0 + x * x + y * y).powf(1.5);
        let clip = |k: u32| if k == 0 || k == self.res { 0.5 } else { 1.0 };
        (RADIUS_KM * RADIUS_KM * d_omega * clip(i) * clip(j) * AREA_SCALE).round() as u64
    }
}

// ---------------------------------------------------------------- results

#[derive(Default, Clone)]
struct Comps {
    /// (label = smallest raw vertex id, nodes, area in 1e-5 km^2), sorted by label.
    list: Vec<(u32, u64, u64)>,
}

impl Comps {
    fn hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &(l, n, a) in &self.list {
            fnv(&mut h, u64::from(l));
            fnv(&mut h, n);
            fnv(&mut h, a);
        }
        h
    }
}

fn bits_hash(b: &Bits) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for f in b {
        for &w in f {
            fnv(&mut h, w);
        }
    }
    h
}

fn zero_bits(ctx: &Ctx) -> Bits {
    let words = (ctx.n * ctx.n).div_ceil(64) as usize;
    (0..6).map(|_| vec![0u64; words]).collect()
}

fn set(b: &mut [u64], idx: u64) {
    b[(idx >> 6) as usize] |= 1 << (idx & 63);
}

// ---------------------------------------------------------------- serial explicit-queue fill

fn serial(ctx: &Ctx, sea: &Bits) -> Comps {
    let mut visited = zero_bits(ctx);
    let n = ctx.n;
    let mut comps = Comps::default();
    let (mut cur, mut next): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
    for f in 0..6u8 {
        for idx in 0..n * n {
            if !get(&sea[f as usize], idx) || get(&visited[f as usize], idx) {
                continue;
            }
            let start = ctx.raw(f, (idx % n) as u32, (idx / n) as u32);
            set(&mut visited[f as usize], idx);
            cur.clear();
            cur.push(start);
            let (mut nodes, mut area) = (0u64, 0u64);
            while !cur.is_empty() {
                next.clear();
                for &r in &cur {
                    let (f, i, j) = ctx.unraw(r);
                    if ctx.canonical(i, j, r) {
                        nodes += 1;
                    }
                    area += ctx.weight(i, j);
                    let mut push = |f: u8, i: u32, j: u32| {
                        let idx = u64::from(j) * n + u64::from(i);
                        if get(&sea[f as usize], idx) && !get(&visited[f as usize], idx) {
                            set(&mut visited[f as usize], idx);
                            next.push(ctx.raw(f, i, j));
                        }
                    };
                    if i > 0 {
                        push(f, i - 1, j);
                    }
                    if i < ctx.res {
                        push(f, i + 1, j);
                    }
                    if j > 0 {
                        push(f, i, j - 1);
                    }
                    if j < ctx.res {
                        push(f, i, j + 1);
                    }
                    if ctx.is_edge(i, j) {
                        for &c in &ctx.groups[ctx.group_of[&r] as usize] {
                            if c != r {
                                let (cf, ci, cj) = ctx.unraw(c);
                                push(cf, ci, cj);
                            }
                        }
                    }
                }
                std::mem::swap(&mut cur, &mut next);
            }
            comps.list.push((start, nodes, area));
        }
    }
    comps.list.sort_unstable();
    comps
}

/// The ocean bitmap by flooding only the components at or above the threshold.
fn serial_ocean(ctx: &Ctx, sea: &Bits, comps: &Comps, thr: u64) -> Bits {
    let mut ocean = zero_bits(ctx);
    let n = ctx.n;
    let mut stack: Vec<u32> = Vec::new();
    for &(label, _, area) in comps.list.iter().filter(|c| c.2 >= thr) {
        let (f, i, j) = ctx.unraw(label);
        set(&mut ocean[f as usize], u64::from(j) * n + u64::from(i));
        stack.push(label);
        while let Some(r) = stack.pop() {
            let (f, i, j) = ctx.unraw(r);
            let mut push = |f: u8, i: u32, j: u32| {
                let idx = u64::from(j) * n + u64::from(i);
                if get(&sea[f as usize], idx) && !get(&ocean[f as usize], idx) {
                    set(&mut ocean[f as usize], idx);
                    stack.push(ctx.raw(f, i, j));
                }
            };
            if i > 0 {
                push(f, i - 1, j);
            }
            if i < ctx.res {
                push(f, i + 1, j);
            }
            if j > 0 {
                push(f, i, j - 1);
            }
            if j < ctx.res {
                push(f, i, j + 1);
            }
            if ctx.is_edge(i, j) {
                for &c in &ctx.groups[ctx.group_of[&r] as usize] {
                    if c != r {
                        let (cf, ci, cj) = ctx.unraw(c);
                        push(cf, ci, cj);
                    }
                }
            }
        }
        let _ = area;
    }
    ocean
}

// ---------------------------------------------------------------- tiled union-find

struct Tile {
    f: u8,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
}

#[derive(Default)]
struct TileOut {
    stats: Vec<(u32, u64, u64)>,
    top: Vec<u32>,
    bottom: Vec<u32>,
    left: Vec<u32>,
    right: Vec<u32>,
}

fn find(p: &mut [u32], mut a: u32) -> u32 {
    while p[a as usize] != a {
        let g = p[p[a as usize] as usize];
        p[a as usize] = g;
        a = g;
    }
    a
}

/// Connected components of the sea vertices of one tile (4-connectivity, in-tile edges only). Local roots are the smallest local
/// index, which is the smallest raw id. `labels[idx]` is the component's ordinal (in order of its smallest vertex) or NONE.
fn ccl_tile(ctx: &Ctx, sea: &[u64], t: &Tile, labels: &mut Vec<u32>, parent: &mut Vec<u32>) -> TileOut {
    let (w, h) = (t.w as usize, t.h as usize);
    parent.clear();
    parent.resize(w * h, NONE);
    let n = ctx.n;
    let is_sea = |lx: usize, ly: usize| get(sea, u64::from(t.y0 + ly as u32) * n + u64::from(t.x0 + lx as u32));
    for ly in 0..h {
        for lx in 0..w {
            if !is_sea(lx, ly) {
                continue;
            }
            let idx = (ly * w + lx) as u32;
            parent[idx as usize] = idx;
            if lx > 0 && parent[idx as usize - 1] != NONE {
                let (ra, rb) = (find(parent, idx), find(parent, idx - 1));
                if ra != rb {
                    parent[ra.max(rb) as usize] = ra.min(rb);
                }
            }
            if ly > 0 && parent[idx as usize - w] != NONE {
                let (ra, rb) = (find(parent, idx), find(parent, idx - w as u32));
                if ra != rb {
                    parent[ra.max(rb) as usize] = ra.min(rb);
                }
            }
        }
    }
    labels.clear();
    labels.resize(w * h, NONE);
    let mut out = TileOut::default();
    for ly in 0..h {
        for lx in 0..w {
            let idx = ly * w + lx;
            if parent[idx] == NONE {
                continue;
            }
            let root = find(parent, idx as u32) as usize;
            let (i, j) = (t.x0 + lx as u32, t.y0 + ly as u32);
            let raw = ctx.raw(t.f, i, j);
            if root == idx {
                labels[idx] = out.stats.len() as u32;
                out.stats.push((raw, 0, 0));
            } else {
                labels[idx] = labels[root];
            }
            let s = &mut out.stats[labels[idx] as usize];
            if ctx.canonical(i, j, raw) {
                s.1 += 1;
            }
            s.2 += ctx.weight(i, j);
        }
    }
    out.top = labels[0..w].to_vec();
    out.bottom = labels[(h - 1) * w..h * w].to_vec();
    out.left = (0..h).map(|y| labels[y * w]).collect();
    out.right = (0..h).map(|y| labels[y * w + w - 1]).collect();
    out
}

struct Tiled {
    comps: Comps,
    tiles: Vec<Tile>,
    tx: u32,
    base: Vec<u32>,
    /// Flattened root of every global component id, and whether that root's component is at or above the threshold.
    root_of: Vec<u32>,
    t_pass1: f64,
    t_merge: f64,
    violations: u64,
}

fn tile_list(ctx: &Ctx, t: u32) -> (Vec<Tile>, u32) {
    let tx = (ctx.n as u32).div_ceil(t);
    let mut v = Vec::new();
    for f in 0..6u8 {
        for ty in 0..tx {
            for x in 0..tx {
                let (x0, y0) = (x * t, ty * t);
                v.push(Tile { f, x0, y0, w: t.min(ctx.n as u32 - x0), h: t.min(ctx.n as u32 - y0) });
            }
        }
    }
    (v, tx)
}

fn tiled(ctx: &Ctx, sea: &Bits, t: u32, threads: usize) -> Tiled {
    let (tiles, tx) = tile_list(ctx, t);
    let t0 = Instant::now();
    let outs: Mutex<Vec<(usize, TileOut)>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let (mut labels, mut parent) = (Vec::new(), Vec::new());
                loop {
                    let k = next.fetch_add(1, Relaxed);
                    if k >= tiles.len() {
                        break;
                    }
                    let o = ccl_tile(ctx, &sea[tiles[k].f as usize], &tiles[k], &mut labels, &mut parent);
                    outs.lock().unwrap().push((k, o));
                }
            });
        }
    });
    let mut outs = outs.into_inner().unwrap();
    outs.sort_by_key(|o| o.0);
    let outs: Vec<TileOut> = outs.into_iter().map(|o| o.1).collect();
    let t_pass1 = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    // Global component ids in tile order.
    let mut base = Vec::with_capacity(tiles.len());
    let mut total = 0u32;
    for o in &outs {
        base.push(total);
        total += o.stats.len() as u32;
    }
    let mut parent: Vec<u32> = (0..total).collect();
    let mut gmin: Vec<u32> = outs.iter().flat_map(|o| o.stats.iter().map(|s| s.0)).collect();
    let mut union = |a: u32, b: u32, parent: &mut Vec<u32>, gmin: &mut Vec<u32>| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            // The root with the smaller minimum raw id stays root (min-id roots).
            let (keep, drop) = if gmin[ra as usize] < gmin[rb as usize] { (ra, rb) } else { (rb, ra) };
            parent[drop as usize] = keep;
        }
    };
    let tidx = |f: u8, x: u32, y: u32| (u32::from(f) * tx * tx + y * tx + x) as usize;
    for f in 0..6u8 {
        for y in 0..tx {
            for x in 0..tx {
                let a = tidx(f, x, y);
                if x + 1 < tx {
                    let b = tidx(f, x + 1, y);
                    for k in 0..outs[a].right.len() {
                        let (la, lb) = (outs[a].right[k], outs[b].left[k]);
                        if la != NONE && lb != NONE {
                            union(base[a] + la, base[b] + lb, &mut parent, &mut gmin);
                        }
                    }
                }
                if y + 1 < tx {
                    let b = tidx(f, x, y + 1);
                    for k in 0..outs[a].bottom.len() {
                        let (la, lb) = (outs[a].bottom[k], outs[b].top[k]);
                        if la != NONE && lb != NONE {
                            union(base[a] + la, base[b] + lb, &mut parent, &mut gmin);
                        }
                    }
                }
            }
        }
    }
    // Shared nodes across faces: the copies on a face edge are one node.
    let strip = |raw: u32| -> u32 {
        let (f, i, j) = ctx.unraw(raw);
        let (x, y) = (i / t, j / t);
        let ti = tidx(f, x, y);
        let (lx, ly) = ((i - x * t) as usize, (j - y * t) as usize);
        let o = &outs[ti];
        let l = if ly == 0 {
            o.top[lx]
        } else if ly == o.left.len() - 1 {
            o.bottom[lx]
        } else if lx == 0 {
            o.left[ly]
        } else {
            o.right[ly]
        };
        if l == NONE {
            NONE
        } else {
            base[ti] + l
        }
    };
    let mut violations = 0u64;
    for g in &ctx.groups {
        let ls: Vec<u32> = g.iter().map(|&r| strip(r)).collect();
        if ls.iter().any(|&l| l == NONE) {
            if ls.iter().any(|&l| l != NONE) {
                violations += 1;
            }
            continue;
        }
        for &l in &ls[1..] {
            union(ls[0], l, &mut parent, &mut gmin);
        }
    }
    // Final components.
    let mut root_of: Vec<u32> = (0..total).map(|c| find(&mut parent, c)).collect();
    let mut agg: HashMap<u32, (u64, u64)> = HashMap::new();
    let mut c = 0usize;
    let mut roots_seen = 0usize;
    for o in &outs {
        for s in &o.stats {
            let e = agg.entry(root_of[c]).or_insert((0, 0));
            e.0 += s.1;
            e.1 += s.2;
            c += 1;
        }
    }
    let mut list: Vec<(u32, u64, u64)> = agg.into_iter().map(|(r, (n, a))| (gmin[r as usize], n, a)).collect();
    list.sort_unstable();
    roots_seen += list.len();
    let _ = roots_seen;
    // Make root_of hold the dense index of the final component (for the ocean flag lookups of pass 2).
    let label_index: HashMap<u32, u32> = list.iter().enumerate().map(|(i, c)| (c.0, i as u32)).collect();
    for r in &mut root_of {
        *r = label_index[&gmin[*r as usize]];
    }
    let t_merge = t1.elapsed().as_secs_f64();
    Tiled { comps: Comps { list }, tiles, tx, base, root_of, t_pass1, t_merge, violations }
}

/// Second pass: re-label every tile, mark ocean vertices, collect dry basins one land vertex from an ocean vertex.
struct Pass2 {
    ocean: Bits,
    candidates: Vec<(u8, u32, u32, u8, u32, u64)>,
    basins_touching: HashSet<u32>,
    /// Per component: the fewest land vertices crossed by an axis-aligned walk (inside a tile, at most 256) from one of its
    /// vertices to an ocean vertex; u32::MAX if none. Only meaningful for components below the threshold.
    gaps: Vec<u32>,
}

fn pass2(ctx: &Ctx, sea: &Bits, td: &Tiled, thr: u64, threads: usize) -> Pass2 {
    let words = (ctx.n * ctx.n).div_ceil(64) as usize;
    let ocean: Vec<Vec<AtomicU64>> = (0..6).map(|_| (0..words).map(|_| AtomicU64::new(0)).collect()).collect();
    let is_ocean: Vec<bool> = td.comps.list.iter().map(|c| c.2 >= thr).collect();
    let next = AtomicUsize::new(0);
    let res: Mutex<(Vec<(u8, u32, u32, u8, u32, u64)>, HashSet<u32>)> = Mutex::new((Vec::new(), HashSet::new()));
    let gaps: Vec<AtomicU64> = (0..td.comps.list.len()).map(|_| AtomicU64::new(u64::from(u32::MAX))).collect();
    let n = ctx.n;
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let (mut labels, mut parent) = (Vec::new(), Vec::new());
                let (mut cands, mut touching) = (Vec::new(), HashSet::new());
                loop {
                    let k = next.fetch_add(1, Relaxed);
                    if k >= td.tiles.len() {
                        break;
                    }
                    let t = &td.tiles[k];
                    ccl_tile(ctx, &sea[t.f as usize], t, &mut labels, &mut parent);
                    let (w, h) = (t.w as usize, t.h as usize);
                    let comp = |idx: usize| -> u32 { td.root_of[(td.base[k] + labels[idx]) as usize] };
                    for ly in 0..h {
                        for lx in 0..w {
                            let idx = ly * w + lx;
                            if labels[idx] == NONE {
                                continue;
                            }
                            let c = comp(idx);
                            if !is_ocean[c as usize] {
                                for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                                    let (mut x, mut y) = (lx as i64 + dx, ly as i64 + dy);
                                    let mut gap = 0u32;
                                    while x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && gap <= 256 {
                                        let l = labels[y as usize * w + x as usize];
                                        if l != NONE {
                                            if gap > 0 && is_ocean[td.root_of[(td.base[k] + l) as usize] as usize] {
                                                gaps[c as usize].fetch_min(u64::from(gap), Relaxed);
                                            }
                                            break;
                                        }
                                        gap += 1;
                                        x += dx;
                                        y += dy;
                                    }
                                }
                            }
                            if is_ocean[c as usize] {
                                let g = u64::from(t.y0 + ly as u32) * n + u64::from(t.x0 + lx as u32);
                                ocean[t.f as usize][(g >> 6) as usize].fetch_or(1 << (g & 63), Relaxed);
                            }
                            for (dir, ok, mid, far) in [(0u8, lx + 2 < w, idx + 1, idx + 2), (1u8, ly + 2 < h, idx + w, idx + 2 * w)] {
                                if ok && labels[mid] == NONE && labels[far] != NONE {
                                    let c2 = comp(far);
                                    if c != c2 && is_ocean[c as usize] != is_ocean[c2 as usize] {
                                        let basin = if is_ocean[c as usize] { c2 } else { c };
                                        touching.insert(basin);
                                        cands.push((t.f, t.x0 + lx as u32, t.y0 + ly as u32, dir, basin, td.comps.list[basin as usize].1));
                                    }
                                }
                            }
                        }
                    }
                }
                let mut g = res.lock().unwrap();
                g.0.extend(cands);
                g.1.extend(touching);
            });
        }
    });
    let (mut candidates, basins_touching) = res.into_inner().unwrap();
    candidates.sort_unstable();
    Pass2 {
        ocean: ocean.into_iter().map(|f| f.into_iter().map(|w| w.load(Relaxed)).collect()).collect(),
        candidates,
        basins_touching,
        gaps: gaps.into_iter().map(|g| g.load(Relaxed) as u32).collect(),
    }
}

// ---------------------------------------------------------------- crop picture

fn crop_png(ctx: &Ctx, sea: &Bits, td: &Tiled, thr: u64, cand: (u8, u32, u32, u8), path: &str) {
    let (f, ci, cj, _) = cand;
    let t = td.tiles.iter().position(|t| t.f == f && ci >= t.x0 && ci < t.x0 + t.w && cj >= t.y0 && cj < t.y0 + t.h).unwrap();
    let tile = &td.tiles[t];
    let (mut labels, mut parent) = (Vec::new(), Vec::new());
    ccl_tile(ctx, &sea[f as usize], tile, &mut labels, &mut parent);
    let half = 48i64;
    let (w, h) = (tile.w as i64, tile.h as i64);
    let (lx, ly) = (i64::from(ci - tile.x0), i64::from(cj - tile.y0));
    let (x0, y0) = ((lx - half).clamp(0, (w - 2 * half).max(0)), (ly - half).clamp(0, (h - 2 * half).max(0)));
    let size = (2 * half).min(w).min(h);
    let scale = 8usize;
    let mut img = vec![0u8; (size as usize * scale) * (size as usize * scale) * 3];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y0 + y) * w + x0 + x) as usize;
            let col = if labels[idx] == NONE {
                [110, 150, 80]
            } else if td.comps.list[td.root_of[(td.base[t] + labels[idx]) as usize] as usize].2 >= thr {
                [30, 80, 200]
            } else {
                [235, 140, 30]
            };
            for dy in 0..scale {
                for dx in 0..scale {
                    let p = ((y as usize * scale + dy) * size as usize * scale + x as usize * scale + dx) * 3;
                    img[p..p + 3].copy_from_slice(&col);
                }
            }
        }
    }
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), (size as usize * scale) as u32, (size as usize * scale) as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
    println!("crop {path}: face {f} vertices x {}..{}, y {}..{} of the tile at ({}, {}), vertex = {scale} px", tile.x0 as i64 + x0, tile.x0 as i64 + x0 + size, tile.y0 as i64 + y0, tile.y0 as i64 + y0 + size, tile.x0, tile.y0);
}

// ---------------------------------------------------------------- run

fn km2(a: u64) -> f64 {
    a as f64 / AREA_SCALE
}

fn run() -> Result<(), String> {
    let path = arg("--mask").ok_or("--mask FILE")?;
    let thr_km2: f64 = arg("--threshold-km2").map(|s| s.parse().unwrap()).unwrap_or(1000.0);
    let thr = (thr_km2 * AREA_SCALE) as u64;
    let configs: Vec<(u32, usize)> = arg("--configs")
        .unwrap_or_else(|| "1024:20".into())
        .split(',')
        .map(|c| {
            let (t, n) = c.split_once(':').unwrap();
            (t.parse().unwrap(), n.parse().unwrap())
        })
        .collect();
    let do_serial = arg("--serial").as_deref() != Some("no");
    let (res, sea) = load_mask(&path)?;
    let t = Instant::now();
    let ctx = Ctx::new(res);
    let mask_mb = sea.iter().map(|f| f.len() * 8).sum::<usize>();
    println!(
        "mask: res {res}, {} shared-node groups ({} corner triples), mask {:.0} MB, built in {:.2} s",
        ctx.groups.len(),
        ctx.groups.iter().filter(|g| g.len() == 3).count(),
        mb(mask_mb),
        t.elapsed().as_secs_f64()
    );
    // Shared nodes must be sea on every copy or on none.
    let bad = ctx.groups.iter().filter(|g| g.iter().map(|&r| { let (f, i, j) = ctx.unraw(r); get(&sea[f as usize], u64::from(j) * ctx.n + u64::from(i)) }).collect::<HashSet<_>>().len() > 1).count();
    println!("shared nodes whose copies disagree on sea: {bad}");
    // Sanity: the dual-cell areas add up to the sphere.
    let one_face: u64 = (0..=res).map(|j| (0..=res).map(|i| ctx.weight(i, j)).sum::<u64>()).sum();
    println!("area check: 6 faces = {:.6e} km^2 vs 4 pi R^2 = {:.6e}", 6.0 * km2(one_face), 4.0 * std::f64::consts::PI * RADIUS_KM * RADIUS_KM);

    let mut reference: Option<(Comps, u64)> = None;
    if do_serial {
        reset_peak();
        let base = LIVE.load(Relaxed);
        let t = Instant::now();
        let c = serial(&ctx, &sea);
        let dt = t.elapsed().as_secs_f64();
        let peak = PEAK.load(Relaxed) - base;
        println!("SERIAL explicit-queue fill: {} components, {:.1} s, extra peak heap {:.0} MB, table hash {:016x}", c.list.len(), dt, mb(peak), c.hash());
        reference = Some((c.clone(), 0));
    }
    let mut first: Option<Tiled> = None;
    for (tile, threads) in configs {
        reset_peak();
        let base = LIVE.load(Relaxed);
        let td = tiled(&ctx, &sea, tile, threads);
        let peak = PEAK.load(Relaxed) - base;
        let h = td.comps.hash();
        let same = reference.as_ref().map(|(c, _)| if c.hash() == h && c.list == td.comps.list { "IDENTICAL to serial" } else { "DIFFERS from serial" }).unwrap_or("(no serial run)");
        println!(
            "TILED T={tile} threads={threads}: {} components, tile pass {:.1} s + merge {:.1} s, extra peak heap {:.0} MB, table hash {h:016x} {same}; shared-node sea violations {}",
            td.comps.list.len(),
            td.t_pass1,
            td.t_merge,
            mb(peak),
            td.violations
        );
        if first.is_none() {
            first = Some(td);
        }
    }
    let Some(td) = first else { return Ok(()) };

    // Area histogram.
    let list = &td.comps.list;
    let sea_nodes: u64 = list.iter().map(|c| c.1).sum();
    let sea_area: f64 = list.iter().map(|c| km2(c.2)).sum();
    println!("\nsea: {} nodes, {:.4e} km^2 ({:.1} % of the sphere), {} components", sea_nodes, sea_area, 100.0 * sea_area / (4.0 * std::f64::consts::PI * RADIUS_KM * RADIUS_KM), list.len());
    let mut by_area: Vec<&(u32, u64, u64)> = list.iter().collect();
    by_area.sort_by(|a, b| b.2.cmp(&a.2));
    println!("\nlargest components (label = smallest vertex id):");
    println!("| rank | label | face (i, j) | nodes | area km^2 |");
    println!("|---|---|---|---|---|");
    for (k, c) in by_area.iter().take(14).enumerate() {
        let (f, i, j) = ctx.unraw(c.0);
        println!("| {} | {} | {f} ({i}, {j}) | {} | {:.0} |", k + 1, c.0, c.1, km2(c.2));
    }
    println!("\nthreshold table (components by area):");
    println!("| threshold km^2 | components at or above | their area km^2 | components below | area below km^2 | largest below km^2 |");
    println!("|---|---|---|---|---|---|");
    for &th in &[0.5, 1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1e3, 3e3, 1e4, 3e4, 1e5, 1e6, 1e7] {
        let q = (th * AREA_SCALE) as u64;
        let (above, below): (Vec<&(u32, u64, u64)>, Vec<&(u32, u64, u64)>) = list.iter().partition(|c| c.2 >= q);
        println!(
            "| {th} | {} | {:.4e} | {} | {:.4e} | {:.0} |",
            above.len(),
            above.iter().map(|c| km2(c.2)).sum::<f64>(),
            below.len(),
            below.iter().map(|c| km2(c.2)).sum::<f64>(),
            below.iter().map(|c| km2(c.2)).fold(0.0, f64::max)
        );
    }
    println!("\nlog10 area histogram (km^2): bin = [10^k, 10^(k+1)), components:");
    let mut bins = [0usize; 16];
    for c in list {
        let a = km2(c.2).max(1e-3);
        bins[((a.log10() + 3.0).floor() as usize).min(15)] += 1;
    }
    for (k, b) in bins.iter().enumerate() {
        if *b > 0 {
            println!("  1e{}: {b}", k as i32 - 3);
        }
    }

    // Pass 2 with the chosen threshold.
    let threads = 20;
    reset_peak();
    let base = LIVE.load(Relaxed);
    let t = Instant::now();
    let p2 = pass2(&ctx, &sea, &td, thr, threads);
    println!(
        "\nPASS 2 (threshold {thr_km2} km^2, {threads} threads): ocean bitmap + dry-basin scan in {:.1} s, extra peak heap {:.0} MB",
        t.elapsed().as_secs_f64(),
        mb(PEAK.load(Relaxed) - base)
    );
    let ocean_hash = bits_hash(&p2.ocean);
    let ocean_nodes: u64 = td.comps.list.iter().filter(|c| c.2 >= thr).map(|c| c.1).sum();
    let ocean_comps = td.comps.list.iter().filter(|c| c.2 >= thr).count();
    println!("ocean: {ocean_comps} components, {ocean_nodes} nodes, bitmap hash {ocean_hash:016x}");
    if let Some((c, _)) = &reference {
        reset_peak();
        let base = LIVE.load(Relaxed);
        let t = Instant::now();
        let so = serial_ocean(&ctx, &sea, c, thr);
        let sh = bits_hash(&so);
        println!(
            "serial ocean bitmap: hash {sh:016x} {} ({:.1} s, extra peak heap {:.0} MB)",
            if sh == ocean_hash { "IDENTICAL to tiled" } else { "DIFFERS from tiled" },
            t.elapsed().as_secs_f64(),
            mb(PEAK.load(Relaxed) - base)
        );
    }
    println!(
        "\ndry basins one land vertex from an ocean vertex (axis pairs inside tiles): {} pairs, {} distinct basin components",
        p2.candidates.len(),
        p2.basins_touching.len()
    );
    println!("\ncomponents below the threshold (dry basins) and the fewest land vertices between one and the ocean (axis walks inside a tile, max 256):");
    println!("| label | face (i, j) | nodes | km^2 | min land gap (vertices) |");
    println!("|---|---|---|---|---|");
    for (ci, c) in td.comps.list.iter().enumerate().filter(|(_, c)| c.2 < thr) {
        let (f, i, j) = ctx.unraw(c.0);
        let g = p2.gaps[ci];
        println!("| {} | {f} ({i}, {j}) | {} | {:.0} | {} |", c.0, c.1, km2(c.2), if g == u32::MAX { "none within 256".into() } else { g.to_string() });
    }
    let mut best: Vec<_> = p2.candidates.clone();
    best.sort_by(|a, b| b.5.cmp(&a.5));
    println!("| face (i, j) | dir | basin label | basin nodes | basin km^2 |");
    println!("|---|---|---|---|---|");
    let mut shown = HashSet::new();
    for c in best.iter().filter(|c| shown.insert(c.4)).take(8) {
        let b = &td.comps.list[c.4 as usize];
        println!("| {} ({}, {}) | {} | {} | {} | {:.1} |", c.0, c.1, c.2, if c.3 == 0 { "x" } else { "y" }, b.0, b.1, km2(b.2));
    }
    if let (Some(path), Some(c)) = (arg("--crop"), best.first()) {
        crop_png(&ctx, &sea, &td, thr, (c.0, c.1, c.2, c.3), &path);
    }
    let _ = td.tx;
    Ok(())
}

/// Serial against tiled on random site-percolation masks (shared nodes agree), small resolutions, many tile sizes and thread counts.
fn selftest() -> Result<(), String> {
    for (res, p, seed) in [(64u32, 0.6f64, 1u64), (64, 0.55, 2), (100, 0.62, 3), (128, 0.5, 4), (37, 0.7, 5)] {
        let res = if res.is_power_of_two() || res == 100 || res == 37 { res } else { res };
        let ctx = Ctx::new(res);
        let mut sea = zero_bits(&ctx);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    let raw = ctx.raw(f, i, j);
                    let key = if ctx.is_edge(i, j) { ctx.groups[ctx.group_of[&raw] as usize][0] } else { raw };
                    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ seed;
                    for k in 0..4 {
                        fnv(&mut h, u64::from(key >> (8 * k) & 255));
                    }
                    h ^= h >> 29;
                    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
                    h ^= h >> 32;
                    if ((h >> 11) as f64 / (1u64 << 53) as f64) < p {
                        set(&mut sea[f as usize], u64::from(j) * ctx.n + u64::from(i));
                    }
                }
            }
        }
        let c = serial(&ctx, &sea);
        let big = c.list.iter().map(|x| x.2).max().unwrap_or(0);
        let thr = big / 4;
        let so = serial_ocean(&ctx, &sea, &c, thr);
        let mut ok = true;
        for (t, th) in [(7u32, 1usize), (7, 3), (16, 4), (res + 1, 2), (5, 8)] {
            let td = tiled(&ctx, &sea, t, th);
            let p2 = pass2(&ctx, &sea, &td, thr, th);
            let same = td.comps.list == c.list && td.violations == 0 && bits_hash(&p2.ocean) == bits_hash(&so);
            ok &= same;
            if !same {
                println!("MISMATCH res {res} T {t} threads {th}: comps {} vs {}", td.comps.list.len(), c.list.len());
            }
        }
        println!("selftest res {res} p {p}: {} components (largest {} nodes), serial == tiled for 5 tile/thread configs: {ok}", c.list.len(), c.list.iter().map(|x| x.1).max().unwrap_or(0));
        if !ok {
            return Err("selftest failed".into());
        }
    }
    fixture()
}

/// A dry basin one land vertex from the ocean (ADR 0041 point 2's known limit), hand-drawn on face 0 of a 32-cell grid.
fn fixture() -> Result<(), String> {
    let ctx = Ctx::new(32);
    let mut sea = zero_bits(&ctx);
    let mut put = |x: u32, y: u32| set(&mut sea[0], u64::from(y) * ctx.n + u64::from(x));
    for y in 2..=29 {
        for x in 2..=9 {
            put(x, y); // the ocean
        }
    }
    for y in 10..=20 {
        for x in 11..=14 {
            put(x, y); // the basin, behind a wall of land at x = 10, one vertex wide
        }
    }
    for y in 4..=6 {
        for x in 20..=22 {
            put(x, y); // a pond far from everything
        }
    }
    let c = serial(&ctx, &sea);
    let thr = c.list.iter().map(|x| x.2).max().unwrap() / 2;
    let so = serial_ocean(&ctx, &sea, &c, thr);
    let mut ok = c.list.len() == 3;
    for (t, th) in [(8u32, 1usize), (11, 3), (33, 2)] {
        let td = tiled(&ctx, &sea, t, th);
        let p2 = pass2(&ctx, &sea, &td, thr, th);
        let basin = td.comps.list.iter().position(|x| x.1 == 44).unwrap();
        let in_ocean = |x: u32, y: u32| get(&p2.ocean[0], u64::from(y) * ctx.n + u64::from(x));
        let pairs = p2.candidates.iter().filter(|c| c.3 == 0).count();
        let this = td.comps.list == c.list
            && bits_hash(&p2.ocean) == bits_hash(&so)
            && in_ocean(9, 15)
            && !in_ocean(11, 15)
            && !in_ocean(21, 5)
            && (t != 33 || (p2.gaps[basin] == 1 && pairs == 11));
        println!("fixture T {t} threads {th}: basin dry (ocean at x=9, not at x=11), min gap {}, one-cell pairs in x: {pairs}: {this}", p2.gaps[basin]);
        ok &= this;
    }
    if ok {
        Ok(())
    } else {
        Err("fixture failed".into())
    }
}

/// Component areas of in-memory bakes at a coarser resolution, for the stored world's seed and some others, to see how the
/// threshold depends on seed and resolution. `seeds --res 2048 --count 8`.
fn seeds() -> Result<(), String> {
    let res: u32 = arg("--res").map(|s| s.parse().unwrap()).unwrap_or(2048);
    let count: u64 = arg("--count").map(|s| s.parse().unwrap()).unwrap_or(8);
    let def = WorldDefinition::single_planet(6_371_000.0, 1);
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("{e}"))?;
    let ctx = Ctx::new(res);
    let mut list = vec![world.seed()];
    list.extend((1..count).map(|k| k.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x5eed));
    println!("| seed | sea % | components | largest km^2 | 2nd | 3rd | 4th | 5th | below 1e4 km^2 | below 1e5 | below 1e6 | tiny (<100 km^2) |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    for seed in list {
        let grid = planet_generators::global::bake(seed, &planet_core::cube::TangentWarp, res, 20).map_err(|e| format!("{e}"))?;
        let mut sea = zero_bits(&ctx);
        for f in 0..6u8 {
            for j in 0..=res {
                for i in 0..=res {
                    if grid.at(Face(f), i, j).continent <= SEA_LEVEL_F32 {
                        set(&mut sea[f as usize], u64::from(j) * ctx.n + u64::from(i));
                    }
                }
            }
        }
        let bad = ctx.groups.iter().filter(|g| g.iter().map(|&r| { let (f, i, j) = ctx.unraw(r); get(&sea[f as usize], u64::from(j) * ctx.n + u64::from(i)) }).collect::<HashSet<_>>().len() > 1).count();
        let td = tiled(&ctx, &sea, 256, 20);
        let mut areas: Vec<f64> = td.comps.list.iter().map(|c| km2(c.2)).collect();
        areas.sort_by(|a, b| b.total_cmp(a));
        let total: f64 = areas.iter().sum();
        let top = |k: usize| areas.get(k).map_or("-".into(), |a| format!("{a:.0}"));
        let below = |t: f64| areas.iter().filter(|&&a| a < t).count();
        println!(
            "| {seed:#x}{} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            if bad > 0 { " (edge mismatch!)" } else { "" },
            100.0 * total / (4.0 * std::f64::consts::PI * RADIUS_KM * RADIUS_KM),
            areas.len(),
            top(0),
            top(1),
            top(2),
            top(3),
            top(4),
            below(1e4),
            below(1e5),
            below(1e6),
            below(100.0)
        );
    }
    Ok(())
}

fn main() {
    let r = match std::env::args().nth(1).as_deref() {
        Some("seeds") => seeds(),
        Some("selftest") => selftest(),
        Some("extract") => extract(),
        Some("run") => run(),
        _ => Err("usage: extract | run".into()),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
