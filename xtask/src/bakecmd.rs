//! `cargo xtask bake pass1 [--planet-res N] [--seed N] [--out DIR] ...` (m9-bake PR 4, GEN-100): pass 1 of the erosion layer at a reduced resolution, one cube face at a time,
//! into a layer store, then the preview maps of the carved base. The work is the `bake_pass1` example of planet-cache; this file picks the subcommand and runs it in release.
//! The example refuses a resolution above its coarse limit (a full-resolution bake needs the planner's go) and a concurrency above 1 (not measured yet).

use crate::util::{run, Res};
use std::path::Path;

/// The `cargo` arguments that run the example with the user's options.
pub fn cargo_args(args: &[String]) -> Res<Vec<String>> {
    match args.first().map(String::as_str) {
        Some("pass1") => {}
        Some(other) => return Err(format!("unknown bake step '{other}' (known: pass1)")),
        None => {
            return Err(
                "usage: cargo xtask bake pass1 [--planet-res N] [--seed N] [--out DIR] [--threads N] [--faces LIST] [--map-width W]".into(),
            )
        }
    }
    let mut v: Vec<String> =
        ["run", "--release", "-q", "-p", "planet-cache", "--example", "bake_pass1", "--"].iter().map(|s| s.to_string()).collect();
    v.extend(args[1..].iter().cloned());
    Ok(v)
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let a = cargo_args(args)?;
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // spec: GEN-100
    #[test]
    fn the_options_are_passed_to_the_example() {
        let c = cargo_args(&a(&["pass1", "--planet-res", "512", "--seed", "3"])).unwrap();
        assert!(c.windows(2).any(|w| w == ["--planet-res", "512"]) && c.windows(2).any(|w| w == ["--seed", "3"]), "{c:?}");
        assert!(c.contains(&"bake_pass1".to_string()), "{c:?}");
    }

    // spec: GEN-100
    #[test]
    fn an_unknown_step_is_refused_naming_what_is_known() {
        assert!(cargo_args(&a(&[])).unwrap_err().contains("usage"));
        assert!(cargo_args(&a(&["pass2"])).unwrap_err().contains("pass1"));
    }
}
