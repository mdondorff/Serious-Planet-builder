//! `cargo xtask transects --chain DIR [--face F] [--out FILE]` (BUILD-061): the cross-section sheet of a chain directory (the `transect_sheet` example of `planet-cache`).

use crate::util::{run, Res};
use std::path::Path;

const USAGE: &str = "usage: cargo xtask transects --chain DIR [--face F] [--out FILE]";

/// The `cargo` arguments of the command.
pub fn plan(args: &[String]) -> Result<Vec<String>, String> {
    let mut tail = Vec::new();
    let mut dir = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--chain" | "--face" | "--out" => {
                let v = it.next().filter(|v| !v.starts_with("--")).ok_or(format!("{a} needs a value ({USAGE})"))?;
                if a == "--chain" {
                    if dir.replace(v.clone()).is_some() {
                        return Err(format!("--chain given twice ({USAGE})"));
                    }
                } else {
                    tail.extend([a.clone(), v.clone()]);
                }
            }
            other => return Err(format!("unknown option '{other}' ({USAGE})")),
        }
    }
    let dir = dir.ok_or(format!("--chain DIR is required ({USAGE})"))?;
    let mut cargo: Vec<String> =
        ["run", "--release", "-q", "-p", "planet-cache", "--example", "transect_sheet", "--"].iter().map(|s| s.to_string()).collect();
    cargo.push(dir);
    cargo.extend(tail);
    Ok(cargo)
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let cargo = plan(args)?;
    let refs: Vec<&str> = cargo.iter().map(String::as_str).collect();
    // the example prints the sheet's and the table's paths
    run(root, "cargo", &refs, &[])
}

#[cfg(test)]
mod tests {
    use super::plan;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-061
    #[test]
    fn the_command_runs_the_example_and_refuses_what_it_does_not_know() {
        let c = plan(&s(&["--chain", "d", "--face", "4", "--out", "o.png"])).unwrap();
        assert_eq!(&c[..8], &s(&["run", "--release", "-q", "-p", "planet-cache", "--example", "transect_sheet", "--"])[..]);
        assert_eq!(&c[8..], &s(&["d", "--face", "4", "--out", "o.png"])[..]);
        assert!(plan(&s(&[])).unwrap_err().contains("--chain DIR is required"));
        assert!(plan(&s(&["--chain", "a", "--chain", "b"])).unwrap_err().contains("twice"));
        assert!(plan(&s(&["--chain", "a", "--step", "3"])).unwrap_err().contains("unknown option '--step'"));
        assert!(plan(&s(&["--chain"])).unwrap_err().contains("needs a value"));
    }
}
