//! `cargo xtask windows-draw [--out FILE] [--threads N]` (BUILD-066, lead #186a): G3-8's out-of-sample windows, a seeded stratified draw over the seed-14 world's coarse
//! field (the `windows_draw` example of `planet-cache`); writes `crates/cache/data/g3-windows.toml` unless `--out` names another file.

use crate::util::{run, Res};
use std::path::Path;

const USAGE: &str = "usage: cargo xtask windows-draw [--out FILE] [--threads N]";

/// The `cargo` arguments of the command.
pub fn plan(args: &[String]) -> Result<Vec<String>, String> {
    let mut cargo: Vec<String> =
        ["run", "--release", "-q", "-p", "planet-cache", "--example", "windows_draw", "--"].iter().map(|s| s.to_string()).collect();
    let mut it = args.iter();
    let mut seen = Vec::new();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" | "--threads" => {
                let v = it.next().filter(|v| !v.starts_with("--")).ok_or(format!("{a} needs a value ({USAGE})"))?;
                if seen.contains(a) {
                    return Err(format!("{a} given twice ({USAGE})"));
                }
                seen.push(a.clone());
                cargo.extend([a.clone(), v.clone()]);
            }
            other => return Err(format!("unknown option '{other}' ({USAGE})")),
        }
    }
    Ok(cargo)
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let cargo = plan(args)?;
    let refs: Vec<&str> = cargo.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])
}

#[cfg(test)]
mod tests {
    use super::plan;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-066
    #[test]
    fn the_command_runs_the_draw_example_and_refuses_what_it_does_not_know() {
        let c = plan(&s(&["--out", "w.toml", "--threads", "4"])).unwrap();
        assert_eq!(&c[..8], &s(&["run", "--release", "-q", "-p", "planet-cache", "--example", "windows_draw", "--"])[..]);
        assert_eq!(&c[8..], &s(&["--out", "w.toml", "--threads", "4"])[..]);
        assert_eq!(plan(&s(&[])).unwrap().len(), 8, "the default data file");
        assert!(plan(&s(&["--seed", "3"])).unwrap_err().contains("unknown option '--seed'"));
        assert!(plan(&s(&["--out"])).unwrap_err().contains("needs a value"));
        assert!(plan(&s(&["--out", "a", "--out", "b"])).unwrap_err().contains("twice"));
    }
}
