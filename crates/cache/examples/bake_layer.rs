//! `cargo xtask bake layer ...` runs this (m9-bake PR 5, GEN-100, GEN-101): pass 1 of the erosion layer over the cube faces at `--face-res` cells per face edge, then pass 2 on the tiles of
//! the `--planet-res` grid (every land tile, or the tiles of a `--region` file), checkpointed per face and per tile (a killed run resumes), then the preview maps and a report of the
//! tiles' statistics. Pass 1 is dropped after a complete planet pass unless `--keep-pass1` is given. Reads no other store. Writes only under `--out`
//! (default `target/bake/layer-seed-<N>`). A full-resolution planet pass is not run without the planner's go: `--planet-res` above 12,288 takes a `--region` of at most 64 tiles.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use planet_cache::global_store::{FieldSource, GlobalWorld};
use planet_cache::layer_fill::load_face_fill;
use planet_cache::layer_manifest::LayerManifest;
use planet_cache::layer_store::{load_face_change, run_pass1};
use planet_cache::layer_tiles::{
    bake_specs, drop_pass1, face_change_reader, face_fill_reader, load_tile, run_pass2, BakeShape, Pass2Inputs,
};
use planet_cache::pack::TileStore;
use planet_core::{Face, Vec3};
use planet_generators::bake::face::{FaceChange, FaceFill};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::bake::map::{field_maps, layer_change_at, tile_image};
use planet_generators::bake::tile::{tile_has_land, TileGrid, TileId as PassTile};
use planet_world_def::WorldDefinition;

const RADIUS_M: f64 = 6_371_000.0;
/// The largest `--planet-res` of a planet pass and of `--face-res` a run takes without the planner's go.
const PLANET_LIMIT: usize = 12_288;
/// GEN-100's pass-1 grid (814 m, owner 2026-11-03): the default and the largest.
const FACE_LIMIT: usize = 12_288;
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
    /// `--global-dir DIR`: the stored field bake whose recorded source the layer follows; `--global-res` its cells a face edge.
    global_dir: Option<PathBuf>,
    global_res: u32,
    class_source: bool,
    legacy_source: bool,
    /// `--tile-dir DIR`: the directory of the layer's tile records (default `<out>/tiles`); the viewer reads the layer from the field store's own directory when it has no other, so a layer baked beside a store is written there.
    tile_dir: Option<PathBuf>,
    /// `--faces LIST` (a region run only): the cube faces whose pass 1 is computed; the other faces read as no change. For a preview whose tiles all lie well inside those faces (beyond the halo's cross-fade).
    faces: Option<Vec<u8>>,
    map_width: usize,
    image_step: Option<usize>,
}

fn main() {
    let o = parse();
    let (threads, threads_note) = planet_streaming::topology::bake_threads(o.threads).unwrap_or_else(|e| fail(&e));
    let out = o.out.clone().unwrap_or_else(|| PathBuf::from(format!("target/bake/layer-seed-{}", o.seed)));
    let tile = o.tile.unwrap_or(o.planet_res.min(3072));
    let halo = o.halo.unwrap_or(tile / 24);
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
        fail(&format!(
            "--face-res must be 8 to {FACE_LIMIT} (GEN-100's grid is {FACE_LIMIT}; a larger pass 1 is not a bake this command runs)"
        ));
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
    if tiles_wanted.is_none() && !out.join("layer").exists() && o.tile_dir.clone().unwrap_or_else(|| out.join("tiles")).exists() {
        println!(
            "layer: {} holds a complete layer (pass 1 dropped); nothing to do (run with --keep-pass1 to keep pass 1 for iteration)",
            out.display()
        );
        return;
    }
    // The world follows the field source: the one the store at --global-dir records (a store of both is refused unless --class-source or --legacy-source says which), or the class source with
    // --class-source alone, else the legacy generator. The layer's keys follow the world (STRM-073).
    let def = WorldDefinition::single_planet(RADIUS_M, o.seed);
    let choice = match (o.class_source, o.legacy_source) {
        (true, true) => fail("--class-source and --legacy-source exclude each other"),
        (true, false) => Some(FieldSource::Class),
        (false, true) => Some(FieldSource::Legacy),
        (false, false) => None,
    };
    let inputs = std::sync::Arc::new(BakeInputs::new(o.seed, threads));
    let world = match &o.global_dir {
        Some(dir) => GlobalWorld::for_store(&def, dir, o.global_res, choice, threads).unwrap_or_else(|e| fail(&e)),
        None if choice == Some(FieldSource::Class) => {
            GlobalWorld::from_definition(&def).unwrap_or_else(|e| fail(&format!("{e:?}"))).with_class_source(inputs.clone())
        }
        None => GlobalWorld::from_definition(&def).unwrap_or_else(|e| fail(&format!("{e:?}"))),
    };
    let (base, factor) = (|d: Vec3| inputs.base(d), |d: Vec3| inputs.carve_factor(d));
    // The specs of both passes come from the one derivation the viewer also uses (`bake_specs`), so a record's key cannot drift between the command and the reader.
    let shape = BakeShape { seed: o.seed, radius_m: RADIUS_M, face_res: o.face_res, planet_res: o.planet_res, tile: o.tile, halo: o.halo };
    let (layer, spec) = bake_specs(&world, &shape).unwrap_or_else(|e| fail(&e));
    let face_halo = layer.halo;
    let (face_dir, tile_dir) = (out.join("layer"), o.tile_dir.clone().unwrap_or_else(|| out.join("tiles")));
    let face_bake_res = o.face_res as u32;
    println!(
        "layer: seed {}, pass 1 at {} cells a face edge (halo {face_halo}), pass 2 at {} cells ({:.0} m), tiles of {tile} with a halo of {halo}, {} threads ({})",
        o.seed,
        o.face_res,
        o.planet_res,
        grid.dx(),
        threads,
        threads_note
    );
    // A region run may compute pass 1 on some faces only (`--faces`): the others read as no change, which is exact for a tile whose window stays clear of the cube edges (the halo's cross-fade, GEN-100).
    let computed_faces: Vec<u8> = match &o.faces {
        Some(list) if tiles_wanted.is_some() => list.clone(),
        Some(_) => fail("--faces applies to a --region run only: a planet pass computes every face"),
        None => (0..6).collect(),
    };
    if computed_faces.len() < 6 {
        println!("pass 1 on faces {computed_faces:?} only: the other faces read as no change; the tiles of this run must lie beyond the halo's cross-fade of the cube edges");
    }
    for &f in &computed_faces {
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
    // The drainage areas are part of the layer: copy them beside the tiles before the pass-1 store can be dropped.
    let copied = planet_cache::layer_channels::copy_areas(
        &face_dir,
        &tile_dir,
        &world,
        face_bake_res,
        &layer,
        &computed_faces.iter().map(|&f| Face(f)).collect::<Vec<_>>(),
    )
    .unwrap_or_else(|e| fail(&e.to_string()));
    println!("drainage area: {} faces copied to the layer store", copied.len());
    let store = TileStore::open(&face_dir, world.params()).unwrap_or_else(|e| fail(&e.to_string()));
    let faces: Vec<FaceChange> = (0..6u8)
        .map(|f| {
            if !computed_faces.contains(&f) {
                let n = layer.cells + 2 * layer.halo;
                return FaceChange { face: Face(f), cells: layer.cells, halo: layer.halo, change: vec![0.0; n * n] };
            }
            load_face_change(&store, &world, face_bake_res, &layer, Face(f))
                .unwrap_or_else(|e| fail(&e.to_string()))
                .unwrap_or_else(|| fail("a pass-1 face is missing"))
        })
        .collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    let reader = face_change_reader(&faces);
    // Pass 1's pit depth (GEN-120): pass 2 reads it for the basins its windows cut; a face this run did not compute has no pits.
    let fills: Vec<FaceFill> = (0..6u8)
        .map(|f| {
            if !computed_faces.contains(&f) {
                return FaceFill { face: Face(f), cells: layer.cells, codes: vec![0; layer.cells * layer.cells] };
            }
            load_face_fill(&store, &world, face_bake_res, &layer, Face(f))
                .unwrap_or_else(|e| fail(&e.to_string()))
                .unwrap_or_else(|| fail("a pass-1 face holds no pit depth"))
        })
        .collect();
    let fills: [FaceFill; 6] = fills.try_into().expect("six faces");
    let fill_reader = face_fill_reader(&fills);
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
        &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor), face_fill: Some(&fill_reader) },
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
        let mut r = load_tile(&tstore, &world, tile_res, &spec, t)
            .unwrap_or_else(|e| fail(&e.to_string()))
            .unwrap_or_else(|| fail("a tile is missing from the store"));
        // The maps and the manifest's range are of what the runtime reads: the change with the pit depth added back (GEN-120).
        if let Some(codes) =
            planet_cache::layer_fill::load_fill(&tstore, &world, tile_res, &spec, t).unwrap_or_else(|e| fail(&e.to_string()))
        {
            r.fill = codes;
            let pits = r.fill.iter().filter(|c| **c > 0).count();
            let land = r.stats.land_cells.max(1);
            println!(
                "pits tile {}/{}/{}: {pits} cells ({:.1} % of {land} land), deepest {} m",
                t.face.0,
                t.ti,
                t.tj,
                100.0 * pits as f64 / land as f64,
                r.fill.iter().max().copied().unwrap_or(0)
            );
            r = r.with_fill_added();
        }
        stored.insert((t.face.0, t.ti, t.tj), r.core);
    }
    // The manifest (STRM-074): the shape, versions and source, and the range of the change over the tiles of this run added to what the directory already says (the range only widens).
    let mut manifest = LayerManifest::new(&world, &shape).unwrap_or_else(|e| fail(&e));
    for core in stored.values() {
        let (lo, hi) = core.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), v| (l.min(*v), h.max(*v)));
        manifest.widen(f64::from(lo), f64::from(hi));
    }
    let written = manifest.merge_into(&tile_dir).unwrap_or_else(|e| fail(&e));
    let (lo, hi) = written.range();
    println!("layer manifest: {} (change range [{lo}, {hi}] m)", tile_dir.join(planet_cache::layer_manifest::MANIFEST_FILE).display());
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
        face_res: FACE_LIMIT,
        planet_res: 98_304,
        tile: None,
        halo: None,
        out: None,
        threads: 0,
        region: None,
        keep_pass1: false,
        global_dir: None,
        global_res: 16_384,
        class_source: false,
        legacy_source: false,
        tile_dir: None,
        faces: None,
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
            "--global-dir" => o.global_dir = Some(PathBuf::from(value())),
            "--global-res" => o.global_res = number("--global-res") as u32,
            "--class-source" => o.class_source = true,
            "--legacy-source" => o.legacy_source = true,
            "--tile-dir" => o.tile_dir = Some(PathBuf::from(value())),
            "--faces" => {
                o.faces = Some(
                    value().split(',').map(|f| f.parse().ok().filter(|f| *f < 6).unwrap_or_else(|| fail("--faces needs a list of 0..5"))).collect(),
                )
            }
            other => fail(&format!(
                "unknown option '{other}' (known: --faces LIST, --tile-dir DIR, --seed N, --face-res N, --planet-res N, --tile N, --halo N, --region FILE, --keep-pass1, --out DIR, --threads N, --map-width W)"
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
