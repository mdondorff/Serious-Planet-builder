//! The single binary's run modes (CON-01): `editor` (interactive viewer), `generate` (headless CLI)
//! and `test-render` (headless harness). M0 only wires them up; real work arrives with M1+.

pub mod globalperf;
pub mod litperf;
pub mod lookload;
pub mod manifest;
pub mod pipelined;
mod plancheck;
pub mod prefill;
pub mod sheet;
pub mod skeleton;
pub mod streamperf;
pub mod watergap;
pub mod window;
pub mod worldfield;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use planet_perf::{AdapterFacts, DeviceKind, Protocol, RunReport};
use planet_render::{AdapterPolicy, GpuContext};

pub const USAGE: &str = "usage: planet <mode> [options]
modes:
  editor        the viewer window (VIEW-001..VIEW-007): the scripted orbit-to-1 m flight on the pipelined loop; keys P/Space pause, R restart, L lit (with the atmosphere) or debug path, V debug view, T vsync, C flight or free camera (WASD move, Q/E down/up, arrows look, Shift boost), Esc quit; the window opens on the lit path with the atmosphere, the look file drawn (sun, adaptive exposure); the title shows frame rate, CPU and GPU time, the path (or the debug view), on the lit path the exposure, and the readiness report; runs on a discrete GPU only (an integrated GPU, a software adapter or an adapter missing a feature or limit is refused with the reason); [--world F --global-res N | --store DIR] [--look F] [--size WxH] [--view NAME] [--cells N --tau PX --finest-level N] [--no-vsync]; --smoke opens the window, drives the keys for a few dozen frames and closes it (test mode: PLANET_ADAPTER or --adapter, software by default; exits with `no display` where there is none); --no-window starts and exits without a window; --offscreen runs the skeleton without a window
  generate      headless generator CLI: --tile F,L,X,Y [--seed N --res N --out height.png --face-net net.png]; --manifest <file> writes the determinism manifest (GEN-003); --cache-dir <dir> --region F,L [--res N] writes a tile face region into a tile store; --verify-cache <dir> [--sample N] regenerates stored tiles and compares (both take --world <definition.toml> or --seed N; --verify-cache fails when nothing could be compared, --allow-stale accepts that); --global-dir <dir> [--global-res N, default 256] bakes the global-coarse field (continents, elevation, climate) of N cells per face edge into a tile store as blocks in the canonical layout (blocks of min(N, 1024) cells) together with its ocean-connectivity channel (one bit per vertex; the area threshold is the definition's `body.0.sea.min_area_fraction`) and prints the time per sample and the channel's cost; --verify-global <dir> [--sample N] regenerates stored blocks and compares; --global-products <dir> [--global-res N] writes cube nets and maps of an in-memory bake (these take --world or --seed and --threads N); --plan-check <dir> --global-res N (required) plans the standard flight at cells 32 and viewport / tau 240 over the stored bake with the terrain and the eroded field and fails if a view is rejected (M3 acceptance); --prefill-dir <dir> --global-res N [--prefill-levels L] [--floor-level F] writes the coarse tiles of levels F..=L (F the render floor, default 3; L default 4; an L below F writes nothing and says so; the tiles of cells 32, finest level 22, compact form, as the streamer generates them) of the stored bake into its tile pack, skipping what is there; --global-dir pre-fills the same way after the bake (--prefill-levels 0 disables; --floor-level takes both); --smoke starts and exits
  test-render   headless render harness
    --scene hello-triangle|tiles|terrain   render a scene (default hello-triangle)
    --script N|name          scripted terrain camera 0..9 or its name (default 3 = aerial-20km)
    --cells N --tau PX --frames N   terrain perf workload parameters (defaults 32, 6, 300)
    --floor-level N          the render floor (LOD-011, default 3): levels below it are not drawn, its 384 tiles at level 3 are the roots; test-render's terrain, streaming and lit scenes and generate's pre-fill take it, the viewer takes no flag; above --finest-level it is refused (the effective floor is the lower of the two)
    --finest-level N         the LOD cap, the finest geometry level of the single views, --stream, --pipelined, --flight and --perf (default 27 - ceil(log2(cells)): 22 at cells 32, 21 at 64, equal vertex spacing of about 7.5 cm; 26 reproduces the SPIKE-07 reference)
    --perf-smoke             run the terrain perf path briefly on any adapter (labelled, never a measurement)
    --flight N --out-dir DIR [--size WxH]   render the scripted orbit-to-1 m flight (TEST-013) as DIR/frame_0000.png... plus DIR/flight.txt and DIR/contact_sheet.png
                             with the production LOD defaults (override with --cells, --tau; --view applies; default size 1280x720,
                             small sizes can fail LOD validation) for the real-GPU jitter review
    --stream                 generate terrain tiles asynchronously through the streaming path (same image as the default)
    --pipelined [--then SCRIPT]   draw the terrain view through the pipelined loop (plans from a worker, a cover plan until the first one arrives,
                             the GPU never waited for; STRM-016): from a cold start, then (with --then) after a jump to the other camera; the
                             image is the one frame the loop settles on, equal to the default path's; --out-dir DIR also writes pipelined.txt
                             (per-frame kind, counts and the time columns plan_ms scheduler_ms upload_ms record_ms gpu_ms wall_ms)
    --lockstep               with --pipelined: wait for the plan worker and the generation after every iteration, so the frames and the tiles each
                             uploads do not depend on thread timing (a test mode; slower than the free-running loop)
    --view face|tile-id|level|morph|height|normals|depth|relief|morph-height   debug view (tiles scene: face, tile-id, height; a flight defaults to relief)
    --tile FACE,LEVEL,X,Y    tile to draw (repeatable; default: a tile and its east neighbour)
    --seed N --res N         generator seed and tile resolution (cells, power of two)
    --look <look.toml>       the presentation parameters per body (LOOK-001..005; exposure, scene units, histogram, tone curve; the file is outside the world definition and every cache key, so it never changes a tile): default the `<world stem>.look.toml` beside --world when it exists, else the Earth defaults; a bad file is refused naming the key and the value
    --world <definition.toml> [--global-res N]   terrain scene (a view, --stream, --bundle, --flight, --perf, --perf-smoke): the world's height field (world.height_field, default the eroded field) over an in-memory bake of N cells per face edge (default 256, at most 1024); without it a view, stream or bundle is the seed-only reference field and --perf is the default world of --seed; a bundle records the world, --repro reads it back
    --bundle <file>          write a repro bundle of the render
    --repro <file>           replay a repro bundle (see cargo xtask repro)
    --out <file.png>         write the image
    --adapter software|hardware   adapter policy (default: PLANET_ADAPTER or software)
    --draw per-draw|indirect   how the terrain renderer submits draws (REND-015): per-draw is the reference loop, indirect one multi-draw per buffer run from a CPU-built argument buffer (default indirect where the adapter supports it, else per-draw)
    --upload per-slot|batched   how a frame's tile writes reach the queue (REND-016): batched (default) writes each run of adjacent slots with one call, per-slot is the reference (one or two calls per tile); the pool contents are identical
    --gen-workers N          threads of the tile generation pool and so jobs running at once in the pipelined loop and the streaming perf scenes (STRM-020; default: the machine's hardware threads; the plan worker's threads are separate)
    --gen-reserve N          hardware threads the default generation pool leaves free for the frame thread, the plan worker and the driver (STRM-020; default 2; ignored when --gen-workers is given)
    --gen-ahead N            jobs queued behind the running ones so a worker that finishes between two frames takes the next at once (STRM-020; default: the worker count; 0: refill at frame boundaries only)
    --gen-priority below-normal|normal   OS priority of the generation workers (STRM-020, ADR 0046; default below-normal: the frame thread wins a core both want; no result depends on it)
    --tile-format vertex|compact   how terrain tiles are built and held (ADR 0033): compact (default) for levels 3 and finer, vertex the 44-byte reference format for all
    --lit                    the streaming perf scenes on the lit path with the atmosphere and water (TEST-025): same scripts, same verdicts, logs named <scene>-lit
    --views a,b,c            only these views of the lit perf scenes (ground-1m, aerial-20km, coast-2km, coast-3m, orbit, coast-glide, coast-descent); a partial log is not a window
    --scene capture-flights  the two coastal flights as 1440p PNG sequences (shore_filter at the look default, 0, and then 1) in --out-dir, never a measurement (TEST-027; `cargo xtask capture flights` also makes the videos); needs --store DIR or --allow-memory-bake, takes --size WxH, --flight-frames N
    --flight-frames N        frames of each coastal flight of the lit perf scenes (default 240; --scene lit | lit-formats, TEST-025)
    --perf                   performance-harness skeleton (needs --adapter hardware; the readiness gate decides, ADR 0020)
    --calibrate              measure the CPU calibration in a smoke run too (TEST-039; a measured run always does): proves the logging on any adapter, the log is still no measurement
    --machine-ready          owner override of a failed readiness gate (ADR 0020); recorded in the log
    --smoke                  start and exit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Generate,
    TestRender,
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub smoke: bool,
    pub scene: String,
    pub out: Option<PathBuf>,
    pub adapter: Option<AdapterPolicy>,
    /// `--draw per-draw|indirect`: how the terrain renderer submits draws (REND-015); default indirect where supported.
    pub draw: Option<planet_render::DrawMode>,
    /// `--tile-format vertex|compact`: how terrain tiles are built and held (ADR 0033); default compact, the vertex format is the reference.
    pub tile_format: Option<planet_render::TileFormat>,
    /// `--upload per-slot|batched`: how a frame's tile writes reach the queue (REND-016); default batched, per-slot the reference.
    pub upload: Option<planet_render::UploadMode>,
    /// `--gen-workers N` and `--gen-ahead N`: the tile generation pool (STRM-020); default the machine's threads and as many queued.
    pub gen_workers: Option<usize>,
    pub gen_ahead: Option<usize>,
    /// `--gen-reserve N`: cores the default pool size leaves free (`planet_streaming::GEN_RESERVE` when absent).
    pub gen_reserve: Option<usize>,
    /// `--gen-priority below-normal|normal`: the priority of the generation workers (STRM-020, ADR 0046); below-normal when absent.
    pub gen_priority: Option<planet_streaming::Priority>,
    pub perf: bool,
    pub machine_ready: bool,
    pub view: String,
    pub tiles: Vec<String>,
    pub seed: u64,
    pub res: u32,
    pub bundle_out: Option<PathBuf>,
    pub repro: Option<PathBuf>,
    pub face_net: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    /// A definition file (TOML, WDEF-002) instead of the default single planet with `--seed`.
    pub world: Option<PathBuf>,
    /// `--look F`: the look file (LOOK-001); without it the file beside `--world`, else the Earth defaults (LOOK-005).
    pub look: Option<PathBuf>,
    /// `generate`: write the tiles of `--region` into the tile store in this directory (ADR 0019).
    pub cache_dir: Option<PathBuf>,
    /// `generate --cache-dir`: all tiles of one face at one level, `FACE,LEVEL`.
    pub region: Option<(u8, u8)>,
    /// `generate`: regenerate a sample of the tile store in this directory and compare (STRM-004).
    pub verify_cache: Option<PathBuf>,
    /// `generate`: compact the store in this directory (STRM-013), dropping records the definition no longer produces.
    pub compact_cache: Option<PathBuf>,
    /// `--compact-cache`: keep at most this many bytes of records (the oldest-written packs are evicted first, ADR 0029).
    pub cap_bytes: Option<u64>,
    /// `--verify-cache`: how many evenly spaced entries to check; 0 is all.
    pub sample: usize,
    /// `--verify-cache`: entries that are all stale are expected (the definition changed on purpose), not a failure.
    pub allow_stale: bool,
    /// `generate`: bake the global-coarse field into a tile store in this directory as blocks (STRM-012).
    pub global_dir: Option<PathBuf>,
    /// `generate`: regenerate a sample of the global bake stored in this directory and compare.
    pub verify_global: Option<PathBuf>,
    /// `generate`: plan the standard flight at production parameters over the stored global bake in this directory.
    pub plan_check: Option<PathBuf>,
    /// `generate`: write the coarse tiles (levels `F..=--prefill-levels`, F the render floor) into the pack of the stored bake in this directory (STRM-021).
    pub prefill_dir: Option<PathBuf>,
    /// `--global-dir` and `--prefill-dir`: how many levels of coarse tiles the pre-fill writes, 0 for none (default 4, ADR 0045 point 3).
    pub prefill_levels: Option<u8>,
    /// `generate`: write the data products (cube nets and maps) of an in-memory bake into this directory.
    pub global_products: Option<PathBuf>,
    /// `--global-dir` and `--global-products`: cells per face edge of the bake (a power of two; blocks are stored in the canonical layout, ADR 0027).
    pub global_res: u32,
    /// `--global-dir`: worker threads (0 = all cores).
    pub threads: usize,
    pub offscreen: bool,
    pub script: String,
    pub stream: bool,
    /// `--pipelined`: the terrain view through the pipelined loop (STRM-016); `--then SCRIPT` jumps to another camera afterwards.
    pub pipelined: bool,
    pub then: Option<String>,
    /// `--lockstep` (with `--pipelined`): after every iteration wait until the plan worker and the generation are idle, so the
    /// frames drawn and the tiles each one uploads are a function of the script alone, not of thread timing (TEST-012).
    pub lockstep: bool,
    pub cells: u32,
    /// `--finest-level N`: the LOD cap (`LodParams::max_level`); none = the production default for the tile size (LOD-010).
    pub finest_level: Option<u8>,
    /// `--floor-level N`: the render floor (`LodParams::min_level`, LOD-011); none = the production default
    /// ([`planet_frame::DEFAULT_RENDER_FLOOR`], 3). Taken by `test-render` and `generate` (the pre-fill), never by the viewer.
    pub floor_level: Option<u8>,
    pub tau: f64,
    pub frames: usize,
    pub perf_smoke: bool,
    /// `--lit`: the streaming perf scenes draw the lit path with the atmosphere and the water (TEST-025).
    pub lit: bool,
    /// `--flight-frames N`: frames of each coastal flight of the lit perf scenes.
    pub flight_frames: usize,
    /// `--views a,b,c`: only these views of the lit perf scenes.
    pub views: Option<Vec<String>>,
    /// `--calibrate`: measure the CPU calibration in a smoke run too (TEST-039).
    pub calibrate: bool,
    /// `--store DIR`: the stored bake the streaming perf scenes read (the production source, GEN-014); `--allow-memory-bake` runs them over the in-memory bake of `--global-res` cells instead, labelled and not valid for streaming budgets.
    pub store: Option<PathBuf>,
    pub allow_memory_bake: bool,
    pub flight: Option<usize>,
    pub out_dir: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    /// `editor --no-vsync`: start with vsync off (the `T` key toggles it).
    pub no_vsync: bool,
    /// `editor --no-window`: do not open the window (the start check of CON-01 on a machine without a display).
    pub no_window: bool,
    /// Every option given on the command line, so modes can reject options they would otherwise ignore.
    pub given: std::collections::BTreeSet<String>,
}

impl Options {
    /// The tile generation pool of the pipelined loop and the streaming scenes (STRM-020): `--gen-workers` threads (default the
    /// machine's hardware threads less `--gen-reserve`, 2 unless given) with `--gen-ahead` jobs queued behind them (default as
    /// many).
    pub fn gen_workers(&self) -> planet_streaming::GenWorkers {
        let reserve = self.gen_reserve.unwrap_or(planet_streaming::GEN_RESERVE);
        let g =
            self.gen_workers.map_or_else(|| planet_streaming::GenWorkers::machine_reserving(reserve), planet_streaming::GenWorkers::new);
        let g = self.gen_priority.map_or(g, |p| g.with_priority(p));
        self.gen_ahead.map_or(g, |a| g.with_ahead(a))
    }

    /// The reserve behind the pool size, for the perf log: `None` when `--gen-workers` fixed the size and the reserve decided
    /// nothing.
    pub fn gen_reserve_used(&self) -> Option<usize> {
        self.gen_workers.is_none().then(|| self.gen_reserve.unwrap_or(planet_streaming::GEN_RESERVE))
    }

    /// The finest geometry level of a run with tiles of `cells` cells: `--finest-level`, else the production default
    /// (`planet_frame::production_finest_level`: 22 at cells 32, 21 at 64). The one place the apps take the LOD cap from (LOD-010).
    pub fn finest_level_for(&self, cells: u32) -> u8 {
        self.finest_level.unwrap_or_else(|| planet_frame::production_finest_level(cells))
    }

    /// The render floor asked for (`LodParams::min_level`): `--floor-level`, else the production default. The effective floor of a
    /// run is the lower of this and the finest level ([`Options::floor_for`], LOD-011).
    pub fn min_level(&self) -> u8 {
        self.floor_level.unwrap_or(planet_frame::DEFAULT_RENDER_FLOOR)
    }

    /// The effective render floor of a run with tiles of `cells` cells: `min(min_level, finest level)` (LOD-011), the level whose
    /// tiles are the roots and the first level the pre-fill writes.
    pub fn floor_for(&self, cells: u32) -> u8 {
        planet_frame::effective_floor(self.min_level(), self.finest_level_for(cells))
    }

    /// An explicit `--floor-level` above the finest level is refused, naming both (LOD-011): checked before any work.
    pub fn check_floor(&self) -> Result<(), String> {
        match self.floor_level {
            Some(f) => planet_frame::check_explicit_floor(f, self.finest_level_for(self.cells)),
            None => Ok(()),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut it = args.iter();
    let mode = match it.next().map(String::as_str) {
        Some("editor") | None => Mode::Editor,
        Some("generate") => Mode::Generate,
        Some("test-render") => Mode::TestRender,
        Some(other) => return Err(format!("unknown mode '{other}'\n{USAGE}")),
    };
    let mut o = Options {
        mode,
        smoke: false,
        scene: "hello-triangle".into(),
        out: None,
        adapter: None,
        draw: None,
        tile_format: None,
        upload: None,
        gen_workers: None,
        gen_priority: None,
        gen_ahead: None,
        gen_reserve: None,
        perf: false,
        machine_ready: false,
        view: "face".into(),
        tiles: vec![],
        seed: 1,
        res: 16,
        bundle_out: None,
        repro: None,
        face_net: None,
        manifest: None,
        world: None,
        look: None,
        cache_dir: None,
        region: None,
        verify_cache: None,
        compact_cache: None,
        cap_bytes: None,
        sample: 0,
        allow_stale: false,
        global_dir: None,
        verify_global: None,
        plan_check: None,
        prefill_dir: None,
        prefill_levels: None,
        global_products: None,
        global_res: 256,
        threads: 0,
        offscreen: false,
        script: "3".into(),
        stream: false,
        pipelined: false,
        lockstep: false,
        then: None,
        cells: 32,
        finest_level: None,
        floor_level: None,
        tau: 6.0,
        frames: 300,
        perf_smoke: false,
        lit: false,
        flight_frames: 240,
        views: None,
        calibrate: false,
        store: None,
        allow_memory_bake: false,
        flight: None,
        out_dir: None,
        size: None,
        no_vsync: false,
        no_window: false,
        given: Default::default(),
    };
    while let Some(a) = it.next() {
        o.given.insert(a.clone());
        match a.as_str() {
            "--smoke" => o.smoke = true,
            "--no-vsync" => o.no_vsync = true,
            "--no-window" => o.no_window = true,
            "--perf" => o.perf = true,
            "--offscreen" => o.offscreen = true,
            "--stream" => o.stream = true,
            "--pipelined" => o.pipelined = true,
            "--lockstep" => o.lockstep = true,
            "--then" => o.then = Some(it.next().ok_or("--then needs a script index or name")?.clone()),
            "--perf-smoke" => o.perf_smoke = true,
            "--lit" => o.lit = true,
            "--calibrate" => o.calibrate = true,
            "--views" => {
                o.views = Some(it.next().ok_or("--views needs a comma-separated list")?.split(',').map(|v| v.trim().to_string()).collect())
            }
            "--flight-frames" => {
                o.flight_frames = it.next().ok_or("--flight-frames needs a value")?.parse().map_err(|e| format!("--flight-frames: {e}"))?;
                if o.flight_frames < 2 {
                    return Err("--flight-frames must be at least 2".into());
                }
            }
            "--store" => o.store = Some(PathBuf::from(it.next().ok_or("--store needs the directory of a stored bake")?)),
            "--allow-memory-bake" => o.allow_memory_bake = true,
            "--cells" => o.cells = it.next().ok_or("--cells needs a value")?.parse().map_err(|e| format!("--cells: {e}"))?,
            "--tau" => o.tau = it.next().ok_or("--tau needs a value")?.parse().map_err(|e| format!("--tau: {e}"))?,
            "--finest-level" => {
                let level: u8 = it.next().ok_or("--finest-level needs a level")?.parse().map_err(|e| format!("--finest-level: {e}"))?;
                if level > planet_core::tile::MAX_LEVEL {
                    return Err(format!("--finest-level must be at most {}, got {level}", planet_core::tile::MAX_LEVEL));
                }
                o.finest_level = Some(level);
            }
            "--floor-level" => {
                let level: u8 = it.next().ok_or("--floor-level needs a level")?.parse().map_err(|e| format!("--floor-level: {e}"))?;
                if level > planet_core::tile::MAX_LEVEL {
                    return Err(format!("--floor-level must be at most {}, got {level}", planet_core::tile::MAX_LEVEL));
                }
                o.floor_level = Some(level);
            }
            "--frames" => o.frames = it.next().ok_or("--frames needs a value")?.parse().map_err(|e| format!("--frames: {e}"))?,
            "--flight" => o.flight = Some(it.next().ok_or("--flight needs a frame count")?.parse().map_err(|e| format!("--flight: {e}"))?),
            "--out-dir" => o.out_dir = Some(PathBuf::from(it.next().ok_or("--out-dir needs a path")?)),
            "--size" => o.size = Some(parse_size(it.next().ok_or("--size needs WxH")?)?),
            "--script" => o.script = it.next().ok_or("--script needs an index or name")?.clone(),
            "--view" => o.view = it.next().ok_or("--view needs a value")?.clone(),
            "--tile" => o.tiles.push(it.next().ok_or("--tile needs FACE,LEVEL,X,Y")?.clone()),
            "--seed" => o.seed = it.next().ok_or("--seed needs a value")?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--res" => {
                o.res = it.next().ok_or("--res needs a value")?.parse().map_err(|e| format!("--res: {e}"))?;
                if !o.res.is_power_of_two() || o.res > planet_render::exec::MAX_TILE_RES {
                    return Err(format!("--res must be a power of two up to {}, got {}", planet_render::exec::MAX_TILE_RES, o.res));
                }
            }
            "--bundle" => o.bundle_out = Some(PathBuf::from(it.next().ok_or("--bundle needs a path")?)),
            "--repro" => o.repro = Some(PathBuf::from(it.next().ok_or("--repro needs a path")?)),
            "--world" => o.world = Some(PathBuf::from(it.next().ok_or("--world needs a path to a definition file")?)),
            "--look" => o.look = Some(PathBuf::from(it.next().ok_or("--look needs a path to a look file")?)),
            "--cache-dir" => o.cache_dir = Some(PathBuf::from(it.next().ok_or("--cache-dir needs a directory")?)),
            "--verify-cache" => o.verify_cache = Some(PathBuf::from(it.next().ok_or("--verify-cache needs a directory")?)),
            "--compact-cache" => o.compact_cache = Some(PathBuf::from(it.next().ok_or("--compact-cache needs a directory")?)),
            "--cap-bytes" => {
                o.cap_bytes = Some(it.next().ok_or("--cap-bytes needs a byte count")?.parse().map_err(|e| format!("--cap-bytes: {e}"))?)
            }
            "--allow-stale" => o.allow_stale = true,
            "--global-dir" => o.global_dir = Some(PathBuf::from(it.next().ok_or("--global-dir needs a directory")?)),
            "--plan-check" => o.plan_check = Some(PathBuf::from(it.next().ok_or("--plan-check needs a directory")?)),
            "--prefill-dir" => o.prefill_dir = Some(PathBuf::from(it.next().ok_or("--prefill-dir needs a directory")?)),
            "--prefill-levels" => {
                let v: u8 = it.next().ok_or("--prefill-levels needs a count")?.parse().map_err(|e| format!("--prefill-levels: {e}"))?;
                if v > planet_cache::meshpack::MAX_PREFILL_LEVELS {
                    return Err(format!("--prefill-levels {v}: at most {} (0 disables)", planet_cache::meshpack::MAX_PREFILL_LEVELS));
                }
                o.prefill_levels = Some(v);
            }
            "--verify-global" => o.verify_global = Some(PathBuf::from(it.next().ok_or("--verify-global needs a directory")?)),
            "--global-products" => o.global_products = Some(PathBuf::from(it.next().ok_or("--global-products needs a directory")?)),
            "--global-res" => {
                o.global_res = it.next().ok_or("--global-res needs a value")?.parse().map_err(|e| format!("--global-res: {e}"))?
            }
            "--threads" => o.threads = it.next().ok_or("--threads needs a count")?.parse().map_err(|e| format!("--threads: {e}"))?,
            "--sample" => o.sample = it.next().ok_or("--sample needs a count")?.parse().map_err(|e| format!("--sample: {e}"))?,
            "--region" => {
                let v = it.next().ok_or("--region needs FACE,LEVEL")?;
                let bad = || format!("--region must be FACE,LEVEL with face 0..=5 and level 0..={MAX_REGION_LEVEL}, got '{v}'");
                let (f, l) = v.split_once(',').ok_or_else(bad)?;
                let (f, l): (u8, u8) = (f.parse().map_err(|_| bad())?, l.parse().map_err(|_| bad())?);
                if f > 5 || l > MAX_REGION_LEVEL {
                    return Err(bad());
                }
                o.region = Some((f, l));
            }
            "--manifest" => o.manifest = Some(PathBuf::from(it.next().ok_or("--manifest needs a path")?)),
            "--face-net" => o.face_net = Some(PathBuf::from(it.next().ok_or("--face-net needs a path")?)),
            "--machine-ready" => o.machine_ready = true,
            "--scene" => o.scene = it.next().ok_or("--scene needs a value")?.clone(),
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--tile-format" => {
                o.tile_format = Some(planet_render::TileFormat::parse(it.next().ok_or("--tile-format needs vertex or compact")?)?)
            }
            "--upload" => {
                let v = it.next().ok_or("--upload needs per-slot or batched")?;
                o.upload =
                    Some(planet_render::UploadMode::parse(v).ok_or_else(|| format!("--upload must be per-slot or batched, got '{v}'"))?)
            }
            "--gen-workers" => {
                let v = it.next().ok_or("--gen-workers needs a thread count")?;
                o.gen_workers = Some(
                    v.parse::<usize>()
                        .ok()
                        .filter(|n| *n >= 1)
                        .ok_or_else(|| format!("--gen-workers must be a positive integer, got '{v}'"))?,
                )
            }
            "--gen-reserve" => {
                let v = it.next().ok_or("--gen-reserve needs a count")?;
                o.gen_reserve = Some(v.parse::<usize>().map_err(|_| format!("--gen-reserve must be a non-negative integer, got '{v}'"))?)
            }
            "--gen-priority" => {
                let v = it.next().ok_or("--gen-priority needs a value")?;
                o.gen_priority = Some(
                    planet_streaming::Priority::parse(v)
                        .ok_or_else(|| format!("--gen-priority must be below-normal or normal, got '{v}'"))?,
                )
            }
            "--gen-ahead" => {
                let v = it.next().ok_or("--gen-ahead needs a count")?;
                o.gen_ahead = Some(v.parse::<usize>().map_err(|_| format!("--gen-ahead must be a non-negative integer, got '{v}'"))?)
            }
            "--draw" => o.draw = Some(planet_render::DrawMode::parse(it.next().ok_or("--draw needs per-draw or indirect")?)?),
            "--adapter" => {
                o.adapter = Some(match it.next().ok_or("--adapter needs a value")?.as_str() {
                    "software" => AdapterPolicy::Software,
                    "hardware" => AdapterPolicy::HighPerformance,
                    other => return Err(format!("--adapter must be software or hardware, got '{other}'")),
                })
            }
            other => return Err(format!("unknown option '{other}'\n{USAGE}")),
        }
    }
    Ok(o)
}

use planet_cache::tiles::MAX_REGION_LEVEL;

/// Options that only the tile store commands of `generate` understand.
pub(crate) const STORE_OPTIONS: [&str; 16] = [
    "--prefill-dir",
    "--prefill-levels",
    "--plan-check",
    "--compact-cache",
    "--cap-bytes",
    "--world",
    "--cache-dir",
    "--verify-cache",
    "--region",
    "--sample",
    "--allow-stale",
    "--global-dir",
    "--verify-global",
    "--global-products",
    "--global-res",
    "--threads",
];

/// Options a `--flight` run would ignore, and the flight-only options other runs would ignore (TEST-013).
const FLIGHT_EXCLUDES: [&str; 13] = [
    "--perf",
    "--perf-smoke",
    "--repro",
    "--out",
    "--bundle",
    "--stream",
    "--pipelined",
    "--tile",
    "--script",
    "--frames",
    "--res",
    "--manifest",
    "--face-net",
];
const FLIGHT_ONLY: [&str; 2] = ["--size", "--out-dir"];

/// Reject option combinations that would be silently ignored, before any GPU work.
fn check_flight_options(o: &Options) -> Result<(), String> {
    if o.flight.is_some() {
        if o.scene != "terrain" {
            return Err("--flight is only available for --scene terrain".into());
        }
        let clash: Vec<&str> = FLIGHT_EXCLUDES.iter().copied().filter(|f| o.given.contains(*f)).collect();
        if !clash.is_empty() {
            return Err(format!("--flight writes a frame sequence and excludes {}", clash.join(", ")));
        }
        if o.out_dir.is_none() {
            return Err("--flight needs --out-dir".into());
        }
    } else if let Some(f) =
        FLIGHT_ONLY.iter().find(|f| o.given.contains(**f) && !(o.pipelined && **f == "--out-dir") && !litperf::is_capture_scene(&o.scene))
    {
        return Err(format!("{f} only applies to --flight"));
    }
    if o.lockstep && !o.pipelined {
        return Err("--lockstep only applies to --pipelined".into());
    }
    if o.then.is_some() && !o.pipelined {
        return Err("--then only applies to --pipelined".into());
    }
    if o.pipelined {
        if o.scene != "terrain" {
            return Err("--pipelined is only available for --scene terrain".into());
        }
        const CLASH: [&str; 8] = ["--stream", "--bundle", "--perf", "--perf-smoke", "--repro", "--flight", "--size", "--tile"];
        let clash: Vec<&str> = CLASH.iter().copied().filter(|f| o.given.contains(*f)).collect();
        if !clash.is_empty() {
            return Err(format!("--pipelined draws one settled view and excludes {}", clash.join(", ")));
        }
    }
    Ok(())
}

/// `--floor-level` (LOD-011): `test-render` and `generate` take it, the viewer takes no flag; it applies to the terrain scenes, the
/// streaming and lit scenes and the pre-fill commands only (anything else would ignore it), not to a replayed bundle (the bundle
/// carries its own `min_level`); and an explicit floor above the finest level is refused, naming both, before any work.
fn check_floor_option(o: &Options) -> Result<(), String> {
    if !o.given.contains("--floor-level") {
        return Ok(());
    }
    match o.mode {
        Mode::Editor => return Err("--floor-level applies to `test-render` and `generate`: the viewer takes no flag (LOD-011)".into()),
        Mode::Generate if o.prefill_dir.is_none() && o.global_dir.is_none() => {
            return Err("--floor-level only applies to the pre-fill of `generate` (--prefill-dir, --global-dir)".into())
        }
        Mode::TestRender => {
            if o.repro.is_some() {
                return Err("--floor-level does not apply to --repro: a bundle carries its own render floor (`min_level`)".into());
            }
            let takes = o.scene == "terrain"
                || streamperf::is_stream_scene(&o.scene)
                || litperf::is_lit_scene(&o.scene)
                || litperf::is_capture_scene(&o.scene);
            if !takes {
                return Err(format!("--floor-level does not apply to --scene {}: it applies to the terrain and streaming scenes", o.scene));
            }
        }
        Mode::Generate => {}
    }
    o.check_floor()
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--size must be WxH with both sides 16..=8192, got '{s}'");
    let (w, h) = s.split_once('x').ok_or_else(bad)?;
    let (w, h): (u32, u32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if !(16..=8192).contains(&w) || !(16..=8192).contains(&h) {
        return Err(bad());
    }
    Ok((w, h))
}

pub fn run(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let o = parse_args(args)?;
    check_floor_option(&o)?;
    if !matches!(o.mode, Mode::Generate) {
        // The global-bake perf scene takes the definition, grid size and thread count of the global commands.
        let bake_scene = matches!(o.mode, Mode::TestRender) && o.perf && o.scene == "global-bake";
        let stream_scene = matches!(o.mode, Mode::TestRender)
            && (o.perf || o.perf_smoke || litperf::is_capture_scene(&o.scene))
            && (streamperf::is_stream_scene(&o.scene) || litperf::is_lit_scene(&o.scene) || litperf::is_capture_scene(&o.scene));
        let terrain_scene = matches!(o.mode, Mode::TestRender) && o.scene == "terrain";
        let window = matches!(o.mode, Mode::Editor) && !o.offscreen && !o.no_window;
        let allowed: &[&str] = if bake_scene {
            &["--world", "--global-res", "--threads"]
        } else if stream_scene {
            // `--prefill-levels`: the levels of the tile pack the gated dives require pre-filled at their start (default 4).
            &["--world", "--global-res", "--prefill-levels"]
        } else if window {
            &["--world", "--global-res", "--prefill-levels"]
        } else if terrain_scene {
            &["--world", "--global-res"]
        } else {
            &[]
        };
        if let Some(f) = STORE_OPTIONS.iter().find(|f| o.given.contains(**f) && !allowed.contains(f)) {
            return Err(format!("{f} only applies to `generate` (the tile store commands)"));
        }
    }
    if matches!(o.mode, Mode::Generate) && o.given.contains("--look") {
        return Err(
            "--look only applies to `editor` and `test-render`: the look is presentation data and never changes a generated tile".into()
        );
    }
    // A replayed bundle is self-contained (CON-19): it carries its plans' look, so no sibling file is looked for beside it.
    if !matches!(o.mode, Mode::Generate) && !(o.repro.is_some() && o.look.is_none()) {
        // The look file is loaded (and so validated) before any world is built or device opened: a bad file stops the run with
        // the key and the value (LOOK-005).
        if let Some(line) = lookload::load(&o)?.describe() {
            writeln!(out, "{line}").map_err(err)?;
        }
    }
    let smoke = if o.smoke { " [smoke ok]" } else { "" };
    match o.mode {
        Mode::Editor if o.offscreen => skeleton::render_offscreen(&o, out),
        Mode::Editor if o.no_window => writeln!(out, "planet editor: window not opened (--no-window){smoke}").map_err(err),
        Mode::Editor => window::run_window(&o, out),
        Mode::Generate if !o.smoke => skeleton::generate(&o, out),
        Mode::Generate => writeln!(out, "planet generate: ready{smoke}").map_err(err),
        Mode::TestRender => test_render(&o, out),
    }
}

pub(crate) fn err(e: std::io::Error) -> String {
    e.to_string()
}

/// ADR 0020: measure the machine and refuse (listing every failed condition with its measured value) unless it is ready.
/// `--machine-ready` is the owner's explicit override: the failed conditions are printed and recorded in the log.
/// The adapter itself is checked by the caller before this (it is never overridable and costs no sampling time).
pub(crate) fn readiness_gate(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<Vec<String>, String> {
    writeln!(out, "readiness gate: sampling the machine for {} s ...", planet_perf::gate::LOAD_WINDOW_S).map_err(err)?;
    let failures = planet_perf::gate::readiness(&planet_perf::gate::measure_machine(facts(ctx)));
    if failures.is_empty() {
        writeln!(out, "readiness gate: ok").map_err(err)?;
        return Ok(Vec::new());
    }
    let lines: Vec<String> = failures.iter().map(ToString::to_string).collect();
    if o.machine_ready {
        for l in &lines {
            writeln!(out, "readiness gate FAILED (owner override --machine-ready, recorded in the log): {l}").map_err(err)?;
        }
        return Ok(lines);
    }
    Err(format!(
        "refusing to measure: the machine is not ready (ADR 0020):\n  {}\nFix these and retry; only the owner overrides with --machine-ready.",
        lines.join("\n  ")
    ))
}

/// Whether a run reported throttling in the clock samples taken at its start or end (`nvidia-smi`, the boundary
/// approximation of "during measured frames"). A missing sample cannot show the absence of throttling, so it counts.
pub(crate) fn run_throttled(r: &planet_perf::RunReport) -> bool {
    [&r.clocks_before, &r.clocks_after].iter().any(|c| c.as_ref().is_none_or(|c| planet_perf::gate::is_throttling(&c.throttle_reasons)))
}

/// The session verdict, printed once before any budget verdict so a bad session never shows "ok" first (ADR 0020). One definition
/// of what voids frame verdicts and what only GPU verdicts (`planet_perf::validity`); the lines come from it.
pub(crate) fn session_verdict_line(out: &mut dyn Write, verdict: &planet_perf::validity::SessionVerdict) -> Result<(), String> {
    session_verdict_line_for(out, verdict, true)
}

/// [`session_verdict_line`] for a session that takes no GPU rows (the streaming scenes) when `gpu_rows` is false.
pub(crate) fn session_verdict_line_for(
    out: &mut dyn Write,
    verdict: &planet_perf::validity::SessionVerdict,
    gpu_rows: bool,
) -> Result<(), String> {
    for l in verdict.lines_for(gpu_rows) {
        writeln!(out, "{l}").map_err(err)?;
    }
    Ok(())
}

pub(crate) fn facts(ctx: &GpuContext) -> AdapterFacts {
    AdapterFacts {
        name: ctx.adapter_name().to_string(),
        backend: ctx.backend_name(),
        kind: match ctx.kind() {
            planet_render::AdapterKind::Discrete => DeviceKind::Discrete,
            planet_render::AdapterKind::Integrated => DeviceKind::Integrated,
            planet_render::AdapterKind::Software => DeviceKind::Cpu,
            _ => DeviceKind::Other,
        },
    }
}

fn test_render(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    if o.smoke {
        return writeln!(out, "planet test-render [smoke ok]").map_err(err);
    }
    check_flight_options(o)?;
    worldfield::check_options(o)?;
    let policy = o.adapter.unwrap_or_else(AdapterPolicy::from_env);
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    // Hybrid-graphics rule (report §8): always log which adapter we got.
    writeln!(out, "{}", ctx.adapter_log_line()).map_err(err)?;
    if let Some(mode) = o.draw {
        ctx.prefer_draw_mode(mode);
        writeln!(out, "draw: {} requested, {} in effect", mode.name(), ctx.default_draw_mode().name()).map_err(err)?;
    }
    if let Some(mode) = o.upload {
        ctx.prefer_upload_mode(mode);
    }
    if let Some(format) = o.tile_format {
        ctx.prefer_tile_format(format);
    }
    writeln!(out, "tile format: {}", ctx.default_tile_format().name()).map_err(err)?;
    if ctx.default_tile_format() == planet_render::TileFormat::Compact && o.cells < planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS {
        return Err(format!(
            "--cells {}: the compact tile format needs at least {} cells per tile edge (the climate of a tile rides in the spare channel of its first 162 texels, GEN-021): use --cells {} or more, or --tile-format vertex",
            o.cells,
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS,
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS
        ));
    }
    if o.perf && o.perf_smoke {
        return Err("--perf and --perf-smoke exclude each other: a smoke run is never a measurement".into());
    }
    if o.perf_smoke && o.scene != "terrain" && !streamperf::is_stream_scene(&o.scene) && !litperf::is_lit_scene(&o.scene) {
        return Err("--perf-smoke is only available for --scene terrain, a streaming scene (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, streaming) or a lit scene (lit, lit-formats)".into());
    }
    if o.lit && !streamperf::is_stream_scene(&o.scene) {
        return Err("--lit applies to the streaming perf scenes (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, dive-cold, streaming); the scenes `lit` and `lit-formats` are always lit".into());
    }
    if o.given.contains("--views") && !litperf::is_lit_scene(&o.scene) && !litperf::is_capture_scene(&o.scene)
        || o.given.contains("--flight-frames") && !litperf::is_lit_scene(&o.scene) && !litperf::is_capture_scene(&o.scene)
    {
        return Err(
            "--views applies to the scenes `lit`, `lit-formats` and `capture-flights`, --flight-frames to those and `capture-flights`"
                .into(),
        );
    }
    streamperf::check_options(o)?;
    if litperf::is_capture_scene(&o.scene) {
        return litperf::capture_flights(o, &ctx, out);
    }
    if o.flight.is_some() {
        return skeleton::render_flight(&ctx, o, out);
    }
    if o.perf || o.perf_smoke {
        return match o.scene.as_str() {
            "terrain" => skeleton::perf_terrain(o, &ctx, out),
            "global-bake" => globalperf::perf_global_bake(o, &ctx, out),
            s if streamperf::is_stream_scene(s) => streamperf::perf_streaming(o, &ctx, out),
            s if litperf::is_lit_scene(s) => litperf::perf_lit(o, &ctx, out),
            _ => perf_skeleton(o, &ctx, out),
        };
    }
    if let Some(bundle) = &o.repro {
        return skeleton::replay(&ctx, bundle, o.out.as_deref(), out);
    }
    match o.scene.as_str() {
        "tiles" => skeleton::render_tiles(&ctx, o, out),
        "terrain" => skeleton::render_terrain(&ctx, o, out),
        "hello-triangle" => {
            let frame = planet_render::hello_triangle(&ctx, 256);
            if let Some(path) = &o.out {
                frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
                writeln!(out, "wrote {}", path.display()).map_err(err)?;
            }
            Ok(())
        }
        other => Err(format!("unknown scene '{other}' (known: hello-triangle, tiles, terrain)")),
    }
}

/// Performance-harness skeleton: adapter check, power/clock logging, warm-up, medians, JSON log.
/// It times the hello-triangle round trip as a stand-in for a frame (the terrain scenes time real frames).
fn perf_skeleton(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = readiness_gate(o, ctx, out)?;
    let protocol = Protocol { warmup_frames: 10, measured_frames: 50, runs: 3, ..Protocol::default() };
    let mut runs = Vec::new();
    for _ in 0..protocol.runs {
        let before = planet_perf::query_nvidia_smi();
        let mut samples = Vec::new();
        for _ in 0..(protocol.warmup_frames + protocol.measured_frames) {
            let t = Instant::now();
            planet_render::hello_triangle(ctx, 256);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let stats = planet_perf::summarize(&samples, protocol.warmup_frames).ok_or("no valid samples")?;
        runs.push(RunReport {
            path_name: "skeleton-hello-triangle".into(),
            vram_cap_gb: 8,
            stats,
            clocks_before: before,
            clocks_after: planet_perf::query_nvidia_smi(),
            metrics: vec![],
            counts: vec![],
        });
    }
    let medians: Vec<f64> = runs.iter().map(|r| r.stats.median).collect();
    let throttled: Vec<String> = runs.iter().filter(|r| run_throttled(r)).map(|r| r.path_name.clone()).collect();
    // The skeleton is a stand-in workload: its noise is judged on its three runs without a retry.
    let noisy: Vec<(String, f64)> = if planet_perf::gate::noise_needs_retry(&medians) {
        vec![("skeleton-hello-triangle".into(), planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN))]
    } else {
        Vec::new()
    };
    let validity = planet_perf::gate::validity(&throttled, &noisy);
    let scheme = planet_perf::query_power_scheme();
    let verdict = planet_perf::validity::SessionVerdict::assess(&facts(ctx), scheme.as_deref(), &runs, false, &validity, &owner_override);
    session_verdict_line(out, &verdict)?;
    writeln!(out, "run-to-run noise (max-min)/median: {:.3}", planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN)).map_err(err)?;
    let json =
        planet_perf::report_json(&facts(ctx), planet_perf::query_power_scheme().as_deref(), &owner_override, &validity, &protocol, &runs);
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("skeleton.json"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_str(args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut buf = Vec::new();
        run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
    }

    // spec: LOD-010
    #[test]
    fn the_finest_level_defaults_to_the_production_cap_for_the_tile_size_and_is_overridable() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let o = args(&["test-render", "--scene", "terrain"]);
        assert_eq!((o.finest_level, o.finest_level_for(32), o.finest_level_for(64), o.finest_level_for(16)), (None, 22, 21, 23));
        let o = args(&["test-render", "--finest-level", "26", "--cells", "64"]);
        assert_eq!((o.finest_level, o.finest_level_for(64)), (Some(26), 26), "the explicit cap wins whatever the tile size");
        assert!(parse_args(&["test-render".into(), "--finest-level".into(), "29".into()]).unwrap_err().contains("at most 28"));
        assert!(parse_args(&["test-render".into(), "--finest-level".into()]).unwrap_err().contains("needs a level"));
    }

    // spec: STRM-020, TEST-012
    #[test]
    fn the_generation_pool_defaults_to_the_machine_less_the_reserve_and_the_options_override_it() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        let o = args(&["test-render", "--scene", "terrain"]);
        let g = o.gen_workers();
        assert_eq!(g.workers, planet_streaming::GenWorkers::workers_for(cores, planet_streaming::GEN_RESERVE));
        assert_eq!((g.ahead, o.gen_reserve_used()), (g.workers, Some(planet_streaming::GEN_RESERVE)));
        let o = args(&["test-render", "--gen-reserve", "5"]);
        assert_eq!(o.gen_workers().workers, planet_streaming::GenWorkers::workers_for(cores, 5));
        assert_eq!(o.gen_reserve_used(), Some(5));
        // An explicit size wins and the reserve decided nothing, so the log records none.
        let o = args(&["test-render", "--gen-workers", "7", "--gen-reserve", "5", "--gen-ahead", "3"]);
        assert_eq!((o.gen_workers().workers, o.gen_workers().ahead, o.gen_reserve_used()), (7, 3, None));
        assert_eq!(args(&["test-render", "--gen-reserve", "0"]).gen_workers().workers, cores);
        assert!(parse_args(&["test-render".into(), "--gen-reserve".into(), "x".into()]).unwrap_err().contains("non-negative"));
        assert!(parse_args(&["test-render".into(), "--gen-reserve".into()]).unwrap_err().contains("needs a count"));
    }

    // spec: STRM-016, TEST-012
    #[test]
    fn lockstep_is_a_pipelined_only_test_mode() {
        let parse = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(!parse(&["test-render", "--scene", "terrain", "--pipelined"]).unwrap().lockstep);
        assert!(parse(&["test-render", "--scene", "terrain", "--pipelined", "--lockstep"]).unwrap().lockstep);
        let o = parse(&["test-render", "--scene", "terrain", "--lockstep"]).unwrap();
        assert!(check_flight_options(&o).unwrap_err().contains("--lockstep only applies to --pipelined"));
    }

    // spec: STRM-020, TEST-012
    #[test]
    fn the_generation_workers_run_below_normal_priority_unless_the_option_says_normal() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let low = planet_streaming::Priority::BelowNormal;
        assert_eq!(args(&["test-render"]).unwrap().gen_workers().priority, low);
        assert_eq!(args(&["test-render", "--gen-priority", "below-normal"]).unwrap().gen_workers().priority, low);
        let o = args(&["test-render", "--gen-priority", "normal", "--gen-workers", "3"]).unwrap();
        assert_eq!((o.gen_workers().priority, o.gen_workers().workers), (planet_streaming::Priority::Normal, 3));
        assert!(args(&["test-render", "--gen-priority", "high"]).unwrap_err().contains("below-normal or normal"));
        assert!(args(&["test-render", "--gen-priority"]).unwrap_err().contains("needs a value"));
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "planet-app-tests-{}-{}-{name}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn s(&self, leaf: &str) -> String {
            self.0.join(leaf).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // spec: STRM-004
    #[test]
    fn generate_writes_a_region_into_a_store_and_cache_verify_checks_it() {
        let d = TempDir::new("store");
        let store = d.s("store");
        let out = run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        assert!(out.contains("4 tiles") && out.contains("4 generated, 0 already stored"), "{out}");
        let again = run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        assert!(again.contains("0 generated, 4 already stored"), "{again}");
        let ok = run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap();
        assert!(ok.contains("checked 4 of 4 entries: 4 match, 0 stale"), "{ok}");
        let sampled = run_str(&["generate", "--verify-cache", &store, "--seed", "3", "--sample", "2"]).unwrap();
        assert!(sampled.contains("checked 2 of 4 entries: 2 match"), "{sampled}");
        // Another seed: every stored tile is stale, nothing could be compared: that is not a pass.
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "4"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("--allow-stale") && e.contains("4 entries"), "{e}");
        // Unless stale entries are expected (the definition changed on purpose).
        let stale = run_str(&["generate", "--verify-cache", &store, "--seed", "4", "--allow-stale"]).unwrap();
        assert!(stale.contains("4 stale without a known reason") && stale.contains("0 differ"), "{stale}");
    }

    // spec: STRM-013
    #[test]
    fn generate_compacts_a_store_dropping_dead_records_and_honouring_the_cap() {
        use planet_cache::pack::{PackWriter, Record, TileStore};
        let d = TempDir::new("compact");
        let store = d.s("store");
        run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        // A record of an older generator version, as left behind by a version bump.
        let dir = std::path::Path::new(&store);
        let params = planet_cache::tiles::World::from_definition(&planet_world_def::WorldDefinition::single_planet(6_371_000.0, 3))
            .unwrap()
            .params();
        let read = TileStore::open(dir, params).unwrap();
        let (kind, key) = read.all_entries()[0];
        let tile = planet_core::TileId::new(planet_core::Face(0), 1, 0, 0).unwrap();
        let live = read.get(kind, tile, key).unwrap().expect("the first tile of the region");
        drop(read);
        let mut w = PackWriter::open(dir, params, planet_cache::pack::DEFAULT_MAX_PACK_BYTES).unwrap();
        w.append(&Record { key: planet_cache::key::CacheKey([9; 16]), generator_version: live.generator_version + 1000, ..live.clone() })
            .unwrap();
        // A record of a kind this build does not know is kept.
        w.append(&Record { key: planet_cache::key::CacheKey([8; 16]), kind: 9, generator_version: 0, ..live.clone() }).unwrap();
        w.finish().unwrap();
        let out = run_str(&["generate", "--compact-cache", &store, "--seed", "3"]).unwrap();
        assert!(out.contains("kept 5 records") && out.contains("dropped 1 dead") && out.contains("seed 3"), "{out}");
        let again = run_str(&["generate", "--compact-cache", &store, "--seed", "3"]).unwrap();
        assert!(again.contains("kept 5 records") && again.contains("dropped 0 dead"), "{again}");
        assert!(run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap().contains("4 match"));
        let capped = run_str(&["generate", "--compact-cache", &store, "--seed", "3", "--cap-bytes", "1"]).unwrap();
        assert!(capped.contains("kept 0 records") && capped.contains("5 evicted"), "{capped}");
        assert!(run_str(&["generate", "--compact-cache", &store]).unwrap_err().contains("give --world"));
        let clash = run_str(&["generate", "--compact-cache", &store, "--region", "0,1"]).unwrap_err();
        assert!(clash.contains("--compact-cache takes only"), "{clash}");
        let both = run_str(&["generate", "--compact-cache", &store, "--seed", "3", "--plan-check", &store]).unwrap_err();
        assert!(both.contains("--compact-cache takes only"), "{both}");
        let lone = run_str(&["generate", "--cap-bytes", "5", "--seed", "3"]).unwrap_err();
        assert!(lone.contains("belongs to --compact-cache"), "{lone}");
        assert!(run_str(&["generate", "--compact-cache", &d.s("missing")]).unwrap_err().contains("no such cache directory"));
    }

    // spec: STRM-012
    #[test]
    fn generate_bakes_the_global_field_into_blocks_and_verify_global_checks_them() {
        let d = TempDir::new("global");
        let store = d.s("store");
        let built =
            run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--threads", "2", "--prefill-levels", "0"])
                .unwrap();
        assert!(
            built.contains("8 cells per face edge in blocks of 8 (level 0)") && built.contains("6 blocks written, 0 already stored"),
            "{built}"
        );
        let again = run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        assert!(again.contains("0 blocks written, 6 already stored"), "{again}");
        // spec: STRM-023
        assert!(built.contains("global ocean channel: 6 blocks written, 0 already stored; "), "{built}");
        assert!(again.contains("global ocean channel: 6 blocks already stored under their current key, nothing recomputed"), "{again}");
        // spec: STRM-025
        assert!(built.contains("global water summary: written, "), "{built}");
        assert!(again.contains("global water summary: already stored under its current key, nothing recomputed"), "{again}");
        let ok = run_str(&["generate", "--verify-global", &store, "--seed", "3"]).unwrap();
        assert!(ok.contains("checked 13 of 13 entries: 13 match"), "6 blocks, 6 ocean blocks and the water summary: {ok}");
        let sampled = run_str(&["generate", "--verify-global", &store, "--seed", "3", "--sample", "5"]).unwrap();
        assert!(sampled.contains("checked 5 of 13 entries: 5 match"), "{sampled}");
        // Another seed: all stale, nothing compared.
        let e = run_str(&["generate", "--verify-global", &store, "--seed", "4"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("--allow-stale"), "{e}");
        assert!(run_str(&["generate", "--verify-global", &store, "--seed", "4", "--allow-stale"]).is_ok());
        // A missing directory is refused, not an empty store.
        let missing = d.s("nowhere");
        assert!(run_str(&["generate", "--verify-global", &missing]).unwrap_err().contains("no such cache directory"));
        // Bad blocks are refused naming the limit.
        let bad = run_str(&["generate", "--global-dir", &d.s("bad"), "--global-res", "3"]).unwrap_err();
        assert!(!std::path::Path::new(&d.s("bad")).exists(), "a refused bake creates nothing");
        assert!(bad.contains("power-of-two"), "{bad}");
        let huge = run_str(&["generate", "--global-dir", &d.s("huge"), "--global-res", "8388608"]).unwrap_err();
        assert!(huge.contains("65536") && !huge.contains("level"), "a resolution error naming the finest grid: {huge}");
    }

    // spec: STRM-021
    #[test]
    fn generate_prefills_the_coarse_tiles_after_the_bake_or_into_an_existing_bake_and_a_second_run_writes_nothing() {
        let d = TempDir::new("prefill");
        let (a, b) = (d.s("a"), d.s("b"));
        // After the bake: levels 0..=1 are 6 + 24 tiles; the bake's own blocks are still there.
        let built = run_str(&[
            "generate",
            "--global-dir",
            &a,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "0",
            "--threads",
            "2",
            "--prefill-levels",
            "1",
        ])
        .unwrap();
        assert!(
            built.contains("6 blocks written")
                && built.contains("levels 0..=1")
                && built.contains("30 tiles, 30 written, 0 already stored"),
            "{built}"
        );
        let again =
            run_str(&["generate", "--global-dir", &a, "--global-res", "8", "--seed", "3", "--floor-level", "0", "--prefill-levels", "1"])
                .unwrap();
        assert!(
            again.contains("0 blocks written, 6 already stored") && again.contains("30 tiles, 0 written, 30 already stored"),
            "{again}"
        );
        // The bake still verifies; the tiles are another kind to it.
        let ok = run_str(&["generate", "--verify-global", &a, "--seed", "3"]).unwrap();
        assert!(ok.contains("13 match") && ok.contains("30 of another kind"), "6 blocks, 6 ocean blocks and the summary match: {ok}");
        // Into an existing bake, a deeper pre-fill adds the new level only; levels 0 disables.
        run_str(&["generate", "--global-dir", &b, "--global-res", "8", "--seed", "3", "--floor-level", "0", "--prefill-levels", "0"])
            .unwrap();
        let off =
            run_str(&["generate", "--prefill-dir", &b, "--global-res", "8", "--seed", "3", "--floor-level", "0", "--prefill-levels", "0"])
                .unwrap();
        assert!(off.contains("disabled"), "{off}");
        let one = run_str(&[
            "generate",
            "--prefill-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "0",
            "--prefill-levels",
            "1",
            "--threads",
            "3",
        ])
        .unwrap();
        assert!(one.contains("30 tiles, 30 written, 0 already stored") && one.contains("on 3 threads"), "{one}");
        let two =
            run_str(&["generate", "--prefill-dir", &b, "--global-res", "8", "--seed", "3", "--floor-level", "0", "--prefill-levels", "2"])
                .unwrap();
        assert!(two.contains("126 tiles, 96 written, 30 already stored"), "{two}");
        let none =
            run_str(&["generate", "--prefill-dir", &b, "--global-res", "8", "--seed", "3", "--floor-level", "0", "--prefill-levels", "2"])
                .unwrap();
        assert!(none.contains("126 tiles, 0 written, 126 already stored") && none.contains("packs grew by 0 bytes"), "{none}");
        // Refusals: the bake's size is required, another store and combinations of commands are refused, the depth is bounded.
        for (args, needle) in [
            (vec!["generate", "--prefill-dir", b.as_str(), "--seed", "3"], "needs --global-res"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--global-dir", b.as_str()], "separate commands"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--verify-global", b.as_str()], "excludes --verify-global"),
            (vec!["generate", "--prefill-levels", "2", "--seed", "3"], "applies to --global-dir"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--prefill-levels", "9"], "at most 8"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--prefill-levels", "22"], "at most 8"),
            (vec!["generate", "--prefill-dir", d.s("nowhere").as_str(), "--global-res", "8"], "no such store directory"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "16", "--seed", "3"], "holds no bake of 16 cells"),
            (
                vec![
                    "generate",
                    "--prefill-dir",
                    b.as_str(),
                    "--global-res",
                    "8",
                    "--seed",
                    "3",
                    "--finest-level",
                    "2",
                    "--prefill-levels",
                    "2",
                ],
                "finest level 2",
            ),
            (vec!["test-render", "--prefill-dir", b.as_str()], "only applies to `generate`"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
    }

    // spec: STRM-021, LOD-011
    #[test]
    fn generate_prefills_from_the_render_floor_says_so_when_the_depth_is_below_it_and_refuses_a_floor_above_the_finest_level() {
        let d = TempDir::new("prefill-floor");
        let (a, b) = (d.s("a"), d.s("b"));
        let common = ["--global-res", "8", "--seed", "3", "--cells", "16", "--threads", "4"];
        let gen = |extra: &[&str]| {
            let mut args = vec!["generate"];
            args.extend(common);
            args.extend(extra);
            run_str(&args)
        };
        // After the bake, at the default floor 3: levels 3 are the 384 roots; a deeper run adds level 4 only; a third writes nothing.
        let built = gen(&["--global-dir", &a, "--prefill-levels", "3"]).unwrap();
        assert!(built.contains("levels 3..=3") && built.contains("384 tiles, 384 written, 0 already stored"), "{built}");
        let four = gen(&["--prefill-dir", &a, "--prefill-levels", "4"]).unwrap();
        assert!(four.contains("levels 3..=4") && four.contains("1920 tiles, 1536 written, 384 already stored"), "{four}");
        let again = gen(&["--prefill-dir", &a, "--prefill-levels", "4"]).unwrap();
        assert!(again.contains("1920 tiles, 0 written, 1920 already stored") && again.contains("packs grew by 0 bytes"), "{again}");
        // `--prefill-levels 0` disables; a depth below the floor writes nothing and says so; floor 0 writes the levels from 0.
        gen(&["--global-dir", &b, "--prefill-levels", "0"]).unwrap();
        let off = gen(&["--prefill-dir", &b, "--prefill-levels", "0"]).unwrap();
        assert!(off.contains("disabled"), "{off}");
        let below = gen(&["--prefill-dir", &b, "--floor-level", "3", "--prefill-levels", "1"]).unwrap();
        assert!(
            below.contains("nothing to write") && below.contains("--prefill-levels 1") && below.contains("below the render floor 3"),
            "{below}"
        );
        let zero = gen(&["--prefill-dir", &b, "--floor-level", "0", "--prefill-levels", "1"]).unwrap();
        assert!(zero.contains("levels 0..=1") && zero.contains("30 tiles, 30 written, 0 already stored"), "{zero}");
        // Refusals: a floor above the finest level names both values; the viewer takes no flag; other generate commands would ignore it.
        let e = gen(&["--prefill-dir", &b, "--floor-level", "5", "--finest-level", "4", "--prefill-levels", "2"]).unwrap_err();
        assert!(e.contains("--floor-level 5") && e.contains("finest level 4"), "{e}");
        let e = run_str(&["test-render", "--scene", "terrain", "--repro", "bundle.txt", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("does not apply to --repro"), "{e}");
        let e = run_str(&["editor", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("the viewer takes no flag"), "{e}");
        let e = gen(&["--verify-global", &a, "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("--floor-level only applies to the pre-fill"), "{e}");
        let e = run_str(&["test-render", "--scene", "hello-triangle", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("does not apply to --scene hello-triangle"), "{e}");
        // The effective floor is the lower of the floor and the finest level: a finest level of 2 needs no flag.
        let o = parse_args(&["test-render".to_string(), "--finest-level".to_string(), "2".to_string()]).unwrap();
        assert_eq!((o.floor_for(32), o.min_level()), (2, 3));
        assert_eq!(parse_args(&["test-render".to_string()]).unwrap().floor_for(32), 3);
    }

    // spec: GEN-019
    #[test]
    fn the_compact_format_with_fewer_than_16_cells_is_refused_naming_the_minimum() {
        let e = run_str(&["test-render", "--scene", "terrain", "--cells", "8", "--tile-format", "compact"]).unwrap_err();
        assert!(e.contains("--cells 8") && e.contains("at least 16") && e.contains("--tile-format vertex"), "{e}");
    }

    // spec: STRM-021, STRM-022
    #[test]
    fn a_pre_filled_tile_read_by_the_streamer_equals_the_generated_one_bit_for_bit_and_a_cold_reader_ignores_the_pack() {
        use planet_streaming::TileSource;
        let d = TempDir::new("prefill-read");
        let store = d.s("store");
        run_str(&[
            "generate",
            "--global-dir",
            &store,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--prefill-levels",
            "3",
            "--floor-level",
            "0",
            "--threads",
            "4",
            "--cells",
            "16",
        ])
        .unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        let world = std::sync::Arc::new(worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap());
        // Levels 0..=3 are all present for the run's cells and finest level in the compact form; the vertex form finds only levels 0-2
        // (those records are shared by both forms, the pack holds the compact form of level 3).
        assert_eq!(world.prefill_present(0, 3, 16, 22, planet_render::TileFormat::Compact).unwrap(), (510, 510));
        assert_eq!(world.prefill_present(0, 3, 16, 22, planet_render::TileFormat::Vertex).unwrap(), (126, 510));
        assert_eq!(world.prefill_present(0, 4, 16, 22, planet_render::TileFormat::Compact).unwrap(), (510, 2046));
        assert_eq!(world.prefill_present(0, 3, 32, 22, planet_render::TileFormat::Compact).unwrap(), (0, 510), "other cells");
        let src = |w: &std::sync::Arc<worldfield::WorldTerrain>, format| skeleton::MeshSource {
            world: std::sync::Arc::clone(w),
            format,
            max_level: 22,
            cells: 16,
        };
        let cold = std::sync::Arc::new(worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap().cold());
        let field = world.field().unwrap();
        let mut sampled = 0;
        for tile in planet_cache::meshpack::prefill_tiles(3)
            .into_iter()
            .step_by(11)
            .chain(planet_cache::meshpack::prefill_tiles(3).into_iter().take(8))
        {
            let on_demand = skeleton::tile_for(planet_render::TileFormat::Compact, field.as_ref(), tile, 16, world.radius_m(), 22);
            let before = world.prefill_hits();
            assert_eq!(
                src(&world, planet_render::TileFormat::Compact).load(tile).unwrap(),
                on_demand,
                "level {} tile {:#x} from the pack",
                tile.level(),
                tile.raw()
            );
            assert_eq!(world.prefill_hits(), before + 1, "served from the pack");
            let staged =
                src(&world, planet_render::TileFormat::Compact).load_staged(tile, &planet_streaming::CancelToken::never()).unwrap();
            assert_eq!(staged, Some(on_demand.clone()));
            assert_eq!(
                src(&cold, planet_render::TileFormat::Compact).load(tile).unwrap(),
                on_demand,
                "the cold reader generates the same tile"
            );
            sampled += 1;
        }
        assert_eq!(cold.prefill_hits(), 0, "the cold reader never reads the pack");
        // The recorded-only counters (STRM-022): each sampled tile was looked up twice (load and load_staged) and found; the cold
        // reader counted nothing; the pack saw those lookups (and the blocks the on-demand tiles read).
        let c = world.counters();
        assert_eq!((c.prefill_lookups, c.prefill_found, c.prefill_skipped), (2 * sampled, 2 * sampled, 0), "{c:?}");
        assert_eq!(cold.counters().prefill_lookups, 0);
        assert!(c.pack.gets >= c.prefill_lookups && c.pack.found >= c.prefill_found, "{c:?}");
        assert!(c.field_builds >= 2 * sampled, "every load builds its field: {c:?}");
        assert!(!cold.reads_prefilled() && world.reads_prefilled() && sampled > 50);
        // A tile deeper than the pre-fill is generated, not found.
        let deep = planet_core::TileId::new(planet_core::Face(0), 5, 1, 1).unwrap();
        let before = world.prefill_hits();
        src(&world, planet_render::TileFormat::Compact).load(deep).unwrap();
        assert_eq!(world.prefill_hits(), before);
        // A tile deeper than the pre-fill but within the readable levels is looked up and missed; one beyond them is skipped at once.
        let after = world.counters().since(&c);
        assert_eq!((after.prefill_lookups, after.prefill_found, after.prefill_skipped), (1, 0, 0), "{after:?}");
        let beyond = planet_core::TileId::new(planet_core::Face(0), 9, 1, 1).unwrap();
        src(&world, planet_render::TileFormat::Compact).load(beyond).unwrap();
        let after = world.counters().since(&c);
        assert_eq!((after.prefill_lookups, after.prefill_skipped), (1, 1), "{after:?}");
    }

    // spec: STRM-012
    #[test]
    fn the_global_commands_refuse_options_they_would_ignore_and_combinations_of_commands() {
        let d = TempDir::new("global-refuse");
        let s = d.s("store");
        for (args, needle) in [
            (vec!["generate", "--global-dir", s.as_str(), "--verify-global", s.as_str()], "separate commands"),
            (vec!["generate", "--global-dir", s.as_str(), "--cache-dir", s.as_str()], "exclude"),
            (vec!["generate", "--global-dir", s.as_str(), "--sample", "2"], "only apply to --verify-global"),
            (vec!["generate", "--verify-global", s.as_str(), "--global-res", "64"], "excludes --global-res"),
            (vec!["generate", "--global-dir", s.as_str(), "--global-products", s.as_str()], "separate commands"),
            (vec!["generate", "--cache-dir", s.as_str(), "--region", "0,1", "--global-res", "64"], "exclude"),
            (vec!["generate", "--tile", "0,1,0,0", "--global-res", "64"], "belongs to the tile store"),
            (vec!["generate", "--cache-dir", s.as_str(), "--region", "0,1", "--threads", "2"], "exclude --threads"),
            (vec!["generate", "--verify-global", s.as_str(), "--threads", "2"], "excludes --threads"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
        let e = run_str(&["test-render", "--global-dir", s.as_str()]).unwrap_err();
        assert!(e.contains("only applies to `generate`"), "{e}");
    }

    // spec: STRM-012
    #[test]
    fn global_products_writes_cube_nets_and_maps_for_review() {
        let d = TempDir::new("global-products");
        let dir = d.s("img");
        let out = run_str(&["generate", "--global-products", &dir, "--global-res", "16", "--seed", "3", "--threads", "2"]).unwrap();
        assert!(out.contains("4 cube nets and 4 maps"), "{out}");
        for ch in ["continent", "elevation", "temperature", "precipitation"] {
            assert!(std::path::Path::new(&dir).join(format!("{ch}-net.png")).is_file(), "{ch} net");
            assert!(std::path::Path::new(&dir).join(format!("{ch}-map.png")).is_file(), "{ch} map");
        }
    }

    // spec: STRM-004
    #[test]
    fn cache_verify_of_an_existing_empty_directory_compared_nothing_and_fails_even_with_allow_stale() {
        let d = TempDir::new("empty-store");
        let store = d.s("store");
        std::fs::create_dir_all(&store).unwrap();
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("0 entries"), "{e}");
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "3", "--allow-stale"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("does not help"), "{e}");
    }

    // spec: STRM-004
    #[test]
    fn cache_verify_fails_naming_a_tile_made_with_an_input_missing_from_its_key() {
        let d = TempDir::new("bad-store");
        let store = d.s("store");
        run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        // Append a tile whose key is the seed-3 world's but whose payload was made with seed 5.
        let world =
            planet_cache::tiles::World::from_definition(&planet_world_def::WorldDefinition::single_planet(RADIUS_M_TEST, 3)).unwrap();
        let t = planet_core::TileId::new(planet_core::Face(1), 2, 1, 1).unwrap();
        let wrong = planet_generators::generate_tile(5, &planet_core::cube::TangentWarp, t, 8);
        let mut bad = world.tile_record(t, 8).unwrap();
        bad.payload = planet_cache::codec::encode_height_tile(&wrong).unwrap();
        let mut w =
            planet_cache::pack::PackWriter::open(std::path::Path::new(&store), world.params(), planet_cache::pack::DEFAULT_MAX_PACK_BYTES)
                .unwrap();
        w.append(&bad).unwrap();
        w.finish().unwrap();
        let mut buf = Vec::new();
        let args: Vec<String> = ["generate", "--verify-cache", &store, "--seed", "3"].iter().map(|s| s.to_string()).collect();
        let e = run(&args, &mut buf).unwrap_err();
        let printed = String::from_utf8(buf).unwrap();
        assert!(e.contains("1 differing"), "{e}");
        assert!(
            printed.contains("MISMATCH tile")
                && printed.contains("face 1 level 2 x 1 y 1")
                && printed.contains("at offset")
                && printed.contains("missing from its key"),
            "{printed}"
        );
        assert!(printed.contains("4 match") && printed.contains("1 differ"), "{printed}");
    }

    const RADIUS_M_TEST: f64 = 6_371_000.0;

    // spec: STRM-004
    #[test]
    fn generate_reads_the_definition_from_a_toml_file_and_refuses_inconsistent_options() {
        let d = TempDir::new("world");
        let world = d.s("world.toml");
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 3\n[body.0]\nradius_m = 6371000.0\n").unwrap();
        let store = d.s("store");
        // The file's world is the default single planet with seed 3: same keys, so the seed-3 store verifies against it.
        run_str(&["generate", "--cache-dir", &store, "--region", "2,1", "--res", "4", "--seed", "3"]).unwrap();
        let ok = run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap();
        assert!(ok.contains("4 match"), "{ok}");
        std::fs::write(&world, "[world]\nseed = 3\n").unwrap();
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap_err().contains("schema_version"));
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &d.s("missing.toml")]).unwrap_err().contains("reading"));
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 3\n").unwrap();
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap_err().contains("body.0.radius_m"));

        // The store options belong to `generate`'s store commands and nowhere else, and nothing is silently ignored.
        for (args, needle) in [
            (vec!["generate", "--verify-cache", store.as_str(), "--world", world.as_str(), "--seed", "3"], "give one"),
            (vec!["generate", "--verify-cache", store.as_str(), "--region", "0,1"], "only reads"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--sample", "0"], "only apply to --verify-cache"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--allow-stale"], "only apply to --verify-cache"),
            (vec!["generate", "--manifest", "m.txt", "--world", world.as_str()], "--world"),
            (vec!["generate", "--manifest", "m.txt", "--cache-dir", store.as_str()], "--cache-dir"),
            (vec!["editor", "--no-window", "--world", world.as_str()], "only applies to `generate`"),
            (vec!["editor", "--smoke", "--sample", "3"], "only applies to `generate`"),
            (vec!["test-render", "--cache-dir", store.as_str()], "only applies to `generate`"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,7"], "--region must be"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
        let missing = d.s("no-such-store");
        let e = run_str(&["generate", "--verify-cache", &missing, "--seed", "3"]).unwrap_err();
        assert!(e.contains("no such cache directory"), "{e}");
        let too_big = run_str(&["generate", "--cache-dir", &d.s("big"), "--region", "0,6", "--res", "2048"]).unwrap_err();
        assert!(too_big.contains("use a lower level or resolution"), "{too_big}");
        let no_region = run_str(&["generate", "--cache-dir", &store]).unwrap_err();
        assert!(no_region.contains("--region"), "{no_region}");
        for (args, needle) in [
            (vec!["generate", "--region", "0,1"], "belongs to the tile store commands"),
            (vec!["generate", "--sample", "3"], "belongs to the tile store commands"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--sample", "3"], "only apply to --verify-cache"),
            (vec!["generate", "--verify-cache", store.as_str(), "--cache-dir", store.as_str()], "only reads"),
            (vec!["generate", "--verify-cache", store.as_str(), "--res", "8"], "only reads"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--tile", "0,0,0,0"], "exclude"),
            (vec!["generate", "--region", "6,1", "--cache-dir", store.as_str()], "--region must be"),
            (vec!["generate", "--region", "0,7", "--cache-dir", store.as_str()], "--region must be"),
            (vec!["generate", "--region", "x", "--cache-dir", store.as_str()], "--region must be"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
    }

    // spec: BUILD-004
    #[test]
    fn all_three_run_modes_start() {
        // The editor's `--smoke` opens the window (tier C, `tests/gpu_window.rs`, VIEW-005); `--no-window` is the start check that
        // needs no display.
        for (mode, args) in [
            ("editor", vec!["editor", "--smoke", "--no-window"]),
            ("generate", vec!["generate", "--smoke"]),
            ("test-render", vec!["test-render", "--smoke"]),
        ] {
            let out = run_str(&args).unwrap_or_else(|e| panic!("{mode}: {e}"));
            assert!(out.contains("smoke ok"), "{mode}: {out}");
        }
    }

    // spec: VIEW-001
    #[test]
    fn the_window_refuses_a_software_adapter_outside_test_mode_before_anything_starts() {
        // No window, no display and no GPU is touched: the policy is checked first.
        let e = run_str(&["editor", "--adapter", "software"]).unwrap_err();
        assert!(e.contains("test and CI mode") && e.contains("discrete GPU"), "{e}");
        // The smoke is the test mode, and the viewer's options parse.
        let o = parse_args(&[
            "editor".into(),
            "--smoke".into(),
            "--no-vsync".into(),
            "--size".into(),
            "800x450".into(),
            "--view".into(),
            "level".into(),
        ])
        .unwrap();
        assert!(o.smoke && o.no_vsync && o.size == Some((800, 450)) && o.view == "level");
    }

    // spec: BUILD-004
    #[test]
    fn bad_arguments_are_actionable() {
        assert!(run_str(&["frobnicate"]).unwrap_err().contains("usage:"));
        assert!(run_str(&["test-render", "--adapter", "gpu"]).unwrap_err().contains("software or hardware"));
        assert!(run_str(&["test-render", "--scene"]).unwrap_err().contains("needs a value"));
    }

    // spec: TEST-013
    #[test]
    fn flight_options_that_would_be_ignored_are_rejected_before_any_gpu_work() {
        let d = std::env::temp_dir().join(format!("planet-flight-args-{}", std::process::id()));
        let p = d.to_str().unwrap();
        let flight = |extra: &[&str]| {
            let mut a = vec!["test-render", "--scene", "terrain", "--flight", "12", "--out-dir", p];
            a.extend_from_slice(extra);
            run_str(&a).unwrap_err()
        };
        assert!(flight(&["--perf-smoke"]).contains("excludes --perf-smoke"));
        assert!(flight(&["--perf", "--machine-ready"]).contains("excludes --perf"));
        assert!(flight(&["--repro", "x.repro"]).contains("excludes --repro"));
        assert!(flight(&["--script", "6", "--tile", "0,0,0,0"]).contains("--tile, --script"));
        let e = run_str(&["test-render", "--scene", "tiles", "--flight", "12", "--out-dir", p]).unwrap_err();
        assert!(e.contains("only available for --scene terrain"), "{e}");
        assert!(run_str(&["test-render", "--scene", "terrain", "--flight", "12"]).unwrap_err().contains("needs --out-dir"));
        assert!(run_str(&["test-render", "--scene", "terrain", "--size", "160x120"])
            .unwrap_err()
            .contains("--size only applies to --flight"));
        assert!(run_str(&["test-render", "--out-dir", p]).unwrap_err().contains("--out-dir only applies to --flight"));
        assert!(run_str(&["test-render", "--size", "12x12"]).unwrap_err().contains("--size must be WxH"));
        assert!(!d.exists(), "no rejected run may create the output directory");
    }

    // spec: GEN-003
    #[test]
    fn generate_manifest_writes_the_determinism_manifest() {
        let path = std::env::temp_dir().join(format!("planet-manifest-{}", std::process::id())).join("m.txt");
        let out = run_str(&["generate", "--manifest", path.to_str().unwrap()]).unwrap();
        assert!(out.contains("wrote"), "{out}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), crate::manifest::full_manifest());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert!(run_str(&["generate", "--manifest", "x.txt", "--tile", "0,0,0,0"]).unwrap_err().contains("excludes --tile"));
        assert!(run_str(&["generate", "--manifest"]).unwrap_err().contains("needs a path"));
    }
}
