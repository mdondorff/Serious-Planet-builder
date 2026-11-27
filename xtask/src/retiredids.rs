//! The retired requirement IDs (BUILD-073): `openspec/retired-ids.txt` lists every requirement ID that was written and is in no main spec any more
//! (removed, or replaced under another ID), one a line, sorted by prefix and number, `#` lines comments. `cargo xtask next-id` reads it instead
//! of `openspec/changes/archive/`, so the archive can be deleted without an ID being issued again. spec-lint checks the file's form, that no
//! requirement reuses a retired ID, and that every requirement of the change archive is either in a main spec or in the file.

use std::collections::{BTreeMap, BTreeSet};

/// The file, relative to the repository root.
pub const FILE: &str = "openspec/retired-ids.txt";

/// `PREFIX-NNN`: uppercase letters, a dash, digits.
fn is_id(s: &str) -> bool {
    match s.split_once('-') {
        Some((p, n)) => {
            !p.is_empty() && p.chars().all(|c| c.is_ascii_uppercase()) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// The sort key of an ID: its prefix, then its number.
fn key(id: &str) -> (String, u32) {
    let (p, n) = id.split_once('-').unwrap_or((id, "0"));
    (p.to_string(), n.parse().unwrap_or(0))
}

/// The IDs of the file, or the reasons its form is wrong (a line not an ID, an ID twice, an ID out of order), each naming the line.
pub fn parse(text: &str) -> Result<Vec<String>, Vec<String>> {
    let (mut ids, mut errors) = (Vec::<String>::new(), Vec::new());
    for (k, raw) in text.lines().enumerate() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let at = format!("{FILE}:{}", k + 1);
        if !is_id(l) {
            errors.push(format!("{at}: `{l}` is not a requirement ID (PREFIX-NNN; BUILD-073)"));
            continue;
        }
        if let Some(prev) = ids.last() {
            match key(prev).cmp(&key(l)) {
                std::cmp::Ordering::Equal => errors.push(format!("{at}: {l} is listed twice (BUILD-073)")),
                std::cmp::Ordering::Greater => {
                    errors.push(format!("{at}: {l} is out of order after {prev} (sorted by prefix, then number; BUILD-073)"))
                }
                std::cmp::Ordering::Less => {}
            }
        }
        ids.push(l.to_string());
    }
    if errors.is_empty() {
        Ok(ids)
    } else {
        Err(errors)
    }
}

/// The IDs of the `### Requirement:` headings of a spec text, with the delta section each stands under (`""` in a main spec). A RENAMED block's
/// old heading (`- FROM: \`### Requirement: ...\``) counts too: a rename that changes the ID retires the old one.
pub fn headings(text: &str) -> Vec<(String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for l in text.lines() {
        let rest = l.strip_prefix("### Requirement: ").or_else(|| l.trim_start().strip_prefix("- FROM: `### Requirement: "));
        if let Some(s) = l.strip_prefix("## ") {
            section = s.trim().to_string();
        } else if let Some(rest) = rest {
            if let Some(id) = rest.split_whitespace().next().filter(|id| is_id(id)) {
                out.push((id.to_string(), section.clone()));
            }
        }
    }
    out
}

/// spec-lint's checks of BUILD-073. `specs` holds the main specs (names without `change:`) and the delta specs of open changes (`change:<id>/...`,
/// as spec-lint reads them); `archived` the delta spec texts of `openspec/changes/archive/` (path, text); `retired` the file's text, `None` when
/// it is missing.
pub fn errors(specs: &BTreeMap<String, String>, archived: &[(String, String)], retired: Option<&str>) -> Vec<String> {
    let Some(text) = retired else {
        return vec![format!("{FILE}: missing; it lists the retired requirement IDs that next-id must never issue again (BUILD-073)")];
    };
    let ids = match parse(text) {
        Ok(ids) => ids,
        Err(e) => return e,
    };
    let retired: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    let mut main = BTreeSet::new();
    for (name, t) in specs {
        for (id, section) in headings(t) {
            let open_change = name.starts_with("change:");
            if !open_change {
                main.insert(id.clone());
            }
            // a requirement of a main spec, or one an open change adds, under a retired ID reuses it
            if (!open_change || section.starts_with("ADDED")) && retired.contains(id.as_str()) {
                out.push(format!("{name}: {id} reuses a retired requirement ID ({FILE}); take `cargo xtask next-id` (BUILD-073)"));
            }
        }
    }
    // every requirement the change archive wrote is in a main spec or in the file, so deleting the archive loses no ID
    let mut missing = BTreeSet::new();
    for (path, t) in archived {
        for (id, _) in headings(t) {
            if !main.contains(&id) && !retired.contains(id.as_str()) && missing.insert(id.clone()) {
                out.push(format!(
                    "{path}: {id} is in no main spec and not in {FILE}: add it there, so the ID is never issued again (BUILD-073)"
                ));
            }
        }
    }
    out
}

/// The delta spec texts of the change archive under `root` (path relative to the root, text).
pub fn archived_specs(root: &std::path::Path) -> Vec<(String, String)> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "spec.md") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&root.join("openspec/changes/archive"), &mut files);
    files.sort();
    files
        .into_iter()
        .filter_map(|f| {
            let t = std::fs::read_to_string(&f).ok()?;
            Some((f.strip_prefix(root).unwrap_or(&f).display().to_string().replace('\\', "/"), t))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(main: &str, change: Option<&str>) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("viewer".to_string(), main.to_string());
        if let Some(c) = change {
            m.insert("change:x/viewer".to_string(), c.to_string());
        }
        m
    }

    // spec: BUILD-073, TEST-160
    #[test]
    fn the_file_s_form_reuse_and_the_archive_s_ids_are_checked() {
        let main = "### Requirement: VIEW-072 Kept\n";
        let archived = vec![(
            "openspec/changes/archive/a/specs/viewer/spec.md".to_string(),
            "## REMOVED Requirements\n### Requirement: VIEW-073 Gone\n### Requirement: VIEW-072 Kept\n".to_string(),
        )];
        // complete and well formed: no error
        assert!(errors(&specs(main, None), &archived, Some("# retired\nVIEW-073\n")).is_empty());
        // the archive's removed ID missing from the file
        let e = errors(&specs(main, None), &archived, Some("# none\n"));
        assert!(e.len() == 1 && e[0].contains("VIEW-073") && e[0].contains("archive/a"), "{e:?}");
        // a main spec or an open change's ADDED requirement reusing a retired ID; an open change's REMOVED heading is no reuse
        let e = errors(&specs("### Requirement: VIEW-073 Again\n### Requirement: VIEW-072 Kept\n", None), &archived, Some("VIEW-073\n"));
        assert!(e.iter().any(|x| x.contains("viewer: VIEW-073 reuses")), "{e:?}");
        let e = errors(&specs(main, Some("## ADDED Requirements\n### Requirement: VIEW-073 Back\n")), &archived, Some("VIEW-073\n"));
        assert!(e.iter().any(|x| x.contains("change:x/viewer: VIEW-073 reuses")), "{e:?}");
        assert!(errors(&specs(main, Some("## REMOVED Requirements\n### Requirement: VIEW-073 Gone\n")), &archived, Some("VIEW-073\n"))
            .is_empty());
        // the form: not an ID, twice, out of order (by number, not by text: VIEW-9 before VIEW-10), and a missing file
        let e = errors(&specs(main, None), &archived, Some("VIEW-073\nview-74\nVIEW-073\nGEN-1\n"));
        assert!(e.iter().any(|x| x.contains(":2:") && x.contains("not a requirement ID")), "{e:?}");
        assert!(e.iter().any(|x| x.contains(":3:") && x.contains("listed twice")), "{e:?}");
        assert!(e.iter().any(|x| x.contains(":4:") && x.contains("out of order")), "{e:?}");
        assert!(parse("VIEW-9\nVIEW-10\n").is_ok());
        // a rename that changes the ID retires the old one: its FROM heading counts as the archive's
        let renamed = vec![(
            "openspec/changes/archive/r/specs/viewer/spec.md".to_string(),
            "## RENAMED Requirements\n- FROM: `### Requirement: VIEW-070 Old name`\n- TO: `### Requirement: VIEW-072 Kept`\n".to_string(),
        )];
        let e = errors(&specs(main, None), &renamed, Some("# none\n"));
        assert!(e.len() == 1 && e[0].contains("VIEW-070"), "{e:?}");
        assert!(errors(&specs(main, None), &renamed, Some("VIEW-070\n")).is_empty());
        assert!(errors(&specs(main, None), &archived, None)[0].contains("missing"));
    }

    // spec: BUILD-073, TEST-160
    #[test]
    fn the_repository_s_retired_file_is_complete() {
        // the committed file against the committed specs and archive: what spec-lint runs, as a test
        let root = crate::util::repo_root();
        let retired = std::fs::read_to_string(root.join(FILE)).ok();
        let mut s = BTreeMap::new();
        let Ok(rd) = std::fs::read_dir(root.join("openspec/specs")) else { return };
        for e in rd.flatten() {
            if let Ok(t) = std::fs::read_to_string(e.path().join("spec.md")) {
                s.insert(e.file_name().to_string_lossy().into_owned(), t);
            }
        }
        let e = errors(&s, &archived_specs(&root), retired.as_deref());
        assert!(e.is_empty(), "{e:#?}");
    }
}
