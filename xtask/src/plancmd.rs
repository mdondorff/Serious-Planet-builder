//! `cargo xtask plan` (BUILD-053, ADR 0120 point 5): the current milestone's exit items with their state, their queue rows in `docs/status.md` and the open PRs whose title names them.
//! The current milestone is the file of `docs/milestones/` whose `Status:` line starts with `active`; its exit table has the columns `#`, `item`, `done when`, `lane`, `state`. The queue's first column is the
//! exit item. A queue row whose Exit cell is not an exit ID of the milestone, and an open exit item (state not starting `done`) without a row, are errors (exit code 1).

use crate::sprintfile;
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

/// The milestone files of `docs/milestones/` as `(name, text)`.
fn milestone_files(root: &Path) -> Res<Vec<(String, String)>> {
    let dir = root.join("docs").join("milestones");
    let mut files = Vec::new();
    for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "md") {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            files.push((p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), text));
        }
    }
    files.sort();
    Ok(files)
}

/// Whether a milestone file is a draft sprint (BUILD-074): its `Status:` starts with `draft`.
pub fn is_draft(text: &str) -> bool {
    milestone_status(text).is_some_and(|s| s.to_lowercase().starts_with("draft"))
}

/// The milestones `plan` reads: the active one and the next sprint's draft, each `(name, text)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub active: Option<(String, String)>,
    pub draft: Option<(String, String)>,
}

/// One active file, one active file and one draft, or one draft alone (between sprints, before the owner's `decision: open`); an error for
/// none, two actives or two drafts (BUILD-053, BUILD-074).
pub fn pick_current(files: Vec<(String, String)>) -> Res<Current> {
    let names = |v: &[(String, String)]| v.iter().map(|a| a.0.clone()).collect::<Vec<_>>().join(", ");
    let (mut active, rest): (Vec<_>, Vec<_>) = files.into_iter().partition(|(_, t)| is_active(t));
    let mut drafts: Vec<(String, String)> = rest.into_iter().filter(|(_, t)| is_draft(t)).collect();
    if active.len() > 1 {
        return Err(format!("{} milestone files have a Status line starting `active`: {}", active.len(), names(&active)));
    }
    if drafts.len() > 1 {
        return Err(format!("{} milestone files have a Status line starting `draft`: {}", drafts.len(), names(&drafts)));
    }
    if active.is_empty() && drafts.is_empty() {
        return Err("no milestone file in docs/milestones/ has a Status line starting `active` (or `draft`, between sprints)".into());
    }
    Ok(Current { active: active.pop(), draft: drafts.pop() })
}

/// The one active milestone among `(name, text)` files; an error for none or for several.
#[cfg(test)]
pub fn pick_active(files: Vec<(String, String)>) -> Res<(String, String)> {
    pick_current(files)?.active.ok_or_else(|| "no milestone file in docs/milestones/ has a Status line starting `active`".into())
}

/// The queue errors of a sprint file: a queue row whose Exit cell is not an exit ID, and an open exit without a row.
fn sprint_queue_errors(s: &sprintfile::Sprint, rows: &[QueueRow]) -> Vec<String> {
    let items: Vec<ExitItem> = s
        .exits
        .iter()
        .map(|x| ExitItem {
            id: x.id.clone(),
            item: x.item.clone(),
            done_when: x.done_when.clone(),
            lane: x.lane.clone(),
            // BUILD-053 calls an item open unless its state starts `done`: promoted, parked and carried rows are closed too
            state: if x.is_open() { x.state.clone() } else { format!("done ({})", x.state) },
        })
        .collect();
    errors(&items, rows)
}

/// The checks of one sprint file (BUILD-074), with its records read from the repository.
fn sprint_errors(root: &Path, name: &str, text: &str) -> (Option<sprintfile::Sprint>, Vec<String>) {
    match sprintfile::parse(text) {
        Ok(s) => {
            let read = |r: &str| std::fs::read_to_string(sprintfile::record_path(root, name, r)).ok();
            let e = sprintfile::errors(&s, name, &read);
            (Some(s), e)
        }
        Err(e) => (None, vec![format!("docs/milestones/{name}.md: {e}")]),
    }
}

/// The plan's text and its problems (BUILD-053, BUILD-074), on `today`; `prs` is the open PR list for the old form.
fn plan(root: &Path, cur: &Current, queue: &[QueueRow], prs: &[(u32, String)], today: i64) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut errs = Vec::new();
    match &cur.active {
        Some((name, text)) if sprintfile::is_sprint_file(text) => {
            let (s, e) = sprint_errors(root, name, text);
            errs.extend(e);
            if let Some(s) = s {
                out.push_str(&sprintfile::render(name, &s, today));
                errs.extend(sprint_queue_errors(&s, queue));
            }
        }
        Some((name, text)) => match parse_exit_table(text) {
            Ok(items) => {
                out.push_str(&render(name, &items, queue, prs));
                errs.extend(errors(&items, queue));
            }
            Err(e) => errs.push(format!("docs/milestones/{name}.md: {e}")),
        },
        None => {
            if let Some((name, _)) = &cur.draft {
                out.push_str(&format!("no active sprint; {name} waits for the owner's decision: open\n"));
                for r in queue {
                    errs.push(format!(
                        "docs/status.md queue row `{}`: no sprint is open ({name} is a draft), so the queue must be empty",
                        r.exit
                    ));
                }
            }
        }
    }
    if let Some((name, text)) = &cur.draft {
        if cur.active.is_some() {
            out.push_str(&format!("next sprint: {name} (draft)\n"));
        }
        if sprintfile::is_sprint_file(text) {
            errs.extend(sprint_errors(root, name, text).1);
        } else {
            errs.push(format!("docs/milestones/{name}.md: a draft is a sprint file (an `Opened:` line, BUILD-074)"));
        }
    }
    (out, errs)
}

/// The problems of BUILD-053 and BUILD-074 for spec-lint: the active milestone (and a draft), its tables and the queue of `docs/status.md`, without the PR list.
pub fn lint_errors(root: &Path) -> Vec<String> {
    let cur = match milestone_files(root).and_then(pick_current) {
        Ok(c) => c,
        Err(e) => return vec![format!("plan: {e}")],
    };
    let status = match std::fs::read_to_string(root.join("docs").join("status.md")) {
        Ok(s) => s,
        Err(e) => return vec![format!("plan: docs/status.md: {e}")],
    };
    plan(root, &cur, &parse_queue(&status), &[], sprintfile::today()).1.into_iter().map(|e| format!("plan: {e}")).collect()
}

/// The exit IDs a PR body names on `Exit:` lines (`Exit: E2`, `Exit: E2, E3`), joined for [`sprintfile::pr_errors`]'s title match.
pub fn body_exits(body: &str) -> String {
    body.lines().filter_map(|l| l.trim().strip_prefix("Exit:")).map(str::trim).collect::<Vec<_>>().join(" ")
}

/// The arguments of `--pr`: the title (plus the exits a `--body-file` names on `Exit:` lines), the branch and the changed paths (given, or one per line of `--files`).
pub fn parse_pr_args(args: &[String], read: &dyn Fn(&str) -> Res<String>) -> Res<(String, Option<String>, Vec<String>)> {
    let mut it = args.iter();
    let mut title = it.next().ok_or("--pr needs the PR title")?.clone();
    let mut branch = None;
    let mut paths = Vec::new();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--branch" => branch = Some(it.next().ok_or("--branch needs a name")?.clone()),
            "--body-file" => {
                let exits = body_exits(&read(it.next().ok_or("--body-file needs a file")?)?);
                if !exits.is_empty() {
                    title = format!("{title} ({exits})");
                }
            }
            "--files" => {
                let text = read(it.next().ok_or("--files needs a file")?)?;
                paths.extend(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from));
            }
            "--" => {}
            p => paths.push(p.to_string()),
        }
    }
    Ok((title, branch, paths))
}

/// `plan --pr TITLE [--branch B] [--body-file F] [--files F] [--] PATH...`: the R/P firewall and the overdue rule for one PR (BUILD-074); without an active sprint file (the old form, a draft) it checks nothing.
pub fn pr_check(root: &Path, args: &[String]) -> Res {
    let (title, branch, paths) = parse_pr_args(args, &|f| std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}")))?;
    let cur = pick_current(milestone_files(root)?)?;
    let Some((name, text)) = cur.active.filter(|(_, t)| sprintfile::is_sprint_file(t)) else {
        println!("plan --pr: no active sprint file (the old milestone form, or a draft): no firewall check");
        return Ok(());
    };
    let s = sprintfile::parse(&text).map_err(|e| format!("docs/milestones/{name}.md: {e}"))?;
    let errs = sprintfile::pr_errors(&name, &s, &title, branch.as_deref(), &paths, sprintfile::today());
    if errs.is_empty() {
        println!("plan --pr: `{title}` passes {name}'s firewall ({} paths)", paths.len());
        Ok(())
    } else {
        Err(format!("plan --pr: {} problem(s):\n  {}", errs.len(), errs.join("\n  ")))
    }
}

/// `cargo xtask plan [--pr ...]`: prints the plan; an `Err` (exit code 1) when the milestone, a draft or the queue breaks BUILD-053 or BUILD-074.
pub fn run(root: &Path, args: &[String]) -> Res {
    match args.first().map(String::as_str) {
        Some("--pr") => return pr_check(root, &args[1..]),
        Some(other) => {
            return Err(format!(
                "plan: unknown argument `{other}` (usage: plan [--pr TITLE [--branch B] [--body-file F] [--files F] PATH...])"
            ))
        }
        None => {}
    }
    let cur = pick_current(milestone_files(root)?)?;
    let status = std::fs::read_to_string(root.join("docs").join("status.md")).map_err(|e| e.to_string())?;
    let rows = parse_queue(&status);
    let prs = if cur.active.as_ref().is_some_and(|(_, t)| !sprintfile::is_sprint_file(t)) { open_prs(root) } else { Vec::new() };
    let (out, errs) = plan(root, &cur, &rows, &prs, sprintfile::today());
    print!("{out}");
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
    fn spec_lint_reads_the_repository_files_and_names_what_breaks() {
        let root = std::env::temp_dir().join(format!("plancmd-lint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("docs").join("milestones")).unwrap();
        assert!(lint_errors(&root).iter().any(|e| e.contains("no milestone file")), "no milestone");
        std::fs::write(root.join("docs").join("milestones").join("P1.md"), MILESTONE).unwrap();
        std::fs::write(root.join("docs").join("status.md"), STATUS).unwrap();
        assert!(lint_errors(&root).is_empty(), "{:?}", lint_errors(&root));
        std::fs::write(root.join("docs").join("status.md"), "## Queue\n| Exit | Step |\n|---|---|\n| X9 | a |\n").unwrap();
        let e = lint_errors(&root);
        assert!(e.iter().any(|m| m.contains("X9")) && e.iter().any(|m| m.contains("E1")), "{e:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    const SPRINT: &str = include_str!("fixtures/sprint-good.md");
    const RECORD: &str = include_str!("fixtures/record-pass.md");

    fn draft_of(text: &str) -> String {
        text.replace("Status: active (owner, 2026-12-01)", "Status: draft (lead, 2026-11-30): waits for the owner's decision: open")
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn one_active_file_one_active_and_a_draft_or_a_draft_alone_and_nothing_else() {
        let f = |n: &str, t: &str| (n.to_string(), t.to_string());
        let old = || f("P3", MILESTONE);
        let draft = || f("S-Next", &draft_of(SPRINT));
        let alone = pick_current(vec![draft(), f("M9", "# M9\nStatus: closed\n")]).unwrap();
        assert!(alone.active.is_none() && alone.draft.as_ref().unwrap().0 == "S-Next");
        let both = pick_current(vec![old(), draft()]).unwrap();
        assert_eq!((both.active.unwrap().0, both.draft.unwrap().0), ("P3".to_string(), "S-Next".to_string()));
        let two = pick_current(vec![draft(), f("S-Other", &draft_of(SPRINT))]).unwrap_err();
        assert!(two.contains("2 milestone files") && two.contains("`draft`"), "{two}");
        assert!(pick_current(vec![old(), f("S-Test", SPRINT)]).unwrap_err().contains("starting `active`"));
        assert!(pick_current(vec![f("M9", "# M9\nStatus: closed\n")]).unwrap_err().contains("no milestone file"));
        assert!(pick_active(vec![draft()]).is_err(), "a draft is not the active milestone");
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn between_sprints_the_draft_waits_for_the_open_and_the_queue_is_empty() {
        let root = Path::new("/nonexistent");
        let cur = Current { active: None, draft: Some(("S-Next".into(), draft_of(SPRINT))) };
        let rec_free = draft_of(SPRINT).replace("promoted E2-record.md", "open");
        let cur_ok = Current { active: None, draft: Some(("S-Next".into(), rec_free)) };
        let (out, e) = plan(root, &cur_ok, &[], &[], 0);
        assert!(out.contains("no active sprint; S-Next waits for the owner's decision: open") && e.is_empty(), "{out} {e:?}");
        let queue = parse_queue("## Queue\n| Exit | Step |\n|---|---|\n| E1 | a |\n");
        let (_, e) = plan(root, &cur_ok, &queue, &[], 0);
        assert!(e.iter().any(|m| m.contains("queue row `E1`") && m.contains("must be empty")), "{e:?}");
        let (_, e) = plan(root, &cur, &[], &[], 0);
        assert!(e.iter().any(|m| m.contains("E2-record.md") && m.contains("does not exist")), "a draft gets the same checks: {e:?}");
        let old_draft = Current { active: None, draft: Some(("S-Next".into(), "# S\nStatus: draft\n".into())) };
        assert!(plan(root, &old_draft, &[], &[], 0).1.iter().any(|m| m.contains("a draft is a sprint file")));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn an_active_sprint_file_is_read_with_its_records_and_its_queue() {
        let root = std::env::temp_dir().join(format!("plancmd-sprint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ms = root.join("docs").join("milestones");
        std::fs::create_dir_all(ms.join("S-Test")).unwrap();
        std::fs::write(ms.join("S-Test.md"), SPRINT).unwrap();
        std::fs::write(ms.join("P3.md"), MILESTONE.replace("Status: active (to 11-20)", "Status: closed (owner)")).unwrap();
        let queue = "## Queue\n| Exit | Step |\n|---|---|\n| E1 | a |\n| E3 | b |\n| E4 | c |\n";
        std::fs::write(root.join("docs").join("status.md"), queue).unwrap();
        assert!(lint_errors(&root).iter().any(|m| m.contains("E2-record.md") && m.contains("does not exist")), "{:?}", lint_errors(&root));
        std::fs::write(ms.join("S-Test").join("E2-record.md"), RECORD).unwrap();
        assert!(lint_errors(&root).is_empty(), "the promoted E2 needs no queue row: {:?}", lint_errors(&root));
        std::fs::write(root.join("docs").join("status.md"), "## Queue\n| Exit | Step |\n|---|---|\n| E1 | a |\n").unwrap();
        let e = lint_errors(&root);
        assert!(e.iter().any(|m| m.contains("E3") && m.contains("no row")) && e.iter().any(|m| m.contains("E4")), "{e:?}");
        let cur = pick_current(milestone_files(&root).unwrap()).unwrap();
        let (out, _) = plan(&root, &cur, &[], &[], sprintfile::day_number("2026-12-02").unwrap());
        assert!(out.starts_with("sprint S-Test: opened 2026-12-01, closes by 2026-12-05;"), "{out}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn the_pr_arguments_take_a_branch_a_body_files_exits_and_a_path_list() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let files = |f: &str| -> Res<String> {
            match f {
                "body.md" => Ok("Sprint: S-Test\nExit: E2, E3\nVerifier: PASS abc\n".into()),
                "paths.txt" => Ok("crates/a.rs\n\nspikes/x/results/r.md\n".into()),
                other => Err(format!("{other}: not found")),
            }
        };
        let (t, b, p) = parse_pr_args(
            &a(&["tidy", "--branch", "change/x", "--body-file", "body.md", "--files", "paths.txt", "--", "docs/y.md"]),
            &files,
        )
        .unwrap();
        assert_eq!((t.as_str(), b.as_deref()), ("tidy (E2, E3)", Some("change/x")));
        assert_eq!(p, ["crates/a.rs", "spikes/x/results/r.md", "docs/y.md"]);
        assert!(names(&t, "E3") && !names(&t, "E1"));
        assert!(parse_pr_args(&a(&[]), &files).unwrap_err().contains("needs the PR title"));
        assert!(parse_pr_args(&a(&["t", "--branch"]), &files).unwrap_err().contains("--branch needs"));
        assert!(parse_pr_args(&a(&["t", "--files", "nope"]), &files).unwrap_err().contains("not found"));
        assert_eq!(body_exits("x\nExit: E1\n  Exit: E4\n"), "E1 E4");
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn the_pr_check_reads_the_repository_and_checks_nothing_without_an_active_sprint_file() {
        let root = std::env::temp_dir().join(format!("plancmd-pr-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ms = root.join("docs").join("milestones");
        std::fs::create_dir_all(&ms).unwrap();
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        std::fs::write(ms.join("P3.md"), MILESTONE).unwrap();
        assert!(pr_check(&root, &a(&["E1: x", "--branch", "change/x", "crates/a.rs"])).is_ok(), "the old form has no firewall");
        std::fs::write(ms.join("P3.md"), MILESTONE.replace("Status: active (to 11-20)", "Status: closed")).unwrap();
        std::fs::write(ms.join("S-Test.md"), SPRINT).unwrap();
        let e = pr_check(&root, &a(&["E3: survey", "crates/a.rs"])).unwrap_err();
        assert!(e.contains("exit E3 is mode R") && e.contains("crates/a.rs"), "{e}");
        assert!(pr_check(&root, &a(&["E3: survey", "spikes/hump/results/results.md"])).is_ok());
        std::fs::write(ms.join("S-Test.md"), draft_of(SPRINT)).unwrap();
        assert!(pr_check(&root, &a(&["E3: survey", "crates/a.rs"])).is_ok(), "a draft has no firewall: nothing runs before the open");
        std::fs::remove_dir_all(&root).unwrap();
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn an_active_file_with_an_old_form_draft_names_the_draft() {
        let cur = Current { active: Some(("P3".into(), MILESTONE.into())), draft: Some(("S-Next".into(), "# S\nStatus: draft\n".into())) };
        let queue = parse_queue(STATUS);
        let (out, e) = plan(Path::new("/nonexistent"), &cur, &queue, &[], 0);
        assert!(out.contains("next sprint: S-Next (draft)"), "{out}");
        assert!(e.iter().any(|m| m.contains("S-Next") && m.contains("a draft is a sprint file")), "{e:?}");
    }

    // spec: BUILD-053
    #[test]
    fn a_duplicate_exit_id_is_refused() {
        let dup = "Status: active\n| # | Item | Done when | Lane | State |\n|--|--|--|--|--|\n| E1 | a | b | c | open |\n| E1 | a | b | c | open |\n";
        assert!(parse_exit_table(dup).unwrap_err().contains("twice"));
    }
}
