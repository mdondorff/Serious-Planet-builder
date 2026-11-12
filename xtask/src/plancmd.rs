//! `cargo xtask plan` (BUILD-053, ADR 0120 point 5): the current milestone's exit items with their state, their queue rows in `docs/status.md` and the open PRs whose title names them.
//! The current milestone is the file of `docs/milestones/` whose `Status:` line starts with `active`; its exit table has the columns `#`, `item`, `done when`, `lane`, `state`. The queue's first column is the
//! exit item. A queue row whose Exit cell is not an exit ID of the milestone, and an open exit item (state not starting `done`) without a row, are errors (exit code 1).

use crate::util::{capture, Res};
use std::path::Path;

/// One exit item of a milestone's exit table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitItem {
    pub id: String,
    pub item: String,
    pub done_when: String,
    pub lane: String,
    pub state: String,
}

impl ExitItem {
    /// An item is open unless its state starts with `done`.
    pub fn is_open(&self) -> bool {
        !self.state.trim().to_lowercase().starts_with("done")
    }
}

/// The cells of a markdown table row, trimmed; `None` for a line that is not a row.
fn cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let inner = t.strip_prefix('|')?;
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    Some(inner.split('|').map(|c| c.trim().to_string()).collect())
}

/// Whether a row is the `|---|---|` separator.
fn is_separator(cells: &[String]) -> bool {
    !cells.is_empty() && cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// The value of the first `Status:` line of a milestone file (a leading `**` or `- ` is ignored).
pub fn milestone_status(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let t = l.trim().trim_start_matches("- ").trim_start_matches("**");
        let rest = t.strip_prefix("Status")?;
        let rest = rest.trim_start_matches("**").strip_prefix(':')?;
        Some(rest.trim().trim_start_matches("**").trim().to_string())
    })
}

/// Whether the milestone file is the current one: its `Status:` starts with `active` (case-insensitive).
pub fn is_active(text: &str) -> bool {
    milestone_status(text).is_some_and(|s| s.to_lowercase().starts_with("active"))
}

/// The exit table: the first table whose header holds the columns `#`, `item`, `done when`, `lane` and `state` (in any order, case-insensitive). Errors name what is missing.
pub fn parse_exit_table(text: &str) -> Result<Vec<ExitItem>, String> {
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(head) = cells(line) else { continue };
        let low: Vec<String> = head.iter().map(|c| c.to_lowercase()).collect();
        let col = |name: &str| low.iter().position(|c| c == name);
        let item_col = col("exit item").or_else(|| col("item"));
        let (Some(i), Some(it), Some(dw), Some(la), Some(st)) = (col("#"), item_col, col("done when"), col("lane"), col("state")) else {
            continue;
        };
        let mut items = Vec::new();
        if lines.peek().and_then(|l| cells(l)).is_some_and(|c| is_separator(&c)) {
            lines.next();
        }
        for row in lines.by_ref() {
            let Some(c) = cells(row) else { break };
            let get = |k: usize| c.get(k).cloned().unwrap_or_default();
            if get(i).is_empty() {
                continue;
            }
            items.push(ExitItem { id: get(i), item: get(it), done_when: get(dw), lane: get(la), state: get(st) });
        }
        if items.is_empty() {
            return Err("the exit table has no rows".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for it in &items {
            if !seen.insert(it.id.clone()) {
                return Err(format!("the exit ID `{}` appears twice in the exit table", it.id));
            }
        }
        return Ok(items);
    }
    Err("no exit table found: a table with the columns #, item, done when, lane, state".into())
}

/// A queue row of `docs/status.md`: the Exit cell (first column) and the row's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    pub exit: String,
    pub text: String,
}

/// The rows of the first table under the `## Queue` heading of `docs/status.md` (the header and the separator are skipped).
pub fn parse_queue(text: &str) -> Vec<QueueRow> {
    let mut rows = Vec::new();
    let mut in_queue = false;
    let mut header_seen = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            in_queue = line.starts_with("## Queue");
            header_seen = false;
            continue;
        }
        if !in_queue {
            continue;
        }
        let Some(c) = cells(line) else {
            if header_seen {
                break;
            }
            continue;
        };
        if !header_seen {
            header_seen = true;
            continue;
        }
        if is_separator(&c) || c.first().is_none_or(String::is_empty) {
            continue;
        }
        rows.push(QueueRow { exit: c[0].clone(), text: c[1..].join(" | ") });
    }
    rows
}

/// Whether `title` names the exit ID `id` as a whole token (`E5` in `E5: plan command`, not in `E55`).
pub fn names(title: &str, id: &str) -> bool {
    let tb = title.as_bytes();
    title.match_indices(id).any(|(at, _)| {
        let before_ok = at == 0 || !tb[at - 1].is_ascii_alphanumeric();
        let end = at + id.len();
        let after_ok = end >= tb.len() || !tb[end].is_ascii_alphanumeric();
        before_ok && after_ok
    })
}

/// The errors of BUILD-053: a queue row whose Exit cell is not an exit ID, and an open exit item without a row.
pub fn errors(items: &[ExitItem], rows: &[QueueRow]) -> Vec<String> {
    let mut errs = Vec::new();
    for r in rows {
        if !items.iter().any(|i| i.id == r.exit) {
            errs.push(format!("docs/status.md queue row `{}`: the Exit cell is not an exit ID of the milestone", r.exit));
        }
    }
    for i in items.iter().filter(|i| i.is_open()) {
        if !rows.iter().any(|r| r.exit == i.id) {
            errs.push(format!(
                "exit item {} ({}, state `{}`) is open and has no row in the queue of docs/status.md",
                i.id, i.item, i.state
            ));
        }
    }
    errs
}

/// The printed plan: per exit item its state, lane, done-when, queue rows and open PRs naming it.
pub fn render(milestone: &str, items: &[ExitItem], rows: &[QueueRow], prs: &[(u32, String)]) -> String {
    let mut out = format!("milestone {milestone}: {} exit items, {} open\n", items.len(), items.iter().filter(|i| i.is_open()).count());
    for i in items {
        out.push_str(&format!("{} {} [{}] lane {}\n  done when: {}\n", i.id, i.item, i.state, i.lane, i.done_when));
        let mine: Vec<&QueueRow> = rows.iter().filter(|r| r.exit == i.id).collect();
        if mine.is_empty() {
            out.push_str("  queue: -\n");
        }
        for r in mine {
            out.push_str(&format!("  queue: {}\n", r.text));
        }
        let open: Vec<&(u32, String)> = prs.iter().filter(|(_, t)| names(t, &i.id)).collect();
        if open.is_empty() {
            out.push_str("  open PRs: -\n");
        }
        for (n, t) in open {
            out.push_str(&format!("  open PR: #{n} {t}\n"));
        }
    }
    out
}

/// The open PRs as `(number, title)` from `gh`; empty with a note on stderr when `gh` is not usable.
fn open_prs(root: &Path) -> Vec<(u32, String)> {
    let jq = ".[] | \"\\(.number)\\t\\(.title)\"";
    match capture(root, "gh", &["pr", "list", "--state", "open", "--limit", "200", "--json", "number,title", "--jq", jq]) {
        Ok(s) => s
            .lines()
            .filter_map(|l| {
                let (n, t) = l.split_once('\t')?;
                Some((n.trim().parse().ok()?, t.trim().to_string()))
            })
            .collect(),
        Err(e) => {
            eprintln!("plan: open PRs not read ({e})");
            Vec::new()
        }
    }
}

/// The current milestone: the name and the text of the file of `docs/milestones/` whose `Status:` starts with `active`.
pub fn current_milestone(root: &Path) -> Res<(String, String)> {
    let dir = root.join("docs").join("milestones");
    let mut files = Vec::new();
    for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "md") {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            files.push((p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), text));
        }
    }
    pick_active(files)
}

/// The one active milestone among `(name, text)` files; an error for none or for several.
pub fn pick_active(files: Vec<(String, String)>) -> Res<(String, String)> {
    let mut active: Vec<(String, String)> = files.into_iter().filter(|(_, t)| is_active(t)).collect();
    match active.len() {
        1 => Ok(active.remove(0)),
        0 => Err("no milestone file in docs/milestones/ has a Status line starting `active`".into()),
        n => Err(format!(
            "{n} milestone files have a Status line starting `active`: {}",
            active.iter().map(|a| a.0.clone()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// `cargo xtask plan`: prints the plan; an `Err` (exit code 1) when a queue row or an open item breaks BUILD-053.
pub fn run(root: &Path) -> Res {
    let (name, text) = current_milestone(root)?;
    let items = parse_exit_table(&text).map_err(|e| format!("docs/milestones/{name}.md: {e}"))?;
    let status = std::fs::read_to_string(root.join("docs").join("status.md")).map_err(|e| e.to_string())?;
    let rows = parse_queue(&status);
    let prs = open_prs(root);
    print!("{}", render(&name, &items, &rows, &prs));
    let errs = errors(&items, &rows);
    if errs.is_empty() {
        Ok(())
    } else {
        Err(format!("plan: {} problem(s):\n  {}", errs.len(), errs.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MILESTONE: &str = "# P1\nStatus: active (to 11-20)\n\n## Exit table\n| # | Item | Done when | Lane | State |\n|---|---|---|---|---|\n| E1 | G-LOCAL | gate row passes | spikes | open |\n| E5 | plan command | on main | explorer | in progress |\n| E7 | f32 rounds | k=4 pair | explorer | done 2026-11-12 |\n\nText after.\n";
    const STATUS: &str = "# Status\n\n## Queue (2026-11-12)\n| Exit | Step | Lane |\n|---|---|---|\n| E1 | gate re-measure | spikes |\n| E5 | xtask plan | explorer |\n\n## Tracked\n| x | y |\n";

    // spec: BUILD-053
    #[test]
    fn the_exit_table_and_the_status_line_are_read() {
        assert!(is_active(MILESTONE) && !is_active("# M9\nStatus: closed 2026-11-01\n"));
        let items = parse_exit_table(MILESTONE).unwrap();
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["E1", "E5", "E7"]);
        assert_eq!((items[1].lane.as_str(), items[1].state.as_str()), ("explorer", "in progress"));
        assert!(items[0].is_open() && items[1].is_open() && !items[2].is_open());
        assert!(parse_exit_table("# x\nno table\n").unwrap_err().contains("no exit table"));
        assert!(parse_exit_table("| # | Item |\n|---|---|\n| E1 | a |\n").unwrap_err().contains("no exit table"));
    }

    // spec: BUILD-053
    #[test]
    fn the_queue_rows_are_the_first_table_under_the_queue_heading() {
        let rows = parse_queue(STATUS);
        assert_eq!(rows.iter().map(|r| r.exit.as_str()).collect::<Vec<_>>(), ["E1", "E5"]);
        assert!(rows[1].text.contains("xtask plan"));
    }

    // spec: BUILD-053
    #[test]
    fn a_row_off_the_milestone_and_an_open_item_without_a_row_are_errors() {
        let items = parse_exit_table(MILESTONE).unwrap();
        assert!(errors(&items, &parse_queue(STATUS)).is_empty(), "E7 is done and needs no row");
        let bad = "## Queue\n| Exit | Step |\n|---|---|\n| E1 | a |\n| P0 (to 11-13) | finished work |\n";
        let e = errors(&items, &parse_queue(bad));
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e.iter().any(|m| m.contains("P0 (to 11-13)") && m.contains("not an exit ID")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("E5") && m.contains("no row")), "{e:?}");
    }

    // spec: BUILD-053
    #[test]
    fn the_plan_prints_each_item_with_its_rows_and_the_prs_that_name_it() {
        let items = parse_exit_table(MILESTONE).unwrap();
        let prs = vec![(1500, "E5: xtask plan".to_string()), (1501, "E55: other".to_string()), (1502, "docs".to_string())];
        let out = render("P1", &items, &parse_queue(STATUS), &prs);
        assert!(out.contains("3 exit items, 2 open"), "{out}");
        assert!(out.contains("E5 plan command [in progress] lane explorer"), "{out}");
        assert!(out.contains("open PR: #1500 E5: xtask plan") && !out.contains("#1501") && !out.contains("#1502"), "{out}");
        assert!(out.contains("queue: xtask plan | explorer"), "{out}");
        assert!(names("E5: x", "E5") && names("fix (E5)", "E5") && !names("E55", "E5") && !names("XE5", "E5"));
    }

    // spec: BUILD-053
    #[test]
    fn the_real_header_of_a_milestone_file_is_read() {
        let p1 = "# P1\n\nStatus: active (owner)\n\n## Exit items\n| # | Exit item | Done when | Lane | State |\n|---|---|---|---|---|\n| E1 | A flight | `x` | explorer | open |\n| E3 | Memory | y | explorer | done (#1447) |\n";
        let items = parse_exit_table(p1).unwrap();
        assert_eq!(
            items.iter().map(|i| (i.id.as_str(), i.item.as_str(), i.is_open())).collect::<Vec<_>>(),
            [("E1", "A flight", true), ("E3", "Memory", false)]
        );
    }

    // spec: BUILD-053
    #[test]
    fn no_active_milestone_and_several_active_milestones_are_errors() {
        let file = |n: &str, s: &str| (n.to_string(), format!("# {n}\nStatus: {s}\n"));
        assert!(pick_active(vec![file("M9", "closed 2026-11-01")]).unwrap_err().contains("no milestone file"));
        assert!(pick_active(vec![]).unwrap_err().contains("no milestone file"));
        let two = pick_active(vec![file("P1", "active"), file("P2", "active (next)")]).unwrap_err();
        assert!(two.contains("2 milestone files") && two.contains("P1") && two.contains("P2"), "{two}");
        assert_eq!(pick_active(vec![file("M9", "closed"), file("P1", "active")]).unwrap().0, "P1");
    }

    // spec: BUILD-053
    #[test]
    fn a_duplicate_exit_id_is_refused() {
        let dup = "Status: active\n| # | Item | Done when | Lane | State |\n|--|--|--|--|--|\n| E1 | a | b | c | open |\n| E1 | a | b | c | open |\n";
        assert!(parse_exit_table(dup).unwrap_err().contains("twice"));
    }
}
