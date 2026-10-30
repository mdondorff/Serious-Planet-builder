use std::path::{Path, PathBuf};
use std::process::Command;

pub type Res<T = ()> = Result<T, String>;

/// Repository root: the folder holding `docs/constitution.md`, found from the current directory.
pub fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("current dir");
    loop {
        if dir.join("docs").join("constitution.md").is_file() {
            return dir;
        }
        if !dir.pop() {
            eprintln!("xtask: run inside the repository (docs/constitution.md not found above the current directory)");
            std::process::exit(2);
        }
    }
}

/// Where checks that were NOT RUN (a locked desktop, TEST-072) leave their markers, relative to the repository root.
pub const NOT_RUN_DIR: &str = "target/review/not-run";

/// The NOT RUN lines the markers hold, printed under `what` (nothing when there are none); returns how many.
pub fn say_not_run(root: &Path, what: &str) -> usize {
    let lines = planet_perf::desktop::markers(&root.join(NOT_RUN_DIR));
    if !lines.is_empty() {
        eprintln!("{what}: {} real-GPU check(s) NOT RUN while the desktop was locked (TEST-072); a NOT RUN is not a pass:", lines.len());
        for l in &lines {
            eprintln!("  {l}");
        }
    }
    lines.len()
}

/// Run a command in `root`, inheriting stdio. `Err` when it fails to start or exits non-zero.
pub fn run(root: &Path, program: &str, args: &[&str], env: &[(&str, &str)]) -> Res {
    eprintln!("$ {program} {}", args.join(" "));
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(root);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd.status().map_err(|e| format!("cannot start `{program}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program} {}` failed with {status}", args.join(" ")))
    }
}

/// Run a command and capture stdout. `Err` includes stderr on failure.
pub fn capture(root: &Path, program: &str, args: &[&str]) -> Res<String> {
    let out = Command::new(program).args(args).current_dir(root).output().map_err(|e| format!("cannot start `{program}`: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("`{program} {}` failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr)))
    }
}

/// All files under `dir` (recursively) with the given extension, skipping `target` and `.git`.
pub fn files_with_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() {
            if name != "target" && name != ".git" && name != "node_modules" {
                files_with_ext(&p, ext, out);
            }
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod not_run_tests {
    use super::*;

    // spec: TEST-072
    #[test]
    fn the_xtask_summary_counts_the_markers_of_checks_that_were_not_run() {
        let root = std::env::temp_dir().join(format!("xtask-notrun-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(say_not_run(&root, "test gpu --real"), 0, "no marker, no line");
        let dir = root.join(NOT_RUN_DIR);
        planet_perf::desktop::report("gate", "test lock", 1, 1, false, Some(&dir));
        planet_perf::desktop::report("gpu_window editor", "test lock", 1, 9, true, Some(&dir));
        assert_eq!(say_not_run(&root, "test gpu --real"), 2);
        planet_perf::desktop::clear(&dir);
        assert_eq!(say_not_run(&root, "test gpu --real"), 0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
