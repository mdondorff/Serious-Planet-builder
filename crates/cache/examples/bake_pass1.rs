//! `cargo xtask bake pass1 ...` runs this (m9-bake PR 4, GEN-100): pass 1 of the erosion layer over the cube faces at `--planet-res` cells per face edge, one face at a time,
//! into a layer store (checkpointed per face: a killed run resumes), then the preview maps of the carved base. Reports the wall time and the peak memory of each face.
//! Reads no other store. Writes only under `--out` (default `target/bake/pass1-seed-<N>-res-<R>`). The full resolution (12,288) is not run without the planner's go.

use std::path::PathBuf;
use std::time::Instant;

use planet_cache::global_store::{FieldSource, GlobalWorld};
use planet_cache::layer_store::{load_face_change, run_pass1_rock, LayerSpec};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::map::{edge_steps, pass1_maps};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
/// The largest `--planet-res` a run takes without the planner's go (the full resolution is 12,288).
const COARSE_LIMIT: usize = 12_288;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut halo: Option<usize> = None;
    let (mut seed, mut res, mut out, mut threads, mut concurrency, mut width) =
        (14u64, 12_288usize, None::<PathBuf>, 0usize, 1usize, 1024usize);
    let mut faces: Vec<u8> = (0..6).collect();
    let (mut global_dir, mut global_res, mut class_source, mut legacy_source) = (None::<PathBuf>, 16_384u32, false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--seed" => seed = value().parse().unwrap_or_else(|_| fail("--seed needs a non-negative integer")),
            "--planet-res" => res = value().parse().unwrap_or_else(|_| fail("--planet-res needs an integer (cells per face edge)")),
            "--halo" => halo = Some(value().parse().unwrap_or_else(|_| fail("--halo needs an integer (cells beyond each face edge)"))),
            "--out" => out = Some(PathBuf::from(value())),
            "--threads" => threads = value().parse().unwrap_or_else(|_| fail("--threads needs an integer")),
            "--concurrency" => concurrency = value().parse().unwrap_or_else(|_| fail("--concurrency needs an integer")),
            "--map-width" => width = value().parse().unwrap_or_else(|_| fail("--map-width needs an even integer")),
            "--global-dir" => global_dir = Some(PathBuf::from(value())),
            "--global-res" => global_res = value().parse().unwrap_or_else(|_| fail("--global-res needs an integer")),
            "--class-source" => class_source = true,
            "--legacy-source" => legacy_source = true,
            "--faces" => {
                faces = value().split(',').map(|f| f.parse().ok().filter(|f| *f < 6).unwrap_or_else(|| fail("--faces needs a list of 0..5"))).collect()
            }
            other => fail(&format!(
                "unknown option '{other}' (known: --seed N, --planet-res N, --halo N, --out DIR, --threads N, --concurrency N, --map-width W, --faces LIST)"
            )),
        }
    }
    if concurrency != 1 {
        fail("--concurrency above 1 is not measured yet: faces run one at a time (the peak memory of a face is reported below)");
    }
    if res < 8 || width < 64 || width % 2 != 0 {
        fail("--planet-res must be at least 8 and --map-width an even number of at least 64");
    }
    if res > COARSE_LIMIT {
        fail(&format!("--planet-res above {COARSE_LIMIT} is larger than GEN-100's pass-1 grid"));
    }
    let (threads, threads_note) = planet_streaming::topology::bake_threads(threads).unwrap_or_else(|e| fail(&e));
    let out = out.unwrap_or_else(|| PathBuf::from(format!("target/bake/pass1-seed-{seed}-res-{res}")));
    let store_dir = out.join("layer");
    // The world follows the field source: the one the store at --global-dir records (a store of both is refused unless --class-source or --legacy-source says which), or the class source
    // with --class-source alone, else the legacy generator (STRM-073).
    let def = WorldDefinition::single_planet(RADIUS_M, seed);
    let choice = match (class_source, legacy_source) {
        (true, true) => fail("--class-source and --legacy-source exclude each other"),
        (true, false) => Some(FieldSource::Class),
        (false, true) => Some(FieldSource::Legacy),
        (false, false) => None,
    };
    let inputs = std::sync::Arc::new(BakeInputs::new(seed, threads));
    let world = match &global_dir {
        Some(dir) => GlobalWorld::for_store(&def, dir, global_res, choice, threads).unwrap_or_else(|e| fail(&e)),
        None if choice == Some(FieldSource::Class) => {
            GlobalWorld::from_definition(&def).unwrap_or_else(|e| fail(&format!("{e:?}"))).with_class_source(inputs.clone())
        }
        None => GlobalWorld::from_definition(&def).unwrap_or_else(|e| fail(&format!("{e:?}"))),
    };
    let base = |d: Vec3| inputs.base(d);
    let factor = |d: Vec3| inputs.carve_factor(d);
    let (rock_k, rock_u) = (|d: Vec3, z: f64| inputs.rock_k(d, z), |d: Vec3| inputs.uplift_at(d));
    let rock = planet_generators::bake::face::RockFields { k: Some(&rock_k), uplift: Some(&rock_u) };
    let halo = halo.unwrap_or(res.div_ceil(128).max(4).min((res - 1) / 2));
    let spec = LayerSpec {
        cells: res,
        radius_m: RADIUS_M,
        sea_level: 0.0,
        seed,
        carve: CarveParams::default(),
        inputs_id: BakeInputs::id(),
        uses_factor: true,
        rock: 3,
        halo,
    };
    let bake_res = res as u32;
    println!(
        "pass 1: seed {seed}, {res} cells per face edge ({:.0} m), halo {halo} cells, faces {faces:?}, one at a time on {threads} threads ({threads_note})",
        RADIUS_M * core::f64::consts::FRAC_PI_2 / res as f64
    );
    let started = Instant::now();
    for &f in &faces {
        let t = Instant::now();
        let report = run_pass1_rock(&store_dir, &world, bake_res, &spec, &base, Some(&factor), &rock, &[Face(f)], threads)
            .unwrap_or_else(|e| fail(&e.to_string()));
        let what = if report.computed.is_empty() { "stored (skipped)" } else { "computed" };
        println!(
            "face {f}: {what} in {:.1} s, peak memory so far {} MiB",
            t.elapsed().as_secs_f64(),
            peak_memory_bytes().map_or(0, |b| b >> 20)
        );
    }
    println!(
        "pass 1: {:.1} s, peak memory {} MiB (the process's peak over all faces)",
        started.elapsed().as_secs_f64(),
        peak_memory_bytes().map_or(0, |b| b >> 20)
    );
    if faces.len() == 6 {
        let store = TileStore::open(&store_dir, world.params()).unwrap_or_else(|e| fail(&e.to_string()));
        let changes: Vec<FaceChange> = (0..6)
            .map(|f| {
                load_face_change(&store, &world, bake_res, &spec, Face(f))
                    .unwrap_or_else(|e| fail(&e.to_string()))
                    .unwrap_or_else(|| fail("a face is missing"))
            })
            .collect();
        let changes: [FaceChange; 6] = changes.try_into().expect("six faces");
        edge_report(&changes);
        let t = Instant::now();
        for (name, image) in pass1_maps(width, seed, &base, &changes) {
            let path = out.join(format!("pass1-{name}.png"));
            image.write_png(&path).unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", path.display())));
            println!("wrote {}", path.display());
        }
        println!("maps {:.1} s", t.elapsed().as_secs_f64());
    } else {
        println!("maps skipped: they need all six faces");
    }
}

fn fail(message: &str) -> ! {
    eprintln!("bake pass1: {message}");
    std::process::exit(2);
}

/// The process's peak working set in bytes (Windows: `PeakWorkingSetSize`; Linux: `VmHWM`).
#[cfg(windows)]
fn peak_memory_bytes() -> Option<u64> {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        rest: [usize; 7],
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: core::mem::size_of::<Counters>() as u32, page_fault_count: 0, peak_working_set: 0, rest: [0; 7] };
    // SAFETY: `c` is a writable PROCESS_MEMORY_COUNTERS of the size it declares; the pseudo handle needs no closing.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (ok != 0).then_some(c.peak_working_set as u64)
}

#[cfg(not(windows))]
fn peak_memory_bytes() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb << 10)
}

/// The step of the change across the cube edges against the step between neighbouring cells inside a face (the halo and the cross-fade of the reader keep it small).
fn edge_report(changes: &[FaceChange; 6]) {
    let ((seam_max, seam_p99), (inner_max, inner_p99)) = edge_steps(changes);
    println!("cube edges: the change steps by at most {seam_max:.1} m (99 % under {seam_p99:.1} m) from the cell at an edge to the cell across it; between neighbouring cells inside a face at most {inner_max:.1} m (99 % under {inner_p99:.1} m)");
}
