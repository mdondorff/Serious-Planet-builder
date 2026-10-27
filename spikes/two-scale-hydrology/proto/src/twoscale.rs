//! SPIKE-31: two-scale hydrology. A coarse cube-sphere bake, fine regions made of whole coarse cells (so the fine grid
//! refines the coarse one exactly), coarse flux injected at the border of a patch, and a comparison of a mosaic of
//! patches against one solve of the whole region.
use super::*;
use std::collections::{BTreeMap, HashSet};

pub struct Coarse {
    pub sph: Sphere,
    pub z: Vec<f32>,
    pub filled: Vec<f32>,
    pub a: Vec<f32>,
}

pub fn coarse_bake(n: usize, threads: usize, hf: &(dyn Fn(Vec3) -> f32 + Sync), tag: &str) -> Coarse {
    let sph = Sphere::new(n);
    let t = Instant::now();
    let Fields { z, lith: _ } = run_parallel(sph.len(), threads, |c| {
        let (f, i, j) = sph.split(c);
        (hf(sph.direction(f, i, j)), 0u8)
    });
    let ts = t.elapsed();
    let t = Instant::now();
    let (filled, order, _) = priority_flood(&sph, &z);
    let a = accumulate(&sph, &filled, &order, Route::Mfd, None);
    println!("COARSE-BAKE {tag} n={n}: sample {:.1}s ({threads} threads), flood+accumulate {:.1}s, peak working set {:.0} MB", ts.as_secs_f64(), t.elapsed().as_secs_f64(), peak_ws_mb());
    Coarse { sph, z, filled, a }
}

// ---------------------------------------------------------------- a fine region of whole coarse cells

struct FaceMap {
    i0: i64,
    j0: i64,
    w: i64,
    h: i64,
    idx: Vec<u32>,
}

pub struct Region {
    pub k: usize,
    pub n_f: usize,
    pub cells: Vec<(u8, u32, u32)>,
    maps: Vec<FaceMap>,
    area: Vec<f32>,
    pub ccells: Vec<u32>,
    cset: HashSet<u32>,
}

impl Region {
    pub fn new(sph: &Sphere, ccells: Vec<u32>, k: usize) -> Region {
        let n_f = sph.n * k;
        let mut bb = [(usize::MAX, 0usize, usize::MAX, 0usize); 6];
        for &c in &ccells {
            let (f, i, j) = sph.split(c as usize);
            let b = &mut bb[f];
            b.0 = b.0.min(i);
            b.1 = b.1.max(i);
            b.2 = b.2.min(j);
            b.3 = b.3.max(j);
        }
        let mut maps: Vec<FaceMap> = bb
            .iter()
            .map(|b| {
                if b.0 == usize::MAX {
                    FaceMap { i0: 0, j0: 0, w: 0, h: 0, idx: vec![] }
                } else {
                    let (w, h) = (((b.1 - b.0 + 1) * k) as i64, ((b.3 - b.2 + 1) * k) as i64);
                    FaceMap { i0: (b.0 * k) as i64, j0: (b.2 * k) as i64, w, h, idx: vec![NONE; (w * h) as usize] }
                }
            })
            .collect();
        let mut cells = Vec::with_capacity(ccells.len() * k * k);
        for &c in &ccells {
            let (f, ci, cj) = sph.split(c as usize);
            for b in 0..k {
                for a in 0..k {
                    let (fi, fj) = (ci * k + a, cj * k + b);
                    let m = &mut maps[f];
                    let slot = (fj as i64 - m.j0) * m.w + (fi as i64 - m.i0);
                    m.idx[slot as usize] = cells.len() as u32;
                    cells.push((f as u8, fi as u32, fj as u32));
                }
            }
        }
        // areas from the exact solid angle
        let r_km = R_M / 1000.0;
        let eu: Vec<Vec<f64>> = maps.iter().map(|m| (0..=m.w).map(|x| plane_u(-1.0 + 2.0 * (m.i0 + x) as f64 / n_f as f64)).collect()).collect();
        let ev: Vec<Vec<f64>> = maps.iter().map(|m| (0..=m.h).map(|y| plane_u(-1.0 + 2.0 * (m.j0 + y) as f64 / n_f as f64)).collect()).collect();
        let area: Vec<f32> = cells
            .iter()
            .map(|&(f, fi, fj)| {
                let m = &maps[f as usize];
                let (x, y) = ((fi as i64 - m.i0) as usize, (fj as i64 - m.j0) as usize);
                let (u0, u1, v0, v1) = (eu[f as usize][x], eu[f as usize][x + 1], ev[f as usize][y], ev[f as usize][y + 1]);
                ((solid_f(u1, v1) - solid_f(u0, v1) - solid_f(u1, v0) + solid_f(u0, v0)) * r_km * r_km) as f32
            })
            .collect();
        let cset = ccells.iter().copied().collect();
        Region { k, n_f, cells, maps, area, ccells, cset }
    }
    pub fn lookup(&self, f: usize, fi: i64, fj: i64) -> u32 {
        let m = &self.maps[f];
        if m.w == 0 {
            return NONE;
        }
        let (x, y) = (fi - m.i0, fj - m.j0);
        if x < 0 || y < 0 || x >= m.w || y >= m.h {
            NONE
        } else {
            m.idx[(y * m.w + x) as usize]
        }
    }
    pub fn dir(&self, c: usize) -> Vec3 {
        let (f, fi, fj) = self.cells[c];
        let n = self.n_f as f64;
        face_to_direction(&TangentWarp, Face(f), -1.0 + (2.0 * fi as f64 + 1.0) / n, -1.0 + (2.0 * fj as f64 + 1.0) / n)
    }
    pub fn find_dir(&self, d: Vec3) -> u32 {
        let (f, s, t) = direction_to_face(&TangentWarp, d);
        let n = self.n_f as f64;
        let i = (((s + 1.0) * 0.5 * n) as i64).clamp(0, self.n_f as i64 - 1);
        let j = (((t + 1.0) * 0.5 * n) as i64).clamp(0, self.n_f as i64 - 1);
        self.lookup(f.0 as usize, i, j)
    }
}

impl Topo for Region {
    fn len(&self) -> usize {
        self.cells.len()
    }
    fn nbrs(&self, c: usize) -> [u32; 8] {
        let (f, fi, fj) = self.cells[c];
        let (f, fi, fj, n) = (f as usize, fi as i64, fj as i64, self.n_f as i64);
        let mut out = [NONE; 8];
        for (k, &(di, dj)) in OFFS.iter().enumerate() {
            let (ni, nj) = (fi + di, fj + dj);
            out[k] = if ni >= 0 && ni < n && nj >= 0 && nj < n {
                self.lookup(f, ni, nj)
            } else {
                let s = -1.0 + (2.0 * ni as f64 + 1.0) / n as f64;
                let t = -1.0 + (2.0 * nj as f64 + 1.0) / n as f64;
                let d = face_to_direction(&TangentWarp, Face(f as u8), s, t);
                self.find_dir(d)
            };
        }
        out
    }
    fn h(&self, c: usize) -> f32 {
        self.area[c].sqrt() * 1000.0
    }
    fn area_km2(&self, c: usize) -> f32 {
        self.area[c]
    }
}

// ---------------------------------------------------------------- injection of the coarse flux

fn coarse_weights(cs: &Coarse, d: usize, nbd: &[u32; 8]) -> [f32; 8] {
    let fd = cs.filled[d];
    let mut w = [0f32; 8];
    let mut sum = 0f32;
    for m in 0..8 {
        if nbd[m] == NONE {
            continue;
        }
        let n = nbd[m] as usize;
        if cs.filled[n] < fd {
            let s = (fd - cs.filled[n]) / cs.sph.dist(d, m, n);
            w[m] = s.powf(1.1);
            sum += w[m];
        }
    }
    if sum > 0.0 {
        for v in w.iter_mut() {
            *v /= sum;
        }
    }
    w
}

#[derive(Default, Clone, Copy)]
pub struct InjStats {
    pub injected_km2: f64,
    pub lost_pairs: u64,
    pub pairs: u64,
}

/// The coarse flux that enters the region through the edges of its coarse cells, spread over the fine cells of the side:
/// weights `((zmax - z) + 1)^p` (p = 0 is uniform, larger p concentrates the flux in the lowest cells of the side, the valley).
pub fn inject(region: &Region, cs: &Coarse, z: &[f32], p_exp: f32, st: &mut InjStats) -> Vec<f32> {
    let k = region.k;
    let mut inflow = vec![0f32; region.len()];
    for &c in &region.ccells {
        let (f, ci, cj) = cs.sph.split(c as usize);
        let nb = cs.sph.nbrs(c as usize);
        for kk in 0..8 {
            let d = nb[kk];
            if d == NONE || region.cset.contains(&d) {
                continue;
            }
            st.pairs += 1;
            let nbd = cs.sph.nbrs(d as usize);
            let Some(m) = nbd.iter().position(|&x| x == c) else {
                st.lost_pairs += 1;
                continue;
            };
            let w = coarse_weights(cs, d as usize, &nbd)[m];
            if w <= 0.0 {
                continue;
            }
            let flux = cs.a[d as usize] * w;
            let (di, dj) = OFFS[kk];
            let ar = match di {
                -1 => 0..1,
                1 => k - 1..k,
                _ => 0..k,
            };
            let br = match dj {
                -1 => 0..1,
                1 => k - 1..k,
                _ => 0..k,
            };
            let mut side: Vec<usize> = Vec::new();
            for b in br.clone() {
                for a in ar.clone() {
                    let idx = region.lookup(f, (ci * k + a) as i64, (cj * k + b) as i64);
                    if idx != NONE {
                        side.push(idx as usize);
                    }
                }
            }
            let zmax = side.iter().map(|&i| z[i]).fold(f32::MIN, f32::max);
            let wts: Vec<f32> = side.iter().map(|&i| (zmax - z[i] + 1.0).powf(p_exp)).collect();
            let sum: f32 = wts.iter().sum();
            for (&i, w) in side.iter().zip(&wts) {
                inflow[i] += flux * w / sum;
            }
            st.injected_km2 += flux as f64;
        }
    }
    inflow
}

// ---------------------------------------------------------------- solving a region

pub struct Sol {
    pub a: Vec<f32>,
    pub z: Vec<f32>,
    pub soil: Vec<u8>,
}

pub fn solve(region: &Region, cs: &Coarse, hf: &(dyn Fn(Vec3) -> f32 + Sync), threads: usize, p_exp: f32, st: &mut InjStats) -> Sol {
    let Fields { z, lith } = run_parallel(region.len(), threads, |c| {
        let d = region.dir(c);
        (hf(d), (lithology(d) * 255.0) as u8)
    });
    let (filled, order, _) = priority_flood(region, &z);
    let inflow = inject(region, cs, &z, p_exp, st);
    let a = accumulate(region, &filled, &order, Route::Mfd, Some(&inflow));
    let dv = derive(region, &z, &a, &lith);
    Sol { a, z, soil: dv.soil }
}

pub struct Tiling {
    pub p: i64,
    pub oi: i64,
    pub oj: i64,
    pub halo: usize,
}

pub struct Mosaic {
    pub a: Vec<f32>,
    pub soil: Vec<u8>,
    pub grp: Vec<u32>,
    pub bd: Vec<u32>,
    pub patches: usize,
    pub secs: f64,
    pub cells_solved: usize,
}

pub fn mosaic(truth: &Region, cs: &Coarse, hf: &(dyn Fn(Vec3) -> f32 + Sync), threads: usize, til: &Tiling, p_exp: f32, st: &mut InjStats) -> Mosaic {
    let t0 = Instant::now();
    let k = truth.k as i64;
    let mut groups: BTreeMap<(usize, i64, i64), Vec<u32>> = BTreeMap::new();
    for &c in &truth.ccells {
        let (f, ci, cj) = cs.sph.split(c as usize);
        groups.entry((f, (ci as i64 - til.oi).div_euclid(til.p), (cj as i64 - til.oj).div_euclid(til.p))).or_default().push(c);
    }
    let n = truth.len();
    let mut m = Mosaic { a: vec![0.0; n], soil: vec![0; n], grp: vec![0; n], bd: vec![0; n], patches: groups.len(), secs: 0.0, cells_solved: 0 };
    for (gid, (&(f, gi, gj), core)) in groups.iter().enumerate() {
        let mut set: HashSet<u32> = core.iter().copied().collect();
        let mut all = core.clone();
        let mut frontier = core.clone();
        for _ in 0..til.halo {
            let mut next = Vec::new();
            for &c in &frontier {
                for nn in cs.sph.nbrs(c as usize) {
                    if nn != NONE && set.insert(nn) {
                        all.push(nn);
                        next.push(nn);
                    }
                }
            }
            frontier = next;
        }
        let pr = Region::new(&cs.sph, all, truth.k);
        m.cells_solved += pr.len();
        let sol = solve(&pr, cs, hf, threads, p_exp, st);
        let (lo_i, lo_j) = ((til.oi + gi * til.p) * k, (til.oj + gj * til.p) * k);
        let hi = til.p * k;
        for &c in core {
            let (_, ci, cj) = cs.sph.split(c as usize);
            for b in 0..truth.k {
                for a in 0..truth.k {
                    let (fi, fj) = ((ci * truth.k + a) as i64, (cj * truth.k + b) as i64);
                    let ti = truth.lookup(f, fi, fj) as usize;
                    let pi_ = pr.lookup(f, fi, fj) as usize;
                    m.a[ti] = sol.a[pi_];
                    m.soil[ti] = sol.soil[pi_];
                    m.grp[ti] = gid as u32;
                    let (x, y) = (fi - lo_i, fj - lo_j);
                    m.bd[ti] = x.min(hi - 1 - x).min(y).min(hi - 1 - y).max(0) as u32;
                }
            }
        }
    }
    m.secs = t0.elapsed().as_secs_f64();
    m
}

// ---------------------------------------------------------------- region choice

/// A point on a cube edge in mountains: the best relief among points whose 45 km neighbourhood is all land above 300 m.
fn find_center() -> (Vec3, Vec3) {
    let mut best = (-1.0f64, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let axes = [(0usize, 1usize, 2usize), (0, 2, 1), (1, 2, 0)];
    let delta = 45_000.0 / R_M;
    for &(a, b, c) in &axes {
        for sa in [-1.0, 1.0] {
            for sb in [-1.0, 1.0] {
                for ti in 0..800 {
                    let t = -1.0 + 2.0 * (ti as f64 + 0.5) / 800.0;
                    let mut v = [0.0f64; 3];
                    v[a] = sa;
                    v[b] = sb;
                    v[c] = t;
                    let d = Vec3::new(v[0], v[1], v[2]).normalized();
                    let mut w = [0.0f64; 3];
                    w[c] = 1.0;
                    let tangent = Vec3::new(w[0], w[1], w[2]);
                    let e1 = (tangent - d * tangent.dot(d)).normalized();
                    let e2 = d.cross(e1);
                    let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
                    for i in -2..=2 {
                        for j in -2..=2 {
                            let q = (d + (e1 * (i as f64 * delta / 2.0) + e2 * (j as f64 * delta / 2.0))).normalized();
                            let e = sample(seed(), q).elevation_m as f64;
                            zmin = zmin.min(e);
                            zmax = zmax.max(e);
                        }
                    }
                    if zmin > 300.0 && zmax - zmin > best.0 {
                        best = (zmax - zmin, d, e1);
                    }
                }
            }
        }
    }
    println!("REGION centre: relief {:.0} m, direction {:?}, lat {:.2} lon {:.2}", best.0, best.1, libm::asin(best.1.z).to_degrees(), libm::atan2(best.1.y, best.1.x).to_degrees());
    (best.1, best.2)
}

/// The coarse cells whose centres lie within the square of half-width `half_m` (gnomonic plane at `c`, axes `e1`, `e2`), connected to the centre cell.
fn region_cells(cs: &Coarse, c: Vec3, e1: Vec3, half_m: f64) -> Vec<u32> {
    let e2 = c.cross(e1);
    let inside = |idx: usize| {
        let (f, i, j) = cs.sph.split(idx);
        let d = cs.sph.direction(f, i, j);
        let q = d.dot(c);
        q > 0.0 && libm::fabs(d.dot(e1) / q) * R_M <= half_m && libm::fabs(d.dot(e2) / q) * R_M <= half_m
    };
    let start = cs.sph.cell_of(c);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut out = vec![];
    let mut stack = vec![start as u32];
    seen.insert(start as u32);
    while let Some(x) = stack.pop() {
        if !inside(x as usize) {
            continue;
        }
        out.push(x);
        for nn in cs.sph.nbrs(x as usize) {
            if nn != NONE && seen.insert(nn) {
                stack.push(nn);
            }
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------- metrics

fn pct(v: &mut [f32], q: f64) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q) as usize]
}

fn err_report(label: &str, err: &[(f32, u32)]) {
    let buckets: [(&str, u32, u32); 5] = [("all", 0, u32::MAX), ("<5 cells from a patch edge", 0, 5), ("5-20", 5, 20), ("20-60", 20, 60), (">=60", 60, u32::MAX)];
    for (name, lo, hi) in buckets {
        let mut v: Vec<f32> = err.iter().filter(|e| e.1 >= lo && e.1 < hi).map(|e| e.0).collect();
        if v.is_empty() {
            continue;
        }
        let mean = v.iter().map(|&x| x as f64).sum::<f64>() / v.len() as f64;
        println!(
            "  {label} |d ln A| [{name}] n={} mean {:.3} median {:.3} p90 {:.3} p99 {:.3} max {:.2}",
            v.len(),
            mean,
            pct(&mut v, 0.5),
            pct(&mut v, 0.9),
            pct(&mut v, 0.99),
            pct(&mut v, 1.0)
        );
    }
}

fn near(region: &Region, set: &[bool], c: usize, r: i64) -> bool {
    let (f, fi, fj) = region.cells[c];
    for dj in -r..=r {
        for di in -r..=r {
            let n = region.lookup(f as usize, fi as i64 + di, fj as i64 + dj);
            if n != NONE && set[n as usize] {
                return true;
            }
        }
    }
    false
}

fn top_set(land: &[usize], a: &[f32], q: f64, n: usize) -> (Vec<bool>, f32) {
    let mut v: Vec<f32> = land.iter().map(|&i| a[i]).collect();
    let thr = pct(&mut v, q);
    let mut s = vec![false; n];
    for &i in land {
        if a[i] >= thr {
            s[i] = true;
        }
    }
    (s, thr)
}

fn compare_truth(label: &str, region: &Region, truth: &Sol, m: &Mosaic) {
    let n = region.len();
    let land: Vec<usize> = (0..n).filter(|&i| truth.z[i] > 0.0).collect();
    println!("COMPARE {label}: patches {}, cells solved {} ({:.2}x the region), {:.1}s", m.patches, m.cells_solved, m.cells_solved as f64 / n as f64, m.secs);
    let err: Vec<(f32, u32)> = land.iter().map(|&i| ((m.a[i].ln() - truth.a[i].ln()).abs(), m.bd[i])).collect();
    err_report("wetness (= ln A; the slope term is identical)", &err);
    let serr: Vec<f32> = land.iter().map(|&i| (soil_decode(m.soil[i]) - soil_decode(truth.soil[i])).abs()).collect();
    let mut sv = serr.clone();
    println!("  soil depth |error| m: mean {:.4} median {:.4} p90 {:.4} p99 {:.4}", sv.iter().map(|&x| x as f64).sum::<f64>() / sv.len() as f64, pct(&mut sv, 0.5), pct(&mut sv, 0.9), pct(&mut sv, 0.99));
    // main river courses: top 1 % and 0.1 % of the land cells
    for q in [0.99, 0.999] {
        let (tt, thr_t) = top_set(&land, &truth.a, q, n);
        let (tm, thr_m) = top_set(&land, &m.a, q, n);
        let nt = tt.iter().filter(|&&x| x).count();
        let nm = tm.iter().filter(|&&x| x).count();
        for r in [0i64, 2] {
            let rec = land.iter().filter(|&&i| tt[i] && near(region, &tm, i, r)).count() as f64 / nt.max(1) as f64;
            let prec = land.iter().filter(|&&i| tm[i] && near(region, &tt, i, r)).count() as f64 / nm.max(1) as f64;
            println!("  top {:.1} % flow cells (truth thr {:.1} km2, mosaic thr {:.1} km2; {} / {} cells), match within {} cells: recall {:.3} precision {:.3}", (1.0 - q) * 100.0, thr_t, thr_m, nt, nm, r, rec, prec);
        }
    }
    // absolute rivers: upstream area of 25 km2 and more
    {
        let (at, am): (Vec<bool>, Vec<bool>) = ((0..n).map(|i| truth.z[i] > 0.0 && truth.a[i] >= 25.0).collect(), (0..n).map(|i| truth.z[i] > 0.0 && m.a[i] >= 25.0).collect());
        let nt = at.iter().filter(|&&x| x).count();
        let nm = am.iter().filter(|&&x| x).count();
        let rec = land.iter().filter(|&&i| at[i] && near(region, &am, i, 2)).count() as f64 / nt.max(1) as f64;
        let prec = land.iter().filter(|&&i| am[i] && near(region, &at, i, 2)).count() as f64 / nm.max(1) as f64;
        println!("  rivers A >= 25 km2: truth {nt} cells, mosaic {nm} cells; within 2 cells recall {rec:.3} precision {prec:.3}");
    }
    // continuity across patch borders: a river cell whose lower orthogonal neighbour lies in another patch
    let mut cross = 0u64;
    let mut brk_m = 0u64;
    let mut brk_t = 0u64;
    let (mut jm_in, mut jm_x, mut jt_in, mut jt_x, mut n_in, mut n_x) = (0f64, 0f64, 0f64, 0f64, 0u64, 0u64);
    let (_, thr_m) = top_set(&land, &m.a, 0.99, n);
    for &c in &land {
        let nb = region.nbrs(c);
        for k in [1usize, 3] {
            let o = nb[k];
            if o == NONE || truth.z[o as usize] <= 0.0 {
                continue;
            }
            let o = o as usize;
            let jm = (m.a[c].ln() - m.a[o].ln()).abs() as f64;
            let jt = (truth.a[c].ln() - truth.a[o].ln()).abs() as f64;
            if m.grp[c] != m.grp[o] {
                jm_x += jm;
                jt_x += jt;
                n_x += 1;
                let (up, dn) = if truth.z[c] > truth.z[o] { (c, o) } else { (o, c) };
                if m.a[up] >= thr_m {
                    cross += 1;
                    if m.a[dn] < 0.5 * m.a[up] {
                        brk_m += 1;
                    }
                    if truth.a[dn] < 0.5 * truth.a[up] {
                        brk_t += 1;
                    }
                }
            } else {
                jm_in += jm;
                jt_in += jt;
                n_in += 1;
            }
        }
    }
    println!(
        "  continuity: neighbour step in ln A across patch borders {:.3} (truth {:.3}), inside patches {:.3} (truth {:.3}); river cells (mosaic top 1 %) at a border whose lower neighbour has less than half the area: {} of {} (truth at the same pairs: {})",
        jm_x / n_x.max(1) as f64,
        jt_x / n_x.max(1) as f64,
        jm_in / n_in.max(1) as f64,
        jt_in / n_in.max(1) as f64,
        brk_m,
        cross,
        brk_t
    );
}

fn compare_tilings(label: &str, region: &Region, truth: &Sol, a: &Mosaic, b: &Mosaic) {
    let land: Vec<usize> = (0..region.len()).filter(|&i| truth.z[i] > 0.0).collect();
    let err: Vec<(f32, u32)> = land.iter().map(|&i| ((a.a[i].ln() - b.a[i].ln()).abs(), a.bd[i].min(b.bd[i]))).collect();
    println!("PARTITION-INVARIANCE {label}");
    err_report("tiling A vs tiling B", &err);
}

// ---------------------------------------------------------------- images

fn lnramp(a: f32) -> [u8; 3] {
    ramp((a.max(1e-4).ln() - (0.01f32).ln()) / ((1.0e5f32).ln() - (0.01f32).ln()))
}
fn diffcol(d: f32) -> [u8; 3] {
    let t = (d / 2.0).clamp(-1.0, 1.0);
    if t >= 0.0 {
        [255, (255.0 * (1.0 - t)) as u8, (255.0 * (1.0 - t)) as u8]
    } else {
        [(255.0 * (1.0 + t)) as u8, (255.0 * (1.0 + t)) as u8, 255]
    }
}

fn render(region: &Region, c: Vec3, e1: Vec3, half_m: f64, truth: &Sol, m: &Mosaic, path: &str, seams: bool) {
    let e2 = c.cross(e1);
    let s = 400usize;
    let gap = 4usize;
    let mut rgb = vec![30u8; (3 * s + 2 * gap) * s * 3];
    for y in 0..s {
        for x in 0..s {
            let (u, v) = (((x as f64 + 0.5) / s as f64 * 2.0 - 1.0) * half_m / R_M, (1.0 - (y as f64 + 0.5) / s as f64 * 2.0) * half_m / R_M);
            let d = (c + e1 * u + e2 * v).normalized();
            let idx = region.find_dir(d);
            if idx == NONE {
                continue;
            }
            let i = idx as usize;
            if truth.z[i] <= 0.0 {
                continue;
            }
            let panels = [lnramp(truth.a[i]), lnramp(m.a[i]), diffcol(m.a[i].ln() - truth.a[i].ln())];
            for (p, col) in panels.iter().enumerate() {
                let o = (y * (3 * s + 2 * gap) + p * (s + gap) + x) * 3;
                let mut col = *col;
                if seams && p == 1 && m.bd[i] == 0 {
                    col = [(col[0] as u16 * 6 / 10) as u8, (col[1] as u16 * 6 / 10) as u8, (col[2] as u16 * 6 / 10) as u8];
                }
                rgb[o..o + 3].copy_from_slice(&[col[0] & 0xf8, col[1] & 0xf8, col[2] & 0xf8]);
            }
        }
    }
    save_png(path, (3 * s + 2 * gap) as u32, s as u32, &rgb);
}

// ---------------------------------------------------------------- the experiment

fn cells_for(km: f64, n_c: usize) -> i64 {
    (km * 1000.0 / (R_M * core::f64::consts::FRAC_PI_2 / n_c as f64)).round() as i64
}

pub fn run(n_c: usize, threads: usize, out: &str) {
    with_field(threads, |fld| {
        let k = 98304 / n_c; // fine cells per coarse cell edge: about 100 m
        println!("TWOSCALE n_c={n_c} k={k} fine grid {} cells per face edge ({:.0} m)", n_c * k, R_M * core::f64::consts::FRAC_PI_2 / (n_c * k) as f64);
        let (c0, e1) = find_center();
        let eroded = |d: Vec3| fld.height(d) as f32;
        let base = |d: Vec3| sample(seed(), d).elevation_m;
        for (cname, coarse_hf) in [("coarse-from-base-heights", &base as &(dyn Fn(Vec3) -> f32 + Sync)), ("coarse-from-eroded-heights", &eroded as &(dyn Fn(Vec3) -> f32 + Sync))] {
            println!("==== {cname} ====");
            let cs = coarse_bake(n_c, threads, coarse_hf, cname);
            let half = 50_000.0;
            let cells = region_cells(&cs, c0, e1, half);
            let region = Region::new(&cs.sph, cells, k);
            let faces: Vec<usize> = (0..6).filter(|&f| region.maps[f].w > 0).collect();
            println!("REGION {} coarse cells, {} fine cells, faces {:?}", region.ccells.len(), region.len(), faces);
            // ground truth: the whole region at once, inflow from the coarse bake
            let t = Instant::now();
            let mut st = InjStats::default();
            let truth = solve(&region, &cs, &eroded, threads, 2.0, &mut st);
            println!("TRUTH solved in {:.1}s: injected {:.0} km2 through {} coarse edges ({} lost to one-sided neighbours)", t.elapsed().as_secs_f64(), st.injected_km2, st.pairs, st.lost_pairs);
            let land = (0..region.len()).filter(|&i| truth.z[i] > 0.0).count();
            let amax = truth.a.iter().cloned().fold(0.0f32, f32::max);
            println!("TRUTH land {:.1} % of the region, max upstream area {:.0} km2", 100.0 * land as f64 / region.len() as f64, amax);
            let (p10, p25) = (cells_for(10.0, n_c), cells_for(25.0, n_c));
            let tilings: Vec<(&str, Tiling, f32)> = vec![
                ("10km-halo0", Tiling { p: p10, oi: 0, oj: 0, halo: 0 }, 2.0),
                ("10km-halo0-offset", Tiling { p: p10, oi: p10 / 2, oj: p10 / 2, halo: 0 }, 2.0),
                ("10km-halo1", Tiling { p: p10, oi: 0, oj: 0, halo: 1 }, 2.0),
                ("10km-halo1-offset", Tiling { p: p10, oi: p10 / 2, oj: p10 / 2, halo: 1 }, 2.0),
                ("10km-halo2", Tiling { p: p10, oi: 0, oj: 0, halo: 2 }, 2.0),
                ("10km-halo2-offset", Tiling { p: p10, oi: p10 / 2, oj: p10 / 2, halo: 2 }, 2.0),
                ("25km-halo0", Tiling { p: p25, oi: 0, oj: 0, halo: 0 }, 2.0),
                ("25km-halo1", Tiling { p: p25, oi: 0, oj: 0, halo: 1 }, 2.0),
                ("25km-halo2", Tiling { p: p25, oi: 0, oj: 0, halo: 2 }, 2.0),
                ("10km-halo1-uniform-injection", Tiling { p: p10, oi: 0, oj: 0, halo: 1 }, 0.0),
                ("10km-halo1-injection-p4", Tiling { p: p10, oi: 0, oj: 0, halo: 1 }, 4.0),
            ];
            let mut results: Vec<(String, Mosaic)> = Vec::new();
            for (name, til, p_exp) in tilings {
                println!("-- tiling {name}: patches of {} coarse cells ({:.1} km), offset ({}, {}), halo {} coarse cells, injection exponent {}", til.p, til.p as f64 * R_M * core::f64::consts::FRAC_PI_2 / n_c as f64 / 1000.0, til.oi, til.oj, til.halo, p_exp);
                let mut st = InjStats::default();
                let m = mosaic(&region, &cs, &eroded, threads, &til, p_exp, &mut st);
                compare_truth(&format!("{cname} {name}"), &region, &truth, &m);
                results.push((name.to_string(), m));
            }
            let get = |n: &str| &results.iter().find(|r| r.0 == n).unwrap().1;
            compare_tilings(&format!("{cname}: 10 km tiling against the same tiling shifted by half a patch, halo 0"), &region, &truth, get("10km-halo0"), get("10km-halo0-offset"));
            compare_tilings(&format!("{cname}: 10 km tiling against the shifted tiling, halo 1 coarse cell"), &region, &truth, get("10km-halo1"), get("10km-halo1-offset"));
            compare_tilings(&format!("{cname}: 10 km tiling against the shifted tiling, halo 2 coarse cells"), &region, &truth, get("10km-halo2"), get("10km-halo2-offset"));
            compare_tilings(&format!("{cname}: 10 km against 25 km tiling, halo 1"), &region, &truth, get("10km-halo1"), get("25km-halo1"));
            compare_tilings(&format!("{cname}: 10 km against 25 km tiling, halo 2"), &region, &truth, get("10km-halo2"), get("25km-halo2"));
            {
                // the cap of the coarse inflow on the truth itself: the same 100 km region cut out of a 200 km solve (inflow from the coarse bake only at the 200 km border)
                let big_cells = region_cells(&cs, c0, e1, 100_000.0);
                let big = Region::new(&cs.sph, big_cells, k);
                let t = Instant::now();
                let mut st = InjStats::default();
                let bs = solve(&big, &cs, &eroded, threads, 2.0, &mut st);
                println!("BIG-TRUTH {} fine cells solved in {:.1}s", big.len(), t.elapsed().as_secs_f64());
                let n = region.len();
                let mut m = Mosaic { a: vec![0.0; n], soil: vec![0; n], grp: vec![0; n], bd: vec![0; n], patches: 1, secs: t.elapsed().as_secs_f64(), cells_solved: big.len() };
                // distance of a coarse cell to the border of the 100 km region, in coarse rings
                let mut ring: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
                let mut frontier: Vec<u32> = region.ccells.iter().copied().filter(|&c| cs.sph.nbrs(c as usize).iter().any(|&x| x != NONE && !region.cset.contains(&x))).collect();
                for &c in &frontier {
                    ring.insert(c, 0);
                }
                let mut r = 0;
                while !frontier.is_empty() {
                    r += 1;
                    let mut next = vec![];
                    for &c in &frontier {
                        for x in cs.sph.nbrs(c as usize) {
                            if x != NONE && region.cset.contains(&x) && !ring.contains_key(&x) {
                                ring.insert(x, r);
                                next.push(x);
                            }
                        }
                    }
                    frontier = next;
                }
                for i in 0..n {
                    let (f, fi, fj) = region.cells[i];
                    let j = big.lookup(f as usize, fi as i64, fj as i64) as usize;
                    m.a[i] = bs.a[j];
                    m.soil[i] = bs.soil[j];
                    let c = cs.sph.cell_of(region.dir(i)) as u32;
                    m.bd[i] = ring.get(&c).copied().unwrap_or(0) * k as u32;
                }
                compare_truth(&format!("{cname} BIG-TRUTH (200 km solve, cut to the 100 km region; the border distance is to the 100 km edge)"), &region, &truth, &m);
            }
            if cname.contains("base") {
                for name in ["10km-halo0", "10km-halo1", "10km-halo2", "25km-halo1"] {
                    render(&region, c0, e1, half, &truth, get(name), &format!("{out}/twoscale-{name}.png"), true);
                }
            } else {
                render(&region, c0, e1, half, &truth, get("10km-halo1"), &format!("{out}/twoscale-eroded-coarse-10km-halo1.png"), true);
            }
        }
    });
}

// ---------------------------------------------------------------- the cube-edge ratio

pub fn edgediag(n: usize, threads: usize) {
    let mut sph = Sphere::new(n);
    let Fields { z, lith: _ } = run_parallel(sph.len(), threads, |c| {
        let (f, i, j) = sph.split(c);
        (sample(seed(), sph.direction(f, i, j)).elevation_m, 0)
    });
    let len = sph.len();
    let (mut cross, mut asym, mut not_opposite) = (0u64, 0u64, 0u64);
    let (mut d_in, mut d_x, mut m_in, mut m_x, mut nn_in, mut nn_x) = (0f64, 0f64, 0f64, 0f64, 0u64, 0u64);
    let (mut dz_in, mut dz_x, mut sl_in, mut sl_x) = (0f64, 0f64, 0f64, 0f64);
    for c in 0..len {
        let nb = sph.nbrs(c);
        let (f, ci, cj) = sph.split(c);
        let a = sph.direction(f, ci, cj);
        for k in 0..4 {
            let m = nb[k] as usize;
            let (fm, mi, mj) = sph.split(m);
            let b = sph.direction(fm, mi, mj);
            let exact = a.distance(b) * R_M;
            let model = 0.5 * (sph.h(c) as f64 + sph.h(m) as f64);
            let is_x = fm != f;
            if is_x {
                cross += 1;
                let back = sph.nbrs(m);
                match back.iter().position(|&x| x as usize == c) {
                    None => asym += 1,
                    Some(pos) => {
                        if pos != (k ^ 1) {
                            not_opposite += 1
                        }
                    }
                }
            }
            if z[c] > 0.0 && z[m] > 0.0 {
                let dz = (z[c] - z[m]).abs() as f64;
                if is_x {
                    d_x += exact;
                    m_x += model;
                    dz_x += dz;
                    sl_x += dz / exact;
                    nn_x += 1;
                } else {
                    d_in += exact;
                    m_in += model;
                    dz_in += dz;
                    sl_in += dz / exact;
                    nn_in += 1;
                }
            }
        }
    }
    println!("EDGE-DIAG n={n}: orthogonal pairs across a face edge {cross}; one-sided (the neighbour does not list the cell back) {asym} ({:.3} %); listed back at a non-opposite index {not_opposite} ({:.3} %)", 100.0 * asym as f64 / cross as f64, 100.0 * not_opposite as f64 / cross as f64);
    println!("EDGE-DIAG land pairs inside a face {nn_in}, across an edge {nn_x}: mean exact centre distance {:.1} m inside, {:.1} m across (ratio {:.3}); the model (sqrt of the areas) {:.1} m inside, {:.1} m across (ratio {:.3}); exact over model: inside {:.3}, across {:.3}", d_in / nn_in as f64, d_x / nn_x as f64, (d_x / nn_x as f64) / (d_in / nn_in as f64), m_in / nn_in as f64, m_x / nn_x as f64, (m_x / nn_x as f64) / (m_in / nn_in as f64), d_in / m_in, d_x / m_x);
    println!("EDGE-DIAG mean |dz| of land pairs: inside {:.2} m, across {:.2} m (ratio {:.3}); mean |dz| / exact distance: inside {:.5}, across {:.5} (ratio {:.3})", dz_in / nn_in as f64, dz_x / nn_x as f64, (dz_x / nn_x as f64) / (dz_in / nn_in as f64), sl_in / nn_in as f64, sl_x / nn_x as f64, (sl_x / nn_x as f64) / (sl_in / nn_in as f64));
    let (filled, order, _) = priority_flood(&sph, &z);
    for (name, route) in [("MFD", Route::Mfd), ("D8", Route::D8)] {
        for exact in [false, true] {
            sph.exact = exact;
            let a = accumulate(&sph, &filled, &order, route, None);
            let (mut s_in, mut s_x, mut c_in, mut c_x) = (0f64, 0f64, 0u64, 0u64);
            let (mut s_rim, mut c_rim) = (0f64, 0u64);
            for c in 0..len {
                if z[c] <= 0.0 {
                    continue;
                }
                let (f, i, j) = sph.split(c);
                let nb = sph.nbrs(c);
                for k in 0..4 {
                    let m = nb[k] as usize;
                    if z[m] <= 0.0 {
                        continue;
                    }
                    let (fm, _, _) = sph.split(m);
                    let dl = (a[c].ln() - a[m].ln()).abs() as f64;
                    if fm != f {
                        s_x += dl;
                        c_x += 1;
                    } else {
                        s_in += dl;
                        c_in += 1;
                        if i == 0 || j == 0 || i == n - 1 || j == n - 1 {
                            s_rim += dl;
                            c_rim += 1;
                        }
                    }
                }
            }
            println!(
                "EDGE-DIAG {name} distances {}: mean |dlnA| inside a face {:.4}, across an edge {:.4} (ratio {:.3}); same-face pairs touching the outermost row {:.4} (ratio of across to those {:.3})",
                if exact { "exact" } else { "model" },
                s_in / c_in as f64,
                s_x / c_x as f64,
                (s_x / c_x as f64) / (s_in / c_in as f64),
                s_rim / c_rim.max(1) as f64,
                (s_x / c_x as f64) / (s_rim / c_rim.max(1) as f64)
            );
        }
    }
}
