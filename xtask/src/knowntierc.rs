//! The known local tier-C classes (BUILD-056; lead, 2026-11-14, codifying the architect's rule of 2026-11-09): the local `test gpu` on this machine fails in a few
//! tracked classes that have nothing to do with a change (WARP aborts, `gpu_skeleton` timeouts under CPU oversubscription, `gpu_preflight`'s 8 s bound under load).
//! When the local tier C fails only in classes of [`LIST`], no golden test mismatched and no candidate golden was written, prepush passes and prints a loud summary that
//! the lane copies into the PR body. Any other failure fails it. The list is local-only: CI runs tier C in full on every push and changes nothing here.

use std::path::Path;

/// The tracked list of known classes.
pub const LIST: &str = "xtask/known-tier-c.txt";

/// What a known class accepts of a failed test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The test process ended with this exit code (lower-case hex, `0x...`).
    Abort(String),
    /// nextest's slow-timeout ended it.
    Timeout,
    /// A FAIL whose panic is at this `file:line` (a path suffix).
    PanicAt(String),
}

/// One line of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub pattern: String,
    pub outcome: Outcome,
    /// The bold title of its `docs/status.md` Tracked entry.
    pub tracked: String,
}

/// What the log says about a test that did not pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    Abort(String),
    Timeout,
    /// A FAIL, with its panic location when the log names one (`file:line`, forward slashes).
    Fail(Option<String>),
    /// Any other nextest status (SIGSEGV, LEAK, ...).
    Other(String),
}

/// A test that did not pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// `binary::test` as nextest prints it after the binary id.
    pub name: String,
    pub observed: Observed,
}

/// The list's lines (comments and blank lines skipped); a malformed line is an error naming it.
pub fn parse_list(text: &str) -> Result<Vec<Known>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = l.split(" | ").map(str::trim).collect();
        let [pattern, outcome, tracked] = parts[..] else {
            return Err(format!("{LIST}:{}: `<test pattern> | <outcome> | <Tracked entry>` expected, not `{l}`", i + 1));
        };
        let outcome = if let Some(code) = outcome.strip_prefix("abort ") {
            Outcome::Abort(code.trim().to_ascii_lowercase())
        } else if outcome == "timeout" {
            Outcome::Timeout
        } else if let Some(at) = outcome.strip_prefix("panic at ") {
            Outcome::PanicAt(at.trim().replace('\\', "/"))
        } else {
            return Err(format!("{LIST}:{}: outcome `{outcome}` is none of `abort 0x<code>`, `timeout`, `panic at <file>:<line>`", i + 1));
        };
        if pattern.is_empty() || tracked.is_empty() {
            return Err(format!("{LIST}:{}: an empty pattern or Tracked entry", i + 1));
        }
        out.push(Known { pattern: pattern.to_string(), outcome, tracked: tracked.to_string() });
    }
    Ok(out)
}

/// Whether `name` matches `pattern`, where `*` matches any run of characters.
pub fn glob(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(at) => rest = &rest[at + mid.len()..],
            None => return false,
        }
    }
    true
}

/// The failed tests of a nextest log, once each, in the order of their first status line; `None` when the log has no `Summary` line (the run did not end:
/// a build failure, a cancelled run), which no class covers.
pub fn failures(log: &str) -> Option<Vec<Failure>> {
    if !log.lines().any(|l| l.trim_start().starts_with("Summary [")) {
        return None;
    }
    let lines: Vec<&str> = log.lines().collect();
    let mut out: Vec<Failure> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let Some((status, rest)) = t.split_once(" [") else { continue };
        if !matches!(status, "FAIL" | "ABORT" | "TIMEOUT") && !status.starts_with("SIG") && status != "LEAK" && status != "LEAK-FAIL" {
            continue;
        }
        // `FAIL [  39.481s] ( 19/235) planet::gpu_all gpu_preflight::name`: the name is the last field.
        let Some(name) = rest.split_whitespace().last().map(str::to_string) else { continue };
        if out.iter().any(|f| f.name == name) {
            continue;
        }
        let observed = match status {
            "ABORT" => {
                let code = lines
                    .get(i + 1)
                    .and_then(|n| n.trim_start().strip_prefix("- with code "))
                    .map(|c| c.split([':', ' ']).next().unwrap_or("").to_ascii_lowercase())
                    .unwrap_or_default();
                Observed::Abort(code)
            }
            "TIMEOUT" => Observed::Timeout,
            "FAIL" => Observed::Fail(panic_at(&lines, &name)),
            other => Observed::Other(other.to_string()),
        };
        out.push(Failure { name, observed });
    }
    Some(out)
}

/// The panic location of test `name` in the log: from its line `thread '<test>' (...) panicked at <file>:<line>:<col>:`, as `file:line` with forward slashes.
fn panic_at(lines: &[&str], name: &str) -> Option<String> {
    let test = name.rsplit_once(' ').map_or(name, |(_, t)| t);
    let line = lines.iter().find(|l| l.contains(&format!("thread '{test}'")) && l.contains("panicked at "))?;
    let at = line.split("panicked at ").nth(1)?.trim_end_matches(':').replace('\\', "/");
    // Drop the column: `file:line:col` to `file:line`.
    let mut parts: Vec<&str> = at.rsplitn(3, ':').collect();
    parts.reverse();
    (parts.len() == 3).then(|| format!("{}:{}", parts[0], parts[1]))
}

/// From the Summary line `... N tests run: M passed ...`: how many tests ran and did not pass (`N - M`).
pub fn summary_not_passed(log: &str) -> Option<usize> {
    let line = log.lines().find(|l| l.trim_start().starts_with("Summary ["))?;
    let after = line.split("] ").nth(1)?;
    let run: usize = after.split(" tests run").next()?.split_whitespace().last()?.parse().ok()?;
    let passed: usize = after.split("run: ").nth(1)?.split(" passed").next()?.trim().parse().ok()?;
    run.checked_sub(passed)
}

/// Whether a known class accepts a failure.
fn accepts(k: &Known, f: &Failure) -> bool {
    glob(&k.pattern, &f.name)
        && match (&k.outcome, &f.observed) {
            (Outcome::Abort(want), Observed::Abort(got)) => want == got,
            (Outcome::Timeout, Observed::Timeout) => true,
            (Outcome::PanicAt(want), Observed::Fail(Some(at))) => at.ends_with(want.as_str()),
            _ => false,
        }
}

/// The verdict on a failed local tier C: `Ok(summary lines)` when every failure is in a listed class whose Tracked entry is in `status`, the run ended, no golden
/// candidate was written and at least one test failed; `Err(reason)` otherwise.
pub fn judge(log: &str, list: &[Known], status: &str, candidates: usize) -> Result<Vec<String>, String> {
    if candidates > 0 {
        return Err(format!("{candidates} candidate golden(s) were written: a golden moved, which no known class covers"));
    }
    let Some(failed) = failures(log) else {
        return Err("the tier C run did not end (no nextest Summary line): no known class covers it".into());
    };
    if failed.is_empty() {
        return Err("the tier C step failed with no failed test in its log: no known class covers it".into());
    }
    // Every test that ran and did not pass must be one of the parsed failures: a status this parser does not know would otherwise hide among known ones.
    match summary_not_passed(log) {
        Some(n) if n == failed.len() => {}
        Some(n) => {
            return Err(format!(
            "the Summary counts {n} test(s) that did not pass, the log names {} failure(s): a failure of a kind this check does not read",
            failed.len()
        ))
        }
        None => return Err("the Summary line gives no `N tests run: M passed` counts: no known class covers the run".into()),
    }
    let tracked: Vec<&Known> = list.iter().filter(|k| status.contains(&format!("**{}**", k.tracked))).collect();
    let mut lines = Vec::new();
    for f in &failed {
        match tracked.iter().find(|k| accepts(k, f)) {
            Some(k) => lines.push(format!("  {} ({}; Tracked: {})", f.name, describe(&f.observed), k.tracked)),
            None => return Err(format!("{} ({}) is in no known class of {LIST}", f.name, describe(&f.observed))),
        }
    }
    let mut out =
        vec![format!("tier C: {} known-class failure(s), listed (CI judges tier C; copy these lines into the PR body):", failed.len())];
    out.extend(lines);
    Ok(out)
}

/// The goldens a change declares it moves (BUILD-068): the names of the `- <name>: <reason>` lines of its proposal's `Expected golden changes:` section (up to the next
/// `## ` heading), sorted, without repeats. A line that is not `- <name>:` (a note) is skipped. Refused: a proposal without the section.
pub fn declared_goldens(proposal: &str) -> Result<Vec<String>, String> {
    let mut lines = proposal.lines().skip_while(|l| l.trim() != "## Expected golden changes:");
    if lines.next().is_none() {
        return Err("the proposal has no `## Expected golden changes:` section".into());
    }
    let mut out: Vec<String> = lines
        .take_while(|l| !l.starts_with("## "))
        .filter_map(|l| l.trim().strip_prefix("- ")?.split_once(':').map(|(n, _)| n.trim().to_string()))
        .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// The `file:line` of the testkit's golden-check panic (`GoldenChecks`: "N golden check(s) failed"), from its source, so the declared mode tracks an edit of it.
pub fn golden_panic_at(testkit_lib: &str) -> Option<String> {
    let k = testkit_lib.lines().position(|l| l.contains("golden check(s) failed") && l.contains("panic!"))?;
    Some(format!("crates/testkit/src/lib.rs:{}", k + 1))
}

/// The verdict on a failed local tier C under a declared change (BUILD-068): `Ok(summary lines)` when the written candidates' names are exactly the `declared`
/// goldens, every failure is either a golden test's golden-check panic (at `golden_at`) or a listed class whose Tracked entry is in `status`, and the run ended;
/// `Err(reason)` otherwise, naming the extra and the missing goldens. Writes and blesses nothing.
pub fn judge_declared(
    log: &str,
    list: &[Known],
    status: &str,
    candidates: &[String],
    declared: &[String],
    golden_at: &str,
) -> Result<Vec<String>, String> {
    let mut written: Vec<String> = candidates.to_vec();
    written.sort();
    written.dedup();
    let extra: Vec<&String> = written.iter().filter(|c| !declared.contains(c)).collect();
    let missing: Vec<&String> = declared.iter().filter(|d| !written.contains(d)).collect();
    if !extra.is_empty() || !missing.is_empty() {
        return Err(format!(
            "the candidates are not the declared golden changes: extra {extra:?}, missing {missing:?} ({} written, {} declared)",
            written.len(),
            declared.len()
        ));
    }
    let Some(failed) = failures(log) else {
        return Err("the tier C run did not end (no nextest Summary line): no known class covers it".into());
    };
    match summary_not_passed(log) {
        Some(n) if n == failed.len() => {}
        Some(n) => {
            return Err(format!(
            "the Summary counts {n} test(s) that did not pass, the log names {} failure(s): a failure of a kind this check does not read",
            failed.len()
        ))
        }
        None => return Err("the Summary line gives no `N tests run: M passed` counts: no known class covers the run".into()),
    }
    let tracked: Vec<&Known> = list.iter().filter(|k| status.contains(&format!("**{}**", k.tracked))).collect();
    let (mut golden, mut known) = (Vec::new(), Vec::new());
    for f in &failed {
        if f.observed == Observed::Fail(Some(golden_at.to_string())) {
            golden.push(format!("  {} (golden check, declared)", f.name));
        } else if let Some(k) = tracked.iter().find(|k| accepts(k, f)) {
            known.push(format!("  {} ({}; Tracked: {})", f.name, describe(&f.observed), k.tracked));
        } else {
            return Err(format!(
                "{} ({}) is neither a declared golden check nor in a known class of {LIST}",
                f.name,
                describe(&f.observed)
            ));
        }
    }
    let mut out = vec![format!(
        "tier C: {} declared golden candidate(s) (the proposal's list, exactly), {} golden test(s), {} known-class failure(s); copy these lines into the PR body:",
        written.len(),
        golden.len(),
        known.len()
    )];
    out.push(format!("  declared: {}", written.join(", ")));
    out.extend(golden);
    out.extend(known);
    Ok(out)
}

fn describe(o: &Observed) -> String {
    match o {
        Observed::Abort(c) => format!("abort {c}"),
        Observed::Timeout => "timeout".into(),
        Observed::Fail(Some(at)) => format!("panic at {at}"),
        Observed::Fail(None) => "fail".into(),
        Observed::Other(s) => s.to_lowercase(),
    }
}

/// The lines of the list whose Tracked entry is missing from `status`: ignored, with a warning each.
pub fn untracked(list: &[Known], status: &str) -> Vec<String> {
    list.iter().filter(|k| !status.contains(&format!("**{}**", k.tracked))).map(|k| format!("{} | {}", k.pattern, k.tracked)).collect()
}

/// The newest `test-gpu-*.log` in the test log folder written at or after `since` (this step's run), if any: an older log is another run's.
pub fn newest_gpu_log(root: &Path, since: std::time::SystemTime) -> Option<std::path::PathBuf> {
    let dir = root.join(crate::testlog::DIR);
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("test-gpu-"))
        .filter_map(|e| e.metadata().and_then(|m| m.modified()).ok().filter(|m| *m >= since).map(|m| (m, e.path())))
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

/// The tier C step of prepush: `test gpu`, and on its failure the known-class verdict (BUILD-056): a pass with the summary printed, or the step's own error.
pub fn gpu_step(root: &Path, declared: Option<&Path>) -> Result<(), String> {
    let since = std::time::SystemTime::now();
    let Err(e) = crate::testcmd::test(root, &["gpu".to_string()]) else { return Ok(()) };
    let Some(log) = newest_gpu_log(root, since).and_then(|p| std::fs::read_to_string(p).ok()) else {
        eprintln!("prepush: no tier C log of this run: the failure is not judged against the known classes");
        return Err(e);
    };
    let list = match std::fs::read_to_string(root.join(LIST)) {
        Ok(t) => parse_list(&t)?,
        Err(err) => {
            eprintln!("prepush: {LIST} cannot be read ({err}): the failure is not judged against the known classes");
            return Err(e);
        }
    };
    let status = std::fs::read_to_string(root.join("docs").join("status.md")).unwrap_or_default();
    for u in untracked(&list, &status) {
        eprintln!("prepush: warning: {LIST} line `{u}` cites no Tracked entry of docs/status.md: ignored (remove the line)");
    }
    // BUILD-068: with `--declared <proposal>` the candidates must be the proposal's declared golden changes, exactly
    if let Some(p) = declared {
        let decl = declared_goldens(&std::fs::read_to_string(p).map_err(|err| format!("{}: {err}", p.display()))?)?;
        let lib = std::fs::read_to_string(root.join("crates").join("testkit").join("src").join("lib.rs")).unwrap_or_default();
        let Some(at) = golden_panic_at(&lib) else {
            eprintln!("prepush: the testkit's golden-check panic is not found: the failure is not judged against the declared goldens");
            return Err(e);
        };
        let names: Vec<String> =
            crate::testcmd::candidates(root).into_iter().map(|(_, n)| n.trim_end_matches(".png").to_string()).collect();
        return match judge_declared(&log, &list, &status, &names, &decl, &at) {
            Ok(summary) => {
                eprintln!("prepush: ===== tier C passes on the declared golden changes and known local classes only (BUILD-068; the goldens wait for golden-review) =====");
                for l in &summary {
                    eprintln!("{l}");
                }
                eprintln!("prepush: =====");
                Ok(())
            }
            Err(why) => {
                eprintln!("prepush: tier C failed outside the declared golden changes and the known classes: {why}");
                Err(e)
            }
        };
    }
    match judge(&log, &list, &status, crate::testcmd::candidates(root).len()) {
        Ok(summary) => {
            eprintln!("prepush: ===== tier C passes on known local classes only (BUILD-056; local-only, CI runs tier C in full) =====");
            for l in &summary {
                eprintln!("{l}");
            }
            eprintln!("prepush: =====");
            Ok(())
        }
        Err(why) => {
            eprintln!("prepush: tier C failed outside the known classes: {why}");
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "## Tracked\n- **Local `test gpu` worse since the display changes** (2026-11-09): ...\n";

    fn list() -> Vec<Known> {
        parse_list(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("known-tier-c.txt")).unwrap()).unwrap()
    }

    /// A nextest log as the local tier C writes it: the given status lines, then the summary.
    fn log(body: &str) -> String {
        // The Summary counts the body's failures as not passed, as nextest does.
        let n = failures(&format!("Summary [\n{body}")).map_or(0, |f| f.len());
        log_counting(body, n)
    }

    /// [`log`] with a Summary that counts `not_passed` tests as not passed.
    fn log_counting(body: &str, not_passed: usize) -> String {
        format!(
            "    Starting 235 tests\n{body}\n     Summary [1328.801s] 235 tests run: {} passed, {not_passed} failed\n{body}\n",
            235 - not_passed
        )
    }

    const ABORT: &str = "       ABORT [  55.757s] ( 11/235) planet::gpu_all gpu_flight::the_flight_matches\n           - with code 0xc0000005: Unzulässiger Zugriff (os error 998)";
    const TIMEOUT: &str =
        "     TIMEOUT [ 331.391s] (188/235) planet::gpu_all gpu_skeleton::the_pipelined_loop_settles_on_the_synchronous_frame";
    const PREFLIGHT: &str = "        FAIL [  39.481s] ( 19/235) planet::gpu_all gpu_preflight::the_preflight_smoke_prints_the_host_facts_and_a_reading_and_never_refuses\n    thread 'gpu_preflight::the_preflight_smoke_prints_the_host_facts_and_a_reading_and_never_refuses' (23588) panicked at crates\\app\\tests\\gpu_preflight.rs:24:9:\n    the refusal took 47 s";

    // spec: BUILD-056
    #[test]
    fn the_tracked_list_parses_and_every_line_names_its_tracked_entry() {
        let l = list();
        assert_eq!(l.len(), 4);
        assert!(l.iter().all(|k| k.tracked == "Local `test gpu` worse since the display changes"));
        assert_eq!(l[0].outcome, Outcome::Abort("0xc0000005".into()));
        assert_eq!(l[2].outcome, Outcome::PanicAt("gpu_preflight.rs:24".into()));
        assert!(parse_list("a | maybe | b").unwrap_err().contains("outcome `maybe`"));
        assert!(parse_list("a | timeout").unwrap_err().contains("expected"));
    }

    // spec: BUILD-056
    #[test]
    fn known_classes_only_pass_with_a_summary_naming_each() {
        let body = format!("{ABORT}\n{TIMEOUT}\n{PREFLIGHT}");
        let s = judge(&log(&body), &list(), STATUS, 0).expect("three known-class failures");
        assert!(s[0].starts_with("tier C: 3 known-class failure(s), listed"), "{s:?}");
        assert!(s.iter().any(|l| l.contains("gpu_flight::the_flight_matches (abort 0xc0000005")));
        assert!(s.iter().any(|l| l.contains("gpu_skeleton::the_pipelined_loop_settles_on_the_synchronous_frame (timeout")));
        assert!(s.iter().any(|l| l.contains("gpu_preflight::") && l.contains("panic at crates/app/tests/gpu_preflight.rs:24")));
        assert_eq!(s.len(), 4, "each failure once, though the log repeats them after the summary");
    }

    // spec: BUILD-056
    #[test]
    fn any_other_failure_a_golden_candidate_or_an_unfinished_run_fails() {
        let l = list();
        // Another test failing.
        let other = "        FAIL [   3.1s] (  5/235) planet::gpu_all gpu_water::the_lit_frame_matches_its_golden\n    thread 'gpu_water::the_lit_frame_matches_its_golden' (1) panicked at crates\\render\\tests\\gpu_water.rs:80:9:";
        assert!(judge(&log(&format!("{ABORT}\n{other}")), &l, STATUS, 0)
            .unwrap_err()
            .contains("gpu_water::the_lit_frame_matches_its_golden"));
        // The known test failing elsewhere than its bound.
        let elsewhere = PREFLIGHT.replace("gpu_preflight.rs:24:9", "gpu_preflight.rs:40:5");
        assert!(judge(&log(&elsewhere), &l, STATUS, 0).is_err(), "gpu_preflight failing at another line is not the 8 s bound");
        // The refusal text (line 23) is a property, not the bound: not covered.
        let text = PREFLIGHT.replace("gpu_preflight.rs:24:9", "gpu_preflight.rs:23:5");
        assert!(judge(&log(&text), &l, STATUS, 0).is_err(), "a wrong refusal text is no known class");
        // The draw-flag test's timeout (the two pipelined loops under oversubscription, spikes/gpu-skeleton) is covered.
        let draw = TIMEOUT.replace(
            "the_pipelined_loop_settles_on_the_synchronous_frame",
            "the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either",
        );
        assert!(judge(&log(&draw), &l, STATUS, 0).is_ok());
        // Another abort code, another gpu_skeleton test timing out, a crash signal.
        assert!(judge(&log(&ABORT.replace("0xc0000005", "0xc0000409")), &l, STATUS, 0).is_err());
        assert!(judge(&log(&TIMEOUT.replace("the_pipelined_loop_settles", "a_terrain_bundle_replays")), &l, STATUS, 0).is_err());
        assert!(judge(&log("     SIGSEGV [ 1.0s] (  1/235) planet::gpu_all gpu_x::y"), &l, STATUS, 0).is_err());
        // A failure of a status this check does not read (EXEC-FAIL), beside known ones: the Summary counts it.
        let unread = format!("{ABORT}\n     EXEC-FAIL [ 0.1s] (  2/235) planet::gpu_all gpu_x::y");
        assert!(judge(&log_counting(&unread, 2), &l, STATUS, 0).unwrap_err().contains("Summary counts 2"));
        assert_eq!(summary_not_passed(&log_counting(ABORT, 4)), Some(4));
        // The real line's shape (test-gpu log of 2026-11-14 12:51): slow and skipped counts beside the others.
        let real = "     Summary [1328.801s] 235 tests run: 231 passed (22 slow), 3 failed, 1 timed out, 3090 skipped";
        assert_eq!(summary_not_passed(real), Some(4));
        assert!(judge(&format!("{ABORT}\n     Summary [1.0s] cancelled\n"), &l, STATUS, 0).unwrap_err().contains("no `N tests run"));
        // A golden candidate, even with only known failures.
        assert!(judge(&log(ABORT), &l, STATUS, 1).unwrap_err().contains("candidate golden"));
        // A run that did not end, and a failed step with no failed test.
        assert!(judge(ABORT, &l, STATUS, 0).unwrap_err().contains("did not end"));
        assert!(judge(&log(""), &l, STATUS, 0).unwrap_err().contains("no failed test"));
    }

    // spec: BUILD-056
    #[test]
    fn a_line_whose_tracked_entry_is_gone_is_ignored() {
        let l = list();
        let without = "## Tracked\n- **Something else**\n";
        assert!(judge(&log(ABORT), &l, without, 0).is_err(), "no Tracked entry: no class");
        assert_eq!(untracked(&l, without).len(), l.len());
        assert!(untracked(&l, STATUS).is_empty());
    }

    // spec: BUILD-056
    #[test]
    fn the_glob_matches_runs_of_characters() {
        assert!(glob("*", "a::b"));
        assert!(glob("gpu_skeleton::the_pipelined_loop_*", "gpu_skeleton::the_pipelined_loop_settles"));
        assert!(!glob("gpu_skeleton::the_pipelined_loop_*", "gpu_skeleton::a_terrain"));
        assert!(glob("a*c*e", "abcde") && !glob("a*c*e", "abde"));
        assert!(glob("exact", "exact") && !glob("exact", "exactly"));
    }

    // spec: BUILD-068, TEST-154
    #[test]
    fn a_proposals_declared_goldens_are_its_named_lines() {
        let p = "## Why\n- x\n\n## Expected golden changes:\n- lit_a: REND-068\n- water_b: because\n- The hashes move too (a note).\n- lit_a: twice\n\n## Bound tests:\n- lit_c: t\n";
        assert_eq!(declared_goldens(p).unwrap(), ["lit_a", "water_b"]);
        assert!(declared_goldens("## Why\n").unwrap_err().contains("no `## Expected golden changes:`"));
        assert_eq!(declared_goldens("## Expected golden changes:\n- none: additive code with no caller\n").unwrap(), ["none"]);
        let lib = "fn a() {}\n        if !self.failures.is_empty() {\n            panic!(\"{} golden check(s) failed:\\n{}\", n, m);\n";
        assert_eq!(golden_panic_at(lib).as_deref(), Some("crates/testkit/src/lib.rs:3"));
        // the real testkit has it
        let real = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/testkit/src/lib.rs")).unwrap();
        assert!(golden_panic_at(&real).is_some());
    }

    // spec: BUILD-068, TEST-154
    #[test]
    fn the_declared_mode_passes_exactly_the_declared_candidates_and_known_classes() {
        let l = list();
        let at = "crates/testkit/src/lib.rs:324";
        let golden = "        FAIL [  19.7s] ( 61/236) planet-render::gpu_all gpu_lit::lit_frames_match_the_adapters_goldens\n    thread 'gpu_lit::lit_frames_match_the_adapters_goldens' (7) panicked at crates\\testkit\\src\\lib.rs:324:13:\n    2 golden check(s) failed:";
        let body = format!("{golden}\n{ABORT}");
        let decl = vec!["lit_a".to_string(), "lit_b".to_string()];
        let cand = vec!["lit_b".to_string(), "lit_a".to_string(), "lit_a".to_string()];
        let s = judge_declared(&log(&body), &l, STATUS, &cand, &decl, at).expect("declared candidates and a known class");
        assert!(
            s[0].starts_with("tier C: 2 declared golden candidate(s)") && s[0].contains("1 golden test(s), 1 known-class failure(s)"),
            "{s:?}"
        );
        assert!(s.iter().any(|x| x.contains("gpu_lit::lit_frames_match_the_adapters_goldens (golden check, declared)")));
        assert!(s.iter().any(|x| x.contains("declared: lit_a, lit_b")));
        // an extra or a missing candidate fails, with the lists
        let e = judge_declared(&log(&body), &l, STATUS, &["lit_a".into(), "lit_c".into()], &decl, at).unwrap_err();
        assert!(e.contains("extra [\"lit_c\"]") && e.contains("missing [\"lit_b\"]"), "{e}");
        // a test failing elsewhere than the golden check, and an unfinished run, fail
        let other = golden.replace("lib.rs:324:13", "lib.rs:40:5");
        assert!(judge_declared(&log(&format!("{other}\n{ABORT}")), &l, STATUS, &cand, &decl, at)
            .unwrap_err()
            .contains("neither a declared golden check"));
        assert!(judge_declared(&body, &l, STATUS, &cand, &decl, at).unwrap_err().contains("did not end"));
        // without the mode, the same run still fails on the candidates (behaviour unchanged)
        assert!(judge(&log(&body), &l, STATUS, 2).unwrap_err().contains("candidate golden"));
    }
}
