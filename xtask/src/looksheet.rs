//! `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` (BUILD-067): the look sheets of a chain run's crop store (the `look_sheet` example of `planet-cache`).

use crate::util::{run, Res};
use std::path::Path;

const USAGE: &str = "usage: cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]";

/// The `cargo` arguments of the command.
pub fn plan(args: &[String]) -> Result<Vec<String>, String> {
    let mut tail = Vec::new();
    let mut runs = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--against" | "--az" | "--out" => {
                let v = it.next().filter(|v| !v.starts_with("--")).ok_or(format!("{a} needs a value ({USAGE})"))?;
                if tail.contains(a) {
                    return Err(format!("{a} given twice ({USAGE})"));
                }
                tail.extend([a.clone(), v.clone()]);
            }
            other if other.starts_with("--") => return Err(format!("unknown option '{other}' ({USAGE})")),
            other => runs.push(other.to_string()),
        }
    }
    let run_dir = match &runs[..] {
        [r] => r.clone(),
        [] => return Err(format!("RUN, a crop store, is required ({USAGE})")),
        _ => return Err(format!("one RUN, not {} ({USAGE})", runs.len())),
    };
    let mut cargo: Vec<String> =
        ["run", "--release", "-q", "-p", "planet-cache", "--example", "look_sheet", "--"].iter().map(|s| s.to_string()).collect();
    cargo.push(run_dir);
    cargo.extend(tail);
    Ok(cargo)
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let cargo = plan(args)?;
    let refs: Vec<&str> = cargo.iter().map(String::as_str).collect();
    // the example prints each sheet's path, size and bytes
    run(root, "cargo", &refs, &[])
}

#[cfg(test)]
mod tests {
    use super::plan;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-067
    #[test]
    fn the_command_runs_the_example_and_refuses_what_it_does_not_know() {
        let c = plan(&s(&["runs/sw", "--against", "runs/def", "--az", "315,135", "--out", "o"])).unwrap();
        assert_eq!(&c[..8], &s(&["run", "--release", "-q", "-p", "planet-cache", "--example", "look_sheet", "--"])[..]);
        assert_eq!(&c[8..], &s(&["runs/sw", "--against", "runs/def", "--az", "315,135", "--out", "o"])[..]);
        assert_eq!(&plan(&s(&["r"])).unwrap()[8..], &s(&["r"])[..], "no option: the example's defaults");
        assert!(plan(&s(&[])).unwrap_err().contains("RUN, a crop store, is required"));
        assert!(plan(&s(&["a", "b"])).unwrap_err().contains("one RUN, not 2"));
        assert!(plan(&s(&["a", "--against", "b", "--against", "c"])).unwrap_err().contains("twice"));
        assert!(plan(&s(&["a", "--step", "3"])).unwrap_err().contains("unknown option '--step'"));
        assert!(plan(&s(&["a", "--az"])).unwrap_err().contains("needs a value"));
    }
}
