//! `cargo xtask stages [--update]` (BUILD-047): the stage manifest `docs/stages/manifest.txt`, the data hashes and key numbers of the stage probe's fixture. Without `--update` it runs the check
//! (a stale manifest fails, naming the stage, field and column); with `--update` it rewrites the file from the run, which is the one place the manifest changes. The fixture and the check are the
//! tier-A test `stage_manifest::the_manifest_is_the_run_of_the_fixture` of `planet-generators`; the sheets are written to `target/review/stages/ci/`.

use crate::util::{run, Res};
use std::path::Path;

/// The `cargo` arguments and the environment variables of the command.
type Plan = (Vec<String>, Vec<(&'static str, &'static str)>);

pub fn plan(args: &[String]) -> Result<Plan, String> {
    let mut update = false;
    for a in args {
        match a.as_str() {
            "--update" => update = true,
            other => return Err(format!("unknown option '{other}' (usage: cargo xtask stages [--update])")),
        }
    }
    let cargo = ["nextest", "run", "-p", "planet-generators", "--no-fail-fast", "stage_manifest"].iter().map(|s| s.to_string()).collect();
    Ok((cargo, if update { vec![("PLANET_UPDATE_STAGES", "1")] } else { Vec::new() }))
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let (cargo, env) = plan(args)?;
    let refs: Vec<&str> = cargo.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &env)?;
    if env.is_empty() {
        println!("stages: docs/stages/manifest.txt is the run of the fixture; the sheets are in target/review/stages/ci/");
    } else {
        println!("stages: docs/stages/manifest.txt rewritten from the run; review the diff and commit it");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-047
    #[test]
    fn the_command_runs_the_fixture_test_and_only_update_sets_the_variable() {
        let (c, env) = plan(&[]).unwrap();
        assert!(c.contains(&"stage_manifest".to_string()) && c.contains(&"planet-generators".to_string()), "{c:?}");
        assert!(env.is_empty(), "the check does not rewrite the manifest");
        let (_, env) = plan(&["--update".to_string()]).unwrap();
        assert_eq!(env, [("PLANET_UPDATE_STAGES", "1")]);
        assert!(plan(&["--force".to_string()]).unwrap_err().contains("usage"));
    }

    // spec: BUILD-047
    #[test]
    fn the_committed_manifest_exists_with_a_line_for_every_stage() {
        let text = std::fs::read_to_string(crate::util::repo_root().join("docs/stages/manifest.txt")).expect("docs/stages/manifest.txt");
        for stage in ["landmass", "round-00", "round-01", "h1", "thermal", "deposition", "pit-spreading"] {
            assert!(text.lines().any(|l| l.starts_with(&format!("{stage} | "))), "no line for {stage}");
        }
        assert!(text.lines().all(|l| l.contains("| hash ") && !l.contains("probe_ms")), "a hash on every line and no cost field");
    }
}
