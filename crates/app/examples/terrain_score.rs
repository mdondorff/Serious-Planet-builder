//! The terrain scorecard run (TEST-079, TEST-080; `cargo xtask terrain-score`): per landform of the sites file the seven descriptors on the real GLO-30 tile's
//! central 0.5 degree square and on the generator's two matched sites (`world.field()` of the production store, read only), the distances between them, and
//! `report.md` / `report.json`. CPU only. A correctness run, not a measurement.
//! `cargo run --release -p planet --example terrain_score -- [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

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
    drop(field);
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
