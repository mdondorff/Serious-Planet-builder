//! SPIKE-99 relief statistics (spike branch only, never merged): one generator tile (the default class source's base, k as built or 1, with the layer's change from exported tile
//! cores, or with an uplift map added directly) or one GLO-30 tile, on the same 101.8 m grid, scored per relief class:
//! - local relief (p95 - p5) in blocks of 1, 3 and 10 km (blocks at least 90 % one class and 95 % land): median and p90 over the blocks;
//! - the std of the height in the scale bands 0.2-1, 1-3, 3-10 and 10-30 km (the difference of two box means), over land cells of the class;
//! - the slope (central differences): median and p90 in degrees;
//! - the hypsometry: elevation p5, p25, p50, p75 and p95.
//! Land is height above 1 m. Output: one tab-separated line per class and measure (`source class cells measure value`).
//!
//! Generator: `relief_stats -- --label L --tile F,TI,TJ [--export DIR] [--read c1] [--fade-sea M] [--k-one] [--uplift p,r,h,m,hm,pl,b,s] [--stride N] [--seed 14]`
//! GLO-30:    `relief_stats -- --label L --dem FILE --class NAME` (the tile's central part, resampled bilinear to 101.8 m on a local east/north grid)

use planet_core::Face;
use planet_generators::bake::experiment as ex;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::tile::TileGrid;

const R: f64 = 6_371_000.0;
const FACE_CELLS: usize = 98_304;
const TILE: usize = 3072;
const CLASSES: [&str; 8] = ["plain", "rolling", "hills", "mountains", "high-mountains", "plateau", "badlands", "sand-sea"];

fn fail(m: &str) -> ! {
    eprintln!("relief_stats: {m}");
    std::process::exit(2)
}

/// A square-celled height grid with a class per cell (255: no class).
struct Grid {
    nx: usize,
    ny: usize,
    dx: f64,
    h: Vec<f64>,
    class: Vec<u8>,
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let i = ((v.len() - 1) as f64 * p).round() as usize;
    v[i]
}

/// Summed-area table of `f` over the grid.
fn integral(g: &Grid, f: &dyn Fn(usize) -> f64) -> Vec<f64> {
    let (w, h) = (g.nx + 1, g.ny + 1);
    let mut s = vec![0.0; w * h];
    for j in 0..g.ny {
        let mut row = 0.0;
        for i in 0..g.nx {
            row += f(j * g.nx + i);
            s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
        }
    }
    s
}

fn box_sum(s: &[f64], nx: usize, i0: usize, j0: usize, i1: usize, j1: usize) -> f64 {
    let w = nx + 1;
    s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]
}

fn stats(g: &Grid, label: &str) {
    let land = |c: usize| g.h[c] > 1.0;
    let sh = integral(g, &|c| g.h[c]);
    let sl = integral(g, &|c| if land(c) { 1.0 } else { 0.0 });
    let mut out: Vec<(u8, String, f64, usize)> = Vec::new();
    let mut counts = [0usize; 8];
    for c in 0..g.h.len() {
        if land(c) && (g.class[c] as usize) < 8 {
            counts[g.class[c] as usize] += 1;
        }
    }
    let present: Vec<u8> = (0..8u8).filter(|k| counts[*k as usize] * 50 > g.h.len().max(1) / 20).collect();
    // Hypsometry and slope.
    for &k in &present {
        let mut e: Vec<f64> = (0..g.h.len()).filter(|&c| land(c) && g.class[c] == k).map(|c| g.h[c]).collect();
        for (p, name) in [(0.05, "elev_p5"), (0.25, "elev_p25"), (0.5, "elev_p50"), (0.75, "elev_p75"), (0.95, "elev_p95")] {
            out.push((k, name.into(), percentile(&mut e, p), counts[k as usize]));
        }
        let mut s = Vec::new();
        for j in 1..g.ny - 1 {
            for i in 1..g.nx - 1 {
                let c = j * g.nx + i;
                if !land(c) || g.class[c] != k {
                    continue;
                }
                let gx = (g.h[c + 1] - g.h[c - 1]) / (2.0 * g.dx);
                let gy = (g.h[c + g.nx] - g.h[c - g.nx]) / (2.0 * g.dx);
                s.push(libm::atan(libm::sqrt(gx * gx + gy * gy)).to_degrees());
            }
        }
        out.push((k, "slope_p50".into(), percentile(&mut s, 0.5), counts[k as usize]));
        out.push((k, "slope_p90".into(), percentile(&mut s, 0.9), counts[k as usize]));
    }
    // Local relief in blocks.
    for (km, name) in [(1.0, "relief_1km"), (3.0, "relief_3km"), (10.0, "relief_10km")] {
        let n = (km * 1000.0 / g.dx).round() as usize;
        let mut per: Vec<Vec<f64>> = vec![Vec::new(); 8];
        for bj in 0..g.ny / n {
            for bi in 0..g.nx / n {
                let mut hs = Vec::with_capacity(n * n);
                let mut cc = [0usize; 9];
                for j in bj * n..(bj + 1) * n {
                    for i in bi * n..(bi + 1) * n {
                        let c = j * g.nx + i;
                        if land(c) {
                            hs.push(g.h[c]);
                            cc[(g.class[c] as usize).min(8)] += 1;
                        }
                    }
                }
                if hs.len() * 100 < n * n * 95 {
                    continue;
                }
                let (k, m) = cc[..8].iter().enumerate().max_by_key(|x| *x.1).expect("eight");
                if m * 10 < hs.len() * 9 {
                    continue;
                }
                let hi = percentile(&mut hs, 0.95);
                let lo = percentile(&mut hs, 0.05);
                per[k].push(hi - lo);
            }
        }
        for &k in &present {
            let v = &mut per[k as usize];
            let blocks = v.len();
            out.push((k, format!("{name}_p50"), percentile(v, 0.5), blocks));
            out.push((k, format!("{name}_p90"), percentile(v, 0.9), blocks));
        }
    }
    // Scale bands: the std of (box mean at a) - (box mean at b) over land cells whose b box is 95 % land.
    for (a_km, b_km, name) in
        [(0.2, 1.0, "band_0.2-1km"), (1.0, 3.0, "band_1-3km"), (3.0, 10.0, "band_3-10km"), (10.0, 30.0, "band_10-30km")]
    {
        let (na, nb) = (((a_km * 1000.0 / g.dx).round() as usize).max(1), (b_km * 1000.0 / g.dx).round() as usize);
        let mut acc = vec![(0.0f64, 0.0f64, 0usize); 8];
        let half = nb / 2 + 1;
        for j in half..g.ny.saturating_sub(half) {
            for i in half..g.nx.saturating_sub(half) {
                let c = j * g.nx + i;
                if !land(c) || (g.class[c] as usize) >= 8 {
                    continue;
                }
                let bx = |n: usize| (i - n / 2, j - n / 2, i - n / 2 + n, j - n / 2 + n);
                let (i0, j0, i1, j1) = bx(nb);
                let cells = (nb * nb) as f64;
                if box_sum(&sl, g.nx, i0, j0, i1, j1) < 0.95 * cells {
                    continue;
                }
                let mb = box_sum(&sh, g.nx, i0, j0, i1, j1) / cells;
                let (i0, j0, i1, j1) = bx(na);
                let ma = box_sum(&sh, g.nx, i0, j0, i1, j1) / (na * na) as f64;
                let v = ma - mb;
                let e = &mut acc[g.class[c] as usize];
                e.0 += v;
                e.1 += v * v;
                e.2 += 1;
            }
        }
        for &k in &present {
            let (s, s2, n) = acc[k as usize];
            let std = if n > 1 { libm::sqrt((s2 / n as f64 - (s / n as f64).powi(2)).max(0.0)) } else { f64::NAN };
            out.push((k, name.into(), std, n));
        }
    }
    for (k, m, v, n) in out {
        println!("{label}\t{}\t{n}\t{m}\t{v:.2}", CLASSES[k as usize]);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let val = |name: &str| {
        args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().unwrap_or_else(|| fail(&format!("{name} needs a value"))))
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let label = val("--label").unwrap_or_else(|| fail("--label L"));
    let threads = planet_streaming::topology::bake_threads(0).map_or(4, |t| t.0);
    let grid = if let Some(dem) = val("--dem") {
        let class = val("--class").unwrap_or_else(|| fail("--class NAME for a DEM"));
        let k = CLASSES
            .iter()
            .position(|c| *c == class)
            .unwrap_or_else(|| fail("--class: one of plain, rolling, hills, mountains, high-mountains, plateau, badlands, sand-sea"))
            as u8;
        let t = planet_testkit::glo30::read(std::path::Path::new(&dem)).unwrap_or_else(|e| fail(&format!("{dem}: {e:?}")));
        let g = t.geo;
        let lat_c = g.origin_lat - 0.5 * t.height as f64 * g.scale_lat;
        let lon_c = g.origin_lon + 0.5 * t.width as f64 * g.scale_lon;
        let m_lat = R * core::f64::consts::PI / 180.0;
        let m_lon = m_lat * libm::cos(lat_c.to_radians());
        let dx = R * core::f64::consts::FRAC_PI_2 / FACE_CELLS as f64;
        let (w_m, h_m) = (t.width as f64 * g.scale_lon * m_lon * 0.9, t.height as f64 * g.scale_lat * m_lat * 0.9);
        let (nx, ny) = ((w_m / dx) as usize, (h_m / dx) as usize);
        let mut h = vec![0.0; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let (x, y) = ((i as f64 + 0.5) * dx - 0.5 * w_m, 0.5 * h_m - (j as f64 + 0.5) * dx);
                let (lon, lat) = (lon_c + x / m_lon, lat_c + y / m_lat);
                let col = (lon - g.origin_lon) / g.scale_lon - 0.5;
                let row = (g.origin_lat - lat) / g.scale_lat - 0.5;
                let (c0, r0) = (col.floor() as usize, row.floor() as usize);
                let (fc, fr) = (col - c0 as f64, row - r0 as f64);
                let s = |r: usize, c: usize| {
                    let v = f64::from(t.sample(r.min(t.height - 1), c.min(t.width - 1)));
                    if v.is_nan() {
                        0.0
                    } else {
                        v
                    }
                };
                h[j * nx + i] =
                    (1.0 - fr) * ((1.0 - fc) * s(r0, c0) + fc * s(r0, c0 + 1)) + fr * ((1.0 - fc) * s(r0 + 1, c0) + fc * s(r0 + 1, c0 + 1));
            }
        }
        eprintln!("{label}: {dem} centre {lat_c:.3} {lon_c:.3}, {nx} x {ny} cells of {dx:.1} m");
        Grid { nx, ny, dx, h, class: vec![k; nx * ny] }
    } else {
        let tile = val("--tile").unwrap_or_else(|| fail("--tile F,TI,TJ or --dem FILE"));
        let p: Vec<usize> = tile.split(',').map(|s| s.parse().unwrap_or_else(|_| fail("--tile takes integers"))).collect();
        let (face, ti, tj) = (p[0] as u8, p[1], p[2]);
        let seed: u64 = val("--seed").map_or(14, |v| v.parse().unwrap());
        let stride: usize = val("--stride").map_or(1, |v| v.parse().unwrap());
        ex::set_k_one(flag("--k-one"));
    // SPIKE-101 candidates.
    ex::set_hm_band_scale(val("--band-scale").map_or(1.0, |v| v.parse().unwrap()));
    ex::set_hm_ridged(flag("--ridged"));
    ex::set_hm_centre(val("--centre").map_or(0.0, |v| v.parse().unwrap()));
    ex::set_hm_mid_amp(val("--mid-amp").map_or(0.0, |v| v.parse().unwrap()));
    ex::set_hm_parts(val("--parts").map_or(0, |v| v.parse().unwrap()));
        let uplift: Option<[f64; 8]> = val("--uplift").map(|v| {
            let u: Vec<f64> = v.split(',').map(|x| x.parse().unwrap()).collect();
            u.try_into().unwrap_or_else(|_| fail("--uplift takes 8 metres"))
        });
        let core: Option<Vec<f32>> = val("--export").map(|dir| {
            let f = format!("{dir}/tile-{face}-{ti}-{tj}.f32");
            let raw = std::fs::read(&f).unwrap_or_else(|e| fail(&format!("{f}: {e}")));
            raw.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
        });
        let fade: f64 = val("--fade-sea").map_or(0.0, |v| v.parse().unwrap());
        let inputs = BakeInputs::new(seed, threads);
        let tg = TileGrid { cells: FACE_CELLS, tile: TILE, halo: 256, radius_m: R };
        let n = TILE / stride;
        let dx = tg.dx() * stride as f64;
        let mut h = vec![0.0; n * n];
        let mut class = vec![255u8; n * n];
        let rows: Vec<(usize, Vec<f64>, Vec<u8>)> = std::thread::scope(|s| {
            let chunk = n.div_ceil(threads);
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let (inputs, core) = (&inputs, &core);
                    s.spawn(move || {
                        let mut v = Vec::new();
                        for j in t * chunk..((t + 1) * chunk).min(n) {
                            let (mut hr, mut cr) = (Vec::with_capacity(n), Vec::with_capacity(n));
                            for i in 0..n {
                                let (ci, cj) = (i * stride, j * stride);
                                let d = tg.direction(Face(face), (ti * TILE + ci) as i64, (tj * TILE + cj) as i64);
                                let base = inputs.base(d);
                                let w = inputs.weights(d);
                                let mut z = base;
                                if let Some(core) = core {
                                    let ch = f64::from(core[cj * TILE + ci]);
                                    let u = if fade > 0.0 { (base / fade).clamp(0.0, 1.0) } else { 1.0 };
                                    z += ch * if fade > 0.0 { u * u * (3.0 - 2.0 * u) } else { 1.0 };
                                } else if let Some(u) = uplift {
                                    if base > 0.0 {
                                        z += w.iter().zip(u.iter()).map(|(a, b)| a * b).sum::<f64>();
                                    }
                                }
                                hr.push(z);
                                cr.push(w.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(255, |x| x.0 as u8));
                            }
                            v.push((j, hr, cr));
                        }
                        v
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().expect("thread")).collect()
        });
        for (j, hr, cr) in rows {
            h[j * n..(j + 1) * n].copy_from_slice(&hr);
            class[j * n..(j + 1) * n].copy_from_slice(&cr);
        }
        eprintln!(
            "{label}: tile {face}/{ti}/{tj}, {n} x {n} cells of {dx:.1} m, k = 1: {}, layer: {}, uplift added: {}",
            ex::k_one(),
            core.is_some(),
            core.is_none() && uplift.is_some()
        );
        Grid { nx: n, ny: n, dx, h, class }
    };
    stats(&grid, &label);
}
