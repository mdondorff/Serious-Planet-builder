//! SPIKE-105 F8 (spike branch only, never merged): memory and time of one resident H1 routing of the planet's largest landmass at the regional grid.
//! `f8_landmass_route CACHE_DIR N_COARSE N_FINE THREADS [MAX_EVAL_SECS]`
//! CACHE_DIR holds base_{N_COARSE}.f32 (BakeInputs::base of seed 14 at the coarse grid). Steps:
//!  1. largest 8-connected land component (z > 0) at N_COARSE over the six faces (cross-edge neighbours by direction);
//!  2. fine cells (N_FINE per face edge) whose coarse parent is in the component or next to it get a base height (BakeInputs::base, or, when the
//!     measured evaluation time exceeds MAX_EVAL_SECS, a bilinear upsample of the coarse base plus 30 m of hash noise); sea cells not adjacent to land are dropped;
//!  3. one compact cell index (per-face bitmap + rank), priority flood from the sea (eps 1e-3), D8 receivers, MFD accumulation highest first.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::inputs::BakeInputs;

const NONE: u32 = u32::MAX;
const R: f64 = 6_371_000.0;
const LIMIT_MIB: u64 = 11_500;

fn key(v: f64) -> u64 {
    let b = v.to_bits();
    if b & (1 << 63) != 0 { !b } else { b | (1 << 63) }
}

#[repr(C)]
struct Counters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set: usize,
    working_set: usize,
    rest: [usize; 6],
}
fn mem() -> (u64, u64) {
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: core::mem::size_of::<Counters>() as u32, page_fault_count: 0, peak_working_set: 0, working_set: 0, rest: [0; 6] };
    // SAFETY: a writable counters struct of the size it declares.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok != 0 { ((c.working_set as u64) >> 20, (c.peak_working_set as u64) >> 20) } else { (0, 0) }
}
fn peak_mib() -> u64 {
    mem().1
}
fn watchdog() {
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let (ws, pk) = mem();
        if ws > LIMIT_MIB || pk > LIMIT_MIB + 300 {
            eprintln!("ABORT: working set {ws} MiB, peak {pk} MiB above the {LIMIT_MIB} MiB limit");
            std::process::exit(3);
        }
    });
}

/// Cube-edge-aware 8-neighbour of a face cell as a global id (face * n^2 + j * n + i); `diag` = both offsets non-zero.
fn nb_global(n: usize, p: usize, out: &mut [(u32, bool); 8]) -> usize {
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
                out[c] = (q as u32, di != 0 && dj != 0);
                c += 1;
            }
        }
    }
    c
}

fn dir(n: usize, f: usize, i: usize, j: usize) -> Vec3 {
    let s = 2.0 * (i as f64 + 0.5) / n as f64 - 1.0;
    let t = 2.0 * (j as f64 + 0.5) / n as f64 - 1.0;
    face_to_direction(&TangentWarp, Face(f as u8), s, t)
}

fn tri(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let num = a.dot(b.cross(c)).abs();
    let den = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
    2.0 * num.atan2(den)
}
fn cell_area(f: usize, n: usize, i: usize, j: usize) -> f64 {
    let d = |a: usize, b: usize| face_to_direction(&TangentWarp, Face(f as u8), 2.0 * a as f64 / n as f64 - 1.0, 2.0 * b as f64 / n as f64 - 1.0);
    let (p00, p10, p11, p01) = (d(i, j), d(i + 1, j), d(i + 1, j + 1), d(i, j + 1));
    (tri(p00, p10, p11) + tri(p00, p11, p01)) * R * R
}

/// Compact index over the landmass cells: global bitmap plus a per-word rank.
struct Idx {
    n: usize,
    bits: Vec<u64>,
    rank: Vec<u32>,
}
impl Idx {
    fn new(n: usize, bits: Vec<u64>) -> Idx {
        let mut rank = Vec::with_capacity(bits.len());
        let mut acc = 0u32;
        for w in &bits {
            rank.push(acc);
            acc += w.count_ones();
        }
        Idx { n, bits, rank }
    }
    #[inline]
    fn get(&self, q: usize) -> Option<u32> {
        let w = self.bits[q >> 6];
        let b = q & 63;
        if (w >> b) & 1 == 0 {
            None
        } else {
            Some(self.rank[q >> 6] + (w & ((1u64 << b) - 1)).count_ones())
        }
    }
    /// Compact neighbours of global cell `p` with their diagonal flags.
    #[inline]
    fn neigh(&self, p: usize, out: &mut [(u32, bool); 8]) -> usize {
        let mut raw = [(0u32, false); 8];
        let c = nb_global(self.n, p, &mut raw);
        let mut m = 0;
        for &(q, d) in &raw[..c] {
            if let Some(k) = self.get(q as usize) {
                out[m] = (k, d);
                m += 1;
            }
        }
        m
    }
}

fn hash_noise(f: usize, i: usize, j: usize) -> f64 {
    let mut x = (f as u64) << 56 ^ (i as u64) << 28 ^ j as u64 ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^= x >> 33;
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

fn main() {
    watchdog();
    let a: Vec<String> = std::env::args().collect();
    let cache = std::path::PathBuf::from(&a[1]);
    let nc: usize = a[2].parse().unwrap();
    let n: usize = a[3].parse().unwrap();
    let threads: usize = a[4].parse().unwrap();
    let max_eval: f64 = a.get(5).map(|s| s.parse().unwrap()).unwrap_or(1200.0);
    let t0 = Instant::now();
    let ratio = n / nc;
    assert!(ratio * nc == n && n % 64 == 0);
    let ncc = nc * nc;
    let nn = n * n;

    // ---- 1. coarse base and largest land component
    let bytes = std::fs::read(cache.join(format!("base_{nc}.f32"))).expect("coarse base cache");
    assert_eq!(bytes.len(), 6 * ncc * 4);
    let zc: Vec<f32> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    drop(bytes);
    let mut label = vec![0u32; 6 * ncc];
    let mut sizes: Vec<u32> = vec![0];
    let mut queue: Vec<u32> = Vec::new();
    let mut nbs = [(0u32, false); 8];
    for s in 0..6 * ncc {
        if zc[s] > 0.0 && label[s] == 0 {
            let id = sizes.len() as u32;
            let mut cnt = 0u32;
            label[s] = id;
            queue.clear();
            queue.push(s as u32);
            let mut h = 0;
            while h < queue.len() {
                let p = queue[h] as usize;
                h += 1;
                cnt += 1;
                let c = nb_global(nc, p, &mut nbs);
                for &(q, _) in &nbs[..c] {
                    let q = q as usize;
                    if zc[q] > 0.0 && label[q] == 0 {
                        label[q] = id;
                        queue.push(q as u32);
                    }
                }
            }
            sizes.push(cnt);
        }
    }
    let total_land: u64 = sizes.iter().map(|&s| u64::from(s)).sum();
    let (big, &big_n) = sizes.iter().enumerate().skip(1).max_by_key(|(_, s)| **s).unwrap();
    let mut order: Vec<u32> = sizes.iter().copied().skip(1).collect();
    order.sort_unstable_by(|x, y| y.cmp(x));
    let dxc = R * core::f64::consts::FRAC_PI_2 / nc as f64;
    let mut true_area = 0.0;
    let mut per_face = [0u64; 6];
    for s in 0..6 * ncc {
        if label[s] == big as u32 {
            let (f, r) = (s / ncc, s % ncc);
            true_area += cell_area(f, nc, r % nc, r / nc);
            per_face[f] += 1;
        }
    }
    println!(
        "coarse {nc}: land cells {total_land} ({} components), largest {big_n} cells = {:.3e} km2 (true cell area) / {:.3e} km2 (dx^2, dx {:.0} m), 2nd {:?}; cells per face {per_face:?}",
        sizes.len() - 1,
        true_area / 1e6,
        f64::from(big_n) * dxc * dxc / 1e6,
        dxc,
        &order[1..order.len().min(4)]
    );
    println!("at {n} cells per face edge: {} landmass land cells (16x, dx {:.0} m)", u64::from(big_n) * (ratio * ratio) as u64, R * core::f64::consts::FRAC_PI_2 / n as f64);

    // dilated mask D: component cells and their 8 neighbours
    let mut dmask = vec![0u8; 6 * ncc];
    for s in 0..6 * ncc {
        if label[s] == big as u32 {
            dmask[s] = 1;
            let c = nb_global(nc, s, &mut nbs);
            for &(q, _) in &nbs[..c] {
                dmask[q as usize] = 1;
            }
        }
    }
    drop(label);
    let cand: u64 = dmask.iter().map(|&v| u64::from(v)).sum::<u64>() * (ratio * ratio) as u64;
    println!("candidate fine cells (component + 1 coarse ring) {cand}; step 1 {:.1} s, mem {:?}", t0.elapsed().as_secs_f64(), mem());

    // ---- 2. fine heights: measure the base evaluation rate, then choose
    let tb = Instant::now();
    let inputs = BakeInputs::new(14, threads);
    println!("BakeInputs::new {:.1} s", tb.elapsed().as_secs_f64());
    // contiguous patches, repeated: the first one pays any lazy initialisation
    let mut per_cell = 0.0;
    for rep in 0..3usize {
        let (pf, pj0, pi0, prows, pcols) = (1usize, 6000usize + rep * 64, 4000usize, 32usize, 2048usize);
        let ts = Instant::now();
        let next = AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..threads {
                sc.spawn(|| loop {
                    let r = next.fetch_add(1, Ordering::Relaxed);
                    if r >= prows {
                        break;
                    }
                    for i in pi0..pi0 + pcols {
                        std::hint::black_box(inputs.base(dir(n, pf, i, pj0 + r)));
                    }
                });
            }
        });
        per_cell = ts.elapsed().as_secs_f64() * threads as f64 / (prows * pcols) as f64;
        println!("  patch {rep}: {:.2} us per cell per thread ({:.2} s wall)", per_cell * 1e6, ts.elapsed().as_secs_f64());
    }
    let est = cand as f64 * per_cell / threads as f64;
    let fallback = est > max_eval;
    println!("base eval: {:.2} us per cell per thread; estimate for {cand} cells on {threads} threads {:.0} s (limit {max_eval:.0} s) -> {}", per_cell * 1e6, est, if fallback { "FALLBACK: bilinear upsample of the coarse base + 30 m hash noise" } else { "full BakeInputs::base" });

    let up = |f: usize, i: usize, j: usize| -> f32 {
        let u = ((i as f64 + 0.5) / ratio as f64 - 0.5).clamp(0.0, (nc - 1) as f64);
        let v = ((j as f64 + 0.5) / ratio as f64 - 0.5).clamp(0.0, (nc - 1) as f64);
        let (i0, j0) = (u.floor() as usize, v.floor() as usize);
        let (i1, j1) = ((i0 + 1).min(nc - 1), (j0 + 1).min(nc - 1));
        let (fu, fv) = (u - i0 as f64, v - j0 as f64);
        let g = |ii: usize, jj: usize| f64::from(zc[f * ncc + jj * nc + ii]);
        let z = g(i0, j0) * (1.0 - fu) * (1.0 - fv) + g(i1, j0) * fu * (1.0 - fv) + g(i0, j1) * (1.0 - fu) * fv + g(i1, j1) * fu * fv;
        (z + 30.0 * hash_noise(f, i, j)) as f32
    };

    let tev = Instant::now();
    let wpr = n / 64;
    let mut gbits: Vec<u64> = Vec::with_capacity(6 * nn / 64);
    let mut gz: Vec<f32> = Vec::new();
    for f in 0..6 {
        let mut bits = vec![0u64; nn / 64];
        let mut rows: Vec<Vec<f32>> = vec![Vec::new(); n];
        {
            let it = Mutex::new(bits.chunks_mut(wpr).zip(rows.iter_mut()).enumerate());
            std::thread::scope(|sc| {
                for _ in 0..threads {
                    sc.spawn(|| loop {
                        let item = it.lock().unwrap().next();
                        let Some((j, (wb, row))) = item else { break };
                        let cj = j / ratio;
                        let drow = &dmask[f * ncc + cj * nc..f * ncc + (cj + 1) * nc];
                        for i in 0..n {
                            if drow[i / ratio] != 0 {
                                wb[i >> 6] |= 1u64 << (i & 63);
                                row.push(if fallback { up(f, i, j) } else { inputs.base(dir(n, f, i, j)) as f32 });
                            }
                        }
                    });
                }
            });
        }
        gbits.extend_from_slice(&bits);
        drop(bits);
        for r in rows {
            gz.extend_from_slice(&r);
        }
        println!("  face {f}: {} candidate cells so far, {:.1} s, mem {:?}", gz.len(), tev.elapsed().as_secs_f64(), mem());
    }
    let t_heights = tev.elapsed().as_secs_f64();
    assert!(gz.iter().all(|v| v.is_finite()));
    println!("heights for {} cells: {t_heights:.1} s ({})", gz.len(), if fallback { "upsample" } else { "BakeInputs::base" });

    // ---- filter: sea cells that touch no land are dropped
    let tf = Instant::now();
    let idx0 = Idx::new(n, gbits);
    let mut nbits = vec![0u64; idx0.bits.len()];
    let mut nz: Vec<f32> = Vec::with_capacity(gz.len());
    let mut ncell: Vec<u32> = Vec::with_capacity(gz.len());
    let mut k = 0usize;
    let mut tmp = [(0u32, false); 8];
    for (w, &word) in idx0.bits.iter().enumerate() {
        let mut m = word;
        while m != 0 {
            let b = m.trailing_zeros() as usize;
            m &= m - 1;
            let p = w * 64 + b;
            let keep = if gz[k] > 0.0 {
                true
            } else {
                let c = idx0.neigh(p, &mut tmp);
                tmp[..c].iter().any(|&(q, _)| gz[q as usize] > 0.0)
            };
            if keep {
                nbits[w] |= 1u64 << b;
                nz.push(gz[k]);
                ncell.push(p as u32);
            }
            k += 1;
        }
    }
    drop(idx0);
    drop(gz);
    let idx = Idx::new(n, nbits);
    let z = nz;
    let cell = ncell;
    let cells = z.len();
    let land = z.iter().filter(|v| **v > 0.0).count();
    println!("filter {:.1} s: route cells {cells} (land {land}, sea outlets {}), mem {:?}", tf.elapsed().as_secs_f64(), cells - land, mem());

    // ---- 3. flood, receivers, MFD
    let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
    let tfl = Instant::now();
    let mut filled = vec![f64::NAN; cells];
    let mut rcv = vec![NONE; cells];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let mut flat: VecDeque<u32> = VecDeque::new();
    for k in 0..cells {
        if z[k] <= 0.0 {
            filled[k] = f64::from(z[k]);
            heap.push(Reverse((key(f64::from(z[k])), k as u32)));
        }
    }
    let mut maxheap = heap.len();
    loop {
        let p = if let Some(p) = flat.pop_front() {
            p as usize
        } else if let Some(Reverse((_, p))) = heap.pop() {
            p as usize
        } else {
            break;
        };
        let c = idx.neigh(cell[p] as usize, &mut nbs);
        for &(q, _) in &nbs[..c] {
            let q = q as usize;
            if !filled[q].is_nan() {
                continue;
            }
            let lowest = filled[p] + 1e-3;
            rcv[q] = p as u32;
            if f64::from(z[q]) <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = f64::from(z[q]);
                heap.push(Reverse((key(f64::from(z[q])), q as u32)));
            }
        }
        maxheap = maxheap.max(heap.len() + flat.len());
    }
    drop(heap);
    drop(flat);
    let t_flood = tfl.elapsed().as_secs_f64();
    let unreached = filled.iter().filter(|v| v.is_nan()).count();
    println!("flood: {t_flood:.1} s; max frontier {maxheap} entries; never reached {unreached}; peak {} MiB", peak_mib());

    // D8 receivers (steepest strictly lower on the filled surface, ties to the lower index; flood parent otherwise)
    let tr = Instant::now();
    let mut kept_parent = 0usize;
    for p in 0..cells {
        if z[p] <= 0.0 {
            rcv[p] = NONE;
            continue;
        }
        let c = idx.neigh(cell[p] as usize, &mut nbs);
        let mut best = NONE;
        let mut best_s = 0.0f64;
        for &(q, diag) in &nbs[..c] {
            let fq = filled[q as usize];
            if fq < filled[p] {
                let s = (filled[p] - fq) / if diag { core::f64::consts::SQRT_2 } else { 1.0 };
                if s > best_s || (s == best_s && best != NONE && q < best) {
                    best_s = s;
                    best = q;
                }
            }
        }
        if best == NONE {
            kept_parent += 1;
        } else {
            rcv[p] = best;
        }
    }
    let t_rcv = tr.elapsed().as_secs_f64();
    println!("D8 receivers: {t_rcv:.1} s (cells on the flood parent: {kept_parent}); peak {} MiB", peak_mib());

    // MFD accumulation: highest filled cell first (ties by index)
    let tm = Instant::now();
    let mut by_height: Vec<u32> = (0..cells as u32).collect();
    by_height.sort_unstable_by(|&x, &y| filled[y as usize].total_cmp(&filled[x as usize]).then(x.cmp(&y)));
    let t_sort = tm.elapsed().as_secs_f64();
    let own = dx * dx;
    let mut area = vec![own; cells];
    for &p in &by_height {
        let p = p as usize;
        if z[p] <= 0.0 {
            continue;
        }
        let c = idx.neigh(cell[p] as usize, &mut nbs);
        let mut weight = [0.0f64; 8];
        let mut sum = 0.0;
        for (d, &(q, diag)) in nbs[..c].iter().enumerate() {
            let slope = (filled[p] - filled[q as usize]) / (dx * if diag { core::f64::consts::SQRT_2 } else { 1.0 });
            if slope > 0.0 {
                weight[d] = libm::pow(slope, 1.1);
                sum += weight[d];
            }
        }
        if sum > 0.0 {
            let held = area[p];
            for d in 0..c {
                if weight[d] > 0.0 {
                    area[nbs[d].0 as usize] += held * weight[d] / sum;
                }
            }
        }
    }
    let t_mfd = tm.elapsed().as_secs_f64();
    println!("MFD: sort {t_sort:.1} s, total {t_mfd:.1} s; peak {} MiB", peak_mib());
    let out_sum: f64 = (0..cells).filter(|&k| z[k] <= 0.0).map(|k| area[k] - own).sum();
    let mut top = (0..cells).filter(|&k| z[k] <= 0.0).map(|k| area[k]).fold(0.0f64, f64::max);
    if top.is_nan() {
        top = 0.0;
    }
    println!("check: area into outlets {:.4e} km2 vs land {:.4e} km2 (dx^2); largest outlet catchment {:.3e} km2", out_sum / 1e6, land as f64 * own / 1e6, top / 1e6);

    // ---- report
    let (ws, pk) = mem();
    let bits_b = idx.bits.len() * 8 + idx.rank.len() * 4;
    let arrays = cells * (4 + 8 + 4 + 4 + 8 + 4);
    println!("array bytes: z f32 4, filled f64 8, rcv u32 4, by_height u32 4, area f64 8, cell u32 4 = 32 B/cell = {:.2} GiB; index bitmap+rank {:.0} MiB ({:.2} B/cell)", arrays as f64 / (1u64 << 30) as f64, bits_b as f64 / (1u64 << 20) as f64, bits_b as f64 / cells as f64);
    println!("resident now {ws} MiB, peak process {pk} MiB; total {:.1} s", t0.elapsed().as_secs_f64());
    println!("time: heights {t_heights:.1} s, flood {t_flood:.1} s, receivers {t_rcv:.1} s, MFD {t_mfd:.1} s");
}
