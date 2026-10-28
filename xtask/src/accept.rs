//! `cargo xtask accept <milestone>`: runs the automated part of milestone acceptance and writes
//! `target/review/accept-<milestone>.md`. The tier C run covers both the software adapter and the discrete GPU
//! (`--real`; the command fails on a machine without one). M4 and M5 leave `--real` to the manual list and also judge the perf logs
//! of a window, refusing without a valid one (TEST-021, TEST-026). Criteria that need CI, the owner or Rasierklinge are listed
//! as such, never as passed.

use crate::util::{run as exec, Res};
use planet_perf::accept::SceneFiles;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Step {
    name: &'static str,
    outcome: Result<f64, String>,
    /// Said beside `pass` in the table (a number the step reports).
    note: String,
}

fn step(name: &'static str, f: impl FnOnce() -> Res) -> Step {
    let t = Instant::now();
    let outcome = f().map(|_| t.elapsed().as_secs_f64());
    Step { name, outcome, note: String::new() }
}

/// A step whose result carries a note for its row.
fn step_with_note(name: &'static str, f: impl FnOnce() -> Result<String, String>) -> Step {
    let t = Instant::now();
    match f() {
        Ok(note) => Step { name, outcome: Ok(t.elapsed().as_secs_f64()), note },
        Err(e) => Step { name, outcome: Err(e), note: String::new() },
    }
}

/// Criteria no local run can settle, per milestone (report §15).
fn manual_criteria(milestone: &str) -> &'static [&'static str] {
    match milestone {
        "M0" => &[
            "CI green on Windows (WARP) and Linux (lavapipe): needs a pushed branch (see docs/status.md)",
            "Candidate goldens blessed by the owner (/bless)",
            "Performance harness run on Rasierklinge: needs the owner's go (charger, power profile)",
        ],
        "M1" => &[
            "Tile hashes identical on Windows and Linux: needs the CI hash comparison job",
            "Candidate goldens blessed by the owner (/bless)",
        ],
        "M2" => &[
            "Orbit to 1 m altitude without jitter: the f32 shader-path emulation passes (REND-004); a visual flight on the real GPU needs the owner",
            "Terrain at most 8 ms at 1440p on the mid-range tier: needs a performance run on Rasierklinge (the owner confirms the machine first)",
            "No cracks in 10 scripted views: checked on the software adapters and, by the `test gpu --real` step above, on the discrete GPU (LOD-005)",
            "Six terrain goldens blessed by the owner (/bless), and the Linux goldens",
            "GO/NO-GO gate: the owner decides after reading docs/milestones/M2-report.md",
        ],
        "M3" => &[
            "Tile hashes identical on Windows and Linux: needs the CI determinism job (determinism manifest)",
            "Global bake of 16384 cells per face edge in at most 10 min within 8 GB (GEN-014): needs the architect's perf window (`cargo xtask perf global-bake`)",
            "Production-resolution planning (cells 32, viewport / tau 240) of the standard flight with the terrain and the eroded field: `generate --global-dir D --global-res 16384` then `generate --plan-check D --global-res 16384` (about 12 GB of disk; recorded in docs/milestones/M3-report.md)",
            "Terrain GPU budgets and regressions on Rasierklinge (TEST-012): needs the architect's perf window",
            "Relief-view flight review at 1280x720 on the real GPU (TEST-014): the owner",
            "`cache-verify` and a definition edit rebuilding only the affected tiles (STRM-004) pass in tiers A and B above; a run on a real store is the owner's",
            "GO/NO-GO gate: the owner decides after reading docs/milestones/M3-report.md",
        ],
        "M4" => &[
            "Tile hashes identical on Windows and Linux: needs the CI determinism job (determinism manifest, GEN-003)",
            "CI green on Windows and Linux for the merge commit",
            "`cargo xtask test gpu --real` (blank-tile and count cases on the discrete GPU; never during a perf window)",
            "Candidate goldens: `cargo xtask bless` lists any waiting for the owner",
            "The findings slots of the report (cold start, upload cap, RAM split, prefetch look-ahead, ...), the verifier over the milestone and the recommendation for M5: the architect (M4 is not a go/no-go gate)",
        ],
        "M5" => &[
            "The owner's single look review of the contact sheet, the six-frame strips (`target/review/water-inspection/flight_*_strip.png`) and the 1440p flight videos (`cargo xtask capture flights --store <store>`, shore_filter 0 and 1) (ADR 0037, milestone gate)",
            "CI green on Windows (WARP) and Linux (lavapipe): needs a pushed branch",
            "Candidate goldens blessed under policy or by the owner (`golden-review`, ADR 0061)",
            "The viewer's manual checks, repeated at the gate: mouse look locally (SPIKE-11), keyboard feel (VIEW-004)",
            "A discrete-GPU run of the tiers (`cargo xtask test gpu --real`): not run by this command so that acceptance may sit beside the architect's window",
            "The go / no-go for M6 (milestone gate after M5): the owner",
        ],
        "M8" => &[
            "Criterion 1: the owner's look review of `docs/milestones/M8/look-review-2b/` on plausible relief with horizon shadows (river and valley forms are not judged in M8; F8 compares the default and the sample look live) and the default look",
            "Criterion 3: `cargo xtask gate` on the real store with the chunk stage at its default (off) (TEST-061, TEST-062): no frame over 100 ms, 0 held-back tiles without edits, no step between levels, no panic; run it before the window, never inside one (`gate --small` is an automated step above)",
            "Criteria 4 to 7 are read from the perf logs only; criterion 6 needs the horizon table `verdict (10 s, horizon maps)` in `edit-latency.md`, and criterion 7's envelope log must come from a run with the chunk stage off",
            "CI green on Windows (WARP) and Linux (lavapipe) for the merge commit, and the determinism comparison (criterion 2)",
            "The owner's live-test items (docs/milestones/M8/owner-test-2026-10-27/README.md) and the M8 report (docs/milestones/M8-report.md): the planner",
        ],
        _ => &["No automated criteria are defined for this milestone yet"],
    }
}

/// The default directory of the streaming scenes' logs (`cargo xtask perf stream all` writes `<scene>.json` and
/// `<scene>.frames.txt` there).
const M4_LOGS: &str = "perf/logs";

/// The six scenes' log files, read from `dir` (a missing file is `None`; judging is the library's job, TEST-021).
fn read_m4_logs(dir: &Path) -> Vec<SceneFiles> {
    planet_perf::streaming::SCENES
        .iter()
        .map(|scene| SceneFiles {
            scene: (*scene).to_string(),
            json: std::fs::read(dir.join(format!("{scene}.json"))).ok(),
            frames: std::fs::read_to_string(dir.join(format!("{scene}.frames.txt"))).ok(),
            last_valid_present: dir.join(format!("{scene}.last-valid.json")).is_file(),
        })
        .collect()
}

/// The M8 window's files, read from `dir` (a missing file is `None`; judging is the library's job, TEST-060): what the M8 scenes write by default,
/// `coupled-make.json`, `shadow-cost.json`, `envelope.json` and, copied from the `--out` directory of `perf stream edit-latency`, `edit-latency.md`.
fn read_m8_logs(dir: &Path) -> planet_perf::accept_m8::M8Files {
    planet_perf::accept_m8::M8Files {
        coupled: std::fs::read(dir.join("coupled-make.json")).ok(),
        shadow: std::fs::read(dir.join("shadow-cost.json")).ok(),
        edit: std::fs::read_to_string(dir.join("edit-latency.md")).ok(),
        envelope: std::fs::read(dir.join("envelope.json")).ok(),
    }
}

/// The M5 window's files, read from `dir` (a missing file is `None`; judging is the library's job, TEST-026): `lit.json`,
/// `lit-formats.json` (only the logs of windows 1 to 11 have one, report-only), and for each streaming scene `<scene>-lit.json` with `<scene>-lit.frames.txt`.
fn read_m5_logs(dir: &Path) -> planet_perf::accept_m5::WindowFiles {
    planet_perf::accept_m5::WindowFiles {
        lit: std::fs::read(dir.join("lit.json")).ok(),
        formats: std::fs::read(dir.join("lit-formats.json")).ok(),
        streams: planet_perf::streaming::SCENES
            .iter()
            .map(|scene| SceneFiles {
                scene: (*scene).to_string(),
                json: std::fs::read(dir.join(format!("{scene}-lit.json"))).ok(),
                frames: std::fs::read_to_string(dir.join(format!("{scene}-lit.frames.txt"))).ok(),
                last_valid_present: dir.join(format!("{scene}-lit.last-valid.json")).is_file(),
            })
            .collect(),
    }
}

/// Where a generated milestone report goes (`M4` or `M5`): the milestone report if none exists yet, else beside the other review output, so a report the
/// architect has already completed is never overwritten, and a run with a failed automated step never creates the milestone report.
fn m4_report_path(root: &Path, steps_failed: bool) -> PathBuf {
    milestone_report_path(root, "M4", steps_failed)
}

fn milestone_report_path(root: &Path, milestone: &str, steps_failed: bool) -> PathBuf {
    let report = root.join("docs").join("milestones").join(format!("{milestone}-report.md"));
    if report.exists() || steps_failed {
        root.join("target").join("review").join(format!("{milestone}-report.generated.md"))
    } else {
        report
    }
}

/// `YYYY-MM-DD` (UTC) of a unix time in seconds (civil-from-days).
fn utc_date(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let mut it = args.iter();
    let mut milestone: Option<&String> = None;
    let mut logs: Option<PathBuf> = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--logs" => logs = Some(PathBuf::from(it.next().ok_or("--logs needs a directory")?)),
            other if other.starts_with("--") => return Err(format!("unknown option {other} (known: --logs DIR, M4, M5 and M8 only)")),
            _ => milestone = Some(a),
        }
    }
    let Some(milestone) = milestone else {
        return Err("usage: cargo xtask accept <milestone>, e.g. M0 (M4, M5 and M8 also take --logs DIR, default perf/logs)".into());
    };
    let m = milestone.to_uppercase();
    if logs.is_some() && m != "M4" && m != "M5" && m != "M8" {
        return Err("--logs only applies to M4, M5 and M8 (the perf logs of a window)".into());
    }
    let logs_dir = logs.map_or_else(|| root.join(M4_LOGS), |p| if p.is_absolute() { p } else { root.join(p) });
    let logs_shown = logs_dir.strip_prefix(root).unwrap_or(&logs_dir).to_string_lossy().replace('\\', "/");
    // M4's hardware verdicts come from the logs of the architect's perf window, read first: cheap, and a refusal must not wait for
    // the test run. Nothing here measures anything (TEST-021).
    let m4 = (m == "M4").then(|| planet_perf::accept::evaluate(&read_m4_logs(&logs_dir)));
    let m5 = (m == "M5").then(|| planet_perf::accept_m5::evaluate(&read_m5_logs(&logs_dir)));
    let m8 = (m == "M8").then(|| planet_perf::accept_m8::evaluate(&read_m8_logs(&logs_dir)));
    let mut steps = vec![
        step("check (fmt, clippy, layering)", || crate::testcmd::check(root)),
        step("spec-lint", || crate::speclint::run(root)),
        step("test (tiers A+B, software adapter)", || crate::testcmd::test(root, &[])),
        step("test gpu (tier C, software adapter)", || crate::testcmd::test(root, &["gpu".to_string()])),
    ];
    // M4 leaves the discrete GPU out of the automated steps: acceptance may run next to the architect's window and
    // `test gpu --real` never runs during one (CLAUDE.md); it is listed under the manual criteria instead.
    // M8: the no-degradation gate on the generated small world (TEST-063); the real-store gate stays on the manual list.
    if m == "M8" {
        steps.push(step("gate --small (TEST-061 to TEST-063)", || crate::gate::run_gate(root, &["--small".to_string()])));
    }
    if m != "M4" && m != "M5" {
        steps.push(step("test gpu --real (tier C, discrete GPU)", || {
            crate::testcmd::test(root, &["gpu".to_string(), "--real".to_string()])
        }));
    }
    steps.extend([
        step("run mode editor --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "editor", "--smoke"], &[])),
        step("run mode generate --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "generate", "--smoke"], &[])),
        step("run mode test-render --smoke", || exec(root, "cargo", &["run", "-q", "-p", "planet", "--", "test-render", "--smoke"], &[])),
        // BUILD-003: the budget is the test time (not the compile) of a selection of changed crates (owner, 2026-10-16); a run of every
        // crate is reported with its number and not judged.
        step_with_note("test fast (timing)", || {
            crate::testcmd::judge_fast_timing(crate::testcmd::test_timed(root, &["fast".to_string()])?)
        }),
    ]);

    let mut steps_md = String::from("| Step | Result | Seconds |\n|---|---|---|\n");
    let mut failed = false;
    for s in &steps {
        match &s.outcome {
            Ok(secs) => steps_md += &format!("| {} | pass{} | {secs:.1} |\n", s.name, s.note),
            Err(e) => {
                failed = true;
                steps_md += &format!("| {} | FAIL: {} | |\n", s.name, e.lines().next().unwrap_or(""));
            }
        }
    }
    let mut md = format!("# Acceptance run {m}\n\n{steps_md}");
    let mut hardware_error: Option<String> = None;
    match &m4 {
        None => {}
        Some(planet_perf::accept::Evaluation::Refused(reasons)) => {
            let text = planet_perf::accept::refusal_text(reasons, &logs_shown);
            md += &format!("\nHardware criteria (perf logs in {logs_shown}/): REFUSED, no verdict.\n\n{text}");
            hardware_error = Some(format!(
                "acceptance M4 refuses to judge the hardware criteria ({} reason(s)): no valid perf window in {logs_shown}/",
                reasons.len()
            ));
        }
        Some(planet_perf::accept::Evaluation::Judged(j)) => {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let report = planet_perf::accept::render_report(j, &steps_md, &utc_date(secs));
            let path = m4_report_path(root, failed);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, &report).map_err(|e| e.to_string())?;
            md += &format!(
                "\nHardware criteria (perf logs in {logs_shown}/): {}; report written to {} (sections marked TO FILL are the architect's).\n",
                if j.pass { "PASS" } else { "FAIL" },
                path.strip_prefix(root).unwrap_or(&path).display()
            );
            for r in &j.scenes {
                for v in r.verdicts.iter().filter(|v| v.fails()) {
                    md += &format!("  {}\n", v.line(&r.scene));
                }
            }
            if !j.pass {
                hardware_error = Some("acceptance M4: the hardware criteria FAIL (see the report)".into());
            }
        }
    }
    match &m5 {
        None => {}
        Some(planet_perf::accept_m5::Evaluation5::Refused(reasons)) => {
            let text = planet_perf::accept_m5::refusal_text(reasons, &logs_shown);
            md += &format!("\nHardware criteria (perf logs in {logs_shown}/): REFUSED, no verdict.\n\n{text}");
            hardware_error = Some(format!(
                "acceptance M5 refuses to judge the hardware criteria ({} reason(s)): no valid perf window in {logs_shown}/",
                reasons.len()
            ));
        }
        Some(planet_perf::accept_m5::Evaluation5::Judged(j)) => {
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let report = planet_perf::accept_m5::render_report(j, &steps_md, &utc_date(secs));
            let path = milestone_report_path(root, "M5", failed);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, &report).map_err(|e| e.to_string())?;
            md += &format!(
                "\nHardware criteria (perf logs in {logs_shown}/): {}; report written to {} (sections marked TO FILL are the architect's).\n",
                j.outcome.word(),
                path.strip_prefix(root).unwrap_or(&path).display()
            );
            for l in j.failing.iter().chain(j.incomplete.iter()) {
                md += &format!("  {l}\n");
            }
            if j.outcome != planet_perf::accept_m5::Outcome::Pass {
                hardware_error = Some(format!("acceptance M5: the hardware criteria are {} (see the report)", j.outcome.word()));
            }
        }
    }
    if let Some(rows) = &m8 {
        let outcome = planet_perf::accept_m8::outcome(rows);
        md += &format!(
            "\nHardware criteria 4 to 7 (perf logs in {logs_shown}/): {}\n\n{}",
            outcome.word(),
            planet_perf::accept_m8::render(rows)
        );
        if outcome != planet_perf::accept_m8::Outcome::Pass {
            hardware_error = Some(format!("acceptance M8: the hardware criteria are {} (see the table)", outcome.word()));
        }
    }
    md += "\nNeeds CI, the owner or Rasierklinge (not passed by this command):\n\n";
    for c in manual_criteria(&m) {
        md += &format!("- {c}\n");
    }
    let out = root.join("target").join("review").join(format!("accept-{m}.md"));
    std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&out, &md).map_err(|e| e.to_string())?;
    eprintln!("\n{md}\nwrote {}", out.display());
    if let Some(e) = hardware_error {
        return Err(if failed { format!("{e}; and at least one automated step failed (see above)") } else { e });
    }
    if failed {
        Err(format!("acceptance {m}: at least one automated step failed (see above)"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-021
    #[test]
    fn the_log_reader_takes_the_seven_scenes_files_and_notes_a_last_valid_log() {
        let dir = std::env::temp_dir().join(format!("xtask-m4-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dive.json"), b"{}").unwrap();
        std::fs::write(dir.join("dive.frames.txt"), "x").unwrap();
        std::fs::write(dir.join("look-1m.last-valid.json"), b"{}").unwrap();
        let files = read_m4_logs(&dir);
        assert_eq!(files.len(), 7, "the six scenes and dive-cold");
        let dive = &files[0];
        assert_eq!((dive.scene.as_str(), dive.json.is_some(), dive.frames.as_deref()), ("dive", true, Some("x")));
        let look = files.iter().find(|f| f.scene == "look-1m").unwrap();
        assert!(look.json.is_none() && look.last_valid_present);
        // No logs at all is a refusal naming the missing scene, never a verdict.
        let none = read_m4_logs(&dir.join("absent"));
        let r = planet_perf::accept::evaluate(&none);
        assert!(matches!(&r, planet_perf::accept::Evaluation::Refused(why) if why.iter().any(|w| w.contains("dive: no log"))), "{r:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: TEST-021
    #[test]
    fn a_report_already_in_the_repository_is_never_overwritten() {
        let root = std::env::temp_dir().join(format!("xtask-m4-report-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("docs").join("milestones")).unwrap();
        assert!(m4_report_path(&root, false).to_string_lossy().ends_with("M4-report.md"));
        std::fs::write(root.join("docs").join("milestones").join("M4-report.md"), "mine").unwrap();
        assert!(m4_report_path(&root, false).to_string_lossy().contains("M4-report.generated.md"));
        assert!(
            m4_report_path(&root, true).to_string_lossy().contains("M4-report.generated.md"),
            "a failed step never writes the milestone report"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // spec: TEST-060
    #[test]
    fn the_m8_log_reader_takes_the_default_names_of_the_four_scenes_and_no_logs_is_incomplete() {
        use planet_perf::accept_m8::{evaluate, outcome, Outcome, Verdict};
        let dir = std::env::temp_dir().join(format!("xtask-m8-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("coupled-make.json"), br#"{"scene":"coupled-make","owner_override":[],"bake_res":16384,"seconds":19.7}"#)
            .unwrap();
        std::fs::write(dir.join("edit-latency.md"), "| a | verdict |\n|---|---|\n| 1 m | ok |\n").unwrap();
        let w = read_m8_logs(&dir);
        assert!(w.coupled.is_some() && w.edit.is_some() && w.shadow.is_none() && w.envelope.is_none());
        let rows = evaluate(&w);
        assert!(matches!(rows[0].verdict, Verdict::Pass(_)) && matches!(rows[1].verdict, Verdict::Missing(_)));
        assert_eq!(outcome(&rows), Outcome::Incomplete);
        assert_eq!(outcome(&evaluate(&read_m8_logs(&dir.join("absent")))), Outcome::Incomplete);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: TEST-026
    #[test]
    fn the_m5_log_reader_takes_the_lit_logs_by_their_names_and_no_logs_is_a_refusal() {
        let dir = std::env::temp_dir().join(format!("xtask-m5-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lit.json"), b"{}").unwrap();
        std::fs::write(dir.join("dive-lit.json"), b"{}").unwrap();
        std::fs::write(dir.join("dive-lit.frames.txt"), "x").unwrap();
        // The debug path's logs of the same scene are not the lit window's.
        std::fs::write(dir.join("look-1m.json"), b"{}").unwrap();
        let w = read_m5_logs(&dir);
        assert!(w.lit.is_some() && w.formats.is_none());
        assert_eq!(w.streams.len(), 7);
        let dive = w.streams.iter().find(|f| f.scene == "dive").unwrap();
        assert!(dive.json.is_some() && dive.frames.as_deref() == Some("x"));
        assert!(w.streams.iter().find(|f| f.scene == "look-1m").unwrap().json.is_none());
        let none = read_m5_logs(&dir.join("absent"));
        match planet_perf::accept_m5::evaluate(&none) {
            planet_perf::accept_m5::Evaluation5::Refused(why) => {
                assert!(
                    why.iter().any(|w| w.contains("lit: no log `lit.json`"))
                        && why.iter().any(|w| w.contains("dive-lit: no log `dive-lit.json`")),
                    "{why:?}"
                )
            }
            _ => panic!("no logs must be refused"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn utc_date_is_the_civil_date() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(1_791_936_000), "2026-10-14");
        assert_eq!(utc_date(951_782_400), "2000-02-29");
    }
}
