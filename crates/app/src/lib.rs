//! The single binary's run modes (CON-01): `editor` (interactive viewer), `generate` (headless CLI)
//! and `test-render` (headless harness). M0 only wires them up; real work arrives with M1+.

pub mod globalperf;
pub mod manifest;
pub mod pipelined;
mod plancheck;
pub mod sheet;
pub mod skeleton;
pub mod streamperf;
pub mod worldfield;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use planet_perf::{AdapterFacts, DeviceKind, Protocol, RunReport};
use planet_render::{AdapterPolicy, GpuContext};

pub const USAGE: &str = "usage: planet <mode> [options]
modes:
  editor        interactive editor/viewer (window arrives in M2); --smoke starts and exits; --offscreen runs the skeleton without a window
  generate      headless generator CLI: --tile F,L,X,Y [--seed N --res N --out height.png --face-net net.png]; --manifest <file> writes the determinism manifest (GEN-003); --cache-dir <dir> --region F,L [--res N] writes a tile face region into a tile store; --verify-cache <dir> [--sample N] regenerates stored tiles and compares (both take --world <definition.toml> or --seed N; --verify-cache fails when nothing could be compared, --allow-stale accepts that); --global-dir <dir> [--global-res N, default 256] bakes the global-coarse field (continents, elevation, climate) of N cells per face edge into a tile store as blocks in the canonical layout (blocks of min(N, 1024) cells) and prints the time per sample; --verify-global <dir> [--sample N] regenerates stored blocks and compares; --global-products <dir> [--global-res N] writes cube nets and maps of an in-memory bake (these take --world or --seed and --threads N); --plan-check <dir> --global-res N (required) plans the standard flight at cells 32 and viewport / tau 240 over the stored bake with the terrain and the eroded field and fails if a view is rejected (M3 acceptance); --smoke starts and exits
  test-render   headless render harness
    --scene hello-triangle|tiles|terrain   render a scene (default hello-triangle)
    --script N|name          scripted terrain camera 0..9 or its name (default 3 = aerial-20km)
    --cells N --tau PX --frames N   terrain perf workload parameters (defaults 32, 6, 300)
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
    --view face|tile-id|level|morph|height|normals|depth|relief|morph-height   debug view (tiles scene: face, tile-id, height; a flight defaults to relief)
    --tile FACE,LEVEL,X,Y    tile to draw (repeatable; default: a tile and its east neighbour)
    --seed N --res N         generator seed and tile resolution (cells, power of two)
    --world <definition.toml> [--global-res N]   terrain scene (a view, --stream, --bundle, --flight, --perf, --perf-smoke): the world's height field (world.height_field, default the eroded field) over an in-memory bake of N cells per face edge (default 256, at most 1024); without it a view, stream or bundle is the seed-only reference field and --perf is the default world of --seed; a bundle records the world, --repro reads it back
    --bundle <file>          write a repro bundle of the render
    --repro <file>           replay a repro bundle (see cargo xtask repro)
    --out <file.png>         write the image
    --adapter software|hardware   adapter policy (default: PLANET_ADAPTER or software)
    --draw per-draw|indirect   how the terrain renderer submits draws (REND-015): per-draw is the reference loop, indirect one multi-draw per buffer run from a CPU-built argument buffer (default indirect where the adapter supports it, else per-draw)
    --tile-format vertex|compact   how terrain tiles are built and held (ADR 0033): compact (default) for levels 3 and finer, vertex the 44-byte reference format for all
    --perf                   performance-harness skeleton (needs --adapter hardware; the readiness gate decides, ADR 0020)
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
    pub cells: u32,
    /// `--finest-level N`: the LOD cap (`LodParams::max_level`); none = the production default for the tile size (LOD-010).
    pub finest_level: Option<u8>,
    pub tau: f64,
    pub frames: usize,
    pub perf_smoke: bool,
    /// `--store DIR`: the stored bake the streaming perf scenes read (the production source, GEN-014); `--allow-memory-bake` runs them over the in-memory bake of `--global-res` cells instead, labelled and not valid for streaming budgets.
    pub store: Option<PathBuf>,
    pub allow_memory_bake: bool,
    pub flight: Option<usize>,
    pub out_dir: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    /// Every option given on the command line, so modes can reject options they would otherwise ignore.
    pub given: std::collections::BTreeSet<String>,
}

impl Options {
    /// The finest geometry level of a run with tiles of `cells` cells: `--finest-level`, else the production default
    /// (`planet_frame::production_finest_level`: 22 at cells 32, 21 at 64). The one place the apps take the LOD cap from (LOD-010).
    pub fn finest_level_for(&self, cells: u32) -> u8 {
        self.finest_level.unwrap_or_else(|| planet_frame::production_finest_level(cells))
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
        global_products: None,
        global_res: 256,
        threads: 0,
        offscreen: false,
        script: "3".into(),
        stream: false,
        pipelined: false,
        then: None,
        cells: 32,
        finest_level: None,
        tau: 6.0,
        frames: 300,
        perf_smoke: false,
        store: None,
        allow_memory_bake: false,
        flight: None,
        out_dir: None,
        size: None,
        given: Default::default(),
    };
    while let Some(a) = it.next() {
        o.given.insert(a.clone());
        match a.as_str() {
            "--smoke" => o.smoke = true,
            "--perf" => o.perf = true,
            "--offscreen" => o.offscreen = true,
            "--stream" => o.stream = true,
            "--pipelined" => o.pipelined = true,
            "--then" => o.then = Some(it.next().ok_or("--then needs a script index or name")?.clone()),
            "--perf-smoke" => o.perf_smoke = true,
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
            "--cache-dir" => o.cache_dir = Some(PathBuf::from(it.next().ok_or("--cache-dir needs a directory")?)),
            "--verify-cache" => o.verify_cache = Some(PathBuf::from(it.next().ok_or("--verify-cache needs a directory")?)),
            "--compact-cache" => o.compact_cache = Some(PathBuf::from(it.next().ok_or("--compact-cache needs a directory")?)),
            "--cap-bytes" => {
                o.cap_bytes = Some(it.next().ok_or("--cap-bytes needs a byte count")?.parse().map_err(|e| format!("--cap-bytes: {e}"))?)
            }
            "--allow-stale" => o.allow_stale = true,
            "--global-dir" => o.global_dir = Some(PathBuf::from(it.next().ok_or("--global-dir needs a directory")?)),
            "--plan-check" => o.plan_check = Some(PathBuf::from(it.next().ok_or("--plan-check needs a directory")?)),
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
pub(crate) const STORE_OPTIONS: [&str; 14] = [
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
    } else if let Some(f) = FLIGHT_ONLY.iter().find(|f| o.given.contains(**f) && !(o.pipelined && **f == "--out-dir")) {
        return Err(format!("{f} only applies to --flight"));
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
    if !matches!(o.mode, Mode::Generate) {
        // The global-bake perf scene takes the definition, grid size and thread count of the global commands.
        let bake_scene = matches!(o.mode, Mode::TestRender) && o.perf && o.scene == "global-bake";
        let stream_scene = matches!(o.mode, Mode::TestRender) && (o.perf || o.perf_smoke) && streamperf::is_stream_scene(&o.scene);
        let terrain_scene = matches!(o.mode, Mode::TestRender) && o.scene == "terrain";
        let allowed: &[&str] = if bake_scene {
            &["--world", "--global-res", "--threads"]
        } else if terrain_scene || stream_scene {
            &["--world", "--global-res"]
        } else {
            &[]
        };
        if let Some(f) = STORE_OPTIONS.iter().find(|f| o.given.contains(**f) && !allowed.contains(f)) {
            return Err(format!("{f} only applies to `generate` (the tile store commands)"));
        }
    }
    let smoke = if o.smoke { " [smoke ok]" } else { "" };
    match o.mode {
        Mode::Editor if o.offscreen => skeleton::render_offscreen(&o, out),
        Mode::Editor => writeln!(out, "planet editor: no window yet (arrives in M2; use --offscreen for the skeleton){smoke}").map_err(err),
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
    for l in verdict.lines() {
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
    if let Some(format) = o.tile_format {
        ctx.prefer_tile_format(format);
    }
    writeln!(out, "tile format: {}", ctx.default_tile_format().name()).map_err(err)?;
    if o.perf && o.perf_smoke {
        return Err("--perf and --perf-smoke exclude each other: a smoke run is never a measurement".into());
    }
    if o.perf_smoke && o.scene != "terrain" && !streamperf::is_stream_scene(&o.scene) {
        return Err("--perf-smoke is only available for --scene terrain or a streaming scene (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, streaming)".into());
    }
    streamperf::check_options(o)?;
    if o.flight.is_some() {
        return skeleton::render_flight(&ctx, o, out);
    }
    if o.perf || o.perf_smoke {
        return match o.scene.as_str() {
            "terrain" => skeleton::perf_terrain(o, &ctx, out),
            "global-bake" => globalperf::perf_global_bake(o, &ctx, out),
            s if streamperf::is_stream_scene(s) => streamperf::perf_streaming(o, &ctx, out),
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
/// It times the hello-triangle round trip as a stand-in for a frame until the renderer exists (M2).
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
        let built = run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--threads", "2"]).unwrap();
        assert!(
            built.contains("8 cells per face edge in blocks of 8 (level 0)") && built.contains("6 blocks written, 0 already stored"),
            "{built}"
        );
        let again = run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3"]).unwrap();
        assert!(again.contains("0 blocks written, 6 already stored"), "{again}");
        let ok = run_str(&["generate", "--verify-global", &store, "--seed", "3"]).unwrap();
        assert!(ok.contains("checked 6 of 6 entries: 6 match"), "{ok}");
        let sampled = run_str(&["generate", "--verify-global", &store, "--seed", "3", "--sample", "5"]).unwrap();
        assert!(sampled.contains("checked 5 of 6 entries: 5 match"), "{sampled}");
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
            (vec!["editor", "--world", world.as_str()], "only applies to `generate`"),
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
        for mode in ["editor", "generate", "test-render"] {
            let out = run_str(&[mode, "--smoke"]).unwrap_or_else(|e| panic!("{mode}: {e}"));
            assert!(out.contains("smoke ok"), "{mode}: {out}");
        }
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
