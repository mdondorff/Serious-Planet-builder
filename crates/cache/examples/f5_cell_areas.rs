//! SPIKE-105 F5 (spike branch only, never merged): bias of treating cell areas as uniform. `f5_cell_areas CELLS THREADS`.
//! Area method: exact spherical quad area of the four corner directions (two spherical triangles, Van Oosterom-Strakhov: tan(E/2) = |a.(b x c)| / (1 + a.b + b.c + c.a)).

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::face::{sample_base, FaceSpec};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::route::route_weighted;
use planet_generators::bake::Grid;

const R: f64 = 6_371_000.0;

fn tri(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let num = a.dot(b.cross(c)).abs();
    let den = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
    2.0 * num.atan2(den)
}

fn dir(face: Face, n: f64, i: f64, j: f64) -> Vec3 {
    face_to_direction(&TangentWarp, face, 2.0 * i / n - 1.0, 2.0 * j / n - 1.0)
}

fn cell_area(face: Face, n: f64, i: f64, j: f64) -> f64 {
    let (p00, p10, p11, p01) = (dir(face, n, i, j), dir(face, n, i + 1.0, j), dir(face, n, i + 1.0, j + 1.0), dir(face, n, i, j + 1.0));
    (tri(p00, p10, p11) + tri(p00, p11, p01)) * R * R
}

fn pct(v: &mut Vec<f64>, q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * q) as usize]
}

fn gc(a: Vec3, b: Vec3) -> f64 {
    R * a.cross(b).length().atan2(a.dot(b))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cells: usize = a[1].parse().unwrap();
    let threads: usize = a[2].parse().unwrap();
    let (seed, sea) = (14u64, 0.0f64);
    let face = Face(2);
    let n = cells as f64;
    let dx = R * std::f64::consts::FRAC_PI_2 / n;
    let dx2 = dx * dx;

    // --- 1. areas over the face's own cells ---
    let (mut mn, mut mx, mut sum) = (f64::MAX, 0.0f64, 0.0f64);
    for j in 0..cells {
        for i in 0..cells {
            let ar = cell_area(face, n, i as f64, j as f64);
            mn = mn.min(ar);
            mx = mx.max(ar);
            sum += ar;
        }
    }
    let face_true = 4.0 * std::f64::consts::PI * R * R / 6.0;
    let centre = cell_area(face, n, n / 2.0 - 1.0, n / 2.0 - 1.0);
    let corner = cell_area(face, n, 0.0, 0.0);
    println!("N {cells}, dx {dx:.1} m, dx^2 {dx2:.4e} m2");
    println!("area min {mn:.4e} max {mx:.4e} mean {:.4e} m2; max/min {:.4}", sum / (n * n), mx / mn);
    println!("area/dx^2: min {:.5} max {:.5} mean {:.5}; centre cell {:.5}, corner cell {:.5}; corner/centre {:.5}", mn / dx2, mx / dx2, sum / (n * n) / dx2, centre / dx2, corner / dx2, corner / centre);
    println!("face sum {sum:.6e} vs 4piR2/6 {face_true:.6e}: rel err {:.2e}", (sum - face_true).abs() / face_true);

    // --- 3. distances ---
    let c = |i: f64, j: f64| dir(face, n, i + 0.5, j + 0.5);
    let ci = (cells / 2) as f64;
    let (o, ox, oy, od) = (c(ci, ci), c(ci + 1.0, ci), c(ci, ci + 1.0), c(ci + 1.0, ci + 1.0));
    println!("centre: axis-i {:.5} axis-j {:.5} diag {:.5} (x dx / dx*sqrt2)", gc(o, ox) / dx, gc(o, oy) / dx, gc(o, od) / (dx * 2f64.sqrt()));
    let (o, ox, oy, od) = (c(0.0, 0.0), c(1.0, 0.0), c(0.0, 1.0), c(1.0, 1.0));
    println!("corner: axis-i {:.5} axis-j {:.5} diag {:.5} (x dx / dx*sqrt2)", gc(o, ox) / dx, gc(o, oy) / dx, gc(o, od) / (dx * 2f64.sqrt()));
    let (o, ox) = (c(0.0, ci), c(1.0, ci));
    let (oy, o2) = (c(0.0, ci + 1.0), c(0.0, ci));
    println!("edge middle: across-edge {:.5} along-edge {:.5}", gc(o, ox) / dx, gc(o2, oy) / dx);

    // --- 2. routing ---
    let inputs = BakeInputs::new(seed, threads);
    let spec = FaceSpec { face, cells, radius_m: R, sea_level: sea, seed, halo: 4 };
    let z = sample_base(&spec, &|d| inputs.base(d), threads);
    let m = spec.grid_cells();
    let grid = Grid::new(m, m, dx);
    let outlet = grid.outlets(&z, sea);
    let h = 4i64;
    let area_true: Vec<f64> = (0..grid.len()).map(|p| cell_area(face, n, (p % m) as f64 - h as f64, (p / m) as f64 - h as f64)).collect();
    let w: Vec<f32> = area_true.iter().map(|a| (a / dx2) as f32).collect();
    let ru = route_weighted(&grid, &z, &outlet, 1e-3, seed, 0.0, None);
    let rw = route_weighted(&grid, &z, &outlet, 1e-3, seed, 0.0, Some(&w));
    println!("receivers identical: {}", ru.receiver == rw.receiver);
    // unweighted area in m2 is already cells*dx^2; weighted is in true m2 (own = w*dx^2)
    let own_w = |p: usize| f64::from(w[p]) * dx2;
    let mut list: Vec<(usize, f64, f64)> = (0..grid.len()).filter(|&p| outlet[p] && rw.area[p] > 1.5 * dx2).map(|p| (p, rw.area[p], ru.area[p])).collect();
    list.sort_by(|x, y| y.1.total_cmp(&x.1));
    println!("10 largest catchments (outlet cell; weighted km2; unweighted km2; rel diff unweighted vs weighted):");
    for (k, (p, aw, au)) in list.iter().take(10).enumerate() {
        println!("  #{} cell {p} {:.4e} {:.4e} {:+.3} %", k + 1, aw / 1e6, au / 1e6, 100.0 * (au / aw - 1.0));
    }
    // conservation
    let tot_out_w: f64 = (0..grid.len()).filter(|&p| outlet[p]).map(|p| rw.area[p]).sum();
    let tot_true: f64 = area_true.iter().sum();
    let tot_out_u: f64 = (0..grid.len()).filter(|&p| outlet[p]).map(|p| ru.area[p]).sum();
    println!("conservation weighted: outlets total {tot_out_w:.6e} vs sum of true areas {tot_true:.6e}: rel err {:.2e}", (tot_out_w - tot_true).abs() / tot_true);
    println!("conservation unweighted: outlets total {tot_out_u:.6e} vs sum of true areas: rel err {:+.2e}", tot_out_u / tot_true - 1.0);
    let _ = own_w;

    // channel cells
    let chan: Vec<usize> = (0..grid.len()).filter(|&p| !outlet[p] && rw.area[p] >= 50e6).collect();
    let mut rel: Vec<f64> = chan.iter().map(|&p| (ru.area[p] / rw.area[p] - 1.0).abs()).collect();
    let mx = rel.iter().cloned().fold(0.0, f64::max);
    println!("channel cells (true area >= 50 km2): {}; |unweighted/weighted - 1|: p50 {:.4} p95 {:.4} max {:.4}", chan.len(), pct(&mut rel, 0.5), pct(&mut rel, 0.95), mx);
    let inner = |p: usize| {
        let (i, j) = ((p % m) as f64 - h as f64 + 0.5, (p / m) as f64 - h as f64 + 0.5);
        let (s, t) = (2.0 * i / n - 1.0, 2.0 * j / n - 1.0);
        s.abs().max(t.abs()) < std::f64::consts::FRAC_1_SQRT_2
    };
    for t in [10e6, 50e6, 100e6] {
        let (mut nw, mut nu, mut diff, mut d_in, mut c_in, mut u_only, mut w_only) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        for p in 0..grid.len() {
            if outlet[p] {
                continue;
            }
            let (iw, iu) = (rw.area[p] >= t, ru.area[p] >= t);
            nw += iw as usize;
            nu += iu as usize;
            if iw {
                c_in += inner(p) as usize;
            }
            if iw != iu {
                diff += 1;
                d_in += inner(p) as usize;
                if iu { u_only += 1 } else { w_only += 1 }
            }
        }
        println!(
            "T {:>3} km2: true-area channel cells {nw}, unweighted {nu}; differing {diff} = {:.3} % of true channel cells (unweighted-only {u_only}, weighted-only {w_only}); changed in inner-50%-by-cells {:.1} % (channel cells there: {:.1} %)",
            t / 1e6,
            100.0 * diff as f64 / nw.max(1) as f64,
            100.0 * d_in as f64 / diff.max(1) as f64,
            100.0 * c_in as f64 / nw.max(1) as f64
        );
    }
}
