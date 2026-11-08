//! SPIKE-105 F2 (spike branch only, never merged): does the carve on one catchment give the carve of a larger window inside it, and what does a domain carve cost?
//! `f2_domain_carve win` : 1228 x 1228 window of face 2 at 814 m around (4800, 4032); carve whole; carve the largest sea catchment D alone (outside = outlet at base height, then wall).
//! `f2_domain_carve cost CELLS_PER_EDGE N` : default carve of an N x N window of a face with that many cells per edge (12288 = 814 m, 98304 = 102 m), timed.
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::face::FaceSpec;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::{route, NONE};
use planet_generators::bake::Grid;

const THREADS: usize = 4;

fn sample(inputs: &BakeInputs, spec: &FaceSpec, i0: i64, j0: i64, n: usize) -> Vec<f64> {
    let mut z = vec![0.0f64; n * n];
    let rows_per = n.div_ceil(THREADS);
    std::thread::scope(|s| {
        for (k, chunk) in z.chunks_mut(rows_per * n).enumerate() {
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows_per * n + c;
                    let d: Vec3 = spec.halo_direction(i0 + (p % n) as i64, j0 + (p / n) as i64);
                    *v = inputs.base(d);
                }
            });
        }
    });
    z
}

fn spec(cells: usize) -> FaceSpec {
    FaceSpec { face: Face(2), cells, radius_m: 6_371_000.0, sea_level: 0.0, seed: 14, halo: 0 }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed = 14u64;
    let inputs = BakeInputs::new(seed, THREADS);
    let p = CarveParams::default();
    if a[1] == "cost" {
        let cells: usize = a[2].parse().unwrap();
        let n: usize = a[3].parse().unwrap();
        let sp = spec(cells);
        let (i0, j0) = ((cells as f64 * 0.390625) as i64 - n as i64 / 2, (cells as f64 * 0.328125) as i64 - n as i64 / 2);
        let t = std::time::Instant::now();
        let mut z = sample(&inputs, &sp, i0, j0, n);
        let ts = t.elapsed().as_secs_f64();
        let grid = Grid::new(n, n, sp.dx()).with_origin((i0 as u32, j0 as u32));
        let outlet = grid.outlets(&z, 0.0);
        let t = std::time::Instant::now();
        carve(&grid, &mut z, &outlet, &p, None, seed);
        println!("cost cells/edge {cells} window {n}x{n} dx {:.1} m: base {ts:.1} s, carve {:.1} s", sp.dx(), t.elapsed().as_secs_f64());
        return;
    }
    let sp = spec(12_288);
    let n = 1228usize;
    let (i0, j0) = (4800 - 614i64, 4032 - 614i64);
    let grid = Grid::new(n, n, sp.dx()).with_origin((i0 as u32, j0 as u32));
    let t = std::time::Instant::now();
    let base = sample(&inputs, &sp, i0, j0, n);
    println!("base {:.1} s; land {:.1} %", t.elapsed().as_secs_f64(), 100.0 * base.iter().filter(|v| **v > 0.0).count() as f64 / base.len() as f64);
    // Step 2: whole window.
    let mut zw = base.clone();
    let outlet = grid.outlets(&zw, 0.0);
    let t = std::time::Instant::now();
    let rw = carve(&grid, &mut zw, &outlet, &p, None, seed);
    println!("whole-window carve {:.1} s", t.elapsed().as_secs_f64());
    // Step 3: catchments of the original routing.
    let r0 = route(&grid, &base, &outlet, p.epsilon, seed, 0.0);
    let mut root = vec![0u32; n * n];
    for &c in &r0.order {
        let c = c as usize;
        root[c] = if r0.receiver[c] == NONE { c as u32 } else { root[r0.receiver[c] as usize] };
    }
    let mut size = std::collections::HashMap::<u32, usize>::new();
    for c in 0..n * n {
        *size.entry(root[c]).or_default() += 1;
    }
    let mut roots: Vec<(u32, usize)> = size.into_iter().filter(|(r, _)| !grid.is_edge(*r as usize)).collect();
    roots.sort_by_key(|x| std::cmp::Reverse(x.1));
    let mut border_roots: Vec<usize> = Vec::new();
    {
        let mut bs = std::collections::HashMap::<u32, usize>::new();
        for c in 0..n * n {
            if grid.is_edge(root[c] as usize) {
                *bs.entry(root[c]).or_default() += 1;
            }
        }
        border_roots.extend(bs.values().copied());
        border_roots.sort_unstable_by(|a, b| b.cmp(a));
    }
    println!("largest interior-outlet catchments (cells): {:?}; largest border-outlet catchments: {:?}", &roots[..roots.len().min(5)].iter().map(|x| x.1).collect::<Vec<_>>(), &border_roots[..border_roots.len().min(5)]);
    let Some(&(droot, dn)) = roots.first() else {
        println!("no interior-outlet catchment in this window");
        return;
    };
    let ind: Vec<bool> = (0..n * n).map(|c| root[c] == droot).collect();
    println!("D: root cell ({}, {}), {dn} cells ({:.0} km2)", droot as usize % n, droot as usize / n, dn as f64 * sp.dx() * sp.dx() / 1e6);
    // Divide crossings in the whole-window final routing.
    let out_of_d = (0..n * n).filter(|c| ind[*c] && rw.receiver[*c] != NONE && !ind[rw.receiver[*c] as usize]).count();
    let into_d = (0..n * n).filter(|c| !ind[*c] && rw.receiver[*c] != NONE && ind[rw.receiver[*c] as usize]).count();
    println!("whole-window final routing: D cells draining out of D {out_of_d}; outside cells draining into D {into_d}");
    // Step 4: D alone.
    for wall in [false, true] {
        let mut zd = base.clone();
        let mut od = vec![true; n * n];
        for c in 0..n * n {
            if ind[c] {
                od[c] = outlet[c];
            } else if wall {
                zd[c] += 1.0e5;
            }
        }
        let t = std::time::Instant::now();
        let rd = carve(&grid, &mut zd, &od, &p, None, seed);
        let secs = t.elapsed().as_secs_f64();
        let (mut mx, mut cnt, mut sum2) = (0.0f64, 0usize, 0.0f64);
        for c in 0..n * n {
            if ind[c] {
                let d = (zd[c] - zw[c]).abs();
                if d > 0.0 {
                    cnt += 1;
                }
                mx = mx.max(d);
                sum2 += d * d;
            }
        }
        let leak = (0..n * n).filter(|c| ind[*c] && rd.receiver[*c] != NONE && !ind[rd.receiver[*c] as usize]).count();
        println!(
            "D alone, outside = {}: {secs:.1} s, max |dz| in D {mx:.3} m, rms {:.3} m, cells differing {cnt} of {dn}, D cells draining out of D in its own routing {leak}",
            if wall { "wall (outlet at +1e5 m)" } else { "outlet at base height" },
            (sum2 / dn as f64).sqrt()
        );
    }
}
