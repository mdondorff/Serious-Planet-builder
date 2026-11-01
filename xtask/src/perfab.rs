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
    let mut runner = TreeRunner { tree: root.to_path_buf(), base, spec: spec.clone(), out_dir: out_dir.clone() };
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
}

impl Runner for TreeRunner {
    fn run(&mut self, side: Side, round: usize) -> Outcome {
        let dir = if side == Side::Base { &self.base } else { &self.tree };
        let tag = format!("r{}-{}", round + 1, if side == Side::Base { "base" } else { "tree" });
        let log_path = dir.join("perf").join("logs").join(&self.spec.log);
        let _ = std::fs::remove_file(&log_path);
        eprintln!("A/B: {tag}: cargo xtask perf {}", self.spec.command.join(" "));
        let out = match Command::new("cargo").arg("xtask").arg("perf").args(&self.spec.command).current_dir(dir).output() {
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
