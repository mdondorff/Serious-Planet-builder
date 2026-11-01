//! `cargo xtask gate [--small] [--site ID]... [--paths DIR] [--store DIR]`: the no-degradation gate (TEST-062, TEST-063).
//!
//! `--small` runs the window gate on a generated 32-cell world on the software adapter (the tier C test `the_gate_passes_on_a_small_world...`);
//! without it the editor window runs over the real store (read only; outputs under `target/gate/`) once per capture site of the owner test,
//! replaying the site's camera path after a scripted 400 m hill. A failed counter listed in [`KNOWN`] is reported as known and does not fail the run;
//! any other failure does.

use crate::util::{run, Res};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The store of the real run, opened read only.
const STORE: &str = "C:/DEV/SeriousPlanetData/global16384";
/// The camera paths of the owner test (`frame-<n>-path.txt`).
const PATHS: &str = "C:/DEV/SeriousPlanetData/owner-test-2026-10-27";

/// A failure main has at the gate's landing (2026-10-28): the site (`*` is every site), the start of the failure's text, the Tracked entry in
/// `docs/status.md` that holds the numbers, and the ceiling of the failure's key number ([`key_number`]; 0 for none). A failure over its ceiling is
/// new. Never a weakened limit: the table only keeps the run from failing on what the baseline already shows.
pub type Known = (&'static str, &'static str, &'static str, f64);

pub const KNOWN: [Known; 15] = [
    ("frame-116591", "held-back tiles", "Tracked: gate held-back tiles", 170.0),
    ("frame-134050", "held-back tiles", "Tracked: gate held-back tiles", 185.0),
    ("frame-139490", "held-back tiles", "Tracked: gate held-back tiles", 170.0),
    ("frame-181555", "held-back tiles", "Tracked: gate held-back tiles", 155.0),
    ("frame-187750", "held-back tiles", "Tracked: gate held-back tiles", 150.0),
    ("frame-197238", "held-back tiles", "Tracked: gate held-back tiles", 190.0),
    ("frame-31358", "held-back tiles", "Tracked: gate held-back tiles", 12.0),
    ("frame-32034", "held-back tiles", "Tracked: gate held-back tiles", 10.0),
    ("frame-41356", "held-back tiles", "Tracked: gate held-back tiles", 9.0),
    ("frame-41595", "held-back tiles", "Tracked: gate held-back tiles", 14.0),
    ("frame-50119", "held-back tiles", "Tracked: gate held-back tiles", 100.0),
    ("frame-72408", "held-back tiles", "Tracked: gate held-back tiles", 125.0),
    ("frame-78773", "held-back tiles", "Tracked: gate held-back tiles", 140.0),
    ("*", "frames over", "Tracked: gate slow frames", 3000.0),
    ("frame-102263", "horizon maps baked", "Tracked: gate no horizon map at frame-102263", 0.0),
];

/// The number a ceiling bounds: the most tiles held back at once, the longest frame in ms, the bake blocks the main thread decoded; 0 for a failure without one.
pub fn key_number(failure: &str) -> f64 {
    let after = |marker: &str, end: &str| -> f64 {
        failure.split(marker).nth(1).and_then(|s| s.split(end).next()).and_then(|n| n.trim().parse().ok()).unwrap_or(0.0)
    };
    if failure.starts_with("held-back tiles") {
        after("up to ", " at once")
    } else if failure.starts_with("frames over") {
        failure
            .split("(longest frame ")
            .nth(1)
            .and_then(|s| s.split(": ").nth(1))
            .and_then(|s| s.split(" ms").next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(0.0)
    } else {
        0.0
    }
}

/// The failures a window run printed: the lines after `gate FAILED:`, each starting with two spaces.
pub fn failures_in(stderr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in stderr.lines() {
        if line.contains("gate FAILED:") {
            inside = true;
        } else if inside && line.starts_with("  ") {
            out.push(line.trim().to_string());
        } else {
            inside = false;
        }
    }
    out
}

/// Splits the failures of `site` into those the table knows and the new ones.
pub fn split_known<'a>(site: &str, failures: &'a [String], known: &[Known]) -> (Vec<(&'a str, String)>, Vec<String>) {
    let mut is_known = Vec::new();
    let mut new = Vec::new();
    for f in failures {
        match known.iter().find(|(s, start, _, _)| (*s == "*" || *s == site) && f.starts_with(start)) {
            Some((_, _, _, ceiling)) if *ceiling > 0.0 && key_number(f) > *ceiling => {
                new.push(format!("{f} [over its known ceiling {ceiling}]"))
            }
            Some((_, _, entry, _)) => is_known.push((f.as_str(), (*entry).to_string())),
            None => new.push(f.clone()),
        }
    }
    (is_known, new)
}

/// The capture sites in `dir`: `frame-<n>-path.txt` as (`frame-<n>`, path), sorted by name.
pub fn sites(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix("-path.txt")?;
            id.starts_with("frame-").then(|| (id.to_string(), e.path()))
        })
        .collect();
    v.sort();
    v
}

/// `gate --small`: the tier C test over a generated world on the software adapter.
pub fn run_small(root: &Path) -> Res {
    run(
        root,
        "cargo",
        &["test", "-p", "planet", "--test", "gpu_all", "the_gate_passes_on_a_small_world", "--", "--nocapture"],
        &[("PLANET_ADAPTER", "software")],
    )
}

/// The real-store smoke on the software adapter (TEST-073): the editor window over the store (read only), the scripted edit session, no
/// camera path. It checks the store's logic while the desktop is locked; the hardware run of the gate stays the presentation check.
/// `Ok` is the table row of a pass, `Err` the row of a failure.
fn software_smoke(root: &Path, exe: &Path, store: &str) -> Result<String, String> {
    eprintln!("gate: editor --smoke on the real store, software adapter ...");
    let before = store_snapshot(Path::new(store))?;
    let out = Command::new(exe)
        .args(["editor", "--smoke", "--edit", "--no-vsync", "--adapter", "software", "--store", store])
        .current_dir(root)
        .output()
        .map_err(|e| format!("software smoke: ERROR cannot start {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut verdict = smoke_verdict(out.status.success(), &stdout, &stderr);
    // The store is opened read only: no file of it may have appeared, changed size or been touched by the run.
    if let Some(change) = store_changes(&before, &store_snapshot(Path::new(store))?) {
        verdict = Err(format!("the run changed the store, which must stay read only: {change}"));
    }
    match &verdict {
        Ok(line) => Ok(format!("software smoke: PASS  {line}")),
        Err(why) => Err(format!("software smoke: FAIL  {why}")),
    }
}

/// The top-level entries of a store directory with their sizes and modification times (the packs and indexes live there; a subfolder counts
/// as one entry). An unreadable directory is an error, never an empty snapshot.
fn store_snapshot(dir: &Path) -> Result<Vec<(String, u64, Option<std::time::SystemTime>)>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("software smoke: ERROR cannot read the store directory {} to check it stays read only: {e}", dir.display()))?;
    let mut v: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), m.len(), m.modified().ok()))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(v)
}

/// What differs between two snapshots of a store (TEST-085): the first added, removed or changed file, or `None`.
fn store_changes(
    before: &[(String, u64, Option<std::time::SystemTime>)],
    after: &[(String, u64, Option<std::time::SystemTime>)],
) -> Option<String> {
    for (name, size, time) in before {
        match after.iter().find(|a| &a.0 == name) {
            None => return Some(format!("{name} is gone")),
            Some(a) if a.1 != *size => return Some(format!("{name} grew or shrank from {size} to {} bytes", a.1)),
            Some(a) if a.2 != *time => return Some(format!("{name} was modified")),
            Some(_) => {}
        }
    }
    after.iter().find(|a| !before.iter().any(|b| b.0 == a.0)).map(|a| format!("{} is new", a.0))
}

/// The error that ends a gate on a locked desktop (TEST-073, TEST-085): a failed software smoke is a failure (exit 1), a passed one leaves the
/// hardware part NOT RUN, incomplete (exit 3).
fn locked_outcome(software_passed: bool) -> String {
    if software_passed {
        format!(
            "{} the hardware gate was NOT RUN (desktop locked); the software smoke on the real store passed; the run is incomplete, not a pass",
            crate::util::INCOMPLETE
        )
    } else {
        "the gate failed: the software smoke on the real store failed (the hardware part was NOT RUN, desktop locked)".into()
    }
}

/// A smoke run passes when it exits successfully and prints its `[smoke ok]` line.
fn smoke_verdict(success: bool, stdout: &str, stderr: &str) -> Result<String, String> {
    let ok_line = stdout.lines().find(|l| l.contains("[smoke ok]"));
    match (success, ok_line) {
        (true, Some(l)) => Ok(l.trim().chars().take(160).collect()),
        _ => Err(format!(
            "the run exited {} without a `[smoke ok]` line: {}",
            if success { "successfully" } else { "with an error" },
            stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("(no output)")
        )),
    }
}

pub fn run_gate(root: &Path, args: &[String]) -> Res {
    let (mut small, mut only, mut paths, mut store) = (false, Vec::new(), PATHS.to_string(), STORE.to_string());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--small" => small = true,
            "--site" => only.push(it.next().ok_or("--site needs a site id (frame-<n>)")?.clone()),
            "--paths" => paths = it.next().ok_or("--paths needs a directory")?.clone(),
            "--store" => store = it.next().ok_or("--store needs a directory")?.clone(),
            other => return Err(format!("gate: unknown argument {other}")),
        }
    }
    if small {
        return run_small(root);
    }
    run(root, "cargo", &["build", "--release", "-p", "planet"], &[])?;
    let exe = root.join("target").join("release").join(if cfg!(windows) { "planet.exe" } else { "planet" });
    // The software smoke runs whatever the desktop does; its result is printed before a locked desktop ends the gate.
    let software = software_smoke(root, &exe, &store);
    match &software {
        Ok(row) | Err(row) => eprintln!("{row}"),
    }
    // A locked desktop makes the window stall and lose its device: NOT RUN, reported and counted, never a pass (TEST-073).
    if let planet_perf::desktop::Desktop::Locked(reason) = planet_perf::desktop::detect() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        planet_perf::desktop::report("gate", &reason, now, now, false, Some(&root.join(crate::util::NOT_RUN_DIR)));
        return Err(locked_outcome(software.is_ok()));
    }
    let all = sites(Path::new(&paths));
    let chosen: Vec<_> =
        all.iter().filter(|(id, _)| only.is_empty() || only.iter().any(|o| o == id || format!("frame-{o}") == *id)).collect();
    if chosen.is_empty() {
        return Err(format!("gate: no camera path (frame-<n>-path.txt) in {paths}"));
    }
    let out_dir = root.join("target").join("gate");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let mut new_failures = 0usize;
    let mut not_run = 0usize;
    let mut table = Vec::new();
    match software {
        Ok(row) => table.push(row),
        Err(row) => {
            table.push(row);
            new_failures += 1;
        }
    }
    for (id, path) in chosen {
        let report = out_dir.join(format!("{id}.txt"));
        eprintln!("gate: {id} ...");
        let out = Command::new(&exe)
            .args(["editor", "--smoke", "--edit", "--no-vsync", "--adapter", "hardware", "--store", &store])
            .current_dir(root)
            .env("PLANET_GATE", "1")
            .env("PLANET_GATE_PATH", path)
            .env("PLANET_GATE_REPORT", &report)
            .env("PLANET_CRASH_DIR", out_dir.join("crashes"))
            .output()
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let line = stdout.lines().find(|l| l.starts_with("gate: ")).unwrap_or("(no gate line)").to_string();
        let failures = failures_in(&stderr);
        if out.status.success() && failures.is_empty() {
            table.push(format!("{id}: PASS  {line}"));
            continue;
        }
        if failures.is_empty() {
            table.push(format!("{id}: ERROR the run failed without a verdict ({}): {}", out.status, stderr.lines().last().unwrap_or("")));
            new_failures += 1;
            continue;
        }
        let (known, new) = split_known(id, &failures, &KNOWN);
        table.push(format!("{id}: {}  {line}", if new.is_empty() { "KNOWN" } else { "FAIL" }));
        for (f, entry) in known {
            table.push(format!("    known ({entry}): {f}"));
        }
        for f in &new {
            table.push(format!("    NEW: {f}"));
        }
        new_failures += new.len();
    }
    // The step between levels (criterion 3, TEST-064): measured on the base field, read only.
    eprintln!("gate: the step between levels ...");
    let steps = Command::new("cargo")
        .args(["test", "-p", "planet", "--release", "--lib", "base_steps_report", "--", "--ignored", "--nocapture"])
        .current_dir(root)
        .env("PLANET_ANET_STORE", &store)
        .env("PLANET_ANET_RES", "16384")
        .env("PLANET_ANET_MIN_HEIGHT_M", "150")
        .output()
        .map_err(|e| format!("cannot start cargo test: {e}"))?;
    let text = String::from_utf8_lossy(&steps.stdout).into_owned();
    match planet_app_level_steps(&text) {
        Some(levels) if steps.status.success() => {
            let worst = levels.iter().map(|&(_, p99)| p99).fold(0.0, f64::max);
            let over: Vec<String> = levels
                .iter()
                .filter(|&&(_, p99)| p99 > LEVEL_STEP_BOUND_M)
                .map(|&(l, p99)| format!("level {l}/{} p99 {p99:.3} m", l - 1))
                .collect();
            if over.is_empty() {
                table.push(format!(
                    "levels: PASS (measured on the base field: worst p99 {worst:.3} m over {} levels, bound {LEVEL_STEP_BOUND_M} m)",
                    levels.len()
                ));
            } else {
                table.push(format!("levels: NEW: step over the bound {LEVEL_STEP_BOUND_M} m on the base field: {}", over.join(", ")));
                new_failures += 1;
            }
        }
        _ => {
            table.push("levels: ERROR the steps report did not run".to_string());
            new_failures += 1;
        }
    }
    // The production store opens with this build without a rebuild (TEST-075, ADR 0086 decision 3): read only; NOT RUN is not a pass.
    eprintln!("gate: the store opens without a rebuild ...");
    let open = Command::new("cargo")
        .args(["test", "-p", "planet", "--release", "--test", "it", "the_real_store_opens", "--", "--ignored", "--nocapture"])
        .current_dir(root)
        .env("PLANET_REAL_STORE", &store)
        .output()
        .map_err(|e| format!("cannot start cargo test: {e}"))?;
    let text = String::from_utf8_lossy(&open.stdout).into_owned();
    match text.lines().find(|l| l.starts_with("real store: ")) {
        Some(l) if open.status.success() && l.starts_with("real store: MATCH") => table.push(format!("store: PASS ({l})")),
        Some(l) if l.starts_with("real store: NOT RUN") => {
            table.push(format!("store: NOT RUN, not a pass ({l})"));
            not_run += 1;
        }
        _ => {
            table.push("store: FAIL the store does not open with this build (run the test for the message)".to_string());
            new_failures += 1;
        }
    }
    let summary = table.join("\n");
    let _ = std::fs::write(out_dir.join("summary.txt"), &summary);
    println!("{summary}");
    verdict(new_failures, not_run)
}

/// The end of a gate run: a failure is code 1; a check that was NOT RUN with nothing failed is incomplete (code 3), never a pass (TEST-073, TEST-075).
fn verdict(new_failures: usize, not_run: usize) -> Result<(), String> {
    if new_failures > 0 {
        Err(format!("gate: {new_failures} failure(s) not in the known list"))
    } else if not_run > 0 {
        Err(format!("{} the gate had {not_run} check(s) NOT RUN; the run is incomplete, not a pass", crate::util::INCOMPLETE))
    } else {
        Ok(())
    }
}

/// The p99 step against the parent per level in a `base_steps_report` (the parse lives in `planet::gate`, which xtask does not depend on: a copy of its few lines); `None` when the report has no level line.
fn planet_app_level_steps(report: &str) -> Option<Vec<(u8, f64)>> {
    let mut levels = Vec::new();
    for l in report.lines() {
        let Some(rest) = l.trim().strip_prefix("level ") else { continue };
        let Some((level, rest)) = rest.split_once(':') else { continue };
        let Some(p99) =
            rest.split("against the parent p99 ").nth(1).and_then(|s| s.split(" m").next()).and_then(|s| s.trim().parse::<f64>().ok())
        else {
            continue;
        };
        let Ok(level) = level.trim().parse::<u8>() else { continue };
        levels.push((level, p99));
    }
    (!levels.is_empty()).then_some(levels)
}

/// The most a level's p99 step against its parent may be on the base field, in m (TEST-064).
const LEVEL_STEP_BOUND_M: f64 = 0.25;

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-075
    #[test]
    fn a_gate_with_a_check_not_run_and_no_failure_is_incomplete_and_a_failure_stays_a_failure() {
        assert!(verdict(0, 0).is_ok());
        let e = verdict(0, 1).unwrap_err();
        assert!(e.starts_with(crate::util::INCOMPLETE), "{e}");
        let e = verdict(2, 1).unwrap_err();
        assert!(!e.starts_with(crate::util::INCOMPLETE) && e.contains("2 failure"), "{e}");
    }

    // spec: TEST-085
    #[test]
    fn a_software_smoke_passes_on_its_ok_line_and_fails_without_one_naming_the_last_stderr_line() {
        let ok = smoke_verdict(
            true,
            "adapter: x
planet editor [smoke ok]: 90 frames drawn
",
            "",
        )
        .unwrap();
        assert!(ok.contains("[smoke ok]") && ok.contains("90 frames"), "{ok}");
        let no_line = smoke_verdict(
            true,
            "adapter: x
",
            "warning: a

",
        )
        .unwrap_err();
        assert!(no_line.contains("without a `[smoke ok]` line") && no_line.contains("warning: a"), "{no_line}");
        let crashed = smoke_verdict(
            false,
            "planet editor [smoke ok]: 1 frames
",
            "panicked at x
",
        )
        .unwrap_err();
        assert!(crashed.contains("with an error") && crashed.contains("panicked at x"), "{crashed}");
    }

    // spec: TEST-085
    #[test]
    fn a_run_that_adds_changes_or_removes_a_store_file_is_named_and_an_untouched_store_is_not() {
        let dir = std::env::temp_dir().join("gate-store-snapshot-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pack-1.tpack"), b"abc").unwrap();
        let before = store_snapshot(&dir).unwrap();
        assert_eq!(store_changes(&before, &store_snapshot(&dir).unwrap()), None);
        std::fs::write(dir.join("pack-1.tpack"), b"abcd").unwrap();
        assert!(store_changes(&before, &store_snapshot(&dir).unwrap()).unwrap().contains("pack-1.tpack grew or shrank from 3 to 4"));
        std::fs::write(dir.join("pack-1.tpack"), b"abc").unwrap();
        let touched = std::fs::OpenOptions::new().write(true).open(dir.join("pack-1.tpack")).unwrap();
        touched.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
        drop(touched);
        assert!(store_changes(&before, &store_snapshot(&dir).unwrap()).unwrap().contains("pack-1.tpack was modified"));
        let before = store_snapshot(&dir).unwrap();
        std::fs::write(dir.join("pack-2.tpack"), b"x").unwrap();
        assert!(store_changes(&before, &store_snapshot(&dir).unwrap()).unwrap().contains("pack-2.tpack is new"));
        std::fs::remove_file(dir.join("pack-1.tpack")).unwrap();
        assert!(store_changes(&before, &store_snapshot(&dir).unwrap()).unwrap().contains("pack-1.tpack is gone"));
    }

    // spec: TEST-085
    #[test]
    fn a_locked_desktop_ends_the_gate_incomplete_after_a_passed_software_smoke_and_failed_after_a_failed_one() {
        assert!(locked_outcome(true).starts_with(crate::util::INCOMPLETE));
        let failed = locked_outcome(false);
        assert!(!failed.starts_with(crate::util::INCOMPLETE) && failed.contains("software smoke"), "{failed}");
        assert!(store_snapshot(Path::new("no/such/store/dir")).unwrap_err().contains("cannot read the store directory"));
    }

    // spec: TEST-063
    #[test]
    fn the_failures_of_a_run_are_the_indented_lines_after_the_verdict_and_known_ones_are_split_off() {
        let err = "noise\ngate FAILED:\n  held-back tiles with no edit in reach (ADR 0074): on 3 frames\n  panics caught: 1\nother\n";
        let f = failures_in(err);
        assert_eq!(f.len(), 2, "{f:?}");
        let known = [("frame-1", "held-back tiles", "Tracked: held-back at frame-1", 10.0)];
        let (k, n) = split_known("frame-1", &f, &known);
        assert_eq!((k.len(), n.len()), (1, 1));
        assert_eq!(n[0], "panics caught: 1");
        let (k, n) = split_known("frame-2", &f, &known);
        assert_eq!((k.len(), n.len()), (0, 2), "a known failure is known at its site only");
        assert_eq!(split_known("frame-2", &f, &[("*", "panics", "t", 0.0)]).0.len(), 1, "* matches every site");
        assert!(failures_in("all fine\n").is_empty());
    }

    // spec: TEST-063
    #[test]
    fn a_main_thread_block_decode_is_never_known_so_it_fails_the_gate_at_every_site() {
        let decode =
            "bake blocks decoded on the main thread (VIEW-061): 1 in 1 frames; first in frame 25 in \"camera\" (1 blocks)".to_string();
        let one = [decode];
        for site in ["frame-102263", "frame-139490", "frame-197238"] {
            let (k, n) = split_known(site, &one, &KNOWN);
            assert_eq!((k.len(), n.len()), (0, 1), "{site}: {n:?}");
        }
    }
    // spec: TEST-063
    #[test]
    fn a_known_failure_over_its_ceiling_is_new() {
        let held = |n: u32| format!("held-back tiles with no edit in reach (ADR 0074): on 9 frames, up to {n} at once; frame 3: x");
        let slow = "frames over 100 ms: 1 (longest frame 64: 2149 ms, most of it in \"horizon frame\" at 2125 ms)".to_string();
        assert_eq!((key_number(&held(152)), key_number(&slow)), (152.0, 2149.0));
        let h152 = [held(152)];
        let (k, n) = split_known("frame-116591", &h152, &KNOWN);
        assert_eq!((k.len(), n.len()), (1, 0));
        let h400 = [held(400)];
        let (k, n) = split_known("frame-116591", &h400, &KNOWN);
        assert_eq!((k.len(), n.len()), (0, 1), "{n:?}");
        assert!(n[0].contains("over its known ceiling"));
        let slow = [slow];
        assert_eq!(split_known("frame-116591", &slow, &KNOWN).0.len(), 1, "slow frames are known at every site");
        let worse = [slow[0].replace("2149 ms", "4000 ms")];
        assert_eq!(split_known("frame-116591", &worse, &KNOWN).1.len(), 1, "over the 3 s ceiling");
    }
    // spec: TEST-063
    #[test]
    fn the_sites_are_the_path_files_of_the_owner_test() {
        let dir = std::env::temp_dir().join(format!("planet-gate-sites-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["frame-9-path.txt", "frame-10-path.txt", "frame-9.bundle", "other-path.txt"] {
            std::fs::write(dir.join(n), "x").unwrap();
        }
        let ids: Vec<String> = sites(&dir).into_iter().map(|s| s.0).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(ids, ["frame-10", "frame-9"]);
    }

    // spec: TEST-064
    #[test]
    fn the_steps_per_level_are_read_from_the_report() {
        let r = "level 13: against the parent p99 3.0 m
level 14: against the parent p99 1.850 m, max 3
level 16: against the parent p99 0.171 m, max 1
";
        assert_eq!(planet_app_level_steps(r), Some(vec![(13, 3.0), (14, 1.85), (16, 0.171)]));
        assert_eq!(planet_app_level_steps("nothing"), None, "no report is not a pass");
    }
}
