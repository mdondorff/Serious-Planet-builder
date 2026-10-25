//! SPIKE-43 throwaway: a fine first-order channel network from local flow accumulation on a region of the chunk's own cells, against one big reference region.
//! usage: fine <win_dir> <site> [out_txt]
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const NB: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
const NONE: u32 = u32::MAX;

struct Window {
    per: usize,
    cell_m: f64,
    w: usize,
    halo: usize,
    z: Vec<f32>,
}

fn load(path: &str) -> Window {
    let b = std::fs::read(path).expect("window file");
    let per = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let cell_m = f64::from_le_bytes(b[4..12].try_into().unwrap());
    let halo = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let w = u32::from_le_bytes(b[16..20].try_into().unwrap()) as usize;
    let off = 20 + 12;
    let z = b[off..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>();
    assert_eq!(z.len(), w * w);
    Window { per, cell_m, w, halo, z }
}

fn ordered(z: f32) -> u32 {
    let b = z.to_bits();
    if b >> 31 == 1 { !b } else { b | 0x8000_0000 }
}

/// Flow on a w x h rectangle: `fill` runs a priority flood from the rectangle border (pits filled, flats routed in the order the flood reaches them), else raw heights with pits as sinks.
/// `inflow[c]` adds area (in cells) arriving from outside at cell c. Returns the receiver (NONE for an outlet) and the accumulation in cells.
fn flow(z: &[f32], w: usize, h: usize, fill: bool, inflow: Option<&[f32]>) -> (Vec<u32>, Vec<f32>, Vec<f32>) {
    let n = w * h;
    let mut order: Vec<u32> = Vec::with_capacity(n);
    let mut zf: Vec<f32> = z.to_vec();
    let mut tree = vec![NONE; n];
    if fill {
        let mut seen = vec![false; n];
        let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
        let mut seq: u64 = 0;
        let mut push = |heap: &mut BinaryHeap<Reverse<(u64, u32)>>, zz: f32, c: u32, seq: &mut u64| {
            heap.push(Reverse((u64::from(ordered(zz)) << 32 | *seq, c)));
            *seq += 1;
        };
        for y in 0..h {
            for x in 0..w {
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    let c = y * w + x;
                    seen[c] = true;
                    push(&mut heap, zf[c], c as u32, &mut seq);
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
                push(&mut heap, zf[m], m as u32, &mut seq);
            }
        }
    } else {
        order = (0..n as u32).collect();
        order.sort_unstable_by_key(|&c| (Reverse(ordered(zf[c as usize])), c));
    }
    // The receiver: the steepest strictly lower neighbour of the (filled) surface, else the flood's own, else none.
    let mut rec = vec![NONE; n];
    for c in 0..n {
        let (cx, cy) = ((c % w) as i64, (c / w) as i64);
        let on_edge = cx == 0 || cy == 0 || cx == w as i64 - 1 || cy == h as i64 - 1;
        let mut best = NONE;
        let mut best_s = 0.0f32;
        if !on_edge {
            for (dx, dy) in NB {
                let m = ((cy + dy) as usize) * w + (cx + dx) as usize;
                let d = zf[c] - zf[m];
                if d > 0.0 {
                    let s = if dx != 0 && dy != 0 { d * std::f32::consts::FRAC_1_SQRT_2 } else { d };
                    if s > best_s {
                        best_s = s;
                        best = m as u32;
                    }
                }
            }
        }
        rec[c] = if best != NONE {
            best
        } else if fill && !on_edge {
            tree[c]
        } else {
            NONE
        };
    }
    // Accumulation: topological from the highest filled cell down. For `fill` the flood's pop order is a valid reverse order; otherwise sort by raw height descending.
    let mut acc = vec![1.0f32; n];
    if let Some(inf) = inflow {
        for c in 0..n {
            acc[c] += inf[c];
        }
    }
    let seq_order: Vec<u32> = if fill {
        order.iter().rev().copied().collect()
    } else {
        let mut o: Vec<u32> = (0..n as u32).collect();
        o.sort_unstable_by_key(|&c| (Reverse(ordered(zf[c as usize])), c));
        o
    };
    for &c in &seq_order {
        let r = rec[c as usize];
        if r != NONE {
            let a = acc[c as usize];
            acc[r as usize] += a;
        }
    }
    let depth: Vec<f32> = zf.iter().zip(z).map(|(a, b)| a - b).collect();
    (rec, acc, depth)
}

struct Sol {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    rec: Vec<u32>,
    acc: Vec<f32>,
    depth: Vec<f32>,
}

fn solve(win: &Window, x0: usize, y0: usize, w: usize, h: usize, fill: bool, inflow: Option<&[f32]>) -> Sol {
    let mut z = Vec::with_capacity(w * h);
    for y in 0..h {
        let r = (y0 + y) * win.w + x0;
        z.extend_from_slice(&win.z[r..r + w]);
    }
    let (rec, acc, depth) = flow(&z, w, h, fill, inflow);
    Sol { x0, y0, w, h, rec, acc, depth }
}

impl Sol {
    fn at(&self, x: usize, y: usize) -> f32 {
        self.acc[(y - self.y0) * self.w + (x - self.x0)]
    }
    fn depth_at(&self, x: usize, y: usize) -> f32 {
        self.depth[(y - self.y0) * self.w + (x - self.x0)]
    }
    fn rec_at(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let r = self.rec[(y - self.y0) * self.w + (x - self.x0)];
        (r != NONE).then(|| (self.x0 + r as usize % self.w, self.y0 + r as usize / self.w))
    }
}

fn hash_sol(s: &Sol) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (a, r) in s.acc.iter().zip(&s.rec) {
        h = (h ^ u64::from(a.to_bits())).wrapping_mul(0x100_0000_01b3);
        h = (h ^ u64::from(*r)).wrapping_mul(0x100_0000_01b3);
    }
    h
}

struct Stats {
    density: f64,
    miss: f64,
    extra: f64,
}

/// Channel statistics of `s` over the core rectangle (x0,y0,per,per) against the reference `r`, at threshold `t` cells.
fn core_stats(win: &Window, s: &Sol, r: &Sol, cx: usize, cy: usize, t: f32) -> Stats {
    let per = win.per;
    let (mut len, mut n_ref, mut miss, mut extra) = (0.0f64, 0usize, 0usize, 0usize);
    for y in cy..cy + per {
        for x in cx..cx + per {
            let a = s.at(x, y) >= t;
            let b = r.at(x, y) >= t;
            if a {
                len += win.cell_m * s.rec_at(x, y).map_or(1.0, |(rx, ry)| if rx != x && ry != y { std::f64::consts::SQRT_2 } else { 1.0 });
            }
            if b {
                n_ref += 1;
                if !a {
                    miss += 1;
                }
            } else if a {
                extra += 1;
            }
        }
    }
    let area_km2 = (per as f64 * win.cell_m / 1000.0).powi(2);
    Stats { density: len / 1000.0 / area_km2, miss: miss as f64 / n_ref.max(1) as f64, extra: extra as f64 / n_ref.max(1) as f64 }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let win = load(&format!("{}/win_{}.bin", a[1], a[2]));
    let (per, halo) = (win.per, win.halo);
    let cell_area = win.cell_m * win.cell_m;
    let tkm2 = [0.1f64, 0.3, 1.0];
    let tc: Vec<f32> = tkm2.iter().map(|&k| (k * 1e6 / cell_area) as f32).collect();
    println!("site {}: window {} cells, cell {:.3} m, core {} cells, margin {}, thresholds {:?} km2 = {:?} cells", a[2], win.w, win.cell_m, per, halo, tkm2, tc);
    let t0 = std::time::Instant::now();
    let mut refs: Vec<(bool, Sol)> = vec![];
    for fill in [true, false] {
        let r = solve(&win, 0, 0, win.w, win.w, fill, None);
        println!("reference fill={fill}: {} cells in {:.1} s", win.w * win.w, t0.elapsed().as_secs_f64());
        refs.push((fill, r));
    }
    let centre = (halo, halo);
    let east = (halo + per, halo);
    let north = (halo, halo - per);
    let hs = [0usize, 25, 50, 100, 200, 375, 600, 780];
    println!("\nRows: halo H (cells) per scheme; stats of the centre chunk's core vs the reference. density km/km2 (chunk / reference), miss and extra as a share of the reference's channel cells.");
    for (fill, r) in &refs {
        println!("\n== fill={fill} ==");
        for (ti, &t) in tc.iter().enumerate() {
            let rs = core_stats(&win, r, r, centre.0, centre.1, t);
            println!("-- threshold {} km2, reference density {:.3} km/km2", tkm2[ti], rs.density);
            println!("{:>5} {:>8} {:>8} {:>8} {:>8} | seam E: {:>6} {:>6} {:>7} {:>7} | seam N: {:>6} {:>6} {:>6}", "H", "ms", "dens", "miss%", "extra%", "links", "differ", "stepdn", "bothno", "links", "differ", "stepdn");
            for &h in &hs {
                let t1 = std::time::Instant::now();
                let sc = solve(&win, centre.0 - h, centre.1 - h, per + 2 * h, per + 2 * h, *fill, None);
                let ms = t1.elapsed().as_secs_f64() * 1000.0;
                let st = core_stats(&win, &sc, r, centre.0, centre.1, t);
                let se = solve(&win, east.0 - h, east.1 - h, per + 2 * h, per + 2 * h, *fill, None);
                let sn = solve(&win, north.0 - h, north.1 - h, per + 2 * h, per + 2 * h, *fill, None);
                let (c1, b1, lo, hi) = seam(&win, r, &sc, &se, centre, east, t, true);
                let (c2, b2, st2, _) = seam(&win, r, &sc, &sn, centre, north, t, false);
                println!("{h:>5} {ms:>8.0} {:>8.3} {:>8.2} {:>8.2} | {c1:>6} {b1:>6} {lo:>7} {hi:>7} | {c2:>6} {b2:>6} {st2:>6}", st.density, st.miss * 100.0, st.extra * 100.0);
            }
        }
    }
    // Inflow schemes (a): the core plus a ring of one cell (the outlet), with the fine area that the reference sends into the core from outside added to the core's edge cells.
    println!("
== (a) edge inflow: core + 1 ring, fill=true ==");
    let r = &refs[0].1;
    let rw = per + 2;
    let mut oracle = vec![0.0f32; rw * rw];
    let mut entries = vec![0usize; 3];
    for y in 0..win.w {
        for x in 0..win.w {
            let inside = x >= centre.0 && x < centre.0 + per && y >= centre.1 && y < centre.1 + per;
            if inside {
                continue;
            }
            if let Some((rx, ry)) = r.rec_at(x, y) {
                if rx >= centre.0 && rx < centre.0 + per && ry >= centre.1 && ry < centre.1 + per {
                    let a = r.at(x, y);
                    oracle[(ry - centre.1 + 1) * rw + (rx - centre.0 + 1)] += a;
                    for (k, &t) in tc.iter().enumerate() {
                        if a >= t {
                            entries[k] += 1;
                        }
                    }
                }
            }
        }
    }
    println!("fine channels entering the core from outside (reference donors with area over the threshold): {} / {} / {} at {:?} km2 (a coarse cell is 24 km2; the coarse run names the stems only)", entries[0], entries[1], entries[2], tkm2);
    let so = solve(&win, centre.0 - 1, centre.1 - 1, rw, rw, true, Some(&oracle));
    let mut smear = vec![0.0f32; rw * rw];
    let mut side = [0.0f32; 4];
    for y in 1..=per {
        for x in 1..=per {
            let v = oracle[y * rw + x];
            if v > 0.0 {
                if x == 1 { side[0] += v } else if x == per { side[1] += v } else if y == 1 { side[2] += v } else { side[3] += v }
            }
        }
    }
    for i in 1..=per {
        smear[i * rw + 1] += side[0] / per as f32;
        smear[i * rw + per] += side[1] / per as f32;
        smear[rw + i] += side[2] / per as f32;
        smear[per * rw + i] += side[3] / per as f32;
    }
    let ss = solve(&win, centre.0 - 1, centre.1 - 1, rw, rw, true, Some(&smear));
    let s0 = solve(&win, centre.0 - 1, centre.1 - 1, rw, rw, true, None);
    for (ti, &t) in tc.iter().enumerate() {
        let rs = core_stats(&win, r, r, centre.0, centre.1, t);
        println!("threshold {} km2 (reference {:.3} km/km2):", tkm2[ti], rs.density);
        for (name, s) in [("restart (no inflow)", &s0), ("oracle entry inflow", &so), ("even smear per side", &ss)] {
            let st = core_stats(&win, s, r, centre.0, centre.1, t);
            println!("  {name:<22} density {:.3}  miss {:.2} %  extra {:.2} %", st.density, st.miss * 100.0, st.extra * 100.0);
        }
    }
    // The fill: how much of the core the reference fills, and how many of the mismatching cells sit in filled ground.
    println!("
== depression fill in the core (reference) and where the mismatch sits (threshold 0.3 km2) ==");
    {
        let (mut n05, mut n5, mut n50, mut mx) = (0usize, 0usize, 0usize, 0.0f32);
        for y in centre.1..centre.1 + per {
            for x in centre.0..centre.0 + per {
                let d = r.depth_at(x, y);
                if d > 0.5 { n05 += 1 }
                if d > 5.0 { n5 += 1 }
                if d > 50.0 { n50 += 1 }
                mx = mx.max(d);
            }
        }
        let tot = (per * per) as f64;
        println!("share of core cells filled by more than 0.5 / 5 / 50 m: {:.1} / {:.1} / {:.2} %, deepest {:.0} m", n05 as f64 / tot * 100.0, n5 as f64 / tot * 100.0, n50 as f64 / tot * 100.0, mx);
        for &h in &[100usize, 200, 375, 600] {
            let sc = solve(&win, centre.0 - h, centre.1 - h, per + 2 * h, per + 2 * h, true, None);
            let (mut bad, mut bad_filled) = (0usize, 0usize);
            for y in centre.1..centre.1 + per {
                for x in centre.0..centre.0 + per {
                    if (sc.at(x, y) >= tc[1]) != (r.at(x, y) >= tc[1]) {
                        bad += 1;
                        if r.depth_at(x, y) > 0.5 || sc.depth_at(x, y) > 0.5 {
                            bad_filled += 1;
                        }
                    }
                }
            }
            println!("H {h}: {bad} mismatching cells, {bad_filled} of them on ground filled by more than 0.5 m in either solution");
        }
    }
    // Channel heads of the reference per km2.
    for (ti, &t) in tc.iter().enumerate() {
        let mut has_donor = vec![false; per * per];
        for y in centre.1..centre.1 + per {
            for x in centre.0..centre.0 + per {
                if r.at(x, y) >= t {
                    if let Some((rx, ry)) = r.rec_at(x, y) {
                        if rx >= centre.0 && rx < centre.0 + per && ry >= centre.1 && ry < centre.1 + per {
                            has_donor[(ry - centre.1) * per + rx - centre.0] = true;
                        }
                    }
                }
            }
        }
        let mut heads = 0usize;
        for y in centre.1..centre.1 + per {
            for x in centre.0..centre.0 + per {
                if r.at(x, y) >= t && !has_donor[(y - centre.1) * per + x - centre.0] {
                    heads += 1;
                }
            }
        }
        let area_km2 = (per as f64 * win.cell_m / 1000.0).powi(2);
        println!("threshold {} km2: {} channel heads in the core, {:.2} per km2", tkm2[ti], heads, heads as f64 / area_km2);
    }
    // Masks of the centre core for the pictures (threshold 0.3 km2): ground, reference, and the chunk at H = 0, 100, 375, 600; u8 per cell, bit 0 channel.
    if let Some(dir) = a.get(3) {
        let t = tc[1];
        let mut g = Vec::with_capacity(per * per * 4);
        for y in centre.1..centre.1 + per {
            for x in centre.0..centre.0 + per {
                g.extend_from_slice(&win.z[y * win.w + x].to_le_bytes());
            }
        }
        std::fs::write(format!("{dir}/{}_ground.f32", a[2]), g).unwrap();
        let m = |s: &Sol| -> Vec<u8> {
            let mut v = Vec::with_capacity(per * per);
            for y in centre.1..centre.1 + per {
                for x in centre.0..centre.0 + per {
                    v.push(u8::from(s.at(x, y) >= t));
                }
            }
            v
        };
        std::fs::write(format!("{dir}/{}_ref.u8", a[2]), m(&refs[0].1)).unwrap();
        for h in [0usize, 100, 375, 600] {
            let sc = solve(&win, centre.0 - h, centre.1 - h, per + 2 * h, per + 2 * h, true, None);
            std::fs::write(format!("{dir}/{}_h{h}.u8", a[2]), m(&sc)).unwrap();
        }
    }
    // Threads: the nine chunk cores of the window, H = 375, 1 worker against 8.
    println!("\n== thread identity and cost: 9 chunks, H = 375, fill=true ==");
    let cores: Vec<(usize, usize)> = (0..9).map(|k| (halo - per + (k % 3) * per, halo - per + (k / 3) * per)).collect();
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
                    let s = solve(&win, cx - 375, cy - 375, per + 750, per + 750, true, None);
                    out.lock().unwrap()[k] = hash_sol(&s);
                });
            }
        });
        (out.into_inner().unwrap(), t.elapsed().as_secs_f64())
    };
    let (h1, t1) = run(1);
    let (h8, t8) = run(8);
    println!("1 worker {t1:.2} s ({:.2} s per chunk), 8 workers {t8:.2} s; hashes identical: {}", t1 / 9.0, h1 == h8);
}

/// Seam statistics between chunk A (solution `sa`, core at `ca`) and its neighbour B (`sb`, core at `cb`) across their shared edge (east if `east`, else north),
/// over the reference's links that cross the edge from A's edge cell to B's with area over `t`: (links, links on which A's and B's channel status differ,
/// links on which B's area at the receiver is below A's at the source, links both call not a channel).
fn seam(win: &Window, r: &Sol, sa: &Sol, sb: &Sol, ca: (usize, usize), cb: (usize, usize), t: f32, east: bool) -> (usize, usize, usize, usize) {
    let per = win.per;
    let (mut cross, mut diff, mut step, mut both) = (0, 0, 0, 0);
    for k in 0..per {
        let (ax, ay) = if east { (ca.0 + per - 1, ca.1 + k) } else { (ca.0 + k, ca.1) };
        let Some((rx, ry)) = r.rec_at(ax, ay) else { continue };
        let inb = rx >= cb.0 && rx < cb.0 + per && ry >= cb.1 && ry < cb.1 + per;
        if !inb || r.at(ax, ay) < t {
            continue;
        }
        cross += 1;
        let (a_src, b_rcv) = (sa.at(ax, ay), sb.at(rx, ry));
        let (ca_, cb_) = (a_src >= t, b_rcv >= t);
        if ca_ != cb_ {
            diff += 1;
        }
        if !ca_ && !cb_ {
            both += 1;
        }
        if b_rcv < a_src {
            step += 1;
        }
    }
    (cross, diff, step, both)
}
