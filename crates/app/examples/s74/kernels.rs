//! SPIKE-74 (spike code, never merged): the kernels of SPIKE-70 to SPIKE-73 and SPIKE-64's deposition, for the pipeline run.
#![allow(dead_code, clippy::all)]
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::time::Instant;


#[allow(dead_code)]
pub const DX: f64 = 101.8;
const NB: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

#[derive(Clone, Copy)]
pub struct G<'a> {
    pub w: usize,
    pub ox: usize,
    pub oy: usize,
    /// Cells held at their height (a neighbour's baked core): base level, never eroded, no thermal exchange.
    pub fix: &'a [bool],
    /// Cell spacing (m).
    pub dx: f64,
    /// Forced receivers (u32::MAX: none) and their least drainage area (m2), per cell; empty for none (SPIKE-70 (a)).
    pub frec: &'a [u32],
    pub farea: &'a [f64],
    /// Cap on the drainage area (m2) the carve sees; 0 for none (SPIKE-70 hybrid: the local pass carves only small catchments).
    pub amax: f64,
}

impl G<'_> {
    fn n(&self) -> usize {
        self.w * self.w
    }
    fn nbr(&self, p: usize, d: usize) -> Option<usize> {
        let w = self.w as i64;
        let (i, j) = ((p % self.w) as i64 + NB[d].0, (p / self.w) as i64 + NB[d].1);
        (i >= 0 && j >= 0 && i < w && j < w).then(|| j as usize * self.w + i as usize)
    }
    fn edge(&self, p: usize) -> bool {
        let (i, j) = (p % self.w, p / self.w);
        i == 0 || j == 0 || i == self.w - 1 || j == self.w - 1 || (!self.fix.is_empty() && self.fix[p])
    }
    fn border(&self, p: usize, h: &[f32]) -> bool {
        self.edge(p) || h[p] <= 0.0
    }
    /// The global cell of a window cell (big-window coordinates).
    fn gid(&self, p: usize) -> u64 {
        (((self.oy + p / self.w) as u64) << 20) | (self.ox + p % self.w) as u64
    }
}

pub fn read(path: &str) -> Vec<f32> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub fn mix(x: u64) -> f64 {
    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = x ^ (x >> 29);
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

pub fn pair_u(a: u64, b: u64) -> f64 {
    let (a, b) = if a < b { (a, b) } else { (b, a) };
    mix(a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
}

pub fn thermal(g: G, h: &mut Vec<f32>, theta_deg: f64, passes: usize, threads: usize) {
    const K: f64 = 0.0625;
    let t = theta_deg.to_radians().tan();
    let w = g.w;
    let mut next = h.clone();
    for _ in 0..passes {
        let rows_per = w.div_ceil(threads);
        let cur = &*h;
        std::thread::scope(|sc| {
            for (k, out) in next.chunks_mut(rows_per * w).enumerate() {
                sc.spawn(move || {
                    for (o, v) in out.iter_mut().enumerate() {
                        let p = k * rows_per * w + o;
                        let hp = f64::from(cur[p]);
                        let mut dh = 0.0;
                        let edge = g.edge(p);
                        for d in 0..8 {
                            let Some(q) = g.nbr(p, d) else { continue };
                            let hq = f64::from(cur[q]);
                            let dist = if d < 4 { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
                            let crit = t * (1.0 + 0.25 * pair_u(g.gid(p), g.gid(q))) * dist;
                            let diff = hp - hq;
                            if diff > crit && hp > 0.0 && !edge {
                                dh -= K * (diff - crit);
                            } else if -diff > crit && hq > 0.0 && !g.edge(q) {
                                dh += K * (-diff - crit);
                            }
                        }
                        *v = (hp + dh) as f32;
                    }
                });
            }
        });
        std::mem::swap(h, &mut next);
    }
}

/// The new height of cell `p` in one thermal pass (the gather of `thermal`).
pub fn thermal_cell(g: G, cur: &[f32], p: usize, t: f64) -> f32 {
    const K: f64 = 0.0625;
    let hp = f64::from(cur[p]);
    let mut dh = 0.0;
    let edge = g.edge(p);
    for d in 0..8 {
        let Some(q) = g.nbr(p, d) else { continue };
        let hq = f64::from(cur[q]);
        let dist = if d < 4 { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
        let crit = t * (1.0 + 0.25 * pair_u(g.gid(p), g.gid(q))) * dist;
        let diff = hp - hq;
        if diff > crit && hp > 0.0 && !edge {
            dh -= K * (diff - crit);
        } else if -diff > crit && hq > 0.0 && !g.edge(q) {
            dh += K * (-diff - crit);
        }
    }
    (hp + dh) as f32
}

/// Thermal on active cells: the same Jacobi passes as `thermal`, but a pass evaluates only the cells that changed in the pass before and their
/// eight neighbours (a cell whose neighbourhood did not change keeps its zero update), so the result is the full Jacobi's bit for bit. Stops early
/// when no cell changes. Returns (passes run, cells evaluated).
pub fn thermal_active(g: G, h: &mut [f32], theta_deg: f64, passes: usize, threads: usize) -> (usize, u64) {
    let t = theta_deg.to_radians().tan();
    let n = g.n();
    let mut cand: Vec<u32> = (0..n as u32).collect();
    let mut stamp = vec![0u32; n];
    let mut evaluated = 0u64;
    for pass in 0..passes {
        if cand.is_empty() {
            return (pass, evaluated);
        }
        evaluated += cand.len() as u64;
        let per = cand.len().div_ceil(threads);
        let cur = &*h;
        let changes: Vec<Vec<(u32, f32)>> = std::thread::scope(|sc| {
            let hs: Vec<_> = cand
                .chunks(per)
                .map(|c| {
                    sc.spawn(move || {
                        let mut out = Vec::new();
                        for &p in c {
                            let v = thermal_cell(g, cur, p as usize, t);
                            if v != cur[p as usize] {
                                out.push((p, v));
                            }
                        }
                        out
                    })
                })
                .collect();
            hs.into_iter().map(|x| x.join().unwrap()).collect()
        });
        let mark = pass as u32 + 1;
        for ch in &changes {
            for &(p, v) in ch {
                h[p as usize] = v;
            }
        }
        let mut next = Vec::new();
        for ch in &changes {
            for &(p, _) in ch {
                let p = p as usize;
                for q in std::iter::once(Some(p)).chain((0..8).map(|d| g.nbr(p, d))).flatten() {
                    if stamp[q] != mark {
                        stamp[q] = mark;
                        next.push(q as u32);
                    }
                }
            }
        }
        next.sort_unstable();
        cand = next;
    }
    (passes, evaluated)
}

pub fn flood(g: G, h: &[f32]) -> Vec<f32> {
    let key = |v: f32| -> u32 {
        let b = v.to_bits();
        if b & 0x8000_0000 != 0 { !b } else { b | 0x8000_0000 }
    };
    let n = g.n();
    let mut filled = vec![0f32; n];
    let mut seen = vec![false; n];
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
    let mut pit: VecDeque<u32> = VecDeque::new();
    for p in 0..n {
        if g.border(p, h) {
            filled[p] = h[p];
            seen[p] = true;
            heap.push(Reverse((key(h[p]), p as u32)));
        }
    }
    loop {
        let p = if let Some(p) = pit.pop_front() {
            p as usize
        } else if let Some(Reverse((_, p))) = heap.pop() {
            p as usize
        } else {
            break;
        };
        for d in 0..8 {
            let Some(q) = g.nbr(p, d) else { continue };
            if seen[q] {
                continue;
            }
            seen[q] = true;
            let lo = filled[p] + 1e-3;
            if h[q] <= lo {
                filled[q] = lo;
                pit.push_back(q as u32);
            } else {
                filled[q] = h[q];
                heap.push(Reverse((key(filled[q]), q as u32)));
            }
        }
    }
    filled
}

/// Receivers (jittered D8 on the filled surface, hashed on the global cell), the order outlets first, the area with the inflow added.
pub fn route(g: G, h: &[f32], inflow: &[(usize, f64)]) -> (Vec<u32>, Vec<u32>, Vec<f64>, Vec<f32>) {
    let filled = flood(g, h);
    let n = g.n();
    let mut rec = vec![u32::MAX; n];
    for p in 0..n {
        if g.border(p, h) {
            continue;
        }
        let mut best = (0.0f64, u32::MAX);
        for d in 0..8 {
            let q = g.nbr(p, d).unwrap();
            let dd = if d < 4 { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
            let mut s = f64::from(filled[p] - filled[q]) / dd;
            if s > 0.0 {
                s *= 1.0 + 0.5 * mix(g.gid(p).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (d as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
            }
            if s > best.0 {
                best = (s, q as u32);
            }
        }
        rec[p] = best.1;
    }
    if !g.frec.is_empty() {
        for p in 0..n {
            if g.frec[p] != u32::MAX && !g.border(p, h) {
                rec[p] = g.frec[p];
            }
        }
    }
    let mut ndon = vec![0u32; n + 1];
    for &r in &rec {
        if r != u32::MAX {
            ndon[r as usize + 1] += 1;
        }
    }
    for i in 0..n {
        ndon[i + 1] += ndon[i];
    }
    let mut fill = ndon.clone();
    let mut don = vec![0u32; n];
    for (p, &r) in rec.iter().enumerate() {
        if r != u32::MAX {
            don[fill[r as usize] as usize] = p as u32;
            fill[r as usize] += 1;
        }
    }
    drop(fill);
    let mut order = Vec::with_capacity(n);
    let mut stack: Vec<u32> = (0..n as u32).filter(|&p| rec[p as usize] == u32::MAX).collect();
    while let Some(p) = stack.pop() {
        order.push(p);
        stack.extend_from_slice(&don[ndon[p as usize] as usize..ndon[p as usize + 1] as usize]);
    }
    let mut area = vec![g.dx * g.dx; n];
    for &(p, a) in inflow {
        area[p] += a;
    }
    for &p in order.iter().rev() {
        let p = p as usize;
        if !g.farea.is_empty() && g.farea[p] > area[p] {
            area[p] = g.farea[p];
        }
        let r = rec[p];
        if r != u32::MAX {
            area[r as usize] += area[p];
        }
    }
    if g.amax > 0.0 {
        for a in area.iter_mut() {
            *a = a.min(g.amax);
        }
    }
    (rec, order, area, filled)
}

/// Land heights `m + k (h - m)`, `m` the box mean over 20 km (radius 98 cells, clamped to the window), at least 1 m.
pub fn amplify(w: usize, h: &[f32], k: f64) -> Vec<f32> {
    const R: usize = 98;
    let mut sat = vec![0f64; (w + 1) * (w + 1)];
    for j in 0..w {
        let mut row = 0.0;
        for i in 0..w {
            row += f64::from(h[j * w + i]);
            sat[(j + 1) * (w + 1) + i + 1] = sat[j * (w + 1) + i + 1] + row;
        }
    }
    let mut out = h.to_vec();
    for j in 0..w {
        for i in 0..w {
            let p = j * w + i;
            if h[p] <= 0.0 {
                continue;
            }
            let (i0, i1, j0, j1) = (i.saturating_sub(R), (i + R + 1).min(w), j.saturating_sub(R), (j + R + 1).min(w));
            let s = sat[j1 * (w + 1) + i1] - sat[j0 * (w + 1) + i1] - sat[j1 * (w + 1) + i0] + sat[j0 * (w + 1) + i0];
            let m = s / ((i1 - i0) * (j1 - j0)) as f64;
            out[p] = (m + k * (f64::from(h[p]) - m)).max(1.0) as f32;
        }
    }
    out
}

/// The transient carve with uplift and the breach before each fill (SPIKE-66 (d)).
pub fn carve(g: G, h: &mut [f32], inflow: &[(usize, f64)]) {
    carve_ring(g, h, inflow, None)
}

/// `ring`: (base, target) of the window: before each step the window's edge cells are set to `base + (target - base) s / steps`.
pub static UK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(20);
pub static LOCAL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn carve_ring(g: G, h: &mut [f32], inflow: &[(usize, f64)], ring: Option<(&[f32], &[f32])>) {
    let mode = if g.amax > 0.0 { LOCAL.load(std::sync::atomic::Ordering::Relaxed) } else { 0 };
    let (u_over_k, kdt, steps, reroute) = (f64::from(UK.load(std::sync::atomic::Ordering::Relaxed)), 0.05, 20, 5);
    let udt = u_over_k * kdt;
    let w = g.w;
    let mut routed = None;
    for s in 0..steps {
        if let Some((b0, b1)) = ring {
            let f = s as f32 / steps as f32;
            for p in 0..g.n() {
                if g.edge(p) {
                    h[p] = b0[p] + (b1[p] - b0[p]) * f;
                }
            }
        }
        if s % reroute == 0 {
            routed = Some(route(g, h, inflow));
            let (rec, order, _, _) = routed.as_ref().unwrap();
            for &p in order.iter().rev() {
                let p = p as usize;
                let r = rec[p];
                if mode == 2 || r == u32::MAX || h[r as usize] <= 0.0 || g.edge(r as usize) {
                    continue;
                }
                let r = r as usize;
                let dist = if (p % w) == (r % w) || (p / w) == (r / w) { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
                let want = f64::from(h[p]) - 1e-3 * dist;
                if f64::from(h[r]) > want {
                    h[r] = want.max(f64::from(h[r]) - 50.0) as f32;
                }
            }
        }
        let (rec, order, area, _) = routed.as_ref().unwrap();
        for &p in order.iter() {
            let p = p as usize;
            let r = rec[p];
            if r == u32::MAX {
                continue;
            }
            let r = r as usize;
            let dist = if (p % w) == (r % w) || (p / w) == (r / w) { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
            let f = kdt * area[p].sqrt() / dist;
            let hr = f64::from(h[r]);
            let hn = (f64::from(h[p]) + udt + f * hr) / (1.0 + f);
            if mode >= 1 && hr >= f64::from(h[p]) {
                // No fill: a cell whose receiver (on the flooded surface) is not below it stays; pits stay pits for the local carve.
                continue;
            }
            h[p] = hn.max(hr + 1e-3) as f32;
        }
    }
}






pub fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}


pub fn write(path: &str, h: &[f32]) {
    let b: Vec<u8> = h.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(path, b).unwrap();
}






/// SPIKE-64's deposition sweep on window `g`: the eroded column (`orig - h`) is carried down the routing of the current surface; it first fills each
/// cell's depression up to the filled level, then `zeta` of the load over the capacity `kc A^0.5 S` settles, up to half the drop to the receiver.
pub fn deposit(g: G, h: &mut [f32], orig: &[f32], kc: f64, zeta: f64) {
    let (rec, order, area, filled) = route(g, h, &[]);
    let n = g.n();
    let ca = g.dx * g.dx;
    let w = g.w;
    let mut qs = vec![0f64; n];
    for &p in order.iter().rev() {
        let p = p as usize;
        let e = (f64::from(orig[p]) - f64::from(h[p])).max(0.0) * ca;
        let mut load = qs[p] + if h[p] > 0.0 { e } else { 0.0 };
        let r = rec[p];
        if r == u32::MAX {
            continue;
        }
        let r = r as usize;
        let hp = f64::from(h[p]);
        let fp = f64::from(filled[p]);
        let pit = ((fp - hp).max(0.0) * ca).min(load);
        let mut rise = pit / ca;
        load -= pit;
        let dd = if (p % w) == (r % w) || (p / w) == (r / w) { g.dx } else { g.dx * std::f64::consts::SQRT_2 };
        let s = ((fp - f64::from(filled[r])) / dd).max(0.0);
        let cap = kc * area[p].sqrt() * s;
        if load > cap {
            let room = (0.5 * (fp - f64::from(filled[r])).max(0.0) + (fp - hp - rise).max(0.0)) * ca;
            let d = (zeta * (load - cap)).min(room);
            rise += d / ca;
            load -= d;
        }
        h[p] = (hp + rise) as f32;
        qs[r] += load;
    }
}
