//! The gate ledger (BUILD-041): `docs/gates/ledger.md` holds one row per measurement of a gate of `docs/design/terrain-target.md` section 12.1.
//! `cargo xtask gates` prints it per gate; spec-lint rejects a gate with no row, a row of a gate that does not exist, and a measured row whose
//! commit does not resolve. A gate that was not measured has a `not run` row with `-` as commit.

use crate::process::{row_key, table_rows, TARGET_DOC};
use crate::util::{capture, Res};
use std::collections::BTreeSet;
use std::path::Path;

pub const LEDGER: &str = "docs/gates/ledger.md";
pub const VERDICTS: [&str; 5] = ["pass", "fail", "partial", "reported", "not run"];

/// One ledger row: `gate | seed | region | resolution | commit | value | verdict | lane`.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub gate: String,
    pub seed: String,
    pub region: String,
    pub resolution: String,
    pub commit: String,
    pub value: String,
    pub verdict: String,
    pub lane: String,
}

/// The gate keys of the table under `### 12.1` (`G-SURF-I (diagnostic)` is `G-SURF-I`).
pub fn gate_keys(target: &str) -> Vec<String> {
    table_rows(target, "### 12.1").iter().map(|r| row_key(r)).filter(|k| k.starts_with("G-")).collect()
}

/// The rows of the ledger table, in file order, and the lines that are not rows of eight cells.
pub fn parse_ledger(text: &str) -> (Vec<Row>, Vec<String>) {
    let mut rows = Vec::new();
    let mut bad = Vec::new();
    for line in text.lines().filter(|l| l.trim_start().starts_with('|')) {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
        if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) || cells.first() == Some(&"gate") {
            continue;
        }
        if cells.len() != 8 {
            bad.push(format!("`{}` has {} cells, not 8", line.trim().chars().take(60).collect::<String>(), cells.len()));
            continue;
        }
        rows.push(Row {
            gate: cells[0].into(),
            seed: cells[1].into(),
            region: cells[2].into(),
            resolution: cells[3].into(),
            commit: cells[4].into(),
            value: cells[5].into(),
            verdict: cells[6].into(),
            lane: cells[7].into(),
        });
    }
    (rows, bad)
}

/// BUILD-041: the errors of a ledger against the gate keys; `commit_ok` says whether a commit resolves (always true when git is not there).
pub fn ledger_errors(keys: &[String], rows: &[Row], bad: &[String], commit_ok: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut e: Vec<String> = bad.iter().map(|b| format!("{LEDGER}: {b}")).collect();
    let known: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    for r in rows {
        let at = format!("{LEDGER}, {} ({})", r.gate, r.lane);
        if !known.contains(r.gate.as_str()) {
            e.push(format!("{at}: `{}` is not a gate of {TARGET_DOC} 12.1", r.gate));
        }
        if !VERDICTS.contains(&r.verdict.as_str()) {
            e.push(format!("{at}: verdict `{}` is none of {}", r.verdict, VERDICTS.join(", ")));
        }
        let cells = [("seed", &r.seed), ("region", &r.region), ("resolution", &r.resolution), ("value", &r.value), ("lane", &r.lane)];
        for (n, v) in cells {
            if v.is_empty() {
                e.push(format!("{at}: the {n} cell is empty (write `-` where there is none)"));
            }
        }
        if r.verdict == "not run" {
            if r.commit != "-" {
                e.push(format!("{at}: a `not run` row has `-` as commit, not `{}`", r.commit));
            }
        } else if r.commit == "-" || !commit_ok(&r.commit) {
            e.push(format!("{at}: the commit `{}` does not resolve; a value without a commit is `not run` (never estimated)", r.commit));
        } else if r.lane == "-" {
            e.push(format!("{at}: a measured row names the lane that measured it"));
        }
    }
    for k in keys {
        if !rows.iter().any(|r| &r.gate == k) {
            e.push(format!("{LEDGER}: the gate `{k}` of {TARGET_DOC} 12.1 has no row (`not run` is a valid verdict)"));
        }
    }
    e
}

/// The text `cargo xtask gates` prints: per gate in the order of 12.1, the verdict of its newest row, the rows and the lanes that measured it.
pub fn summary(keys: &[String], rows: &[Row]) -> String {
    let mut s = String::new();
    let mut one_lane = Vec::new();
    for k in keys {
        let mine: Vec<&Row> = rows.iter().filter(|r| &r.gate == k).collect();
        let Some(last) = mine.last() else {
            s.push_str(&format!("{k:<10} NO ROW\n"));
            continue;
        };
        let lanes: BTreeSet<&str> = mine.iter().filter(|r| r.verdict != "not run").map(|r| r.lane.as_str()).collect();
        if !lanes.is_empty() && lanes.len() < 2 {
            one_lane.push(k.as_str());
        }
        let lane_text = if lanes.is_empty() { "-".to_string() } else { lanes.iter().copied().collect::<Vec<_>>().join(", ") };
        s.push_str(&format!("{k:<10} {:<9} {} row(s); lanes: {lane_text}; newest {}\n", last.verdict, mine.len(), last.commit));
    }
    let count = |v: &str| rows.iter().filter(|r| r.verdict == v).count();
    s.push_str(&format!(
        "{} gates; {} rows: {} pass, {} partial, {} reported, {} fail, {} not run\n",
        keys.len(),
        rows.len(),
        count("pass"),
        count("partial"),
        count("reported"),
        count("fail"),
        count("not run")
    ));
    if !one_lane.is_empty() {
        s.push_str(&format!("measured by one lane only (exit list: another lane): {}\n", one_lane.join(", ")));
    }
    s
}

fn read(root: &Path) -> Res<(Vec<String>, Vec<Row>, Vec<String>)> {
    let target = std::fs::read_to_string(root.join(TARGET_DOC)).map_err(|e| format!("cannot read {TARGET_DOC}: {e}"))?;
    let ledger = std::fs::read_to_string(root.join(LEDGER)).map_err(|e| format!("cannot read {LEDGER}: {e}"))?;
    let (rows, bad) = parse_ledger(&ledger);
    Ok((gate_keys(&target), rows, bad))
}

/// BUILD-041 for this repository; a repository without the ledger or the target document has nothing to check.
pub fn ledger_errors_in(root: &Path) -> Vec<String> {
    if !root.join(TARGET_DOC).is_file() {
        return Vec::new();
    }
    let (keys, rows, bad) = match read(root) {
        Ok(x) => x,
        Err(e) => return vec![e],
    };
    let git = capture(root, "git", &["rev-parse", "--git-dir"]).is_ok();
    let resolves = |c: &str| !git || (c.len() >= 7 && capture(root, "git", &["cat-file", "-e", &format!("{c}^{{commit}}")]).is_ok());
    ledger_errors(&keys, &rows, &bad, &resolves)
}

/// `cargo xtask gates`.
pub fn run(root: &Path) -> Res {
    let (keys, rows, bad) = read(root)?;
    print!("{}", summary(&keys, &rows));
    for b in &bad {
        eprintln!("{LEDGER}: {b}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = "### 12.1 Gates\n\n| Gate | Measure | Pass |\n|---|---|---|\n| G-SURF | a | b |\n| G-SURF-I (diagnostic) | a | b |\n| G-CON | a | b |\n\n### 12.2 Fixtures\n| G-X | no | no |\n";

    fn ledger(rows: &[&str]) -> String {
        format!(
            "# Ledger\n\n| gate | seed | region | resolution | commit | value | verdict | lane |\n|---|---|---|---|---|---|---|---|\n{}\n",
            rows.join("\n")
        )
    }

    const A: &str = "| G-SURF | 14 | r1 | 102 m | a561a419 | 5.7e-14 m | pass | explorer |";
    const B: &str = "| G-SURF | 14 | r1 | 102 m | 8b57df69 | agrees | pass | spikes |";
    const C: &str = "| G-SURF-I | 14 | r1 | 102 m | a561a419 | 0.66 % | reported | explorer |";
    const D: &str = "| G-CON | - | - | - | - | no measurement | not run | - |";

    fn errors(rows: &[&str], ok: bool) -> Vec<String> {
        let (r, bad) = parse_ledger(&ledger(rows));
        ledger_errors(&gate_keys(TARGET), &r, &bad, &|_| ok)
    }

    // spec: BUILD-041
    #[test]
    fn the_gate_keys_are_those_of_12_1_only() {
        assert_eq!(gate_keys(TARGET), ["G-SURF", "G-SURF-I", "G-CON"]);
    }

    // spec: BUILD-041
    #[test]
    fn a_complete_ledger_with_a_not_run_row_has_no_error() {
        assert!(errors(&[A, B, C, D], true).is_empty(), "{:?}", errors(&[A, B, C, D], true));
    }

    // spec: BUILD-041
    #[test]
    fn a_gate_without_a_row_and_a_row_of_an_unknown_gate_are_named() {
        let e = errors(&[A, C], true);
        assert!(e.len() == 1 && e[0].contains("`G-CON`") && e[0].contains("no row"), "{e:?}");
        let e = errors(&[A, C, D, "| G-NEW | 1 | r | 1 | a561a419 | v | pass | explorer |"], true);
        assert!(e.len() == 1 && e[0].contains("`G-NEW` is not a gate"), "{e:?}");
    }

    // spec: BUILD-041
    #[test]
    fn a_measured_row_needs_a_commit_that_resolves_and_a_not_run_row_has_none() {
        let e = errors(&[A, C, D], false);
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e.iter().all(|x| x.contains("does not resolve")), "{e:?}");
        let e = errors(&[A, C, "| G-CON | - | - | - | a561a419 | x | not run | - |"], true);
        assert!(e.len() == 1 && e[0].contains("`not run` row has `-` as commit"), "{e:?}");
        let e = errors(&[A, C, "| G-CON | 14 | r | 1 | - | 1e-9 | pass | explorer |"], true);
        assert!(e.len() == 1 && e[0].contains("does not resolve"), "{e:?}");
    }

    // spec: BUILD-041
    #[test]
    fn a_bad_verdict_an_empty_cell_and_a_short_row_are_refused() {
        let e = errors(&[A, C, D, "| G-CON | 14 | r | 1 | a561a419 | v | great | explorer |"], true);
        assert!(e.len() == 1 && e[0].contains("verdict `great`"), "{e:?}");
        let e = errors(&[A, C, D, "| G-CON | 14 | r | 1 | a561a419 |  | pass | explorer |"], true);
        assert!(e.len() == 1 && e[0].contains("value cell is empty"), "{e:?}");
        let e = errors(&[A, C, D, "| G-CON | 14 | r |"], true);
        assert!(e.len() == 1 && e[0].contains("has 3 cells"), "{e:?}");
    }

    // spec: BUILD-041
    #[test]
    fn the_summary_gives_the_newest_verdict_the_lanes_and_the_gates_measured_by_one_lane() {
        let (r, _) = parse_ledger(&ledger(&[A, B, C, D]));
        let s = summary(&gate_keys(TARGET), &r);
        assert!(s.contains("G-SURF") && s.contains("2 row(s); lanes: explorer, spikes; newest 8b57df69"), "{s}");
        assert!(s.contains("3 gates; 4 rows: 2 pass, 0 partial, 1 reported, 0 fail, 1 not run"), "{s}");
        assert!(s.contains("measured by one lane only") && s.contains("G-SURF-I") && !s.contains("G-SURF, "), "{s}");
        let (r, _) = parse_ledger(&ledger(&[A]));
        assert!(summary(&gate_keys(TARGET), &r).contains("G-CON      NO ROW"));
    }

    // spec: BUILD-041
    #[test]
    fn the_ledger_of_this_repository_covers_every_gate_and_its_commits_resolve() {
        let e = ledger_errors_in(&crate::util::repo_root());
        assert!(e.is_empty(), "{e:?}");
    }
}
