//! `cargo xtask perf window <plan> [--no-preflight]` (BUILD-034, ADR 0087 point 7, SPIKE-95): one process runs a declared window plan. It
//! builds the release binary once, runs the window's preflight (the readiness gate and the CPU calibration, TEST-040) and stops on a
//! refusal, then runs the plan's configurations and flights in rotated rounds with the duty idle between runs (BUILD-035), bounded
//! retries and the stop rule. The rules are in `planet_perf::window` (tier A on fake runs); this file starts the children, sleeps and
//! keeps every run's output and log, the attempts with their times and the table under `target/perf-window/<time>/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use planet_perf::ab::{self, Outcome};
use planet_perf::window::{self, ChildRun, Config, Flight, WindowRunner};

use crate::util::Res;

/// How long a refused run waits before it is tried again (as the A/B helper).
const REFUSAL_WAIT: Duration = Duration::from_secs(30);

fn now_s() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let (mut plan_path, mut preflight) = (None::<PathBuf>, true);
    for a in args {
        match a.as_str() {
            "--no-preflight" => preflight = false,
            other if other.starts_with("--") => return Err(format!("unknown perf window option '{other}' (known: --no-preflight)")),
            other => plan_path = Some(PathBuf::from(other)),
        }
    }
    let plan_path = plan_path.ok_or("usage: cargo xtask perf window <plan file> [--no-preflight]")?;
    let text = std::fs::read_to_string(&plan_path).map_err(|e| format!("cannot read the plan {}: {e}", plan_path.display()))?;
    let plan = window::parse_plan(&text)?;
    let out_dir = root.join("target").join("perf-window").join(format!("{}", now_s() as u64));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    std::fs::write(out_dir.join("plan.txt"), &text).map_err(|e| format!("cannot keep the plan: {e}"))?;
    eprintln!(
        "perf window: {} run(s) at most; runs, logs and the table go to {}",
        plan.rounds * plan.configs.len() * plan.flights.len(),
        out_dir.display()
    );
    // One build: the children's `cargo run --release` then only checks.
    let started = now_s();
    status(root, "cargo", &["build", "--release", "-p", "planet"], &[])?;
    eprintln!("perf window: release build in {:.0} s", now_s() - started);
    if preflight {
        let dir = out_dir.to_string_lossy().into_owned();
        status(root, "cargo", &["xtask", "perf", "preflight", "--out", &dir], &[])
            .map_err(|e| format!("the window's preflight refused, so the window does not start (see {dir}/preflight.md): {e}"))?;
    }
    let mut runner = Children { root: root.to_path_buf(), out_dir: out_dir.clone() };
    let report = window::execute(&plan, &mut runner);
    let lines = window::table(&plan, &report);
    for l in &lines {
        println!("{l}");
    }
    // The table is written before a failure is reported: the attempts before it are kept (BUILD-034).
    std::fs::write(out_dir.join("table.md"), lines.join("\n") + "\n").map_err(|e| format!("cannot write the table: {e}"))?;
    match report.failure {
        Some(f) => Err(format!("the window stopped: {f}")),
        None => Ok(()),
    }
}

/// Run a command to its end with its output on the console; a non-zero exit is an error.
fn status(root: &Path, program: &str, args: &[&str], env: &[(String, String)]) -> Res {
    let st = Command::new(program)
        .args(args)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("{program} {} exited with {st}", args.join(" ")))
    }
}

struct Children {
    root: PathBuf,
    out_dir: PathBuf,
}

impl WindowRunner for Children {
    fn run(&mut self, flight: &Flight, config: &Config, tag: &str) -> ChildRun {
        let log_path = self.root.join("perf").join("logs").join(&flight.log);
        let _ = std::fs::remove_file(&log_path);
        eprintln!("perf window: {tag}: cargo xtask perf {}", flight.command.join(" "));
        let start_s = now_s();
        let out = Command::new("cargo")
            .arg("xtask")
            .arg("perf")
            .args(&flight.command)
            .envs(config.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(&self.root)
            .output();
        let end_s = now_s();
        let outcome = match out {
            Err(e) => Outcome::Failed(format!("cannot start cargo: {e}")),
            Ok(out) => {
                let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                let _ = std::fs::write(self.out_dir.join(format!("{tag}.out.txt")), &text);
                if let Some(conditions) = ab::gate_refusal(&text) {
                    Outcome::Refused(conditions)
                } else if !out.status.success() {
                    let tail: Vec<&str> = text.lines().rev().filter(|l| !l.trim().is_empty()).take(4).collect();
                    Outcome::Failed(format!("{} ({})", out.status, tail.into_iter().rev().collect::<Vec<_>>().join(" | ")))
                } else {
                    match std::fs::read_to_string(&log_path) {
                        Ok(log) => {
                            let _ = std::fs::write(self.out_dir.join(format!("{tag}.json")), &log);
                            Outcome::Log(log)
                        }
                        // A run that wrote no log never counts (window 1: a script copied the previous run's log).
                        Err(e) => Outcome::Failed(format!("the command wrote no {}: {e}", log_path.display())),
                    }
                }
            }
        };
        ChildRun { outcome, start_s, end_s }
    }

    fn rest(&mut self, seconds: f64) {
        eprintln!("perf window: duty idle {seconds:.0} s (BUILD-035)");
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }

    fn wait_refused(&mut self) {
        eprintln!("perf window: the readiness gate refused; trying again in {} s", REFUSAL_WAIT.as_secs());
        std::thread::sleep(REFUSAL_WAIT);
    }
}
