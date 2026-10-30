//! BUILD-028: `spec-lint` refuses a retired term in the current specs and working docs. The terms are read from the table of section 8 of
//! `docs/domain.md` ("Former terms"), first column; nothing is hard-coded here.
//!
//! Matching: case-insensitive, whole words or phrases, with `-` and `_` read as a space (`chunk-stage` and `chunk_stage` are "chunk stage").
//! A first cell lists terms separated by commas; text in parentheses is a remark and is dropped; a single word is too common to check, so it is
//! skipped unless the row says `Lint: all`. A cell of the row may add spellings with `Lint: a; b`, or switch the row off with `Lint: none`.
//! A row that carries the marker `*code removal pending*` is reported as a warning count, not a failure, until the marker is gone.

use std::collections::BTreeMap;

/// One retired term: how it is matched, what replaces it, and whether its code is still to be removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetiredTerm {
    /// The term as written in the table.
    pub term: String,
    /// The normalised phrase that is matched (lower case, `-` and `_` as a space).
    pub phrase: String,
    /// The row's "Now" column: what to use instead.
    pub now: String,
    pub pending: bool,
}

/// What the lint found in the text it was given.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Findings {
    /// `file:line: retired term 'X' (use: ...)`, one per mention of a term whose code is gone.
    pub errors: Vec<String>,
    /// Mentions of a term with code removal pending, counted per term.
    pub pending: BTreeMap<String, usize>,
    /// Files that mention a pending term, counted.
    pub pending_files: usize,
}

fn clean(cell: &str) -> String {
    cell.trim().replace(['*', '`'], "").trim().to_string()
}

/// The text of `cell` with every `( ... )` removed.
fn without_parentheses(cell: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in cell.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The phrase a term is matched as: lower case, `-` and `_` as a space, runs of white space as one space.
pub fn normalise(s: &str) -> String {
    let mapped: String = s.chars().flat_map(char::to_lowercase).map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The retired terms of the "Former terms" table of `domain` (the text of `docs/domain.md`).
pub fn retired_terms(domain: &str) -> Vec<RetiredTerm> {
    let mut out = Vec::new();
    let mut in_section = false;
    let mut lint_col: Option<usize> = None;
    for line in domain.lines() {
        if line.starts_with("## ") {
            in_section = line.starts_with("## 8.") || line.to_lowercase().starts_with("## 8 ");
            lint_col = None;
            continue;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        if cells.iter().all(|c| c.trim().chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
            continue;
        }
        if clean(cells[0]).eq_ignore_ascii_case("former term") {
            lint_col = cells.iter().position(|c| clean(c).eq_ignore_ascii_case("lint"));
            continue;
        }
        let pending = line.contains("*code removal pending*");
        let now = cells.get(3).map(|c| clean(c)).unwrap_or_default();
        // `Lint:` notes: in a `Lint` column or anywhere in a cell.
        let mut extra: Vec<String> = Vec::new();
        let (mut all, mut none) = (false, false);
        for (i, cell) in cells.iter().enumerate() {
            let text = clean(cell);
            let note =
                if Some(i) == lint_col { Some(text.as_str()) } else { text.find("Lint:").map(|at| text[at + "Lint:".len()..].trim()) };
            let Some(note) = note else { continue };
            let note = note.strip_prefix("Lint:").unwrap_or(note).trim();
            match note {
                "all" => all = true,
                "none" => none = true,
                _ => extra.extend(note.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
            }
        }
        if none {
            continue;
        }
        let first = without_parentheses(cells[0]);
        let first = first.split("Lint:").next().unwrap_or("");
        let mut terms: Vec<String> = clean(first).split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        terms.retain(|t| all || normalise(t).contains(' '));
        terms.extend(extra);
        for term in terms {
            let phrase = normalise(&term);
            if !phrase.is_empty() {
                out.push(RetiredTerm { term, phrase, now: now.clone(), pending });
            }
        }
    }
    out
}

/// The 1-based lines of `text` on which `phrase` occurs as whole words (the line normalised as the phrase is).
fn mentions(text: &str, phrase: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let norm = normalise(line);
        let mut from = 0;
        while let Some(at) = norm[from..].find(phrase) {
            let start = from + at;
            let end = start + phrase.len();
            let before = norm[..start].chars().next_back();
            let after = norm[end..].chars().next();
            if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
                lines.push(i + 1);
                break;
            }
            from = start + phrase.len().max(1);
            if from >= norm.len() {
                break;
            }
        }
    }
    lines
}

/// Check `files` (path, text) against `terms`.
pub fn findings(terms: &[RetiredTerm], files: &[(String, String)]) -> Findings {
    let mut out = Findings::default();
    for (path, text) in files {
        let mut pending_here = false;
        for t in terms {
            for line in mentions(text, &t.phrase) {
                if t.pending {
                    *out.pending.entry(t.term.clone()).or_insert(0) += 1;
                    pending_here = true;
                } else {
                    let now = if t.now.is_empty() { "see docs/domain.md section 8".to_string() } else { t.now.clone() };
                    out.errors.push(format!("{path}:{line}: retired term '{}' (use: {now})", t.term));
                }
            }
        }
        out.pending_files += usize::from(pending_here);
    }
    out
}

/// The one line `spec-lint` prints about the mentions of terms whose code removal is pending.
pub fn pending_summary(f: &Findings) -> String {
    if f.pending.is_empty() {
        return String::new();
    }
    let total: usize = f.pending.values().sum();
    let per: Vec<String> = f.pending.iter().map(|(t, n)| format!("{t} {n}")).collect();
    format!("  retired terms with code removal pending (warnings): {total} mentions in {} files ({})\n", f.pending_files, per.join(", "))
}

/// Whether `spec-lint` checks the file at the repo-relative `path` (BUILD-028): the main specs, `domain.md` (above section 8), `status.md`,
/// `roadmap.md`, `README.md` and the files of `docs/briefings/`. ADRs, the archives, the log, background, learnings, spikes and the archived
/// changes may use the old words.
pub fn is_checked(path: &str) -> bool {
    let p = path.replace('\\', "/");
    if let Some(rest) = p.strip_prefix("openspec/specs/") {
        return rest.ends_with("/spec.md");
    }
    if let Some(rest) = p.strip_prefix("docs/briefings/") {
        return rest.ends_with(".md") && !rest.contains('/');
    }
    matches!(p.as_str(), "docs/domain.md" | "docs/status.md" | "docs/roadmap.md" | "docs/README.md")
}

/// The part of the file that is checked: `domain.md` stops at its "Former terms" section.
pub fn checked_text(path: &str, text: &str) -> String {
    if path.replace('\\', "/") != "docs/domain.md" {
        return text.to_string();
    }
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("## 8.") || line.to_lowercase().starts_with("## 8 ") {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAIN: &str = "# Domain\n\n## 3. Terms\n| a | b |\n|---|---|\n| chunk stage | still here |\n\n\
## 8. Former terms (non-normative)\nText.\n| Former term | What it was | Left | Now |\n|---|---|---|---|\n\
| old widget, legacy `gizmo mode` | a thing (see ADR 1) | 2026-10-01 | the new widget |\n\
| chunk stage, A-net erosion | fine erosion | off; *code removal pending* | M9's erosion |\n\
| block (regional square) | the first name | renamed | chunk |\n\
| spark | informal; Lint: spark plug | gone | plug |\n\
| quiet thing | skipped; Lint: none | gone | - |\n";

    fn term<'a>(all: &'a [RetiredTerm], t: &str) -> &'a RetiredTerm {
        all.iter().find(|x| x.term == t).unwrap_or_else(|| panic!("{t} not in {all:?}"))
    }

    // spec: BUILD-028
    #[test]
    fn the_terms_are_read_from_the_first_column_of_the_former_terms_table_with_remarks_dropped_and_single_words_skipped() {
        let all = retired_terms(DOMAIN);
        let names: Vec<&str> = all.iter().map(|t| t.term.as_str()).collect();
        assert_eq!(names, ["old widget", "legacy gizmo mode", "chunk stage", "A-net erosion", "spark plug"], "{all:?}");
        assert_eq!(term(&all, "old widget").now, "the new widget");
        assert!(!term(&all, "old widget").pending && term(&all, "chunk stage").pending && term(&all, "A-net erosion").pending);
        assert_eq!(term(&all, "A-net erosion").phrase, "a net erosion");
        assert!(retired_terms("## 3. Terms\n| Former term | x |\n| old widget | y |\n").is_empty(), "only section 8 counts");
    }

    // spec: BUILD-028
    #[test]
    fn a_planted_retired_term_fails_with_its_file_line_term_and_replacement_and_a_pending_one_only_warns() {
        let all = retired_terms(DOMAIN);
        let files = vec![
            (
                "openspec/specs/x/spec.md".to_string(),
                "fine line\nThe Old-Widget is drawn.\nthe chunk stage runs\nand the chunk_stage setting\n".to_string(),
            ),
            ("docs/status.md".to_string(), "a legacy gizmo   mode here\nwidgets and old widgetry are not whole words\n".to_string()),
        ];
        let f = findings(&all, &files);
        assert_eq!(
            f.errors,
            [
                "openspec/specs/x/spec.md:2: retired term 'old widget' (use: the new widget)",
                "docs/status.md:1: retired term 'legacy gizmo mode' (use: the new widget)",
            ]
        );
        assert_eq!(f.pending.get("chunk stage"), Some(&2), "{f:?}");
        assert_eq!(f.pending_files, 1);
        let line = pending_summary(&f);
        assert!(line.contains("2 mentions in 1 files") && line.contains("chunk stage 2"), "{line}");
        // Once the marker is gone the same text fails.
        let gone = retired_terms(&DOMAIN.replace("*code removal pending*", "removed"));
        let f = findings(&gone, &files);
        assert_eq!(f.errors.len(), 4, "{:?}", f.errors);
        assert!(f.pending.is_empty() && pending_summary(&f).is_empty());
    }

    // spec: BUILD-028
    #[test]
    fn a_text_that_is_not_checked_may_use_the_terms_and_only_the_given_files_are_looked_at() {
        let all = retired_terms(DOMAIN);
        // `findings` sees only the files it is given: an ADR is never handed to it.
        assert_eq!(findings(&all, &[]), Findings::default());
        assert!(!is_checked("docs/adr/0001-x.md") && !is_checked("docs/archive/M8/x.md") && !is_checked("docs/log.d/a.md"));
        assert!(!is_checked("docs/background/r.md") && !is_checked("docs/learnings.md") && !is_checked("spikes/a/results.md"));
        assert!(!is_checked("openspec/changes/archive/2026-10-30-x/proposal.md") && !is_checked("docs/log.md"));
        for p in [
            "openspec/specs/testing/spec.md",
            "docs/status.md",
            "docs/roadmap.md",
            "docs/README.md",
            "docs/briefings/week-x.md",
            "docs/domain.md",
        ] {
            assert!(is_checked(p), "{p}");
        }
    }

    // spec: BUILD-028
    #[test]
    fn domain_md_is_checked_above_section_eight_only() {
        let text = "# D\n## 6. Edits\nthe old widget\n## 8. Former terms\nthe old widget again\n";
        assert_eq!(checked_text("docs/domain.md", text), "# D\n## 6. Edits\nthe old widget\n");
        assert_eq!(checked_text("docs/status.md", text), text);
    }
}
