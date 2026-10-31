//! The terrain scorecard run (TEST-079, TEST-080; `cargo xtask terrain-score`): per landform of the sites file the seven descriptors on the real GLO-30 tile's
//! central 0.5 degree square and on the generator's two matched sites (`world.field()` of the production store, read only), the distances between them, and
//! `report.md` / `report.json`. CPU only. A correctness run, not a measurement.
//! `cargo run --release -p planet --example terrain_score -- [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

#[path = "s74/kernels.rs"]
mod k74;

use planet::terrainscore::{self as ts, LandformResult, LandformSpec, Notice, RasterStats, RunInfo, TileView};
use planet::worldfield::WorldTerrain;
use planet_core::par::Par;
use planet_testkit::glo30;
use planet_world_def::WorldDefinition;

const DEFAULT_STORE: &str = "C:/DEV/SeriousPlanetData/global16384";
const DEFAULT_DEM: &str = "C:/DEV/SeriousPlanetData/dem/glo30-samples";
const DEFAULT_SITES: &str = "crates/app/data/terrain-score-sites.toml";
/// The production store's cells per face edge.
const STORE_RES: u32 = 16384;
/// Rasters analysed at once: each holds a few hundred MB of kernel output while it runs.
const RASTER_WORKERS: usize = 6;

struct Config {
    store: PathBuf,
    dem: PathBuf,
    sites: PathBuf,
    out: Option<PathBuf>,
    maps: bool,
}

fn parse_args() -> Result<Config, String> {
    let mut c = Config { store: DEFAULT_STORE.into(), dem: DEFAULT_DEM.into(), sites: DEFAULT_SITES.into(), out: None, maps: false };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--store" => c.store = value("--store")?.into(),
            "--dem" => c.dem = value("--dem")?.into(),
            "--sites" => c.sites = value("--sites")?.into(),
            "--out" => c.out = Some(value("--out")?.into()),
            "--maps" => c.maps = true,
            other => return Err(format!("unknown option '{other}' (known: --store DIR, --dem DIR, --sites FILE, --out DIR, --maps)")),
        }
    }
    Ok(c)
}

fn secs_since_epoch() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or("unknown".to_string(), |o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Runs `f(i)` for `i in 0..n` on up to `workers` threads and returns the results in index order (each index writes its own slot: CON-14).
fn map_par<T: Send>(workers: usize, n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let slots: Vec<Mutex<Option<T>>> = (0..n).map(|_| Mutex::new(None)).collect();
    workers.run(n, &|i| {
        let v = f(i);
        *slots[i].lock().expect("a slot") = Some(v);
    });
    slots.into_iter().map(|s| s.into_inner().expect("a slot").expect("every index ran")).collect()
}

fn read_real(dem: &Path, tile: &str) -> Result<Vec<f64>, String> {
    let file = ts::tile_file_name(tile).ok_or_else(|| format!("tile '{tile}' is not a name like N52E012"))?;
    let t = glo30::read(&dem.join(&file)).map_err(|e| format!("{}: {e}", dem.join(&file).display()))?;
    let view = TileView {
        data: &t.data,
        width: t.width,
        height: t.height,
        origin_lon: t.geo.origin_lon,
        origin_lat: t.geo.origin_lat,
        scale_lon: t.geo.scale_lon,
        scale_lat: t.geo.scale_lat,
    };
    Ok(ts::resample_real(&view))
}

fn write_maps(dir: &Path, landform: &LandformSpec, label: &str, maps: Vec<ts::MapImage>) -> Result<(), String> {
    let dir = dir.join("maps").join(&landform.tile);
    for m in maps {
        let img = planet_testkit::Image::new(ts::MAP_PX as u32, ts::MAP_PX as u32, m.rgba);
        planet_testkit::write_png(&dir.join(format!("{label}-{}.png", m.descriptor)), &img)
            .map_err(|e| format!("writing a map under {}: {e}", dir.display()))?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let c = parse_args()?;
    let total = Instant::now();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let date = ts::utc_date(secs_since_epoch());
    let out_dir = c.out.clone().unwrap_or_else(|| PathBuf::from(format!("target/review/terrain-score/{date}")));
    let specs = ts::parse_sites(&std::fs::read_to_string(&c.sites).map_err(|e| format!("--sites {}: {e}", c.sites.display()))?)?;
    if !c.dem.is_dir() {
        return Err(format!("tile directory {} does not exist (--dem DIR)", c.dem.display()));
    }
    let mut phases: Vec<(String, f64)> = Vec::new();
    let mut lap = |name: &str, t: Instant| {
        let s = t.elapsed().as_secs_f64();
        eprintln!("terrain-score: {name}: {s:.1} s");
        phases.push((name.to_string(), s));
    };

    // Phase 1: the real rasters.
    let t = Instant::now();
    let reals: Vec<Result<Vec<f64>, String>> = map_par(threads.min(specs.len()), specs.len(), |i| read_real(&c.dem, &specs[i].tile));
    let reals = reals.into_iter().collect::<Result<Vec<_>, _>>()?;
    lap("read tiles", t);

    // Phase 2: the generator windows, from the store read only.
    let t = Instant::now();
    let def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
    let definition_hash = format!("{:016x}", def.hash());
    let world =
        WorldTerrain::open_store(def, &c.store, STORE_RES).map_err(|e| format!("store {} (--store DIR): {e}", c.store.display()))?;
    let field = world.field()?;
    let radius = world.radius_m();
    let mut gens: Vec<Vec<Vec<f64>>> = Vec::new();
    for s in &specs {
        let mut windows = Vec::new();
        for &(lat, lon) in &s.sites {
            windows.push(ts::sample_window(&*field, lat, lon, radius, threads));
        }
        gens.push(windows);
    }
    let field_s74 = field;
    lap("sample generator windows", t);

    // SPIKE-73 (spike code, never merged): `S73_DIR=DIR` scores variants of the plains rasters (Brandenburg, Loess, Erg Chebbi) and returns.
    // It writes each raster as f32 (`<tile>-real.f32`, `<tile>-site<k>.f32`, 1853^2) for offline variants, and scores any `<tile>-site<k>-<tag>.f32`
    // found there. Printed per raster: flat share, the three largest classes, slope p50, relief 1 km p50, and against the real tile the class L1 and
    // the relief 1 km distance.
    if let Ok(dir) = std::env::var("S73_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = ts::GRID;
        let write = |name: &str, z: &[f64]| {
            let b: Vec<u8> = z.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
            std::fs::write(dir.join(name), b).expect("write a raster");
        };
        let read = |path: &Path| -> Vec<f64> {
            let b = std::fs::read(path).expect("read a raster");
            b.chunks_exact(4).map(|c| f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect()
        };
        // A box mean of side 2r+1 by summed-area table (clamped at the edge).
        let box_mean = |z: &[f64], r: usize| -> Vec<f64> {
            let w = g + 1;
            let mut s = vec![0f64; w * w];
            for j in 0..g {
                let mut row = 0.0;
                for i in 0..g {
                    row += z[j * g + i];
                    s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
                }
            }
            let mut out = vec![0f64; g * g];
            for j in 0..g {
                for i in 0..g {
                    let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(g), j.saturating_sub(r), (j + r + 1).min(g));
                    let t = s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0];
                    out[j * g + i] = t / ((i1 - i0) * (j1 - j0)) as f64;
                }
            }
            out
        };
        // White noise of standard deviation `sd` from an integer hash of the cell (a stand-in for G1's sub-30 m octaves sampled at 30 m).
        let noise = |z: &[f64], sd: f64, salt: u64| -> Vec<f64> {
            z.iter()
                .enumerate()
                .map(|(k, v)| {
                    let mut x = (k as u64 ^ salt).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    x ^= x >> 29;
                    let u = (x >> 11) as f64 / (1u64 << 53) as f64;
                    let mut y = x.wrapping_mul(0x94D0_49BB_1331_11EB);
                    y ^= y >> 32;
                    let u2 = ((y >> 11) as f64 / (1u64 << 53) as f64).max(1e-12);
                    v + sd * (-2.0 * u2.ln()).sqrt() * (2.0 * std::f64::consts::PI * u).cos()
                })
                .collect()
        };
        let line = |label: &str, s: &RasterStats, real: Option<&RasterStats>| {
            let mut top: Vec<(usize, f64)> = (0..10).map(|k| (k, s.forms[k])).collect();
            top.sort_by(|a, b| b.1.total_cmp(&a.1));
            let tops: Vec<String> = top.iter().take(3).map(|(k, v)| format!("{} {:.0}%", ts::FORM_NAMES[*k], 100.0 * v)).collect();
            let d = real.map(|r| ts::distances(r, s));
            println!(
                "S73 {label:40} flat {:5.1}% | {} | slope p50 {:.2} | relief1 p50 {:.1} | vs real: classes L1 {} relief1 {}",
                100.0 * s.forms[0],
                tops.join(", "),
                s.series[0].p(50),
                s.series[3].p(50),
                d.as_ref().map_or("-".into(), |d| format!("{:.2}", d.forms_l1)),
                d.as_ref().and_then(|d| d.continuous[3]).map_or("-".into(), |x| format!("{x:.2}")),
            );
        };
        let threads_k = threads;
        for (i, s) in specs.iter().enumerate() {
            if !["N52E012", "N41W096", "N31W004"].contains(&s.tile.as_str()) {
                continue;
            }
            let real = &reals[i];
            write(&format!("{}-real.f32", s.tile), real);
            let (rs, _) = ts::analyse(real, g, g, threads_k, false);
            line(&format!("{} real", s.tile), &rs, None);
            // The real tile smoothed: does surface clutter (GLO-30 is a surface model) make the real plains non-flat?
            for r in [1usize, 2] {
                let (st, _) = ts::analyse(&box_mean(real, r), g, g, threads_k, false);
                line(&format!("{} real box {}x{}", s.tile, 2 * r + 1, 2 * r + 1), &st, Some(&rs));
            }
            for (k, z) in gens[i].iter().enumerate() {
                write(&format!("{}-site{}.f32", s.tile, k + 1), z);
                let (st, _) = ts::analyse(z, g, g, threads_k, false);
                line(&format!("{} site{} base", s.tile, k + 1), &st, Some(&rs));
                // A plains relief floor in k: m + k (h - m), m the 10 km box mean.
                let m = box_mean(z, 167);
                for kf in [1.5f64, 2.0, 2.5, 3.0] {
                    let zk: Vec<f64> = z.iter().zip(&m).map(|(h, mm)| if *h > 0.0 { mm + kf * (h - mm) } else { *h }).collect();
                    let (st, _) = ts::analyse(&zk, g, g, threads_k, false);
                    line(&format!("{} site{} k {kf}", s.tile, k + 1), &st, Some(&rs));
                }
                // G1-like sub-30 m detail sampled at 30 m: white noise.
                for sd in [0.1f64, 0.3, 1.0] {
                    let (st, _) = ts::analyse(&noise(z, sd, 7 + k as u64), g, g, threads_k, false);
                    line(&format!("{} site{} noise {sd} m", s.tile, k + 1), &st, Some(&rs));
                }
                // Offline variants (the local carve).
                let prefix = format!("{}-site{}-", s.tile, k + 1);
                let mut extra: Vec<PathBuf> = std::fs::read_dir(&dir)
                    .map_err(|e| e.to_string())?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".f32")))
                    .collect();
                extra.sort();
                for p in extra {
                    let (st, _) = ts::analyse(&read(&p), g, g, threads_k, false);
                    let tag = p.file_stem().and_then(|n| n.to_str()).unwrap_or("?").to_string();
                    line(&tag, &st, Some(&rs));
                }
            }
        }
        return Ok(());
    }

    // SPIKE-77 (spike code, never merged; from SPIKE-76): `S77_DIR=DIR` times a whole-planet preview, then runs the arid sites with k under 1 and a gentler carve. SPIKE-76: runs the pipeline with an assigned relief class per landform (k and crease floor per class, the stand-in for an authored class map) next to SPIKE-74's `ref`. SPIKE-75: logs the pipeline's inputs at every scorecard site and runs SPIKE-74's pipeline with variants:
    // `ref` (SPIKE-74 as run), `arid` (k held at 1 or below where the generator is dry, the crease floor only where k is about 1), `arid+top8`
    // (and the top knot 8), `sand` (as arid+top8, k 0.4 where very dry). Same tables as SPIKE-74, plus per site: L, k, base elevation, precipitation,
    // temperature, zone, inland, ridge at the centre and k p10/p50/p90 over the window.
    if let Ok(dir) = std::env::var("S77_DIR") {
        use planet_generators::height::HeightField as _;
        use std::sync::atomic::Ordering::Relaxed;
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = ts::GRID;
        let seed = field_s74.seed();
        let lattice = planet_generators::global::landform_lattice(seed);
        let smooth = |x: f64, a: f64, b: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        #[derive(Clone, Copy)]
        struct V {
            name: &'static str,
            arid: bool,
            top: f64,
            sand: bool,
        }
        // Variants for the arid sites: `top` is the class k, `sand` halves the carve's K dt.
        let variants = [
            V { name: "k1", arid: true, top: 1.0, sand: false },
            V { name: "k0.6", arid: true, top: 0.6, sand: false },
            V { name: "k1-carve50", arid: true, top: 1.0, sand: true },
            V { name: "k0.6-carve50", arid: true, top: 0.6, sand: true },
            V { name: "k0.35-carve50", arid: true, top: 0.35, sand: true },
        ];
        // The preview: the landform lattice (a fresh seed, not the memo) and the global-coarse field on cube grids of about 16 and 4 km, 8 threads.
        {
            let t0 = Instant::now();
            let lat2 = planet_generators::global::landform_lattice(seed ^ 0x77);
            println!("S77PREVIEW landform lattice (257^2 x 6) built in {:.2} s", t0.elapsed().as_secs_f64());
            drop(lat2);
            for n in [625usize, 2500] {
                let t0 = Instant::now();
                let rows: Vec<usize> = (0..6 * n).collect();
                let next = std::sync::atomic::AtomicUsize::new(0);
                let sum = std::sync::Mutex::new(0f64);
                std::thread::scope(|sc| {
                    for _ in 0..8 {
                        sc.spawn(|| {
                            let mut acc = 0f64;
                            loop {
                                let r = next.fetch_add(1, Relaxed);
                                if r >= rows.len() {
                                    break;
                                }
                                let (f, j) = (r / n, r % n);
                                for i in 0..n {
                                    let (s1, t1) = (-1.0 + (2.0 * i as f64 + 1.0) / n as f64, -1.0 + (2.0 * j as f64 + 1.0) / n as f64);
                                    let d = planet_core::cube::face_to_direction(&planet_core::cube::TangentWarp, planet_core::cube::Face(f as u8), s1, t1);
                                    let gs = planet_generators::global::sample(seed, d);
                                    acc += f64::from(gs.elevation_m) + lattice.at(d);
                                }
                            }
                            *sum.lock().unwrap() += acc;
                        });
                    }
                });
                println!("S77PREVIEW global-coarse sample and landform at {} points ({}^2 per face, about {:.0} km): {:.1} s", 6 * n * n, n, 10_000.0 / n as f64, t0.elapsed().as_secs_f64());
            }
        }
        // The assigned class per scorecard tile: (class name, k, crease floor).
        let class_of = |tile: &str| -> (&'static str, f64, f64) {
            match tile {
                "N52E012" => ("plain", 1.0, 0.2),
                "N41W096" => ("rolling plain", 1.0, 0.35),
                "N43W102" => ("badlands (arid rugged)", 1.0, 0.0),
                "N31W004" => ("sand sea (arid flat)", 0.35, 0.0),
                "N47E011" | "N27E086" => ("high mountains", 7.5, 0.0),
                _ => ("hills", 5.0, 0.0),
            }
        };
        // k of L (SPIKE-69's knots, the top knot `top`), the coast taper on `e`, and the aridity gate on precipitation `p` (mm): from 1 under 250 mm to
        // full over 500 mm; with `sand`, under 150 mm k falls toward 0.4.
        let class_k = std::cell::Cell::new(1.0f64);
        let k_of = |v: V, l: f64, e: f64, p: f64| {
            if v.arid {
                return 1.0 + smooth(e, 0.0, 400.0) * (class_k.get() - 1.0);
            }
            let (xs, ys) = ([0.0, 0.028, 0.176, 1.0], [1.0, 4.5, 4.5, v.top]);
            let mut kk = ys[3];
            for j in 0..3 {
                if l <= xs[j + 1] {
                    kk = ys[j] + (ys[j + 1] - ys[j]) * (l - xs[j]) / (xs[j + 1] - xs[j]);
                    break;
                }
            }
            let mut k = 1.0 + smooth(e, 0.0, 400.0) * (kk - 1.0);
            if v.arid {
                k = 1.0 + (k - 1.0) * smooth(p, 250.0, 500.0);
            }
            if v.sand {
                k -= 0.6 * (1.0 - smooth(p, 80.0, 150.0)) * smooth(e, 0.0, 400.0);
            }
            k
        };
        let box_mean = |z: &[f64], n: usize, r: usize| -> Vec<f64> {
            let w = n + 1;
            let mut s = vec![0f64; w * w];
            for j in 0..n {
                let mut row = 0.0;
                for i in 0..n {
                    row += z[j * n + i];
                    s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
                }
            }
            let mut out = vec![0f64; n * n];
            for j in 0..n {
                for i in 0..n {
                    let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(n), j.saturating_sub(r), (j + r + 1).min(n));
                    out[j * n + i] = (s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]) / ((i1 - i0) * (j1 - j0)) as f64;
                }
            }
            out
        };
        let frame = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
            (
                planet_core::Vec3::new(cla * clo, cla * slo, sla),
                planet_core::Vec3::new(-slo, clo, 0.0),
                planet_core::Vec3::new(-sla * clo, -sla * slo, cla),
            )
        };
        const NC: usize = 491;
        const CC: f64 = 814.0;
        let report = |label: &str, s: &RasterStats, real: &RasterStats| -> String {
            let d = ts::distances(real, s);
            let cs: Vec<String> = ts::CONTINUOUS
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{}={:.3}/{}", c.id, s.series[k].p(50), d.continuous[k].map_or("-".into(), |x| format!("{x:.2}"))))
                .collect();
            format!(
                "S77 {label} | flat {:.1}% | classes L1 {:.2} | density ratio {} | {}",
                100.0 * s.forms[0],
                d.forms_l1,
                d.density_ratio.map_or("-".into(), |x| format!("{x:.2}")),
                cs.join(" ")
            )
        };
        let half_m = ts::HALF_DEG.to_radians() * radius;
        for (i, s) in specs.iter().enumerate() {
            if !["N43W102", "N31W004"].contains(&s.tile.as_str()) {
                continue;
            }
            let (rs, _) = ts::analyse(&reals[i], g, g, threads, false);
            let cs: Vec<String> = ts::CONTINUOUS.iter().enumerate().map(|(k, c)| format!("{}={:.3}", c.id, rs.series[k].p(50))).collect();
            println!("S77 {} real | flat {:.1}% | {}", s.tile, 100.0 * rs.forms[0], cs.join(" "));
            for (k, &(lat, lon)) in s.sites.iter().enumerate() {
                let z0 = &gens[i][k];
                let (bs, _) = ts::analyse(z0, g, g, threads, false);
                println!("{}", report(&format!("{} site{} baseline", s.tile, k + 1), &bs, &rs));
                let dirs = ts::generator_directions(lat, lon, radius);
                planet_generators::erosion::spike73_set_floor(0.35);
                let z1 = field_s74.heights_threaded(&dirs, &threads);
                // The coarse window: point samples at 814 m (floor on, as SPIKE-74), and per coarse cell L, base elevation and precipitation.
                let (c, east, north) = frame(lat, lon);
                let mut dc = Vec::with_capacity(NC * NC);
                for row in 0..NC {
                    for col in 0..NC {
                        let (x, y) = ((col as f64 - (NC / 2) as f64) * CC, ((NC / 2) as f64 - row as f64) * CC);
                        let p = c * radius + east * x + north * y;
                        dc.push(p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z)));
                    }
                }
                let zc = field_s74.heights_threaded(&dc, &threads);
                planet_generators::erosion::spike73_set_floor(0.0);
                let pc: Vec<f64> = dc.iter().map(|d| f64::from(planet_generators::global::sample(seed, *d).precipitation_mm)).collect();
                let lc: Vec<f64> = dc.iter().map(|d| lattice.at(*d)).collect();
                // Site log at the centre.
                {
                    let d0 = planet_core::Vec3::new(c.x, c.y, c.z);
                    let gs = planet_generators::global::sample(seed, d0);
                    let parts = planet_generators::global::spike69_parts(seed, d0);
                    let l0 = lattice.at(d0);
                    let kref = k_of(variants[0], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    let karid = k_of(variants[1], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    // k over the window (ref variant), from the coarse cells inside the 55 km square.
                    let mut ks: Vec<f64> = Vec::new();
                    let r = (half_m / CC) as usize;
                    for row in NC / 2 - r..=NC / 2 + r {
                        for col in NC / 2 - r..=NC / 2 + r {
                            let q = row * NC + col;
                            if zc[q] > 0.0 {
                                ks.push(k_of(variants[0], lc[q], zc[q].max(0.0), pc[q]));
                            }
                        }
                    }
                    ks.sort_by(f64::total_cmp);
                    let pk = |f: f64| ks.get(((ks.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(f64::NAN);
                    println!(
                        "S77SITE {} site{} | L {:.3} | k ref {:.2} (window p10/p50/p90 {:.2}/{:.2}/{:.2}) | k arid {:.2} | bake elevation {:.0} m | precipitation {:.0} mm | temperature {:.1} C | zone {:.2} inland {:.2} ridge {:.2} hill band {:.0} m",
                        s.tile,
                        k + 1,
                        l0,
                        kref,
                        pk(0.1),
                        pk(0.5),
                        pk(0.9),
                        karid,
                        gs.elevation_m,
                        gs.precipitation_mm,
                        gs.temperature_c,
                        parts[1],
                        parts[2],
                        parts[3],
                        parts[4]
                    );
                }
                let (cname, ck, cfloor) = class_of(&s.tile);
                class_k.set(ck);
                println!("S77CLASS {} site{} | {} | k {} | floor {}", s.tile, k + 1, cname, ck, cfloor);
                for v in variants {
                    let t = Instant::now();
                    class_k.set(v.top);
                    k74::KDT_PCT.store(if v.sand { 50 } else { 100 }, Relaxed);
                    // Fine raster: k per cell from L (lattice) and precipitation (bilinear from the coarse grid), the 10 km mean as elevation.
                    let at_coarse = |a: &[f64], row: usize, col: usize| -> f64 {
                        let (x, y) = (-half_m + (col as f64 + 0.5) * ts::CELL_M, half_m - (row as f64 + 0.5) * ts::CELL_M);
                        let fx = (x / CC + (NC / 2) as f64).clamp(0.0, (NC - 2) as f64);
                        let fy = ((NC / 2) as f64 - y / CC).clamp(0.0, (NC - 2) as f64);
                        let (x0, y0) = (fx as usize, fy as usize);
                        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
                        (a[y0 * NC + x0] * (1.0 - tx) + a[y0 * NC + x0 + 1] * tx) * (1.0 - ty) + (a[(y0 + 1) * NC + x0] * (1.0 - tx) + a[(y0 + 1) * NC + x0 + 1] * tx) * ty
                    };
                    let m = box_mean(&z1, g, 167);
                    let mut kf = vec![1f64; g * g];
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            kf[p] = k_of(v, lattice.at(dirs[p]), m[p], at_coarse(&pc, row, col));
                        }
                    }
                    // The crease floor: everywhere (ref), or only where k is about 1 (the arid variants): blend of the floored and plain samples.
                    let zf: Vec<f64> = if v.arid {
                        (0..g * g).map(|p| z0[p] + (z1[p] - z0[p]) * (cfloor / 0.35)).collect()
                    } else {
                        z1.clone()
                    };
                    let mf = box_mean(&zf, g, 167);
                    let h: Vec<f64> = (0..g * g).map(|p| if zf[p] > 0.0 { mf[p] + kf[p] * (zf[p] - mf[p]) } else { zf[p] }).collect();
                    // Pass 1 on the coarse window with the same k rule.
                    let mcs = box_mean(&zc, NC, 6);
                    let hc: Vec<f32> =
                        (0..NC * NC).map(|q| if zc[q] > 0.0 { (mcs[q] + k_of(v, lc[q], mcs[q], pc[q]) * (zc[q] - mcs[q])) as f32 } else { zc[q] as f32 }).collect();
                    let mut cc = hc.clone();
                    let gc = k74::G { w: NC, ox: 0, oy: 0, fix: &[], dx: CC, frec: &[], farea: &[], amax: 0.0 };
                    k74::UK.store(20, Relaxed);
                    k74::LOCAL.store(0, Relaxed);
                    k74::carve(gc, &mut cc, &[]);
                    let dcf: Vec<f64> = (0..NC * NC).map(|q| f64::from(cc[q] - hc[q])).collect();
                    let mut h2: Vec<f32> = Vec::with_capacity(g * g);
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            let dv = at_coarse(&dcf, row, col);
                            h2.push(if zf[p] > 0.0 { (h[p] + dv) as f32 } else { h[p] as f32 });
                        }
                    }
                    let orig: Vec<f32> = h.iter().map(|x| *x as f32).collect();
                    let g2 = k74::G { w: g, ox: 0, oy: 0, fix: &[], dx: ts::CELL_M, frec: &[], farea: &[], amax: 1e6 };
                    k74::LOCAL.store(2, Relaxed);
                    k74::UK.store(0, Relaxed);
                    k74::carve(g2, &mut h2, &[]);
                    k74::thermal_active(g2, &mut h2, 33.0, 100, threads);
                    k74::deposit(g2, &mut h2, &orig, 1e5, 0.5);
                    k74::LOCAL.store(0, Relaxed);
                    k74::UK.store(20, Relaxed);
                    k74::KDT_PCT.store(100, Relaxed);
                    let hf: Vec<f64> = h2.iter().map(|x| f64::from(*x)).collect();
                    let (ps, _) = ts::analyse(&hf, g, g, threads, false);
                    println!("{}", report(&format!("{} site{} {}", s.tile, k + 1, v.name), &ps, &rs));
                    if k == 0 {
                        let b: Vec<u8> = hf.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
                        std::fs::write(dir.join(format!("{}-site1-{}.f32", s.tile, v.name.replace('+', "-"))), b).map_err(|e| e.to_string())?;
                    }
                    eprintln!("S77 {} site{} {}: {:.0} s", s.tile, k + 1, v.name, t.elapsed().as_secs_f64());
                }
            }
        }
        return Ok(());
    }

    // SPIKE-76 (spike code, never merged; from SPIKE-75): `S76_DIR=DIR` runs the pipeline with an assigned relief class per landform (k and crease floor per class, the stand-in for an authored class map) next to SPIKE-74's `ref`. SPIKE-75: logs the pipeline's inputs at every scorecard site and runs SPIKE-74's pipeline with variants:
    // `ref` (SPIKE-74 as run), `arid` (k held at 1 or below where the generator is dry, the crease floor only where k is about 1), `arid+top8`
    // (and the top knot 8), `sand` (as arid+top8, k 0.4 where very dry). Same tables as SPIKE-74, plus per site: L, k, base elevation, precipitation,
    // temperature, zone, inland, ridge at the centre and k p10/p50/p90 over the window.
    if let Ok(dir) = std::env::var("S76_DIR") {
        use planet_generators::height::HeightField as _;
        use std::sync::atomic::Ordering::Relaxed;
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = ts::GRID;
        let seed = field_s74.seed();
        let lattice = planet_generators::global::landform_lattice(seed);
        let smooth = |x: f64, a: f64, b: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        #[derive(Clone, Copy)]
        struct V {
            name: &'static str,
            arid: bool,
            top: f64,
            sand: bool,
        }
        let variants = [
            V { name: "ref", arid: false, top: 5.625, sand: false },
            V { name: "class", arid: true, top: 0.0, sand: false },
        ];
        // The assigned class per scorecard tile: (class name, k, crease floor).
        let class_of = |tile: &str| -> (&'static str, f64, f64) {
            match tile {
                "N52E012" => ("plain", 1.0, 0.2),
                "N41W096" => ("rolling plain", 1.0, 0.35),
                "N43W102" => ("badlands (arid rugged)", 1.0, 0.0),
                "N31W004" => ("sand sea (arid flat)", 0.35, 0.0),
                "N47E011" | "N27E086" => ("high mountains", 7.5, 0.0),
                _ => ("hills", 5.0, 0.0),
            }
        };
        // k of L (SPIKE-69's knots, the top knot `top`), the coast taper on `e`, and the aridity gate on precipitation `p` (mm): from 1 under 250 mm to
        // full over 500 mm; with `sand`, under 150 mm k falls toward 0.4.
        let class_k = std::cell::Cell::new(1.0f64);
        let k_of = |v: V, l: f64, e: f64, p: f64| {
            if v.arid {
                return 1.0 + smooth(e, 0.0, 400.0) * (class_k.get() - 1.0);
            }
            let (xs, ys) = ([0.0, 0.028, 0.176, 1.0], [1.0, 4.5, 4.5, v.top]);
            let mut kk = ys[3];
            for j in 0..3 {
                if l <= xs[j + 1] {
                    kk = ys[j] + (ys[j + 1] - ys[j]) * (l - xs[j]) / (xs[j + 1] - xs[j]);
                    break;
                }
            }
            let mut k = 1.0 + smooth(e, 0.0, 400.0) * (kk - 1.0);
            if v.arid {
                k = 1.0 + (k - 1.0) * smooth(p, 250.0, 500.0);
            }
            if v.sand {
                k -= 0.6 * (1.0 - smooth(p, 80.0, 150.0)) * smooth(e, 0.0, 400.0);
            }
            k
        };
        let box_mean = |z: &[f64], n: usize, r: usize| -> Vec<f64> {
            let w = n + 1;
            let mut s = vec![0f64; w * w];
            for j in 0..n {
                let mut row = 0.0;
                for i in 0..n {
                    row += z[j * n + i];
                    s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
                }
            }
            let mut out = vec![0f64; n * n];
            for j in 0..n {
                for i in 0..n {
                    let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(n), j.saturating_sub(r), (j + r + 1).min(n));
                    out[j * n + i] = (s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]) / ((i1 - i0) * (j1 - j0)) as f64;
                }
            }
            out
        };
        let frame = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
            (
                planet_core::Vec3::new(cla * clo, cla * slo, sla),
                planet_core::Vec3::new(-slo, clo, 0.0),
                planet_core::Vec3::new(-sla * clo, -sla * slo, cla),
            )
        };
        const NC: usize = 491;
        const CC: f64 = 814.0;
        let report = |label: &str, s: &RasterStats, real: &RasterStats| -> String {
            let d = ts::distances(real, s);
            let cs: Vec<String> = ts::CONTINUOUS
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{}={:.3}/{}", c.id, s.series[k].p(50), d.continuous[k].map_or("-".into(), |x| format!("{x:.2}"))))
                .collect();
            format!(
                "S76 {label} | flat {:.1}% | classes L1 {:.2} | density ratio {} | {}",
                100.0 * s.forms[0],
                d.forms_l1,
                d.density_ratio.map_or("-".into(), |x| format!("{x:.2}")),
                cs.join(" ")
            )
        };
        let half_m = ts::HALF_DEG.to_radians() * radius;
        for (i, s) in specs.iter().enumerate() {
            let (rs, _) = ts::analyse(&reals[i], g, g, threads, false);
            let cs: Vec<String> = ts::CONTINUOUS.iter().enumerate().map(|(k, c)| format!("{}={:.3}", c.id, rs.series[k].p(50))).collect();
            println!("S76 {} real | flat {:.1}% | {}", s.tile, 100.0 * rs.forms[0], cs.join(" "));
            for (k, &(lat, lon)) in s.sites.iter().enumerate() {
                let z0 = &gens[i][k];
                let (bs, _) = ts::analyse(z0, g, g, threads, false);
                println!("{}", report(&format!("{} site{} baseline", s.tile, k + 1), &bs, &rs));
                let dirs = ts::generator_directions(lat, lon, radius);
                planet_generators::erosion::spike73_set_floor(0.35);
                let z1 = field_s74.heights_threaded(&dirs, &threads);
                // The coarse window: point samples at 814 m (floor on, as SPIKE-74), and per coarse cell L, base elevation and precipitation.
                let (c, east, north) = frame(lat, lon);
                let mut dc = Vec::with_capacity(NC * NC);
                for row in 0..NC {
                    for col in 0..NC {
                        let (x, y) = ((col as f64 - (NC / 2) as f64) * CC, ((NC / 2) as f64 - row as f64) * CC);
                        let p = c * radius + east * x + north * y;
                        dc.push(p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z)));
                    }
                }
                let zc = field_s74.heights_threaded(&dc, &threads);
                planet_generators::erosion::spike73_set_floor(0.0);
                let pc: Vec<f64> = dc.iter().map(|d| f64::from(planet_generators::global::sample(seed, *d).precipitation_mm)).collect();
                let lc: Vec<f64> = dc.iter().map(|d| lattice.at(*d)).collect();
                // Site log at the centre.
                {
                    let d0 = planet_core::Vec3::new(c.x, c.y, c.z);
                    let gs = planet_generators::global::sample(seed, d0);
                    let parts = planet_generators::global::spike69_parts(seed, d0);
                    let l0 = lattice.at(d0);
                    let kref = k_of(variants[0], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    let karid = k_of(variants[1], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    // k over the window (ref variant), from the coarse cells inside the 55 km square.
                    let mut ks: Vec<f64> = Vec::new();
                    let r = (half_m / CC) as usize;
                    for row in NC / 2 - r..=NC / 2 + r {
                        for col in NC / 2 - r..=NC / 2 + r {
                            let q = row * NC + col;
                            if zc[q] > 0.0 {
                                ks.push(k_of(variants[0], lc[q], zc[q].max(0.0), pc[q]));
                            }
                        }
                    }
                    ks.sort_by(f64::total_cmp);
                    let pk = |f: f64| ks.get(((ks.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(f64::NAN);
                    println!(
                        "S76SITE {} site{} | L {:.3} | k ref {:.2} (window p10/p50/p90 {:.2}/{:.2}/{:.2}) | k arid {:.2} | bake elevation {:.0} m | precipitation {:.0} mm | temperature {:.1} C | zone {:.2} inland {:.2} ridge {:.2} hill band {:.0} m",
                        s.tile,
                        k + 1,
                        l0,
                        kref,
                        pk(0.1),
                        pk(0.5),
                        pk(0.9),
                        karid,
                        gs.elevation_m,
                        gs.precipitation_mm,
                        gs.temperature_c,
                        parts[1],
                        parts[2],
                        parts[3],
                        parts[4]
                    );
                }
                let (cname, ck, cfloor) = class_of(&s.tile);
                class_k.set(ck);
                println!("S76CLASS {} site{} | {} | k {} | floor {}", s.tile, k + 1, cname, ck, cfloor);
                for v in variants {
                    let t = Instant::now();
                    // Fine raster: k per cell from L (lattice) and precipitation (bilinear from the coarse grid), the 10 km mean as elevation.
                    let at_coarse = |a: &[f64], row: usize, col: usize| -> f64 {
                        let (x, y) = (-half_m + (col as f64 + 0.5) * ts::CELL_M, half_m - (row as f64 + 0.5) * ts::CELL_M);
                        let fx = (x / CC + (NC / 2) as f64).clamp(0.0, (NC - 2) as f64);
                        let fy = ((NC / 2) as f64 - y / CC).clamp(0.0, (NC - 2) as f64);
                        let (x0, y0) = (fx as usize, fy as usize);
                        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
                        (a[y0 * NC + x0] * (1.0 - tx) + a[y0 * NC + x0 + 1] * tx) * (1.0 - ty) + (a[(y0 + 1) * NC + x0] * (1.0 - tx) + a[(y0 + 1) * NC + x0 + 1] * tx) * ty
                    };
                    let m = box_mean(&z1, g, 167);
                    let mut kf = vec![1f64; g * g];
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            kf[p] = k_of(v, lattice.at(dirs[p]), m[p], at_coarse(&pc, row, col));
                        }
                    }
                    // The crease floor: everywhere (ref), or only where k is about 1 (the arid variants): blend of the floored and plain samples.
                    let zf: Vec<f64> = if v.arid {
                        (0..g * g).map(|p| z0[p] + (z1[p] - z0[p]) * (cfloor / 0.35)).collect()
                    } else {
                        z1.clone()
                    };
                    let mf = box_mean(&zf, g, 167);
                    let h: Vec<f64> = (0..g * g).map(|p| if zf[p] > 0.0 { mf[p] + kf[p] * (zf[p] - mf[p]) } else { zf[p] }).collect();
                    // Pass 1 on the coarse window with the same k rule.
                    let mcs = box_mean(&zc, NC, 6);
                    let hc: Vec<f32> =
                        (0..NC * NC).map(|q| if zc[q] > 0.0 { (mcs[q] + k_of(v, lc[q], mcs[q], pc[q]) * (zc[q] - mcs[q])) as f32 } else { zc[q] as f32 }).collect();
                    let mut cc = hc.clone();
                    let gc = k74::G { w: NC, ox: 0, oy: 0, fix: &[], dx: CC, frec: &[], farea: &[], amax: 0.0 };
                    k74::UK.store(20, Relaxed);
                    k74::LOCAL.store(0, Relaxed);
                    k74::carve(gc, &mut cc, &[]);
                    let dcf: Vec<f64> = (0..NC * NC).map(|q| f64::from(cc[q] - hc[q])).collect();
                    let mut h2: Vec<f32> = Vec::with_capacity(g * g);
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            let dv = at_coarse(&dcf, row, col);
                            h2.push(if zf[p] > 0.0 { (h[p] + dv) as f32 } else { h[p] as f32 });
                        }
                    }
                    let orig: Vec<f32> = h.iter().map(|x| *x as f32).collect();
                    let g2 = k74::G { w: g, ox: 0, oy: 0, fix: &[], dx: ts::CELL_M, frec: &[], farea: &[], amax: 1e6 };
                    k74::LOCAL.store(2, Relaxed);
                    k74::UK.store(0, Relaxed);
                    k74::carve(g2, &mut h2, &[]);
                    k74::thermal_active(g2, &mut h2, 33.0, 100, threads);
                    k74::deposit(g2, &mut h2, &orig, 1e5, 0.5);
                    k74::LOCAL.store(0, Relaxed);
                    k74::UK.store(20, Relaxed);
                    let hf: Vec<f64> = h2.iter().map(|x| f64::from(*x)).collect();
                    let (ps, _) = ts::analyse(&hf, g, g, threads, false);
                    println!("{}", report(&format!("{} site{} {}", s.tile, k + 1, v.name), &ps, &rs));
                    if k == 0 {
                        let b: Vec<u8> = hf.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
                        std::fs::write(dir.join(format!("{}-site1-{}.f32", s.tile, v.name.replace('+', "-"))), b).map_err(|e| e.to_string())?;
                    }
                    eprintln!("S76 {} site{} {}: {:.0} s", s.tile, k + 1, v.name, t.elapsed().as_secs_f64());
                }
            }
        }
        return Ok(());
    }

    // SPIKE-75 (spike code, never merged): `S75_DIR=DIR` logs the pipeline's inputs at every scorecard site and runs SPIKE-74's pipeline with variants:
    // `ref` (SPIKE-74 as run), `arid` (k held at 1 or below where the generator is dry, the crease floor only where k is about 1), `arid+top8`
    // (and the top knot 8), `sand` (as arid+top8, k 0.4 where very dry). Same tables as SPIKE-74, plus per site: L, k, base elevation, precipitation,
    // temperature, zone, inland, ridge at the centre and k p10/p50/p90 over the window.
    if let Ok(dir) = std::env::var("S75_DIR") {
        use planet_generators::height::HeightField as _;
        use std::sync::atomic::Ordering::Relaxed;
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = ts::GRID;
        let seed = field_s74.seed();
        let lattice = planet_generators::global::landform_lattice(seed);
        let smooth = |x: f64, a: f64, b: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        #[derive(Clone, Copy)]
        struct V {
            name: &'static str,
            arid: bool,
            top: f64,
            sand: bool,
        }
        let variants = [
            V { name: "ref", arid: false, top: 5.625, sand: false },
            V { name: "arid", arid: true, top: 5.625, sand: false },
            V { name: "arid+top8", arid: true, top: 8.0, sand: false },
            V { name: "sand", arid: true, top: 8.0, sand: true },
        ];
        // k of L (SPIKE-69's knots, the top knot `top`), the coast taper on `e`, and the aridity gate on precipitation `p` (mm): from 1 under 250 mm to
        // full over 500 mm; with `sand`, under 150 mm k falls toward 0.4.
        let k_of = |v: V, l: f64, e: f64, p: f64| {
            let (xs, ys) = ([0.0, 0.028, 0.176, 1.0], [1.0, 4.5, 4.5, v.top]);
            let mut kk = ys[3];
            for j in 0..3 {
                if l <= xs[j + 1] {
                    kk = ys[j] + (ys[j + 1] - ys[j]) * (l - xs[j]) / (xs[j + 1] - xs[j]);
                    break;
                }
            }
            let mut k = 1.0 + smooth(e, 0.0, 400.0) * (kk - 1.0);
            if v.arid {
                k = 1.0 + (k - 1.0) * smooth(p, 250.0, 500.0);
            }
            if v.sand {
                k -= 0.6 * (1.0 - smooth(p, 80.0, 150.0)) * smooth(e, 0.0, 400.0);
            }
            k
        };
        let box_mean = |z: &[f64], n: usize, r: usize| -> Vec<f64> {
            let w = n + 1;
            let mut s = vec![0f64; w * w];
            for j in 0..n {
                let mut row = 0.0;
                for i in 0..n {
                    row += z[j * n + i];
                    s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
                }
            }
            let mut out = vec![0f64; n * n];
            for j in 0..n {
                for i in 0..n {
                    let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(n), j.saturating_sub(r), (j + r + 1).min(n));
                    out[j * n + i] = (s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]) / ((i1 - i0) * (j1 - j0)) as f64;
                }
            }
            out
        };
        let frame = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
            (
                planet_core::Vec3::new(cla * clo, cla * slo, sla),
                planet_core::Vec3::new(-slo, clo, 0.0),
                planet_core::Vec3::new(-sla * clo, -sla * slo, cla),
            )
        };
        const NC: usize = 491;
        const CC: f64 = 814.0;
        let report = |label: &str, s: &RasterStats, real: &RasterStats| -> String {
            let d = ts::distances(real, s);
            let cs: Vec<String> = ts::CONTINUOUS
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{}={:.3}/{}", c.id, s.series[k].p(50), d.continuous[k].map_or("-".into(), |x| format!("{x:.2}"))))
                .collect();
            format!(
                "S75 {label} | flat {:.1}% | classes L1 {:.2} | density ratio {} | {}",
                100.0 * s.forms[0],
                d.forms_l1,
                d.density_ratio.map_or("-".into(), |x| format!("{x:.2}")),
                cs.join(" ")
            )
        };
        let half_m = ts::HALF_DEG.to_radians() * radius;
        for (i, s) in specs.iter().enumerate() {
            let (rs, _) = ts::analyse(&reals[i], g, g, threads, false);
            let cs: Vec<String> = ts::CONTINUOUS.iter().enumerate().map(|(k, c)| format!("{}={:.3}", c.id, rs.series[k].p(50))).collect();
            println!("S75 {} real | flat {:.1}% | {}", s.tile, 100.0 * rs.forms[0], cs.join(" "));
            for (k, &(lat, lon)) in s.sites.iter().enumerate() {
                let z0 = &gens[i][k];
                let (bs, _) = ts::analyse(z0, g, g, threads, false);
                println!("{}", report(&format!("{} site{} baseline", s.tile, k + 1), &bs, &rs));
                let dirs = ts::generator_directions(lat, lon, radius);
                planet_generators::erosion::spike73_set_floor(0.35);
                let z1 = field_s74.heights_threaded(&dirs, &threads);
                // The coarse window: point samples at 814 m (floor on, as SPIKE-74), and per coarse cell L, base elevation and precipitation.
                let (c, east, north) = frame(lat, lon);
                let mut dc = Vec::with_capacity(NC * NC);
                for row in 0..NC {
                    for col in 0..NC {
                        let (x, y) = ((col as f64 - (NC / 2) as f64) * CC, ((NC / 2) as f64 - row as f64) * CC);
                        let p = c * radius + east * x + north * y;
                        dc.push(p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z)));
                    }
                }
                let zc = field_s74.heights_threaded(&dc, &threads);
                planet_generators::erosion::spike73_set_floor(0.0);
                let pc: Vec<f64> = dc.iter().map(|d| f64::from(planet_generators::global::sample(seed, *d).precipitation_mm)).collect();
                let lc: Vec<f64> = dc.iter().map(|d| lattice.at(*d)).collect();
                // Site log at the centre.
                {
                    let d0 = planet_core::Vec3::new(c.x, c.y, c.z);
                    let gs = planet_generators::global::sample(seed, d0);
                    let parts = planet_generators::global::spike69_parts(seed, d0);
                    let l0 = lattice.at(d0);
                    let kref = k_of(variants[0], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    let karid = k_of(variants[1], l0, f64::from(gs.elevation_m), f64::from(gs.precipitation_mm));
                    // k over the window (ref variant), from the coarse cells inside the 55 km square.
                    let mut ks: Vec<f64> = Vec::new();
                    let r = (half_m / CC) as usize;
                    for row in NC / 2 - r..=NC / 2 + r {
                        for col in NC / 2 - r..=NC / 2 + r {
                            let q = row * NC + col;
                            if zc[q] > 0.0 {
                                ks.push(k_of(variants[0], lc[q], zc[q].max(0.0), pc[q]));
                            }
                        }
                    }
                    ks.sort_by(f64::total_cmp);
                    let pk = |f: f64| ks.get(((ks.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(f64::NAN);
                    println!(
                        "S75SITE {} site{} | L {:.3} | k ref {:.2} (window p10/p50/p90 {:.2}/{:.2}/{:.2}) | k arid {:.2} | bake elevation {:.0} m | precipitation {:.0} mm | temperature {:.1} C | zone {:.2} inland {:.2} ridge {:.2} hill band {:.0} m",
                        s.tile,
                        k + 1,
                        l0,
                        kref,
                        pk(0.1),
                        pk(0.5),
                        pk(0.9),
                        karid,
                        gs.elevation_m,
                        gs.precipitation_mm,
                        gs.temperature_c,
                        parts[1],
                        parts[2],
                        parts[3],
                        parts[4]
                    );
                }
                for v in variants {
                    let t = Instant::now();
                    // Fine raster: k per cell from L (lattice) and precipitation (bilinear from the coarse grid), the 10 km mean as elevation.
                    let at_coarse = |a: &[f64], row: usize, col: usize| -> f64 {
                        let (x, y) = (-half_m + (col as f64 + 0.5) * ts::CELL_M, half_m - (row as f64 + 0.5) * ts::CELL_M);
                        let fx = (x / CC + (NC / 2) as f64).clamp(0.0, (NC - 2) as f64);
                        let fy = ((NC / 2) as f64 - y / CC).clamp(0.0, (NC - 2) as f64);
                        let (x0, y0) = (fx as usize, fy as usize);
                        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
                        (a[y0 * NC + x0] * (1.0 - tx) + a[y0 * NC + x0 + 1] * tx) * (1.0 - ty) + (a[(y0 + 1) * NC + x0] * (1.0 - tx) + a[(y0 + 1) * NC + x0 + 1] * tx) * ty
                    };
                    let m = box_mean(&z1, g, 167);
                    let mut kf = vec![1f64; g * g];
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            kf[p] = k_of(v, lattice.at(dirs[p]), m[p], at_coarse(&pc, row, col));
                        }
                    }
                    // The crease floor: everywhere (ref), or only where k is about 1 (the arid variants): blend of the floored and plain samples.
                    let zf: Vec<f64> = if v.arid {
                        (0..g * g).map(|p| z0[p] + (z1[p] - z0[p]) * (1.0 - smooth(kf[p], 1.0, 1.5))).collect()
                    } else {
                        z1.clone()
                    };
                    let mf = box_mean(&zf, g, 167);
                    let h: Vec<f64> = (0..g * g).map(|p| if zf[p] > 0.0 { mf[p] + kf[p] * (zf[p] - mf[p]) } else { zf[p] }).collect();
                    // Pass 1 on the coarse window with the same k rule.
                    let mcs = box_mean(&zc, NC, 6);
                    let hc: Vec<f32> =
                        (0..NC * NC).map(|q| if zc[q] > 0.0 { (mcs[q] + k_of(v, lc[q], mcs[q], pc[q]) * (zc[q] - mcs[q])) as f32 } else { zc[q] as f32 }).collect();
                    let mut cc = hc.clone();
                    let gc = k74::G { w: NC, ox: 0, oy: 0, fix: &[], dx: CC, frec: &[], farea: &[], amax: 0.0 };
                    k74::UK.store(20, Relaxed);
                    k74::LOCAL.store(0, Relaxed);
                    k74::carve(gc, &mut cc, &[]);
                    let dcf: Vec<f64> = (0..NC * NC).map(|q| f64::from(cc[q] - hc[q])).collect();
                    let mut h2: Vec<f32> = Vec::with_capacity(g * g);
                    for row in 0..g {
                        for col in 0..g {
                            let p = row * g + col;
                            let dv = at_coarse(&dcf, row, col);
                            h2.push(if zf[p] > 0.0 { (h[p] + dv) as f32 } else { h[p] as f32 });
                        }
                    }
                    let orig: Vec<f32> = h.iter().map(|x| *x as f32).collect();
                    let g2 = k74::G { w: g, ox: 0, oy: 0, fix: &[], dx: ts::CELL_M, frec: &[], farea: &[], amax: 1e6 };
                    k74::LOCAL.store(2, Relaxed);
                    k74::UK.store(0, Relaxed);
                    k74::carve(g2, &mut h2, &[]);
                    k74::thermal_active(g2, &mut h2, 33.0, 100, threads);
                    k74::deposit(g2, &mut h2, &orig, 1e5, 0.5);
                    k74::LOCAL.store(0, Relaxed);
                    k74::UK.store(20, Relaxed);
                    let hf: Vec<f64> = h2.iter().map(|x| f64::from(*x)).collect();
                    let (ps, _) = ts::analyse(&hf, g, g, threads, false);
                    println!("{}", report(&format!("{} site{} {}", s.tile, k + 1, v.name), &ps, &rs));
                    if k == 0 {
                        let b: Vec<u8> = hf.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
                        std::fs::write(dir.join(format!("{}-site1-{}.f32", s.tile, v.name.replace('+', "-"))), b).map_err(|e| e.to_string())?;
                    }
                    eprintln!("S75 {} site{} {}: {:.0} s", s.tile, k + 1, v.name, t.elapsed().as_secs_f64());
                }
            }
        }
        return Ok(());
    }

    // SPIKE-74 (spike code, never merged): `S74_DIR=DIR` runs the proposed M9 pipeline on every scorecard site and returns. Per site:
    // the fine-crease floor 0.35 (SPIKE-73), k from SPIKE-69's curve of the landform lattice with the coast taper about the 10 km mean, pass 1 (the
    // carve on a 400 km window at 814 m, its change upsampled), pass 2 (the incising local carve without uplift at 30 m, cap 1 km^2), active-cell
    // thermal at 33 degrees and one deposition sweep. Prints, per landform and site, the scorecard's p50s and distances for the baseline (today's
    // field) and the pipeline against the real tile, and writes both rasters as f32 for the stills.
    if let Ok(dir) = std::env::var("S74_DIR") {
        use planet_generators::height::HeightField as _;
        use std::sync::atomic::Ordering::Relaxed;
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = ts::GRID;
        let lattice = planet_generators::global::landform_lattice(field_s74.seed());
        let smooth = |x: f64, a: f64, b: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // SPIKE-69's curve of L (knots in L) and the coast taper (0 to 400 m).
        let k_of = |l: f64, e: f64| {
            let (xs, ys) = ([0.0, 0.028, 0.176, 1.0], [1.0, 4.5, 4.5, 5.625]);
            let mut v = ys[3];
            for j in 0..3 {
                if l <= xs[j + 1] {
                    v = ys[j] + (ys[j + 1] - ys[j]) * (l - xs[j]) / (xs[j + 1] - xs[j]);
                    break;
                }
            }
            1.0 + smooth(e, 0.0, 400.0) * (v - 1.0)
        };
        let box_mean = |z: &[f64], n: usize, r: usize| -> Vec<f64> {
            let w = n + 1;
            let mut s = vec![0f64; w * w];
            for j in 0..n {
                let mut row = 0.0;
                for i in 0..n {
                    row += z[j * n + i];
                    s[(j + 1) * w + i + 1] = s[j * w + i + 1] + row;
                }
            }
            let mut out = vec![0f64; n * n];
            for j in 0..n {
                for i in 0..n {
                    let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(n), j.saturating_sub(r), (j + r + 1).min(n));
                    out[j * n + i] = (s[j1 * w + i1] - s[j0 * w + i1] - s[j1 * w + i0] + s[j0 * w + i0]) / ((i1 - i0) * (j1 - j0)) as f64;
                }
            }
            out
        };
        // The pipeline's amplitude step on a raster of `n` cells of `cell` m: m + k (h - m) on land, m the 10 km box mean, k per cell.
        let amplify = |z: &[f64], dirs: &[planet_core::Vec3], n: usize, cell: f64| -> Vec<f64> {
            let m = box_mean(z, n, (10_000.0 / cell / 2.0).round() as usize);
            z.iter().zip(&m).zip(dirs).map(|((h, mm), d)| if *h > 0.0 { mm + k_of(lattice.at(*d), *mm) * (h - mm) } else { *h }).collect()
        };
        let frame = |lat: f64, lon: f64| {
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
            (
                planet_core::Vec3::new(cla * clo, cla * slo, sla),
                planet_core::Vec3::new(-slo, clo, 0.0),
                planet_core::Vec3::new(-sla * clo, -sla * slo, cla),
            )
        };
        const NC: usize = 491;
        const CC: f64 = 814.0;
        let report = |label: &str, s: &RasterStats, real: &RasterStats| -> String {
            let d = ts::distances(real, s);
            let cs: Vec<String> = ts::CONTINUOUS
                .iter()
                .enumerate()
                .map(|(k, c)| format!("{}={:.3}/{}", c.id, s.series[k].p(50), d.continuous[k].map_or("-".into(), |x| format!("{x:.2}"))))
                .collect();
            format!(
                "S74 {label} | flat {:.1}% | classes L1 {:.2} | density ratio {} | {}",
                100.0 * s.forms[0],
                d.forms_l1,
                d.density_ratio.map_or("-".into(), |x| format!("{x:.2}")),
                cs.join(" ")
            )
        };
        for (i, s) in specs.iter().enumerate() {
            let (rs, _) = ts::analyse(&reals[i], g, g, threads, false);
            let cs: Vec<String> = ts::CONTINUOUS.iter().enumerate().map(|(k, c)| format!("{}={:.3}", c.id, rs.series[k].p(50))).collect();
            println!("S74 {} real | flat {:.1}% | {}", s.tile, 100.0 * rs.forms[0], cs.join(" "));
            for (k, &(lat, lon)) in s.sites.iter().enumerate() {
                let t = Instant::now();
                let z0 = &gens[i][k];
                let (bs, _) = ts::analyse(z0, g, g, threads, false);
                println!("{}", report(&format!("{} site{} baseline", s.tile, k + 1), &bs, &rs));
                // The floor, then k.
                planet_generators::erosion::spike73_set_floor(0.35);
                let dirs = ts::generator_directions(lat, lon, radius);
                let z1 = field_s74.heights_threaded(&dirs, &threads);
                let h = amplify(&z1, &dirs, g, ts::CELL_M);
                // Pass 1 on a 400 km window at 814 m.
                let (c, east, north) = frame(lat, lon);
                let mut dc = Vec::with_capacity(NC * NC);
                for row in 0..NC {
                    for col in 0..NC {
                        let (x, y) = ((col as f64 - (NC / 2) as f64) * CC, ((NC / 2) as f64 - row as f64) * CC);
                        let p = c * radius + east * x + north * y;
                        dc.push(p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z)));
                    }
                }
                let zc = field_s74.heights_threaded(&dc, &threads);
                planet_generators::erosion::spike73_set_floor(0.0);
                let hc: Vec<f32> = amplify(&zc, &dc, NC, CC).iter().map(|x| *x as f32).collect();
                let mut cc = hc.clone();
                let gc = k74::G { w: NC, ox: 0, oy: 0, fix: &[], dx: CC, frec: &[], farea: &[], amax: 0.0 };
                k74::UK.store(20, Relaxed);
                k74::carve(gc, &mut cc, &[]);
                let half_m = ts::HALF_DEG.to_radians() * radius;
                let mut h2: Vec<f32> = Vec::with_capacity(g * g);
                for row in 0..g {
                    for col in 0..g {
                        let (x, y) = (-half_m + (col as f64 + 0.5) * ts::CELL_M, half_m - (row as f64 + 0.5) * ts::CELL_M);
                        let fx = (x / CC + (NC / 2) as f64).clamp(0.0, (NC - 2) as f64);
                        let fy = ((NC / 2) as f64 - y / CC).clamp(0.0, (NC - 2) as f64);
                        let (x0, y0) = (fx as usize, fy as usize);
                        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
                        let dl = |xx: usize, yy: usize| f64::from(cc[yy * NC + xx] - hc[yy * NC + xx]);
                        let v = (dl(x0, y0) * (1.0 - tx) + dl(x0 + 1, y0) * tx) * (1.0 - ty) + (dl(x0, y0 + 1) * (1.0 - tx) + dl(x0 + 1, y0 + 1) * tx) * ty;
                        let p = row * g + col;
                        h2.push(if z1[p] > 0.0 { (h[p] + v) as f32 } else { h[p] as f32 });
                    }
                }
                let orig: Vec<f32> = h.iter().map(|x| *x as f32).collect();
                // Pass 2: incise only, no uplift; thermal; deposition.
                let g2 = k74::G { w: g, ox: 0, oy: 0, fix: &[], dx: ts::CELL_M, frec: &[], farea: &[], amax: 1e6 };
                k74::LOCAL.store(2, Relaxed);
                k74::UK.store(0, Relaxed);
                k74::carve(g2, &mut h2, &[]);
                k74::thermal_active(g2, &mut h2, 33.0, 100, threads);
                k74::deposit(g2, &mut h2, &orig, 1e5, 0.5);
                k74::LOCAL.store(0, Relaxed);
                k74::UK.store(20, Relaxed);
                let hf: Vec<f64> = h2.iter().map(|x| f64::from(*x)).collect();
                let (ps, _) = ts::analyse(&hf, g, g, threads, false);
                println!("{}", report(&format!("{} site{} pipeline", s.tile, k + 1), &ps, &rs));
                let wr = |name: String, z: &[f64]| {
                    let b: Vec<u8> = z.iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
                    std::fs::write(dir.join(name), b).expect("write a raster");
                };
                wr(format!("{}-site{}-baseline.f32", s.tile, k + 1), z0);
                wr(format!("{}-site{}-pipeline.f32", s.tile, k + 1), &hf);
                eprintln!("S74 {} site{}: {:.0} s", s.tile, k + 1, t.elapsed().as_secs_f64());
            }
            let rw: Vec<u8> = reals[i].iter().flat_map(|x| (*x as f32).to_le_bytes()).collect();
            std::fs::write(dir.join(format!("{}-real.f32", s.tile)), rw).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    // Phase 3: the descriptors on 27 rasters, a few at a time.
    let t = Instant::now();
    let jobs: Vec<(usize, Option<usize>)> =
        (0..specs.len()).flat_map(|i| std::iter::once((i, None)).chain((0..specs[i].sites.len()).map(move |k| (i, Some(k))))).collect();
    let workers = threads.min(RASTER_WORKERS);
    let kernel_threads = (threads / workers).max(1);
    let analysed: Vec<(RasterStats, Vec<ts::MapImage>)> = map_par(workers, jobs.len(), |j| {
        let (i, site) = jobs[j];
        let z = match site {
            None => &reals[i],
            Some(k) => &gens[i][k],
        };
        ts::analyse(z, ts::GRID, ts::GRID, kernel_threads, c.maps)
    });
    drop((reals, gens));
    lap("descriptors", t);

    // Phase 4: the report.
    let t = Instant::now();
    let mut results: Vec<LandformResult> = Vec::new();
    let mut maps_out: Vec<(usize, String, Vec<ts::MapImage>)> = Vec::new();
    let mut it = analysed.into_iter();
    for (i, s) in specs.iter().enumerate() {
        let (real, m) = it.next().expect("a real raster per landform");
        maps_out.push((i, "real".into(), m));
        let mut generator = Vec::new();
        for k in 0..s.sites.len() {
            let (g, m) = it.next().expect("a raster per site");
            maps_out.push((i, format!("site{}", k + 1), m));
            generator.push(g);
        }
        results.push(LandformResult { name: s.name.clone(), tile: s.tile.clone(), sites: s.sites.clone(), real, generator });
    }
    let notice = Notice { notice: glo30::COPERNICUS_NOTICE, no_liability: glo30::COPERNICUS_NO_LIABILITY, doi: glo30::COPERNICUS_DOI };
    lap("collect results", t);
    let mut info = RunInfo {
        date,
        commit: git_commit(),
        store: c.store.display().to_string(),
        definition_hash,
        dem_dir: c.dem.display().to_string(),
        sites_file: c.sites.display().to_string(),
        threads,
        phases: phases.clone(),
    };
    if c.maps {
        let t = Instant::now();
        for (i, label, m) in maps_out {
            write_maps(&out_dir, &specs[i], &label, m)?;
        }
        info.phases.push(("maps".into(), t.elapsed().as_secs_f64()));
    }
    info.phases.push(("total".into(), total.elapsed().as_secs_f64()));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;
    let (md, js) = (out_dir.join("report.md"), out_dir.join("report.json"));
    std::fs::write(&md, ts::render_markdown(&info, &notice, &results)).map_err(|e| format!("writing {}: {e}", md.display()))?;
    std::fs::write(&js, ts::render_json(&info, &notice, &results)).map_err(|e| format!("writing {}: {e}", js.display()))?;
    println!("terrain-score: report {} ({:.1} s)", md.display(), total.elapsed().as_secs_f64());
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("terrain-score: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
