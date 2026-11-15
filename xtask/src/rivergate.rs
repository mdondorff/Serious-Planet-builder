//! `cargo xtask river-gate <store dir | window> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--no-key-check]` (BUILD-055): the river gate's numbers on a region. Runs the `river_gate` example of
//! `planet-cache` in release under the heavy lock: the first argument is a commit's store (counted over its tiles, on the window `--window`, default R1h) or a window, measured on the start
//! surface. A window is `face,i0,i1,j0,j1,seed` in regional cells or one of the canonical names of [`WINDOWS`] (lead, 2026-11-14). The report is printed, never judged: the exit status is 0 when the run ends.

use crate::util::{run, Res};
use std::path::Path;

/// The options the example takes after the target, each with a value.
const OPTIONS: [&str; 5] = ["--window", "--margin", "--min-cells", "--d", "--threads"];

/// The canonical region windows (lead, 2026-11-14): `face,i0,i1,j0,j1,seed`, 620-cell cores (with the gate's 200-cell margin SPIKE-111's 1,020-cell windows). R1h is the E1 region stores'
/// window (r3a-e1, r3a-p2, r3a-p2e). M1 (mountain belt) and F1 (low-gradient river landscape) are SPIKE-119's (spikes, 2026-11-15).
pub const WINDOWS: [(&str, &str); 5] = [
    ("r1h", "1,8056,8675,760,1379,14"),
    ("r3a", "4,368,987,4720,5339,15"),
    ("r3b", "2,9968,10587,8304,8923,15"),
    ("m1", "4,6600,7219,10200,10819,15"),
    ("f1", "4,4800,5419,3300,3919,15"),
];

/// A window argument: a canonical name (any case) becomes its numbers; anything else is passed as written.
pub fn window_of(arg: &str) -> String {
    WINDOWS.iter().find(|(n, _)| n.eq_ignore_ascii_case(arg)).map_or_else(|| arg.to_string(), |(_, w)| w.to_string())
}

/// The example's arguments for the xtask's: the target becomes `--window` or `--store`; an unknown option, a missing value or a second window is refused.
pub fn example_args(args: &[String]) -> Result<Vec<String>, String> {
    let usage = "usage: cargo xtask river-gate <store dir | face,i0,i1,j0,j1,seed | r1h | r3a | r3b | m1 | f1> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--no-key-check]";
    let Some(target) = args.first().filter(|a| !a.starts_with("--")) else { return Err(usage.to_string()) };
    let mut out = Vec::new();
    let window = window_of(target);
    let window_target = window.split(',').count() == 6 && window.split(',').all(|x| x.parse::<usize>().is_ok());
    if window_target {
        out.extend(["--window".to_string(), window]);
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
        out.extend([name.to_string(), if name == "--window" { window_of(v) } else { v.clone() }]);
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
        // The canonical names (lead, 2026-11-14): R1h is the E1 region stores' window; R3a and R3b are SPIKE-111's face 4 and face 2 windows of seed 15.
        assert_eq!(example_args(&s(&["r1h"])).unwrap(), s(&["--window", "1,8056,8675,760,1379,14"]));
        assert_eq!(example_args(&s(&["r3a"])).unwrap(), s(&["--window", "4,368,987,4720,5339,15"]));
        assert_eq!(example_args(&s(&["M1"])).unwrap(), s(&["--window", "4,6600,7219,10200,10819,15"]));
        assert_eq!(example_args(&s(&["f1"])).unwrap(), s(&["--window", "4,4800,5419,3300,3919,15"]));
        assert_eq!(example_args(&s(&["R3B"])).unwrap(), s(&["--window", "2,9968,10587,8304,8923,15"]));
        assert_eq!(
            example_args(&s(&["store", "--window", "r1h"])).unwrap(),
            s(&["--store", "store", "--window", "1,8056,8675,760,1379,14"])
        );
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
