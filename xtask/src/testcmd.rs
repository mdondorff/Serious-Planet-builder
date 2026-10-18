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
/// The `test fast` budget: 20 s (owner's decision, 2026-10-16; it was 10 s, then 15 s; BUILD-003).
pub const FAST_BUDGET_SECS: f64 = 20.0;
/// Where nextest writes the run's JUnit report (`.config/nextest.toml`, `[profile.default.junit]`).
const JUNIT: &str = "target/nextest/default/junit.xml";
/// The JUnit report of a `--real` run: it has a profile of its own (BUILD-003).
const JUNIT_REAL: &str = "target/nextest/real-gpu/junit.xml";
/// The nextest profile of `test gpu --real` (`.config/nextest.toml`): the heavy binaries share a test group of two.
const REAL_GPU_PROFILE: &str = "real-gpu";
/// Most test processes alive at once on the discrete GPU (BUILD-003): about 20 parallel processes lost the device.
const REAL_GPU_THREADS: usize = 4;
/// How many of the slowest tests `test fast` names after a run.
const SLOWEST: usize = 5;

/// The `n` slowest tests of a nextest JUnit report, slowest first, as `(seconds, "binary test")`.
pub fn slowest_tests(junit: &str, n: usize) -> Vec<(f64, String)> {
    fn attr<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let at = line.find(&format!(" {key}=\""))? + key.len() + 3;
        Some(&line[at..at + line[at..].find('"')?])
    }
    let mut all: Vec<(f64, String)> = junit
        .lines()
        .filter(|l| l.trim_start().starts_with("<testcase "))
        .filter_map(|l| {
            let secs: f64 = attr(l, "time")?.parse().ok()?;
            let name =
                format!("{} {}", attr(l, "classname")?, attr(l, "name")?).replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
            Some((secs, name))
        })
        .collect();
    all.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    all.truncate(n);
    all
}

fn package(dir: &str) -> String {
    match dir {
        "app" => "planet".into(),
        "xtask" => "xtask".into(),
        d => format!("planet-{d}"),
    }
}

/// Every crate of the workspace, the selection of a run with nothing to narrow it (BUILD-003).
pub fn all_packages() -> Vec<String> {
    let mut v: Vec<String> = CHAIN.iter().map(|d| package(d)).collect();
    v.extend(["planet-testkit", "planet-perf", "xtask"].map(String::from));
    v
}

/// Crates whose tests `test fast` runs: the changed crates and every crate above them in the chain.
/// With nothing changed, or a change outside `crates/`, everything runs.
pub fn packages_for_changes(changed_paths: &[String]) -> Vec<String> {
    let all = all_packages;
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
    let base = diff_base(&|r| capture(root, "git", &["rev-parse", "--verify", "--quiet", r]).is_ok());
    if let Ok(s) = capture(root, "git", &["diff", "--name-only", &format!("{base}...HEAD")]) {
        paths.extend(s.lines().map(String::from));
    }
    paths
}

/// What `test fast` says about its test time (BUILD-003).
#[derive(Debug, PartialEq, Eq)]
enum Budget {
    Within,
    /// Over the budget on a run of every crate: reported, not gated.
    Note,
    /// Over the budget on a selection of changed crates: a warning that names the slowest tests.
    Over,
}

fn budget_verdict(test_secs: f64, whole_workspace: bool) -> Budget {
    match (test_secs > FAST_BUDGET_SECS, whole_workspace) {
        (false, _) => Budget::Within,
        (true, true) => Budget::Note,
        (true, false) => Budget::Over,
    }
}

/// The ref a branch is compared with to find its changed files: the remote `origin/main` when there is one, else `main`. A
/// worktree's local `main` is whatever the primary checkout last pulled, often hundreds of commits behind, and against it every
/// change looks like "everything changed" (BUILD-003).
fn diff_base(exists: &dyn Fn(&str) -> bool) -> &'static str {
    if exists("origin/main") {
        "origin/main"
    } else {
        "main"
    }
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

/// The nextest arguments that cap a run on the discrete GPU (BUILD-003): the `real-gpu` profile and at most [`REAL_GPU_THREADS`]
/// processes at once, unless the caller passed its own `-j` / `--test-threads`. Nothing for a run on a software adapter.
fn real_gpu_args(real: bool, passthrough: &[&str]) -> Vec<String> {
    if !real {
        return Vec::new();
    }
    let mut v: Vec<String> = vec!["--profile".into(), REAL_GPU_PROFILE.into()];
    let own = passthrough.iter().any(|a| *a == "-j" || a.starts_with("-j") || *a == "--test-threads" || a.starts_with("--test-threads="));
    if !own {
        v.extend(["--test-threads".into(), REAL_GPU_THREADS.to_string()]);
    }
    v
}

/// `--no-fail-fast` for the GPU tier (BUILD-003): every golden test runs to the end and writes its candidates, so one failing test
/// no longer cancels the rest and hides their candidates from `golden-review` (CI's WARP run delivered only the sky goldens of the
/// first failing test). Left out when the caller passed its own `--fail-fast` / `--no-fail-fast`.
fn fail_fast_args(sub: Option<&str>, passthrough: &[&str]) -> Vec<String> {
    let own = passthrough.iter().any(|a| *a == "--fail-fast" || *a == "--no-fail-fast");
    if sub == Some("gpu") && !own {
        vec!["--no-fail-fast".into()]
    } else {
        Vec::new()
    }
}

/// What a run of `test` measured: the test time (nextest's run, not the compile) and whether the run selected every crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestTiming {
    pub test_secs: f64,
    pub whole: bool,
}

pub fn test(root: &Path, args: &[String]) -> Res {
    test_timed(root, args).map(|_| ())
}

/// [`test`] that also says how long the tests took and what they covered (BUILD-003), for `cargo xtask accept`.
pub fn test_timed(root: &Path, args: &[String]) -> Result<TestTiming, String> {
    let mut sub = None;
    let (mut strict, mut real, mut all) = (false, false, false);
    let mut passthrough: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "fast" | "gpu" if sub.is_none() => sub = Some(a.as_str()),
            "--strict" => strict = true,
            "--real" => real = true,
            "--all" => all = true,
            "--" => {
                passthrough.extend(it.by_ref().map(String::as_str));
            }
            other => {
                return Err(format!(
                    "unknown test argument '{other}'\n(test fast [--all] | test | test gpu [--strict] [--real] [-- nextest args])"
                ))
            }
        }
    }
    let adapter = if real { "hardware" } else { "software" };
    let env_strict = std::env::var("PLANET_GOLDEN_STRICT").unwrap_or_default() == "1";
    let env = [("PLANET_ADAPTER", adapter), ("PLANET_GOLDEN_STRICT", if strict || env_strict { "1" } else { "0" })];
    // A report left by an earlier run must not be taken for this one's.
    let _ = std::fs::remove_file(root.join(if real { JUNIT_REAL } else { JUNIT }));

    let mut cmd: Vec<String> = vec!["nextest".into(), "run".into(), "--no-tests=pass".into()];
    let mut whole = sub != Some("fast");
    let mut selection: Vec<String> = Vec::new();
    if sub == Some("fast") {
        let pkgs = if all { all_packages() } else { packages_for_changes(&changed_paths(root)) };
        whole = pkgs == all_packages();
        eprintln!("test fast: {}", if whole { "every crate".to_string() } else { pkgs.join(", ") });
        for p in pkgs {
            selection.extend(["-p".into(), p]);
        }
    } else {
        selection.push("--workspace".into());
    }
    cmd.extend(selection.iter().cloned());
    let filter = tier_filter(sub, real);
    cmd.extend(["-E".into(), filter.into()]);
    cmd.extend(real_gpu_args(real, &passthrough));
    cmd.extend(fail_fast_args(sub, &passthrough));
    cmd.extend(passthrough.iter().map(|s| s.to_string()));
    // The compile is timed apart from the tests: the budget is for the tests (BUILD-003), and an incremental compile of the
    // changed crate is the compiler's time.
    let built = Instant::now();
    let mut build: Vec<&str> = vec!["nextest", "run", "--no-run"];
    build.extend(selection.iter().map(String::as_str));
    run(root, "cargo", &build, &env)?;
    let build_secs = built.elapsed().as_secs_f64();
    let started = Instant::now();
    let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let result = run(root, "cargo", &args, &env);

    let secs = started.elapsed().as_secs_f64();
    eprintln!("elapsed {secs:.1}s (tests; build {build_secs:.1}s)");
    if sub == Some("fast") {
        match budget_verdict(secs, whole) {
            Budget::Within => {}
            Budget::Note => eprintln!(
                "note: a run of every crate took {secs:.1}s; the {FAST_BUDGET_SECS}s budget gates a selection of changed crates, not the whole workspace (BUILD-003)"
            ),
            Budget::Over => eprintln!("warning: test fast took {secs:.1}s, over the {FAST_BUDGET_SECS}s budget (BUILD-003); see docs/status.md"),
        }
    }
    if sub == Some("fast") {
        // The budget is a warning, not a failure: a loaded machine makes any hard limit flaky. The names make a regression
        // easy to find.
        if let Ok(junit) = std::fs::read_to_string(root.join(JUNIT)) {
            eprintln!("slowest {SLOWEST} tests (a machine running other work inflates them all):");
            for (t, name) in slowest_tests(&junit, SLOWEST) {
                eprintln!("  {t:6.2}s  {name}");
            }
        }
    }
    if sub == Some("gpu") {
        let pending = candidates(root);
        if !pending.is_empty() {
            eprintln!("{} candidate golden(s) wait for the owner's /bless (`cargo xtask bless` lists them)", pending.len());
        }
    }
    result.map(|()| TestTiming { test_secs: secs, whole })
}

/// How `cargo xtask accept` reads the timing of `test fast` (BUILD-003): within the budget, or over it on a selection of changed
/// crates, is judged; a run of every crate is reported with its number and never fails the step. `Ok` carries the note for the
/// step's row.
pub fn judge_fast_timing(t: TestTiming) -> Result<String, String> {
    match budget_verdict(t.test_secs, t.whole) {
        Budget::Within => Ok(format!(" ({:.1} s of tests, budget {FAST_BUDGET_SECS} s)", t.test_secs)),
        Budget::Note => Ok(format!(" (whole workspace: reported (BUILD-003), {:.1} s of tests, budget {FAST_BUDGET_SECS} s)", t.test_secs)),
        Budget::Over => {
            Err(format!("test fast took {:.1}s of tests, budget is {FAST_BUDGET_SECS} s (owner, 2026-10-16; BUILD-003)", t.test_secs))
        }
    }
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

/// The lit perf scenes (TEST-025): per-pass GPU time of the lit path over the viewer's views and the coastal flights, and the same
/// views in both tile formats.
const LIT_SCENES: [&str; 2] = ["lit", "lit-formats"];

/// The `cargo` arguments of `perf lit | lit-formats [--store DIR | --allow-memory-bake] [--global-res N] [--out PATH] [--frames N]
/// [--flight-frames N] [--tile-format F] [--machine-ready] [--smoke]`: the scenes under the readiness gate; `--smoke` is the
/// software-adapter proof (never a measurement).
fn perf_lit_args(scene: &str, args: &[String]) -> Result<Vec<String>, String> {
    if !LIT_SCENES.contains(&scene) {
        return Err(format!("unknown lit scene '{scene}' (known: {})", LIT_SCENES.join(", ")));
    }
    let mut smoke = false;
    let mut tail: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke" => smoke = true,
            "--machine-ready" | "--allow-memory-bake" => tail.push(a.clone()),
            "--store" | "--global-res" | "--out" | "--frames" | "--flight-frames" | "--views" | "--tile-format" | "--prefill-levels" | "--look" | "--world" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                tail.push(a.clone());
                tail.push(value.clone());
            }
            other => {
                return Err(format!(
                    "unknown perf {scene} option '{other}' (known: --store DIR, --allow-memory-bake, --global-res, --out PATH, --frames N, --flight-frames N, --views a,b,c, --tile-format vertex|compact, --prefill-levels N, --look F, --world F, --smoke, --machine-ready, the owner's override of a failed readiness gate)"
                ))
            }
        }
    }
    let mut v: Vec<String> = if smoke {
        ["run", "-q", "-p", "planet", "--", "test-render", "--perf-smoke", "--scene", scene, "--adapter", "software"]
    } else {
        ["run", "--release", "-p", "planet", "--", "test-render", "--perf", "--scene", scene, "--adapter", "hardware"]
    }
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(tail);
    Ok(v)
}

/// `perf m5 --store DIR [--out DIR] [--machine-ready]`: the first M5 window in the order the architect authorizes it: the lit views and
/// flights, the same views in both tile formats, then the seven streaming scenes on the lit path (one child process each). Every
/// part runs under the readiness gate; a failing part does not stop the next; the exit is non-zero if any failed.
fn perf_m5(root: &Path, args: &[String]) -> Res {
    let mut out_dir: Option<String> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--out" {
            out_dir = Some(it.next().ok_or("--out needs a directory")?.clone());
        } else if a == "--smoke" {
            return Err("perf m5 is the measured window; smoke the parts with `perf lit --smoke`, `perf lit-formats --smoke` and `perf stream all --lit --smoke`".into());
        } else {
            rest.push(a.clone());
        }
    }
    let dir = out_dir.unwrap_or_else(|| "perf/logs".to_string());
    let mut failed: Vec<String> = Vec::new();
    for scene in LIT_SCENES {
        let mut a = rest.clone();
        a.push("--out".into());
        a.push(Path::new(&dir).join(format!("{scene}.json")).to_string_lossy().into_owned());
        let cargo_args = perf_lit_args(scene, &a)?;
        let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
        eprintln!("$ cargo {}", refs.join(" "));
        if let Err(e) = run(root, "cargo", &refs, &[]) {
            failed.push(format!("{scene}: {e}"));
        }
        // The next gate must not measure this child's tail (window 6: three refusals after the heavy children).
        settle_cpu_after(scene);
    }
    let mut stream = rest.clone();
    stream.extend(["--lit".to_string(), "--out".to_string(), dir.clone()]);
    if let Err(e) = perf_stream_all(root, &stream, true) {
        failed.push(format!("stream --lit: {e}"));
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("perf m5: {} part(s) failed: {}", failed.len(), failed.join("; ")))
    }
}

/// The `cargo` arguments of `capture flights` (TEST-027) and the directory the frames go to: the flight capture of `test-render`
/// (a tool beside the harness, never a measurement). `--smoke` runs it on the software adapter.
fn capture_args(args: &[String]) -> Result<(Vec<String>, String), String> {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("flights") => {}
        other => return Err(format!("usage: cargo xtask capture flights [--store DIR | --allow-memory-bake --global-res N] [--out-dir DIR] [--size WxH] [--flight-frames N] [--smoke] (got {other:?})")),
    }
    let mut smoke = false;
    let mut out_dir = "target/review/m5-flights".to_string();
    let mut tail: Vec<String> = Vec::new();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke" => smoke = true,
            "--allow-memory-bake" => tail.push(a.clone()),
            "--out-dir" => out_dir = it.next().ok_or("--out-dir needs a directory")?.clone(),
            "--store" | "--global-res" | "--size" | "--flight-frames" | "--tile-format" | "--look" | "--world" | "--prefill-levels" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                tail.push(a.clone());
                tail.push(value.clone());
            }
            other => {
                return Err(format!(
                    "unknown capture option '{other}' (known: --store DIR, --allow-memory-bake, --global-res, --out-dir DIR, --size WxH, --flight-frames N, --tile-format, --look F, --world F, --smoke)"
                ))
            }
        }
    }
    let mut v: Vec<String> = if smoke {
        ["run", "-q", "-p", "planet", "--", "test-render", "--scene", "capture-flights", "--adapter", "software"]
    } else {
        ["run", "--release", "-p", "planet", "--", "test-render", "--scene", "capture-flights", "--adapter", "hardware"]
    }
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(tail);
    v.extend(["--out-dir".to_string(), out_dir.clone()]);
    Ok((v, out_dir))
}

/// The `ffmpeg` arguments that encode one frame sequence (`<dir>/frame_NNNN.png`) at 60 frames per second (the flights' frame step)
/// into `<out>/<name>.mp4`: H.264, yuv420p (plays everywhere), visually lossless.
fn ffmpeg_args(dir: &Path, out: &Path) -> Vec<String> {
    [
        "-y",
        "-loglevel",
        "error",
        "-framerate",
        "60",
        "-i",
        &dir.join("frame_%04d.png").to_string_lossy(),
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-crf",
        "14",
        &out.to_string_lossy(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// `cargo xtask capture flights`: the frames, then a video per flight and shore filter when `ffmpeg` is installed (else the PNG
/// sequences stand and the command says so). Writes only below `--out-dir`.
pub fn capture_cmd(root: &Path, args: &[String]) -> Res {
    let (cargo_args, out_dir) = capture_args(args)?;
    let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])?;
    let dir = if Path::new(&out_dir).is_absolute() { Path::new(&out_dir).to_path_buf() } else { root.join(&out_dir) };
    let index = std::fs::read_to_string(dir.join("capture.txt")).map_err(|e| format!("{}: {e}", dir.join("capture.txt").display()))?;
    let have_ffmpeg = std::process::Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success());
    if !have_ffmpeg {
        eprintln!("ffmpeg is not on the PATH: the frames are PNG sequences in {} (no video was made)", dir.display());
        return Ok(());
    }
    for line in index.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let name = line.split_whitespace().next().unwrap_or("");
        let mp4 = dir.join(format!("{name}.mp4"));
        let a = ffmpeg_args(&dir.join(name), &mp4);
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        eprintln!("$ ffmpeg {}", refs.join(" "));
        run(root, "ffmpeg", &refs, &[])?;
        eprintln!("wrote {}", mp4.display());
    }
    Ok(())
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

/// The streaming scenes of `perf stream` (ADR 0032 point 8 and `dive-cold`, ADR 0045 point 3); `all` runs the seven, one child process each.
const STREAM_SCENES: [&str; 7] = ["dive", "dive-fast", "teleport-1m", "teleport-2km", "look-1m", "look-2km", "dive-cold"];

/// The `cargo` arguments of `perf stream [SCENE|all] [--store DIR | --allow-memory-bake] [--global-res N] [--out PATH]
/// [--machine-ready] [--smoke]` (TEST-019): the streaming scenes under the readiness gate. `--smoke` is the software-adapter
/// proof that the scenes run (a few frames, labelled, never a measurement); without it the scenes need the discrete GPU.
fn perf_stream_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut scene = "streaming".to_string();
    let mut rest = args;
    if let Some(first) = args.first().filter(|a| !a.starts_with("--")) {
        if first == "all" {
            scene = "streaming".into();
        } else if STREAM_SCENES.contains(&first.as_str()) {
            scene = first.clone();
        } else {
            return Err(format!("unknown streaming scene '{first}' (known: {}, all)", STREAM_SCENES.join(", ")));
        }
        rest = &args[1..];
    }
    let mut smoke = false;
    let mut tail: Vec<String> = Vec::new();
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke" => smoke = true,
            "--machine-ready" | "--allow-memory-bake" | "--lit" => tail.push(a.clone()),
            "--store" | "--global-res" | "--out" | "--upload" | "--prefill-levels" | "--gen-workers" | "--gen-ahead" | "--gen-reserve" | "--gen-priority" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                tail.push(a.clone());
                tail.push(value.clone());
            }
            other => {
                return Err(format!(
                    "unknown perf stream option '{other}' (known: --store DIR, --allow-memory-bake, --global-res, --out, --upload per-slot|batched, --prefill-levels N, --lit (the lit path with the atmosphere and water, TEST-025), --gen-workers N, --gen-ahead N, --gen-reserve N, --gen-priority below-normal|normal, --smoke, --machine-ready, the owner's override of a failed readiness gate)"
                ))
            }
        }
    }
    let mut v: Vec<String> = if smoke {
        ["run", "-q", "-p", "planet", "--", "test-render", "--perf-smoke", "--scene", &scene, "--adapter", "software"]
    } else {
        ["run", "--release", "-p", "planet", "--", "test-render", "--perf", "--scene", &scene, "--adapter", "hardware"]
    }
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(tail);
    Ok(v)
}

/// The `cargo` arguments of a `cache-compact` run: `planet generate --compact-cache <dir>` plus the definition and the cap.
fn cache_compact_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut it = args.iter();
    let dir = it
        .next()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: cargo xtask cache-compact <dir> [--world <file> | --seed N] [--cap-bytes N]")?;
    let mut v: Vec<String> =
        ["run", "-q", "--release", "-p", "planet", "--", "generate", "--compact-cache"].iter().map(|s| s.to_string()).collect();
    v.push(dir.clone());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--world" | "--seed" | "--cap-bytes" => {
                let value = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                v.push(a.clone());
                v.push(value.clone());
            }
            other => return Err(format!("unknown cache-compact option '{other}' (known: --world, --seed, --cap-bytes)")),
        }
    }
    Ok(v)
}

/// `cargo xtask cache-compact`: compact a tile store (STRM-013). The output is byte-deterministic; the report prints its hash.
pub fn cache_compact(root: &Path, args: &[String]) -> Res {
    let cargo_args = cache_compact_args(args)?;
    let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])
}

/// Whether `perf stream ARGS` names every scene: `all`, or no scene at all.
fn stream_runs_all(args: &[String]) -> bool {
    args.first().is_none_or(|a| a.starts_with("--") || a == "all")
}

/// The arguments of the child that runs `scene` in a `perf stream all` session: the session's options passed through, with
/// `--out DIR` becoming `--out DIR/<scene>.json` so each scene keeps its own log and baseline in the directory (TEST-019).
fn perf_stream_child_args(scene: &str, rest: &[String]) -> Result<Vec<String>, String> {
    let mut a = vec![scene.to_string()];
    let mut it = rest.iter();
    // The lit variant keeps its own logs beside the debug path's: `<scene>-lit.json`.
    let suffix = if rest.iter().any(|x| x == "--lit") { "-lit" } else { "" };
    while let Some(x) = it.next() {
        if x == "--out" {
            let dir = it.next().ok_or("--out needs a value")?;
            a.push(x.clone());
            a.push(Path::new(dir).join(format!("{scene}{suffix}.json")).to_string_lossy().into_owned());
        } else {
            a.push(x.clone());
        }
    }
    perf_stream_args(&a)
}

/// The real [`ChildRunner`]: `cargo` with the child's arguments, stdout teed to the console and kept for the summary.
struct CargoChildren<'a> {
    root: &'a Path,
    /// Wait for the system CPU load to settle between children (the measured window, `perf m5`).
    settle: bool,
}

/// Wait until the system CPU load is below the readiness gate's limit (polling one gate window at a time, capped) and say so.
fn settle_cpu_after(scene: &str) {
    let mut read = || planet_perf::gate::query_cpu_load(planet_perf::gate::LOAD_WINDOW_S);
    let s = planet_perf::session::settle_cpu(&mut read, planet_perf::gate::MAX_CPU_LOAD_PCT, planet_perf::session::SETTLE_MAX_POLLS);
    eprintln!("{}", planet_perf::session::settle_line(scene, &s, planet_perf::gate::MAX_CPU_LOAD_PCT));
}

impl planet_perf::session::ChildRunner for CargoChildren<'_> {
    fn settle_after(&mut self, scene: &str) {
        if self.settle {
            settle_cpu_after(scene);
        }
    }

    fn run(&mut self, args: &[String]) -> Result<planet_perf::session::ChildResult, String> {
        use std::io::BufRead;
        eprintln!("$ cargo {}", args.join(" "));
        let mut child = std::process::Command::new("cargo")
            .args(args)
            .current_dir(self.root)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start `cargo`: {e}"))?;
        let mut output = String::new();
        if let Some(stdout) = child.stdout.take() {
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                println!("{line}");
                output.push_str(&line);
                output.push('\n');
            }
        }
        let status = child.wait().map_err(|e| format!("cannot wait for `cargo`: {e}"))?;
        Ok(planet_perf::session::ChildResult { code: status.code(), output })
    }
}

/// `perf stream all`: one child process per scene, in order, so each scene's RAM is its own peak working set and each starts cold.
/// The readiness gate (ADR 0020) runs in every child, before its measured case: each scene is gated on the machine as it is then.
/// A failing scene does not stop the others; the exit is non-zero if any failed.
fn perf_stream_all(root: &Path, rest: &[String], settle: bool) -> Res {
    let mut runner = CargoChildren { root, settle };
    let outcomes = planet_perf::session::run_scenes(&mut runner, &STREAM_SCENES, &|scene| perf_stream_child_args(scene, rest))?;
    let (text, ok) = planet_perf::session::summary(&outcomes);
    println!("{text}");
    if ok {
        Ok(())
    } else {
        Err(text.lines().last().unwrap_or("a scene failed").to_string())
    }
}

pub fn perf(root: &Path, args: &[String]) -> Res {
    if args.first().map(String::as_str) == Some("stream") {
        if stream_runs_all(&args[1..]) {
            let rest = if args.get(1).is_some_and(|a| a == "all") { &args[2..] } else { &args[1..] };
            return perf_stream_all(root, rest, false);
        }
        let cargo_args = perf_stream_args(&args[1..])?;
        let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
        return run(root, "cargo", &refs, &[]);
    }
    if let Some(scene) = args.first().filter(|a| LIT_SCENES.contains(&a.as_str())) {
        let cargo_args = perf_lit_args(scene, &args[1..])?;
        let refs: Vec<&str> = cargo_args.iter().map(String::as_str).collect();
        return run(root, "cargo", &refs, &[]);
    }
    if args.first().map(String::as_str) == Some("m5") {
        return perf_m5(root, &args[1..]);
    }
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

    // spec: TEST-019
    #[test]
    fn perf_stream_all_runs_a_child_per_scene_with_the_options_passed_through_and_a_log_per_scene() {
        assert!(stream_runs_all(&s(&[])) && stream_runs_all(&s(&["all"])) && stream_runs_all(&s(&["--store", "D"])));
        assert!(!stream_runs_all(&s(&["dive"])));
        for name in STREAM_SCENES {
            let rest = s(&[
                "--store",
                "D:/bake",
                "--global-res",
                "256",
                "--out",
                "target/perf",
                "--machine-ready",
                "--allow-memory-bake",
                "--gen-workers",
                "12",
                "--gen-ahead",
                "0",
                "--gen-reserve",
                "3",
                "--gen-priority",
                "normal",
            ]);
            let a = perf_stream_child_args(name, &rest).unwrap();
            let at = a.iter().position(|x| x == "--scene").unwrap();
            assert_eq!(a[at + 1], name, "one named scene, never `streaming`");
            assert!(!a.contains(&"streaming".to_string()));
            let out = a.iter().position(|x| x == "--out").unwrap();
            assert!(a[out + 1].ends_with(&format!("{name}.json")) && a[out + 1].starts_with("target/perf"), "{a:?}");
            for keep in [
                "--store",
                "D:/bake",
                "--global-res",
                "256",
                "--machine-ready",
                "--allow-memory-bake",
                "--gen-workers",
                "12",
                "--gen-ahead",
                "0",
                "--gen-reserve",
                "3",
                "--gen-priority",
                "normal",
            ] {
                assert!(a.contains(&keep.to_string()), "{keep} in {a:?}");
            }
        }
        let smoke = perf_stream_child_args("dive", &s(&["--smoke", "--allow-memory-bake"])).unwrap();
        assert!(smoke.contains(&"--perf-smoke".to_string()) && !smoke.contains(&"--out".to_string()));
        assert!(perf_stream_child_args("dive", &s(&["--out"])).unwrap_err().contains("needs a value"));
    }

    // spec: BUILD-003
    #[test]
    fn the_slowest_tests_of_a_junit_report_are_named_slowest_first() {
        let xml = r#"<testsuites name="nextest-run" tests="3" time="9.5">
    <testsuite name="planet-frame" tests="3">
        <testcase name="a::quick" classname="planet-frame" timestamp="t" time="0.010"/>
        <testcase name="b::slow" classname="planet-frame::frustum_cut" timestamp="t" time="4.250"/>
        <testcase name="c::slower &amp; odd" classname="planet" timestamp="t" time="7.125"/>
    </testsuite>
</testsuites>"#;
        let top = slowest_tests(xml, 2);
        assert_eq!(top, [(7.125, "planet c::slower & odd".to_string()), (4.25, "planet-frame::frustum_cut b::slow".to_string())]);
        assert!(slowest_tests("not a report", 5).is_empty());
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
    fn changed_files_are_found_against_the_remote_main_when_there_is_one() {
        assert_eq!(diff_base(&|r| r == "origin/main" || r == "main"), "origin/main");
        assert_eq!(diff_base(&|r| r == "main"), "main");
        assert_eq!(diff_base(&|_| false), "main");
    }

    // spec: BUILD-003
    #[test]
    fn the_budget_gates_a_selection_of_changed_crates_and_only_reports_the_whole_workspace() {
        assert_eq!(budget_verdict(19.9, false), Budget::Within);
        assert_eq!(budget_verdict(25.0, false), Budget::Over);
        assert_eq!(budget_verdict(25.0, true), Budget::Note);
        assert_eq!(budget_verdict(5.0, true), Budget::Within);
        assert_eq!(packages_for_changes(&[]), all_packages());
        assert_ne!(packages_for_changes(&s(&["crates/app/src/lib.rs"])), all_packages());
    }

    // spec: BUILD-003
    #[test]
    fn acceptance_judges_a_selection_by_test_time_and_only_reports_a_whole_workspace_run() {
        let t = |test_secs: f64, whole: bool| judge_fast_timing(TestTiming { test_secs, whole });
        // A selection of changed crates: within the budget passes with its number, over it fails naming the test time.
        assert!(t(19.9, false).unwrap().contains("19.9 s of tests"));
        let e = t(32.3, false).unwrap_err();
        assert!(e.contains("32.3s of tests") && e.contains("BUILD-003"), "{e}");
        // A run of every crate is reported with its number and never fails the step (32.3 s was the first M5 acceptance).
        let note = t(32.3, true).unwrap();
        assert!(note.contains("whole workspace: reported (BUILD-003)") && note.contains("32.3 s of tests"), "{note}");
        assert!(t(5.0, true).unwrap().contains("5.0 s of tests"));
    }

    // spec: BUILD-003
    #[test]
    fn the_gpu_tier_runs_every_test_so_that_every_candidate_golden_is_written() {
        assert_eq!(fail_fast_args(Some("gpu"), &[]), ["--no-fail-fast"]);
        assert!(fail_fast_args(Some("gpu"), &["--fail-fast"]).is_empty() && fail_fast_args(Some("gpu"), &["--no-fail-fast"]).is_empty());
        assert!(fail_fast_args(Some("fast"), &[]).is_empty() && fail_fast_args(None, &[]).is_empty());
    }

    // spec: BUILD-003
    #[test]
    fn each_subcommand_selects_its_tiers_and_the_real_gpu_adds_the_oracles() {
        assert_eq!(tier_filter(Some("fast"), false), TIER_A);
        assert_eq!(tier_filter(None, false), TIER_AB);
        assert_eq!(tier_filter(Some("gpu"), false), TIER_C);
        assert_eq!(tier_filter(Some("gpu"), true), "binary(/^gpu_/) or binary(/^oracle_/)");
    }

    // spec: BUILD-003
    #[test]
    fn a_real_gpu_run_uses_the_capped_profile_and_a_software_run_is_untouched() {
        assert!(real_gpu_args(false, &[]).is_empty() && real_gpu_args(false, &["-j", "2"]).is_empty());
        assert_eq!(real_gpu_args(true, &[]), ["--profile", "real-gpu", "--test-threads", "4"]);
        // The caller's own thread count wins, in every spelling nextest takes; the profile stays.
        for own in [vec!["-j", "2"], vec!["-j2"], vec!["--test-threads", "1"], vec!["--test-threads=3"]] {
            assert_eq!(real_gpu_args(true, &own), ["--profile", "real-gpu"], "{own:?}");
        }
        assert_eq!(JUNIT_REAL, "target/nextest/real-gpu/junit.xml");
    }

    // spec: BUILD-003
    #[test]
    fn the_real_gpu_profile_groups_existing_binaries_only_and_leaves_the_default_profile_alone() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let toml = std::fs::read_to_string(root.join(".config/nextest.toml")).unwrap();
        let (default_part, real_part) = toml.split_once("[profile.real-gpu]").expect("the real-gpu profile exists");
        assert!(!default_part.contains("test-group ="), "the default profile (software, WARP, CI) has no test group");
        assert!(toml.contains("real-gpu-heavy = { max-threads ="));
        // Every binary the group names is a test file somewhere in the workspace (a typo would silently leave a heavy binary uncapped).
        let named: Vec<&str> = real_part.match_indices("binary(").map(|(i, _)| real_part[i + 7..].split(')').next().unwrap()).collect();
        assert!(named.len() >= 8, "{named:?}");
        let mut files = Vec::new();
        for krate in std::fs::read_dir(root.join("crates")).unwrap().flatten() {
            if let Ok(rd) = std::fs::read_dir(krate.path().join("tests")) {
                files.extend(rd.flatten().map(|e| e.file_name().to_string_lossy().trim_end_matches(".rs").to_string()));
            }
        }
        for n in named {
            assert!(files.iter().any(|f| f == n), "{n} is not a test binary");
        }
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

    // spec: TEST-027
    #[test]
    fn the_capture_builds_its_command_line_encodes_at_60_fps_and_refuses_what_it_does_not_know() {
        let (a, dir) = capture_args(&s(&["flights", "--store", "C:/bake", "--flight-frames", "240"])).unwrap();
        assert_eq!(dir, "target/review/m5-flights");
        assert!(
            a.contains(&"capture-flights".to_string()) && a.contains(&"hardware".to_string()) && a.contains(&"--release".to_string()),
            "{a:?}"
        );
        assert_eq!(a[a.iter().position(|x| x == "--store").unwrap() + 1], "C:/bake");
        assert_eq!(a[a.iter().position(|x| x == "--out-dir").unwrap() + 1], "target/review/m5-flights");
        assert!(!a.contains(&"--perf".to_string()) && !a.contains(&"--machine-ready".to_string()), "a tool, not a measurement: {a:?}");
        let (a, dir) = capture_args(&s(&["flights", "--smoke", "--allow-memory-bake", "--global-res", "128", "--out-dir", "X"])).unwrap();
        assert_eq!(dir, "X");
        assert!(a.contains(&"software".to_string()) && !a.contains(&"--release".to_string()), "{a:?}");
        assert!(capture_args(&s(&["videos"])).unwrap_err().contains("usage"));
        assert!(capture_args(&s(&["flights", "--machine-ready"])).unwrap_err().contains("unknown capture option"));
        let f = ffmpeg_args(Path::new("d/coast-glide-filter1"), Path::new("d/coast-glide-filter1.mp4"));
        assert_eq!(f[f.iter().position(|x| x == "-framerate").unwrap() + 1], "60");
        assert!(
            f.iter().any(|x| x.replace('\\', "/") == "d/coast-glide-filter1/frame_%04d.png")
                && f.contains(&"libx264".to_string())
                && f.contains(&"yuv420p".to_string()),
            "{f:?}"
        );
        assert!(f.last().unwrap().ends_with("coast-glide-filter1.mp4"));
    }

    // spec: TEST-019, TEST-025
    #[test]
    fn the_lit_scenes_and_the_lit_stream_variant_build_their_command_lines_and_keep_their_logs_apart() {
        let a = perf_lit_args("lit", &s(&["--store", "C:/bake", "--out", "perf/logs/lit.json", "--machine-ready"])).unwrap();
        assert!(a.contains(&"--perf".to_string()) && a.contains(&"hardware".to_string()) && a.contains(&"--release".to_string()), "{a:?}");
        assert_eq!(a[a.iter().position(|x| x == "--scene").unwrap() + 1], "lit");
        assert_eq!(a[a.iter().position(|x| x == "--store").unwrap() + 1], "C:/bake");
        let smoke = perf_lit_args("lit-formats", &s(&["--smoke", "--global-res", "128"])).unwrap();
        assert!(
            smoke.contains(&"--perf-smoke".to_string())
                && smoke.contains(&"software".to_string())
                && !smoke.contains(&"--perf".to_string()),
            "{smoke:?}"
        );
        assert!(perf_lit_args("dive", &[]).unwrap_err().contains("known: lit, lit-formats"));
        assert!(perf_lit_args("lit", &s(&["--force"])).unwrap_err().contains("override"));
        // The lit variant of the streaming scenes: --lit goes through, and a log directory gets `<scene>-lit.json`.
        let d = perf_stream_child_args("dive", &s(&["--lit", "--store", "S", "--out", "D"])).unwrap();
        assert!(d.contains(&"--lit".to_string()), "{d:?}");
        assert!(d[d.iter().position(|x| x == "--out").unwrap() + 1].replace('\\', "/").ends_with("D/dive-lit.json"), "{d:?}");
        let plain = perf_stream_child_args("dive", &s(&["--out", "D"])).unwrap();
        assert!(plain[plain.iter().position(|x| x == "--out").unwrap() + 1].replace('\\', "/").ends_with("D/dive.json"), "{plain:?}");
    }

    // spec: TEST-019
    #[test]
    fn perf_stream_runs_the_scenes_under_the_gate_over_a_store_and_has_a_software_smoke() {
        let a = perf_stream_args(&s(&["dive", "--store", "C:/bake", "--machine-ready"])).unwrap();
        assert!(a.contains(&"--perf".to_string()) && a.contains(&"hardware".to_string()) && a.contains(&"--release".to_string()), "{a:?}");
        assert_eq!(a[a.iter().position(|x| x == "--scene").unwrap() + 1], "dive");
        assert_eq!(a[a.iter().position(|x| x == "--store").unwrap() + 1], "C:/bake");
        assert!(a.contains(&"--machine-ready".to_string()));
        let all = perf_stream_args(&s(&["--allow-memory-bake"])).unwrap();
        assert_eq!(all[all.iter().position(|x| x == "--scene").unwrap() + 1], "streaming");
        assert!(perf_stream_args(&s(&["all"])).unwrap().contains(&"streaming".to_string()));
        let smoke = perf_stream_args(&s(&["look-1m", "--smoke"])).unwrap();
        assert!(
            smoke.contains(&"--perf-smoke".to_string())
                && smoke.contains(&"software".to_string())
                && !smoke.contains(&"--perf".to_string()),
            "{smoke:?}"
        );
        // The store check of the smoke run (TEST-024) takes the pre-fill depth through.
        let check = perf_stream_args(&s(&["dive", "--smoke", "--store", "C:/bake", "--prefill-levels", "4"])).unwrap();
        assert_eq!(check[check.iter().position(|x| x == "--prefill-levels").unwrap() + 1], "4", "{check:?}");
        for name in STREAM_SCENES {
            assert!(perf_stream_args(&s(&[name])).is_ok(), "{name}");
        }
        assert!(perf_stream_args(&s(&["terrain"])).unwrap_err().contains("known: dive"));
        assert!(perf_stream_args(&s(&["--store"])).unwrap_err().contains("needs a value"));
        assert!(perf_stream_args(&s(&["--force"])).unwrap_err().contains("override"));
    }

    // spec: STRM-013
    #[test]
    fn cache_compact_forwards_the_definition_and_the_cap_and_refuses_the_rest() {
        let a = cache_compact_args(&s(&["d", "--seed", "3", "--cap-bytes", "1000"])).unwrap();
        assert!(a.contains(&"--compact-cache".to_string()) && a.contains(&"d".to_string()), "{a:?}");
        assert_eq!(a[a.iter().position(|x| x == "--cap-bytes").unwrap() + 1], "1000");
        assert!(cache_compact_args(&[]).unwrap_err().contains("usage"));
        assert!(cache_compact_args(&s(&["d", "--cap-bytes"])).unwrap_err().contains("needs a value"));
        let e = cache_compact_args(&s(&["d", "--force"])).unwrap_err();
        assert!(e.contains("--force") && e.contains("known"), "{e}");
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
