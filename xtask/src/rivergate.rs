//! `cargo xtask river-gate <store dir | window | r3a> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--no-key-check]` (BUILD-055): the river gate's numbers on a region. Runs the `river_gate` example of
//! `planet-cache` in release under the heavy lock: the first argument is a commit's store (counted over its tiles, on the window `--window`, default R3a) or a window
//! `face,i0,i1,j0,j1,seed` in regional cells (`r3a` names R3a's), measured on the start surface. The report is printed, never judged: the exit status is 0 when the run ends.

use crate::util::{run, Res};
use std::path::Path;

/// The options the example takes after the target, each with a value.
const OPTIONS: [&str; 5] = ["--window", "--margin", "--min-cells", "--d", "--threads"];

/// The example's arguments for the xtask's: the target becomes `--window` or `--store`; an unknown option, a missing value or a second window is refused.
pub fn example_args(args: &[String]) -> Result<Vec<String>, String> {
    let usage = "usage: cargo xtask river-gate <store dir | face,i0,i1,j0,j1,seed | r3a> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--no-key-check]";
    let Some(target) = args.first().filter(|a| !a.starts_with("--")) else { return Err(usage.to_string()) };
    let mut out = Vec::new();
    let window_target = target == "r3a" || target.split(',').count() == 6 && target.split(',').all(|x| x.parse::<usize>().is_ok());
    if window_target {
        if target != "r3a" {
            out.extend(["--window".to_string(), target.clone()]);
        }
    } else {
        out.extend(["--store".to_string(), target.clone()]);
    }
    let mut k = 1;
    while k < args.len() {
        let name = args[k].as_str();
        if name == "--no-key-check" {
            if window_target {
                return Err(format!("--no-key-check reads a store: the target is a window; {usage}"));
            }
            out.push(name.to_string());
            k += 1;
            continue;
        }
        if !OPTIONS.contains(&name) {
            return Err(format!("unknown river-gate option '{name}' (known: {}); {usage}", OPTIONS.join(", ")));
        }
        let Some(v) = args.get(k + 1).filter(|v| !v.starts_with("--")) else { return Err(format!("{name} takes a value; {usage}")) };
        if name == "--window" && window_target {
            return Err(format!("the target is a window already: --window goes with a store; {usage}"));
        }
        out.extend([name.to_string(), v.clone()]);
        k += 2;
    }
    Ok(out)
}

pub fn run_gate(root: &Path, args: &[String]) -> Res {
    let tail = example_args(args)?;
    let mut cmd: Vec<&str> = vec!["run", "--release", "-q", "-p", "planet-cache", "--example", "river_gate", "--"];
    cmd.extend(tail.iter().map(String::as_str));
    run(root, "cargo", &cmd, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-055
    #[test]
    fn the_target_is_a_window_or_a_store() {
        assert_eq!(example_args(&s(&["r3a"])).unwrap(), Vec::<String>::new());
        assert_eq!(
            example_args(&s(&["1,8056,8675,760,1379,14", "--d", "30"])).unwrap(),
            s(&["--window", "1,8056,8675,760,1379,14", "--d", "30"])
        );
        assert_eq!(
            example_args(&s(&["C:/DEV/SeriousPlanetData/worlds/r3a-e1", "--min-cells", "30"])).unwrap(),
            s(&["--store", "C:/DEV/SeriousPlanetData/worlds/r3a-e1", "--min-cells", "30"])
        );
        assert_eq!(example_args(&s(&["store", "--window", "2,1,2,3,4,5"])).unwrap(), s(&["--store", "store", "--window", "2,1,2,3,4,5"]));
    }

    // spec: BUILD-055
    #[test]
    fn a_missing_target_an_unknown_option_a_missing_value_and_two_windows_are_refused() {
        assert!(example_args(&[]).is_err());
        assert!(example_args(&s(&["--d", "3"])).is_err());
        assert!(example_args(&s(&["r3a", "--fast"])).unwrap_err().contains("unknown river-gate option '--fast'"));
        assert!(example_args(&s(&["r3a", "--d"])).unwrap_err().contains("--d takes a value"));
        assert!(example_args(&s(&["r3a", "--window", "1,2,3,4,5,6"])).unwrap_err().contains("a window already"));
        assert!(example_args(&s(&["r3a", "--no-key-check"])).unwrap_err().contains("--no-key-check reads a store"));
        assert_eq!(example_args(&s(&["p2e", "--no-key-check", "--d", "5"])).unwrap(), s(&["--store", "p2e", "--no-key-check", "--d", "5"]));
    }
}
