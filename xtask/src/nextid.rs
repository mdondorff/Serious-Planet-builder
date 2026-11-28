//! `cargo xtask next-id` (BUILD-062): pure-std allocator for the next contiguous `max + 1` requirement, ADR, or spike ID
//! across `openspec/`, `docs/adr/`, `docs/spikes.md`, and optional sibling lane files. Requirement IDs come from `openspec/` without the change
//! archive, plus the retired IDs of `openspec/retired-ids.txt` (BUILD-073), so deleting the archive issues no ID again.

use crate::heavy;
use crate::util::{capture, Res};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Requirement,
    Adr,
    Spike,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextId {
    pub formatted: String,
    pub max_found: u32,
    pub source: Option<String>,
}

fn scan_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            // BUILD-073: the change archive is not read; its retired IDs are in openspec/retired-ids.txt
            if p.file_name().is_some_and(|n| n == "archive") && p.parent().is_some_and(|d| d.file_name().is_some_and(|n| n == "changes")) {
                continue;
            }
            scan_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

fn max_u32_after_prefix(text: &str, prefix_with_sep: &str, kind: IdKind) -> Option<(u32, usize)> {
    let mut best: Option<(u32, usize)> = None;
    let mut pos = 0;
    let bytes = text.as_bytes();
    while let Some(rel) = text[pos..].find(prefix_with_sep) {
        let at = pos + rel;
        let left_ok = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let num_start = at + prefix_with_sep.len();
        if left_ok && num_start < bytes.len() {
            let digits: String = text[num_start..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let after_num = num_start + digits.len();
            let right_ok = after_num == bytes.len() || !bytes[after_num].is_ascii_alphanumeric();
            if !digits.is_empty() && right_ok {
                if let Ok(val) = digits.parse::<u32>() {
                    let skip_adr_false = kind == IdKind::Adr && (digits.len() != 4 || (2020..=2099).contains(&val));
                    if !skip_adr_false && best.is_none_or(|(b, _)| val > b) {
                        best = Some((val, digits.len()));
                    }
                }
            }
        }
        pos = num_start;
    }
    best
}

/// Compute `max + 1` over `root` (and optional `lanes_dir` / extra open PR text) for `raw_prefix` (`VIEW`, `BUILD`, `ADR`, `SPIKE`, ...).
pub fn next_id(root: &Path, raw_prefix: &str, lanes_dir: Option<&Path>, extra_text: &str) -> Result<NextId, String> {
    let upper = raw_prefix.trim().to_ascii_uppercase();
    if upper.is_empty() || !upper.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(format!("invalid prefix '{raw_prefix}': expected uppercase letters like VIEW, BUILD, ADR, or SPIKE"));
    }

    let kind = match upper.as_str() {
        "ADR" => IdKind::Adr,
        "SPIKE" => IdKind::Spike,
        _ => IdKind::Requirement,
    };

    let mut files = Vec::new();
    match kind {
        IdKind::Adr => {
            scan_files(&root.join("docs/adr"), &mut files);
            let status = root.join("docs/status.md");
            if status.is_file() {
                files.push(status);
            }
            // The active sprint file (ADR 0130), which may name a reserved ADR before it exists.
            if let Some(name) = crate::prbody::active_sprint(&root) {
                let p = root.join("docs/milestones").join(format!("{name}.md"));
                if p.is_file() {
                    files.push(p);
                }
            }
        }
        IdKind::Spike => {
            for p in [root.join("docs/spikes.md"), root.join("spikes/README.md"), root.join("docs/status.md")] {
                if p.is_file() {
                    files.push(p);
                }
            }
        }
        IdKind::Requirement => {
            scan_files(&root.join("openspec"), &mut files);
            let retired = root.join(crate::retiredids::FILE);
            if retired.is_file() {
                files.push(retired);
            }
        }
    }
    if let Some(ld) = lanes_dir {
        scan_files(ld, &mut files);
    }

    let mut max_val = 0u32;
    let mut width = match kind {
        IdKind::Adr => 4,
        IdKind::Spike => 3,
        IdKind::Requirement => 3,
    };
    let mut source: Option<String> = None;

    #[allow(clippy::too_many_arguments)]
    fn consider_text(
        kind: IdKind,
        upper: &str,
        text: &str,
        label: &str,
        max_val: &mut u32,
        width: &mut usize,
        source: &mut Option<String>,
    ) {
        let needles: Vec<String> = match kind {
            IdKind::Adr => vec!["ADR ".into(), "ADR-".into()],
            IdKind::Spike => vec!["SPIKE-".into()],
            IdKind::Requirement => vec![format!("{upper}-")],
        };
        for needle in &needles {
            if let Some((v, w)) = max_u32_after_prefix(text, needle, kind) {
                if v > *max_val {
                    *max_val = v;
                    *width = (*width).max(w);
                    *source = Some(label.to_string());
                }
            }
        }
    }

    let adr_dir = root.join("docs/adr");
    for f in &files {
        if kind == IdKind::Adr && f.parent() == Some(adr_dir.as_path()) {
            if let Some(name) = f.file_name().and_then(|s| s.to_str()) {
                let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.len() == 4 {
                    if let Ok(v) = digits.parse::<u32>() {
                        if v > max_val {
                            max_val = v;
                            width = width.max(digits.len());
                            source = Some(f.strip_prefix(root).unwrap_or(f).display().to_string());
                        }
                    }
                }
            }
        }
        if let Ok(txt) = fs::read_to_string(f) {
            let label = f.strip_prefix(root).unwrap_or(f).display().to_string();
            consider_text(kind, &upper, &txt, &label, &mut max_val, &mut width, &mut source);
        }
    }
    if !extra_text.is_empty() {
        consider_text(kind, &upper, extra_text, "open-prs", &mut max_val, &mut width, &mut source);
    }

    let next = max_val + 1;
    let formatted = match kind {
        IdKind::Adr => format!("{next:0width$}"),
        IdKind::Spike => format!("SPIKE-{next:0width$}"),
        IdKind::Requirement => format!("{upper}-{next:0width$}"),
    };
    Ok(NextId { formatted, max_found: max_val, source })
}

fn default_lanes_dir(root: &Path) -> Option<PathBuf> {
    if let Ok(v) = std::env::var("PLANET_INBOX_DIR") {
        let p = PathBuf::from(v);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(heavy_dir) = heavy::state_dir() {
        if let Some(data_root) = heavy_dir.parent().and_then(|p| p.parent()) {
            let lanes = data_root.join("lanes");
            if lanes.is_dir() {
                return Some(lanes);
            }
        }
    }
    root.ancestors().map(|a| a.join("SeriousPlanetData").join("lanes")).find(|p| p.is_dir())
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let mut prefix: Option<String> = None;
    let mut lanes_dir: Option<PathBuf> = None;
    let mut query_gh = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lanes-dir" => {
                let d = it.next().ok_or("--lanes-dir requires a path")?;
                lanes_dir = Some(PathBuf::from(d));
            }
            "--no-lanes" => lanes_dir = Some(PathBuf::new()),
            "--gh" => query_gh = true,
            other if !other.starts_with('-') && prefix.is_none() => prefix = Some(other.to_string()),
            other => {
                return Err(format!("unknown argument '{other}' (usage: cargo xtask next-id <PREFIX|ADR|SPIKE> [--lanes-dir DIR] [--gh])"))
            }
        }
    }
    let prefix = prefix.ok_or("missing prefix (usage: cargo xtask next-id <PREFIX|ADR|SPIKE> [--lanes-dir DIR] [--gh])")?;
    let lanes = match lanes_dir {
        Some(p) if p.as_os_str().is_empty() => None,
        Some(p) => Some(p),
        None => default_lanes_dir(root),
    };
    let extra = if query_gh {
        capture(root, "gh", &["pr", "list", "--state", "open", "--limit", "200", "--json", "title,body"]).unwrap_or_default()
    } else {
        String::new()
    };
    let res = next_id(root, &prefix, lanes.as_deref(), &extra)?;
    match res.source {
        Some(src) => println!("{} (next after {} in {src})", res.formatted, res.max_found),
        None => println!("{} (no prior entries found)", res.formatted),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-062
    #[test]
    fn next_id_finds_contiguous_max_plus_one_across_specs_lanes_and_prs() {
        let base = std::env::temp_dir().join(format!("xtask-nextid-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("openspec/specs/viewer")).unwrap();
        fs::create_dir_all(base.join("openspec/changes/archive/old/specs/viewer")).unwrap();
        fs::create_dir_all(base.join("docs/adr")).unwrap();
        fs::create_dir_all(base.join("lanes")).unwrap();

        fs::write(base.join("openspec/specs/viewer/spec.md"), "### Requirement: VIEW-072 X\n").unwrap();
        fs::write(base.join("openspec/changes/archive/old/specs/viewer/spec.md"), "### Requirement: VIEW-073 Retired\n").unwrap();
        fs::write(base.join("docs/adr/0122-x.md"), "# 0122 X\n").unwrap();
        fs::write(base.join("lanes/digest.md"), "Proposed ADR 0127 and VIEW-074 draft").unwrap();
        fs::write(base.join("lanes/2026-11-13-preview.md"), "Maintained ADR 2026-10-23 note and ADR 0125").unwrap();

        let v = next_id(&base, "view", Some(&base.join("lanes")), "open PR VIEW-075").unwrap();
        assert_eq!(v.formatted, "VIEW-076");
        assert_eq!(v.max_found, 75);

        let adr = next_id(&base, "adr", Some(&base.join("lanes")), "").unwrap();
        assert_eq!(adr.formatted, "0128");
        assert_eq!(adr.max_found, 127);

        assert!(next_id(&base, "bad-1", None, "").is_err());
        let _ = fs::remove_dir_all(&base);
    }

    // spec: BUILD-073, TEST-160
    #[test]
    fn next_id_counts_the_retired_file_and_not_the_change_archive() {
        let base = std::env::temp_dir().join(format!("xtask-nextid-retired-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("openspec/specs/viewer")).unwrap();
        fs::create_dir_all(base.join("openspec/changes/archive/old/specs/viewer")).unwrap();
        fs::write(
            base.join("openspec/specs/viewer/spec.md"),
            "### Requirement: VIEW-072 X
",
        )
        .unwrap();
        fs::write(
            base.join("openspec/retired-ids.txt"),
            "# retired
VIEW-080
",
        )
        .unwrap();
        // the archive's VIEW-090 no longer counts (the archive may be deleted); the retired VIEW-080 is never issued again
        fs::write(
            base.join("openspec/changes/archive/old/specs/viewer/spec.md"),
            "### Requirement: VIEW-090 Archived
",
        )
        .unwrap();
        let v = next_id(&base, "view", None, "").unwrap();
        assert_eq!(v.formatted, "VIEW-081");
        assert_eq!(v.source.as_deref().map(|s| s.replace('\\', "/")), Some("openspec/retired-ids.txt".to_string()));
        // without the file, the main spec alone: VIEW-073 (the retired VIEW-080 would be issued again, which the file prevents)
        fs::remove_file(base.join("openspec/retired-ids.txt")).unwrap();
        assert_eq!(next_id(&base, "view", None, "").unwrap().formatted, "VIEW-073");
        let _ = fs::remove_dir_all(&base);
    }

    // spec: BUILD-062
    #[test]
    fn next_id_reads_the_active_sprint_file_and_not_a_closed_milestone_file() {
        let base = std::env::temp_dir().join(format!("xtask-nextid-sprint-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("docs/adr")).unwrap();
        fs::create_dir_all(base.join("docs/milestones")).unwrap();
        fs::write(base.join("docs/adr/0122-x.md"), "# 0122 X\n").unwrap();
        fs::write(base.join("docs/milestones/P3.md"), "# P3\nStatus: closed (owner)\nADR 0140 reserved\n").unwrap();
        assert_eq!(next_id(&base, "adr", None, "").unwrap().formatted, "0123", "a closed milestone file is not read");
        let sprint = include_str!("fixtures/sprint-good.md").to_string() + "\nADR 0131 reserved for E1\n";
        fs::write(base.join("docs/milestones/S-Test.md"), sprint).unwrap();
        assert_eq!(next_id(&base, "adr", None, "").unwrap().formatted, "0132");
        let _ = fs::remove_dir_all(&base);
    }
}
