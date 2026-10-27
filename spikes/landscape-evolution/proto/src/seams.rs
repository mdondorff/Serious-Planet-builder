//! SPIKE-33: the seam between two neighbouring blocks as computed, and with SPIKE-31's overlap blend.
use super::exp::setup;
use super::lem::*;
use super::look::save_gray;
use super::net::evolve_net;
use super::*;

const CH: f64 = 5.0e4; // m2: channel cells
const CORE: usize = 250;

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() - 1) as f64 * q) as usize]
}

struct Blk {
    i0: usize,
    j0: usize,
    w: usize,
    out: Out,
}

impl Blk {
    fn has(&self, x: usize, y: usize) -> bool {
        x >= self.i0 && y >= self.j0 && x < self.i0 + self.w && y < self.j0 + self.w
    }
    fn at(&self, v: &[f64], x: usize, y: usize) -> f64 {
        v[(y - self.j0) * self.w + x - self.i0]
    }
    fn z(&self, x: usize, y: usize) -> f64 {
        self.at(&self.out.z, x, y)
    }
    fn a(&self, x: usize, y: usize) -> f64 {
        self.at(&self.out.a, x, y)
    }
    /// Lattice position of the receiver of (x, y).
    fn rec(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let r = self.out.rec[(y - self.j0) * self.w + x - self.i0];
        if r == u32::MAX {
            None
        } else {
            Some((self.i0 + r as usize % self.w, self.j0 + r as usize / self.w))
        }
    }
    /// Distance in cells to the block's own border.
    fn dist(&self, x: usize, y: usize) -> f64 {
        let (dx, dy) = (x - self.i0, y - self.j0);
        dx.min(self.w - 1 - dx).min(dy).min(self.w - 1 - dy) as f64
    }
}

fn run(site: &Site, cs: &CoarseState, bi: usize, bj: usize, hc: usize, n: u32, net: bool) -> Blk {
    let (ci, cj) = (250 + bi * CORE, 250 + bj * CORE);
    let (i0, j0, w) = (ci - hc, cj - hc, CORE + 2 * hc);
    if net {
        let dm = site.domain_net(cs, i0, j0, w, w);
        let (out, cyc) = evolve_net(&dm, n, &PARAMS);
        println!("NETBLOCK ({bi},{bj}): {:.1} s, cells cut from cycles {cyc}", out.ms / 1000.0);
        return Blk { i0, j0, w, out };
    }
    let dm = site.domain(cs, i0, j0, w, w, true);
    Blk { i0, j0, w, out: evolve(&dm, n, &PARAMS, false) }
}

struct Rep {
    s: String,
}
impl Rep {
    fn line(&mut self, l: String) {
        println!("{l}");
        self.s += &(l + "\n");
    }
}

fn seam_stats(rep: &mut Rep, label: &str, f: &dyn Fn(usize, usize) -> f64, ds: f64, x: usize, rows: std::ops::Range<usize>, refx: usize) {
    let (mut step, mut jump, mut rstep, mut rjump) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for y in rows {
        step.push((f(x, y) - f(x - 1, y)).abs());
        jump.push((((f(x + 1, y) - f(x, y)) - (f(x - 1, y) - f(x - 2, y))) / ds).abs());
        rstep.push((f(refx, y) - f(refx - 1, y)).abs());
        rjump.push((((f(refx + 1, y) - f(refx, y)) - (f(refx - 1, y) - f(refx - 2, y))) / ds).abs());
    }
    let q = |v: &Vec<f64>| format!("p50 {:.3} p90 {:.3} p99 {:.3} max {:.3}", pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), v.iter().cloned().fold(0.0, f64::max));
    rep.line(format!("SEAM {label}: height step between neighbouring cells across the seam (m): {}; interior reference {}", q(&step), q(&rstep)));
    rep.line(format!("SEAM {label}: slope jump (second difference / ds, 1/m): {}; interior reference {}", q(&jump), q(&rjump)));
}

pub fn seams(coarse_n: usize, coarse_steps: usize, threads: usize, out: &str, net: bool) {
    let (cs, mut site) = setup(coarse_n, coarse_steps, threads, 20.0, 20000.0, 400.0);
    if net {
        site.build_net(&cs, 100.0e6);
        println!("NET {} coarse river segments (A >= 100 km2) touch the lattice", site.segs.len());
    }
    let site = site;
    let (hc, n) = (250usize, 30u32); // H = 5 km
    let (b1, b2) = {
        let (cs, site) = (&cs, &site);
        std::thread::scope(|sc| {
            let h1 = sc.spawn(move || run(site, cs, 0, 0, hc, n, net));
            let h2 = sc.spawn(move || run(site, cs, 1, 0, hc, n, net));
            (h1.join().unwrap(), h2.join().unwrap())
        })
    };
    let mut rep = Rep { s: String::new() };
    if net {
        let dm = site.domain_net(&cs, 0, 0, 1000, 1000);
        let (t, cyc) = evolve_net(&dm, n, &PARAMS);
        rep.line(format!("NET truth (whole 20 km lattice, frozen network): {:.1} s, {cyc} cycle cells", t.ms / 1000.0));
        for (name, b) in [("block 1", &b1), ("block 2", &b2)] {
            let (mut dz, mut dl) = (Vec::new(), Vec::new());
            let (ci, cj) = (if name == "block 1" { 250 } else { 500 }, 250);
            for y in cj..cj + CORE {
                for x in ci..ci + CORE {
                    dz.push((b.z(x, y) - t.z[y * 1000 + x]).abs());
                    dl.push((libm::log(b.a(x, y).max(1.0)) - libm::log(t.a[y * 1000 + x].max(1.0))).abs());
                }
            }
            rep.line(format!("NET {name} against the truth with the same network: |dz| p50 {:.3} p90 {:.3} p99 {:.2} max {:.1} m; |dlnA| mean {:.3} p99 {:.2}", pct(&dz, 0.5), pct(&dz, 0.9), pct(&dz, 0.99), dz.iter().cloned().fold(0.0, f64::max), dl.iter().sum::<f64>() / dl.len() as f64, pct(&dl, 0.99)));
        }
    }
    rep.line(format!("SEAMS block 1 core x 250..500, block 2 core x 500..750, y 250..500, H = {} km, N = {n}, ds {} m, blocks {:.1} s and {:.1} s", hc as f64 * site.ds / 1000.0, site.ds, b1.out.ms / 1000.0, b2.out.ms / 1000.0));
    let rows = 250..500;
    // (a) as computed: the hard join at x = 500
    let hard = |x: usize, y: usize| if x < 500 { b1.z(x, y) } else { b2.z(x, y) };
    seam_stats(&mut rep, "hard join", &hard, site.ds, 500, rows.clone(), 375);
    let (mut brk, mut tot) = (0usize, 0usize);
    for y in rows.clone() {
        let (c1, c2) = (b1.a(499, y) > CH, b2.a(500, y) > CH);
        if c1 || c2 {
            tot += 1;
            if c1 != c2 {
                brk += 1;
            }
        }
    }
    rep.line(format!("SEAM hard join: of {tot} rows where a channel (A > {CH:.0} m2) reaches the seam on either side, {brk} have it on one side only"));
    // channel cells within 3 cells of the seam: does the hard-joined surface run uphill along the receiver
    let (mut up, mut chn) = (0usize, 0usize);
    for y in rows.clone() {
        for x in 497..503 {
            let b = if x < 500 { &b1 } else { &b2 };
            if b.a(x, y) > CH {
                if let Some((rx, ry)) = b.rec(x, y) {
                    chn += 1;
                    if hard(rx, ry) > hard(x, y) {
                        up += 1;
                    }
                }
            }
        }
    }
    rep.line(format!("SEAM hard join: {up} of {chn} channel cells within 3 cells of the seam have a receiver higher than the cell in the joined surface"));
    // (b) the overlap blend: each block weighted by its distance to its own border
    let blend = |x: usize, y: usize| -> f64 {
        if !b2.has(x, y) {
            return b1.z(x, y);
        }
        if !b1.has(x, y) {
            return b2.z(x, y);
        }
        let (d1, d2) = (b1.dist(x, y), b2.dist(x, y));
        if d1 + d2 <= 0.0 {
            return 0.5 * (b1.z(x, y) + b2.z(x, y));
        }
        (d1 * b1.z(x, y) + d2 * b2.z(x, y)) / (d1 + d2)
    };
    seam_stats(&mut rep, "overlap blend", &blend, site.ds, 500, rows.clone(), 375);
    // the blend zone: x 250..750 (both rectangles), rows 250..500
    let (mut up, mut chn, mut dis, mut allc) = (0usize, 0usize, 0usize, 0usize);
    let (mut dzs, mut dz_ref) = (Vec::new(), Vec::new());
    for y in rows.clone() {
        for x in 252..748 {
            let (d1, d2) = (b1.dist(x, y), b2.dist(x, y));
            dzs.push((b1.z(x, y) - b2.z(x, y)).abs());
            let (a1, a2) = (b1.a(x, y), b2.a(x, y));
            if a1 > CH || a2 > CH {
                allc += 1;
                if a1.min(a2) < CH / 5.0 {
                    dis += 1;
                }
            }
            let dom = if d1 >= d2 { &b1 } else { &b2 };
            if dom.a(x, y) > CH {
                if let Some((rx, ry)) = dom.rec(x, y) {
                    chn += 1;
                    if blend(rx, ry) > blend(x, y) {
                        up += 1;
                    }
                }
            }
        }
        dz_ref.push(0.0);
    }
    rep.line(format!("SEAM blend zone (x 252..748): the two blocks differ by |dz| p50 {:.2} p90 {:.2} p99 {:.2} max {:.1} m", pct(&dzs, 0.5), pct(&dzs, 0.9), pct(&dzs, 0.99), dzs.iter().cloned().fold(0.0, f64::max)));
    rep.line(format!("SEAM blend zone: {up} of {chn} channel cells (dominant block, A > {CH:.0} m2) have a receiver higher than the cell in the blended surface ({:.2} %)", 100.0 * up as f64 / chn.max(1) as f64));
    rep.line(format!("SEAM blend zone: of {allc} cells that are a channel in either block, {dis} are under {:.0} m2 in the other ({:.1} %): the channel network does not agree", CH / 5.0, 100.0 * dis as f64 / allc.max(1) as f64));
    // (c) stills of the border: a raymarched heightfield, hard join against blend
    let (w, h) = (1000usize, 750usize);
    let mut zh = vec![0f64; w * h];
    let mut zbl = vec![0f64; w * h];
    for y in 0..h {
        for x in 0..w {
            if b1.has(x, y) || b2.has(x, y) {
                zh[y * w + x] = if x < 500 { if b1.has(x, y) { b1.z(x, y) } else { b2.z(x, y) } } else if b2.has(x, y) { b2.z(x, y) } else { b1.z(x, y) };
                zbl[y * w + x] = blend(x, y);
            }
        }
    }
    for (name, cx, cy, eye, pitch) in [("300m", 300.0, 375.0, 300.0, -6.0f64), ("30m", 440.0, 375.0, 30.0, 0.0)] {
        let mut sheet = vec![0u8; 960 * 270];
        for (k, z) in [&zh, &zbl].iter().enumerate() {
            let img = render(z, w, h, site.ds, cx, cy, eye, pitch, 480, 270);
            for r in 0..270 {
                sheet[r * 960 + k * 480..r * 960 + (k + 1) * 480].copy_from_slice(&img[r * 480..(r + 1) * 480]);
            }
        }
        save_gray(&format!("{out}/seam{}_{name}.png", if net { "-net" } else { "" }), 960, 270, &sheet);
    }
    std::fs::write(format!("{out}/seams{}.txt", if net { "-net" } else { "" }), rep.s).ok();
}

fn sample(z: &[f64], w: usize, h: usize, x: f64, y: f64) -> Option<f64> {
    if x < 1.0 || y < 1.0 || x >= (w - 2) as f64 || y >= (h - 2) as f64 {
        return None;
    }
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let i = yi as usize * w + xi as usize;
    Some(z[i] * (1.0 - fx) * (1.0 - fy) + z[i + 1] * fx * (1.0 - fy) + z[i + w] * (1.0 - fx) * fy + z[i + w + 1] * fx * fy)
}

/// Look along +x from (cx, cy) in cells, `eye` metres above the ground, 70 degrees wide.
fn render(z: &[f64], w: usize, h: usize, ds: f64, cx: f64, cy: f64, eye: f64, pitch_deg: f64, pw: usize, ph: usize) -> Vec<u8> {
    let ez = sample(z, w, h, cx, cy).unwrap() + eye;
    let fov_h = 70f64.to_radians();
    let fov_v = fov_h * ph as f64 / pw as f64;
    let sun = {
        let v = (0.5f64, -0.6f64, 0.55f64);
        let l = libm::sqrt(v.0 * v.0 + v.1 * v.1 + v.2 * v.2);
        (v.0 / l, v.1 / l, v.2 / l)
    };
    let mut img = vec![0u8; pw * ph];
    for py in 0..ph {
        for px in 0..pw {
            let a = ((px as f64 + 0.5) / pw as f64 - 0.5) * fov_h;
            let phi = pitch_deg.to_radians() + (0.5 - (py as f64 + 0.5) / ph as f64) * fov_v;
            let (dx, dy, tp) = (libm::cos(a), libm::sin(a), libm::tan(phi));
            let mut t = 2.0;
            let mut val = (205.0 - 90.0 * (0.5 - py as f64 / ph as f64).max(0.0)) as u8;
            while t < 14000.0 {
                let (x, y) = (cx + dx * t / ds, cy + dy * t / ds);
                let Some(g) = sample(z, w, h, x, y) else { break };
                if ez + t * tp < g {
                    let gx = (sample(z, w, h, x + 1.0, y).unwrap_or(g) - sample(z, w, h, x - 1.0, y).unwrap_or(g)) / (2.0 * ds);
                    let gy = (sample(z, w, h, x, y + 1.0).unwrap_or(g) - sample(z, w, h, x, y - 1.0).unwrap_or(g)) / (2.0 * ds);
                    let nn = 1.0 / libm::sqrt(1.0 + gx * gx + gy * gy);
                    let l = ((-gx) * nn * sun.0 + (-gy) * nn * sun.1 + nn * sun.2).max(0.0);
                    let fog = libm::exp(-t / 9000.0);
                    val = (205.0 * (1.0 - fog) + (25.0 + 210.0 * l) * fog).min(255.0) as u8;
                    break;
                }
                t += (ds * 0.5).max(t * 0.003);
            }
            img[py * pw + px] = val;
        }
    }
    img
}
