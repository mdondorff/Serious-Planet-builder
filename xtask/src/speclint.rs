//! CON-16: every requirement has a trace ID and a verification tier; every cited ID exists; every
//! active tier A/B/C requirement is cited by at least one automated test; tier D names a review artefact.
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

use crate::util::{files_with_ext, Res};
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
    for (n, line) in text.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("### Requirement:") {
            let rest = rest.trim();
            let id = rest.split_whitespace().next().unwrap_or("");
            if !is_trace_id(id) {
                errors.push(format!("{spec_name}:{}: requirement heading '{rest}' does not start with a trace ID like COORD-003", n + 1));
            }
            reqs.push(Requirement { id: id.to_string(), spec: spec_name.to_string(), ..Default::default() });
        } else if let Some(r) = reqs.last_mut() {
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

/// Constitution rule IDs (`CON-01`..), which tests may also cite.
fn constitution_ids(text: &str) -> BTreeSet<String> {
    text.split("**").filter(|s| s.starts_with("CON-") && is_trace_id(s)).map(String::from).collect()
}

pub struct Report {
    pub errors: Vec<String>,
    pub summary: String,
}

pub fn lint(specs: &BTreeMap<String, String>, sources: &BTreeMap<PathBuf, String>, constitution: &str) -> Report {
    let mut errors = Vec::new();
    let mut reqs = Vec::new();
    for (name, text) in specs {
        let (r, e) = parse_requirements(name, text);
        reqs.extend(r);
        errors.extend(e);
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
    let mut cited: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, text) in sources {
        for id in parse_citations(text) {
            if !known.contains(id.as_str()) && !con.contains(&id) {
                errors.push(format!(
                    "{}: cites '{id}', which is not a requirement in openspec/specs/ or a constitution rule",
                    path.display()
                ));
            }
            cited.entry(id).or_default().push(path.display().to_string());
        }
    }
    for r in &reqs {
        let active = r.status.as_deref() == Some("active");
        if active && matches!(r.tier, Some('A' | 'B' | 'C')) && !cited.contains_key(&r.id) {
            errors.push(format!(
                "{} ({}): active tier-{} requirement is not cited by any test (add `// spec: {}` to the test that covers it, or mark it planned)",
                r.id,
                r.spec,
                r.tier.unwrap_or('?'),
                r.id
            ));
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

pub fn run(root: &Path) -> Res {
    let mut specs = BTreeMap::new();
    let spec_dir = root.join("openspec").join("specs");
    let rd = std::fs::read_dir(&spec_dir).map_err(|e| format!("{}: {e}", spec_dir.display()))?;
    for e in rd.flatten() {
        let f = e.path().join("spec.md");
        if f.is_file() {
            specs.insert(e.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(&f).map_err(|e| e.to_string())?);
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
    let report = lint(&specs, &sources, &constitution);
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

    fn lint_with(spec: &str, source: &str) -> Report {
        let specs = BTreeMap::from([("coordinates".to_string(), spec.to_string())]);
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
        let r = lint_with(&spec, "// spec: COORD-003\n// spec: NOPE-001\n");
        assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
        assert!(r.errors[0].contains("NOPE-001"));
        assert!(lint_with(&spec, "// spec: COORD-003, CON-15\n").errors.is_empty());
    }

    // spec: TEST-001
    #[test]
    fn missing_tier_status_and_duplicates_are_reported() {
        let spec = "### Requirement: COORD-001 A\ntext\n\n### Requirement: COORD-001 B\nVerify: Q\nStatus: soon\n";
        let r = lint_with(spec, "");
        let j = r.errors.join("\n");
        assert!(j.contains("duplicate") && j.contains("Verify") && j.contains("Status"), "{j}");
    }
}
