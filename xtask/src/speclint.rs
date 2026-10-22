//! CON-16: every requirement has a trace ID and a verification tier; every cited ID exists; every
//! active tier A/B/C requirement is cited by a `#[test]` function of its own tier (a `// spec:` comment counts only when
//! it sits directly above a `#[test]` fn; integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B, everything else tier A);
//! tier D names a review artefact. A `// spec:` comment directly above a perf-harness scene (`fn perf_*` in `crates/app/src`)
//! counts as tier-C evidence (CON-17: a scene measures what no test may); there is no tier E.
//!
//! Requirement format (OpenSpec headings, metadata lines in the body):
//! ```text
//! ### Requirement: COORD-003 Geo round-trip
//! The system SHALL ...
//! Verify: A            (A | B | C | D)
//! Status: active       (active | planned | deprecated)
//! Source: CON-15
//! Review: <artefact>   (tier D only)
//! ```

use crate::util::{capture, files_with_ext, Res};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct Requirement {
    pub id: String,
    pub spec: String,
    pub tier: Option<char>,
    pub status: Option<String>,
    pub review: Option<String>,
}

/// `ABC-012` style ID: 2-6 uppercase letters, a dash, two or three digits (constitution rules are `CON-15`).
pub fn is_trace_id(s: &str) -> bool {
    let Some((prefix, num)) = s.split_once('-') else { return false };
    (2..=6).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_uppercase())
        && (2..=3).contains(&num.len())
        && num.chars().all(|c| c.is_ascii_digit())
}

pub fn parse_requirements(spec_name: &str, text: &str) -> (Vec<Requirement>, Vec<String>) {
    let mut reqs: Vec<Requirement> = Vec::new();
    let mut errors = Vec::new();
    let mut removed = false;
    for (n, line) in text.lines().enumerate() {
        if let Some(h) = line.strip_prefix("## ") {
            removed = h.trim_start().starts_with("REMOVED");
        }
        if let Some(rest) = line.strip_prefix("### Requirement:") {
            let rest = rest.trim();
            let id = rest.split_whitespace().next().unwrap_or("");
            if !is_trace_id(id) {
                errors.push(format!("{spec_name}:{}: requirement heading '{rest}' does not start with a trace ID like COORD-003", n + 1));
            }
            let mut r = Requirement { id: id.to_string(), spec: spec_name.to_string(), ..Default::default() };
            if removed {
                // Removed by a change delta: outside the coverage rules.
                r.tier = Some('A');
                r.status = Some("deprecated".into());
            }
            reqs.push(r);
        } else if let (false, Some(r)) = (removed, reqs.last_mut()) {
            let t = line.trim();
            if let Some(v) = t.strip_prefix("Verify:") {
                r.tier = v.trim().chars().next();
            } else if let Some(v) = t.strip_prefix("Status:") {
                r.status = Some(v.trim().to_string());
            } else if let Some(v) = t.strip_prefix("Review:") {
                r.review = Some(v.trim().to_string());
            }
        }
    }
    (reqs, errors)
}

/// Trace IDs cited in `// spec: A-001, B-002` comments.
pub fn parse_citations(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let Some(c) = t.strip_prefix("//") else { continue };
        let c = c.trim_start_matches(['/', '!']).trim_start();
        if let Some(list) = c.strip_prefix("spec:") {
            ids.extend(list.split([',', ' ']).map(str::trim).filter(|s| is_trace_id(s)).map(String::from));
        }
    }
    ids
}

/// Trace IDs cited by `// spec:` comments that are attached to a `#[test]` function: the comment is followed only by
/// more comments and attributes, one of which is `#[test]`, and then the `fn`.
pub fn parse_test_citations(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut ids = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let Some(c) = t.strip_prefix("//") else { continue };
        let Some(list) = c.trim_start_matches(['/', '!']).trim_start().strip_prefix("spec:") else { continue };
        let (mut saw_test, mut attached, mut depth) = (false, false, 0i32);
        for next in &lines[i + 1..] {
            let n = next.trim();
            if depth > 0 {
                // Continuation of a multi-line attribute such as a wrapped `#[should_panic(expected = "x")]`.
                depth += bracket_balance(n);
                continue;
            }
            if n.starts_with("//") {
                continue;
            }
            if n.starts_with("#[") {
                let code = n.split("//").next().unwrap_or("").trim();
                saw_test |= code == "#[test]";
                depth = bracket_balance(code);
                continue;
            }
            attached = saw_test && starts_fn_item(n);
            break;
        }
        if attached {
            ids.extend(list.split([',', ' ']).map(str::trim).filter(|s| is_trace_id(s)).map(String::from));
        }
    }
    ids
}

/// Trace IDs cited by `// spec:` comments attached (comments and attributes only in between) to a perf-harness scene
/// entry point, a function named `perf_*` (CON-17: a scene measures what no test may). Callers restrict this to
/// [`is_perf_scene_file`]; it counts as tier C evidence only.
pub fn parse_scene_citations(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut ids = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let Some(c) = t.strip_prefix("//") else { continue };
        let Some(list) = c.trim_start_matches(['/', '!']).trim_start().strip_prefix("spec:") else { continue };
        let (mut depth, mut next_item) = (0i32, None);
        for next in &lines[i + 1..] {
            let n = next.trim();
            if depth > 0 {
                depth += bracket_balance(n);
            } else if n.starts_with("#[") {
                depth = bracket_balance(n.split("//").next().unwrap_or("").trim());
            } else if !n.starts_with("//") {
                next_item = Some(n);
                break;
            }
        }
        if next_item.is_some_and(is_perf_scene_fn) {
            ids.extend(list.split([',', ' ']).map(str::trim).filter(|s| is_trace_id(s)).map(String::from));
        }
    }
    ids
}

/// A `fn perf_*` item line (`pub`, `pub(crate)` allowed).
fn is_perf_scene_fn(line: &str) -> bool {
    let mut rest = line.strip_prefix("pub").map_or(line, str::trim_start);
    if rest.starts_with('(') {
        match rest.find(')') {
            Some(i) => rest = rest[i + 1..].trim_start(),
            None => return false,
        }
    }
    rest.strip_prefix("fn perf_").is_some()
}

/// Perf scenes live in the app crate's sources (`crates/app/src/*.rs`).
fn is_perf_scene_file(path: &Path) -> bool {
    let comps: Vec<_> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    comps.windows(3).any(|w| w[0] == "app" && w[1] == "src") && comps.first().is_some_and(|c| c == "crates")
}

/// Unclosed `[` minus `]` in `line` (attribute continuation tracking; brackets inside string literals are rare enough
/// in attributes that this is good enough, and a miscount only makes a citation detach loudly).
fn bracket_balance(line: &str) -> i32 {
    line.chars().map(|c| i32::from(c == '[') - i32::from(c == ']')).sum()
}

/// Whether `line` starts a function item: optional `pub`/`pub(..)`, then `const`/`async`/`unsafe`/`extern ".."`, then `fn`.
fn starts_fn_item(line: &str) -> bool {
    let mut rest = line;
    if let Some(r) = rest.strip_prefix("pub") {
        rest = r.trim_start();
        if rest.starts_with('(') {
            match rest.find(')') {
                Some(i) => rest = rest[i + 1..].trim_start(),
                None => return false,
            }
        }
    }
    loop {
        let before = rest;
        for kw in ["const ", "async ", "unsafe "] {
            if let Some(r) = rest.strip_prefix(kw) {
                rest = r.trim_start();
            }
        }
        if rest == before {
            break;
        }
    }
    rest.starts_with("fn ")
}

/// Verification tier of the tests in a source file (ADR 0009, TEST-001), matching the runner's `binary(/^gpu_/)` filters:
/// integration test files `tests/gpu_*.rs` are tier C, `tests/oracle_*.rs` tier B; every other file, including unit
/// tests under `src/`, is tier A.
pub fn test_tier(path: &Path) -> char {
    let in_tests_dir = path.parent().and_then(|p| p.file_name()).is_some_and(|d| d == "tests");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if in_tests_dir && stem.starts_with("gpu_") {
        'C'
    } else if in_tests_dir && stem.starts_with("oracle_") {
        'B'
    } else {
        'A'
    }
}

/// Constitution rule IDs (`CON-01`..), which tests may also cite.
fn constitution_ids(text: &str) -> BTreeSet<String> {
    text.split("**").filter(|s| s.starts_with("CON-") && is_trace_id(s)).map(String::from).collect()
}

pub struct Report {
    pub errors: Vec<String>,
    pub summary: String,
}

/// The Purpose section of a main spec: the text between `## Purpose` and the next `## ` heading, trimmed.
pub fn purpose_of(text: &str) -> Option<String> {
    let mut lines = text.lines();
    lines.find(|l| l.trim_end() == "## Purpose")?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
    Some(body.join("\n").trim().to_string())
}

/// A main spec needs a real Purpose: present, not empty, not OpenSpec's archive placeholder ("TBD ...").
fn purpose_errors(name: &str, text: &str) -> Option<String> {
    match purpose_of(text) {
        None => Some(format!("{name}: spec has no '## Purpose' section (say what the subsystem owns and which ADRs and constitution rules govern it)")),
        Some(p) if p.is_empty() => Some(format!("{name}: the Purpose section is empty")),
        Some(p) if p.to_ascii_uppercase().contains("TBD") => {
            Some(format!("{name}: the Purpose is a placeholder (contains 'TBD'): write what the subsystem owns and which ADRs and constitution rules govern it"))
        }
        Some(_) => None,
    }
}

pub fn lint(specs: &BTreeMap<String, String>, sources: &BTreeMap<PathBuf, String>, constitution: &str) -> Report {
    let mut errors = Vec::new();
    let mut reqs: Vec<Requirement> = Vec::new();
    // Main specs first, then change deltas (names starting with "change:"). A delta requirement replaces a
    // main requirement of the same ID (MODIFIED) or is added (ADDED).
    let is_delta = |n: &str| n.starts_with("change:");
    // Change deltas have no Purpose; main specs must.
    errors.extend(specs.iter().filter(|(n, _)| !is_delta(n)).filter_map(|(n, t)| purpose_errors(n, t)));
    for (name, text) in specs.iter().filter(|(n, _)| !is_delta(n)).chain(specs.iter().filter(|(n, _)| is_delta(n))) {
        let (parsed, e) = parse_requirements(name, text);
        errors.extend(e);
        for req in parsed {
            match reqs.iter().position(|x| x.id == req.id && is_delta(name) && !is_delta(&x.spec)) {
                Some(i) => reqs[i] = req,
                None => reqs.push(req),
            }
        }
    }
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for r in &reqs {
        if let Some(other) = seen.insert(&r.id, &r.spec) {
            errors.push(format!("{}: duplicate requirement ID (also in spec '{other}')", r.id));
        }
        match r.tier {
            Some('A' | 'B' | 'C' | 'D') => {}
            _ => errors.push(format!("{} ({}): missing or invalid 'Verify:' tier (A, B, C or D)", r.id, r.spec)),
        }
        match r.status.as_deref() {
            Some("active" | "planned" | "deprecated") => {}
            _ => errors.push(format!("{} ({}): missing or invalid 'Status:' (active, planned or deprecated)", r.id, r.spec)),
        }
        if r.tier == Some('D') && r.review.is_none() && r.status.as_deref() != Some("deprecated") {
            errors.push(format!("{} ({}): tier D requirements must name their review artefact in a 'Review:' line", r.id, r.spec));
        }
    }

    let known: BTreeSet<&str> = reqs.iter().map(|r| r.id.as_str()).collect();
    let con = constitution_ids(constitution);
    // id -> tiers of the `#[test]` functions that cite it.
    let mut cited: BTreeMap<String, BTreeSet<char>> = BTreeMap::new();
    for (path, text) in sources {
        for id in parse_citations(text) {
            if !known.contains(id.as_str()) && !con.contains(&id) {
                errors.push(format!(
                    "{}: cites '{id}', which is not a requirement in openspec/specs/ or a constitution rule",
                    path.display()
                ));
            }
        }
        for id in parse_test_citations(text) {
            cited.entry(id).or_default().insert(test_tier(path));
        }
        if is_perf_scene_file(path) {
            for id in parse_scene_citations(text) {
                cited.entry(id).or_default().insert('C');
            }
        }
    }
    for r in &reqs {
        let active = r.status.as_deref() == Some("active");
        let Some(tier @ ('A' | 'B' | 'C')) = r.tier else { continue };
        if !active {
            continue;
        }
        match cited.get(&r.id) {
            None => errors.push(format!(
                "{} ({}): active tier-{tier} requirement is not cited by any test (put `// spec: {}` directly above the `#[test]` fn that covers it, or mark the requirement planned)",
                r.id, r.spec, r.id
            )),
            Some(tiers) if !tiers.contains(&tier) => errors.push(format!(
                "{} ({}): requires a tier-{tier} test but is cited only by tier {} tests (tests/gpu_*.rs are tier C, tests/oracle_*.rs tier B, everything else tier A): add a tier-{tier} test or change the requirement's Verify tier",
                r.id,
                r.spec,
                tiers.iter().collect::<String>()
            )),
            Some(_) => {}
        }
    }

    let count = |f: &dyn Fn(&Requirement) -> bool| reqs.iter().filter(|r| f(r)).count();
    let mut summary = format!("{} requirements in {} specs\n", reqs.len(), specs.len());
    for tier in ['A', 'B', 'C', 'D'] {
        summary += &format!(
            "  tier {tier}: {} (active {}, planned {})\n",
            count(&|r| r.tier == Some(tier)),
            count(&|r| r.tier == Some(tier) && r.status.as_deref() == Some("active")),
            count(&|r| r.tier == Some(tier) && r.status.as_deref() == Some("planned")),
        );
    }
    let c_or_d = count(&|r| matches!(r.tier, Some('C' | 'D')) && r.status.as_deref() != Some("deprecated"));
    let live = count(&|r| r.status.as_deref() != Some("deprecated"));
    summary += &format!("  verifiable only at C or D: {c_or_d} of {live}\n");
    Report { errors, summary }
}

/// The most words a requirement's statement may have (ADR 0055 point 4, BUILD-024); a longer one is split, or its numbers go to a table.
pub const STATEMENT_MAX_WORDS: usize = 120;

/// The statements that were over [`STATEMENT_MAX_WORDS`] when BUILD-024 was added, one `ID words` line each (`long_statements.txt`). A
/// listed statement may shrink but not grow; the line is deleted when the statement is rewritten to the limit. No new statement is ever
/// listed.
pub const LONG_STATEMENTS: &str = include_str!("long_statements.txt");

/// First-person words a statement does not use (ADR 0055 point 4: no "we", no "I").
const FIRST_PERSON: [&str; 4] = ["we", "I", "our", "my"];

/// The requirements of a spec text with the words of the statement (the lines between the heading and the first metadata line or scenario)
/// and whether the statement uses a first-person word. Requirements removed by a change delta are left out.
pub fn statement_facts(text: &str) -> Vec<(String, usize, bool)> {
    let mut out: Vec<(String, usize, bool)> = Vec::new();
    let (mut removed, mut open) = (false, false);
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            removed = h.trim_start().starts_with("REMOVED");
        }
        if let Some(rest) = line.strip_prefix("### Requirement:") {
            open = !removed;
            if open {
                out.push((rest.split_whitespace().next().unwrap_or("").to_string(), 0, false));
            }
        } else if open {
            let t = line.trim();
            if ["Verify:", "Status:", "Source:", "Review:", "#"].iter().any(|p| t.starts_with(p)) {
                open = false;
            } else if let Some(last) = out.last_mut() {
                last.1 += t.split_whitespace().count();
                last.2 |= t
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '/'))
                    .any(|w| FIRST_PERSON.contains(&if w == "I" { w.to_string() } else { w.to_ascii_lowercase() }.as_str()));
            }
        }
    }
    out
}

/// BUILD-024: one error per requirement statement over [`STATEMENT_MAX_WORDS`] words (or over the words it was listed with in
/// [`LONG_STATEMENTS`]) and per statement with a first-person word. `specs` are the main specs by name.
pub fn style_errors(specs: &BTreeMap<String, String>, long_statements: &str) -> Vec<String> {
    let listed: BTreeMap<&str, usize> = long_statements
        .lines()
        .filter_map(|l| {
            let (id, w) = l.split_once(' ')?;
            Some((id, w.trim().parse().ok()?))
        })
        .collect();
    let mut errors = Vec::new();
    for (spec, text) in specs {
        for (id, words, first_person) in statement_facts(text) {
            let allowed = listed.get(id.as_str()).map_or(STATEMENT_MAX_WORDS, |w| (*w).max(STATEMENT_MAX_WORDS));
            if words > allowed {
                errors.push(format!(
                    "{id} ({spec}): the statement has {words} words, over {allowed} (ADR 0055, BUILD-024): split it into requirements, or move numbers into a table"
                ));
            }
            if first_person {
                errors.push(format!(
                    "{id} ({spec}): the statement uses \"we\", \"I\", \"our\" or \"my\" (ADR 0055, BUILD-024): write a definition"
                ));
            }
        }
    }
    errors
}

/// The files every session reads at its start and the most bytes each may hold (ADR 0075 point 1, BUILD-023): the status file, the learnings and
/// the spike register by size (15, 20 and 10 KiB), `CLAUDE.md` and the constitution by the size they had when the ADR was accepted. A cap changes
/// only by a new ADR.
pub const HOT_FILE_CAPS: [(&str, usize); 5] = [
    ("docs/status.md", 15 * 1024),
    ("docs/learnings.md", 20 * 1024),
    ("docs/spikes.md", 10 * 1024),
    ("CLAUDE.md", 7_514),
    ("docs/constitution.md", 8_574),
];

/// BUILD-023: one error per hot file over its cap, naming the file, its size and the cap. `sizes` is `(path, bytes)` with a line end counted
/// as one byte (a checkout with CRLF measures the same as one with LF); a file that is not in the list is not capped, one that is missing is not
/// an error here (the other checks read them).
pub fn hot_file_errors(sizes: &[(String, usize)]) -> Vec<String> {
    HOT_FILE_CAPS
        .iter()
        .filter_map(|(path, cap)| {
            let (_, size) = sizes.iter().find(|(p, _)| p == path)?;
            (size > cap).then(|| {
                format!(
                    "{path}: {size} B is over its cap of {cap} B (ADR 0075): trim it (the housekeeping skill: promote, keep or archive; a Tracked entry is at most three lines)"
                )
            })
        })
        .collect()
}

/// The size of a text with every `\r\n` counted as one byte.
pub fn lf_size(bytes: &[u8]) -> usize {
    bytes.len() - bytes.windows(2).filter(|w| w == b"\r\n").count()
}

/// Largest tracked file the conflict-marker scan reads (BUILD-009); bigger files are generated or binary in practice.
const MARKER_SCAN_MAX_BYTES: usize = 2 * 1024 * 1024;

/// BUILD-009: the 1-based line of a git conflict marker in `text`, if any. A full block (opening, separator, closing, in order)
/// reports its opening line; an opening or closing marker of git format (`<<<<<<< ref`) alone reports its own line. A lone
/// `=======` (a markdown setext underline) or a bare `<<<<<<<` never counts.
pub fn conflict_marker_line(text: &str) -> Option<usize> {
    // A marker of git format: seven characters, a space, a ref name.
    fn named(line: &str, c: char) -> bool {
        let seven = c.to_string().repeat(7);
        line.strip_prefix(seven.as_str()).is_some_and(|rest| rest.strip_prefix(' ').is_some_and(|r| !r.trim().is_empty()))
    }
    let (mut open, mut sep) = (None, false);
    for (i, line) in text.lines().enumerate() {
        let open_marker = line == "<<<<<<<" || line.starts_with("<<<<<<< ");
        let close_marker = line == ">>>>>>>" || line.starts_with(">>>>>>> ");
        if named(line, '<') || named(line, '>') {
            return Some(open.unwrap_or(i + 1));
        }
        if open_marker && open.is_none() {
            open = Some(i + 1);
        } else if open.is_some() && line == "=======" {
            sep = true;
        } else if close_marker && sep {
            return open;
        }
    }
    None
}

/// BUILD-009: one message per file with conflict markers. Files that are not UTF-8, hold a NUL byte or exceed the size cap are skipped.
pub fn conflict_marker_errors(files: &[(String, Vec<u8>)]) -> Vec<String> {
    files
        .iter()
        .filter(|(_, bytes)| bytes.len() <= MARKER_SCAN_MAX_BYTES && !bytes.contains(&0))
        .filter_map(|(path, bytes)| {
            let line = conflict_marker_line(std::str::from_utf8(bytes).ok()?)?;
            Some(format!("{path}:{line}: git conflict marker (resolve the merge conflict; BUILD-009)"))
        })
        .collect()
}

pub fn run(root: &Path) -> Res {
    let mut specs = BTreeMap::new();
    let openspec = root.join("openspec");
    // Main specs, plus the delta specs of changes in progress (not the archive).
    let mut dirs = vec![(String::new(), openspec.join("specs"))];
    if let Ok(rd) = std::fs::read_dir(openspec.join("changes")) {
        for e in rd.flatten().filter(|e| e.file_name() != "archive" && e.path().is_dir()) {
            dirs.push((format!("change:{}/", e.file_name().to_string_lossy()), e.path().join("specs")));
        }
    }
    for (prefix, dir) in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let f = e.path().join("spec.md");
            if f.is_file() {
                let name = format!("{prefix}{}", e.file_name().to_string_lossy());
                specs.insert(name, std::fs::read_to_string(&f).map_err(|e| e.to_string())?);
            }
        }
    }
    let mut files = Vec::new();
    for d in ["crates", "xtask", "tests"] {
        files_with_ext(&root.join(d), "rs", &mut files);
    }
    let mut sources = BTreeMap::new();
    for f in files {
        let rel = f.strip_prefix(root).unwrap_or(&f).to_path_buf();
        sources.insert(rel, std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?);
    }
    let constitution = std::fs::read_to_string(root.join("docs").join("constitution.md")).map_err(|e| e.to_string())?;
    let mut report = lint(&specs, &sources, &constitution);
    report.errors.extend(style_errors(
        &specs.iter().filter(|(n, _)| !n.starts_with("change:")).map(|(n, t)| (n.clone(), t.clone())).collect(),
        LONG_STATEMENTS,
    ));
    // BUILD-009: tracked files only (git is on every CI runner).
    let tracked = capture(root, "git", &["ls-files", "-z"])?;
    let files: Vec<(String, Vec<u8>)> =
        tracked.split('\0').filter(|p| !p.is_empty()).filter_map(|p| Some((p.to_string(), std::fs::read(root.join(p)).ok()?))).collect();
    report.errors.extend(conflict_marker_errors(&files));
    // BUILD-023: the hot files stay within their caps (ADR 0075).
    let sizes: Vec<(String, usize)> =
        HOT_FILE_CAPS.iter().filter_map(|(path, _)| Some((path.to_string(), lf_size(&std::fs::read(root.join(path)).ok()?)))).collect();
    report.errors.extend(hot_file_errors(&sizes));
    // BUILD-010: every log fragment is one well-formed entry.
    report.errors.extend(crate::logcmd::fragment_errors(&crate::logcmd::read_fragments(root)?));
    eprint!("{}", report.summary);
    if report.errors.is_empty() {
        eprintln!("spec-lint ok");
        Ok(())
    } else {
        Err(format!("spec-lint found {} problem(s):\n  {}", report.errors.len(), report.errors.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(c: char) -> String {
        c.to_string().repeat(7)
    }

    /// A merge-conflict block, built at run time so that no tracked file holds a marker line.
    fn conflict_block() -> String {
        format!("intro\n{} HEAD\nours\n{}\ntheirs\n{} origin/main\noutro\n", marks('<'), marks('='), marks('>'))
    }

    // spec: BUILD-023
    #[test]
    fn a_hot_file_over_its_cap_is_named_with_its_size_and_one_at_or_under_it_passes() {
        let at = |p: &str, n: usize| (p.to_string(), n);
        // At the cap and under it: nothing.
        let ok: Vec<(String, usize)> = HOT_FILE_CAPS.iter().map(|(p, c)| at(p, *c)).collect();
        assert!(hot_file_errors(&ok).is_empty(), "{:?}", hot_file_errors(&ok));
        // One byte over: named, with the size and the cap; the others are not.
        for (path, cap) in HOT_FILE_CAPS {
            let mut sizes = ok.clone();
            sizes.iter_mut().find(|(p, _)| p == path).unwrap().1 = cap + 1;
            let errs = hot_file_errors(&sizes);
            assert_eq!(errs.len(), 1, "{path}: {errs:?}");
            assert!(
                errs[0].starts_with(path) && errs[0].contains(&format!("{} B", cap + 1)) && errs[0].contains(&format!("cap of {cap} B")),
                "{errs:?}"
            );
        }
        // The caps are those of ADR 0075: 15, 20 and 10 KiB, and the two files at the size they had when it was accepted.
        let cap = |name: &str| HOT_FILE_CAPS.iter().find(|(p, _)| *p == name).unwrap().1;
        assert_eq!(
            [cap("docs/status.md"), cap("docs/learnings.md"), cap("docs/spikes.md"), cap("CLAUDE.md"), cap("docs/constitution.md")],
            [15_360, 20_480, 10_240, 7_514, 8_574]
        );
        // A file that is missing or not capped is no error.
        assert!(hot_file_errors(&[]).is_empty());
        assert!(hot_file_errors(&[at("docs/log.md", 10_000_000)]).is_empty());
    }

    // spec: BUILD-023
    #[test]
    fn the_size_counts_a_crlf_line_end_as_one_byte() {
        assert_eq!(lf_size(b"a\r\nb\r\n"), 4);
        assert_eq!(lf_size(b"a\nb\n"), 4);
        assert_eq!(lf_size(b"a\rb"), 3, "a lone carriage return is a byte");
        assert_eq!(lf_size(b""), 0);
    }

    // spec: BUILD-023
    #[test]
    fn the_hot_files_of_this_repository_are_within_their_caps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let sizes: Vec<(String, usize)> = HOT_FILE_CAPS
            .iter()
            .map(|(p, _)| (p.to_string(), lf_size(&std::fs::read(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}")))))
            .collect();
        assert!(hot_file_errors(&sizes).is_empty(), "{:?}", hot_file_errors(&sizes));
    }

    // spec: BUILD-009
    #[test]
    fn full_conflict_block_names_file_and_line() {
        let files = [("docs/log.md".to_string(), conflict_block().into_bytes())];
        let errs = conflict_marker_errors(&files);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("docs/log.md:2"), "{errs:?}");
    }

    // spec: BUILD-009
    #[test]
    fn lone_marker_with_ref_fails_but_bare_or_separator_passes() {
        assert_eq!(conflict_marker_line(&format!("a\n{} main\n", marks('>'))), Some(2));
        assert_eq!(conflict_marker_line(&format!("a\r\n{} main\r\n", marks('<'))), Some(2));
        assert_eq!(conflict_marker_line(&format!("a\n{}\n", marks('<'))), None);
        assert_eq!(conflict_marker_line(&format!("{}\nb\n{}\n", marks('='), marks('='))), None);
        // out of order: closing, separator, opening is not a block
        assert_eq!(conflict_marker_line(&format!("{}\n{}\n{}\n", marks('>'), marks('='), marks('<'))), None);
        // indented quotes (code in a list) are not markers
        assert_eq!(conflict_marker_line(&format!("  {} HEAD\n", marks('<'))), None);
    }

    // spec: BUILD-009
    #[test]
    fn setext_underline_and_clean_tree_pass() {
        let md = format!("Title\n{}\n\ntext\n", marks('='));
        let files = [("README.md".to_string(), md.into_bytes()), ("a.rs".to_string(), b"fn main() {}\n".to_vec())];
        assert!(conflict_marker_errors(&files).is_empty());
        assert!(conflict_marker_errors(&[]).is_empty());
    }

    // spec: BUILD-009
    #[test]
    fn binary_and_oversized_files_are_skipped() {
        let mut bin = conflict_block().into_bytes();
        bin.push(0);
        let mut big = conflict_block().into_bytes();
        big.resize(MARKER_SCAN_MAX_BYTES + 1, b'x');
        let mut non_utf8 = conflict_block().into_bytes();
        non_utf8.push(0xff);
        let files = [("a.png".to_string(), bin), ("big.txt".to_string(), big), ("b.bin".to_string(), non_utf8)];
        assert!(conflict_marker_errors(&files).is_empty());
    }

    const SPEC: &str = "\
### Requirement: COORD-003 Geo round-trip
The system SHALL round-trip.
Verify: A
Status: active

### Requirement: REND-001 Looks nice
The renderer SHALL look nice.
Verify: D
Status: active
";

    /// A main spec file: a real Purpose section, then the fixture's requirements.
    fn with_purpose(body: &str) -> String {
        format!("# coordinates Specification\n\n## Purpose\nOwns the fixture coordinates.\n\n## Requirements\n\n{body}")
    }

    fn lint_with(spec: &str, source: &str) -> Report {
        let specs = BTreeMap::from([("coordinates".to_string(), with_purpose(spec))]);
        let sources = BTreeMap::from([(PathBuf::from("a.rs"), source.to_string())]);
        lint(&specs, &sources, "- **CON-15** x")
    }

    // spec: TEST-001
    #[test]
    fn trace_ids_are_recognised() {
        assert!(is_trace_id("COORD-003") && is_trace_id("CON-15"));
        for bad in ["coord-003", "COORD-3", "COORD-0004", "COORD003", "C-001", "TOOLONGX-001", "COORD-0x3"] {
            assert!(!is_trace_id(bad), "{bad}");
        }
    }

    // spec: TEST-001
    #[test]
    fn citations_are_parsed_from_comments_only() {
        let src = "// spec: COORD-003, COORD-004\nlet s = \"spec: ZZZ-999\";\n    /// spec: REND-001 STRM-002\n";
        assert_eq!(parse_citations(src), ["COORD-003", "COORD-004", "REND-001", "STRM-002"]);
    }

    // spec: TEST-001
    #[test]
    fn uncited_active_requirement_and_missing_review_are_reported() {
        let r = lint_with(SPEC, "// nothing here\n");
        let joined = r.errors.join("\n");
        assert!(joined.contains("COORD-003") && joined.contains("not cited by any test"), "{joined}");
        assert!(joined.contains("REND-001") && joined.contains("Review:"), "{joined}");
    }

    // spec: TEST-001
    #[test]
    fn citing_an_unknown_id_is_reported_and_known_ids_pass() {
        let spec = SPEC.replace("Verify: D\nStatus: active", "Verify: D\nStatus: active\nReview: contact sheet");
        let r = lint_with(&spec, "// spec: COORD-003\n#[test]\nfn t() {}\n// spec: NOPE-001\n");
        assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
        assert!(r.errors[0].contains("NOPE-001"));
        assert!(lint_with(&spec, "// spec: COORD-003, CON-15\n#[test]\nfn t() {}\n").errors.is_empty());
    }

    // spec: TEST-001
    #[test]
    fn change_deltas_replace_main_requirements_and_removed_ones_leave_the_rules() {
        let main =
            "### Requirement: COORD-003 Geo\nVerify: A\nStatus: planned\n\n### Requirement: COORD-009 Old\nVerify: A\nStatus: active\n";
        let delta = "## MODIFIED Requirements\n### Requirement: COORD-003 Geo\nVerify: A\nStatus: active\n\n\
                     ## REMOVED Requirements\n### Requirement: COORD-009 Old\n**Reason**: gone\n";
        let specs =
            BTreeMap::from([("coordinates".to_string(), with_purpose(main)), ("change:m1/coordinates".to_string(), delta.to_string())]);
        let sources = BTreeMap::new();
        let r = lint(&specs, &sources, "");
        // COORD-003 is active in the delta and uncited; COORD-009 is removed, so not reported, and no duplicate error.
        assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
        assert!(r.errors[0].starts_with("COORD-003"), "{:?}", r.errors);
    }

    // spec: TEST-001
    #[test]
    fn missing_tier_status_and_duplicates_are_reported() {
        let spec = "### Requirement: COORD-001 A\ntext\n\n### Requirement: COORD-001 B\nVerify: Q\nStatus: soon\n";
        let r = lint_with(spec, "");
        let j = r.errors.join("\n");
        assert!(j.contains("duplicate") && j.contains("Verify") && j.contains("Status"), "{j}");
    }

    fn lint_at(spec: &str, path: &str, source: &str) -> Report {
        let specs = BTreeMap::from([("coordinates".to_string(), with_purpose(spec))]);
        let sources = BTreeMap::from([(PathBuf::from(path), source.to_string())]);
        lint(&specs, &sources, "- **CON-15** x")
    }

    const ONE: &str = "### Requirement: COORD-003 Geo round-trip
The system SHALL round-trip.
Verify: A
Status: active
";

    // spec: TEST-001
    #[test]
    fn only_citations_attached_to_a_test_function_count() {
        let attached = "// spec: COORD-003
#[test]
fn t() {}
";
        assert!(lint_at(ONE, "a.rs", attached).errors.is_empty());
        // Attributes and further comments between the citation and the fn are fine.
        assert!(lint_at(
            ONE,
            "a.rs",
            "// spec: COORD-003
// more
#[test]
#[ignore]
fn t() {}
"
        )
        .errors
        .is_empty());
        // A citation above a helper, a constant, a module or a test-less fn covers nothing.
        for loose in [
            "// spec: COORD-003
fn helper() {}
",
            "// spec: COORD-003
const X: u32 = 1;
",
            "// spec: COORD-003
mod tests {
    #[test]
    fn t() {}
}
",
            "// spec: COORD-003
#[cfg(test)]
fn not_a_test() {}
",
            "// spec: COORD-003
",
        ] {
            let r = lint_at(ONE, "a.rs", loose);
            assert!(
                r.errors.iter().any(|e| e.contains("not cited by any test") && e.contains("directly above")),
                "{loose}: {:?}",
                r.errors
            );
        }
        // A stray citation of an unknown ID is still reported wherever it sits.
        assert!(lint_at(
            ONE,
            "a.rs",
            "// spec: NOPE-001
fn helper() {}
"
        )
        .errors
        .iter()
        .any(|e| e.contains("NOPE-001")));
    }

    // spec: TEST-001
    #[test]
    fn a_requirement_needs_a_test_of_its_own_tier() {
        let test = "// spec: COORD-003
#[test]
fn t() {}
";
        for (tier, ok_path, bad_path) in [
            ('A', "x/src/a.rs", "x/tests/gpu_a.rs"),
            ('B', "x/tests/oracle_a.rs", "x/tests/a.rs"),
            ('C', "x/tests/gpu_a.rs", "x/tests/oracle_a.rs"),
        ] {
            let spec = ONE.replace("Verify: A", &format!("Verify: {tier}"));
            assert!(lint_at(&spec, ok_path, test).errors.is_empty(), "tier {tier} at {ok_path}");
            let r = lint_at(&spec, bad_path, test);
            let j = r.errors.join(
                "
",
            );
            assert!(
                j.contains(&format!("requires a tier-{tier} test")) && j.contains("cited only by tier"),
                "tier {tier} at {bad_path}: {j}"
            );
        }
        // One test of the right tier among several of others is enough.
        let spec = ONE.replace("Verify: A", "Verify: C");
        let specs = BTreeMap::from([("coordinates".to_string(), with_purpose(&spec))]);
        let sources =
            BTreeMap::from([(PathBuf::from("a/tests/x.rs"), test.to_string()), (PathBuf::from("a/tests/gpu_x.rs"), test.to_string())]);
        assert!(lint(&specs, &sources, "").errors.is_empty());
    }

    // spec: TEST-001
    #[test]
    fn a_perf_scene_citation_is_tier_c_evidence() {
        let spec = ONE.replace("Verify: A", "Verify: C");
        let scene = "// spec: COORD-003
/// docs
pub fn perf_thing() {}
";
        // Cited above a perf scene entry point under crates/app/src: tier C evidence, and no tier A/B.
        assert!(lint_at(&spec, "crates/app/src/thingperf.rs", scene).errors.is_empty());
        // Removing the citation fails the rule.
        let r = lint_at(
            &spec,
            "crates/app/src/thingperf.rs",
            "/// docs
pub fn perf_thing() {}
",
        );
        assert!(r.errors.iter().any(|e| e.contains("COORD-003") && e.contains("not cited by any test")), "{:?}", r.errors);
        // Not a scene: another function name, another place, or a tier A requirement.
        for (path, src) in [
            (
                "crates/app/src/thingperf.rs",
                "// spec: COORD-003
pub fn helper() {}
",
            ),
            ("crates/core/src/thing.rs", scene),
            ("crates/app/tests/thing.rs", scene),
        ] {
            assert!(!lint_at(&spec, path, src).errors.is_empty(), "{path}");
        }
        assert!(!lint_at(ONE, "crates/app/src/thingperf.rs", scene).errors.is_empty(), "a scene is not tier A evidence");
        // Wrapped attributes between the citation and the scene are fine, as for tests.
        let wrapped = "// spec: COORD-003
#[allow(
    dead_code
)]
pub(crate) fn perf_thing() {}
";
        assert!(lint_at(&spec, "crates/app/src/thingperf.rs", wrapped).errors.is_empty());
    }

    // spec: TEST-001
    #[test]
    fn test_files_are_classified_by_name() {
        assert_eq!(test_tier(Path::new("crates/render/tests/gpu_terrain.rs")), 'C');
        assert_eq!(test_tier(Path::new("crates/render/tests/oracle_shader_validation.rs")), 'B');
        assert_eq!(test_tier(Path::new("crates/core/tests/cube_sphere.rs")), 'A');
        assert_eq!(test_tier(Path::new("xtask/src/speclint.rs")), 'A');
        // Only top-level integration test files count: a gpu_* file under src/ is a unit-test module (tier A) and a
        // substring match (debug_gpu_x) is not the runner's prefix match.
        assert_eq!(test_tier(Path::new("crates/x/src/gpu_util.rs")), 'A');
        assert_eq!(test_tier(Path::new("crates/x/tests/debug_gpu_x.rs")), 'A');
    }

    // spec: TEST-001
    #[test]
    fn realistic_test_declarations_are_attached() {
        for decl in [
            "#[test]
pub(crate) fn t() {}
",
            "#[test] // checks the thing
fn t() {}
",
            "#[test]
#[should_panic(
    expected = \"boom\"
)]
fn t() {}
",
            "#[test]
#[cfg_attr(
    miri,
    ignore
)]
pub fn t() {}
",
            "#[should_panic]
#[test]
fn t() {}
",
            "#[test]
unsafe fn t() {}
",
        ] {
            let src = format!(
                "// spec: COORD-003
{decl}"
            );
            assert!(lint_at(ONE, "a.rs", &src).errors.is_empty(), "{decl}");
        }
        // Still detached: a code line or a blank line in between, a non-test attribute only, a non-fn item.
        for decl in [
            "const X: u32 = 1;
#[test]
fn t() {}
",
            "
#[test]
fn t() {}
",
            "#[inline]
fn t() {}
",
            "#[test]
struct S;
",
        ] {
            let src = format!(
                "// spec: COORD-003
{decl}"
            );
            assert!(!lint_at(ONE, "a.rs", &src).errors.is_empty(), "{decl}");
        }
    }

    // spec: TEST-001
    #[test]
    fn a_main_spec_needs_a_real_purpose() {
        let run = |text: &str| {
            let specs = BTreeMap::from([("coordinates".to_string(), text.to_string())]);
            lint(&specs, &BTreeMap::new(), "").errors.join("\n")
        };
        let reqs = "## Requirements\n\n";
        let ok = format!("## Purpose\nOwns the frames (ADR 0001, CON-01).\n\n{reqs}");
        assert!(run(&ok).is_empty(), "{}", run(&ok));
        // The archive placeholder, in any case, an empty section and a missing section are all refused.
        let placeholder = format!("## Purpose\nTBD - created by archiving change m0-foundations. Update Purpose after archive.\n\n{reqs}");
        assert!(run(&placeholder).contains("placeholder"), "{}", run(&placeholder));
        assert!(run(&format!("## Purpose\nowns frames, tbd later\n\n{reqs}")).contains("placeholder"));
        assert!(run(&format!("## Purpose\n\n{reqs}")).contains("empty"));
        assert!(run(reqs).contains("no '## Purpose'"));
        // A change delta carries no Purpose and is not asked for one.
        let specs = BTreeMap::from([("change:x/coordinates".to_string(), "## ADDED Requirements\n".to_string())]);
        assert!(lint(&specs, &BTreeMap::new(), "").errors.is_empty());
    }

    fn req(id: &str, statement: &str) -> String {
        format!("### Requirement: {id} Name\n{statement}\n\nVerify: A\nStatus: active\nSource: CON-01\n\n#### Scenario: S\n- **WHEN** a thing\n")
    }

    fn style(text: &str, listed: &str) -> Vec<String> {
        style_errors(&BTreeMap::from([("coordinates".to_string(), text.to_string())]), listed)
    }

    // spec: BUILD-024
    #[test]
    fn a_statement_over_the_word_limit_fails_unless_listed_with_at_least_its_words() {
        let words = |n: usize| vec!["word"; n].join(" ");
        assert!(style(&req("COORD-001", &words(120)), "").is_empty(), "120 words pass");
        let e = style(&req("COORD-001", &words(121)), "");
        assert!(e.len() == 1 && e[0].starts_with("COORD-001 (coordinates)") && e[0].contains("121 words, over 120"), "{e:?}");
        // The scenario and the metadata lines do not count; the lines of a long statement do.
        assert!(style(&req("COORD-001", &format!("{}\n{}", words(100), words(30))), "")[0].contains("130 words"));
        // A listed statement may keep its words or shrink, never grow; a list entry below the limit does not lower the limit.
        assert!(style(&req("COORD-001", &words(200)), "COORD-001 200\n").is_empty());
        assert!(style(&req("COORD-001", &words(150)), "COORD-001 200\n").is_empty());
        assert!(style(&req("COORD-001", &words(201)), "COORD-001 200\n")[0].contains("201 words, over 200"));
        assert!(style(&req("COORD-001", &words(120)), "COORD-001 50\n").is_empty());
        // Another ID is not covered by the entry.
        assert_eq!(style(&req("COORD-002", &words(200)), "COORD-001 200\n").len(), 1);
    }

    // spec: BUILD-024
    #[test]
    fn a_statement_with_a_first_person_word_fails_but_io_and_units_pass() {
        assert!(style(&req("COORD-001", "The system SHALL read I/O and wait 52,000 us, at the user's position."), "").is_empty());
        for bad in ["We chose the cube.", "The value is ours, we say.", "It SHALL do what I mean.", "This is my reading."] {
            let e = style(&req("COORD-001", bad), "");
            assert!(e.len() == 1 && e[0].contains("\"we\""), "{bad}: {e:?}");
        }
        // A scenario may say anything; a REMOVED requirement is not read.
        assert!(style("### Requirement: COORD-001 N\nThe system SHALL work.\n\nVerify: A\n\n#### Scenario: S\n- **WHEN** we do\n", "")
            .is_empty());
        assert!(
            style("## REMOVED Requirements\n### Requirement: COORD-009 Old\nWe removed it, and I agree: word word word.\n", "").is_empty()
        );
    }
}
