//! spec-lint's checks of the sprint process's files (BUILD-076, ADR 0130): the byte caps of sprint files, records and reports; closed sprint
//! files outside the archive; the `Check` column of `docs/routing.md`'s rules; the research package header of spikes opened from 2026-11-27;
//! and the exit-cell diff check, which reuses BUILD-040's mechanism (`process.rs`): a changed exit or gate cell of the active sprint needs an
//! added `## Changes` line naming it.

use crate::sprintfile::{day_number, is_sprint_file};
use std::path::Path;

/// The largest sprint file, record and report, bytes (combined plan 3: 4 KB, 4 KB, 6 KB).
pub const SPRINT_MAX_BYTES: usize = 4096;
pub const RECORD_MAX_BYTES: usize = 4096;
pub const REPORT_MAX_BYTES: usize = 6144;
/// Spikes opened on or after this date, or numbered from this spike on, carry the research package header.
pub const RESEARCH_FROM: &str = "2026-11-27";
pub const RESEARCH_FROM_SPIKE: u32 = 145;
/// The research package header's labels (spikes' `/spike` skill, 2026-11-27).
pub const RESEARCH_LABELS: [&str; 10] = [
    "Opened",
    "Exit",
    "Question",
    "Hypothesis",
    "Constraints",
    "Out of scope",
    "Evaluation (quantitative)",
    "Evaluation (visual)",
    "Falsifier",
    "Code location",
];
/// The values a closed research package's `Decision:` line starts with.
pub const DECISIONS: [&str; 4] = ["continue", "revise", "reject", "promote"];

/// The byte caps and the archive rule for the files of `docs/milestones/` as `(relative path, text)`: a sprint file (an `Opened:` line) at
/// most 4 KB and never closed outside the archive; a record `<M>/<E>-record.md` of a sprint at most 4 KB; a sprint's `<M>-report.md` at most 6 KB.
pub fn file_errors(files: &[(String, String)]) -> Vec<String> {
    let sprint = |m: &str| files.iter().any(|(p, t)| p == &format!("{m}.md") && is_sprint_file(t));
    let mut e = Vec::new();
    for (p, t) in files {
        let at = format!("docs/milestones/{p}");
        let n = t.len();
        if !p.contains('/') && p.ends_with(".md") && is_sprint_file(t) {
            if n > SPRINT_MAX_BYTES {
                e.push(format!("{at}: {n} bytes, a sprint file is at most {SPRINT_MAX_BYTES} (BUILD-076)"));
            }
            if crate::plancmd::milestone_status(t).is_some_and(|s| s.to_lowercase().starts_with("closed")) {
                let m = p.trim_end_matches(".md");
                e.push(format!("{at}: a closed sprint file stays in docs/archive/{m}/, not in docs/milestones/ (`cargo xtask archive-milestone {m}`, BUILD-076)"));
            }
        } else if let Some(m) = p.strip_suffix("-report.md").filter(|m| !m.contains('/') && sprint(m)) {
            if n > REPORT_MAX_BYTES {
                e.push(format!("{at}: {n} bytes, a sprint's report is at most {REPORT_MAX_BYTES} (BUILD-076; {m}.md is a sprint file)"));
            }
        } else if let Some((m, name)) = p.split_once('/') {
            if name.ends_with("-record.md") && !name.contains('/') && sprint(m) && n > RECORD_MAX_BYTES {
                e.push(format!("{at}: {n} bytes, a record is at most {RECORD_MAX_BYTES} (BUILD-076)"));
            }
        }
    }
    e
}

/// The table of `docs/routing.md`'s `## Rules` section: every row needs a `Check` cell (a check's name, or `judgement`). The section must be a
/// table with the columns `#` and `Check` (beside the rule's text).
pub fn routing_errors(text: &str) -> Vec<String> {
    let body: Vec<&str> = text.lines().skip_while(|l| !l.starts_with("## Rules")).skip(1).take_while(|l| !l.starts_with("## ")).collect();
    if body.is_empty() {
        return vec!["docs/routing.md: no `## Rules` section (BUILD-076)".into()];
    }
    let rows: Vec<Vec<String>> = body
        .iter()
        .filter(|l| l.trim_start().starts_with('|'))
        .map(|l| l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect())
        .collect();
    let Some(head) = rows.first() else {
        return vec!["docs/routing.md: `## Rules` is not a table with the columns # and Check (BUILD-076)".into()];
    };
    let low: Vec<String> = head.iter().map(|h| h.to_lowercase()).collect();
    let (Some(id), Some(check)) = (low.iter().position(|h| h == "#"), low.iter().position(|h| h == "check")) else {
        return vec!["docs/routing.md: the `## Rules` table needs the columns # and Check (BUILD-076)".into()];
    };
    let mut e = Vec::new();
    for r in rows.iter().skip(1).filter(|r| !r.iter().all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))) {
        if r.get(check).is_none_or(|c| c.is_empty()) {
            e.push(format!(
                "docs/routing.md rule {}: the Check cell is empty: name the check that enforces it, or write `judgement` (ADR 0120 point 10, BUILD-076)",
                r.get(id).map_or("?", String::as_str)
            ));
        }
    }
    // `14. A rule` is a numbered rule line; `10.5 s` is not
    let loose = body
        .iter()
        .filter(|l| {
            let digits = l.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && l[digits..].starts_with(". ")
        })
        .count();
    if loose > 0 {
        e.push(format!("docs/routing.md: {loose} numbered rule lines outside the `## Rules` table: every rule is a table row with a Check cell (BUILD-076)"));
    }
    e
}

/// The value of a `Label: value` line of a research package (the first line starting with `Label:`).
fn label<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines().find_map(|l| l.trim_start().strip_prefix(name).and_then(|r| r.strip_prefix(':')).map(str::trim))
}

/// Whether a results file carries the research package rule: opened on or after [`RESEARCH_FROM`], or a `# SPIKE-<n>` title from [`RESEARCH_FROM_SPIKE`] on.
pub fn needs_research_header(text: &str) -> bool {
    let from = day_number(RESEARCH_FROM).unwrap_or(i64::MAX);
    let opened = label(text, "Opened").and_then(day_number).is_some_and(|d| d >= from);
    let numbered = text
        .lines()
        .find(|l| l.starts_with("# "))
        .and_then(|l| l.split("SPIKE-").nth(1))
        .and_then(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u32>().ok())
        .is_some_and(|n| n >= RESEARCH_FROM_SPIKE);
    opened || numbered
}

/// The research package header's errors of one `spikes/<name>/results/results.md`: every label of [`RESEARCH_LABELS`] with a value; once a
/// `Decision:` line is there (closed), a `Finding:` line and a decision of [`DECISIONS`], and `Research-Review:` for a promotion.
pub fn research_errors(path: &str, text: &str) -> Vec<String> {
    if !needs_research_header(text) {
        return Vec::new();
    }
    let mut e = Vec::new();
    for l in RESEARCH_LABELS {
        if label(text, l).is_none_or(str::is_empty) {
            e.push(format!("{path}: the research package header needs `{l}:` with a value (BUILD-076)"));
        }
    }
    if let Some(d) = label(text, "Decision") {
        let word = d.split(|c: char| c == ':' || c.is_whitespace()).next().unwrap_or("").to_lowercase();
        if !DECISIONS.contains(&word.as_str()) {
            e.push(format!("{path}: `Decision: {d}` is none of continue, revise, reject, promote: <why> (BUILD-076)"));
        }
        if label(text, "Finding").is_none_or(str::is_empty) {
            e.push(format!("{path}: a closed research package needs `Finding:` (BUILD-076)"));
        }
        if word == "promote" && label(text, "Research-Review").is_none_or(|r| !r.to_lowercase().starts_with("ready")) {
            e.push(format!("{path}: a promotion needs `Research-Review: ready (<reviewer>, <date>, <sha>)` (BUILD-076)"));
        }
    }
    e
}

/// The rows of `## Exit items` keyed by exit ID, with the State and Result cells left out (they move every day without a decision).
fn decided_rows(text: &str) -> Vec<String> {
    let rows = crate::process::table_rows(text, "## Exit items");
    let Some(head) = rows.first() else { return Vec::new() };
    let low: Vec<String> = head.trim_matches('|').split('|').map(|c| c.trim().to_lowercase()).collect();
    let drop: Vec<usize> = low.iter().enumerate().filter(|(_, h)| *h == "state" || *h == "result").map(|(i, _)| i).collect();
    rows.iter()
        .skip(2)
        .map(|r| {
            let cells: Vec<&str> = r.trim().trim_matches('|').split('|').map(str::trim).collect();
            let kept: Vec<&str> = cells.iter().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, c)| *c).collect();
            format!("| {} |", kept.join(" | "))
        })
        .collect()
}

/// The exit-cell diff check of an active sprint file before and after a change (BUILD-040's mechanism): an exit row whose Item, Mode, Done
/// when, Gate, Park or Lane changed, or that was added or removed, needs an added `## Changes` line `- YYYY-MM-DD | <cell> | <old -> new> |
/// <who decided>` whose cell names its exit ID. A file not at the base (a newly opened sprint) and a draft are not checked.
pub fn change_errors(name: &str, before: Option<&str>, after: &str) -> Vec<String> {
    let Some(before) = before else { return Vec::new() };
    if !crate::plancmd::is_active(after) || !is_sprint_file(after) {
        return Vec::new();
    }
    let keys = crate::process::changed_keys(&decided_rows(before), &decided_rows(after));
    if keys.is_empty() {
        return Vec::new();
    }
    let section = |t: &str| -> Vec<String> {
        t.lines()
            .skip_while(|l| !l.starts_with("## Changes"))
            .skip(1)
            .take_while(|l| !l.starts_with("## "))
            .filter(|l| l.starts_with("- "))
            .map(|l| l.trim_end().to_string())
            .collect()
    };
    let old: std::collections::BTreeSet<String> = section(before).into_iter().collect();
    let mut e = Vec::new();
    let mut parsed = Vec::new();
    for l in section(after).into_iter().filter(|l| !old.contains(l)) {
        match crate::process::parse_contract_line(&l) {
            Ok(f) => parsed.push(f),
            Err(err) => e.push(format!("docs/milestones/{name}.md, ## Changes: {err}")),
        }
    }
    for k in keys {
        if !parsed.iter().any(|f| crate::plancmd::names(&f[1], &k)) {
            e.push(format!(
                "docs/milestones/{name}.md: exit {k}'s row changed without an added `## Changes` line naming it: `- YYYY-MM-DD | {k} <cell> | <old -> new> | <who decided>` (BUILD-076)"
            ));
        }
    }
    e
}

/// The files of `docs/milestones/` as `(path relative to it, text)`, one level of folders deep.
pub fn milestone_tree(root: &Path) -> Vec<(String, String)> {
    let dir = root.join("docs").join("milestones");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&p) {
                for s in sub.flatten().filter(|s| s.path().extension().is_some_and(|x| x == "md")) {
                    if let Ok(t) = std::fs::read_to_string(s.path()) {
                        out.push((format!("{name}/{}", s.file_name().to_string_lossy()), t));
                    }
                }
            }
        } else if p.extension().is_some_and(|x| x == "md") {
            if let Ok(t) = std::fs::read_to_string(&p) {
                out.push((name, t));
            }
        }
    }
    out.sort();
    out
}

/// Every check of BUILD-076 on the repository; the diff check runs against the merge base with `origin/main` (none without git or a base).
pub fn errors(root: &Path) -> Vec<String> {
    let files = milestone_tree(root);
    let mut e = file_errors(&files);
    match std::fs::read_to_string(root.join("docs").join("routing.md")) {
        Ok(t) => e.extend(routing_errors(&t)),
        Err(err) => e.push(format!("docs/routing.md: {err}")),
    }
    if let Ok(rd) = std::fs::read_dir(root.join("spikes")) {
        let mut dirs: Vec<_> = rd.flatten().map(|d| d.file_name().to_string_lossy().into_owned()).collect();
        dirs.sort();
        for d in dirs {
            let rel = format!("spikes/{d}/results/results.md");
            if let Ok(t) = std::fs::read_to_string(root.join(&rel)) {
                e.extend(research_errors(&rel, &t));
            }
        }
    }
    if let Ok(base) = crate::util::capture(root, "git", &["merge-base", "HEAD", "origin/main"]) {
        for (p, t) in files.iter().filter(|(p, t)| !p.contains('/') && is_sprint_file(t)) {
            let before = crate::util::capture(root, "git", &["show", &format!("{}:docs/milestones/{p}", base.trim())]).ok();
            e.extend(change_errors(p.trim_end_matches(".md"), before.as_deref(), t));
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRINT: &str = include_str!("fixtures/sprint-good.md");
    const RESEARCH: &str = include_str!("fixtures/research-open.md");
    const ROUTING: &str = "# Routing\n\n## Rules\n| # | Rule | Check |\n|---|---|---|\n| 1 | Orders in files. | judgement |\n| 2 | Exit items. | `cargo xtask plan` (BUILD-074) |\n\n## Git\n1. not a rule\n";

    fn f(p: &str, t: &str) -> (String, String) {
        (p.to_string(), t.to_string())
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn sprint_files_records_and_reports_keep_their_byte_caps_and_closed_sprints_leave_for_the_archive() {
        let big = format!("{SPRINT}{}", "x".repeat(4096));
        let files = vec![f("S-Test.md", &big), f("S-Test/E2-record.md", &"r".repeat(4097)), f("S-Test-report.md", &"p".repeat(6145))];
        let e = file_errors(&files);
        assert!(e.iter().any(|m| m.contains("S-Test.md") && m.contains("at most 4096")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("E2-record.md") && m.contains("at most 4096")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("S-Test-report.md") && m.contains("at most 6144")), "{e:?}");
        let ok = vec![f("S-Test.md", SPRINT), f("S-Test/E2-record.md", "r"), f("S-Test-report.md", &"p".repeat(6144))];
        assert!(file_errors(&ok).is_empty(), "{:?}", file_errors(&ok));
        let old = vec![f("P3.md", &"o".repeat(20000)), f("P3-report.md", &"o".repeat(20000)), f("P3/E4-record.md", &"o".repeat(9000))];
        assert!(file_errors(&old).is_empty(), "the old milestone form has no caps");
        let draft = SPRINT.replace("Status: active (owner, 2026-12-01)", "Status: draft (lead, 2026-11-30)");
        assert!(
            file_errors(&[f("S-Next.md", &format!("{draft}{}", "x".repeat(4096)))]).iter().any(|m| m.contains("at most 4096")),
            "a draft is capped too"
        );
        let closed = SPRINT.replace("Status: active (owner, 2026-12-01)", "Status: closed (owner, 2026-12-05)");
        assert!(file_errors(&[f("S-Test.md", &closed)]).iter().any(|m| m.contains("docs/archive/S-Test/")));
        assert!(file_errors(&[f("P1.md", "# P1\nStatus: closed (owner)\n")]).is_empty(), "an old closed milestone is not a sprint file");
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn every_routing_rule_is_a_table_row_with_a_check_cell() {
        assert!(routing_errors(ROUTING).is_empty(), "{:?}", routing_errors(ROUTING));
        let empty = ROUTING.replace("| judgement |", "|  |");
        assert!(routing_errors(&empty).iter().any(|m| m.contains("rule 1") && m.contains("Check cell is empty")));
        let list = "## Rules\n1. Orders in files.\n2. Exit items.\n";
        assert!(routing_errors(list).iter().any(|m| m.contains("not a table")));
        assert!(
            routing_errors(&ROUTING.replace("\n\n## Git", "\n10.5 s is not a rule\n\n## Git")).is_empty(),
            "a decimal is not a numbered rule"
        );
        let mixed = ROUTING.replace("\n\n## Git", "\n14. A loose rule.\n\n## Git");
        assert!(routing_errors(&mixed).iter().any(|m| m.contains("1 numbered rule lines outside")), "{:?}", routing_errors(&mixed));
        assert!(routing_errors(&ROUTING.replace("| Check |", "| Note |")).iter().any(|m| m.contains("columns # and Check")));
        assert!(routing_errors("# R\n").iter().any(|m| m.contains("no `## Rules`")));
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn a_spike_opened_from_today_carries_the_research_package_header() {
        let p = "spikes/x/results/results.md";
        assert!(research_errors(p, RESEARCH).is_empty(), "{:?}", research_errors(p, RESEARCH));
        let missing = RESEARCH.replace("Falsifier: ", "Notes: ");
        assert!(research_errors(p, &missing).iter().any(|m| m.contains("`Falsifier:`")));
        let empty = RESEARCH.replace("Hypothesis: the C2 read removes the facets", "Hypothesis:");
        assert!(research_errors(p, &empty).iter().any(|m| m.contains("`Hypothesis:`")));
        let old = RESEARCH
            .replace("Opened: 2026-11-28", "Opened: 2026-11-20")
            .replace("# SPIKE-146", "# SPIKE-120")
            .replace("Falsifier: ", "Notes: ");
        assert!(research_errors(p, &old).is_empty(), "older spikes are exempt");
        assert!(needs_research_header(&RESEARCH.replace("Opened: 2026-11-28", "Opened: 2026-11-20")), "SPIKE-146 is numbered from 145 on");
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn a_closed_research_package_has_a_finding_a_decision_and_a_review_for_a_promotion() {
        let p = "spikes/x/results/results.md";
        let closed = format!("{RESEARCH}Finding: facets gone on R1h\nDecision: reject: the C2 read costs 2 ms\n");
        assert!(research_errors(p, &closed).is_empty(), "{:?}", research_errors(p, &closed));
        let bad = closed.replace("Decision: reject", "Decision: maybe");
        assert!(research_errors(p, &bad).iter().any(|m| m.contains("none of continue")));
        let no_finding = closed.replace("Finding: facets gone on R1h\n", "");
        assert!(research_errors(p, &no_finding).iter().any(|m| m.contains("needs `Finding:`")));
        let promote = closed.replace("Decision: reject: the C2 read costs 2 ms", "Decision: promote: passes every criterion");
        assert!(research_errors(p, &promote).iter().any(|m| m.contains("Research-Review: ready")));
        let reviewed = format!("{promote}Research-Review: ready (companion, 2026-11-29, abc1234)\n");
        assert!(research_errors(p, &reviewed).is_empty(), "{:?}", research_errors(p, &reviewed));
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn a_changed_exit_or_gate_cell_of_the_active_sprint_needs_a_changes_line() {
        let gate = SPRINT.replace("fun: BUILD-074 (2026-12-01)", "fun: BUILD-075 (2026-12-02)");
        let e = change_errors("S-Test", Some(SPRINT), &gate);
        assert!(e.iter().any(|m| m.contains("exit E1") && m.contains("without an added")), "{e:?}");
        let logged = gate.replace(
            "- 2026-12-01 | E1 Gate | - -> fun: BUILD-074 | owner\n",
            "- 2026-12-01 | E1 Gate | - -> fun: BUILD-074 | owner\n- 2026-12-02 | E1 Gate | BUILD-074 -> BUILD-075 | owner\n",
        );
        assert!(change_errors("S-Test", Some(SPRINT), &logged).is_empty(), "{:?}", change_errors("S-Test", Some(SPRINT), &logged));
        let state = SPRINT.replace("| explorer | running | - |", "| explorer | done 2026-12-03 | bakes |");
        assert!(change_errors("S-Test", Some(SPRINT), &state).is_empty(), "State and Result move without a line");
        let removed: String = SPRINT.lines().filter(|l| !l.starts_with("| E3 |")).map(|l| format!("{l}\n")).collect();
        assert!(change_errors("S-Test", Some(SPRINT), &removed).iter().any(|m| m.contains("exit E3")), "a removed row needs a line too");
        let added = SPRINT.replacen("| E4 |", "| E5 | New | X | x | fun: BUILD-001 (2026-12-02) | - | lead | open | - |\n| E4 |", 1);
        assert!(change_errors("S-Test", Some(SPRINT), &added).iter().any(|m| m.contains("exit E5")));
        let malformed = gate.replace(
            "- 2026-12-01 | E1 Gate | - -> fun: BUILD-074 | owner\n",
            "- 2026-12-01 | E1 Gate | - -> fun: BUILD-074 | owner\n- E1 gate moved\n",
        );
        assert!(change_errors("S-Test", Some(SPRINT), &malformed).iter().any(|m| m.contains("is not `- YYYY-MM-DD")));
        assert!(change_errors("S-Test", None, &gate).is_empty(), "a newly opened sprint is not checked");
        let draft = |t: &str| t.replace("Status: active (owner, 2026-12-01)", "Status: draft (lead, 2026-11-30)");
        assert!(change_errors("S-Test", Some(&draft(SPRINT)), &draft(&gate)).is_empty(), "a draft changes freely before the open");
    }

    // spec: BUILD-076, TEST-163
    #[test]
    fn the_repository_files_are_read_and_its_routing_rules_pass() {
        let root = std::env::temp_dir().join(format!("sprintlint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("docs").join("milestones").join("S-Test")).unwrap();
        std::fs::create_dir_all(root.join("spikes").join("x").join("results")).unwrap();
        std::fs::write(root.join("docs").join("milestones").join("S-Test.md"), SPRINT).unwrap();
        std::fs::write(root.join("docs").join("milestones").join("S-Test").join("E2-record.md"), "r".repeat(5000)).unwrap();
        std::fs::write(root.join("docs").join("routing.md"), ROUTING).unwrap();
        std::fs::write(root.join("spikes").join("x").join("results").join("results.md"), RESEARCH.replace("Falsifier: ", "Notes: "))
            .unwrap();
        let e = errors(&root);
        assert!(e.iter().any(|m| m.contains("E2-record.md")) && e.iter().any(|m| m.contains("`Falsifier:`")) && e.len() == 2, "{e:?}");
        std::fs::remove_dir_all(&root).unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let real = std::fs::read_to_string(repo.join("docs").join("routing.md")).unwrap();
        assert!(routing_errors(&real).is_empty(), "{:?}", routing_errors(&real));
    }
}
