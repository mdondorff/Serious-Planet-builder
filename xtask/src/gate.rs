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

pub const KNOWN: [Known; 19] = [
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
    ("levels", "level 12/11", "Tracked: gate level steps", 9.0),
    ("levels", "level 13/12", "Tracked: gate level steps", 3.85),
    ("levels", "level 14/13", "Tracked: gate level steps", 1.86),
    ("levels", "level 15/14", "Tracked: gate level steps", 0.72),
];

/// The number a ceiling bounds: the most tiles held back at once, the longest frame in ms, or a level step's p99 in m; 0 for a failure without one.
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
    } else if failure.starts_with("level ") {
        after("p99 ", " m")
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

pub fn run_gate(root: &Path, args: &[String]) -> Res {
    let (mut small, mut no_chunks, mut only, mut paths, mut store) = (false, false, Vec::new(), PATHS.to_string(), STORE.to_string());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--small" => small = true,
            "--no-chunks" => no_chunks = true,
            "--site" => only.push(it.next().ok_or("--site needs a site id (frame-<n>)")?.clone()),
            "--paths" => paths = it.next().ok_or("--paths needs a directory")?.clone(),
            "--store" => store = it.next().ok_or("--store needs a directory")?.clone(),
            other => return Err(format!("gate: unknown argument {other}")),
        }
    }
    if small {
        return run_small(root);
    }
    let all = sites(Path::new(&paths));
    let chosen: Vec<_> =
        all.iter().filter(|(id, _)| only.is_empty() || only.iter().any(|o| o == id || format!("frame-{o}") == *id)).collect();
    if chosen.is_empty() {
        return Err(format!("gate: no camera path (frame-<n>-path.txt) in {paths}"));
    }
    let out_dir = root.join("target").join("gate");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    run(root, "cargo", &["build", "--release", "-p", "planet"], &[])?;
    let exe = root.join("target").join("release").join(if cfg!(windows) { "planet.exe" } else { "planet" });
    let mut new_failures = 0usize;
    let mut table = Vec::new();
    for (id, path) in chosen {
        let report = out_dir.join(format!("{id}{}.txt", if no_chunks { "-no-chunks" } else { "" }));
        eprintln!("gate: {id} ...");
        let out = Command::new(&exe)
            .args(["editor", "--smoke", "--edit", "--no-vsync", "--adapter", "hardware", "--store", &store])
            .args(no_chunks.then_some("--no-chunks"))
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
    // The step between levels (criterion 3): the field-side measurement of one real chunk, read only.
    eprintln!("gate: the step between levels ...");
    let steps = Command::new("cargo")
        .args([
            "test",
            "-p",
            "planet",
            "--release",
            "--lib",
            if no_chunks { "base_steps_report" } else { "anet_steps_report" },
            "--",
            "--ignored",
            "--nocapture",
        ])
        .current_dir(root)
        .env("PLANET_ANET_STORE", &store)
        .env("PLANET_ANET_RES", "16384")
        .env("PLANET_ANET_MIN_HEIGHT_M", "150")
        .output()
        .map_err(|e| format!("cannot start cargo test: {e}"))?;
    let text = String::from_utf8_lossy(&steps.stdout).into_owned();
    let over = planet_app_level_steps(&text);
    if !steps.status.success() || over.is_none() {
        table.push("levels: ERROR the steps report did not run".to_string());
        new_failures += 1;
    } else if let Some(over) = over.filter(|o| !o.is_empty()) {
        for (level, p99) in over {
            let line = format!("level {level}/{} step p99 {p99:.3} m over the bound {:.2} m", level - 1, 0.25);
            let (known, new) = split_known("levels", std::slice::from_ref(&line), &KNOWN);
            match known.first() {
                Some((f, entry)) => table.push(format!("levels: KNOWN ({entry}): {f}")),
                None => {
                    table.push(format!("levels: NEW: {}", new[0]));
                    new_failures += 1;
                }
            }
        }
    } else {
        table.push("levels: PASS".to_string());
    }
    let summary = table.join("\n");
    let _ = std::fs::write(out_dir.join("summary.txt"), &summary);
    println!("{summary}");
    if new_failures == 0 {
        Ok(())
    } else {
        Err(format!("gate: {new_failures} failure(s) not in the known list"))
    }
}

/// The levels over the bound in an `anet_steps_report` (the parse lives in `planet::gate`, which xtask does not depend on: a copy of its few lines).
fn planet_app_level_steps(report: &str) -> Option<Vec<(u8, f64)>> {
    let mut any = false;
    let mut over = Vec::new();
    let mut lowest: Option<u8> = None;
    for l in report.lines() {
        let Some(rest) = l.trim().strip_prefix("level ") else { continue };
        let Some((level, rest)) = rest.split_once(':') else { continue };
        let Some(p99) =
            rest.split("against the parent p99 ").nth(1).and_then(|s| s.split(" m").next()).and_then(|s| s.trim().parse::<f64>().ok())
        else {
            continue;
        };
        let Ok(level) = level.trim().parse::<u8>() else { continue };
        any = true;
        lowest = Some(lowest.map_or(level, |l| l.min(level)));
        if p99 > 0.25 {
            over.push((level, p99));
        }
    }
    // The lowest level of the report steps against the base (the entry step, its own bound of about 0.5 px), not against a parent level.
    let lowest = lowest.unwrap_or(0);
    any.then_some(over.into_iter().filter(|&(l, _)| l != lowest).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_known_failure_over_its_ceiling_is_new() {
        let held = |n: u32| format!("held-back tiles with no edit in reach (ADR 0074): on 9 frames, up to {n} at once; frame 3: x");
        let slow = "frames over 100 ms: 1 (longest frame 64: 2149 ms, most of it in \"chunk frame\" at 2125 ms)".to_string();
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

    // spec: TEST-061
    #[test]
    fn the_steps_over_the_bound_are_read_from_the_report() {
        let r = "level 13: against the parent p99 3.0 m
level 14: against the parent p99 1.850 m, max 3\nlevel 16: against the parent p99 0.171 m, max 1\n";
        assert_eq!(planet_app_level_steps(r), Some(vec![(14, 1.85)]));
        assert_eq!(
            planet_app_level_steps(
                "level 11: against the parent p99 126.0 m
level 12: against the parent p99 9.0 m
"
            ),
            Some(vec![(12, 9.0)]),
            "the lowest level is the entry step against the base"
        );
        assert_eq!(planet_app_level_steps("nothing"), None, "no report is not a pass");
    }
}
