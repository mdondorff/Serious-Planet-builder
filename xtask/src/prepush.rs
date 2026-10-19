//! `cargo xtask prepush`: what to run before pushing a PR branch while waiting is cheap (BUILD-016).
//!
//! The steps, in order, stopping at the first failure: `check` (fmt, clippy, layering), `test fast`, and `test gpu` (tier C on the
//! software adapter) when the branch's diff against `origin/main` touches a crate or file that produces goldens. It prints each
//! step's time and one verdict line. Lavapipe-only golden differences stay CI's job.

use crate::testcmd;
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
    let mut plan: Vec<(&str, Action)> =
        vec![("check", Box::new(|| testcmd::check(root))), ("test fast", Box::new(|| testcmd::test(root, &["fast".to_string()])))];
    if with_gpu {
        plan.push(("test gpu", Box::new(|| testcmd::test(root, &["gpu".to_string()]))));
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
