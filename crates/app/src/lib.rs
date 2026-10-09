//! The single binary's run modes (CON-01): `editor` (interactive viewer), `generate` (headless CLI)
//! and `test-render` (headless harness). M0 only wires them up; real work arrives with M1+.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use planet_perf::{AdapterFacts, DeviceKind, Protocol, RunReport};
use planet_render::{AdapterPolicy, GpuContext};

pub const USAGE: &str = "usage: planet <mode> [options]
modes:
  editor        interactive editor/viewer (window arrives in M2); --smoke starts and exits
  generate      headless generator CLI; --smoke starts and exits
  test-render   headless render harness
    --scene hello-triangle   render a scene (default)
    --out <file.png>         write the image
    --adapter software|hardware   adapter policy (default: PLANET_ADAPTER or software)
    --perf                   performance-harness skeleton (needs --adapter hardware and --machine-ready)
    --machine-ready          the owner confirms charger, power profile and discrete GPU mode
    --smoke                  start and exit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Generate,
    TestRender,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub smoke: bool,
    pub scene: String,
    pub out: Option<PathBuf>,
    pub adapter: Option<AdapterPolicy>,
    pub perf: bool,
    pub machine_ready: bool,
}

pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut it = args.iter();
    let mode = match it.next().map(String::as_str) {
        Some("editor") | None => Mode::Editor,
        Some("generate") => Mode::Generate,
        Some("test-render") => Mode::TestRender,
        Some(other) => return Err(format!("unknown mode '{other}'\n{USAGE}")),
    };
    let mut o = Options { mode, smoke: false, scene: "hello-triangle".into(), out: None, adapter: None, perf: false, machine_ready: false };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke" => o.smoke = true,
            "--perf" => o.perf = true,
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

pub fn run(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let o = parse_args(args)?;
    let smoke = if o.smoke { " [smoke ok]" } else { "" };
    match o.mode {
        Mode::Editor => writeln!(out, "planet editor: no window yet (arrives in M2){smoke}").map_err(err),
        Mode::Generate => writeln!(out, "planet generate: no generators wired yet{smoke}").map_err(err),
        Mode::TestRender => test_render(&o, out),
    }
}

fn err(e: std::io::Error) -> String {
    e.to_string()
}

fn facts(ctx: &GpuContext) -> AdapterFacts {
    AdapterFacts {
        name: ctx.info.name.clone(),
        backend: format!("{:?}", ctx.info.backend),
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
    let policy = o.adapter.unwrap_or_else(AdapterPolicy::from_env);
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    // Hybrid-graphics rule (report §8): always log which adapter we got.
    writeln!(out, "adapter: {:?} {} ({:?}), key {}", ctx.info.backend, ctx.info.name, ctx.info.device_type, ctx.adapter_key())
        .map_err(err)?;
    if o.perf {
        return perf_skeleton(o, &ctx, out);
    }
    match o.scene.as_str() {
        "hello-triangle" => {
            let frame = planet_render::hello_triangle(&ctx, 256);
            if let Some(path) = &o.out {
                frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
                writeln!(out, "wrote {}", path.display()).map_err(err)?;
            }
            Ok(())
        }
        other => Err(format!("unknown scene '{other}' (known: hello-triangle)")),
    }
}

/// Performance-harness skeleton: adapter check, power/clock logging, warm-up, medians, JSON log.
/// It times the hello-triangle round trip as a stand-in for a frame until the renderer exists (M2).
fn perf_skeleton(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&facts(ctx)).map_err(|r| r.to_string())?;
    if !o.machine_ready {
        return Err("--machine-ready is required: the owner must confirm charger, performance power profile and discrete GPU mode".into());
    }
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
        });
    }
    let medians: Vec<f64> = runs.iter().map(|r| r.stats.median).collect();
    writeln!(out, "run-to-run noise (max-min)/median: {:.3}", planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN)).map_err(err)?;
    let json = planet_perf::report_json(&facts(ctx), planet_perf::query_power_scheme().as_deref(), o.machine_ready, &protocol, &runs);
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
}
