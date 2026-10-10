use crate::layering::CHAIN;
use crate::util::{capture, files_with_ext, run, Res};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

/// Test binaries named `gpu_*` are tier C, `oracle_*` tier B; everything else is tier A.
const TIER_A: &str = "not binary(~gpu_) and not binary(~oracle_)";
const TIER_AB: &str = "not binary(~gpu_)";
const TIER_C: &str = "binary(~gpu_)";
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
    let filter = match sub {
        Some("fast") => {
            let pkgs = packages_for_changes(&changed_paths(root));
            eprintln!("test fast: {}", pkgs.join(", "));
            for p in pkgs {
                cmd.extend(["-p".into(), p]);
            }
            TIER_A
        }
        Some("gpu") => {
            cmd.push("--workspace".into());
            TIER_C
        }
        _ => {
            cmd.push("--workspace".into());
            TIER_AB
        }
    };
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

pub fn perf(root: &Path, args: &[String]) -> Res {
    if !args.iter().any(|a| a == "--machine-ready") {
        return Err("refusing to run: pass --machine-ready only after the owner confirms Rasierklinge is ready \
                    (charger connected, performance power profile, discrete GPU mode). CLAUDE.md: never run perf on your own."
            .into());
    }
    run(
        root,
        "cargo",
        &["run", "--release", "-p", "planet", "--", "test-render", "--perf", "--scene", "terrain", "--adapter", "hardware", "--machine-ready"],
        &[],
    )
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

    // spec: BUILD-002
    #[test]
    fn perf_needs_the_owner_flag() {
        let e = perf(Path::new("."), &[]).unwrap_err();
        assert!(e.contains("--machine-ready") && e.contains("owner"), "{e}");
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
