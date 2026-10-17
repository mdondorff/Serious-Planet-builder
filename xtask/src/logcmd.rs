//! `cargo xtask log`: the project log is `docs/log.md` (history) plus one fragment per change in `docs/log.d/` (BUILD-010).
//! A fragment holds exactly one entry, `- YYYY-MM-DD **title** ...`, in a file named `YYYY-MM-DD-<id>.md`, so parallel
//! writers never touch the same file. The command prints both, merged newest first.

use crate::util::Res;
use std::path::Path;

/// Where the fragments live, relative to the repository root.
pub const FRAGMENT_DIR: &str = "docs/log.d";

/// One log entry with its sort key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub date: String,
    /// The fragment's file name, or `None` for a line of `docs/log.md`.
    pub file: Option<String>,
    pub text: String,
}

/// `YYYY-MM-DD` at the start of `s` (digits and dashes in place, a plausible month and day); returns the date.
fn leading_date(s: &str) -> Option<&str> {
    let d = s.get(..10)?;
    let b = d.as_bytes();
    let ok = b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
    let month: u32 = d[5..7].parse().ok()?;
    let day: u32 = d[8..10].parse().ok()?;
    (ok && (1..=12).contains(&month) && (1..=31).contains(&day)).then_some(d)
}

/// The date of an entry line `- YYYY-MM-DD ...`.
fn entry_date(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- ")?;
    let d = leading_date(rest)?;
    (rest.len() == 10 || rest.as_bytes()[10] == b' ').then_some(d)
}

/// BUILD-010: the reason a fragment is malformed, or the entry it holds.
pub fn parse_fragment(name: &str, text: &str) -> Result<Entry, String> {
    let stem = name.strip_suffix(".md").ok_or("the name must end in `.md`")?;
    let name_date = leading_date(stem).ok_or("the name must start with `YYYY-MM-DD-`")?;
    let id = stem[10..].strip_prefix('-').ok_or("the name must be `YYYY-MM-DD-<id>.md`")?;
    let id_ok = !id.is_empty()
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'));
    if !id_ok {
        return Err(format!("the id `{id}` must be lower-case letters, digits, `-`, `_` or `.`, starting with a letter or digit"));
    }
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let [line] = lines.as_slice() else {
        return Err(format!("expected exactly one entry line, found {} non-blank lines", lines.len()));
    };
    let date = entry_date(line).ok_or("the line must start with `- YYYY-MM-DD `")?;
    if date != name_date {
        return Err(format!("the entry date {date} differs from the file-name date {name_date}"));
    }
    Ok(Entry { date: date.to_string(), file: Some(name.to_string()), text: line.to_string() })
}

/// The entries of `docs/log.md`: the lines `- YYYY-MM-DD ...`, in file order.
pub fn parse_log(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_end();
            Some(Entry { date: entry_date(l)?.to_string(), file: None, text: l.to_string() })
        })
        .collect()
}

/// Newest first: date descending, then fragments (by file name descending) before `log.md` lines (in file order).
pub fn merge(fragments: Vec<Entry>, log: Vec<Entry>) -> Vec<Entry> {
    let mut all: Vec<(Entry, usize)> = fragments.into_iter().chain(log).zip(0..).collect();
    all.sort_by(|(a, ia), (b, ib)| {
        b.date.cmp(&a.date).then_with(|| match (&a.file, &b.file) {
            (Some(x), Some(y)) => y.cmp(x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => ia.cmp(ib),
        })
    });
    all.into_iter().map(|(e, _)| e).collect()
}

/// BUILD-010: every fragment in `files` (name, text) that is malformed, as `docs/log.d/<name>: <reason>`.
pub fn fragment_errors(files: &[(String, String)]) -> Vec<String> {
    files
        .iter()
        .filter_map(|(name, text)| parse_fragment(name, text).err().map(|e| format!("{FRAGMENT_DIR}/{name}: {e} (BUILD-010)")))
        .collect()
}

/// The files of `docs/log.d/` (name, text); a missing folder is empty.
pub fn read_fragments(root: &Path) -> Res<Vec<(String, String)>> {
    let dir = root.join(FRAGMENT_DIR);
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for e in rd.flatten().filter(|e| e.path().is_file()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == ".gitkeep" || name == "README.md" {
            continue;
        }
        let text = std::fs::read_to_string(e.path()).map_err(|err| format!("{}: {err}", e.path().display()))?;
        out.push((name, text));
    }
    out.sort();
    Ok(out)
}

/// `cargo xtask log [--since YYYY-MM-DD] [--limit N]`.
pub fn run(root: &Path, args: &[String]) -> Res {
    let mut since: Option<String> = None;
    let mut limit = usize::MAX;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--since" => {
                let v = it.next().ok_or("--since needs a date")?;
                since =
                    Some(leading_date(v).filter(|d| d.len() == v.len()).ok_or(format!("--since: `{v}` is not YYYY-MM-DD"))?.to_string());
            }
            "--limit" => limit = it.next().and_then(|v| v.parse().ok()).ok_or("--limit needs a number")?,
            other => return Err(format!("log: unknown option `{other}` (usage: cargo xtask log [--since YYYY-MM-DD] [--limit N])")),
        }
    }
    let mut fragments = Vec::new();
    for (name, text) in read_fragments(root)? {
        fragments.push(parse_fragment(&name, &text).map_err(|e| format!("{FRAGMENT_DIR}/{name}: {e}"))?);
    }
    let log = std::fs::read_to_string(root.join("docs").join("log.md")).map_err(|e| format!("docs/log.md: {e}"))?;
    let mut out = String::new();
    for e in merge(fragments, parse_log(&log)).iter().filter(|e| since.as_ref().is_none_or(|s| e.date >= *s)).take(limit) {
        out.push_str(&e.text);
        out.push('\n');
    }
    print!("{out}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag(name: &str, line: &str) -> Entry {
        parse_fragment(name, &format!("{line}\n")).unwrap()
    }

    // spec: BUILD-010
    #[test]
    fn a_well_formed_fragment_is_accepted() {
        let e = frag("2026-10-17-log-fragments.md", "- 2026-10-17 **log-fragments archived:** one file per change.");
        assert_eq!(e.date, "2026-10-17");
        assert!(fragment_errors(&[("2026-10-17-a.md".into(), "- 2026-10-17 **a**\r\n\r\n".into())]).is_empty());
    }

    // spec: BUILD-010
    #[test]
    fn a_malformed_fragment_is_rejected_with_its_name_and_reason() {
        let files = [
            ("2026-10-17-two.md".to_string(), "- 2026-10-17 **a**\n- 2026-10-17 **b**\n".to_string()),
            ("2026-10-17-none.md".to_string(), "\n".to_string()),
            ("2026-10-18-date.md".to_string(), "- 2026-10-17 **a**\n".to_string()),
            ("notes.md".to_string(), "- 2026-10-17 **a**\n".to_string()),
            ("2026-10-17-Upper.md".to_string(), "- 2026-10-17 **a**\n".to_string()),
            ("2026-10-17.md".to_string(), "- 2026-10-17 **a**\n".to_string()),
            ("2026-10-17-x.txt".to_string(), "- 2026-10-17 **a**\n".to_string()),
            ("2026-10-17-bullet.md".to_string(), "2026-10-17 **a**\n".to_string()),
            ("2026-10-17-prose.md".to_string(), "- 2026-10-17**a**\n".to_string()),
        ];
        let errs = fragment_errors(&files);
        assert_eq!(errs.len(), files.len(), "{errs:#?}");
        for ((name, _), e) in files.iter().zip(&errs) {
            assert!(e.starts_with(&format!("docs/log.d/{name}: ")), "{e}");
        }
        assert!(errs[0].contains("exactly one"), "{}", errs[0]);
        assert!(errs[2].contains("differs"), "{}", errs[2]);
    }

    // spec: BUILD-010
    #[test]
    fn merged_output_is_newest_first_and_deterministic() {
        let log = parse_log("# Log\n\nintro\n\n- 2026-10-17 **old1**\n- 2026-10-16 **old2**\n- 2026-10-17 **old3**\n");
        let fragments = vec![frag("2026-10-18-a.md", "- 2026-10-18 **new**"), frag("2026-10-17-z.md", "- 2026-10-17 **frag**")];
        let order = |f: Vec<Entry>, l: Vec<Entry>| merge(f, l).into_iter().map(|e| e.text).collect::<Vec<_>>();
        let got = order(fragments.clone(), log.clone());
        assert_eq!(
            got,
            ["- 2026-10-18 **new**", "- 2026-10-17 **frag**", "- 2026-10-17 **old1**", "- 2026-10-17 **old3**", "- 2026-10-16 **old2**"]
        );
        let mut rev = fragments;
        rev.reverse();
        assert_eq!(order(rev, log), got);
    }

    // spec: BUILD-010
    #[test]
    fn fragments_of_one_day_sort_by_file_name_descending() {
        let f = vec![
            frag("2026-10-17-a.md", "- 2026-10-17 **a**"),
            frag("2026-10-17-c.md", "- 2026-10-17 **c**"),
            frag("2026-10-17-b.md", "- 2026-10-17 **b**"),
        ];
        let got: Vec<_> = merge(f, Vec::new()).into_iter().map(|e| e.file.unwrap()).collect();
        assert_eq!(got, ["2026-10-17-c.md", "2026-10-17-b.md", "2026-10-17-a.md"]);
    }
}
