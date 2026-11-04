//! SPIKE-100 detail statistics (spike branch only, never merged): the production field below 1 km against GLO-30 at 30 m.
//! - `--store DIR`: finds the purest site of each relief class (class lattice of seed 1, the m9r2 world's) on a coarse scan, samples the production field
//!   (`WorldTerrain::open_store(..).field()`, the viewer's field) at 30 m over a 30 km window there, and prints the statistics and each site's lat/lon.
//! - `--dem FILE --class NAME`: the same window at the centre of a GLO-30 tile, bilinear at 30 m on a local east/north grid.
//! Statistics: std of the band between box means of 1-2 cells (30-60 m), 2-4 (60-120 m), 4-8 (120-240 m), 8-32 (240-960 m); local relief p95 - p5 in
//! blocks of 300 m and 3 km (median); slope p50 and p90 at 30 m. Output lines: `label class measure value`, and `site class lat lon` for the store.

use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_generators::classes::{ClassLattice, ClassSource, CLASS_LATTICE_N};
use planet_generators::height::HeightField;
use planet_world_def::WorldDefinition;

const DX: f64 = 30.0;
const N: usize = 1000;
const CLASSES: [&str; 8] = ["plain", "rolling", "hills", "mountains", "high-mountains", "plateau", "badlands", "sand-sea"];

fn fail(m: &str) -> ! {
    eprintln!("detail_stats: {m}");
    std::process::exit(2)
}

fn frame(lat: f64, lon: f64) -> (Vec3, Vec3, Vec3) {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
    (Vec3::new(cla * clo, cla * slo, sla), Vec3::new(-slo, clo, 0.0), Vec3::new(-sla * clo, -sla * slo, cla))
}

fn dir_at(f: &(Vec3, Vec3, Vec3), r: f64, x: f64, y: f64) -> Vec3 {
    (f.0 * r + f.1 * x + f.2 * y).normalized()
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn stats(label: &str, class: &str, h: &[f64]) {
    let w = N + 1;
    let mut s = vec![0.0; w * w];
    for j in 0..N {
        let mut row = 0.0;
        for i in 0..N {
            row += h[j * N + i];
            s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
        }
    }
    let mean = |i: usize, j: usize, n: usize| {
        let (i0, j0) = (i - n / 2, j - n / 2);
        let (i1, j1) = (i0 + n, j0 + n);
        (s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]) / (n * n) as f64
    };
    let mut out = Vec::new();
    for (a, b, name) in [(1, 2, "band_30-60m"), (2, 4, "band_60-120m"), (4, 8, "band_120-240m"), (8, 32, "band_240-960m")] {
        let (mut sum, mut sum2, mut n) = (0.0, 0.0, 0.0);
        for j in (b / 2 + 1..N - b / 2 - 1).step_by(2) {
            for i in (b / 2 + 1..N - b / 2 - 1).step_by(2) {
                let v = if a == 1 { h[j * N + i] } else { mean(i, j, a) } - mean(i, j, b);
                sum += v;
                sum2 += v * v;
                n += 1.0;
            }
        }
        out.push((name.to_string(), libm::sqrt((sum2 / n - (sum / n) * (sum / n)).max(0.0))));
    }
    for (cells, name) in [(10, "relief_300m_p50"), (100, "relief_3km_p50")] {
        let mut r = Vec::new();
        for bj in 0..N / cells {
            for bi in 0..N / cells {
                let mut v: Vec<f64> = (bj * cells..(bj + 1) * cells).flat_map(|j| (bi * cells..(bi + 1) * cells).map(move |i| (i, j))).map(|(i, j)| h[j * N + i]).collect();
                let hi = percentile(&mut v, 0.95);
                r.push(hi - percentile(&mut v, 0.05));
            }
        }
        out.push((name.to_string(), percentile(&mut r, 0.5)));
    }
    let mut sl = Vec::new();
    for j in 1..N - 1 {
        for i in 1..N - 1 {
            let gx = (h[j * N + i + 1] - h[j * N + i - 1]) / (2.0 * DX);
            let gy = (h[(j + 1) * N + i] - h[(j - 1) * N + i]) / (2.0 * DX);
            sl.push(libm::atan(libm::sqrt(gx * gx + gy * gy)).to_degrees());
        }
    }
    out.push(("slope_p50".into(), percentile(&mut sl, 0.5)));
    out.push(("slope_p90".into(), percentile(&mut sl, 0.9)));
    for (m, v) in out {
        println!("{label}\t{class}\t{m}\t{v:.3}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let val = |name: &str| args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().unwrap_or_else(|| fail(&format!("{name} needs a value"))));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    if let Some(dem) = val("--dem") {
        let class = val("--class").unwrap_or_else(|| fail("--class NAME"));
        let label = val("--label").unwrap_or_else(|| "dem".into());
        let t = planet_testkit::glo30::read(std::path::Path::new(&dem)).unwrap_or_else(|e| fail(&format!("{dem}: {e:?}")));
        let g = t.geo;
        let lat_c = g.origin_lat - 0.5 * t.height as f64 * g.scale_lat;
        let lon_c = g.origin_lon + 0.5 * t.width as f64 * g.scale_lon;
        let m_lat = 6_371_000.0 * core::f64::consts::PI / 180.0;
        let m_lon = m_lat * libm::cos(lat_c.to_radians());
        let mut h = vec![0.0; N * N];
        for j in 0..N {
            for i in 0..N {
                let (x, y) = ((i as f64 + 0.5 - N as f64 / 2.0) * DX, (N as f64 / 2.0 - j as f64 - 0.5) * DX);
                let col = (lon_c + x / m_lon - g.origin_lon) / g.scale_lon - 0.5;
                let row = (g.origin_lat - (lat_c + y / m_lat)) / g.scale_lat - 0.5;
                let (c0, r0) = (col.floor() as usize, row.floor() as usize);
                let (fc, fr) = (col - c0 as f64, row - r0 as f64);
                let s = |r: usize, c: usize| {
                    let v = f64::from(t.sample(r.min(t.height - 1), c.min(t.width - 1)));
                    if v.is_nan() { 0.0 } else { v }
                };
                h[j * N + i] = (1.0 - fr) * ((1.0 - fc) * s(r0, c0) + fc * s(r0, c0 + 1)) + fr * ((1.0 - fc) * s(r0 + 1, c0) + fc * s(r0 + 1, c0 + 1));
            }
        }
        stats(&label, &class, &h);
        return;
    }
    let store = val("--store").unwrap_or_else(|| fail("--store DIR or --dem FILE"));
    let world = WorldTerrain::open_store(WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1), std::path::Path::new(&store), 16384)
        .unwrap_or_else(|e| fail(&format!("{store}: {e}")));
    let field = world.field().unwrap_or_else(|e| fail(&e.to_string()));
    let r = world.radius_m();
    eprintln!("field {} ({})", field.implementation_id(), store);
    let source = ClassSource::new(1);
    let lattice = ClassLattice::build_with(&source, CLASS_LATTICE_N, threads);
    // Coarse scan: lat/lon every 1.5 degrees within 60 degrees of the equator; a site's purity is the mean weight of its class over a 5 x 5 grid
    // across the 30 km window, on land (height over 20 m at every point).
    let mut best: Vec<(f64, f64, f64)> = vec![(0.0, 0.0, 0.0); 8];
    let max_lat: f64 = val("--max-lat").map_or(60.0, |v| v.parse().unwrap());
    let mut lat = -max_lat;
    while lat <= max_lat {
        let mut lon = -180.0;
        while lon < 180.0 {
            let f = frame(lat, lon);
            let mut acc = [0.0f64; 8];
            let mut land = true;
            for a in -2..=2 {
                for b in -2..=2 {
                    let d = dir_at(&f, r, f64::from(a) * 7000.0, f64::from(b) * 7000.0);
                    if field.height(d) < 20.0 {
                        land = false;
                    }
                    for (k, w) in lattice.weights_at(d).iter().enumerate() {
                        acc[k] += w / 25.0;
                    }
                }
            }
            if land {
                for k in 0..8 {
                    // Ties (many windows are pure) go to the lower latitude, so the stills' materials are not polar.
                    if acc[k] > best[k].0 + 0.02 || (acc[k] > best[k].0 - 0.02 && libm::fabs(lat) < libm::fabs(best[k].1)) {
                        best[k] = (acc[k], lat, lon);
                    }
                }
            }
            lon += 1.5;
        }
        lat += 1.5;
    }
    // `--site LAT,LON[,NAME]`: one named site instead of the class scan's.
    let named: Option<(f64, f64, String)> = val("--site").map(|v| {
        let p: Vec<&str> = v.split(',').collect();
        (p[0].parse().unwrap(), p[1].parse().unwrap(), p.get(2).map_or("site".to_string(), |s| s.to_string()))
    });
    if let Some((lat, lon, name)) = &named {
        best = vec![(1.0, *lat, *lon); 8];
        let _ = name;
    }
    let want: Vec<usize> = val("--classes").map_or(vec![0, 1, 2, 3, 4, 5], |v| v.split(',').map(|c| CLASSES.iter().position(|n| *n == c).unwrap_or_else(|| fail(c))).collect());
    for k in want {
        let (purity, lat, lon) = best[k];
        if purity < 0.3 {
            eprintln!("{}: no site (best purity {purity:.2})", CLASSES[k]);
            continue;
        }
        println!("site\t{}\t{lat:.3}\t{lon:.3}\tpurity {purity:.2}", CLASSES[k]);
        let f = frame(lat, lon);
        let rows: Vec<Vec<f64>> = std::thread::scope(|s| {
            let chunk = N.div_ceil(threads);
            let hs: Vec<_> = (0..threads)
                .map(|t| {
                    let (f, field) = (&f, &field);
                    s.spawn(move || {
                        (t * chunk..((t + 1) * chunk).min(N))
                            .map(|j| {
                                (0..N)
                                    .map(|i| {
                                        let (x, y) = ((i as f64 + 0.5 - N as f64 / 2.0) * DX, (N as f64 / 2.0 - j as f64 - 0.5) * DX);
                                        field.height(dir_at(f, r, x, y))
                                    })
                                    .collect::<Vec<f64>>()
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().expect("thread")).collect()
        });
        let h: Vec<f64> = rows.into_iter().flatten().collect();
        stats(named.as_ref().map_or("runtime", |n| n.2.as_str()), CLASSES[k], &h);
    }
}
