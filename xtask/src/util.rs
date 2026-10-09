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
