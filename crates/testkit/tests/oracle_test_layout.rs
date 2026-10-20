//! BUILD-015 (tier B): a crate that builds one test binary per tier loses no test. The test names `cargo nextest list` reports for its
//! `it`, `oracle_all` and `gpu_all` binaries are exactly the `#[test]` functions of its test files (a test marked `#[ignore]` is not
//! listed), each as `<file>::<function>`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The identifiers a macro invocation `NAME! { ident: ...; ident: ...; }` defines: each line of the block that starts with
/// `ident:`.
fn macro_invocation_names(text: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with(&format!("{name}! {{")) {
            inside = true;
        } else if inside && line.starts_with('}') {
            inside = false;
        } else if inside {
            if let Some((ident, _)) = line.split_once(':') {
                if !ident.is_empty() && ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.push(ident.to_string());
                }
            }
        }
    }
    out
}

/// `<file>::<function>` of every `#[test]` of the test files of `krate` that is not `#[ignore]`d.
fn declared_tests(krate: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let aggregators = ["it", "oracle_all", "gpu_all"];
    for entry in std::fs::read_dir(krate.join("tests")).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".rs").filter(|s| !aggregators.contains(s)) else { continue };
        let text = std::fs::read_to_string(entry.path()).unwrap();
        // A file compiled only with a feature (`#![cfg(feature = "...")]`, the SPIKE-07 checks) lists nothing without it.
        if text.lines().any(|l| l.trim_start().starts_with("#![cfg(feature")) {
            continue;
        }
        let (mut pending, mut ignored) = (false, false);
        let mut macro_name = String::new();
        for line in text.lines().map(str::trim) {
            if let Some(rest) = line.strip_prefix("macro_rules! ") {
                macro_name = rest.split_whitespace().next().unwrap_or("").to_string();
            }
            if line == "#[test]" {
                (pending, ignored) = (true, false);
            } else if pending && line.starts_with("#[") {
                ignored |= line.starts_with("#[ignore");
            } else if pending && !line.starts_with("//") {
                if let Some(rest) = line.strip_prefix("fn ") {
                    let name = rest.split('(').next().unwrap().trim();
                    if ignored {
                    } else if name.starts_with('$') {
                        // A test defined by a macro (`fn $name()`): its names are the `name:` entries of the macro's invocation.
                        for generated in macro_invocation_names(&text, &macro_name) {
                            out.insert(format!("{stem}::{generated}"));
                        }
                    } else {
                        out.insert(format!("{stem}::{name}"));
                    }
                }
                pending = false;
            }
        }
    }
    out
}

/// The tests `cargo nextest list` reports for the per-tier binaries of every package, as `<file>::<function>` per package. One call for
/// the whole workspace: a call per package costs 6 to 25 s of cargo start-up and binary listing each (six converted crates: 100 to 180 s on
/// the CI runners, the 180 s timeout of the default profile, the CI runs of 2026-10-19 to 20), and `--workspace` is the resolution of the run that built the binaries.
fn listed_tests() -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let out = Command::new("cargo")
        .args([
            "nextest",
            "list",
            "--workspace",
            "-E",
            "binary(it) or binary(oracle_all) or binary(gpu_all)",
            "--message-format",
            "oneline",
        ])
        .current_dir(repo_root())
        .output()
        .expect("cargo nextest list");
    assert!(out.status.success(), "cargo nextest list --workspace: {}", String::from_utf8_lossy(&out.stderr));
    let mut by_package: std::collections::BTreeMap<String, BTreeSet<String>> = std::collections::BTreeMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some((binary, test)) = line.split_once(' ') else { continue };
        let Some((pkg, bin)) = binary.split_once("::") else { continue };
        if ["it", "oracle_all", "gpu_all"].contains(&bin) {
            by_package.entry(pkg.to_string()).or_default().insert(test.trim().to_string());
        }
    }
    by_package
}

// spec: BUILD-015
#[test]
fn a_crate_with_one_test_binary_per_tier_lists_exactly_the_test_functions_of_its_test_files() {
    let listed_all = listed_tests();
    let mut checked = Vec::new();
    for krate in std::fs::read_dir(repo_root().join("crates")).unwrap().flatten() {
        let manifest = std::fs::read_to_string(krate.path().join("Cargo.toml")).unwrap_or_default();
        if !manifest.contains("autotests = false") {
            continue;
        }
        let pkg = manifest.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|r| r.split('"').next()).unwrap().to_string();
        let (declared, listed) = (declared_tests(&krate.path()), listed_all.get(&pkg).cloned().unwrap_or_default());
        let missing: Vec<_> = declared.difference(&listed).collect();
        let extra: Vec<_> = listed.difference(&declared).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{pkg}: not listed by nextest {missing:?}; listed but not a #[test] of a test file {extra:?}"
        );
        assert!(declared.len() >= 10, "{pkg}: only {} tests found, the scan is wrong", declared.len());
        checked.push(pkg);
    }
    assert!(checked.iter().any(|p| p == "planet") && checked.iter().any(|p| p == "planet-generators"), "{checked:?}");
}
