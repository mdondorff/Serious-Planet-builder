//! SPIKE-119 probe (spike code, never merged): where are a mountain window and a low-gradient river window?
//!
//! `window_scan coarse <face> <seed> [stride] [threads]`: B over a whole face at a node stride, then per 620-cell-window block (827 nodes a side) the land share, mean elevation and the p50/p90 of the
//! range over each 3 x 3 coarse neighbourhood (a regional-scale relief proxy); one line per block, sorted by a "mountain" and a "lowland" score.
//! `window_scan win <face> <i0> <i1> <j0> <j1> <seed> [threads]`: B at full lattice resolution over the window's core (regional cells), then the proxy statistics of the start surface's relief:
//! the p50/p90 of the range over 2 x 2, 3 x 3 and 5 x 5 lattice nodes (611 m nodes: 1.2, 1.8 and 3.1 km), the elevation p5/p50/p95 and the land share, the steepest-neighbour slope p50/p95.
use planet_cache::global_store::class_inputs;
use planet_cache::region_bake::lattice_direction;
use planet_core::Face;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;
const LATTICE: usize = 16_384;
const REGIONAL: usize = 12_288;
/// Lattice node spacing in metres (a face edge is about R * pi / 2 along the cube's centre line; the lattice is 16,384 nodes).
const NODE_M: f64 = 611.0;

fn par_fill(out: &mut [f64], w: usize, threads: usize, f: &(dyn Fn(usize) -> f64 + Sync)) {
    let rows = (out.len() / w).div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    *v = f(k * rows * w + c);
                }
            });
        }
    });
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Range (max - min) over each k x k block of cells at every cell, as a list.
fn ranges(z: &[f64], w: usize, h: usize, k: usize, sea: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for y in 0..h.saturating_sub(k - 1) {
        for x in 0..w.saturating_sub(k - 1) {
            let (mut lo, mut hi, mut land) = (f64::MAX, f64::MIN, true);
            for yy in y..y + k {
                for xx in x..x + k {
                    let v = z[yy * w + xx];
                    land &= v > sea;
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            if land {
                out.push(hi - lo);
            }
        }
    }
    out
}

/// D8 drained area (in cells) over a square coarse grid after a priority flood from the sea (z <= 0) and the grid's edge (water leaves the face).
fn accumulate(z: &[f64], n: usize) -> Vec<f64> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    #[derive(PartialEq, PartialOrd)]
    struct K(f64);
    impl Eq for K {}
    impl Ord for K {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.partial_cmp(o).unwrap()
        }
    }
    let mut filled = z.to_vec();
    let mut recv = vec![usize::MAX; n * n];
    let mut done = vec![false; n * n];
    let mut heap = BinaryHeap::new();
    for c in 0..n * n {
        let (x, y) = (c % n, c / n);
        if z[c] <= 0.0 || x == 0 || y == 0 || x == n - 1 || y == n - 1 {
            done[c] = true;
            heap.push(Reverse((K(z[c]), c)));
        }
    }
    let mut order = Vec::with_capacity(n * n);
    while let Some(Reverse((K(h), c))) = heap.pop() {
        order.push(c);
        let (x, y) = ((c % n) as i64, (c / n) as i64);
        for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= n as i64 || ny >= n as i64 {
                continue;
            }
            let q = (ny as usize) * n + nx as usize;
            if done[q] {
                continue;
            }
            done[q] = true;
            filled[q] = z[q].max(h + 1e-4);
            recv[q] = c;
            heap.push(Reverse((K(filled[q]), q)));
        }
    }
    let mut acc = vec![1.0f64; n * n];
    for &c in order.iter().rev() {
        if recv[c] != usize::MAX {
            let r = recv[c];
            acc[r] += acc[c];
        }
    }
    acc
}

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let num = |k: usize| -> Result<usize, String> { a.get(k).ok_or("missing argument")?.parse::<usize>().map_err(|e| e.to_string()) };
    match a.first().map(String::as_str) {
        Some("coarse") => {
            let (face, seed) = (num(1)?, num(2)? as u64);
            let stride = if a.len() > 3 { num(3)? } else { 16 };
            let threads = if a.len() > 4 { num(4)? } else { 8 };
            let def = WorldDefinition::single_planet(R, seed);
            let inputs = class_inputs(&def, seed, threads);
            let n = LATTICE / stride + 1;
            let mut z = vec![0f64; n * n];
            par_fill(&mut z, n, threads, &|p| inputs.base(lattice_direction(Face(face as u8), LATTICE, (p % n) * stride, (p / n) * stride)));
            let acc = accumulate(&z, n);
            // Blocks of 827 nodes (a 620-cell window), at half-block steps.
            let b = 827 / stride;
            let step = (b / 2).max(1);
            let mut rows = Vec::new();
            let mut y = 0;
            while y + b <= n {
                let mut x = 0;
                while x + b <= n {
                    let mut blk = Vec::with_capacity(b * b);
                    for yy in y..y + b {
                        blk.extend_from_slice(&z[yy * n + x..yy * n + x + b]);
                    }
                    let land = blk.iter().filter(|&&v| v > 0.0).count() as f64 / blk.len() as f64;
                    let mean = blk.iter().sum::<f64>() / blk.len() as f64;
                    let mut r3 = ranges(&blk, b, b, 3, f64::MIN);
                    let (p50, p90) = (pct(&mut r3.clone(), 0.5), pct(&mut r3, 0.9));
                    let mut bacc = 0f64;
                    for yy in y..y + b {
                        for xx in x..x + b {
                            bacc = bacc.max(acc[yy * n + xx]);
                        }
                    }
                    rows.push((x * stride, y * stride, land, mean, p50, p90, bacc * (stride as f64 * NODE_M / 1000.0).powi(2)));
                    x += step;
                }
                y += step;
            }
            println!("# face {face} seed {seed} stride {stride} ({:.1} km a coarse step); window = 827 nodes = 620 regional cells", stride as f64 * NODE_M / 1000.0);
            println!("# node_i node_j  regional_i regional_j  land  mean_z  range3_p50  range3_p90  max_area_km2");
            let mut by_m = rows.clone();
            by_m.retain(|r| r.2 > 0.999);
            by_m.sort_by(|p, q| q.4.partial_cmp(&p.4).unwrap());
            println!("## mountain (all land, by p50 range over 3 x 3 coarse nodes)");
            for r in by_m.iter().take(12) {
                println!("{:5} {:5}  {:5} {:5}  {:.3} {:7.0} {:7.0} {:7.0} {:9.0}", r.0, r.1, r.0 * REGIONAL / LATTICE, r.1 * REGIONAL / LATTICE, r.2, r.3, r.4, r.5, r.6);
            }
            let mut by_l = rows.clone();
            by_l.retain(|r| r.2 > 0.999 && r.3 > 20.0 && r.3 < 400.0);
            by_l.retain(|r| r.5 < 70.0);
            by_l.sort_by(|p, q| q.6.partial_cmp(&p.6).unwrap());
            println!("## lowland (all land, mean z 20-400 m, p90 range under 70 m, by the largest river's drained area)");
            for r in by_l.iter().take(12) {
                println!("{:5} {:5}  {:5} {:5}  {:.3} {:7.0} {:7.0} {:7.0} {:9.0}", r.0, r.1, r.0 * REGIONAL / LATTICE, r.1 * REGIONAL / LATTICE, r.2, r.3, r.4, r.5, r.6);
            }
        }
        Some("win") => {
            let (face, i0, i1, j0, j1, seed) = (num(1)?, num(2)?, num(3)?, num(4)?, num(5)?, num(6)? as u64);
            let threads = if a.len() > 7 { num(7)? } else { 8 };
            let def = WorldDefinition::single_planet(R, seed);
            let inputs = class_inputs(&def, seed, threads);
            let to_node = |r: usize| r * LATTICE / REGIONAL;
            let (n0i, n0j) = (to_node(i0), to_node(j0));
            let (w, h) = (to_node(i1 + 1) - n0i, to_node(j1 + 1) - n0j);
            let mut z = vec![0f64; w * h];
            par_fill(&mut z, w, threads, &|p| inputs.base(lattice_direction(Face(face as u8), LATTICE, n0i + p % w, n0j + p / w)));
            let land: Vec<f64> = z.iter().copied().filter(|&v| v > 0.0).collect();
            let share = land.len() as f64 / z.len() as f64;
            let (e5, e50, e95) = (pct(&mut land.clone(), 0.05), pct(&mut land.clone(), 0.5), pct(&mut land.clone(), 0.95));
            println!("WIN face {face} i {i0}-{i1} j {j0}-{j1} seed {seed}: {w} x {h} nodes, land {share:.3}, z p5/p50/p95 {e5:.0}/{e50:.0}/{e95:.0} m");
            for k in [2usize, 3, 5] {
                let mut r = ranges(&z, w, h, k, 0.0);
                println!(
                    "  range over {k} x {k} nodes ({:.1} km): p50 {:.1} m  p90 {:.1} m  p99 {:.1} m  ({} windows)",
                    (k - 1) as f64 * NODE_M / 1000.0 + NODE_M / 1000.0,
                    pct(&mut r.clone(), 0.5),
                    pct(&mut r.clone(), 0.9),
                    pct(&mut r, 0.99),
                    r.len()
                );
            }
            let mut sl = Vec::new();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let c = z[y * w + x];
                    if c <= 0.0 {
                        continue;
                    }
                    let mut m = 0f64;
                    for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                        let nz = z[((y as i64 + dy) as usize) * w + (x as i64 + dx) as usize];
                        let d = if dx != 0 && dy != 0 { NODE_M * 1.4142 } else { NODE_M };
                        m = m.max((c - nz) / d);
                    }
                    sl.push(m);
                }
            }
            println!("  steepest-neighbour slope p50 {:.4}  p95 {:.4}", pct(&mut sl.clone(), 0.5), pct(&mut sl, 0.95));
        }
        _ => return Err("usage: window_scan coarse <face> <seed> [stride] [threads] | win <face> <i0> <i1> <j0> <j1> <seed> [threads]".into()),
    }
    Ok(())
}
