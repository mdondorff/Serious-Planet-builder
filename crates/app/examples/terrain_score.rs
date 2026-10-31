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
            let dirs = ts::generator_directions(lat, lon, radius);
            windows.push(field.heights_threaded(&dirs, &threads));
        }
        gens.push(windows);
    }
    drop(field);
    lap("sample generator windows", t);

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
