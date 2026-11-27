//! `cargo xtask archive-milestone <M> [--before YYYY-MM-DD] [--dry-run]` (BUILD-072): the housekeeping skill's mechanical archive steps for a
//! finished milestone, in one run.
//! - The fragments of `docs/log.d/` (those dated before `--before`, when given) are merged into `docs/archive/<M>/log.md` in `cargo xtask log`'s
//!   order (an existing archive's entries merged in), then deleted.
//! - The rows of `docs/spikes.md`'s index whose status starts with `closed` move to `docs/archive/<M>/spikes.md` (appended to an existing table).
//! - The pointers in the headers of `docs/log.md` and `docs/spikes.md` are added when missing, so a second run adds none.
//!
//! Every new file text is computed before anything is written; a malformed fragment or a missing header sentence refuses the run. Nothing else is
//! deleted: no change archive, no image, no ADR. `--dry-run` prints the plan and writes nothing. An existing log archive is rewritten as its header
//! (the lines before its first entry) and its entries; files are written with LF line ends (`.gitattributes` keeps the docs LF).

use crate::logcmd::{merge, parse_fragment, parse_log, read_fragments, Entry, FRAGMENT_DIR};
use crate::util::Res;
use std::path::Path;

/// The sentence of `docs/log.md`'s header that lists the log archives.
const LOG_POINTER: &str = "Finished milestones are archived";

/// What a run does: the new file texts and the fragments it deletes.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    /// (path relative to the root, new text), in write order.
    pub writes: Vec<(String, String)>,
    /// Fragment file names under `docs/log.d/` to delete.
    pub deletes: Vec<String>,
    /// The log entries moved and the spike ids moved.
    pub entries: usize,
    pub spikes: Vec<String>,
}

/// A milestone name: letters, digits and `-` only (it becomes a folder name).
fn check_name(m: &str) -> Res {
    if m.is_empty() || !m.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("archive-milestone: `{m}` is not a milestone name (letters, digits and `-`, e.g. P3 or M0-M3)"));
    }
    Ok(())
}

/// The cell `k` (0-based) of a table row `| a | b | ... |`, trimmed.
fn cell(row: &str, k: usize) -> Option<&str> {
    row.trim().strip_prefix('|')?.split('|').nth(k).map(str::trim)
}

/// The number of a `SPIKE-NN` id (the digits after the prefix), for the range in the pointers.
fn spike_number(id: &str) -> Option<u32> {
    id.strip_prefix("SPIKE-")?.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// The plan of `archive-milestone` for milestone `m` on the docs tree under `root`.
pub fn plan(root: &Path, m: &str, before: Option<&str>) -> Res<Plan> {
    check_name(m)?;
    let read = |p: &str| std::fs::read_to_string(root.join(p)).map_err(|e| format!("{p}: {e}"));
    let mut out = Plan::default();

    // ---- the log
    let mut moved: Vec<Entry> = Vec::new();
    for (name, text) in read_fragments(root)? {
        let e = parse_fragment(&name, &text).map_err(|e| format!("{FRAGMENT_DIR}/{name}: {e} (BUILD-010)"))?;
        if before.is_none_or(|b| e.date.as_str() < b) {
            out.deletes.push(name);
            moved.push(e);
        }
    }
    let log_archive = format!("docs/archive/{m}/log.md");
    let mut log_md = read("docs/log.md")?;
    if !moved.is_empty() {
        out.entries = moved.len();
        let (header, old) = match std::fs::read_to_string(root.join(&log_archive)) {
            Ok(t) => {
                // the existing archive's header is every line before its first entry
                let first = t.lines().position(|l| l.starts_with("- ")).unwrap_or(t.lines().count());
                let header: String = t.lines().take(first).map(|l| format!("{l}\n")).collect();
                (header, parse_log(&t))
            }
            Err(_) => {
                let dates: Vec<&str> = moved.iter().map(|e| e.date.as_str()).collect();
                let (first, last) = (dates.iter().min().unwrap(), dates.iter().max().unwrap());
                (format!("# Log, {m} ({first} to {last}; the fragments of docs/log.d/ merged by `cargo xtask archive-milestone`, newest first)\n\n"), Vec::new())
            }
        };
        let body: String = merge(moved, old).iter().map(|e| format!("{}\n", e.text)).collect();
        out.writes.push((log_archive.clone(), header + &body));
        if !log_md.contains(&format!("`{log_archive}`")) {
            let (i, line) = log_md
                .lines()
                .enumerate()
                .find(|(_, l)| l.contains(LOG_POINTER))
                .ok_or(format!("docs/log.md: no header sentence `{LOG_POINTER} ...` to add the pointer to"))?;
            let line = line.to_string();
            let new = match line.strip_suffix('.') {
                Some(head) => format!("{head}, `{log_archive}`."),
                None => format!("{line} `{log_archive}`."),
            };
            log_md = log_md.lines().enumerate().map(|(k, l)| if k == i { format!("{new}\n") } else { format!("{l}\n") }).collect();
            out.writes.push(("docs/log.md".into(), log_md));
        }
    }

    // ---- the spikes
    let spikes_md = read("docs/spikes.md")?;
    let lines: Vec<&str> = spikes_md.lines().collect();
    let closed = |l: &str| l.starts_with("| SPIKE-") && cell(l, 2).is_some_and(|s| s.starts_with("closed"));
    let rows: Vec<&str> = lines.iter().copied().filter(|l| closed(l)).collect();
    if !rows.is_empty() {
        out.spikes = rows.iter().filter_map(|r| cell(r, 0).map(str::to_string)).collect();
        let nums: Vec<u32> = out.spikes.iter().filter_map(|s| spike_number(s)).collect();
        let range = match (nums.iter().min(), nums.iter().max()) {
            (Some(a), Some(b)) if a != b => format!("SPIKE-{a} to {b}"),
            (Some(a), _) => format!("SPIKE-{a}"),
            _ => "the closed spikes".to_string(),
        };
        let spike_archive = format!("docs/archive/{m}/spikes.md");
        let archived = match std::fs::read_to_string(root.join(&spike_archive)) {
            Ok(t) => {
                // the rows go after the archive's last table row (a table header first when it has none); a row it already holds is not added again
                let mut al: Vec<String> = t.lines().map(str::to_string).collect();
                let at = match al.iter().rposition(|l| l.starts_with('|')) {
                    Some(k) => k + 1,
                    None => {
                        al.push(String::new());
                        al.push("| Id | Title | Status | Results |".into());
                        al.push("|---|---|---|---|".into());
                        al.len()
                    }
                };
                let new: Vec<&str> = rows.iter().copied().filter(|r| !al.iter().any(|l| l == r)).collect();
                for (k, r) in new.iter().enumerate() {
                    al.insert(at + k, (*r).to_string());
                }
                al.join("\n") + "\n"
            }
            Err(_) => format!(
                "# Closed spikes of {m} ({range}; index rows moved from docs/spikes.md by `cargo xtask archive-milestone`)\n\nEach spike's report is in its results folder.\n\n| Id | Title | Status | Results |\n|---|---|---|---|\n{}\n",
                rows.join("\n")
            ),
        };
        out.writes.push((spike_archive.clone(), archived));
        let mut kept: Vec<String> = lines.iter().filter(|l| !closed(l)).map(|l| (*l).to_string()).collect();
        if !spikes_md.contains(&format!("`{spike_archive}`")) {
            // the pointer goes on the header's last line that names an archive (above the index)
            let index = kept.iter().position(|l| l.starts_with("## ")).unwrap_or(kept.len());
            let k = kept[..index]
                .iter()
                .rposition(|l| l.contains("`docs/archive/"))
                .ok_or("docs/spikes.md: no header line naming an archive (`docs/archive/...`) to add the pointer to")?;
            kept[k] = format!("{} Closed spikes of {m} ({range}): `{spike_archive}`.", kept[k].trim_end());
        }
        out.writes.push(("docs/spikes.md".into(), kept.join("\n") + if spikes_md.ends_with('\n') { "\n" } else { "" }));
    }
    Ok(out)
}

/// Writes the plan: the files, then the fragment deletions.
pub fn apply(root: &Path, p: &Plan) -> Res {
    for (path, text) in &p.writes {
        let full = root.join(path);
        if let Some(dir) = full.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&full, text).map_err(|e| format!("{path}: {e}"))?;
    }
    for name in &p.deletes {
        let f = root.join(FRAGMENT_DIR).join(name);
        std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display()))?;
    }
    Ok(())
}

/// `cargo xtask archive-milestone <M> [--before YYYY-MM-DD] [--dry-run]`.
pub fn run(root: &Path, args: &[String]) -> Res {
    let usage = "usage: cargo xtask archive-milestone <M> [--before YYYY-MM-DD] [--dry-run]";
    let (mut m, mut before, mut dry) = (None, None, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => dry = true,
            "--before" => {
                let v = it.next().ok_or(format!("--before needs a date ({usage})"))?;
                let ok = v.len() == 10 && v.bytes().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == b'-' } else { c.is_ascii_digit() });
                if !ok {
                    return Err(format!("--before: `{v}` is not YYYY-MM-DD"));
                }
                before = Some(v.clone());
            }
            s if s.starts_with('-') => return Err(format!("archive-milestone: unknown option `{s}` ({usage})")),
            s if m.is_none() => m = Some(s.to_string()),
            s => return Err(format!("archive-milestone: one milestone only, not also `{s}` ({usage})")),
        }
    }
    let m = m.ok_or(usage)?;
    let p = plan(root, &m, before.as_deref())?;
    println!(
        "archive-milestone {m}: {} log entries from {} fragments, {} spike rows ({}){}",
        p.entries,
        p.deletes.len(),
        p.spikes.len(),
        p.spikes.join(", "),
        if dry { "; dry run, nothing written" } else { "" }
    );
    for (path, _) in &p.writes {
        println!("  write  {path}");
    }
    for name in &p.deletes {
        println!("  delete {FRAGMENT_DIR}/{name}");
    }
    if p.writes.is_empty() && p.deletes.is_empty() {
        println!("  nothing to archive");
    }
    if !dry {
        apply(root, &p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A docs tree with a log header, three fragments (two before 2026-02-01), and a spike register with a closed, an open and a planned row.
    fn fixture(dir: &Path) {
        let w = |p: &str, t: &str| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, t).unwrap();
        };
        w("docs/log.md", "# Log\n\nNew entries go to docs/log.d/. Finished milestones are archived by the `housekeeping` skill: `docs/archive/M1/log.md`.\n");
        w("docs/log.d/2026-01-10-a.md", "- 2026-01-10 **a**: first\n");
        w("docs/log.d/2026-01-20-b.md", "- 2026-01-20 **b**: second\n");
        w("docs/log.d/2026-02-05-c.md", "- 2026-02-05 **c**: after the milestone\n");
        w(
            "docs/spikes.md",
            "# Spike register\n\nClosed spikes of M1: `docs/archive/M1/spikes.md`.\n\n## Index of open and planned spikes\n| Id | Title | Status | Results |\n|---|---|---|---|\n| SPIKE-7 | Seven | open | x |\n| SPIKE-8 | Eight | closed 01-15: done | y |\n| SPIKE-9 | Nine | planned only | z |\n| SPIKE-11 | Eleven | closed 01-25: no | w |\n\nEvery exploration gets a `SPIKE-NN` entry.\n",
        );
    }

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-archivems-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    // spec: BUILD-072, TEST-159
    #[test]
    fn a_milestone_is_archived_once_and_nothing_else_is_touched() {
        let d = tmp("run");
        fixture(&d);
        let p = plan(&d, "M2", Some("2026-02-01")).unwrap();
        assert_eq!(p.entries, 2);
        assert_eq!(p.deletes, vec!["2026-01-10-a.md".to_string(), "2026-01-20-b.md".to_string()]);
        assert_eq!(p.spikes, vec!["SPIKE-8".to_string(), "SPIKE-11".to_string()]);
        apply(&d, &p).unwrap();
        let r = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
        // the log archive in `cargo xtask log`'s order, newest first; the later fragment stays
        let log = r("docs/archive/M2/log.md");
        assert!(log.starts_with("# Log, M2 (2026-01-10 to 2026-01-20;"), "{log}");
        let entries: Vec<&str> = log.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries, vec!["- 2026-01-20 **b**: second", "- 2026-01-10 **a**: first"]);
        assert!(!d.join("docs/log.d/2026-01-10-a.md").exists() && d.join("docs/log.d/2026-02-05-c.md").exists());
        assert!(r("docs/log.md").contains("`docs/archive/M1/log.md`, `docs/archive/M2/log.md`."), "{}", r("docs/log.md"));
        // the closed spike rows moved byte for byte, the open and planned ones kept, the pointer added
        let sp = r("docs/archive/M2/spikes.md");
        assert!(
            sp.contains("(SPIKE-8 to 11;")
                && sp.contains("| SPIKE-8 | Eight | closed 01-15: done | y |")
                && sp.contains("| SPIKE-11 | Eleven | closed 01-25: no | w |"),
            "{sp}"
        );
        let reg = r("docs/spikes.md");
        assert!(
            !reg.contains("| SPIKE-8 |") && !reg.contains("| SPIKE-11 |") && reg.contains("| SPIKE-7 |") && reg.contains("| SPIKE-9 |"),
            "{reg}"
        );
        assert!(reg.contains("`docs/archive/M1/spikes.md`. Closed spikes of M2 (SPIKE-8 to 11): `docs/archive/M2/spikes.md`."), "{reg}");
        assert!(reg.ends_with("Every exploration gets a `SPIKE-NN` entry.\n"), "{reg}");
        // a second run moves nothing and adds no pointer
        let again = plan(&d, "M2", Some("2026-02-01")).unwrap();
        assert_eq!(again, Plan::default(), "a second run has nothing to do");
        // later: the remaining fragment and a newly closed spike join the existing archives
        std::fs::write(
            d.join("docs/spikes.md"),
            reg.replace("| SPIKE-9 | Nine | planned only | z |", "| SPIKE-9 | Nine | closed 02-03: yes | z |"),
        )
        .unwrap();
        let more = plan(&d, "M2", None).unwrap();
        apply(&d, &more).unwrap();
        let log = r("docs/archive/M2/log.md");
        let entries: Vec<&str> = log.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), 3, "{log}");
        assert_eq!(entries[0], "- 2026-02-05 **c**: after the milestone");
        assert!(log.starts_with("# Log, M2 (2026-01-10 to 2026-01-20;"), "the existing header is kept: {log}");
        let sp = r("docs/archive/M2/spikes.md");
        assert!(sp.trim_end().ends_with("| SPIKE-9 | Nine | closed 02-03: yes | z |"), "{sp}");
        assert_eq!(r("docs/log.md").matches("docs/archive/M2/log.md").count(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-072, TEST-159
    #[test]
    fn a_dry_run_writes_nothing_and_a_bad_tree_is_refused_before_any_write() {
        let d = tmp("dry");
        fixture(&d);
        let snapshot = |d: &Path| {
            let mut v: Vec<(String, String)> = walk(d);
            v.sort();
            v
        };
        fn walk(d: &Path) -> Vec<(String, String)> {
            let mut out = Vec::new();
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    out.extend(walk(&p));
                } else {
                    out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
                }
            }
            out
        }
        let before = snapshot(&d);
        run(&d, &["M2".into(), "--dry-run".into()]).unwrap();
        assert_eq!(snapshot(&d), before, "a dry run changes no file");
        // a malformed fragment refuses the run, and nothing is written
        std::fs::write(d.join("docs/log.d/2026-01-11-bad.md"), "not an entry\n").unwrap();
        let before = snapshot(&d);
        let e = run(&d, &["M2".into()]).unwrap_err();
        assert!(e.contains("2026-01-11-bad.md"), "{e}");
        assert_eq!(snapshot(&d), before, "a refused run changes no file");
        std::fs::remove_file(d.join("docs/log.d/2026-01-11-bad.md")).unwrap();
        // a log header without the archive sentence refuses the run before any write
        std::fs::write(d.join("docs/log.md"), "# Log\n").unwrap();
        let before = snapshot(&d);
        let e = run(&d, &["M2".into()]).unwrap_err();
        assert!(e.contains("docs/log.md"), "{e}");
        assert_eq!(snapshot(&d), before);
        // a bad milestone name and an unknown option are refused
        assert!(run(&d, &["../x".into()]).is_err() && run(&d, &["M2".into(), "--all".into()]).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-072, TEST-159
    #[test]
    fn the_edges_a_fragment_on_the_date_an_archive_without_a_table_a_row_already_archived_a_register_without_a_pointer_line() {
        let d = tmp("edges");
        fixture(&d);
        // a fragment dated on `--before` stays (before is strict)
        std::fs::write(
            d.join("docs/log.d/2026-02-01-on.md"),
            "- 2026-02-01 **on**: the boundary
",
        )
        .unwrap();
        let p = plan(&d, "M2", Some("2026-02-01")).unwrap();
        assert!(!p.deletes.contains(&"2026-02-01-on.md".to_string()) && p.entries == 2, "{:?}", p.deletes);
        // an existing spike archive without a table gets one, and a row it already holds is not added twice
        std::fs::create_dir_all(d.join("docs/archive/M2")).unwrap();
        std::fs::write(
            d.join("docs/archive/M2/spikes.md"),
            "# Closed spikes of M2

SPIKE-11 was archived by hand: | SPIKE-11 | Eleven | closed 01-25: no | w |
",
        )
        .unwrap();
        apply(&d, &plan(&d, "M2", Some("2026-02-01")).unwrap()).unwrap();
        let sp = std::fs::read_to_string(d.join("docs/archive/M2/spikes.md")).unwrap();
        assert!(
            sp.contains(
                "
| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-8 | Eight | closed 01-15: done | y |"
            ),
            "{sp}"
        );
        assert_eq!(sp.matches("| SPIKE-8 |").count(), 1, "{sp}");
        std::fs::write(
            d.join("docs/archive/M2/spikes.md"),
            "# M2

| Id | Title | Status | Results |
|---|---|---|---|
| SPIKE-11 | Eleven | closed 01-25: no | w |
",
        )
        .unwrap();
        let reg = std::fs::read_to_string(d.join("docs/spikes.md")).unwrap();
        std::fs::write(
            d.join("docs/spikes.md"),
            reg.replace(
                "| SPIKE-7 | Seven | open | x |",
                "| SPIKE-7 | Seven | open | x |
| SPIKE-11 | Eleven | closed 01-25: no | w |",
            ),
        )
        .unwrap();
        apply(&d, &plan(&d, "M2", None).unwrap()).unwrap();
        let sp = std::fs::read_to_string(d.join("docs/archive/M2/spikes.md")).unwrap();
        assert_eq!(sp.matches("| SPIKE-11 |").count(), 1, "a row the archive holds is not added again: {sp}");
        // a register whose header names no archive refuses the run when a closed row moves (the pointer has no line)
        let e = tmp("edges2");
        fixture(&e);
        let reg = std::fs::read_to_string(e.join("docs/spikes.md")).unwrap();
        std::fs::write(e.join("docs/spikes.md"), reg.replace("Closed spikes of M1: `docs/archive/M1/spikes.md`.", "No archive yet."))
            .unwrap();
        let err = plan(&e, "M2", None).unwrap_err();
        assert!(err.contains("docs/spikes.md"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&e);
    }
}
