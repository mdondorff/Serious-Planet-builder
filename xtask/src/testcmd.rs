use crate::layering::CHAIN;
use crate::util::{capture, files_with_ext, run, Res};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

/// Test binaries named `gpu_*` are tier C, `oracle_*` tier B; everything else is tier A.
const TIER_A: &str = "not binary(/^gpu_/) and not binary(/^oracle_/)";
const TIER_AB: &str = "not binary(/^gpu_/)";
const TIER_C: &str = "binary(/^gpu_/)";
const TIER_BC_REAL: &str = "binary(/^gpu_/) or binary(/^oracle_/)";
const FAST_BUDGET_SECS: f64 = 10.0;

fn package(dir: &str) -> String {
    match dir {
        "app" => "planet".into(),
        "xtask" => "xtask".into(),
        d => format!("planet-{d}"),
    }
}

/// Crates whose tests `test fast` runs: the changed crates and every crate above them in the chain.
/// With nothing changed, or a change outside `crates/`, everything runs.
pub fn packages_for_changes(changed_paths: &[String]) -> Vec<String> {
    let all = || {
        let mut v: Vec<String> = CHAIN.iter().map(|d| package(d)).collect();
        v.extend(["planet-testkit", "planet-perf", "xtask"].map(String::from));
        v
    };
    let mut lowest: Option<usize> = None;
    let mut extra: BTreeSet<String> = BTreeSet::new();
    for p in changed_paths {
        let p = p.replace('\\', "/");
        if let Some(rest) = p.strip_prefix("crates/") {
            let dir = rest.split('/').next().unwrap_or("");
            if let Some(i) = CHAIN.iter().position(|c| *c == dir) {
                lowest = Some(lowest.map_or(i, |l| l.min(i)));
            } else if dir == "testkit" || dir == "perf" {
                // Used by tests of every crate that depends on them: run everything.
                return all();
            }
        } else if let Some(rest) = p.strip_prefix("xtask/") {
            if !rest.is_empty() {
                extra.insert("xtask".into());
            }
        } else if p == "Cargo.toml" || p == "Cargo.lock" || p.starts_with(".cargo/") {
            return all();
        }
    }
    if lowest.is_none() && extra.is_empty() {
        return all();
    }
    let mut v: Vec<String> = lowest.map(|i| CHAIN[i..].iter().map(|d| package(d)).collect()).unwrap_or_default();
    v.extend(extra);
    v
}

fn changed_paths(root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(s) = capture(root, "git", &["status", "--porcelain", "--untracked-files=all"]) {
        paths.extend(s.lines().filter(|l| l.len() > 3).map(|l| l[3..].trim().trim_matches('"').to_string()));
    }
    if let Ok(s) = capture(root, "git", &["diff", "--name-only", "main...HEAD"]) {
        paths.extend(s.lines().map(String::from));
    }
    paths
}

/// The nextest filter of a `test` subcommand. On the real GPU (`test gpu --real`) the tier-B oracles run with tier C:
/// they compare its shaders with the CPU twins (BUILD-003).
fn tier_filter(sub: Option<&str>, real: bool) -> &'static str {
    match sub {
        Some("fast") => TIER_A,
        Some("gpu") if real => TIER_BC_REAL,
        Some("gpu") => TIER_C,
        _ => TIER_AB,
    }
}

pub fn test(root: &Path, args: &[String]) -> Res {
    let mut sub = None;
    let (mut strict, mut real) = (false, false);
    let mut passthrough: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "fast" | "gpu" if sub.is_none() => sub = Some(a.as_str()),
            "--strict" => strict = true,
            "--real" => real = true,
            "--" => {
                passthrough.extend(it.by_ref().map(String::as_str));
            }
            other => {
                return Err(format!("unknown test argument '{other}'\n(test fast | test | test gpu [--strict] [--real] [-- nextest args])"))
            }
        }
    }
    let adapter = if real { "hardware" } else { "software" };
    let env_strict = std::env::var("PLANET_GOLDEN_STRICT").unwrap_or_default() == "1";
    let env = [("PLANET_ADAPTER", adapter), ("PLANET_GOLDEN_STRICT", if strict || env_strict { "1" } else { "0" })];
    let started = Instant::now();

    let mut cmd: Vec<String> = vec!["nextest".into(), "run".into(), "--no-tests=pass".into()];
    if sub == Some("fast") {
        let pkgs = packages_for_changes(&changed_paths(root));
        eprintln!("test fast: {}", pkgs.join(", "));
        for p in pkgs {
            cmd.extend(["-p".into(), p]);
        }
    } else {
        cmd.push("--workspace".into());
    }
    let filter = tier_filter(sub, real);
    cmd.extend(["-E".into(), filter.into()]);
    cmd.extend(passthrough.iter().map(|s| s.to_string()));
    let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let result = run(root, "cargo", &args, &env);

    let secs = started.elapsed().as_secs_f64();
    eprintln!("elapsed {secs:.1}s");
    if sub == Some("fast") && secs > FAST_BUDGET_SECS {
        eprintln!("warning: test fast took {secs:.1}s, over the {FAST_BUDGET_SECS}s budget (report §16); see docs/status.md");
    }
    if sub == Some("gpu") {
        let pending = candidates(root);
        if !pending.is_empty() {
            eprintln!("{} candidate golden(s) wait for the owner's /bless (`cargo xtask bless` lists them)", pending.len());
        }
    }
    result
}

pub fn check(root: &Path) -> Res {
    run(root, "cargo", &["fmt", "--all", "--", "--check"], &[])?;
    run(root, "cargo", &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], &[])?;
    crate::layering::run(root)?;
    stale_pending(root)
}

/// Names in `tests/goldens/pending.txt` that already have a golden for some adapter: `/bless` must remove them.
pub fn stale_pending_names(root: &Path) -> Vec<String> {
    let list = std::fs::read_to_string(root.join("tests").join("goldens").join("pending.txt")).unwrap_or_default();
    let mut files = Vec::new();
    files_with_ext(&root.join("tests").join("goldens"), "png", &mut files);
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter(|name| files.iter().any(|f| f.file_stem().is_some_and(|s| s == *name)))
        .map(String::from)
        .collect()
}

fn stale_pending(root: &Path) -> Res {
    let stale = stale_pending_names(root);
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!("tests/goldens/pending.txt lists goldens that are already blessed (remove the lines): {}", stale.join(", ")))
    }
}

/// Candidate goldens under `target/review/candidates/<adapter>/<name>.png` that have no golden yet or differ from it.
fn candidates(root: &Path) -> Vec<(String, String)> {
    let base = root.join("target").join("review").join("candidates");
    let mut files = Vec::new();
    files_with_ext(&base, "png", &mut files);
    let mut out = Vec::new();
    for f in files {
        let Ok(rel) = f.strip_prefix(&base) else { continue };
        let parts: Vec<String> = rel.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        if let [adapter, name] = parts.as_slice() {
            // Stale candidates identical to the committed golden are not waiting for anyone.
            let golden = root.join("tests").join("goldens").join(adapter).join(name);
            if std::fs::read(&golden).ok().as_deref() == std::fs::read(&f).ok().as_deref() && golden.is_file() {
                continue;
            }
            out.push((adapter.clone(), name.clone()));
        }
    }
    out
}

pub fn bless_list(root: &Path) -> Res {
    let c = candidates(root);
    if c.is_empty() {
        eprintln!("no candidate goldens in target/review/candidates/");
        return Ok(());
    }
    for (adapter, name) in c {
        let golden = root.join("tests").join("goldens").join(&adapter).join(&name);
        let state = if golden.is_file() { "differs from golden" } else { "no golden yet" };
        eprintln!("{adapter}/{name}: {state}");
    }
    eprintln!("Promotion is owner-only: run /bless in a Claude Code session and answer each prompt.");
    Ok(())
}

pub fn repro(root: &Path, args: &[String]) -> Res {
    let Some(bundle) = args.iter().find(|a| !a.starts_with("--")) else {
        return Err("usage: cargo xtask repro <bundle> [--real]".into());
    };
    let bundle = std::fs::canonicalize(bundle)
        .or_else(|_| std::fs::canonicalize(root.join(bundle)))
        .map_err(|e| format!("repro bundle '{bundle}' not found: {e}"))?;
    let bundle = bundle.to_string_lossy().trim_start_matches("\\?\\").to_string();
    let bundle = bundle.as_str();
    let adapter = if args.iter().any(|a| a == "--real") { "hardware" } else { "software" };
    let out = "target/review/repro.png";
    run(root, "cargo", &["run", "-q", "-p", "planet", "--", "test-render", "--adapter", adapter, "--repro", bundle, "--out", out], &[])
}

/// The `cargo` arguments of a perf run. The harness decides readiness itself (ADR 0020: AC power, performance scheme,
/// reference adapter, quiet machine, load and temperature, measured and refused, not asserted); `--machine-ready` is
/// only the owner's override of a failed gate and is passed through when given.
fn perf_cargo_args(args: &[String]) -> Result<Vec<&'static str>, String> {
    let mut v = vec!["run", "--release", "-p", "planet", "--", "test-render", "--perf", "--scene", "terrain", "--adapter", "hardware"];
    for a in args {
        match a.as_str() {
            "--machine-ready" => v.push("--machine-ready"),
            other => {
                return Err(format!(
                    "unknown perf option '{other}' (known: --machine-ready, the owner's override of a failed readiness gate)"
                ))
            }
        }
    }
    Ok(v)
}

/// The `cargo` arguments of a `cache-verify` run: `planet generate --verify-cache <dir>` plus the options that select the
/// definition and the sample size. Unknown options are refused rather than ignored.
fn cache_verify_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut it = args.iter();
    let dir = it
        .next()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: cargo xtask cache-verify <dir> [--world <file> | --seed N] [--sample N]")?;
    let mut v: Vec<String> =
        ["run", "-q", "--release", "-p", "planet", "--", "generate", "--verify-cache"].iter().map(|s| s.to_string()).collect();
    v.push(dir.clone());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--allow-stale" => v.push(a.clone()),
            "--world" | "--seed" | "--sample" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                v.push(a.clone());
                v.push(value.clone());
            }
            other => return Err(format!("unknown cache-verify option '{other}' (known: --world, --seed, --sample, --allow-stale)")),
        }
    }
    Ok(v)
}

/// `cargo xtask cache-verify`: regenerate a sample of a tile store from the definition and compare it with the store.
pub fn cache_verify(root: &Path, args: &[String]) -> Res {
    let cargo_args = cache_verify_args(args)?;
    let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])
}

/// The `cargo` arguments of `perf global-bake` (GEN-014): the global-bake scene under the same readiness gate, with the grid
/// size and thread count as options. The default grid is the 1 km one of GEN-014 (16384 cells per face edge).
fn perf_global_bake_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut v: Vec<String> =
        ["run", "--release", "-p", "planet", "--", "test-render", "--perf", "--scene", "global-bake", "--adapter", "hardware"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--machine-ready" => v.push(a.clone()),
            "--global-res" | "--threads" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                v.push(a.clone());
                v.push(value.clone());
            }
            other => {
                return Err(format!(
                    "unknown perf global-bake option '{other}' (known: --global-res, --threads, --machine-ready, the owner's override of a failed readiness gate)"
                ))
            }
        }
    }
    Ok(v)
}

pub fn perf(root: &Path, args: &[String]) -> Res {
    if args.first().map(String::as_str) == Some("global-bake") {
        let cargo_args = perf_global_bake_args(&args[1..])?;
        let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
        return run(root, "cargo", &refs, &[]);
    }
    let cargo_args = perf_cargo_args(args)?;
    run(root, "cargo", &cargo_args, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-003
    #[test]
    fn fast_runs_changed_crates_and_everything_above() {
        let p = packages_for_changes(&s(&["crates/streaming/src/lib.rs"]));
        assert_eq!(p, ["planet-streaming", "planet-frame", "planet-render", "planet-editor", "planet"]);
        let p = packages_for_changes(&s(&["crates/frame/src/a.rs", "crates/cache/src/b.rs"]));
        assert_eq!(p[0], "planet-cache");
        assert_eq!(p.len(), 6);
    }

    // spec: BUILD-003
    #[test]
    fn fast_runs_everything_for_shared_or_unknown_changes() {
        for paths in [&["crates/testkit/src/lib.rs"][..], &["Cargo.toml"], &["docs/status.md"], &[]] {
            assert!(packages_for_changes(&s(paths)).len() >= 12, "{paths:?}");
        }
        assert_eq!(packages_for_changes(&s(&["xtask/src/main.rs"])), ["xtask"]);
    }

    // spec: BUILD-003
    #[test]
    fn each_subcommand_selects_its_tiers_and_the_real_gpu_adds_the_oracles() {
        assert_eq!(tier_filter(Some("fast"), false), TIER_A);
        assert_eq!(tier_filter(None, false), TIER_AB);
        assert_eq!(tier_filter(Some("gpu"), false), TIER_C);
        assert_eq!(tier_filter(Some("gpu"), true), "binary(/^gpu_/) or binary(/^oracle_/)");
    }

    // spec: STRM-004
    #[test]
    fn cache_verify_passes_the_directory_and_known_options_to_the_generate_mode_and_refuses_the_rest() {
        let a = cache_verify_args(&s(&["target/cache", "--seed", "7", "--sample", "50"])).unwrap();
        assert!(a.windows(2).any(|w| w == ["--verify-cache", "target/cache"]), "{a:?}");
        assert_eq!(&a[a.len() - 4..], ["--seed", "7", "--sample", "50"]);
        assert!(cache_verify_args(&s(&[])).unwrap_err().contains("usage"));
        assert!(cache_verify_args(&s(&["--seed", "7"])).unwrap_err().contains("usage"));
        assert!(cache_verify_args(&s(&["d", "--sample"])).unwrap_err().contains("needs a value"));
        let e = cache_verify_args(&s(&["d", "--fix"])).unwrap_err();
        assert!(e.contains("--fix") && e.contains("known"), "{e}");
    }

    // spec: BUILD-002
    #[test]
    fn perf_no_longer_needs_the_owner_flag_and_passes_it_through_only_as_an_override() {
        let plain = perf_cargo_args(&[]).unwrap();
        assert!(plain.contains(&"--perf") && plain.contains(&"hardware") && !plain.contains(&"--machine-ready"), "{plain:?}");
        let over = perf_cargo_args(&s(&["--machine-ready"])).unwrap();
        assert_eq!(over.last(), Some(&"--machine-ready"));
        let e = perf_cargo_args(&s(&["--force"])).unwrap_err();
        assert!(e.contains("--force") && e.contains("override"), "{e}");
    }

    // spec: GEN-014, BUILD-002
    #[test]
    fn perf_global_bake_runs_the_scene_under_the_gate_and_takes_the_grid_size() {
        let a = perf_global_bake_args(&s(&["--global-res", "1024", "--machine-ready"])).unwrap();
        assert!(
            a.contains(&"global-bake".to_string()) && a.contains(&"--perf".to_string()) && a.contains(&"hardware".to_string()),
            "{a:?}"
        );
        assert_eq!(a[a.iter().position(|x| x == "--global-res").unwrap() + 1], "1024");
        assert!(a.contains(&"--machine-ready".to_string()));
        assert!(perf_global_bake_args(&s(&["--global-res"])).unwrap_err().contains("needs a value"));
        let e = perf_global_bake_args(&s(&["--force"])).unwrap_err();
        assert!(e.contains("--force") && e.contains("override"), "{e}");
    }
}

#[cfg(test)]
mod repro_tests {
    use super::*;

    // spec: TEST-008
    #[test]
    fn repro_needs_an_existing_bundle() {
        assert!(repro(Path::new("."), &[]).unwrap_err().contains("usage"));
        assert!(repro(Path::new("."), &["no-such-bundle.repro".to_string()]).unwrap_err().contains("not found"));
    }
}

#[cfg(test)]
mod candidate_tests {
    use super::*;

    // spec: TEST-002
    #[test]
    fn candidates_list_only_new_or_changed_images() {
        let root = std::env::temp_dir().join(format!("xtask-candidates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (cand, gold) = (root.join("target/review/candidates/a"), root.join("tests/goldens/a"));
        std::fs::create_dir_all(&cand).unwrap();
        std::fs::create_dir_all(&gold).unwrap();
        std::fs::write(cand.join("same.png"), b"1").unwrap();
        std::fs::write(gold.join("same.png"), b"1").unwrap();
        std::fs::write(cand.join("changed.png"), b"2").unwrap();
        std::fs::write(gold.join("changed.png"), b"1").unwrap();
        std::fs::write(cand.join("new.png"), b"3").unwrap();
        let mut got = candidates(&root);
        got.sort();
        assert_eq!(got, [("a".to_string(), "changed.png".to_string()), ("a".to_string(), "new.png".to_string())]);
    }
}

#[cfg(test)]
mod pending_tests {
    use super::*;

    // spec: TEST-002
    #[test]
    fn blessed_names_must_leave_the_pending_list() {
        let root = std::env::temp_dir().join(format!("xtask-pending-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let g = root.join("tests/goldens");
        std::fs::create_dir_all(g.join("a")).unwrap();
        std::fs::write(g.join("pending.txt"), "# comment\nwaiting\nblessed\n").unwrap();
        std::fs::write(g.join("a/blessed.png"), b"x").unwrap();
        assert_eq!(stale_pending_names(&root), ["blessed"]);
        assert!(stale_pending(&root).unwrap_err().contains("blessed"));
        std::fs::remove_file(g.join("a/blessed.png")).unwrap();
        assert!(stale_pending(&root).is_ok());
    }

    // spec: TEST-002
    #[test]
    fn the_repository_pending_list_is_clean() {
        assert!(stale_pending(&crate::util::repo_root()).is_ok());
    }
}
