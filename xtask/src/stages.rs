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

/// BUILD-048: the largest committed sheet, in bytes (CLAUDE.md: over 1 MB needs an ADR).
pub const SHEET_LIMIT: u64 = 1_000_000;
/// BUILD-048: the largest total of the sheets of one gate, in bytes.
pub const GATE_LIMIT: u64 = 5_000_000;

/// BUILD-048: one error per sheet over `SHEET_LIMIT` and per gate whose sheets total over `GATE_LIMIT`, naming the file or gate, its size and the limit.
/// `sheets` is `(gate, file, bytes)`; every file under `docs/stages/sheets/<gate>/` counts.
pub fn sheet_size_errors(sheets: &[(String, String, u64)]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut totals: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for (gate, file, bytes) in sheets {
        *totals.entry(gate.as_str()).or_default() += bytes;
        if *bytes > SHEET_LIMIT {
            errors.push(format!(
                "{file}: {bytes} B is over the limit of {SHEET_LIMIT} B per sheet (BUILD-048): shrink the sheet or probe fewer bins"
            ));
        }
    }
    for (gate, total) in totals {
        if total > GATE_LIMIT {
            errors.push(format!(
                "docs/stages/sheets/{gate}/: the sheets total {total} B, over the limit of {GATE_LIMIT} B per gate (BUILD-048): commit fewer sheets"
            ));
        }
    }
    errors
}

/// The committed sheets under `docs/stages/sheets/<gate>/`, as `(gate, path, bytes)`; none when the folder does not exist.
pub fn committed_sheets(root: &Path) -> Vec<(String, String, u64)> {
    let mut out = Vec::new();
    let Ok(gates) = std::fs::read_dir(root.join("docs/stages/sheets")) else { return out };
    for g in gates.flatten().filter(|g| g.path().is_dir()) {
        let gate = g.file_name().to_string_lossy().to_string();
        let mut stack = vec![g.path()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(md) = e.metadata() {
                    let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                    out.push((gate.clone(), rel, md.len()));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-048
    #[test]
    fn a_sheet_over_one_megabyte_or_a_gate_over_five_fails_naming_file_size_and_limit() {
        let s = |g: &str, f: &str, b: u64| (g.to_string(), format!("docs/stages/sheets/{g}/{f}"), b);
        assert!(sheet_size_errors(&[]).is_empty());
        assert!(sheet_size_errors(&[s("g1", "a.png", SHEET_LIMIT), s("g1", "b.png", SHEET_LIMIT)]).is_empty(), "at the limits passes");
        let e = sheet_size_errors(&[s("g1", "a.png", SHEET_LIMIT + 1)]);
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("g1/a.png") && e[0].contains("1000001") && e[0].contains("1000000"), "{e:?}");
        let six: Vec<_> = (0..6).map(|i| s("g2", &format!("{i}.png"), 900_000)).collect();
        let e = sheet_size_errors(&six);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("g2") && e[0].contains("5400000") && e[0].contains("5000000"), "{e:?}");
        let exact: Vec<_> = (0..5).map(|i| s("g5", &format!("{i}.png"), 1_000_000)).collect();
        assert!(sheet_size_errors(&exact).is_empty(), "a gate at exactly its limit passes");
        let other = sheet_size_errors(&[
            s("g3", "a.png", 900_000),
            s("g3", "b.png", 900_000),
            s("g4", "a.png", 900_000),
            s("g4", "b.png", 900_000),
            s("g4", "c.png", 900_000),
            s("g4", "d.png", 900_000),
            s("g4", "e.png", 900_000),
        ]);
        assert!(other.is_empty(), "the total is per gate: {other:?}");
    }

    // spec: BUILD-048
    #[test]
    fn the_collector_reads_every_file_under_a_gate_folder() {
        let dir = std::env::temp_dir().join(format!("sheets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("docs/stages/sheets/g9/sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("docs/stages/sheets/g9/a.png"), vec![0u8; 10]).unwrap();
        std::fs::write(sub.join("b.png"), vec![0u8; 20]).unwrap();
        let mut got = committed_sheets(&dir);
        got.sort();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("g9".to_string(), "docs/stages/sheets/g9/a.png".to_string(), 10));
        assert_eq!(got[1].1, "docs/stages/sheets/g9/sub/b.png");
        assert!(committed_sheets(&dir.join("none")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        for stage in ["landmass", "round-00", "round-01", "h1", "thermal", "deposition", "pit-spreading", "k7-f1-t3-4"] {
            assert!(text.lines().any(|l| l.starts_with(&format!("{stage} | "))), "no line for {stage}");
        }
        assert!(text.lines().all(|l| l.contains("| hash ") && !l.contains("probe_ms")), "a hash on every line and no cost field");
    }
}
