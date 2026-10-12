//! SPIKE-03: global stream-power erosion on the cube sphere against the fallback (routing and rivers on the base heights).
//! Throwaway. `cargo run --release -p hydrology-spike` writes target/review/hydrology-spike/.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::Path;
use std::time::Instant;

use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::global::sample;
use planet_render::RgbaFrame;

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const NONE: u32 = u32::MAX;
const OFFSETS: [(i64, i64); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

struct Grid {
    res: usize,
    dir: Vec<[f32; 3]>,
    elev: Vec<f32>,
    cell_area: Vec<f32>,
}

fn face_of(g: &Grid, id: u32) -> usize {
    id as usize / (g.res * g.res)
}

impl Grid {
    fn n(&self) -> usize {
        self.elev.len()
    }

    fn id(&self, f: usize, i: usize, j: usize) -> u32 {
        (f * self.res * self.res + j * self.res + i) as u32
    }

    /// The cell next to `id` at offset `(di, dj)`: inside the face by index, across a face edge through the direction of the
    /// extended cell centre. `NONE` when the step lands on the cell itself (a cube corner).
    fn step(&self, id: u32, di: i64, dj: i64) -> u32 {
        let r = self.res as i64;
        let f = id as usize / (self.res * self.res);
        let rest = id as usize % (self.res * self.res);
        let (i, j) = ((rest % self.res) as i64, (rest / self.res) as i64);
        let (ni, nj) = (i + di, j + dj);
        if (0..r).contains(&ni) && (0..r).contains(&nj) {
            return self.id(f, ni as usize, nj as usize);
        }
        let s = (ni as f64 + 0.5) / r as f64 * 2.0 - 1.0;
        let t = (nj as f64 + 0.5) / r as f64 * 2.0 - 1.0;
        let dir = face_to_direction(&TangentWarp, Face(f as u8), s, t);
        let (f2, s2, t2) = direction_to_face(&TangentWarp, dir);
        let i2 = (((s2 + 1.0) * 0.5 * r as f64) as i64).clamp(0, r - 1) as usize;
        let j2 = (((t2 + 1.0) * 0.5 * r as f64) as i64).clamp(0, r - 1) as usize;
        let out = self.id(f2.0 as usize, i2, j2);
        if out == id {
            NONE
        } else {
            out
        }
    }

    fn nbrs(&self, id: u32) -> [u32; 8] {
        let mut out = [NONE; 8];
        for (k, (di, dj)) in OFFSETS.iter().enumerate() {
            out[k] = self.step(id, *di, *dj);
        }
        out
    }

    fn dist(&self, a: u32, b: u32) -> f64 {
        let (p, q) = (self.dir[a as usize], self.dir[b as usize]);
        let d = (f64::from(p[0] - q[0]).powi(2) + f64::from(p[1] - q[1]).powi(2) + f64::from(p[2] - q[2]).powi(2)).sqrt();
        2.0 * libm::asin((d / 2.0).min(1.0)) * R
    }

    fn lookup(&self, dir: Vec3) -> u32 {
        let (f, s, t) = direction_to_face(&TangentWarp, dir);
        let r = self.res as f64;
        let i = (((s + 1.0) * 0.5 * r) as usize).min(self.res - 1);
        let j = (((t + 1.0) * 0.5 * r) as usize).min(self.res - 1);
        self.id(f.0 as usize, i, j)
    }
}

fn build_grid(res: usize, threads: usize) -> Grid {
    let n = 6 * res * res;
    let mut dir = vec![[0f32; 3]; n];
    let mut elev = vec![0f32; n];
    let chunk = n.div_ceil(threads);
    std::thread::scope(|s| {
        for (c, (dchunk, echunk)) in dir.chunks_mut(chunk).zip(elev.chunks_mut(chunk)).enumerate() {
            s.spawn(move || {
                for k in 0..dchunk.len() {
                    let id = c * chunk + k;
                    let (f, rest) = (id / (res * res), id % (res * res));
                    let (i, j) = (rest % res, rest / res);
                    let sv = (i as f64 + 0.5) / res as f64 * 2.0 - 1.0;
                    let tv = (j as f64 + 0.5) / res as f64 * 2.0 - 1.0;
                    let d = face_to_direction(&TangentWarp, Face(f as u8), sv, tv);
                    dchunk[k] = [d.x as f32, d.y as f32, d.z as f32];
                    echunk[k] = sample(SEED, d).elevation_m;
                }
            });
        }
    });
    let mut g = Grid { res, dir, elev, cell_area: vec![0.0; n] };
    let areas: Vec<f32> = (0..n as u32)
        .map(|id| {
            let nb = g.nbrs(id);
            let d = |k: usize| if nb[k] == NONE { f64::NAN } else { g.dist(id, nb[k]) };
            let (e, w, nn, so) = (d(0), d(1), d(2), d(3));
            let avg = |a: f64, b: f64| {
                if a.is_nan() {
                    b
                } else if b.is_nan() {
                    a
                } else {
                    0.5 * (a + b)
                }
            };
            (avg(e, w) * avg(nn, so)) as f32
        })
        .collect();
    g.cell_area = areas;
    g
}

fn key(h: f32) -> u32 {
    let b = h.to_bits();
    if b >> 31 == 1 {
        !b
    } else {
        b | 0x8000_0000
    }
}

struct Routing {
    /// Land cells in the order the flood reaches them (receivers before donors), after the ocean cells.
    order: Vec<u32>,
    first_land: usize,
    rec: Vec<u32>,
}

/// Priority flood from the ocean (Barnes): every land cell gets a receiver; depressions are filled by construction.
fn route(g: &Grid, h: &[f32]) -> Routing {
    let n = g.n();
    let mut visited = vec![false; n];
    let mut rec = vec![NONE; n];
    let mut filled = vec![0f32; n];
    let mut order: Vec<u32> = Vec::with_capacity(n);
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
    for id in 0..n as u32 {
        if h[id as usize] <= 0.0 {
            visited[id as usize] = true;
            rec[id as usize] = id;
            filled[id as usize] = h[id as usize];
            order.push(id);
        }
    }
    let first_land = order.len();
    // Ocean cells next to land seed the flood.
    for k in 0..first_land {
        let id = order[k];
        if g.nbrs(id).iter().any(|&m| m != NONE && !visited[m as usize]) {
            heap.push(Reverse((key(filled[id as usize]), id)));
        }
    }
    if heap.is_empty() && first_land < n {
        // No ocean at all: the lowest cell is the outlet.
        let lowest = (0..n).min_by(|&a, &b| h[a].total_cmp(&h[b])).unwrap() as u32;
        visited[lowest as usize] = true;
        rec[lowest as usize] = lowest;
        filled[lowest as usize] = h[lowest as usize];
        heap.push(Reverse((key(h[lowest as usize]), lowest)));
    }
    let mut land_order: Vec<u32> = Vec::with_capacity(n - first_land);
    while let Some(Reverse((_, id))) = heap.pop() {
        if h[id as usize] > 0.0 {
            land_order.push(id);
        }
        for m in g.nbrs(id) {
            if m == NONE || visited[m as usize] {
                continue;
            }
            visited[m as usize] = true;
            rec[m as usize] = id;
            let f = h[m as usize].max(filled[id as usize] + 1e-3);
            filled[m as usize] = f;
            heap.push(Reverse((key(f), m)));
        }
    }
    order.truncate(first_land);
    order.extend(land_order);
    Routing { order, first_land, rec }
}

/// Drainage area in m^2 (every cell's own area plus everything that drains through it).
fn accumulate(g: &Grid, r: &Routing) -> Vec<f32> {
    let mut a = g.cell_area.clone();
    for &id in r.order[r.first_land..].iter().rev() {
        let to = r.rec[id as usize];
        a[to as usize] += a[id as usize];
    }
    a
}

#[derive(Default, Clone, Copy)]
struct Timing {
    sample_s: f64,
    route_s: f64,
    accumulate_s: f64,
    erode_s: f64,
}

/// Implicit stream-power steps (n = 1, Braun and Willett): uplift, then receivers before donors.
fn stream_power(g: &Grid, base: &[f32], steps: usize, uplift_m: f32, k: f64, m: f64, t: &mut Timing) -> (Vec<f32>, Routing, Vec<f32>) {
    let mut h = base.to_vec();
    let mut last: Option<(Routing, Vec<f32>)> = None;
    for _ in 0..steps {
        let t0 = Instant::now();
        let r = route(g, &h);
        t.route_s += t0.elapsed().as_secs_f64();
        let t1 = Instant::now();
        let a = accumulate(g, &r);
        t.accumulate_s += t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        for &id in &r.order[r.first_land..] {
            h[id as usize] += uplift_m;
        }
        for &id in &r.order[r.first_land..] {
            let to = r.rec[id as usize];
            let f = k * (f64::from(a[id as usize]) / 1e6).powf(m) / g.dist(id, to).max(1.0);
            h[id as usize] = ((f64::from(h[id as usize]) + f * f64::from(h[to as usize])) / (1.0 + f)) as f32;
        }
        t.erode_s += t2.elapsed().as_secs_f64();
        last = Some((r, a));
    }
    let (r, a) = last.expect("at least one step");
    (h, r, a)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

struct MapView {
    w: usize,
    h: usize,
    /// Longitude and latitude window in degrees: (lon0, lon1, lat0, lat1).
    window: (f64, f64, f64, f64),
}

fn pixel_dir(v: &MapView, x: usize, y: usize) -> Vec3 {
    let lon = (v.window.0 + (x as f64 + 0.5) / v.w as f64 * (v.window.1 - v.window.0)).to_radians();
    let lat = (v.window.3 - (y as f64 + 0.5) / v.h as f64 * (v.window.3 - v.window.2)).to_radians();
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

/// Hillshaded map of `elev`, optionally with rivers (`area` above `thr_m2`) and face edges marked.
fn render(g: &Grid, v: &MapView, elev: &[f32], area: Option<(&[f32], f64)>, edges: bool, exag: f64) -> RgbaFrame {
    let ids: Vec<u32> = (0..v.w * v.h).map(|k| g.lookup(pixel_dir(v, k % v.w, k / v.w))).collect();
    let deg_per_px = (v.window.1 - v.window.0) / v.w as f64;
    let mut rgba = vec![255u8; v.w * v.h * 4];
    let sun = Vec3::new(-0.6, 0.6, 0.55).normalized();
    for y in 0..v.h {
        for x in 0..v.w {
            let at = |dx: i64, dy: i64| {
                let (xx, yy) = ((x as i64 + dx).clamp(0, v.w as i64 - 1) as usize, (y as i64 + dy).clamp(0, v.h as i64 - 1) as usize);
                f64::from(elev[ids[yy * v.w + xx] as usize])
            };
            let lat = (v.window.3 - (y as f64 + 0.5) / v.h as f64 * (v.window.3 - v.window.2)).to_radians();
            let px_m = deg_per_px.to_radians() * R * libm::cos(lat).max(0.05);
            let py_m = deg_per_px.to_radians() * R;
            let dx = (at(1, 0) - at(-1, 0)) / (2.0 * px_m) * exag;
            let dy = (at(0, -1) - at(0, 1)) / (2.0 * py_m) * exag;
            let nrm = Vec3::new(-dx, -dy, 1.0).normalized();
            let l = (0.35 + 0.65 * nrm.dot(sun).max(0.0)).min(1.0);
            let e = at(0, 0);
            let mut c = if e <= 0.0 {
                [0.10, 0.25 + 0.2 * smoothstep(-6000.0, 0.0, e), 0.45 + 0.25 * smoothstep(-6000.0, 0.0, e)]
            } else {
                let t = smoothstep(0.0, 5000.0, e);
                [0.30 + 0.55 * t, 0.45 + 0.35 * t, 0.22 + 0.60 * t]
            };
            if e > 0.0 {
                for k in &mut c {
                    *k *= l;
                }
            }
            if let Some((a, thr)) = area {
                let id = ids[y * v.w + x] as usize;
                if e > 0.0 && f64::from(a[id]) > thr {
                    let s = smoothstep(0.0, 2.5, libm::log10(f64::from(a[id]) / thr));
                    c = [c[0] * (1.0 - s) + 0.1 * s, c[1] * (1.0 - s) + 0.4 * s, c[2] * (1.0 - s) + 1.0 * s];
                }
            }
            if edges {
                let here = face_of(g, ids[y * v.w + x]);
                let right = if x + 1 < v.w { face_of(g, ids[y * v.w + x + 1]) } else { here };
                let down = if y + 1 < v.h { face_of(g, ids[(y + 1) * v.w + x]) } else { here };
                if right != here || down != here {
                    c = [1.0, 0.0, 1.0];
                }
            }
            let o = (y * v.w + x) * 4;
            for k in 0..3 {
                rgba[o + k] = (c[k] * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
    RgbaFrame { width: v.w as u32, height: v.h as u32, rgba }
}

fn paste(sheet: &mut [u8], sheet_w: usize, img: &RgbaFrame, col: usize, row: usize) {
    let (w, h) = (img.width as usize, img.height as usize);
    for y in 0..h {
        let d = ((row * h + y) * sheet_w + col * w) * 4;
        sheet[d..d + w * 4].copy_from_slice(&img.rgba[y * w * 4..(y + 1) * w * 4]);
    }
}

/// Seam statistics over 4-neighbour land pairs: mean |height change| per metre and mean |change of log10 area|, for pairs
/// inside a face and pairs across a face edge.
fn seam_stats(g: &Grid, h: &[f32], a: &[f32], min_area_m2: f64) -> [(usize, f64, f64); 2] {
    let mut acc = [(0usize, 0.0f64, 0.0f64); 2];
    for id in 0..g.n() as u32 {
        if h[id as usize] <= 0.0 {
            continue;
        }
        for (k, (di, dj)) in OFFSETS.iter().enumerate().take(4) {
            if k % 2 == 1 {
                continue; // each pair once: +i and +j
            }
            let nb = g.step(id, *di, *dj);
            if nb == NONE || h[nb as usize] <= 0.0 {
                continue;
            }
            let cross = usize::from(face_of(g, id) != face_of(g, nb));
            let d = g.dist(id, nb);
            acc[cross].0 += 1;
            acc[cross].1 += f64::from((h[id as usize] - h[nb as usize]).abs()) / d;
            if f64::from(a[id as usize]) > min_area_m2 && f64::from(a[nb as usize]) > min_area_m2 {
                acc[cross].2 += (libm::log10(f64::from(a[id as usize])) - libm::log10(f64::from(a[nb as usize]))).abs();
            }
        }
    }
    acc
}

fn main() {
    let out = Path::new("target").join("review").join("hydrology-spike");
    std::fs::create_dir_all(&out).unwrap();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let (k, m, uplift) = (31.6, 0.5, 20.0f32);
    println!("threads {threads}; stream power K {k} per sqrt(km^2), m {m}, uplift {uplift} m per step, n = 1");
    let mut sheet_grid: Option<(Grid, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>)> = None;
    for (res, steps) in [(256usize, 30usize), (512, 30), (1024, 30), (2048, 2)] {
        let t = Instant::now();
        let g = build_grid(res, threads);
        let mut tm = Timing { sample_s: t.elapsed().as_secs_f64(), ..Timing::default() };
        let cells = g.n();
        let mem_mb = (cells * (12 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4)) as f64 / 1e6;
        // Fallback: one routing and accumulation on the base heights.
        let t0 = Instant::now();
        let r0 = route(&g, &g.elev);
        let a0 = accumulate(&g, &r0);
        let fallback_s = t0.elapsed().as_secs_f64();
        let (h, r, a) = stream_power(&g, &g.elev, steps, uplift, k, m, &mut tm);
        let per_step = (tm.route_s + tm.accumulate_s + tm.erode_s) / steps as f64;
        println!(
            "res {res} ({:.1} km cells, {cells} cells, about {mem_mb:.0} MB of arrays): sampling {:.1} s; fallback routing+accumulation {fallback_s:.2} s; stream power {steps} steps: route {:.2} s, accumulate {:.2} s, erode {:.2} s, per step {per_step:.2} s, 30 steps {:.0} s",
            R * core::f64::consts::FRAC_PI_2 / res as f64 / 1000.0,
            tm.sample_s,
            tm.route_s,
            tm.accumulate_s,
            tm.erode_s,
            per_step * 30.0
        );
        if steps >= 30 {
            let thr = 5.0 * f64::from(g.cell_area.iter().sum::<f32>()) / cells as f64;
            let s_base = seam_stats(&g, &g.elev, &a0, thr);
            let s_gs = seam_stats(&g, &h, &a, thr);
            let line = |name: &str, s: [(usize, f64, f64); 2]| {
                println!(
                    "  seams {name}: inside faces {} pairs, mean |dh|/dist {:.5}, mean |dlog10 A| {:.4}; across edges {} pairs, {:.5}, {:.4}",
                    s[0].0,
                    s[0].1 / s[0].0 as f64,
                    s[0].2 / s[0].0 as f64,
                    s[1].0,
                    s[1].1 / s[1].0.max(1) as f64,
                    s[1].2 / s[1].0.max(1) as f64
                );
            };
            line("fallback (base heights, area from base routing)", s_base);
            line("stream power", s_gs);
            let cross_links =
                (0..cells as u32).filter(|&id| h[id as usize] > 0.0 && face_of(&g, id) != face_of(&g, r.rec[id as usize])).count();
            let land = (0..cells).filter(|&i| h[i] > 0.0).count();
            println!("  receiver links across face edges: {cross_links} of {land} land cells");
        }
        if res == 1024 {
            sheet_grid = Some((g, h, a, a0.clone(), Vec::new()));
        }
    }
    let (g, h, a, a0, _) = sheet_grid.unwrap();
    let cell_m2 = f64::from(g.cell_area.iter().sum::<f32>()) / g.n() as f64;
    let thr = 40.0 * cell_m2;
    // Fallback look: base heights with rivers carved a little along the network.
    let carved: Vec<f32> = (0..g.n())
        .map(|i| {
            let e = g.elev[i];
            if e > 0.0 && f64::from(a0[i]) > thr {
                (e - (60.0 * libm::log10(f64::from(a0[i]) / thr) as f32).min(200.0)).max(0.5)
            } else {
                e
            }
        })
        .collect();
    let world = MapView { w: 1536, h: 768, window: (-180.0, 180.0, -90.0, 90.0) };
    let maps = [
        render(&g, &world, &g.elev, None, false, 12.0),
        render(&g, &world, &h, None, false, 12.0),
        render(&g, &world, &carved, Some((&a0, thr)), false, 12.0),
        render(&g, &world, &h, Some((&a, thr)), false, 12.0),
    ];
    let mut sheet = vec![0u8; 2 * 1536 * 2 * 768 * 4];
    for (k, img) in maps.iter().enumerate() {
        paste(&mut sheet, 2 * 1536, img, k % 2, k / 2);
        img.write_png(&out.join(["1-base.png", "2-stream-power.png", "3-fallback-rivers.png", "4-stream-power-rivers.png"][k])).unwrap();
    }
    RgbaFrame { width: 3072, height: 1536, rgba: sheet }.write_png(&out.join("contact-sheet-world.png")).unwrap();
    // Seam crops: the cube corner (+X, +Y, +Z) and a face edge at the equator, 20 degrees across.
    let crops = [("cube-corner", (35.0, 55.0, 25.3, 45.3)), ("face-edge", (35.0, 55.0, -10.0, 10.0))];
    let mut sheet2 = vec![0u8; 3 * 768 * 2 * 768 * 4];
    for (row, (name, (lon0, lon1, lat0, lat1))) in crops.iter().enumerate() {
        let v = MapView { w: 768, h: 768, window: (*lon0, *lon1, *lat0, *lat1) };
        let imgs = [
            render(&g, &v, &carved, Some((&a0, thr)), false, 12.0),
            render(&g, &v, &h, Some((&a, thr)), false, 12.0),
            render(&g, &v, &h, Some((&a, thr)), true, 12.0),
        ];
        for (col, img) in imgs.iter().enumerate() {
            paste(&mut sheet2, 3 * 768, img, col, row);
            img.write_png(&out.join(format!("crop-{name}-{col}.png"))).unwrap();
        }
    }
    RgbaFrame { width: 3 * 768, height: 2 * 768, rgba: sheet2 }.write_png(&out.join("contact-sheet-seams.png")).unwrap();
    println!("wrote {}", out.display());
}
