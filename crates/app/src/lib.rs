//! The single binary's run modes (CON-01): `editor` (interactive viewer), `generate` (headless CLI)
//! and `test-render` (headless harness). M0 only wires them up; real work arrives with M1+.

pub mod sheet;
pub mod skeleton;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use planet_perf::{AdapterFacts, DeviceKind, Protocol, RunReport};
use planet_render::{AdapterPolicy, GpuContext};

pub const USAGE: &str = "usage: planet <mode> [options]
modes:
  editor        interactive editor/viewer (window arrives in M2); --smoke starts and exits; --offscreen runs the skeleton without a window
  generate      headless generator CLI: --tile F,L,X,Y [--seed N --res N --out height.png --face-net net.png]; --manifest <file> writes the determinism manifest (GEN-003); --smoke starts and exits
  test-render   headless render harness
    --scene hello-triangle|tiles|terrain   render a scene (default hello-triangle)
    --script N|name          scripted terrain camera 0..9 or its name (default 3 = aerial-20km)
    --cells N --tau PX --frames N   terrain perf workload parameters (defaults 32, 6, 300)
    --perf-smoke             run the terrain perf path briefly on any adapter (labelled, never a measurement)
    --flight N --out-dir DIR [--size WxH]   render the scripted orbit-to-1 m flight (TEST-013) as DIR/frame_0000.png... plus DIR/flight.txt and DIR/contact_sheet.png
                             with the production LOD defaults (override with --cells, --tau; --view applies; default size 1280x720,
                             small sizes can fail LOD validation) for the real-GPU jitter review
    --stream                 generate terrain tiles asynchronously through the streaming path (same image as the default)
    --view face|tile-id|level|morph|height|normals|depth|relief   debug view (tiles scene: face, tile-id, height; a flight defaults to relief)
    --tile FACE,LEVEL,X,Y    tile to draw (repeatable; default: a tile and its east neighbour)
    --seed N --res N         generator seed and tile resolution (cells, power of two)
    --bundle <file>          write a repro bundle of the render
    --repro <file>           replay a repro bundle (see cargo xtask repro)
    --out <file.png>         write the image
    --adapter software|hardware   adapter policy (default: PLANET_ADAPTER or software)
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
    pub offscreen: bool,
    pub script: String,
    pub stream: bool,
    pub cells: u32,
    pub tau: f64,
    pub frames: usize,
    pub perf_smoke: bool,
    pub flight: Option<usize>,
    pub out_dir: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    /// Every option given on the command line, so modes can reject options they would otherwise ignore.
    pub given: std::collections::BTreeSet<String>,
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
        offscreen: false,
        script: "3".into(),
        stream: false,
        cells: 32,
        tau: 6.0,
        frames: 300,
        perf_smoke: false,
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
            "--perf-smoke" => o.perf_smoke = true,
            "--cells" => o.cells = it.next().ok_or("--cells needs a value")?.parse().map_err(|e| format!("--cells: {e}"))?,
            "--tau" => o.tau = it.next().ok_or("--tau needs a value")?.parse().map_err(|e| format!("--tau: {e}"))?,
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
            "--manifest" => o.manifest = Some(PathBuf::from(it.next().ok_or("--manifest needs a path")?)),
            "--face-net" => o.face_net = Some(PathBuf::from(it.next().ok_or("--face-net needs a path")?)),
            "--machine-ready" => o.machine_ready = true,
            "--scene" => o.scene = it.next().ok_or("--scene needs a value")?.clone(),
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
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

/// Options a `--flight` run would ignore, and the flight-only options other runs would ignore (TEST-013).
const FLIGHT_EXCLUDES: [&str; 12] = [
    "--perf",
    "--perf-smoke",
    "--repro",
    "--out",
    "--bundle",
    "--stream",
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
    } else if let Some(f) = FLIGHT_ONLY.iter().find(|f| o.given.contains(**f)) {
        return Err(format!("{f} only applies to --flight"));
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

/// The session verdict, printed before any budget verdict so a bad session never shows "ok" first (ADR 0020). An invalid
/// or overridden session's verdicts do not count.
pub(crate) fn session_verdict_line(
    out: &mut dyn Write,
    validity: &planet_perf::gate::Validity,
    owner_override: &[String],
) -> Result<(), String> {
    if validity.is_valid() && owner_override.is_empty() {
        return writeln!(out, "SESSION VALID for budgets").map_err(err);
    }
    writeln!(out, "SESSION INVALID for budgets: the verdicts below do not count").map_err(err)?;
    for r in &validity.reasons {
        writeln!(out, "  INVALID SESSION: {r}").map_err(err)?;
    }
    for r in owner_override {
        writeln!(out, "  INVALID SESSION: owner override of the readiness gate ({r})").map_err(err)?;
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
    let policy = o.adapter.unwrap_or_else(AdapterPolicy::from_env);
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    // Hybrid-graphics rule (report §8): always log which adapter we got.
    writeln!(out, "{}", ctx.adapter_log_line()).map_err(err)?;
    if o.perf && o.perf_smoke {
        return Err("--perf and --perf-smoke exclude each other: a smoke run is never a measurement".into());
    }
    if o.perf_smoke && o.scene != "terrain" {
        return Err("--perf-smoke is only available for --scene terrain".into());
    }
    if o.flight.is_some() {
        return skeleton::render_flight(&ctx, o, out);
    }
    if o.perf || o.perf_smoke {
        return if o.scene == "terrain" { skeleton::perf_terrain(o, &ctx, out) } else { perf_skeleton(o, &ctx, out) };
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
    session_verdict_line(out, &validity, &owner_override)?;
    writeln!(out, "run-to-run noise (max-min)/median: {:.3}", planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN)).map_err(err)?;
    let scheme = planet_perf::query_power_scheme();
    for w in planet_perf::session_warnings(&facts(ctx), scheme.as_deref(), &runs, false) {
        writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
    }
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
        assert_eq!(std::fs::read_to_string(&path).unwrap(), planet_generators::manifest::determinism_manifest());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert!(run_str(&["generate", "--manifest", "x.txt", "--tile", "0,0,0,0"]).unwrap_err().contains("excludes --tile"));
        assert!(run_str(&["generate", "--manifest"]).unwrap_err().contains("needs a path"));
    }
}
