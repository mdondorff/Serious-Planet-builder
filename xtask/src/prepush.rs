//! `cargo xtask prepush`: what to run before pushing a PR branch while waiting is cheap (BUILD-016).
//!
//! The steps, in order, stopping at the first failure: `check` (fmt, clippy, layering), `test fast`, and `test gpu` (tier C on the
//! software adapter) when the branch's diff against `origin/main` touches a crate or file that produces goldens. It prints each
//! step's time and one verdict line. Lavapipe-only golden differences stay CI's job.

use crate::testcmd;
use crate::util::capture;
use std::path::Path;
use std::time::Instant;

type Res = Result<(), String>;

/// Directories under `crates/` whose changes can move a golden image or a debug view: the renderer and what it draws from.
/// The editor, perf and xtask crates cannot.
const GOLDEN_CRATES: [&str; 9] = ["render", "frame", "generators", "core", "streaming", "world-def", "cache", "app", "testkit"];

/// True when a changed path can move a golden: a file of a golden-producing crate, a shader, a golden itself, or a dependency or
/// toolchain change that reaches every crate.
pub fn touches_goldens(path: &str) -> bool {
    let p = path.replace('\\', "/");
    if p.ends_with(".wgsl") || p.starts_with("tests/goldens/") || p == "Cargo.toml" || p == "Cargo.lock" || p.starts_with(".cargo/") {
        return true;
    }
    p.strip_prefix("crates/").and_then(|rest| rest.split('/').next()).is_some_and(|dir| GOLDEN_CRATES.contains(&dir))
}

/// The paths among `changed` that make `test gpu` part of the run, in the order given.
pub fn golden_triggers(changed: &[String]) -> Vec<&str> {
    changed.iter().map(String::as_str).filter(|p| touches_goldens(p)).collect()
}

/// One step's outcome.
pub struct Step {
    pub name: String,
    pub secs: f64,
    pub ok: bool,
}

/// The one-line verdict: every step with its time, and the total.
pub fn verdict_line(steps: &[Step], skipped_gpu: bool) -> String {
    let total: f64 = steps.iter().map(|s| s.secs).sum();
    let failed = steps.iter().find(|s| !s.ok);
    let parts: Vec<String> = steps.iter().map(|s| format!("{} {} {:.0} s", s.name, if s.ok { "ok" } else { "FAILED" }, s.secs)).collect();
    let gpu = if skipped_gpu { "; test gpu skipped (no golden-producing path changed)" } else { "" };
    match failed {
        None => format!("prepush OK in {total:.0} s: {}{gpu}", parts.join(", ")),
        Some(f) => format!("prepush FAILED at {} after {total:.0} s: {}", f.name, parts.join(", ")),
    }
}

/// True when the changed paths hold no crate path: `test fast` would select no crate (BUILD-021), so the code checks have nothing to
/// look at and the run is `spec-lint` (and `test gpu` when a golden changed). An empty list is the whole workspace, not this case.
pub fn no_crate_changed(changed: &[String]) -> bool {
    !changed.is_empty() && testcmd::packages_for_changes(changed).is_empty()
}

/// The object id `git merge-tree --write-tree` prints first: 40 (SHA-1) or 64 (SHA-256) hex digits on the first line.
pub fn tree_id(output: &str) -> Option<String> {
    let first = output.lines().next()?.trim();
    ((first.len() == 40 || first.len() == 64) && first.bytes().all(|b| b.is_ascii_hexdigit())).then(|| first.to_string())
}

/// `spec-lint` on the branch merged onto the current `origin/main` (BUILD-022): what main will hold once the PR lands, so a requirement
/// ID or a spec that another change added meanwhile collides here and not on main. The merge is made by `git merge-tree` in the object
/// database and checked out in a temporary worktree; the branch, the index and the working tree are not touched. A fetch that fails
/// (offline) or a branch that already holds `origin/main` has nothing to add and passes; a branch that does not merge cleanly fails.
pub fn merged_spec_lint(root: &Path) -> Res {
    if let Err(e) = capture(root, "git", &["fetch", "--quiet", "origin", "main"]) {
        eprintln!("prepush: cannot fetch origin/main ({}), so the merged-tree spec-lint is skipped", e.lines().next().unwrap_or(""));
        return Ok(());
    }
    if capture(root, "git", &["merge-base", "--is-ancestor", "origin/main", "HEAD"]).is_ok() {
        eprintln!("prepush: the branch holds origin/main, so the merged tree is the branch (its spec-lint is the next step)");
        return Ok(());
    }
    // `merge-tree` names the conflicting files on stdout (exit 1), which `capture` would drop.
    let out = std::process::Command::new("git")
        .args(["merge-tree", "--write-tree", "HEAD", "origin/main"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot start `git merge-tree`: {e}"))?;
    let merged = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let shown: Vec<&str> = merged.lines().skip(1).filter(|l| !l.trim().is_empty()).take(8).collect();
        return Err(format!(
            "the branch does not merge cleanly onto origin/main: merge origin/main into it first\n  {}",
            if shown.is_empty() { String::from_utf8_lossy(&out.stderr).trim().to_string() } else { shown.join("\n  ") }
        ));
    }
    let tree = tree_id(&merged).ok_or_else(|| format!("git merge-tree printed no tree id: {merged}"))?;
    sweep_stale_worktrees(root);
    let tmp = std::env::temp_dir().join(format!("planet-prepush-merged-{}", std::process::id()));
    let tmp_s = tmp.to_string_lossy().into_owned();
    capture(root, "git", &["worktree", "add", "--detach", "--no-checkout", "--quiet", &tmp_s, "HEAD"])?;
    let result = capture(&tmp, "git", &["read-tree", "-u", "--reset", &tree]).and_then(|_| crate::speclint::run(&tmp));
    if let Err(e) = capture(root, "git", &["worktree", "remove", "--force", &tmp_s]) {
        eprintln!(
            "prepush: the temporary worktree {tmp_s} was not removed ({}); delete it and run `git worktree prune`",
            e.lines().next().unwrap_or("")
        );
    }
    result.map_err(|e| format!("spec-lint of the branch merged onto origin/main: {e}"))
}

/// Removes the temporary worktrees of merged-tree runs that were killed: `planet-prepush-merged-*` directories in the temp directory not
/// touched for an hour (a live run, even of another session, is younger).
fn sweep_stale_worktrees(root: &Path) {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stale = name.starts_with("planet-prepush-merged-")
            && e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600);
        if stale {
            let path = e.path().to_string_lossy().into_owned();
            let _ = capture(root, "git", &["worktree", "remove", "--force", &path]);
            let _ = std::fs::remove_dir_all(e.path());
            let _ = capture(root, "git", &["worktree", "prune"]);
        }
    }
}

/// The steps `prepush` runs for a diff, in order (BUILD-016, BUILD-021): `spec-lint` alone when no changed path is a crate path, else
/// `check` and `test fast`; `test gpu` after them when a golden-producing path changed.
pub fn plan_steps(changed: &[String]) -> Vec<&'static str> {
    let mut steps = vec!["merged spec-lint"];
    steps.extend(if no_crate_changed(changed) { vec!["spec-lint"] } else { vec!["check", "test fast"] });
    if !golden_triggers(changed).is_empty() {
        steps.push("test gpu");
    }
    steps
}

pub fn run(root: &Path) -> Res {
    let changed = testcmd::changed_paths(root);
    let triggers = golden_triggers(&changed);
    let with_gpu = !triggers.is_empty();
    if with_gpu {
        let shown: Vec<&str> = triggers.iter().copied().take(3).collect();
        eprintln!("prepush: test gpu is part of the run ({} golden-producing path(s), e.g. {})", triggers.len(), shown.join(", "));
    } else {
        eprintln!("prepush: no golden-producing path changed, so test gpu is skipped");
    }
    let mut steps: Vec<Step> = Vec::new();
    type Action<'a> = Box<dyn Fn() -> Res + 'a>;
    let mut plan: Vec<(&str, Action)> = Vec::new();
    for name in plan_steps(&changed) {
        let action: Action = match name {
            "merged spec-lint" => Box::new(|| merged_spec_lint(root)),
            "spec-lint" => {
                eprintln!("prepush: no crate path changed, so check and test fast are skipped; spec-lint runs");
                Box::new(|| crate::speclint::run(root))
            }
            "check" => Box::new(|| testcmd::check(root)),
            "test fast" => Box::new(|| testcmd::test(root, &["fast".to_string()])),
            _ => Box::new(|| testcmd::test(root, &["gpu".to_string()])),
        };
        plan.push((name, action));
    }
    for (name, step) in plan {
        eprintln!("prepush: {name} ...");
        let started = Instant::now();
        let result = step();
        steps.push(Step { name: name.to_string(), secs: started.elapsed().as_secs_f64(), ok: result.is_ok() });
        if let Err(e) = result {
            eprintln!("{}", verdict_line(&steps, false));
            return Err(e);
        }
    }
    eprintln!("{}", verdict_line(&steps, !with_gpu));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // spec: BUILD-016
    // spec: BUILD-021
    // spec: BUILD-022
    #[test]
    fn the_plan_of_a_diff_is_spec_lint_alone_for_docs_and_the_gpu_tier_follows_a_golden_path() {
        assert_eq!(plan_steps(&paths(&["docs/status.md"])), ["merged spec-lint", "spec-lint"]);
        assert_eq!(plan_steps(&paths(&["docs/status.md", "crates/editor/src/lib.rs"])), ["merged spec-lint", "check", "test fast"]);
        assert_eq!(plan_steps(&paths(&["crates/render/src/terrain.rs"])), ["merged spec-lint", "check", "test fast", "test gpu"]);
        assert_eq!(
            plan_steps(&paths(&["spikes/x/shader.wgsl", "docs/a.md"])),
            ["merged spec-lint", "spec-lint", "test gpu"],
            "a golden path outside the crates"
        );
        assert_eq!(plan_steps(&[]), ["merged spec-lint", "check", "test fast"], "nothing changed is the whole workspace");

        // The tree id git prints first (BUILD-022); anything else is not one.
        let id = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(tree_id(&format!("{id}\n\nAuto-merging x\n")).as_deref(), Some(id));
        assert_eq!(tree_id(&format!("{id}{id}\n")).as_deref(), None, "80 digits");
        assert_eq!(tree_id("CONFLICT (content): Merge conflict in x\n"), None);
        assert_eq!(tree_id(""), None);
    }

    // spec: BUILD-016
    #[test]
    fn a_change_to_the_renderer_or_what_it_draws_from_needs_the_gpu_tier() {
        for p in [
            "crates/render/src/terrain.rs",
            "crates/render/tests/gpu_water.rs",
            "crates/frame/src/look/water.rs",
            "crates/generators/src/height.rs",
            "crates/core/src/vec3.rs",
            "crates/app/src/window.rs",
            "crates/render\\src\\ui.rs",
            "crates/render/src/terrain.wgsl",
            "tests/goldens/software/lit_orbit.png",
            "Cargo.lock",
            ".cargo/config.toml",
        ] {
            assert!(touches_goldens(p), "{p}");
        }
    }

    // spec: BUILD-016
    #[test]
    fn docs_specs_the_editor_perf_and_xtask_do_not() {
        for p in [
            "docs/log.d/2026-10-19-x.md",
            "openspec/specs/viewer/spec.md",
            "CLAUDE.md",
            ".github/workflows/ci.yml",
            "crates/editor/src/lib.rs",
            "crates/perf/src/calibration.rs",
            "xtask/src/prepush.rs",
            "crates/renderer-notes/readme.md",
        ] {
            assert!(!touches_goldens(p), "{p}");
        }
    }

    // spec: BUILD-016
    #[test]
    fn the_triggers_are_the_golden_paths_among_the_changed_ones_in_order() {
        let changed = paths(&["docs/status.md", "crates/frame/src/pick.rs", "xtask/src/main.rs", "crates/render/src/ui.rs"]);
        assert_eq!(golden_triggers(&changed), ["crates/frame/src/pick.rs", "crates/render/src/ui.rs"]);
        assert!(golden_triggers(&paths(&["docs/status.md", "xtask/src/main.rs"])).is_empty());
        assert!(golden_triggers(&[]).is_empty());
    }

    // spec: BUILD-021
    #[test]
    fn a_diff_with_no_crate_path_runs_only_spec_lint_and_nothing_at_all_is_the_whole_workspace() {
        assert!(no_crate_changed(&paths(&["docs/status.md", "openspec/specs/viewer/spec.md", "spikes/x/results/a.md"])));
        assert!(
            !no_crate_changed(&paths(&["tests/goldens/software/a.png"])),
            "goldens are read by the xtask checks, and test gpu runs too"
        );
        assert!(!no_crate_changed(&paths(&["openspec/changes/x/tasks.md"])), "the unarchived-change check reads it");
        assert!(!no_crate_changed(&paths(&["docs/status.md", "crates/frame/src/a.rs"])));
        assert!(!no_crate_changed(&paths(&["Cargo.lock"])));
        assert!(!no_crate_changed(&paths(&["xtask/src/main.rs"])));
        assert!(!no_crate_changed(&paths(&[".github/workflows/ci.yml"])));
        assert!(!no_crate_changed(&[]), "nothing changed is the whole workspace (BUILD-003)");
    }

    // spec: BUILD-016
    #[test]
    fn the_verdict_is_one_line_with_each_steps_time_and_names_the_failed_step() {
        let ok = |n: &str, s: f64| Step { name: n.into(), secs: s, ok: true };
        let line = verdict_line(&[ok("check", 20.0), ok("test fast", 31.0)], true);
        assert_eq!(line, "prepush OK in 51 s: check ok 20 s, test fast ok 31 s; test gpu skipped (no golden-producing path changed)");
        assert!(!line.contains('\n'));
        let bad = verdict_line(&[ok("check", 20.0), Step { name: "test fast".into(), secs: 9.0, ok: false }], false);
        assert_eq!(bad, "prepush FAILED at test fast after 29 s: check ok 20 s, test fast FAILED 9 s");
    }
}
