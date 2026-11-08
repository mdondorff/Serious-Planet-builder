//! SPIKE-105 F6 (spike branch only, never merged): planet-wide coarse routing across cube edges, heights read from a stored bake.
//! `f6_cross_face_h0 STORE N THREADS` reads the C1 elevation at the N x N cell centres of all six faces from the store (read-only),
//! floods from the sea over ONE graph (neighbours across cube edges by directions), routes D8 and accumulates, and reports the catchments.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use planet_cache::global_store::{decode_block, GlobalWorld, KIND_GLOBAL_BLOCK};
use planet_generators::global::{GlobalSample, LatticeSource, ReadError};
use planet_core::TileId;
use std::sync::{Arc, Mutex};
use planet_cache::pack::TileStore;
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::Face;
use planet_generators::global::read_elevation_c1;

const NONE: u32 = u32::MAX;
const R: f64 = 6_371_000.0;

fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b & (1 << 63) != 0 { !b } else { b | (1 << 63) }
}

struct SendPtr(*mut f32);
unsafe impl Sync for SendPtr {}

struct G {
    n: usize,
}

impl G {
    /// Up to 8 neighbours of cell `p`; across a cube edge found through the direction of the extended face coordinates.
    #[inline]
    fn nb(&self, p: usize, out: &mut [u32; 8]) -> usize {
        let n = self.n;
        let nn = n * n;
        let f = p / nn;
        let r = p % nn;
        let (j, i) = (r / n, r % n);
        let mut c = 0;
        let interior = i > 0 && j > 0 && i + 1 < n && j + 1 < n;
        for dj in -1i64..=1 {
            for di in -1i64..=1 {
                if di == 0 && dj == 0 {
                    continue;
                }
                let (ii, jj) = (i as i64 + di, j as i64 + dj);
                let q = if interior || (ii >= 0 && jj >= 0 && ii < n as i64 && jj < n as i64) {
                    f * nn + jj as usize * n + ii as usize
                } else {
                    let s = 2.0 * (ii as f64 + 0.5) / n as f64 - 1.0;
                    let t = 2.0 * (jj as f64 + 0.5) / n as f64 - 1.0;
                    let d = face_to_direction(&TangentWarp, Face(f as u8), s, t);
                    let (f2, s2, t2) = direction_to_face(&TangentWarp, d);
                    let c2 = |x: f64| (((x + 1.0) * 0.5 * n as f64).floor() as i64).clamp(0, n as i64 - 1) as usize;
                    f2.0 as usize * nn + c2(t2) * n + c2(s2)
                };
                if q != p {
                    out[c] = q as u32;
                    c += 1;
                }
            }
        }
        c
    }
}


/// Key-agnostic read of the stored blocks: the store's own key of each block is taken from its index (`keys_for`), because the
/// worktree's code no longer derives the keys the store was baked under. Same layout and decode as `StoreSource`.
struct RawSource<'a> {
    store: &'a TileStore,
    res: u32,
    level: u8,
    cells: u32,
    cap: usize,
    lru: Mutex<Vec<(TileId, Arc<Vec<GlobalSample>>)>>,
    decodes: AtomicUsize,
}

impl LatticeSource for RawSource<'_> {
    fn res(&self) -> u32 {
        self.res
    }
    fn block_cells(&self) -> u32 {
        self.cells
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let side = 1u32 << self.level;
        let shift = self.cells.trailing_zeros();
        let (bx, by) = ((i >> shift).min(side - 1), (j >> shift).min(side - 1));
        let (k, l) = (i - bx * self.cells, j - by * self.cells);
        let tile = TileId::new(face, self.level, bx, by).unwrap();
        let hit = {
            let mut g = self.lru.lock().unwrap();
            g.iter().position(|(t, _)| *t == tile).map(|at| {
                let e = g.remove(at);
                let b = e.1.clone();
                g.push(e);
                b
            })
        };
        let block = match hit {
            Some(b) => b,
            None => {
                let key = *self.store.keys_for(KIND_GLOBAL_BLOCK, tile).first().ok_or_else(|| ReadError::Source(format!("no block {tile:?}")))?;
                let rec = self.store.get(KIND_GLOBAL_BLOCK, tile, key).map_err(|e| ReadError::Source(e.to_string()))?.ok_or_else(|| ReadError::Source("missing".into()))?;
                let (c, samples) = decode_block(&rec.payload).map_err(|e| ReadError::Source(e.to_string()))?;
                assert_eq!(c, self.cells);
                self.decodes.fetch_add(1, Ordering::Relaxed);
                let b = Arc::new(samples);
                let mut g = self.lru.lock().unwrap();
                if !g.iter().any(|(t, _)| *t == tile) {
                    g.push((tile, b.clone()));
                    if g.len() > self.cap {
                        g.remove(0);
                    }
                }
                b
            }
        };
        Ok(block[(l * (self.cells + 1) + k) as usize])
    }
}

fn peak_mib() -> u64 {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        rest: [usize; 7],
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: core::mem::size_of::<Counters>() as u32, page_fault_count: 0, peak_working_set: 0, rest: [0; 7] };
    // SAFETY: a writable counters struct of the size it declares.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok != 0 { (c.peak_working_set as u64) >> 20 } else { 0 }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let n: usize = a[2].parse().unwrap();
    let threads: usize = a[3].parse().unwrap();
    let t0 = Instant::now();
    let def = planet_world_def::WorldDefinition::single_planet(R, 14);
    let world = GlobalWorld::from_definition(&def).expect("world");
    let (level, cells) = world.layout(16384).expect("layout");
    let store = TileStore::open(&dir, world.params()).expect("store opens");
    println!("open store: {:.1} s; level {level}, block {cells}; packs {}, entries {}, problems {:?}", t0.elapsed().as_secs_f64(), store.pack_count(), store.len(), store.problems());
    let src = RawSource { store: &store, res: 16384, level, cells, cap: 40, lru: Mutex::new(Vec::new()), decodes: AtomicUsize::new(0) };
    let nn = n * n;
    let total = 6 * nn;
    let mut z = vec![0f32; total];
    let t_read = Instant::now();
    let mut face_secs = [0f64; 6];
    for f in 0..6 {
        let tf = Instant::now();
        let tilew = (n / 16).max(1);
        let tiles = n.div_ceil(tilew);
        let next = AtomicUsize::new(0);
        let zp = SendPtr(z[f * nn..(f + 1) * nn].as_mut_ptr());
        std::thread::scope(|sc| {
            for _ in 0..threads {
                sc.spawn(|| {
                    let zp = &zp;
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        if k >= tiles * tiles {
                            break;
                        }
                        let (ty, tx) = (k / tiles, k % tiles);
                        for j in ty * tilew..((ty + 1) * tilew).min(n) {
                            let t = 2.0 * (j as f64 + 0.5) / n as f64 - 1.0;
                            for i in tx * tilew..((tx + 1) * tilew).min(n) {
                                let s = 2.0 * (i as f64 + 0.5) / n as f64 - 1.0;
                                let d = face_to_direction(&TangentWarp, Face(f as u8), s, t);
                                let v = read_elevation_c1(&src, &TangentWarp, d).expect("read");
                                // SAFETY: each (i, j) is written by exactly one work item.
                                unsafe { *zp.0.add(j * n + i) = v };
                            }
                        }
                    }
                });
            }
        });
        face_secs[f] = tf.elapsed().as_secs_f64();
        println!("  face {f}: store read {:.1} s", face_secs[f]);
    }
    let t_read = t_read.elapsed().as_secs_f64();
    println!("store read total {t_read:.1} s ({} cells); block decodes {}, peak {} MiB", total, src.decodes.load(Ordering::Relaxed), peak_mib());
    assert!(z.iter().all(|v| v.is_finite()));
    let land = z.iter().filter(|v| **v > 0.0).count();
    println!("land cells {land} ({:.1} %)", 100.0 * land as f64 / total as f64);

    let g = G { n };
    let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
    let own = dx * dx;
    // Flood.
    let tfl = Instant::now();
    let mut filled = vec![f64::NAN; total];
    let mut rcv = vec![NONE; total];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let mut flat: VecDeque<u32> = VecDeque::new();
    let mut nbs = [0u32; 8];
    for p in 0..total {
        if z[p] <= 0.0 {
            filled[p] = z[p] as f64;
        }
    }
    for p in 0..total {
        if z[p] <= 0.0 {
            let c = g.nb(p, &mut nbs);
            if nbs[..c].iter().any(|&q| z[q as usize] > 0.0) {
                heap.push(Reverse((key(z[p] as f64), p as u32)));
            }
        }
    }
    loop {
        let p = if let Some(p) = flat.pop_front() {
            p as usize
        } else if let Some(Reverse((_, p))) = heap.pop() {
            p as usize
        } else {
            break;
        };
        let c = g.nb(p, &mut nbs);
        for k in 0..c {
            let q = nbs[k] as usize;
            if !filled[q].is_nan() {
                continue;
            }
            let lowest = filled[p] + 1e-3;
            rcv[q] = p as u32;
            if (z[q] as f64) <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = z[q] as f64;
                heap.push(Reverse((key(z[q] as f64), q as u32)));
            }
        }
    }
    drop(heap);
    let t_flood = tfl.elapsed().as_secs_f64();
    let unreached_flood = filled.iter().filter(|v| v.is_nan()).count();
    println!("flood: {t_flood:.1} s; cells never reached by the flood {unreached_flood}; peak {} MiB", peak_mib());

    // Receivers: steepest strictly-lower neighbour on the filled surface, ties by index; the flood parent otherwise.
    let tr = Instant::now();
    let mut parent_only = 0usize;
    for p in 0..total {
        if z[p] <= 0.0 {
            rcv[p] = NONE;
            continue;
        }
        let c = g.nb(p, &mut nbs);
        let (pi, pj) = ((p % nn) % n, (p % nn) / n);
        let pf = p / nn;
        let mut best = NONE;
        let mut best_s = 0.0f64;
        for k in 0..c {
            let q = nbs[k] as usize;
            let fq = filled[q];
            if fq < filled[p] {
                // diagonal if both coordinates differ on the same face; across an edge compare by position distance via index offsets
                let diag = if q / nn == pf {
                    let (qi, qj) = ((q % nn) % n, (q % nn) / n);
                    qi != pi && qj != pj
                } else {
                    false
                };
                let dist = if diag { core::f64::consts::SQRT_2 } else { 1.0 };
                let s = (filled[p] - fq) / dist;
                if s > best_s || (s == best_s && best != NONE && (q as u32) < best) {
                    best_s = s;
                    best = q as u32;
                }
            }
        }
        if best == NONE {
            parent_only += 1;
        } else {
            rcv[p] = best;
        }
    }
    let t_rcv = tr.elapsed().as_secs_f64();
    println!("receivers: {t_rcv:.1} s (cells kept on the flood parent: {parent_only})");

    // Accumulation by Kahn's order (leaves first).
    let ta = Instant::now();
    let mut indeg = vec![0u8; total];
    for p in 0..total {
        if rcv[p] != NONE {
            let r = rcv[p] as usize;
            indeg[r] = indeg[r].saturating_add(1);
        }
    }
    let mut area = vec![1u32; total];
    let mut order: Vec<u32> = Vec::with_capacity(total);
    for p in 0..total {
        if indeg[p] == 0 {
            order.push(p as u32);
        }
    }
    let mut head = 0;
    while head < order.len() {
        let p = order[head] as usize;
        head += 1;
        if rcv[p] != NONE {
            let r = rcv[p] as usize;
            area[r] += area[p];
            indeg[r] -= 1;
            if indeg[r] == 0 {
                order.push(r as u32);
            }
        }
    }
    let t_acc = ta.elapsed().as_secs_f64();
    println!("accumulation: {t_acc:.1} s; ordered {} of {total} (a cycle would leave cells out); peak {} MiB", order.len(), peak_mib());
    drop(indeg);

    // Outlet labels (outlets first = reverse order).
    let mut label = vec![NONE; total];
    for &p in order.iter().rev() {
        let p = p as usize;
        label[p] = if rcv[p] == NONE { p as u32 } else { label[rcv[p] as usize] };
    }
    let unreached = (0..total).filter(|&p| z[p] > 0.0 && (label[p] == NONE || z[label[p] as usize] > 0.0)).count();
    println!("land cells that do not reach a sea outlet: {unreached}");
    let mut mask = vec![0u8; total];
    for p in 0..total {
        if label[p] != NONE {
            mask[label[p] as usize] |= 1 << (p / nn);
        }
    }
    let mut list: Vec<(u32, u32, u8)> = Vec::new();
    for p in 0..total {
        if z[p] <= 0.0 && area[p] > 1 {
            list.push((area[p], p as u32, mask[p]));
        }
    }
    list.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
    let faces_of = |m: u8| (0..6).filter(|f| m & (1 << f) != 0).map(|f| f.to_string()).collect::<Vec<_>>().join(",");
    let cross = list.iter().filter(|e| e.2.count_ones() >= 2).count();
    let cross_land: u64 = list.iter().filter(|e| e.2.count_ones() >= 2).map(|e| u64::from(e.0 - 1)).sum();
    println!("catchments (sea outlet cells with >= 1 land donor): {}; spanning >= 2 faces: {cross} ({:.1} % of land cells)", list.len(), 100.0 * cross_land as f64 / land as f64);
    println!("dx {dx:.0} m (cell area dx^2 = {:.2} km2 used; true cell area varies about 2x across a face)", own / 1e6);
    for (i, e) in list.iter().take(10).enumerate() {
        println!("  #{}: {:.3e} km2, faces {}, outlet cell face {}", i + 1, f64::from(e.0 - 1) * own / 1e6, faces_of(e.2), e.1 as usize / nn);
    }
    // Largest cross-edge catchment connectivity.
    if let Some(top) = list.iter().find(|e| e.2.count_ones() >= 2) {
        let outlet = top.1;
        let members: Vec<u32> = (0..total as u32).filter(|&p| label[p as usize] == outlet && p != outlet).collect();
        let step = (members.len() / 20000).max(1);
        let (mut sampled, mut reach) = (0usize, 0usize);
        for &m in members.iter().step_by(step) {
            sampled += 1;
            let mut p = m as usize;
            let mut steps = 0usize;
            while rcv[p] != NONE && steps <= total {
                p = rcv[p] as usize;
                steps += 1;
            }
            if p as u32 == outlet {
                reach += 1;
            }
        }
        println!("largest cross-edge catchment: {:.3e} km2, faces {}, {} cells; sampled {sampled}, reached its one outlet {reach}", f64::from(top.0 - 1) * own / 1e6, faces_of(top.2), members.len());
        let mut per = [0usize; 6];
        for &m in &members {
            per[m as usize / nn] += 1;
        }
        println!("  cells per face {per:?}");
    }
    println!("total {:.1} s, peak memory {} MiB", t0.elapsed().as_secs_f64(), peak_mib());
}
