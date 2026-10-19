//! The perf preflight scene (TEST-040): `cargo xtask perf preflight`, or `test-render --perf --scene preflight`. It measures the host for
//! about 70 s (the readiness gate, three CPU calibrations 30 s apart, the load of the whole period) and lets
//! `planet_perf::preflight::decide` say whether a perf window may start. It renders nothing and writes no perf log.

use crate::{err, Options};
use planet_perf::preflight::{decide, PreflightFacts, ROUNDS, ROUND_SPACING_S};
use planet_render::GpuContext;
use std::io::Write;

pub const SCENE: &str = "preflight";

pub fn is_preflight_scene(name: &str) -> bool {
    name == SCENE
}

/// Measure, decide, print, and refuse (an `Err`) unless the machine is fit, the run is a smoke run or the owner overrode the gate.
pub fn perf_preflight(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    let smoke = o.perf_smoke;
    let rounds = if smoke { 1 } else { ROUNDS };
    if smoke {
        writeln!(out, "SMOKE RUN: one calibration round on any adapter; the preflight never refuses and is no measurement").map_err(err)?;
    }
    // The report's place is checked and made before the measurement: a bad `--out` must not cost the run.
    let report_path = match &o.out {
        Some(dir) => {
            let path = crate::streamperf::out_log_path(dir, "preflight.md")?;
            if path.file_name().is_none_or(|n| n != "preflight.md") {
                return Err(format!("--out {}: give a directory (the report is written there as preflight.md)", dir.display()));
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            Some(path)
        }
        None => None,
    };
    writeln!(out, "preflight: sampling the machine for about {} s ...", 10 + (rounds as u64 - 1) * ROUND_SPACING_S + 2).map_err(err)?;
    let sampler = planet_perf::load::LoadSampler::start(1);
    // The gate samples the machine for its own 10 s window; the calibration rounds follow it.
    let machine = planet_perf::gate::measure_machine(crate::facts(ctx));
    let gate = planet_perf::gate::readiness(&machine);
    let threads = crate::litperf::calibration_threads();
    let mut calibrations = Vec::new();
    for round in 0..rounds {
        if round > 0 {
            std::thread::sleep(std::time::Duration::from_secs(ROUND_SPACING_S));
        }
        calibrations.push(planet_perf::streaming::calibrate_cpu(threads));
    }
    let load = sampler.stop();
    let facts = PreflightFacts {
        gate,
        ac_power: machine.ac_power,
        power_plan: machine.power_scheme.clone(),
        calibrations,
        top: load.top_over_case(),
    };
    let verdict = decide(&facts);
    for l in &verdict.report {
        writeln!(out, "{l}").map_err(err)?;
    }
    if let Some(path) = &report_path {
        let text = format!("# Perf preflight\n\n```\n{}\n```\n", verdict.report.join("\n"));
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        writeln!(out, "report written to {}", path.display()).map_err(err)?;
    }
    if verdict.passed() || smoke {
        return Ok(());
    }
    if o.machine_ready {
        writeln!(out, "preflight FAILED (owner override --machine-ready): the window may start on the owner's word").map_err(err)?;
        return Ok(());
    }
    Err(format!("the preflight refuses the window:\n  {}", verdict.failures.join("\n  ")))
}
