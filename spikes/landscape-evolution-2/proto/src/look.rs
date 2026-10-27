//! SPIKE-33: the look (hillshades of A, B, C and main at one site), the cost of B and C, and the LOD stability of A.
use super::lem::*;
use super::net::evolve_net;
use super::*;

pub fn hillshade(z: &[f64], w: usize, ds: f64, az_deg: f64, el_deg: f64, x0: usize, y0: usize, cw: usize, step: usize) -> Vec<u8> {
    let (az, el) = (az_deg.to_radians(), el_deg.to_radians());
    let l = (libm::sin(az) * libm::cos(el), libm::cos(az) * libm::cos(el), libm::sin(el));
    let mut out = Vec::new();
    for oy in 0..cw / step {
        for ox in 0..cw / step {
            let (i, j) = (x0 + ox * step, y0 + (cw / step - 1 - oy) * step);
            let (zx, zy) = ((z[j * w + i + 1] - z[j * w + i - 1]) / (2.0 * ds), (z[(j + 1) * w + i] - z[(j - 1) * w + i]) / (2.0 * ds));
            let n = 1.0 / libm::sqrt(1.0 + zx * zx + zy * zy);
            let d = ((-zx) * n * l.0 + (-zy) * n * l.1 + n * l.2).max(0.0);
            out.push((20.0 + 235.0 * libm::pow(d, 0.8)).min(255.0) as u8);
        }
    }
    out
}

pub fn save_gray(path: &str, w: u32, h: u32, g: &[u8]) {
    let f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut e = png::Encoder::new(f, w, h);
    e.set_color(png::ColorType::Grayscale);
    e.set_depth(png::BitDepth::Eight);
    e.set_compression(png::Compression::High);
    e.write_header().unwrap().write_image_data(g).unwrap();
    println!("wrote {path} ({w}x{h}, {} KB)", std::fs::metadata(path).unwrap().len() / 1024);
}

fn bilinear_h(h: &[f64], n: usize, x: f64, y: f64) -> (f64, f64, f64) {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let i = (yi as usize) * n + xi as usize;
    let (h00, h10, h01, h11) = (h[i], h[i + 1], h[i + n], h[i + n + 1]);
    let gx = (h10 - h00) * (1.0 - fy) + (h11 - h01) * fy;
    let gy = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;
    (h00 * (1.0 - fx) * (1.0 - fy) + h10 * fx * (1.0 - fy) + h01 * (1.0 - fx) * fy + h11 * fx * fy, gx, gy)
}

/// SPIKE-32's droplet erosion (variant C), unchanged except for the grid size.
fn droplet_erosion(h: &mut [f64], n: usize, droplets: u32, life: u32, capacity_k: f64) {
    let mut state = 7u64.wrapping_mul(0x9e37_79b9_7f4a_7c15).wrapping_add(1);
    let mut rnd = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let radius = 3i32;
    let mut brush: Vec<(i32, i32, f64)> = Vec::new();
    let mut total = 0.0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d = libm::sqrt((dx * dx + dy * dy) as f64);
            if d <= f64::from(radius) {
                let w = 1.0 - d / (f64::from(radius) + 1.0);
                brush.push((dx, dy, w));
                total += w;
            }
        }
    }
    for b in &mut brush {
        b.2 /= total;
    }
    let (inertia, erode_k, deposit_k, evaporate, gravity) = (0.25, 0.3, 0.3, 0.012, 4.0);
    let hi = (n - 5) as f64;
    for _ in 0..droplets {
        let (mut x, mut y) = (4.0 + rnd() * (hi - 4.0), 4.0 + rnd() * (hi - 4.0));
        let (mut dx, mut dy, mut speed, mut water, mut sediment) = (0.0f64, 0.0f64, 1.0f64, 1.0f64, 0.0f64);
        for _ in 0..life {
            let (h0, gx, gy) = bilinear_h(h, n, x, y);
            dx = dx * inertia - gx * (1.0 - inertia);
            dy = dy * inertia - gy * (1.0 - inertia);
            let len = libm::sqrt(dx * dx + dy * dy);
            if len < 1e-9 {
                let a = rnd() * core::f64::consts::TAU;
                dx = libm::cos(a);
                dy = libm::sin(a);
            } else {
                dx /= len;
                dy /= len;
            }
            let (ox, oy) = (x, y);
            x += dx;
            y += dy;
            if x < 4.0 || y < 4.0 || x >= hi || y >= hi {
                break;
            }
            let (h1, _, _) = bilinear_h(h, n, x, y);
            let dh = h1 - h0;
            let capacity = (-dh).max(0.02) * speed * water * capacity_k;
            let (xi, yi) = (ox.floor(), oy.floor());
            let (fx, fy) = (ox - xi, oy - yi);
            let ci = (yi as usize) * n + xi as usize;
            if sediment > capacity || dh > 0.0 {
                let amount = if dh > 0.0 { dh.min(sediment) } else { (sediment - capacity) * deposit_k };
                sediment -= amount;
                h[ci] += amount * (1.0 - fx) * (1.0 - fy);
                h[ci + 1] += amount * fx * (1.0 - fy);
                h[ci + n] += amount * (1.0 - fx) * fy;
                h[ci + n + 1] += amount * fx * fy;
            } else {
                let amount = ((capacity - sediment) * erode_k).min(-dh);
                for &(bx, by, w) in &brush {
                    let q = ((yi as i32 + by) as usize) * n + (xi as i32 + bx) as usize;
                    let take = (amount * w).min(h[q].max(0.0));
                    h[q] -= take;
                    sediment += take;
                }
            }
            speed = libm::sqrt((speed * speed - dh * gravity).max(0.0));
            water *= 1.0 - evaporate;
        }
    }
}

fn thermal(h: &mut [f64], n: usize, ds: f64, passes: u32, talus: f64) {
    for _ in 0..passes {
        let mut delta = vec![0f64; n * n];
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let c = j * n + i;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let q = (j as i32 + dy) as usize * n + (i as i32 + dx) as usize;
                    let dist = if dx != 0 && dy != 0 { ds * core::f64::consts::SQRT_2 } else { ds };
                    let s = (h[c] - h[q]) / dist;
                    if s > talus {
                        let m = 0.09 * (s - talus) * dist;
                        delta[c] -= m;
                        delta[q] += m;
                    }
                }
            }
        }
        for c in 0..n * n {
            h[c] += delta[c];
        }
    }
}

/// Variant B: erosion-like noise on the pre-erosion field. Creases (1 - |noise|)^2 at 400, 130 and 45 m, deepest on steep ground
/// (damped where the slope is below 0.3); the only neighbour read is the slope of the input.
fn gully_noise(z0: &[f64], n: usize, ds: f64) -> Vec<f64> {
    let mut o = z0.to_vec();
    for j in 1..n - 1 {
        for i in 1..n - 1 {
            let c = j * n + i;
            let (gx, gy) = ((z0[c + 1] - z0[c - 1]) / (2.0 * ds), (z0[c + n] - z0[c - n]) / (2.0 * ds));
            let s = libm::sqrt(gx * gx + gy * gy);
            let damp = (s / 0.3).clamp(0.0, 1.0);
            let (x, y) = (i as f64 * ds, j as f64 * ds);
            let mut g = 0.0;
            for (k, (wl, amp)) in [(400.0, 22.0), (130.0, 8.0), (45.0, 2.5)].iter().enumerate() {
                let r = 1.0 - perlin(500 + k as u32, x / wl, y / wl).abs();
                g += amp * r * r;
            }
            o[c] = z0[c] - damp * g;
        }
    }
    o
}

pub fn look(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str) {
    let (cs, _log) = coarse_run(coarse_n, threads, coarse_steps, 0.6, "look");
    let (c, lat, lon) = find_site(&cs);
    let ds = 4.0;
    let extent = 7000.0;
    let mut site = Site::build(&cs, c, lat, lon, ds, extent, 400.0, threads);
    site.build_net(&cs, 100.0e6);
    let n = site.lw;
    let km2 = (extent / 1000.0) * (extent / 1000.0);
    let dm = site.domain(&cs, 0, 0, n, n, true);
    let mut rep = String::new();
    let mut log = |s: String| {
        println!("{s}");
        rep += &(s + "\n");
    };
    log(format!("LOOK site lat {lat:.3} lon {lon:.3}, {n}x{n} cells of {ds} m = {km2:.0} km2, detail {} m", site.detail_m));
    let dmn = site.domain_net(&cs, 0, 0, n, n);
    let (onet, cyc) = evolve_net(&dmn, 30, &PARAMS);
    log(format!("COST A-net N=30: {:.1} s single thread for {km2:.0} km2 at {ds} m = {:.2} s/km2 ({:.0} ns per cell-step), {} coarse segments, {cyc} cycle cells", onet.ms / 1000.0, onet.ms / 1000.0 / km2, onet.ms * 1e6 / (n * n) as f64 / 30.0, site.segs.len()));
    let mut a_runs = Vec::new();
    for steps in [30u32] {
        let o = evolve(&dm, steps, &PARAMS, false);
        log(format!("COST A N={steps}: {:.1} s single thread for {km2:.0} km2 at {ds} m = {:.2} s/km2 ({:.0} ns per cell-step)", o.ms / 1000.0, o.ms / 1000.0 / km2, o.ms * 1e6 / (n * n) as f64 / steps as f64));
        a_runs.push(o);
    }
    let t = Instant::now();
    let zb = gully_noise(&dm.z0, n, ds);
    let tb = t.elapsed().as_secs_f64();
    log(format!("COST B (gully noise): {tb:.2} s single thread for {km2:.0} km2 = {:.3} s/km2", tb / km2));
    let mut zc = dm.z0.clone();
    let t = Instant::now();
    droplet_erosion(&mut zc, n, 1_000_000, 200, 0.1);
    thermal(&mut zc, n, ds, 12, 1.48);
    let tc = t.elapsed().as_secs_f64();
    log(format!("COST C (1 M droplets, life 200, thermal 12 passes at 56 deg): {tc:.1} s single thread for {km2:.0} km2 = {:.2} s/km2", tc / km2));
    let (cw, step) = (1248usize, 4usize);
    let x0 = (n - cw) / 2;
    let zmain = site.zc.clone();
    let sets: Vec<(&str, &[f64])> = vec![("main", &zmain), ("A_net", &onet.z), ("B", &zb), ("C", &zc), ("A_dyn_N30", &a_runs[0].z)];
    let pw = cw / step;
    for (az, el, name) in [(262.0, 4.0, "sunset"), (200.0, 55.0, "midday")] {
        let mut sheet = vec![0u8; pw * 4 * pw];
        for (k, (_, z)) in sets.iter().take(4).enumerate() {
            let g = hillshade(z, n, ds, az, el, x0, x0, cw, step);
            for r in 0..pw {
                sheet[r * pw * 4 + k * pw..r * pw * 4 + (k + 1) * pw].copy_from_slice(&g[r * pw..(r + 1) * pw]);
            }
        }
        save_gray(&format!("{out}/sheet_{name}.png"), (pw * 4) as u32, pw as u32, &sheet);
    }
    for (name, z) in &sets {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        let mut slopes = Vec::new();
        for j in (x0..x0 + cw).step_by(8) {
            for i in (x0..x0 + cw).step_by(8) {
                lo = lo.min(z[j * n + i]);
                hi = hi.max(z[j * n + i]);
                let (gx, gy) = ((z[j * n + i + 1] - z[j * n + i - 1]) / (2.0 * ds), (z[(j + 1) * n + i] - z[(j - 1) * n + i]) / (2.0 * ds));
                slopes.push(libm::atan(libm::sqrt(gx * gx + gy * gy)).to_degrees());
            }
        }
        slopes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        log(format!("SHAPE {name}: relief {:.0} m in the central 5 km, slope median {:.1} deg p90 {:.1} deg p99 {:.1} deg", hi - lo, slopes[slopes.len() / 2], slopes[slopes.len() * 9 / 10], slopes[slopes.len() * 99 / 100]));
    }
    std::fs::write(format!("{out}/look.txt"), rep).ok();
}

/// A fine tile and its parent: the same block run at 20 m and at 40 m, compared at the 40 m cells (the 20 m heights averaged 2 x 2).
pub fn lod(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str) {
    let (cs, _log) = coarse_run(coarse_n, threads, coarse_steps, 0.6, "lod");
    let (c, lat, lon) = find_site(&cs);
    let extent = 7000.0;
    let mut res = Vec::new();
    for ds in [20.0f64, 40.0] {
        let site = Site::build(&cs, c, lat, lon, ds, extent, 400.0, threads);
        let n = site.lw;
        let dm = site.domain(&cs, 0, 0, n, n, true);
        res.push((n, evolve(&dm, 30, &PARAMS, false), dm.z0));
    }
    let (n20, o20, z0_20) = (&res[0].0, &res[0].1, &res[0].2);
    let (n40, o40, z0_40) = (&res[1].0, &res[1].1, &res[1].2);
    let (mut dz, mut dz0, mut dl) = (Vec::new(), Vec::new(), Vec::new());
    for j in 20..n40 - 20 {
        for i in 20..n40 - 20 {
            let (i2, j2) = (i * 2, j * 2);
            let avg = |z: &[f64]| (z[j2 * n20 + i2] + z[j2 * n20 + i2 + 1] + z[(j2 + 1) * n20 + i2] + z[(j2 + 1) * n20 + i2 + 1]) / 4.0;
            dz.push((avg(&o20.z) - o40.z[j * n40 + i]).abs());
            dz0.push((avg(z0_20) - z0_40[j * n40 + i]).abs());
            let al = (o20.a[j2 * n20 + i2] + o20.a[j2 * n20 + i2 + 1] + o20.a[(j2 + 1) * n20 + i2] + o20.a[(j2 + 1) * n20 + i2 + 1]).max(1.0);
            dl.push((libm::log(al) - libm::log(o40.a[j * n40 + i].max(1.0))).abs());
        }
    }
    let p = |v: &mut Vec<f64>, q: f64| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[((v.len() - 1) as f64 * q) as usize]
    };
    let s = format!(
        "LOD 20 m run averaged 2x2 against the 40 m run (N=30): |dz| p50 {:.2} p90 {:.2} p99 {:.2} m (the initial fields differ by p50 {:.2} p90 {:.2} p99 {:.2}); |dlnA| median {:.2} p90 {:.2}",
        p(&mut dz.clone(), 0.5),
        p(&mut dz.clone(), 0.9),
        p(&mut dz.clone(), 0.99),
        p(&mut dz0.clone(), 0.5),
        p(&mut dz0.clone(), 0.9),
        p(&mut dz0.clone(), 0.99),
        p(&mut dl.clone(), 0.5),
        p(&mut dl.clone(), 0.9)
    );
    println!("{s}");
    std::fs::write(format!("{out}/lod.txt"), s).ok();
}
