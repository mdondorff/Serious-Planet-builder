//! SPIKE-105 §13.3 (spike branch only, never merged): finds the terrain prototype region and its edit-fixture locations on seed 14 at 3.26 km cells.
//! `prototype_region STORE N THREADS CACHE_DIR` reads the C1 elevation of all six faces from the store (read-only; cached as raw f32 in CACHE_DIR),
//! samples the class weights, aridity and rain at every 6th cell, floods and routes planet-wide over cube edges (as F6), scores 500 km windows
//! on a 125 km grid, and prints the best five, the fixtures E1-E6 of the best one, and the cube-edge crossing window.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use planet_cache::global_store::{decode_block, GlobalWorld, KIND_GLOBAL_BLOCK};
use planet_cache::pack::TileStore;
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, TileId, Vec3};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::classes::{HIGH_MOUNTAINS, MOUNTAINS, PLATEAU};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};

const NONE: u32 = u32::MAX;
const R: f64 = 6_371_000.0;
const ST: usize = 6;
const WIN: usize = 154; // 500 km at 3.26 km
const STEP: usize = 38; // 125 km
const CH: u32 = 5; // 50 km2 at 10.6 km2 a cell
const TRIB: u32 = 47; // 500 km2
const RIV: u32 = 283; // 3000 km2

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

struct RawSource<'a> {
    store: &'a TileStore,
    res: u32,
    level: u8,
    cells: u32,
    cap: usize,
    lru: Mutex<Vec<(TileId, Arc<Vec<GlobalSample>>)>>,
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

fn dir(n: usize, f: usize, i: usize, j: usize) -> Vec3 {
    let s = 2.0 * (i as f64 + 0.5) / n as f64 - 1.0;
    let t = 2.0 * (j as f64 + 0.5) / n as f64 - 1.0;
    face_to_direction(&TangentWarp, Face(f as u8), s, t)
}

struct Prefix {
    w: usize,
    v: Vec<f64>,
}
impl Prefix {
    fn new(n: usize, f: impl Fn(usize, usize) -> f64) -> Prefix {
        let w = n + 1;
        let mut v = vec![0f64; w * w];
        for j in 0..n {
            let mut row = 0.0;
            for i in 0..n {
                row += f(i, j);
                v[(j + 1) * w + i + 1] = v[j * w + i + 1] + row;
            }
        }
        Prefix { w, v }
    }
    fn sum(&self, i0: usize, j0: usize, i1: usize, j1: usize) -> f64 {
        let w = self.w;
        self.v[j1 * w + i1] - self.v[j0 * w + i1] - self.v[j1 * w + i0] + self.v[j0 * w + i0]
    }
}

struct Basin {
    size: usize,
    lowest: u32,
    depth: f64,
    spill: u32,
    spill_level: f64,
    beyond: u32,
    arid: f64,
    zmax: f64,
    cells: Vec<u32>,
}

struct Stem {
    cell: u32,
    tribs: u8,
    dist_km: f64,
}

#[derive(Clone, Default)]
struct Cand {
    face: usize,
    i0: usize,
    j0: usize,
    land: f64,
    mh: f64,
    pl: f64,
    chan: f64,
    mouth: u32,
    river_km2: f64,
    tribs: usize,
    basins: usize,
    basin_best: i32,
    score: f64,
    lat: f64,
    arid_basin: bool,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir_s = std::path::PathBuf::from(&a[1]);
    let n: usize = a[2].parse().unwrap();
    let threads: usize = a[3].parse().unwrap();
    let cache = std::path::PathBuf::from(&a[4]);
    let t0 = std::time::Instant::now();
    let nn = n * n;
    let total = 6 * nn;
    let m = n / ST;
    let dx = R * core::f64::consts::FRAC_PI_2 / n as f64;
    let own = dx * dx;
    let dxkm = dx / 1000.0;
    let ll = |p: usize| -> (f64, f64) {
        let (la, lo) = dir(n, p / nn, (p % nn) % n, (p % nn) / n).lat_lon();
        (la.to_degrees(), lo.to_degrees())
    };
    let fc = |p: usize| (p / nn, (p % nn) % n, (p % nn) / n);
    let desc = |p: usize| {
        let (f, i, j) = fc(p);
        let (la, lo) = ll(p);
        format!("face {f} cell ({i},{j}) lat {la:.2} lon {lo:.2}")
    };

    // --- elevation from BakeInputs::base (cached)
    let tf = std::time::Instant::now();
    let inputs = BakeInputs::new(14, threads);
    println!("BakeInputs::new {:.1} s", tf.elapsed().as_secs_f64());
    let zpath = cache.join(format!("base_{n}.f32"));
    let mut z = vec![0f32; total];
    if let Ok(bytes) = std::fs::read(&zpath) {
        assert_eq!(bytes.len(), total * 4);
        for (k, c) in bytes.chunks_exact(4).enumerate() {
            z[k] = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        }
        println!("base elevation from cache {:.1} s", t0.elapsed().as_secs_f64());
    } else {
        let _ = (&dir_s, GlobalWorld::from_definition as usize);
        for f in 0..6 {
            let next = AtomicUsize::new(0);
            let zp = SendPtr(z[f * nn..(f + 1) * nn].as_mut_ptr());
            std::thread::scope(|sc| {
                for _ in 0..threads {
                    sc.spawn(|| {
                        let zp = &zp;
                        loop {
                            let j = next.fetch_add(1, Ordering::Relaxed);
                            if j >= n {
                                break;
                            }
                            for i in 0..n {
                                // SAFETY: each row is written by one work item.
                                unsafe { *zp.0.add(j * n + i) = inputs.base(dir(n, f, i, j)) as f32 };
                            }
                        }
                    });
                }
            });
            println!("  face {f} base {:.1} s", t0.elapsed().as_secs_f64());
        }
        let mut bytes = Vec::with_capacity(total * 4);
        for v in &z {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(&zpath, bytes).unwrap();
    }
    assert!(z.iter().all(|v| v.is_finite()));
    let land = z.iter().filter(|v| **v > 0.0).count();
    println!("land {:.1} %", 100.0 * land as f64 / total as f64);

    // --- coarse fields at every ST-th cell: mountains+high, plateau, aridity, precipitation
    let cm = m * m;
    let mut mh = vec![0f32; 6 * cm];
    let mut pl = vec![0f32; 6 * cm];
    let mut ar = vec![0f32; 6 * cm];
    let mut pr = vec![0f32; 6 * cm];
    {
        let next = AtomicUsize::new(0);
        let res = Mutex::new((&mut mh, &mut pl, &mut ar, &mut pr));
        std::thread::scope(|sc| {
            for _ in 0..threads {
                sc.spawn(|| loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= 6 * m {
                        break;
                    }
                    let (f, cj) = (k / m, k % m);
                    let mut row = vec![(0f32, 0f32, 0f32, 0f32); m];
                    for (ci, r) in row.iter_mut().enumerate() {
                        let d = dir(n, f, ci * ST + ST / 2, cj * ST + ST / 2);
                        let w = inputs.weights_at(d);
                        let o = inputs.orography();
                        *r = ((w[MOUNTAINS] + w[HIGH_MOUNTAINS]) as f32, w[PLATEAU] as f32, o.aridity(d) as f32, o.precipitation_mm(d) as f32);
                    }
                    let mut g = res.lock().unwrap();
                    for (ci, r) in row.iter().enumerate() {
                        let at = f * cm + cj * m + ci;
                        g.0[at] = r.0;
                        g.1[at] = r.1;
                        g.2[at] = r.2;
                        g.3[at] = r.3;
                    }
                });
            }
        });
    }
    println!("coarse fields {:.1} s (stride {ST} = {:.1} km)", tf.elapsed().as_secs_f64(), ST as f64 * dxkm);
    let cidx = |p: usize| {
        let (f, i, j) = fc(p);
        f * cm + (j / ST) * m + i / ST
    };
    // --- flood, receivers, area
    let g = G { n };
    let tfl = std::time::Instant::now();
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
    println!("flood {:.1} s, unreached {}", tfl.elapsed().as_secs_f64(), filled.iter().filter(|v| v.is_nan()).count());
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
        if best != NONE {
            rcv[p] = best;
        }
    }
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
    drop(indeg);
    println!("routed, ordered {} of {total}; {:.1} s", order.len(), t0.elapsed().as_secs_f64());
    // catchment root (the land cell whose receiver is the sea) and max-area donor
    let mut root = vec![NONE; total];
    for &p in order.iter().rev() {
        let p = p as usize;
        if z[p] <= 0.0 {
            continue;
        }
        root[p] = if rcv[p] == NONE || z[rcv[p] as usize] <= 0.0 { p as u32 } else { root[rcv[p] as usize] };
    }
    let mut up = vec![NONE; total];
    for p in 0..total {
        if rcv[p] != NONE {
            let r = rcv[p] as usize;
            if up[r] == NONE || area[p] > area[up[r] as usize] {
                up[r] = p as u32;
            }
        }
    }
    let km2 = |a: u32| f64::from(a) * own / 1e6;

    // --- closed (filled) basins: connected cells of depth over 50 m, at least 500 km2
    let mut basins: Vec<Basin> = Vec::new();
    {
        let mut seen = vec![false; total];
        let mut q: Vec<u32> = Vec::new();
        for s in 0..total {
            if seen[s] || z[s] <= 0.0 || filled[s] - (z[s] as f64) <= 50.0 {
                continue;
            }
            seen[s] = true;
            q.clear();
            q.push(s as u32);
            let mut h = 0;
            while h < q.len() {
                let p = q[h] as usize;
                h += 1;
                let c = g.nb(p, &mut nbs);
                for k in 0..c {
                    let r = nbs[k] as usize;
                    if !seen[r] && z[r] > 0.0 && filled[r] - (z[r] as f64) > 50.0 {
                        seen[r] = true;
                        q.push(r as u32);
                    }
                }
            }
            if q.len() < TRIB as usize {
                continue;
            }
            let lowest = *q.iter().min_by(|a, b| z[**a as usize].total_cmp(&z[**b as usize])).unwrap();
            let depth = q.iter().map(|&p| filled[p as usize] - z[p as usize] as f64).fold(0.0, f64::max);
            let mut sp = lowest as usize;
            let mut guard = 0;
            while filled[sp] - (z[sp] as f64) > 0.01 && rcv[sp] != NONE && guard < 100_000 {
                sp = rcv[sp] as usize;
                guard += 1;
            }
            let beyond = if rcv[sp] != NONE { rcv[sp] } else { sp as u32 };
            let arid = q.iter().map(|&p| f64::from(ar[cidx(p as usize)])).sum::<f64>() / q.len() as f64;
            let zmax = q.iter().map(|&p| z[p as usize] as f64).fold(f64::MIN, f64::max);
            basins.push(Basin { size: q.len(), lowest, depth, spill: sp as u32, spill_level: filled[sp], beyond, arid, zmax, cells: q.clone() });
        }
    }
    println!("basins (depth > 50 m, >= 500 km2): {}", basins.len());

    // --- mouths and stems
    let mut mouths: Vec<(u32, Vec<Stem>)> = Vec::new();
    for p in 0..total {
        if z[p] > 0.0 && rcv[p] != NONE && z[rcv[p] as usize] <= 0.0 && area[p] >= RIV {
            let mut stem = Vec::new();
            let mut s = p;
            let mut dist = 0.0;
            loop {
                let c = g.nb(s, &mut nbs);
                let mut best = NONE;
                let mut tribs = 0u8;
                for k in 0..c {
                    let q = nbs[k] as usize;
                    if rcv[q] == s as u32 {
                        if best == NONE || area[q] > area[best as usize] {
                            if best != NONE && area[best as usize] >= TRIB {
                                tribs += 1;
                            }
                            best = q as u32;
                        } else if area[q] >= TRIB {
                            tribs += 1;
                        }
                    }
                }
                stem.push(Stem { cell: s as u32, tribs, dist_km: dist });
                if best == NONE || area[best as usize] < TRIB {
                    break;
                }
                let (a, b) = (fc(s), fc(best as usize));
                let diag = a.0 == b.0 && a.1 != b.1 && a.2 != b.2;
                dist += dxkm * if diag { core::f64::consts::SQRT_2 } else { 1.0 };
                s = best as usize;
            }
            mouths.push((p as u32, stem));
        }
    }
    println!("mouths with >= 3000 km2: {}", mouths.len());

    // --- window scoring
    let wkm2 = (WIN * WIN) as f64;
    let mut cands: Vec<Cand> = Vec::new();
    let mut n_pass = 0usize;
    let mut n_relaxed = 0usize;
    for f in 0..6 {
        let base = f * nn;
        let pland = Prefix::new(n, |i, j| if z[base + j * n + i] > 0.0 { 1.0 } else { 0.0 });
        let pchan = Prefix::new(n, |i, j| if z[base + j * n + i] > 0.0 && area[base + j * n + i] >= CH { 1.0 } else { 0.0 });
        let pmh = Prefix::new(n, |i, j| f64::from(mh[cidx(base + j * n + i)]));
        let ppl = Prefix::new(n, |i, j| f64::from(pl[cidx(base + j * n + i)]));
        let fm: Vec<usize> = (0..mouths.len()).filter(|&k| mouths[k].0 as usize / nn == f).collect();
        let fb: Vec<usize> = (0..basins.len()).filter(|&k| basins[k].lowest as usize / nn == f).collect();
        let mut j0 = 0;
        while j0 + WIN <= n {
            let mut i0 = 0;
            while i0 + WIN <= n {
                let (i1, j1) = (i0 + WIN, j0 + WIN);
                let inside = |p: u32| {
                    let (pf, i, j) = fc(p as usize);
                    pf == f && i >= i0 && i < i1 && j >= j0 && j < j1
                };
                let landf = pland.sum(i0, j0, i1, j1) / wkm2;
                let mhf = pmh.sum(i0, j0, i1, j1) / wkm2;
                let plf = ppl.sum(i0, j0, i1, j1) / wkm2;
                let chan = pchan.sum(i0, j0, i1, j1);
                let mut c = Cand { face: f, i0, j0, land: landf, mh: mhf, pl: plf, chan, ..Default::default() };
                for &k in &fm {
                    let (mo, stem) = &mouths[k];
                    if !inside(*mo) {
                        continue;
                    }
                    let tr: usize = stem.iter().filter(|s| inside(s.cell)).map(|s| s.tribs as usize).sum();
                    let ak = km2(area[*mo as usize]);
                    if tr >= 3 && (ak > c.river_km2 || c.tribs < 3) || (c.tribs < 3 && ak > c.river_km2) {
                        c.mouth = *mo;
                        c.river_km2 = ak;
                        c.tribs = tr;
                    }
                }
                for &k in &fb {
                    let b = &basins[k];
                    if inside(b.lowest) && inside(b.spill) {
                        c.basins += 1;
                        let good = b.arid < 0.3 || b.zmax > 1500.0;
                        c.basin_best = c.basin_best.max(if good { 2 } else { 1 });
                        if b.arid < 0.3 {
                            c.arid_basin = true;
                        }
                    }
                }
                let relaxed = landf >= 0.4 && landf <= 0.9 && c.river_km2 >= 3000.0 && c.tribs >= 3;
                let full = relaxed && mhf >= 0.15 && plf >= 0.03 && c.basins >= 1;
                if relaxed {
                    n_relaxed += 1;
                }
                if full {
                    c.lat = dir(n, f, i0 + WIN / 2, j0 + WIN / 2).lat_lon().0.to_degrees();
                    n_pass += 1;
                    let t = |x: f64, s: f64| (x / s).min(1.0);
                    c.score = t(mhf, 0.3) + t(plf, 0.1) + (1.0 - ((landf - 0.65).abs() / 0.25).min(1.0)) + t(c.tribs as f64, 8.0) + t(c.river_km2, 30000.0) + f64::from(c.basin_best) / 2.0 + t(chan, 600.0) - if c.lat.abs() > 58.0 { 3.0 } else { 0.0 } + if c.lat.abs() >= 20.0 && c.lat.abs() <= 55.0 { 1.0 } else { 0.0 } + if c.arid_basin { 1.5 } else { 0.0 };
                    cands.push(c);
                }
                i0 += STEP;
            }
            j0 += STEP;
        }
    }
    println!("windows meeting the land/river/tributary rule: {n_relaxed}; meeting every rule: {n_pass}");
    cands.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut top: Vec<Cand> = Vec::new();
    for c in &cands {
        let ok = top.iter().all(|t| {
            let (a, b) = (dir(n, c.face, c.i0 + WIN / 2, c.j0 + WIN / 2), dir(n, t.face, t.i0 + WIN / 2, t.j0 + WIN / 2));
            a.angle_to(b) * R / 1000.0 > 500.0
        });
        if ok {
            top.push(c.clone());
            if top.len() == 5 {
                break;
            }
        }
    }
    println!("\nTOP 5 windows (500 km = {WIN} cells at {dxkm:.2} km)");
    for (k, c) in top.iter().enumerate() {
        let (la, lo) = dir(n, c.face, c.i0 + WIN / 2, c.j0 + WIN / 2).lat_lon();
        println!(
            "#{} face {} i {}..{} j {}..{} centre lat {:.1} lon {:.1} | land {:.0}% mt+high {:.0}% plateau {:.0}% chan cells {} | river {:.0} km2 tribs {} | basins {} (best {}) | score {:.2}",
            k + 1, c.face, c.i0, c.i0 + WIN, c.j0, c.j0 + WIN, la.to_degrees(), lo.to_degrees(), c.land * 100.0, c.mh * 100.0, c.pl * 100.0, c.chan, c.river_km2, c.tribs, c.basins, c.basin_best, c.score
        );
    }
    println!("passing windows with an arid (P/PET < 0.3) basin inside: {}", cands.iter().filter(|c| c.arid_basin).count());
    let pr_line = |tag: &str, c: &Cand| println!("{tag}: face {} i {}..{} j {}..{} lat {:.1} | land {:.0}% mt+high {:.0}% plateau {:.0}% | river {:.0} km2 tribs {} basins {} arid {} | score {:.2}", c.face, c.i0, c.i0 + WIN, c.j0, c.j0 + WIN, c.lat, c.land * 100.0, c.mh * 100.0, c.pl * 100.0, c.river_km2, c.tribs, c.basins, c.arid_basin, c.score);
    if let Some(c) = cands.iter().find(|c| c.lat.abs() < 20.0) { pr_line("best tropical", c); }
    if let Some(c) = cands.iter().find(|c| c.arid_basin) { pr_line("best with arid basin", c); }
    if top.is_empty() {
        println!("no window meets every rule");
        return;
    }
    let w = top[0].clone();
    let (f, i0, j0) = (w.face, w.i0, w.j0);
    let (i1, j1) = (i0 + WIN, j0 + WIN);
    let inside = |p: u32| {
        let (pf, i, j) = fc(p as usize);
        pf == f && i >= i0 && i < i1 && j >= j0 && j < j1
    };
    println!("\nCHOSEN: face {f} i {i0}..{i1} j {j0}..{j1}, centre {}", desc(f * nn + (j0 + WIN / 2) * n + i0 + WIN / 2));
    let pz = |p: usize| z[p] as f64;

    // E1: 15 x 15 cells (49 km) with the highest mean elevation
    let b1 = 15usize;
    let mut best1 = (f64::MIN, 0usize, 0usize);
    {
        let pz_p = Prefix::new(n.min(j1).min(i1), |_, _| 0.0);
        let _ = pz_p;
        let loc = Prefix::new(WIN, |i, j| z[f * nn + (j0 + j) * n + i0 + i] as f64);
        for j in 0..=WIN - b1 {
            for i in 0..=WIN - b1 {
                let s = loc.sum(i, j, i + b1, j + b1) / (b1 * b1) as f64;
                if s > best1.0 {
                    best1 = (s, i0 + i, j0 + j);
                }
            }
        }
    }
    let e1c = f * nn + (best1.2 + b1 / 2) * n + best1.1 + b1 / 2;
    println!("E1: block face {f} i {}..{} j {}..{} (49 km), mean elevation {:.0} m, mountains+high weight {:.2}; centre {}", best1.1, best1.1 + b1, best1.2, best1.2 + b1, best1.0, mh[cidx(e1c)], desc(e1c));

    // E2: the divide between the two largest catchments (both over 10,000 km2) with the longest shared boundary in the window
    let mut pairs: HashMap<(u32, u32), Vec<(u32, u32)>> = HashMap::new();
    for j in j0..j1 - 1 {
        for i in i0..i1 - 1 {
            let p = f * nn + j * n + i;
            for q in [p + 1, p + n] {
                if z[p] > 0.0 && z[q] > 0.0 && root[p] != root[q] && area[root[p] as usize] >= 944 && area[root[q] as usize] >= 944 {
                    let k = (root[p].min(root[q]), root[p].max(root[q]));
                    pairs.entry(k).or_default().push((p as u32, q as u32));
                }
            }
        }
    }
    let modr = |r: u32| (944..=9440).contains(&area[r as usize]);
    let mut pl_: Vec<_> = pairs.iter().filter(|(k, _)| modr(k.0) && modr(k.1)).collect();
    if pl_.is_empty() {
        println!("E2: no pair of two moderate catchments (10-100k km2); using pairs with one moderate");
        pl_ = pairs.iter().filter(|(k, _)| modr(k.0) || modr(k.1)).collect();
    }
    pl_.sort_by_key(|(k, v)| (Reverse(v.len()), **k));
    if let Some(((ra, rb), v)) = pl_.first() {
        let cells: Vec<(f64, f64)> = v.iter().map(|&(p, _)| ((p as usize % nn % n) as f64, (p as usize % nn / n) as f64)).collect();
        let (cx, cy) = (cells.iter().map(|c| c.0).sum::<f64>() / cells.len() as f64, cells.iter().map(|c| c.1).sum::<f64>() / cells.len() as f64);
        let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
        for c in &cells {
            sxx += (c.0 - cx).powi(2);
            syy += (c.1 - cy).powi(2);
            sxy += (c.0 - cx) * (c.1 - cy);
        }
        let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let (ux, uy) = (th.cos(), th.sin());
        let mid = v.iter().min_by(|a, b| {
            let da = ((a.0 as usize % nn % n) as f64 - cx).powi(2) + ((a.0 as usize % nn / n) as f64 - cy).powi(2);
            let db = ((b.0 as usize % nn % n) as f64 - cx).powi(2) + ((b.0 as usize % nn / n) as f64 - cy).powi(2);
            da.total_cmp(&db)
        }).unwrap();
        let sill = v.iter().filter(|c| pz(c.0 as usize).max(pz(c.1 as usize)) >= 500.0).min_by(|a, b| pz(a.0 as usize).max(pz(a.1 as usize)).total_cmp(&pz(b.0 as usize).max(pz(b.1 as usize)))).unwrap();
        let top_ = v.iter().max_by(|a, b| pz(a.0 as usize).max(pz(a.1 as usize)).total_cmp(&pz(b.0 as usize).max(pz(b.1 as usize)))).unwrap();
        println!(
            "E2: divide between catchments {:.0} km2 (mouth {}) and {:.0} km2 (mouth {}); {} boundary pairs (~{:.0} km)",
            km2(area[*ra as usize]), desc(*ra as usize), km2(area[*rb as usize]), desc(*rb as usize), v.len(), v.len() as f64 * dxkm
        );
        println!("    mid-divide cell {} z {:.0} m; sill (lowest pass) {} z {:.0} m; highest divide cell z {:.0} m", desc(mid.0 as usize), pz(mid.0 as usize), desc(sill.0 as usize), pz(sill.0 as usize).max(pz(sill.1 as usize)), pz(top_.0 as usize).max(pz(top_.1 as usize)));
        println!(
            "    ridge line of 100 km (31 cells) along the divide: from ({:.0},{:.0}) to ({:.0},{:.0}) on face {f} (direction {:.0} deg from +i towards +j)",
            cx - 15.5 * ux, cy - 15.5 * uy, cx + 15.5 * ux, cy + 15.5 * uy, th.to_degrees()
        );
    } else {
        println!("E2: no divide between two catchments over 10,000 km2 in the window");
    }

    // E3: basins
    let mut inw: Vec<&Basin> = basins.iter().filter(|b| inside(b.lowest) && inside(b.spill)).collect();
    inw.sort_by(|a, b| (b.depth * b.size as f64).total_cmp(&(a.depth * a.size as f64)));
    for (k, b) in inw.iter().take(3).enumerate() {
        println!(
            "E3 candidate {}: {} cells ({:.0} km2), depth {:.0} m, mean aridity {:.2}, max z {:.0} m\n    lowest {} z {:.0} m; spill {} level {:.0} m; cell beyond the spill {} z {:.0} m",
            k + 1, b.size, km2(b.size as u32), b.depth, b.arid, b.zmax, desc(b.lowest as usize), pz(b.lowest as usize), desc(b.spill as usize), b.spill_level, desc(b.beyond as usize), pz(b.beyond as usize)
        );
    }
    if let Some(b) = inw.first() {
        let lp = fc(b.lowest as usize);
        let mut near: Vec<(f64, &Basin)> = basins
            .iter()
            .filter(|o| o.lowest != b.lowest && o.lowest as usize / nn == lp.0)
            .map(|o| {
                let q = fc(o.lowest as usize);
                (((q.1 as f64 - lp.1 as f64).powi(2) + (q.2 as f64 - lp.2 as f64).powi(2)).sqrt() * dxkm, o)
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, o)) = near.first() {
            println!("    nearest other basin to merge: {d:.0} km away, {} cells depth {:.0} m, lowest {} z {:.0} m, spill level {:.0} m", o.size, o.depth, desc(o.lowest as usize), pz(o.lowest as usize), o.spill_level);
        }
        println!("    smallest outlet: cut the spill cell from {:.0} m to the cell beyond ({:.0} m): {:.0} m cut at the spill (spill level over beyond cell)", b.spill_level, pz(b.beyond as usize), b.spill_level - pz(b.beyond as usize));
    }

    // E4 / E5: the main river of the window
    let (mo, stem) = mouths.iter().find(|(mo, _)| *mo == w.mouth).unwrap();
    println!("E4: main river mouth {} area {:.0} km2, stem length in cells {} ({:.0} km), tribs in window {}", desc(*mo as usize), km2(area[*mo as usize]), stem.len(), stem.last().unwrap().dist_km, w.tribs);
    let far = stem.iter().find(|s| s.dist_km >= 160.0 && inside(s.cell));
    let near = stem.iter().find(|s| s.dist_km >= 15.0 && inside(s.cell));
    if let Some(s) = far {
        println!("    upstream trunk cell {} at {:.0} km from the mouth, area {:.0} km2, z {:.0} m", desc(s.cell as usize), s.dist_km, km2(area[s.cell as usize]), pz(s.cell as usize));
    } else {
        println!("    no stem cell over 160 km from the mouth inside the window");
    }
    if let Some(s) = near {
        println!("    near-mouth cell {} at {:.0} km, area {:.0} km2, z {:.0} m", desc(s.cell as usize), s.dist_km, km2(area[s.cell as usize]), pz(s.cell as usize));
    }
    // E5: the tributary with the farthest head
    let mut best5: Option<(f64, usize, usize, usize)> = None; // dist, head, junction, tributary cell
    for s in stem.iter().filter(|s| s.tribs > 0 && inside(s.cell)) {
        let c = g.nb(s.cell as usize, &mut nbs);
        for k in 0..c {
            let q = nbs[k] as usize;
            if rcv[q] == s.cell && q as u32 != up[s.cell as usize] && area[q] >= TRIB {
                let mut h = q;
                let mut len = 0.0;
                while up[h] != NONE && area[up[h] as usize] >= CH {
                    h = up[h] as usize;
                    len += dxkm;
                }
                if inside(h as u32) && best5.map_or(true, |b| len > b.0) {
                    best5 = Some((len, h, s.cell as usize, q));
                }
            }
        }
    }
    if let Some((len, h, jn, q)) = best5 {
        let (hf, hi, hj) = fc(h);
        let (mut sp, mut sa, mut cnt) = (0.0f64, 0.0f64, 0);
        for j in hj.saturating_sub(15)..(hj + 16).min(n) {
            for i in hi.saturating_sub(15)..(hi + 16).min(n) {
                let ci = hf * cm + (j / ST) * m + i / ST;
                sp += f64::from(pr[ci]);
                sa += f64::from(ar[ci]);
                cnt += 1;
            }
        }
        let hp = inputs.orography().precipitation_mm(dir(n, hf, hi, hj));
        {
            let mut v: Vec<f64> = Vec::new();
            for j in j0..j1 {
                for i in i0..i1 {
                    if z[f * nn + j * n + i] > 0.0 {
                        v.push(f64::from(pr[cidx(f * nn + j * n + i)]));
                    }
                }
            }
            v.sort_by(|a, b| a.total_cmp(b));
            let q = |x: f64| v[((v.len() - 1) as f64 * x) as usize];
            println!("PRECIP window land (coarse 19.5 km reads of orography().precipitation_mm): p50 {:.0} p95 {:.0} max {:.0} mm/yr; share over 4500: {:.1} %", q(0.5), q(0.95), v[v.len() - 1], 100.0 * v.iter().filter(|x| **x > 4500.0).count() as f64 / v.len() as f64);
        }
        println!(
            "E5: tributary joining at {} (area at the join of the tributary {:.0} km2 at {}); head (area >= 50 km2) {} z {:.0} m, {len:.0} km above the join\n    rain window 31x31 cells (101 km) centred on the head: face {hf} i {}..{} j {}..{}; head precipitation {:.0} mm/yr, window mean {:.0} mm/yr, mean aridity {:.2}",
            desc(jn), km2(area[q]), desc(q), desc(h), pz(h), hi.saturating_sub(15), hi + 16, hj.saturating_sub(15), hj + 16, hp, sp / cnt as f64, sa / cnt as f64
        );
    } else {
        println!("E5: no tributary head found inside the window");
    }

    // E6: 31 x 31 cells with the highest mean mountains+high weight, away from E1
    let b6 = 31usize;
    let loc = Prefix::new(WIN, |i, j| f64::from(mh[cidx(f * nn + (j0 + j) * n + i0 + i)]));
    let mut best6 = (f64::MIN, 0usize, 0usize);
    for j in (0..=WIN - b6).step_by(2) {
        for i in (0..=WIN - b6).step_by(2) {
            let (ci, cj) = (i0 + i + b6 / 2, j0 + j + b6 / 2);
            let d = (((ci as f64 - (best1.1 + b1 / 2) as f64).powi(2) + (cj as f64 - (best1.2 + b1 / 2) as f64).powi(2)).sqrt()) * dxkm;
            if d < 80.0 {
                continue;
            }
            let s = loc.sum(i, j, i + b6, j + b6) / (b6 * b6) as f64;
            if s > best6.0 {
                best6 = (s, i0 + i, j0 + j);
            }
        }
    }
    let e6c = f * nn + (best6.2 + b6 / 2) * n + best6.1 + b6 / 2;
    let d6 = dir(n, f, best6.1 + b6 / 2, best6.2 + b6 / 2);
    let mz: f64 = (0..b6 * b6).map(|k| pz(f * nn + (best6.2 + k / b6) * n + best6.1 + k % b6)).sum::<f64>() / (b6 * b6) as f64;
    println!(
        "E6: block face {f} i {}..{} j {}..{} (101 km), mean mountains+high {:.2}, mean z {:.0} m; centre {}; uplift_at {:.3}, hardness {:.2}",
        best6.1, best6.1 + b6, best6.2, best6.2 + b6, best6.0, mz, desc(e6c), inputs.uplift_at(d6), inputs.lithology().hardness(d6)
    );

    // --- cube-edge crossings
    let mut cross: Vec<(u32, usize, usize)> = Vec::new();
    for p in 0..total {
        if z[p] > 0.0 && area[p] >= RIV && rcv[p] != NONE && rcv[p] as usize / nn != p / nn && z[rcv[p] as usize] > 0.0 {
            cross.push((area[p], p, rcv[p] as usize));
        }
    }
    cross.sort_by(|a, b| b.0.cmp(&a.0));
    println!("\nCUBE-EDGE crossings of rivers >= 3000 km2 (land on both sides): {}", cross.len());
    let wc = dir(n, f, i0 + WIN / 2, j0 + WIN / 2);
    let mut sel = cross.iter().filter(|c| km2(c.0) <= 60000.0).collect::<Vec<_>>();
    sel.sort_by(|a, b| dir(n, a.1 / nn, a.1 % nn % n, a.1 % nn / n).angle_to(wc).total_cmp(&dir(n, b.1 / nn, b.1 % nn % n, b.1 % nn / n).angle_to(wc)));
    for (tag, list) in [("largest", cross.iter().take(3).collect::<Vec<_>>()), ("nearest to the chosen window (area <= 60,000 km2)", sel.into_iter().take(3).collect::<Vec<_>>())] {
        println!("{tag}:");
        for c in list {
            let (pa, pb) = (fc(c.1), fc(c.2));
            let rng = |x: usize| (x.saturating_sub(15), (x + 16).min(n));
            let (ai, aj, bi, bj) = (rng(pa.1), rng(pa.2), rng(pb.1), rng(pb.2));
            println!(
                "  area {:.0} km2: upstream side {} (z {:.0} m, area {:.0}); downstream side {} (z {:.0} m, area {:.0}); window 100 km: face {} i {}..{} j {}..{} and face {} i {}..{} j {}..{}; distance of crossing to chosen window {:.0} km",
                km2(c.0), desc(c.1), pz(c.1), km2(area[c.1]), desc(c.2), pz(c.2), km2(area[c.2]), pa.0, ai.0, ai.1, aj.0, aj.1, pb.0, bi.0, bi.1, bj.0, bj.1,
                dir(n, pa.0, pa.1, pa.2).angle_to(wc) * R / 1000.0
            );
        }
    }
    println!("total {:.1} s", t0.elapsed().as_secs_f64());
}
