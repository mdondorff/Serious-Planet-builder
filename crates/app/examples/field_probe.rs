//! Diagnosis probe (inbox #49, not committed): height, slope and curvature crops of the default field around a site, the bake's base alone and the
//! detail, and profile lines across the bake's lattice cells. CPU only.
//! `cargo run --release -p planet --example field_probe -- --store DIR --out DIR`

use std::path::PathBuf;

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::Vec3;
use planet_testkit::{write_png, Image};
use planet_world_def::WorldDefinition;

const N: usize = 401;
const STEP_M: f64 = 10.0;

fn frame(lat: f64, lon: f64) -> (Vec3, Vec3, Vec3) {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
    (Vec3::new(cla * clo, cla * slo, sla), Vec3::new(-slo, clo, 0.0), Vec3::new(-sla * clo, -sla * slo, cla))
}

fn dir_at(f: &(Vec3, Vec3, Vec3), r: f64, x: f64, y: f64) -> Vec3 {
    let p = f.0 * r + f.1 * x + f.2 * y;
    p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z))
}

fn grey(v: f64) -> [u8; 4] {
    let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
    [g, g, g, 255]
}

fn write_map(path: PathBuf, vals: &[f64], lo: f64, hi: f64) {
    let mut rgba = Vec::with_capacity(vals.len() * 4);
    for v in vals {
        rgba.extend_from_slice(&grey((v - lo) / (hi - lo).max(1e-12)));
    }
    write_png(&path, &Image::new(N as u32, N as u32, rgba)).expect("png");
}

fn percentile(v: &[f64], q: f64) -> f64 {
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() as f64 - 1.0) * q) as usize]
}

/// Slope (degrees) and Laplacian curvature (1/m) of a row-major N x N raster (row 0 north), borders zero.
fn derive(h: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut slope = vec![0.0; N * N];
    let mut curv = vec![0.0; N * N];
    for r in 1..N - 1 {
        for c in 1..N - 1 {
            let i = r * N + c;
            let gx = (h[i + 1] - h[i - 1]) / (2.0 * STEP_M);
            let gy = (h[i - N] - h[i + N]) / (2.0 * STEP_M);
            slope[i] = libm::atan(libm::sqrt(gx * gx + gy * gy)).to_degrees();
            curv[i] = (h[i + 1] + h[i - 1] + h[i + N] + h[i - N] - 4.0 * h[i]) / (STEP_M * STEP_M);
        }
    }
    (slope, curv)
}

fn rms(v: &[f64]) -> f64 {
    let f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    libm::sqrt(f.iter().map(|x| x * x).sum::<f64>() / f.len().max(1) as f64)
}

fn plot(path: PathBuf, series: &[(&[f64], [u8; 4])], vlines: &[usize], len: usize) {
    let (w, h) = (len.max(2) as u32 * 2, 400u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for &v in vlines {
        let x = (v * 2).min(w as usize - 1);
        for y in 0..h as usize {
            let o = (y * w as usize + x) * 4;
            rgba[o..o + 4].copy_from_slice(&[200, 200, 200, 255]);
        }
    }
    for (vals, col) in series {
        let lo = percentile(vals, 0.0);
        let hi = percentile(vals, 1.0);
        let mut prev: Option<(i64, i64)> = None;
        for (i, v) in vals.iter().enumerate() {
            if !v.is_finite() {
                continue;
            }
            let (x, y) = ((i * 2) as i64, ((1.0 - (v - lo) / (hi - lo).max(1e-12)) * (h as f64 - 8.0) + 4.0) as i64);
            if let Some((px, py)) = prev {
                let steps = (y - py).abs().max(1);
                for s in 0..=steps {
                    let yy = py + (y - py) * s / steps;
                    let xx = px + (x - px) * s / steps;
                    if xx >= 0 && xx < w as i64 && yy >= 0 && yy < h as i64 {
                        let o = (yy as usize * w as usize + xx as usize) * 4;
                        rgba[o..o + 4].copy_from_slice(col);
                    }
                }
            }
            prev = Some((x, y));
        }
    }
    write_png(&path, &Image::new(w, h, rgba)).expect("plot");
}

fn run() -> Result<(), String> {
    let mut store = PathBuf::from("C:/DEV/SeriousPlanetData/global16384-m9r2");
    let mut out = PathBuf::from("target/e2b");
    let mut v2 = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--store" => store = it.next().ok_or("--store")?.into(),
            "--out" => out = it.next().ok_or("--out")?.into(),
            "--v2" => v2 = true,
            o => return Err(format!("unknown {o}")),
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
    if v2 {
        def.set_height_field("cpu-erosion-v2");
    }
    let world = WorldTerrain::open_store(def, &store, 16384)?;
    let field = world.field()?;
    let r = world.radius_m();
    let base_at = |d: Vec3| {
        if v2 {
            world.base_elevation_c1(d).unwrap_or(f64::NAN)
        } else {
            world.bake_probe(d).map_or(f64::NAN, |p| f64::from(p.elevation_m))
        }
    };

    // The steep belt flank: the steepest base slope over a 100 km window at 1 km spacing around belt-east.
    let belt = (18.97, 155.34);
    let fb = frame(belt.0, belt.1);
    let mut best = (0.0f64, 0.0f64, 0.0f64);
    let m = 50i32;
    for ix in -m..=m {
        for iy in -m..=m {
            let (x, y) = (f64::from(ix) * 1000.0, f64::from(iy) * 1000.0);
            let (e0, e1, e2) =
                (base_at(dir_at(&fb, r, x, y)), base_at(dir_at(&fb, r, x + 600.0, y)), base_at(dir_at(&fb, r, x, y + 600.0)));
            let s = libm::sqrt((e1 - e0) * (e1 - e0) + (e2 - e0) * (e2 - e0)) / 600.0;
            if s.is_finite() && s > best.0 {
                best = (s, x, y);
            }
        }
    }
    let (lat_f, lon_f) = {
        let d = dir_at(&fb, r, best.1, best.2);
        (libm::asin(d.z).to_degrees(), libm::atan2(d.y, d.x).to_degrees())
    };
    eprintln!(
        "steepest belt flank: base slope {:.3} ({:.1} deg) at {:.4} N {:.4} E",
        best.0,
        libm::atan(best.0).to_degrees(),
        lat_f,
        lon_f
    );

    let mut report = String::new();
    for (name, lat, lon) in [("belt-east", belt.0, belt.1), ("belt-flank", lat_f, lon_f)] {
        let f = frame(lat, lon);
        let half = (N as f64 - 1.0) / 2.0 * STEP_M;
        let mut dirs = Vec::with_capacity(N * N);
        for row in 0..N {
            for col in 0..N {
                dirs.push(dir_at(&f, r, -half + col as f64 * STEP_M, half - row as f64 * STEP_M));
            }
        }
        let h = field.heights_threaded(&dirs, &threads);
        let e: Vec<f64> = dirs.iter().map(|d| base_at(*d)).collect();
        let d: Vec<f64> = h.iter().zip(&e).map(|(a, b)| a - b).collect();
        let dir = out.join(name);
        std::fs::create_dir_all(&dir).map_err(|x| x.to_string())?;
        report += &format!("## {name} ({lat:.4} N, {lon:.4} E), 4 x 4 km at 10 m\n");
        for (label, v) in [("height", &h), ("base", &e), ("detail", &d)] {
            let (slope, curv) = derive(v);
            let (lo, hi) = (percentile(v, 0.01), percentile(v, 0.99));
            write_map(dir.join(format!("{label}-height.png")), v, lo, hi);
            let s_hi = percentile(&slope, 0.99).max(1e-3);
            write_map(dir.join(format!("{label}-slope.png")), &slope, 0.0, s_hi);
            let c_hi = percentile(&curv.iter().map(|x| x.abs()).collect::<Vec<_>>(), 0.98).max(1e-9);
            write_map(dir.join(format!("{label}-curvature.png")), &curv, -c_hi, c_hi);
            report += &format!(
                "- {label}: range {:.1} to {:.1} m, slope p50 {:.2} p99 {:.2} deg, curvature rms {:.2e} 1/m\n",
                percentile(v, 0.0),
                percentile(v, 1.0),
                percentile(&slope, 0.5),
                percentile(&slope, 0.99),
                rms(&curv)
            );
        }
        // Profiles: three lines through the centre across about 5 lattice cells (611 m), sampled every 10 m.
        let len = 320usize;
        for (pname, (ux, uy)) in
            [("east-west", (1.0, 0.0)), ("north-south", (0.0, 1.0)), ("diagonal", (std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2))]
        {
            let pd: Vec<Vec3> = (0..len)
                .map(|i| {
                    let s = (i as f64 - len as f64 / 2.0) * STEP_M * 10.0 / 10.0;
                    dir_at(&f, r, s * ux, s * uy)
                })
                .collect();
            let ph = field.heights_threaded(&pd, &threads);
            let pe: Vec<f64> = pd.iter().map(|d| base_at(*d)).collect();
            let pdt: Vec<f64> = ph.iter().zip(&pe).map(|(a, b)| a - b).collect();
            let cells: Vec<(u32, u32)> = pd
                .iter()
                .map(|d| planet_generators::global::locate_direction(16384, &TangentWarp, *d).map_or((0, 0), |p| (p.i, p.j)))
                .collect();
            let edges: Vec<usize> = (1..len).filter(|&i| cells[i] != cells[i - 1]).collect();
            let slope = |v: &[f64]| -> Vec<f64> {
                (0..len).map(|i| if i == 0 || i + 1 == len { f64::NAN } else { (v[i + 1] - v[i - 1]) / (2.0 * STEP_M) }).collect()
            };
            let second = |v: &[f64]| -> Vec<f64> {
                (0..len)
                    .map(|i| if i == 0 || i + 1 == len { f64::NAN } else { (v[i + 1] - 2.0 * v[i] + v[i - 1]) / (STEP_M * STEP_M) })
                    .collect()
            };
            let (sb, s2b, sd, s2d) = (slope(&pe), second(&pe), slope(&pdt), second(&pdt));
            // Kink size: the jump of the base slope across each lattice edge (the mean of the two samples after minus before), against the
            // detail's own slope change over the same samples.
            let mut jumps = Vec::new();
            let mut djumps = Vec::new();
            for &ed in &edges {
                if ed >= 3 && ed + 3 < len {
                    jumps.push(((sb[ed + 2] + sb[ed + 3]) - (sb[ed - 3] + sb[ed - 2])) / 2.0);
                    djumps.push(((sd[ed + 2] + sd[ed + 3]) - (sd[ed - 3] + sd[ed - 2])) / 2.0);
                }
            }
            let med = |v: &[f64]| -> f64 {
                let a: Vec<f64> = v.iter().map(|x| x.abs()).collect();
                if a.is_empty() {
                    f64::NAN
                } else {
                    percentile(&a, 0.5)
                }
            };
            report += &format!(
                "- profile {pname}: {} lattice edges crossed; base slope jump across an edge: median |{:.4}| (max {:.4}); detail slope change over the same samples: median |{:.4}|; second difference rms: base {:.2e}, detail {:.2e}, full {:.2e}\n",
                edges.len(),
                med(&jumps),
                jumps.iter().fold(0.0f64, |a, b| a.max(b.abs())),
                med(&djumps),
                rms(&s2b),
                rms(&s2d),
                rms(&second(&ph))
            );
            plot(dir.join(format!("profile-{pname}-slope.png")), &[(&sb, [200, 0, 0, 255]), (&sd, [0, 0, 200, 255])], &edges, len);
            plot(dir.join(format!("profile-{pname}-second.png")), &[(&s2b, [200, 0, 0, 255]), (&s2d, [0, 0, 200, 255])], &edges, len);
        }
    }
    std::fs::write(out.join("report.txt"), &report).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("field_probe: {e}");
        std::process::exit(1);
    }
}
