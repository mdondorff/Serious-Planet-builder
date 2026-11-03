//! `cargo xtask bake layer ...` runs this (m9-bake PR 5, GEN-100, GEN-101): pass 1 of the erosion layer over the cube faces at `--face-res` cells per face edge, then pass 2 on the tiles of
//! the `--planet-res` grid (every land tile, or the tiles of a `--region` file), checkpointed per face and per tile (a killed run resumes), then the preview maps and a report of the
//! tiles' statistics. Pass 1 is dropped after a complete planet pass unless `--keep-pass1` is given. Reads no other store. Writes only under `--out`
//! (default `target/bake/layer-seed-<N>`). A full-resolution planet pass is not run without the planner's go: `--planet-res` above 12,288 takes a `--region` of at most 64 tiles.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_store::{load_face_change, run_pass1, LayerSpec};
use planet_cache::layer_tiles::{drop_pass1, face_change_reader, faces_digest, load_tile, run_pass2, Pass2Inputs, TileLayerSpec};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::FaceChange;
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::map::{field_maps, layer_change_at, tile_image};
use planet_generators::bake::tile::{tile_has_land, Pass2Params, TileGrid, TileId as PassTile};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
/// The largest `--planet-res` of a planet pass and of `--face-res` a run takes without the planner's go.
const PLANET_LIMIT: usize = 12_288;
const FACE_LIMIT: usize = 4096;
const REGION_LIMIT: usize = 64;
/// The largest tile (core cells a side) a run takes: its window and the arrays of its kernels are `(tile + 2 halo)^2` cells of several f64 each.
const TILE_LIMIT: usize = 4096;

struct Opts {
    seed: u64,
    face_res: usize,
    planet_res: usize,
    tile: Option<usize>,
    halo: Option<usize>,
    out: Option<PathBuf>,
    threads: usize,
    region: Option<PathBuf>,
    keep_pass1: bool,
    map_width: usize,
    image_step: Option<usize>,
}

fn main() {
    let o = parse();
    let threads = if o.threads == 0 { std::thread::available_parallelism().map_or(1, |n| n.get()) } else { o.threads };
    let out = o.out.clone().unwrap_or_else(|| PathBuf::from(format!("target/bake/layer-seed-{}", o.seed)));
    let tile = o.tile.unwrap_or(o.planet_res.min(3072));
    let halo = o.halo.unwrap_or(tile / 12);
    let grid = TileGrid { cells: o.planet_res, tile, halo, radius_m: RADIUS_M };
    if let Err(e) = grid.check() {
        fail(&e);
    }
    if !grid.tiles_per_edge().is_power_of_two() {
        fail(&format!(
            "{} tiles a face edge is not a power of two ({} cells a face edge, {tile} a tile)",
            grid.tiles_per_edge(),
            o.planet_res
        ));
    }
    if tile > TILE_LIMIT {
        fail(&format!("--tile above {TILE_LIMIT} cells builds a window of several GB: refused"));
    }
    if o.face_res < 8 || o.face_res > FACE_LIMIT {
        fail(&format!("--face-res must be 8 to {FACE_LIMIT} (a larger pass 1 is a full-resolution bake and needs the planner's go)"));
    }
    if o.map_width < 64 || !o.map_width.is_multiple_of(2) {
        fail("--map-width must be an even number of at least 64");
    }
    let tiles_wanted = o.region.as_ref().map(|p| read_region(p, &grid));
    match &tiles_wanted {
        Some(t) if t.len() > REGION_LIMIT => {
            fail(&format!("a region of {} tiles is over the {REGION_LIMIT} a run takes without the planner's go", t.len()))
        }
        None if o.planet_res > PLANET_LIMIT => {
            fail(&format!("--planet-res above {PLANET_LIMIT} without a --region is a full planet bake and needs the planner's go"))
        }
        _ => {}
    }
    let started = Instant::now();
    // A planet pass that was complete (its pass 1 dropped) has nothing to resume: pass 1 would be computed again only to find every tile stored.
    if tiles_wanted.is_none() && !out.join("layer").exists() && out.join("tiles").exists() {
        println!(
            "layer: {} holds a complete layer (pass 1 dropped); nothing to do (run with --keep-pass1 to keep pass 1 for iteration)",
            out.display()
        );
        return;
    }
    let world = GlobalWorld::from_definition(&WorldDefinition::single_planet(RADIUS_M, o.seed)).unwrap_or_else(|e| fail(&format!("{e:?}")));
    let inputs = BakeInputs::new(o.seed, threads);
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    let face_halo = o.face_res.div_ceil(128).max(4).min((o.face_res - 1) / 2);
    let layer = LayerSpec {
        cells: o.face_res,
        radius_m: RADIUS_M,
        sea_level: 0.0,
        seed: o.seed,
        carve: CarveParams::default(),
        inputs_id: BakeInputs::id(),
        uses_factor: true,
        halo: face_halo,
    };
    let (face_dir, tile_dir) = (out.join("layer"), out.join("tiles"));
    let face_bake_res = o.face_res as u32;
    println!(
        "layer: seed {}, pass 1 at {} cells a face edge (halo {face_halo}), pass 2 at {} cells ({:.0} m), tiles of {tile} with a halo of {halo}, {} threads",
        o.seed,
        o.face_res,
        o.planet_res,
        grid.dx(),
        threads
    );
    for f in 0..6u8 {
        let t = Instant::now();
        let r = run_pass1(&face_dir, &world, face_bake_res, &layer, &base, Some(&factor), &[Face(f)], threads)
            .unwrap_or_else(|e| fail(&e.to_string()));
        println!(
            "pass 1 face {f}: {} in {:.1} s, peak memory {} MiB",
            if r.computed.is_empty() { "stored" } else { "computed" },
            t.elapsed().as_secs_f64(),
            mib()
        );
    }
    let store = TileStore::open(&face_dir, world.params()).unwrap_or_else(|e| fail(&e.to_string()));
    let faces: Vec<FaceChange> = (0..6)
        .map(|f| {
            load_face_change(&store, &world, face_bake_res, &layer, Face(f))
                .unwrap_or_else(|e| fail(&e.to_string()))
                .unwrap_or_else(|| fail("a pass-1 face is missing"))
        })
        .collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    let reader = face_change_reader(&faces);
    let spec = TileLayerSpec {
        grid,
        params: Pass2Params::standard(o.seed, 0.0),
        inputs_id: BakeInputs::id(),
        uses_factor: true,
        faces: faces_digest(&world, face_bake_res, &layer),
    };
    let surface = |d: Vec3| base(d) + reader(d);
    let planet_pass = tiles_wanted.is_none();
    let tiles: Vec<PassTile> = tiles_wanted.unwrap_or_else(|| {
        // The look is at pass 1's own lattice (a tile with no land cell there has no pass-1 change to keep when pass 1 is dropped).
        let stride = (grid.cells / o.face_res).max(1);
        let all = grid.all_tiles();
        let land: Vec<PassTile> = all.iter().copied().filter(|&t| tile_has_land(&grid, t, &surface, 0.0, stride)).collect();
        println!("pass 2: {} land tiles of {} (looked at every {stride}th cell, pass 1's lattice)", land.len(), all.len());
        land
    });
    let mut report =
        String::from("face tile_i tile_j land_cells thermal_passes thermal_moving_at_cap window_eroded_m3 window_deposited_m3 window_exported_m3 clamped_cells seconds\n");
    let tile_res = o.planet_res as u32;
    let mut clock = Instant::now();
    let done = run_pass2(
        &tile_dir,
        &world,
        tile_res,
        &spec,
        &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor) },
        &tiles,
        threads,
        &mut |r| {
            let (s, secs) = (&r.stats, clock.elapsed().as_secs_f64());
            clock = Instant::now();
            println!(
                "pass 2 tile {}/{}/{}: {secs:.1} s, {} land cells, thermal {} passes with {} cells still moving, eroded {:.3e} m3 (deposited {:.1} %), peak memory {} MiB",
                r.id.face.0,
                r.id.ti,
                r.id.tj,
                s.land_cells,
                s.thermal_passes,
                s.thermal_moving_at_cap,
                s.eroded_m3,
                100.0 * s.deposited_m3 / s.eroded_m3.max(1.0),
                mib()
            );
            report.push_str(&format!(
                "{} {} {} {} {} {} {:.6e} {:.6e} {:.6e} {} {secs:.2}\n",
                r.id.face.0, r.id.ti, r.id.tj, s.land_cells, s.thermal_passes, s.thermal_moving_at_cap, s.eroded_m3, s.deposited_m3, s.exported_m3, s.clamped_cells
            ));
        },
    )
    .unwrap_or_else(|e| fail(&e.to_string()));
    println!("pass 2: {} tiles computed, {} stored (skipped)", done.computed.len(), done.skipped.len());
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&e.to_string()));
    std::fs::write(out.join("tiles.txt"), &report).unwrap_or_else(|e| fail(&e.to_string()));
    // The maps, from the stored layer.
    let tstore = TileStore::open(&tile_dir, world.params()).unwrap_or_else(|e| fail(&e.to_string()));
    let mut stored: HashMap<(u8, u32, u32), Vec<f32>> = HashMap::new();
    for &t in &tiles {
        let r = load_tile(&tstore, &world, tile_res, &spec, t)
            .unwrap_or_else(|e| fail(&e.to_string()))
            .unwrap_or_else(|| fail("a tile is missing from the store"));
        stored.insert((t.face.0, t.ti, t.tj), r.core);
    }
    if planet_pass {
        let t = Instant::now();
        for (name, image) in field_maps(o.map_width, o.seed, &base, &|d| layer_change_at(&grid, &stored, &faces, d), "BAKE LAYER", "LAYER")
        {
            let path = out.join(format!("layer-{name}.png"));
            image.write_png(&path).unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", path.display())));
            println!("wrote {}", path.display());
        }
        println!("maps {:.1} s", t.elapsed().as_secs_f64());
    } else {
        for &t in &tiles {
            let core = &stored[&(t.face.0, t.ti, t.tj)];
            let (i0, j0) = (t.ti as i64 * tile as i64, t.tj as i64 * tile as i64);
            let b: Vec<f64> =
                (0..tile * tile).map(|p| base(grid.direction(t.face, i0 + (p % tile) as i64, j0 + (p / tile) as i64))).collect();
            let path = out.join(format!("tile-{}-{}-{}.png", t.face.0, t.ti, t.tj));
            // A tile image takes every `step`th cell (default: down to about 1,024 cells a side), so a 3,072-cell tile is not a 110 MB picture.
            let step = o.image_step.unwrap_or(tile.div_ceil(1024)).max(1);
            let m = tile / step;
            let (bs, cs): (Vec<f64>, Vec<f32>) =
                (0..m * m).map(|p| (p / m) * step * tile + (p % m) * step).map(|q| (b[q], core[q])).unzip();
            tile_image(m, grid.dx() * step as f64, &bs, &cs, 400.0)
                .write_png(&path)
                .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", path.display())));
            println!("wrote {}", path.display());
        }
    }
    if planet_pass && !o.keep_pass1 {
        drop_pass1(&face_dir).unwrap_or_else(|e| fail(&e.to_string()));
        println!("pass 1 dropped ({} removed; --keep-pass1 keeps it)", face_dir.display());
    } else {
        println!("pass 1 kept at {} ({})", face_dir.display(), if planet_pass { "--keep-pass1" } else { "a region run" });
    }
    println!("layer: {:.1} s, peak memory {} MiB", started.elapsed().as_secs_f64(), mib());
}

fn parse() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Opts {
        seed: 14,
        face_res: 768,
        planet_res: 3072,
        tile: None,
        halo: None,
        out: None,
        threads: 0,
        region: None,
        keep_pass1: false,
        map_width: 1024,
        image_step: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        let mut number = |what: &str| value().parse::<usize>().unwrap_or_else(|_| fail(&format!("{what} needs an integer")));
        match a.as_str() {
            "--seed" => o.seed = value().parse().unwrap_or_else(|_| fail("--seed needs a non-negative integer")),
            "--face-res" => o.face_res = number("--face-res"),
            "--planet-res" => o.planet_res = number("--planet-res"),
            "--tile" => o.tile = Some(number("--tile")),
            "--halo" => o.halo = Some(number("--halo")),
            "--threads" => o.threads = number("--threads"),
            "--map-width" => o.map_width = number("--map-width"),
            "--out" => o.out = Some(PathBuf::from(value())),
            "--region" => o.region = Some(PathBuf::from(value())),
            "--image-step" => o.image_step = Some(number("--image-step")),
            "--keep-pass1" => o.keep_pass1 = true,
            other => fail(&format!(
                "unknown option '{other}' (known: --seed N, --face-res N, --planet-res N, --tile N, --halo N, --region FILE, --keep-pass1, --out DIR, --threads N, --map-width W)"
            )),
        }
    }
    o
}

/// The tiles of a region file: one `face ti tj` a line, `#` starts a comment.
fn read_region(path: &std::path::Path, g: &TileGrid) -> Vec<PassTile> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("cannot read the region {}: {e}", path.display())));
    let n = g.tiles_per_edge() as u32;
    let mut tiles = Vec::new();
    for (k, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let v: Vec<u32> = line
            .split_whitespace()
            .map(|w| w.parse().unwrap_or_else(|_| fail(&format!("{}:{}: '{w}' is not a number", path.display(), k + 1))))
            .collect();
        if v.len() != 3 || v[0] > 5 || v[1] >= n || v[2] >= n {
            fail(&format!(
                "{}:{}: expected 'face tile_i tile_j' with face 0..5 and tiles 0..{} on this grid, got '{line}'",
                path.display(),
                k + 1,
                n - 1
            ));
        }
        tiles.push(PassTile { face: Face(v[0] as u8), ti: v[1], tj: v[2] });
    }
    if tiles.is_empty() {
        fail(&format!("the region {} lists no tile", path.display()));
    }
    tiles
}

fn fail(message: &str) -> ! {
    eprintln!("bake layer: {message}");
    std::process::exit(2);
}

/// The process's peak working set in MiB.
fn mib() -> u64 {
    peak_memory_bytes().map_or(0, |b| b >> 20)
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
