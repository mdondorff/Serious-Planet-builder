//! `cargo xtask perf --ab <ref> [--rounds N] [--retries N] [--metrics a,b] [--log FILE] <perf command...>` (BUILD-033, ADR 0087): the working tree and a ref, run
//! alternately, one table. The rules are in `planet_perf::ab` (tier A on fake runs); this file only makes the worktree of the ref, runs `cargo xtask perf ...` in
//! it and in this tree, and keeps every run's output and log under `target/ab/runs/<time>/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use planet_perf::ab::{self, AbSpec, Outcome, Runner, Side};

use crate::util::{capture, Res};

/// How long a refused run waits before it is tried again.
const REFUSAL_WAIT: Duration = Duration::from_secs(30);

pub fn run(root: &Path, args: &[String]) -> Res {
    let spec = ab::parse(args)?;
    let commit = capture(root, "git", &["rev-parse", "--verify", &format!("{}^{{commit}}", spec.reference)])
        .map_err(|e| format!("cannot resolve the ref '{}': {e}", spec.reference))?
        .trim()
        .to_string();
    let base = root.join("target").join("ab").join("base");
    ensure_worktree(root, &base, &commit)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let out_dir = root.join("target").join("ab").join("runs").join(stamp.to_string());
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    eprintln!(
        "A/B: {} ({}) against the working tree, {} rounds; runs, logs and the table go to {}",
        spec.reference,
        &commit[..commit.len().min(10)],
        spec.rounds,
        out_dir.display()
    );
    let mut runner =
        TreeRunner { tree: root.to_path_buf(), base, spec: spec.clone(), out_dir: out_dir.clone(), extra: [Vec::new(), Vec::new()] };
    let cmp = ab::execute(&spec, &mut runner)?;
    let lines = ab::table(&spec, &cmp);
    for l in &lines {
        println!("{l}");
    }
    std::fs::write(out_dir.join("table.md"), lines.join("\n") + "\n").map_err(|e| format!("cannot write the table: {e}"))?;
    Ok(())
}

/// The worktree of the ref under `target/ab/base`: made on the first use, moved to `commit` on the next ones.
fn ensure_worktree(root: &Path, base: &Path, commit: &str) -> Res {
    let dir = base.to_string_lossy().into_owned();
    if base.join(".git").exists() {
        capture(base, "git", &["checkout", "--detach", "--quiet", commit]).map(|_| ())
    } else {
        std::fs::create_dir_all(base.parent().unwrap_or(root)).map_err(|e| format!("cannot create target/ab: {e}"))?;
        capture(root, "git", &["worktree", "add", "--detach", "--quiet", &dir, commit]).map(|_| ())
    }
}

struct TreeRunner {
    tree: PathBuf,
    base: PathBuf,
    spec: AbSpec,
    out_dir: PathBuf,
    /// Arguments added to the command of each side after the perf arguments: `[base, tree]` (the reserve comparison runs the tree twice, without and with `--reserve`).
    extra: [Vec<String>; 2],
}

impl Runner for TreeRunner {
    fn run(&mut self, side: Side, round: usize) -> Outcome {
        let dir = if side == Side::Base { &self.base } else { &self.tree };
        let tag = format!("r{}-{}", round + 1, if side == Side::Base { "base" } else { "tree" });
        let log_path = dir.join("perf").join("logs").join(&self.spec.log);
        let _ = std::fs::remove_file(&log_path);
        eprintln!("A/B: {tag}: cargo xtask perf {}", self.spec.command.join(" "));
        let out = match Command::new("cargo")
            .arg("xtask")
            .arg("perf")
            .args(&self.spec.command)
            .args(&self.extra[usize::from(side == Side::Tree)])
            .current_dir(dir)
            .output()
        {
            Ok(o) => o,
            Err(e) => return Outcome::Failed(format!("cannot start cargo in {}: {e}", dir.display())),
        };
        let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let _ = std::fs::write(self.out_dir.join(format!("{tag}.out.txt")), &text);
        if let Some(conditions) = ab::gate_refusal(&text) {
            return Outcome::Refused(conditions);
        }
        if !out.status.success() {
            let tail: Vec<&str> = text.lines().rev().filter(|l| !l.trim().is_empty()).take(4).collect();
            return Outcome::Failed(format!("{} ({})", out.status, tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
        }
        match std::fs::read_to_string(&log_path) {
            Ok(log) => {
                let _ = std::fs::write(self.out_dir.join(format!("{tag}.json")), &log);
                Outcome::Log(log)
            }
            Err(e) => Outcome::Failed(format!("the command wrote no {}: {e}", log_path.display())),
        }
    }

    fn wait(&mut self) {
        eprintln!("A/B: the readiness gate refused; trying again in {} s", REFUSAL_WAIT.as_secs());
        std::thread::sleep(REFUSAL_WAIT);
    }
}

/// The standard flights of the reserve test (BUDG-012): the gate's streaming flights.
const RESERVE_FLIGHTS: [&str; 5] = ["dive", "look-1m", "look-2km", "teleport-1m", "teleport-2km"];

/// The metrics the reserve comparison reads: the frame, tile-ready, and terrain's own readings with the loads on.
const RESERVE_METRICS: [&str; 7] = [
    "median_ms",
    "p99_ms",
    "load_run_us_median",
    "reserve_terrain_main_us_median",
    "reserve_terrain_gpu_us_median",
    "reserve_terrain_peak_threads",
    "reserve_spin_us_median",
];

/// `cargo xtask perf reserve [--rounds N] [--retries N] [--lit] [--store DIR] [--loaded] ...` (BUDG-012): for each standard flight the streaming scene without the synthetic
/// loads and with them (`--reserve`), alternating for N rounds in this tree, one table per flight and the reserve report of the loads-on runs. Every other option goes to the
/// scene. The planner gives the go for a measured run (ADR 0087); the run itself is a loaded or a quiet one by the options.
pub fn reserve(root: &Path, args: &[String]) -> Res {
    let (mut rounds, mut retries, mut passthrough) = (3usize, ab::DEFAULT_RETRIES, Vec::<String>::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rounds" => rounds = it.next().and_then(|v| v.parse().ok()).ok_or("--rounds needs a count")?,
            "--retries" => retries = it.next().and_then(|v| v.parse().ok()).ok_or("--retries needs a count")?,
            "--reserve" => return Err("perf reserve adds --reserve itself".into()),
            _ => passthrough.push(a.clone()),
        }
    }
    let lit = passthrough.iter().any(|a| a == "--lit");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let out_dir = root.join("target").join("ab").join("reserve").join(stamp.to_string());
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let mut all_lines: Vec<String> = Vec::new();
    for flight in RESERVE_FLIGHTS {
        let mut command = vec!["stream".to_string(), flight.to_string()];
        command.extend(passthrough.iter().cloned());
        let spec = AbSpec {
            reference: "no loads".into(),
            rounds,
            retries,
            metrics: RESERVE_METRICS.iter().map(ToString::to_string).collect(),
            log: format!("{flight}{}.json", if lit { "-lit" } else { "" }),
            command,
        };
        let flight_dir = out_dir.join(flight);
        std::fs::create_dir_all(&flight_dir).map_err(|e| format!("cannot create {}: {e}", flight_dir.display()))?;
        let mut runner = TreeRunner {
            tree: root.to_path_buf(),
            base: root.to_path_buf(),
            spec: spec.clone(),
            out_dir: flight_dir.clone(),
            extra: [Vec::new(), vec!["--reserve".to_string()]],
        };
        let cmp = ab::execute(&spec, &mut runner)?;
        let mut lines = vec![format!("== {flight}: loads off (ref column) against loads on (tree column)")];
        lines.extend(ab::table(&spec, &cmp));
        // The reserve report of the last loads-on run, as the scene printed it.
        if let Ok(text) = std::fs::read_to_string(flight_dir.join(format!("r{rounds}-tree.out.txt"))) {
            lines.extend(text.lines().filter(|l| l.contains("reserve")).map(str::to_string));
        }
        for l in &lines {
            println!("{l}");
        }
        all_lines.extend(lines);
    }
    std::fs::write(
        out_dir.join("reserve.md"),
        all_lines.join(
            "
",
        ) + "
",
    )
    .map_err(|e| format!("cannot write the report: {e}"))?;
    eprintln!("reserve report: {}", out_dir.join("reserve.md").display());
    Ok(())
}
