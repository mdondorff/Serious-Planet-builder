//! SPIKE-43 step 2 throwaway: a fine channel network whose cell values depend only on cells within a stated radius (a bounded breach plus an accumulation of bounded depth),
//! against the unbounded priority-flood fill, on the A-net's solved ground.
//! usage: fine2 <dir> <site> [mask_dir]
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const NB: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
const NONE: u32 = u32::MAX;

struct Ground {
    per: usize,
    cell_m: f64,
    w: usize,
    z: Vec<f32>,
    zb: Vec<f32>,
}

fn load(path: &str) -> Ground {
    let b = std::fs::read(path).expect("ground file");
    let per = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let cell_m = f64::from_le_bytes(b[4..12].try_into().unwrap());
    let w = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let off = 28;
    let rd = |o: usize| b[o..o + 4 * w * w].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<f32>>();
    Ground { per, cell_m, w, z: rd(off), zb: rd(off + 4 * w * w) }
}

fn ordered(z: f32) -> u32 {
    let b = z.to_bits();
    if b >> 31 == 1 { !b } else { b | 0x8000_0000 }
}

fn rect(z: &[f32], ww: usize, x0: usize, y0: usize, w: usize, h: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        let r = (y0 + y) * ww + x0;
        v.extend_from_slice(&z[r..r + w]);
    }
    v
}

fn steepest(z: &[f64], w: usize, h: usize) -> Vec<u32> {
    let mut rec = vec![NONE; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = y * w + x;
            let (mut best, mut best_s) = (NONE, 0.0f64);
            for (dx, dy) in NB {
                let m = (y as i64 + dy) as usize * w + (x as i64 + dx) as usize;
                let d = z[c] - z[m];
                if d > 0.0 {
                    let s = if dx != 0 && dy != 0 { d * std::f64::consts::FRAC_1_SQRT_2 } else { d };
                    if s > best_s {
                        best_s = s;
                        best = m as u32;
                    }
                }
            }
            rec[c] = best;
        }
    }
    rec
}

/// The unbounded scheme of step 1: priority flood from the rectangle's border, flats routed by the flood.
fn flow_fill(z: &[f32], w: usize, h: usize) -> (Vec<u32>, Vec<f32>, usize) {
    let n = w * h;
    let mut zf: Vec<f64> = z.iter().map(|&v| f64::from(v)).collect();
    let mut tree = vec![NONE; n];
    let mut seen = vec![false; n];
    let mut order: Vec<u32> = Vec::with_capacity(n);
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    let mut seq: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                let c = y * w + x;
                seen[c] = true;
                heap.push(Reverse((u64::from(ordered(z[c])) << 32 | seq, c as u32)));
                seq += 1;
            }
        }
    }
    while let Some(Reverse((_, c))) = heap.pop() {
        order.push(c);
        let (cx, cy) = ((c as usize % w) as i64, (c as usize / w) as i64);
        let zc = zf[c as usize];
        for (dx, dy) in NB {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let m = ny as usize * w + nx as usize;
            if seen[m] {
                continue;
            }
            seen[m] = true;
            if zf[m] < zc {
                zf[m] = zc;
            }
            tree[m] = c;
            heap.push(Reverse((u64::from(ordered(zf[m] as f32)) << 32 | seq, m as u32)));
            seq += 1;
        }
    }
    let mut rec = steepest(&zf, w, h);
    let mut flats = 0;
    for c in 0..n {
        let (x, y) = (c % w, c / w);
        if x > 0 && y > 0 && x < w - 1 && y < h - 1 && rec[c] == NONE {
            rec[c] = tree[c];
            flats += 1;
        }
    }
    let mut acc = vec![1.0f32; n];
    for &c in order.iter().rev() {
        let r = rec[c as usize];
        if r != NONE {
            let a = acc[c as usize];
            acc[r as usize] += a;
        }
    }
    (rec, acc, flats)
}

#[derive(Clone, Copy)]
struct Params {
    rb: usize,
    depth: f64,
    l: usize,
}

struct Local {
    rec: Vec<u32>,
    acc: Vec<f32>,
    pits: usize,
    carved: usize,
    sinks: usize,
    ms: [f64; 3],
}

/// The bounded scheme. A pit (an interior cell with no strictly lower neighbour) looks for the nearest lower cell by a least-excess path inside the box of radius `rb` round it,
/// reading only the original heights, and is breached along that path by a strictly falling ramp if the cut is at most `depth` metres; otherwise it stays a sink. The carves are
/// combined by `min`. Receivers are the steepest strictly lower neighbour. The area of a cell counts the cells that reach it in at most `l` steps.
fn flow_local(z: &[f32], w: usize, h: usize, p: Params) -> Local {
    let t0 = std::time::Instant::now();
    let n = w * h;
    let mut z2: Vec<f64> = z.iter().map(|&v| f64::from(v)).collect();
    let (mut pits, mut carved, mut sinks) = (0, 0, 0);
    let side = 2 * p.rb + 1;
    let mut cost = vec![f64::INFINITY; side * side];
    let mut prev = vec![NONE; side * side];
    let mut touched: Vec<u32> = vec![];
    for c in 0..n {
        let (x, y) = (c % w, c / w);
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            continue;
        }
        let zc = f64::from(z[c]);
        let lower = NB.iter().any(|&(dx, dy)| f64::from(z[(y as i64 + dy) as usize * w + (x as i64 + dx) as usize]) < zc);
        if lower {
            continue;
        }
        pits += 1;
        let (bx, by) = (x as i64 - p.rb as i64, y as i64 - p.rb as i64);
        let loc = |gx: i64, gy: i64| ((gy - by) as usize) * side + (gx - bx) as usize;
        let mut heap: BinaryHeap<Reverse<(u64, u32, u32)>> = BinaryHeap::new();
        let mut seq = 0u32;
        cost[loc(x as i64, y as i64)] = 0.0;
        touched.push(loc(x as i64, y as i64) as u32);
        heap.push(Reverse((0, seq, c as u32)));
        let mut target = NONE;
        while let Some(Reverse((cb, _, g))) = heap.pop() {
            let (gx, gy) = ((g as usize % w) as i64, (g as usize / w) as i64);
            let gl = loc(gx, gy);
            if f64::from_bits(cb) > cost[gl] {
                continue;
            }
            if g as usize != c && f64::from(z[g as usize]) < zc - 0.01 {
                target = g;
                break;
            }
            for (dx, dy) in NB {
                let (nx, ny) = (gx + dx, gy + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 || (nx - x as i64).abs() > p.rb as i64 || (ny - y as i64).abs() > p.rb as i64 {
                    continue;
                }
                let m = ny as usize * w + nx as usize;
                let step = (f64::from(z[m]) - zc).max(0.0) + 1e-3;
                let nc = f64::from_bits(cb) + step;
                let ml = loc(nx, ny);
                if nc < cost[ml] {
                    if cost[ml].is_infinite() {
                        touched.push(ml as u32);
                    }
                    cost[ml] = nc;
                    prev[ml] = g;
                    seq += 1;
                    heap.push(Reverse((nc.to_bits(), seq, m as u32)));
                }
            }
        }
        if target != NONE {
            let mut path = vec![target];
            let mut g = target;
            while g as usize != c {
                let (gx, gy) = ((g as usize % w) as i64, (g as usize / w) as i64);
                g = prev[loc(gx, gy)];
                path.push(g);
            }
            path.reverse();
            let m = path.len() - 1;
            let zq = f64::from(z[target as usize]);
            let mut r_prev = zc;
            let mut ramp = Vec::with_capacity(m);
            let mut dmax = 0.0f64;
            for (i, &g) in path.iter().enumerate().take(m).skip(1) {
                let lin = zc + (zq - zc) * i as f64 / m as f64;
                let r = f64::from(z[g as usize]).min(lin).min(r_prev - 1e-6);
                dmax = dmax.max(f64::from(z[g as usize]) - r);
                ramp.push((g, r));
                r_prev = r;
            }
            if dmax <= p.depth {
                carved += 1;
                for (g, r) in ramp {
                    if r < z2[g as usize] {
                        z2[g as usize] = r;
                    }
                }
            } else {
                sinks += 1;
            }
        } else {
            sinks += 1;
        }
        for &t in &touched {
            cost[t as usize] = f64::INFINITY;
            prev[t as usize] = NONE;
        }
        touched.clear();
    }
    let t1 = t0.elapsed().as_secs_f64() * 1000.0;
    let rec = steepest(&z2, w, h);
    let t2 = t0.elapsed().as_secs_f64() * 1000.0;
    // Children lists, then rounds of bounded depth: acc_k(c) = 1 + sum of acc_(k-1) of the cells that drain into c; only cells whose child changed are recomputed.
    let mut off = vec![0u32; n + 1];
    for c in 0..n {
        if rec[c] != NONE {
            off[rec[c] as usize + 1] += 1;
        }
    }
    for i in 0..n {
        off[i + 1] += off[i];
    }
    let mut fill = off.clone();
    let mut kids = vec![0u32; off[n] as usize];
    for c in 0..n {
        if rec[c] != NONE {
            let r = rec[c] as usize;
            kids[fill[r] as usize] = c as u32;
            fill[r] += 1;
        }
    }
    let mut acc = vec![1u32; n];
    let mut changed: Vec<u32> = (0..n as u32).collect();
    let mut stamp = vec![0u32; n];
    for k in 1..=p.l {
        let mut parents: Vec<u32> = vec![];
        for &c in &changed {
            let r = rec[c as usize];
            if r != NONE && stamp[r as usize] != k as u32 {
                stamp[r as usize] = k as u32;
                parents.push(r);
            }
        }
        if parents.is_empty() {
            break;
        }
        let mut upd: Vec<(u32, u32)> = vec![];
        for &q in &parents {
            let mut s = 1u32;
            for &ch in &kids[off[q as usize] as usize..off[q as usize + 1] as usize] {
                s += acc[ch as usize];
            }
            if s != acc[q as usize] {
                upd.push((q, s));
            }
        }
        changed.clear();
        for (q, s) in upd {
            acc[q as usize] = s;
            changed.push(q);
        }
        if changed.is_empty() {
            break;
        }
    }
    let t3 = t0.elapsed().as_secs_f64() * 1000.0;
    Local { rec, acc: acc.iter().map(|&a| a as f32).collect(), pits, carved, sinks, ms: [t1, t2 - t1, t3 - t2] }
}

struct Sol {
    x0: usize,
    y0: usize,
    w: usize,
    rec: Vec<u32>,
    acc: Vec<f32>,
}
impl Sol {
    fn at(&self, x: usize, y: usize) -> f32 {
        self.acc[(y - self.y0) * self.w + (x - self.x0)]
    }
    fn rec_at(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let r = self.rec[(y - self.y0) * self.w + (x - self.x0)];
        (r != NONE).then(|| (self.x0 + r as usize % self.w, self.y0 + r as usize / self.w))
    }
}

/// The region of a chunk with core at `core` and halo `hh`, clipped to the window: (x0, y0, width, height).
fn clip(g: &Ground, core: (usize, usize), hh: usize) -> (usize, usize, usize, usize) {
    let (x0, y0) = (core.0.saturating_sub(hh), core.1.saturating_sub(hh));
    let (x1, y1) = ((core.0 + g.per + hh).min(g.w), (core.1 + g.per + hh).min(g.w));
    (x0, y0, x1 - x0, y1 - y0)
}

fn solve_fill(g: &Ground, x0: usize, y0: usize, w: usize, h: usize) -> (Sol, usize, f64) {
    let z = rect(&g.z, g.w, x0, y0, w, h);
    let t = std::time::Instant::now();
    let (rec, acc, flats) = flow_fill(&z, w, h);
    (Sol { x0, y0, w, rec, acc }, flats, t.elapsed().as_secs_f64() * 1000.0)
}

fn solve_local(g: &Ground, x0: usize, y0: usize, w: usize, h: usize, p: Params) -> (Sol, Local) {
    let z = rect(&g.z, g.w, x0, y0, w, h);
    let mut l = flow_local(&z, w, h, p);
    let sol = Sol { x0, y0, w, rec: std::mem::take(&mut l.rec), acc: std::mem::take(&mut l.acc) };
    (sol, l)
}

fn hash_sol(s: &Sol) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (a, r) in s.acc.iter().zip(&s.rec) {
        h = (h ^ u64::from(a.to_bits())).wrapping_mul(0x100_0000_01b3);
        h = (h ^ u64::from(*r)).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// density (km/km2), miss and extra (share of the reference's channel cells), channel cells in straight runs of at least 40 links (share of the chunk's channel cells).
fn stats(g: &Ground, s: &Sol, r: &Sol, c0: usize, t: f32) -> (f64, f64, f64, f64) {
    let per = g.per;
    let (mut len, mut nref, mut miss, mut extra, mut nch, mut straight) = (0.0f64, 0usize, 0usize, 0usize, 0usize, 0usize);
    for y in c0..c0 + per {
        for x in c0..c0 + per {
            let a = s.at(x, y) >= t;
            let b = r.at(x, y) >= t;
            if a {
                nch += 1;
                len += g.cell_m * s.rec_at(x, y).map_or(1.0, |(rx, ry)| if rx != x && ry != y { std::f64::consts::SQRT_2 } else { 1.0 });
                if let Some((rx, ry)) = s.rec_at(x, y) {
                    let d = (rx as i64 - x as i64, ry as i64 - y as i64);
                    let (mut cx, mut cy, mut k) = (rx, ry, 1);
                    while k < 40 {
                        match s.rec_at(cx, cy) {
                            Some((nx, ny)) if (nx as i64 - cx as i64, ny as i64 - cy as i64) == d && s.at(cx, cy) >= t => {
                                cx = nx;
                                cy = ny;
                                k += 1;
                            }
                            _ => break,
                        }
                    }
                    if k >= 40 {
                        straight += 1;
                    }
                }
            }
            if b {
                nref += 1;
                if !a {
                    miss += 1;
                }
            } else if a {
                extra += 1;
            }
        }
    }
    let area = (per as f64 * g.cell_m / 1000.0).powi(2);
    (len / 1000.0 / area, miss as f64 / nref.max(1) as f64 * 100.0, extra as f64 / nref.max(1) as f64 * 100.0, straight as f64 / nch.max(1) as f64 * 100.0)
}

/// The share of `a`'s channel cells in the core that have no channel cell of `b` within `tol` cells (Chebyshev).
fn far_share(g: &Ground, a: &Sol, b: &Sol, c0: usize, t: f32, tol: i64) -> f64 {
    let (mut n, mut far) = (0usize, 0usize);
    for y in c0..c0 + g.per {
        for x in c0..c0 + g.per {
            if a.at(x, y) < t {
                continue;
            }
            n += 1;
            let mut near = false;
            'o: for dy in -tol..=tol {
                for dx in -tol..=tol {
                    let (nx, ny) = ((x as i64 + dx) as usize, (y as i64 + dy) as usize);
                    if b.at(nx, ny) >= t {
                        near = true;
                        break 'o;
                    }
                }
            }
            if !near {
                far += 1;
            }
        }
    }
    far as f64 / n.max(1) as f64 * 100.0
}

/// Seam links of the reference between A (core at `ca`) and B (core at `cb`) across the east or north edge: (links, differ, stepdn).
fn seam(per: usize, r: &Sol, sa: &Sol, sb: &Sol, ca: (usize, usize), cb: (usize, usize), t: f32, east: bool) -> (usize, usize, usize) {
    let (mut cross, mut diff, mut step) = (0, 0, 0);
    for k in 0..per {
        let (ax, ay) = if east { (ca.0 + per - 1, ca.1 + k) } else { (ca.0 + k, ca.1) };
        let Some((rx, ry)) = r.rec_at(ax, ay) else { continue };
        if !(rx >= cb.0 && rx < cb.0 + per && ry >= cb.1 && ry < cb.1 + per) || r.at(ax, ay) < t {
            continue;
        }
        cross += 1;
        let (a, b) = (sa.at(ax, ay), sb.at(rx, ry));
        if (a >= t) != (b >= t) {
            diff += 1;
        }
        if b < a {
            step += 1;
        }
    }
    (cross, diff, step)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = load(&format!("{}/gnd_{}.bin", a[1], a[2]));
    let per = g.per;
    let c0 = per;
    let centre = (c0, c0);
    let east = (c0 + per, c0);
    let north = (c0, c0 - per);
    let cell_area = g.cell_m * g.cell_m;
    let t03 = (0.3e6 / cell_area) as f32;
    let (zmin, zmax) = g.z.iter().fold((f32::MAX, f32::MIN), |m, &v| (m.0.min(v), m.1.max(v)));
    let dmax = g.z.iter().zip(&g.zb).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    println!("site {}: solved ground {} cells, core {}, height {zmin:.0} to {zmax:.0} m, largest change from base {dmax:.0} m, A_t 0.3 km2 = {t03:.0} cells", a[2], g.w, per);
    let full = g.w;
    let base_p = Params { rb: 64, depth: 20.0, l: 245 };
    let rd = |p: Params| p.l + 2 * p.rb + 2;
    println!("bounded scheme: breach radius {} cells, carve limit {} m, area depth {} steps: dependence radius {} cells", base_p.rb, base_p.depth, base_p.l, rd(base_p));

    // References: the whole window.
    let (rf, flats_ref, ms) = solve_fill(&g, 0, 0, full, full);
    println!("fill reference over the window: {ms:.0} ms, {flats_ref} flat cells");
    let (rb_ref, lb) = solve_local(&g, 0, 0, full, full, base_p);
    println!("bounded reference over the window: pits {}, carved {}, sinks {} ({:.2} s)", lb.pits, lb.carved, lb.sinks, (lb.ms[0] + lb.ms[1] + lb.ms[2]) / 1000.0);

    println!("\n== A. fill (step 1 scheme) on solved ground: chunk with halo H vs the fill reference (margin 608); A_t 0.3 km2 ==");
    println!("{:>5} {:>8} {:>8} {:>8} {:>8} | seam E {:>5} {:>6} {:>6} | seam N {:>5} {:>6} {:>6}", "H", "ms", "dens", "miss%", "extra%", "links", "differ", "stepdn", "links", "differ", "stepdn");
    for h in [0usize, 100, 200, 375, 600] {
        let r = clip(&g, centre, h);
        let (sc, _, ms) = solve_fill(&g, r.0, r.1, r.2, r.3);
        let r = clip(&g, east, h);
        let (se, _, _) = solve_fill(&g, r.0, r.1, r.2, r.3);
        let r = clip(&g, north, h);
        let (sn, _, _) = solve_fill(&g, r.0, r.1, r.2, r.3);
        let st = stats(&g, &sc, &rf, c0, t03);
        let s1 = seam(per, &rf, &sc, &se, centre, east, t03, true);
        let s2 = seam(per, &rf, &sc, &sn, centre, north, t03, false);
        println!("{h:>5} {ms:>8.0} {:>8.3} {:>8.2} {:>8.2} | {:>12} {:>6} {:>6} | {:>12} {:>6} {:>6}", st.0, st.1, st.2, s1.0, s1.1, s1.2, s2.0, s2.1, s2.2);
    }
    println!("(the chunks east and north are clipped to the window: H above 608 is H 608 for them)");

    println!("\n== B. bounded scheme: chunk with halo H vs the bounded reference (same parameters, the whole window); A_t 0.3 km2 ==");
    println!("{:>5} {:>8} {:>8} {:>8} {:>8} {:>8} | seam E {:>5} {:>6} {:>6} | seam N {:>5} {:>6} {:>6}", "H", "ms", "dens", "miss%", "extra%", "mismatch", "links", "differ", "stepdn", "links", "differ", "stepdn");
    for h in [100usize, 200, 300, 375, 600] {
        let r = clip(&g, centre, h);
        let (sc, lc) = solve_local(&g, r.0, r.1, r.2, r.3, base_p);
        let r = clip(&g, east, h);
        let (se, _) = solve_local(&g, r.0, r.1, r.2, r.3, base_p);
        let r = clip(&g, north, h);
        let (sn, _) = solve_local(&g, r.0, r.1, r.2, r.3, base_p);
        let st = stats(&g, &sc, &rb_ref, c0, t03);
        let mut mism = 0usize;
        for y in c0..c0 + per {
            for x in c0..c0 + per {
                if sc.at(x, y) != rb_ref.at(x, y) || sc.rec_at(x, y) != rb_ref.rec_at(x, y) {
                    mism += 1;
                }
            }
        }
        let s1 = seam(per, &rb_ref, &sc, &se, centre, east, t03, true);
        let s2 = seam(per, &rb_ref, &sc, &sn, centre, north, t03, false);
        println!("{h:>5} {:>8.0} {:>8.3} {:>8.2} {:>8.2} {mism:>8} | {:>12} {:>6} {:>6} | {:>12} {:>6} {:>6}", lc.ms[0] + lc.ms[1] + lc.ms[2], st.0, st.1, st.2, s1.0, s1.1, s1.2, s2.0, s2.1, s2.2);
    }
    println!("(mismatch: core cells where the chunk's area or receiver differs from the reference's, any value; the chunks east and north have the same H, their regions fit the window up to H 608 only on the sides facing the centre)");

    println!("\n== C. what the bound changes: bounded reference vs fill reference, whole window core (A_t 0.3 km2) ==");
    let (sf, sp) = (stats(&g, &rf, &rf, c0, t03), stats(&g, &rb_ref, &rf, c0, t03));
    println!("fill:    density {:.3}  straight-run share {:.1} %  flat cells in the core: {}", sf.0, sf.3, {
        let mut n = 0;
        for y in c0..c0 + per {
            for x in c0..c0 + per {
                let c = y * full + x;
                let r = rf.rec[c];
                if r != NONE {
                    let (rx, ry) = (r as usize % full, r as usize / full);
                    if g.z[ry * full + rx] >= g.z[c] {
                        n += 1;
                    }
                }
            }
        }
        n
    });
    let mut sink_core = 0;
    for y in c0..c0 + per {
        for x in c0..c0 + per {
            if rb_ref.rec[y * full + x] == NONE {
                sink_core += 1;
            }
        }
    }
    println!("bounded: density {:.3}  straight-run share {:.1} %  sink cells in the core: {}  miss {:.2} % extra {:.2} % against the fill reference", sp.0, sp.3, sink_core, sp.1, sp.2);
    println!("tolerance 3 cells (24 m): fill channel cells with no bounded channel within 3 cells {:.1} %, bounded cells with no fill channel within 3 cells {:.1} %; tolerance 8 cells (64 m): {:.1} % and {:.1} %",
        far_share(&g, &rf, &rb_ref, c0, t03, 3), far_share(&g, &rb_ref, &rf, c0, t03, 3), far_share(&g, &rf, &rb_ref, c0, t03, 8), far_share(&g, &rb_ref, &rf, c0, t03, 8));
    println!("{:>4} {:>6} {:>4} | {:>7} {:>8} {:>8} {:>7} {:>8} {:>6} {:>6}", "Rb", "carve", "L", "dens", "miss%", "extra%", "straight", "sinks", "carved", "ms");
    for (rb, depth, l) in [(32usize, 20.0f64, 309usize), (64, 5.0, 245), (64, 20.0, 245), (64, 100.0, 245), (96, 20.0, 181), (64, 20.0, 1400)] {
        let p = Params { rb, depth, l };
        let (s, lc) = solve_local(&g, 0, 0, full, full, p);
        let st = stats(&g, &s, &rf, c0, t03);
        let mut sk = 0;
        for y in c0..c0 + per {
            for x in c0..c0 + per {
                if s.rec[y * full + x] == NONE {
                    sk += 1;
                }
            }
        }
        println!("{rb:>4} {depth:>6} {l:>4} | {:>7.3} {:>8.2} {:>8.2} {:>7.1} {:>8} {:>6} {:>6.0}", st.0, st.1, st.2, st.3, sk, lc.carved, lc.ms[0] + lc.ms[1] + lc.ms[2]);
    }
    println!("(Rb breach radius in cells, carve limit in m, L area depth in steps; the last row is the bounded breach with an area depth no tree reaches)");

    // Cost of one chunk and thread identity.
    println!("\n== D. cost and thread identity (bounded, H 375, centre chunk; 9 cores clipped to the window) ==");
    let r0 = clip(&g, centre, 375);
    let (sc, lc) = solve_local(&g, r0.0, r0.1, r0.2, r0.3, base_p);
    println!("centre chunk {} cells: breach {:.0} ms, receivers {:.0} ms, area {:.0} ms, total {:.0} ms; pits {}, carved {}, sinks {}; hash {:016x}", (per + 750) * (per + 750), lc.ms[0], lc.ms[1], lc.ms[2], lc.ms[0] + lc.ms[1] + lc.ms[2], lc.pits, lc.carved, lc.sinks, hash_sol(&sc));
    let cores: Vec<(usize, usize)> = (0..9).map(|k| ((k % 3) * per, (k / 3) * per)).collect();
    let run = |workers: usize| -> (Vec<u64>, f64) {
        let t = std::time::Instant::now();
        let out = std::sync::Mutex::new(vec![0u64; 9]);
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..workers {
                sc.spawn(|| loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if k >= 9 {
                        break;
                    }
                    let (cx, cy) = cores[k];
                    let r = clip(&g, (cx, cy), 375);
                    let (s, _) = solve_local(&g, r.0, r.1, r.2, r.3, base_p);
                    out.lock().unwrap()[k] = hash_sol(&s);
                });
            }
        });
        (out.into_inner().unwrap(), t.elapsed().as_secs_f64())
    };
    let (h1, t1) = run(1);
    let (h8, t8) = run(8);
    println!("9 chunks: 1 worker {t1:.2} s, 8 workers {t8:.2} s, hashes identical: {}", h1 == h8);

    if let Some(dir) = a.get(3) {
        let mut gz = Vec::with_capacity(per * per * 4);
        for y in c0..c0 + per {
            for x in c0..c0 + per {
                gz.extend_from_slice(&g.z[y * full + x].to_le_bytes());
            }
        }
        std::fs::write(format!("{dir}/{}_sg.f32", a[2]), gz).unwrap();
        let m = |s: &Sol| -> Vec<u8> {
            let mut v = Vec::with_capacity(per * per);
            for y in c0..c0 + per {
                for x in c0..c0 + per {
                    v.push(u8::from(s.at(x, y) >= t03));
                }
            }
            v
        };
        std::fs::write(format!("{dir}/{}_sg_fill.u8", a[2]), m(&rf)).unwrap();
        std::fs::write(format!("{dir}/{}_sg_bounded.u8", a[2]), m(&rb_ref)).unwrap();
        let r = clip(&g, centre, 200);
        let (s200, _, _) = solve_fill(&g, r.0, r.1, r.2, r.3);
        std::fs::write(format!("{dir}/{}_sg_fill200.u8", a[2]), m(&s200)).unwrap();
        let r = clip(&g, centre, 375);
        let (s375, _, _) = solve_fill(&g, r.0, r.1, r.2, r.3);
        std::fs::write(format!("{dir}/{}_sg_fill375.u8", a[2]), m(&s375)).unwrap();
        std::fs::write(format!("{dir}/{}_sg_bounded375.u8", a[2]), m(&sc)).unwrap();
    }
}
