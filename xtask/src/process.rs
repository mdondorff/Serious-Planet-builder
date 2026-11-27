//! Management rules as checks (BUILD-040): spec-lint reads the diff of `HEAD` against the merge base with `origin/main` and fails a change that
//! moves a contract without leaving the record the owner asked for.
//!
//! BUILD-040: a row added, removed or changed in the gate table of `docs/design/terrain-target.md` (section 12.1) needs a line added to the same
//! document's "Contract changes" list (section 14): `- YYYY-MM-DD | gate or section | what changed | who decided`.

use crate::util::capture;
use std::collections::BTreeSet;
use std::path::Path;

pub const TARGET_DOC: &str = "docs/design/terrain-target.md";
const GATE_HEADING: &str = "### 12.1";
const CONTRACT_HEADING: &str = "## 14. Contract changes";

/// The table rows (lines starting with `|`) of the section whose heading starts with `heading`, up to the next heading.
pub fn table_rows(text: &str, heading: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with('#') {
            inside = line.starts_with(heading);
            continue;
        }
        if inside && line.trim_start().starts_with('|') {
            rows.push(line.trim().to_string());
        }
    }
    rows
}

/// The key of a table row: its first cell, without markup and without a trailing parenthesis (`G-SURF-I (diagnostic)` is `G-SURF-I`).
pub fn row_key(row: &str) -> String {
    let cell = row.trim().trim_start_matches('|').split('|').next().unwrap_or("").trim().replace(['*', '`'], "");
    match cell.find(" (") {
        Some(i) => cell[..i].trim().to_string(),
        None => cell,
    }
}

/// The keys of the rows that differ between `before` and `after`: added, removed or changed.
pub fn changed_keys(before: &[String], after: &[String]) -> BTreeSet<String> {
    let b: BTreeSet<&String> = before.iter().collect();
    let a: BTreeSet<&String> = after.iter().collect();
    b.symmetric_difference(&a).map(|r| row_key(r)).filter(|k| !k.is_empty()).collect()
}

/// The list lines (`- ...`) of the "Contract changes" section.
pub fn contract_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with('#') {
            inside = line.starts_with(CONTRACT_HEADING);
            continue;
        }
        if inside && line.starts_with("- ") {
            out.push(line.trim_end().to_string());
        }
    }
    out
}

/// True for `YYYY-MM-DD` with a month 1-12 and a day 1-31.
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    matches!((num(0..4), num(5..7), num(8..10)), (Some(_), Some(1..=12), Some(1..=31)))
}

/// The four fields of a contract-change line, or why it is malformed.
pub fn parse_contract_line(line: &str) -> Result<[String; 4], String> {
    let body = line.strip_prefix("- ").unwrap_or(line);
    let f: Vec<&str> = body.split('|').map(str::trim).collect();
    if f.len() != 4 || f.iter().any(|x| x.is_empty()) || !is_date(f[0]) {
        return Err(format!("`{line}` is not `- YYYY-MM-DD | gate or section | what changed | who decided`"));
    }
    Ok([f[0].to_string(), f[1].to_string(), f[2].to_string(), f[3].to_string()])
}

/// BUILD-040: the errors for the target document before and after a change. `None` is a file that does not exist.
pub fn contract_errors(target: (Option<&str>, Option<&str>)) -> Vec<String> {
    let rows = |t: Option<&str>| t.map(|x| table_rows(x, GATE_HEADING)).unwrap_or_default();
    let keys: BTreeSet<String> = changed_keys(&rows(target.0), &rows(target.1));
    if keys.is_empty() {
        return Vec::new();
    }
    let shown: Vec<&str> = keys.iter().map(String::as_str).collect();
    let Some(after) = target.1 else {
        return vec![format!("{TARGET_DOC} was removed with its gate table ({}): restore it or amend BUILD-040", shown.join(", "))];
    };
    let before_lines: BTreeSet<String> = contract_lines(target.0.unwrap_or("")).into_iter().collect();
    let added: Vec<String> = contract_lines(after).into_iter().filter(|l| !before_lines.contains(l)).collect();
    let mut errors = Vec::new();
    let mut parsed = Vec::new();
    for l in &added {
        match parse_contract_line(l) {
            Ok(f) => parsed.push(f),
            Err(e) => errors.push(format!("{TARGET_DOC}, {CONTRACT_HEADING}: {e}")),
        }
    }
    for k in &keys {
        if !parsed.iter().any(|f| f[1].contains(k.as_str())) {
            errors.push(format!(
                "the row `{k}` of the gate table changed without a line in {TARGET_DOC}, \"{}\" naming it: `- YYYY-MM-DD | {k} | what changed | who decided` (BUILD-040)",
                CONTRACT_HEADING.trim_start_matches("## ")
            ));
        }
    }
    errors
}

/// The text of `path` at `rev` (`HEAD` or a commit), `None` when the file does not exist there.
fn show(root: &Path, rev: &str, path: &str) -> Option<String> {
    capture(root, "git", &["show", &format!("{rev}:{path}")]).ok()
}

/// BUILD-040 against the merge base with `origin/main`; empty without git or a base (a fresh clone).
pub fn contract_errors_against_base(root: &Path) -> Vec<String> {
    let Ok(base) = capture(root, "git", &["merge-base", "HEAD", "origin/main"]) else { return Vec::new() };
    let base = base.trim();
    let (t0, t1) = (show(root, base, TARGET_DOC), show(root, "HEAD", TARGET_DOC));
    contract_errors((t0.as_deref(), t1.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(rows: &[&str], contract: &[&str]) -> String {
        format!(
            "## 12. Validation\n\n### 12.1 Gates\n\n| Gate | Measure | Pass |\n|---|---|---|\n{}\n\n### 12.2 Fixtures\n| not | a | gate |\n\n## 13. ADRs\n- x\n\n## 14. Contract changes\n{}\n",
            rows.join("\n"),
            contract.join("\n")
        )
    }

    const G1: &str = "| G-SURF | at the centres | 0 |";
    const G1_CHANGED: &str = "| G-SURF | at the centres and the vertices | 0.25 m |";
    const G2: &str = "| G-SURF-I (diagnostic) | the vertices | reported |";

    // spec: BUILD-040
    #[test]
    fn rows_and_keys_are_read_from_their_own_section_only() {
        let t = target(&[G1, G2], &[]);
        let rows = table_rows(&t, GATE_HEADING);
        assert_eq!(rows.len(), 4, "the header, the separator and two gates; not the table of 12.2");
        assert_eq!(row_key(G2), "G-SURF-I");
        assert_eq!(row_key("| **G-CON** | a | b |"), "G-CON");
    }

    // spec: BUILD-040
    #[test]
    fn an_unchanged_gate_table_needs_no_line() {
        let t = target(&[G1, G2], &[]);
        assert!(contract_errors((Some(&t), Some(&t))).is_empty());
    }

    // spec: BUILD-040
    #[test]
    fn a_changed_gate_row_without_a_contract_line_is_named() {
        let (t0, t1) = (target(&[G1, G2], &[]), target(&[G1_CHANGED, G2], &[]));
        let e = contract_errors((Some(&t0), Some(&t1)));
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("`G-SURF`") && e[0].contains("Contract changes"), "{e:?}");
    }

    // spec: BUILD-040
    #[test]
    fn a_line_added_by_the_same_change_that_names_the_gate_passes_and_an_old_line_does_not() {
        let line = "- 2026-11-09 | G-SURF | gate at the centres, vertices reported | owner";
        let (t0, t1) = (target(&[G1, G2], &[]), target(&[G1_CHANGED, G2], &[line]));
        assert!(contract_errors((Some(&t0), Some(&t1))).is_empty());
        // The line was already there before this change: it does not cover a new change of the same gate.
        let (t0, t1) = (target(&[G1, G2], &[line]), target(&[G1_CHANGED, G2], &[line]));
        let e = contract_errors((Some(&t0), Some(&t1)));
        assert_eq!(e.len(), 1, "{e:?}");
        // A line that names another gate does not cover this one.
        let other = "- 2026-11-09 | G-CON | tolerance | owner";
        let (t0, t1) = (target(&[G1, G2], &[]), target(&[G1_CHANGED, G2], &[other]));
        assert_eq!(contract_errors((Some(&t0), Some(&t1))).len(), 1);
    }

    // spec: BUILD-040
    #[test]
    fn an_added_or_removed_gate_counts_as_a_change() {
        let (t0, t1) = (target(&[G1], &[]), target(&[G1, G2], &[]));
        assert_eq!(contract_errors((Some(&t0), Some(&t1))).len(), 1);
        assert_eq!(contract_errors((Some(&t1), Some(&t0))).len(), 1);
    }

    // spec: BUILD-040
    #[test]
    fn a_malformed_line_and_a_removed_target_are_refused() {
        let bad = "- yesterday | G-SURF | changed";
        let (t0, t1) = (target(&[G1], &[]), target(&[G1_CHANGED], &[bad]));
        let e = contract_errors((Some(&t0), Some(&t1)));
        assert!(e.iter().any(|x| x.contains("is not `- YYYY-MM-DD")), "{e:?}");
        assert!(e.iter().any(|x| x.contains("`G-SURF`")), "a malformed line covers nothing: {e:?}");
        let e = contract_errors((Some(&t0), None));
        assert!(e.len() == 1 && e[0].contains("was removed"), "{e:?}");
        assert!(is_date("2026-11-09") && !is_date("2026-13-09") && !is_date("26-11-09"));
    }
}
