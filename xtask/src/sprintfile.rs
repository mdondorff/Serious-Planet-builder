//! The sprint file of ADR 0130 (BUILD-074): `docs/milestones/<M>.md` in the form of the combined plan's 2.2, read and checked by `cargo xtask plan` v2.
//! A file is a sprint file when it has an `Opened:` line; an older milestone file (P3 and before) keeps BUILD-053's form.
//! Checked here: the header (dates, at most 7 days, objective, out of scope), at most five exit rows with every cell, the mode, the gate class,
//! the state and, for `promoted` and `parked`, the record file and its criteria table; at most three open decisions, each with a default and a
//! `By` date, and A1 (only a safe default fires on silence; any other waits for the owner); `overdue` past `Closes by`; and `--pr`, the R/P
//! firewall. The gate-ledger park counters are not built (combined plan 0, item 3b).

use std::path::Path;

/// The modes of an exit row (ADR 0130 point 2).
pub const MODES: [&str; 4] = ["R", "P", "X", "M"];
/// The gate classes of a Gate cell part (combined plan 2.4), short and long forms.
pub const GATE_CLASSES: [&str; 7] = ["num", "numerical", "fun", "functional", "dom", "domain", "look"];
/// The longest sprint, days from `Opened` to `Closes by`.
pub const MAX_DAYS: i64 = 7;
/// At most this many exit rows.
pub const MAX_EXITS: usize = 5;
/// At most this many open decisions.
pub const MAX_OPEN_DECISIONS: usize = 3;
/// The paths a PR of an R or P exit may touch.
pub const RP_PATHS: [&str; 2] = ["docs/", "spikes/"];

/// One exit row of a sprint file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exit {
    pub id: String,
    pub item: String,
    pub mode: String,
    pub done_when: String,
    pub gate: String,
    pub park: String,
    pub lane: String,
    pub state: String,
    pub result: String,
}

impl Exit {
    /// The first word of the state, lower case (`done`, `promoted`, ...).
    pub fn state_word(&self) -> String {
        self.state.split_whitespace().next().unwrap_or("").to_lowercase()
    }
    /// An exit is open unless done, promoted, parked or carried.
    pub fn is_open(&self) -> bool {
        !matches!(self.state_word().as_str(), "done" | "promoted" | "parked" | "carried")
    }
    /// The record named by a `promoted <record>` or `parked <record>` state (backticks stripped).
    pub fn record(&self) -> Option<String> {
        let mut w = self.state.split_whitespace();
        match w.next()?.to_lowercase().as_str() {
            "promoted" | "parked" => w.next().map(|r| r.trim_matches('`').to_string()).filter(|r| !r.is_empty()),
            _ => None,
        }
    }
}

/// One row of the Decisions table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub decision: String,
    pub recommendation: String,
    pub default: String,
    pub by: String,
    pub state: String,
}

impl Decision {
    /// Open while the owner has not answered and no default was taken: state `open` or `waiting-owner`.
    pub fn is_open(&self) -> bool {
        let s = self.state.trim().to_lowercase();
        s.is_empty() || s.starts_with("open") || s.starts_with("waiting-owner")
    }
    /// A1: only park, keep-off or keep today's may fire on silence.
    pub fn default_is_safe(&self) -> bool {
        let d = self.default.trim().to_lowercase().replace('\u{2019}', "'");
        ["park", "keep-off", "keep off", "keep today", "keep-today"].iter().any(|p| d.starts_with(p))
    }
}

/// A parsed sprint file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sprint {
    pub status: String,
    pub opened: String,
    pub closes_by: String,
    pub owner_look: String,
    pub rd_lane: String,
    pub objective: String,
    pub out_of_scope: Vec<String>,
    pub exits: Vec<Exit>,
    pub decisions: Vec<Decision>,
}

/// Whether a milestone file is in the sprint form: it has an `Opened:` line.
pub fn is_sprint_file(text: &str) -> bool {
    text.lines().any(|l| l.trim_start().starts_with("Opened:"))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` date at the start of `s` (later text is ignored); `None` when there is none or it is not a calendar date.
pub fn day_number(s: &str) -> Option<i64> {
    let s = s.trim();
    let d = s.get(0..10)?;
    let b = d.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || !d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return None;
    }
    let (y, m, dd): (i64, i64, i64) = (d[0..4].parse().ok()?, d[5..7].parse().ok()?, d[8..10].parse().ok()?);
    if !(1..=12).contains(&m) || dd < 1 {
        return None;
    }
    let n = days_from_civil(y, m, dd);
    (crate::accept::utc_date((n * 86_400) as u64) == d).then_some(n)
}

/// Today's day number (UTC).
pub fn today() -> i64 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    (secs / 86_400) as i64
}

/// The header labels of 2.2's date line, in order.
const LABELS: [&str; 4] = ["Opened:", "Closes by:", "Owner look:", "R&D lane:"];

/// The value after `label` on the first line holding it, up to the next label of [`LABELS`] or the line's end.
fn field(text: &str, label: &str) -> Option<String> {
    let line = text.lines().take_while(|l| !l.starts_with("## ")).find(|l| l.contains(label))?;
    let rest = &line[line.find(label)? + label.len()..];
    let end = LABELS.iter().filter_map(|l| rest.find(l)).min().unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// The value of a `Name:` line (the first line starting with it).
fn line_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim_start().strip_prefix(name).map(|v| v.trim().to_string()))
}

/// The cells of a markdown table row, trimmed.
fn cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let inner = t.strip_prefix('|')?;
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    Some(inner.split('|').map(|c| c.trim().to_string()).collect())
}

fn is_separator(c: &[String]) -> bool {
    !c.is_empty() && c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// The first table under the heading starting with `heading` (up to the next `## `): the lower-cased header and the rows.
fn section_table(text: &str, heading: &str) -> Option<(Vec<String>, Vec<Vec<String>>)> {
    let mut lines = text.lines().skip_while(|l| !l.starts_with(heading)).skip(1).take_while(|l| !l.starts_with("## ")).peekable();
    let head = loop {
        let l = lines.next()?;
        if let Some(c) = cells(l) {
            break c.iter().map(|x| x.to_lowercase()).collect::<Vec<_>>();
        }
    };
    if lines.peek().and_then(|l| cells(l)).is_some_and(|c| is_separator(&c)) {
        lines.next();
    }
    let rows = lines.map_while(cells).filter(|c| c.first().is_some_and(|x| !x.is_empty())).collect();
    Some((head, rows))
}

/// Reads a table's rows by column name; an error names the missing columns.
fn by_columns(head: &[String], rows: &[Vec<String>], want: &[&[&str]], what: &str) -> Result<Vec<Vec<String>>, String> {
    let mut idx = Vec::new();
    let mut missing = Vec::new();
    for names in want {
        match head.iter().position(|h| names.contains(&h.as_str())) {
            Some(i) => idx.push(i),
            None => missing.push(names[0]),
        }
    }
    if !missing.is_empty() {
        return Err(format!("{what}: the columns {} are missing", missing.join(", ")));
    }
    Ok(rows.iter().map(|r| idx.iter().map(|&i| r.get(i).cloned().unwrap_or_default()).collect()).collect())
}

/// Parses a sprint file; an error names the section or the column that is missing.
pub fn parse(text: &str) -> Result<Sprint, String> {
    let mut s = Sprint {
        status: line_value(text, "Status:").unwrap_or_default(),
        opened: field(text, "Opened:").unwrap_or_default(),
        closes_by: field(text, "Closes by:").unwrap_or_default(),
        owner_look: field(text, "Owner look:").unwrap_or_default(),
        rd_lane: field(text, "R&D lane:").unwrap_or_default(),
        objective: line_value(text, "Objective:").unwrap_or_default(),
        ..Sprint::default()
    };
    if let Some(first) = line_value(text, "Out of scope:") {
        if !first.is_empty() {
            s.out_of_scope.push(first);
        }
        let after = text.lines().skip_while(|l| !l.trim_start().starts_with("Out of scope:")).skip(1);
        s.out_of_scope.extend(after.map_while(|l| l.trim_start().strip_prefix("- ").map(|v| v.trim().to_string())));
    }
    let (head, rows) = section_table(text, "## Exit items").ok_or("no `## Exit items` table")?;
    let cols: [&[&str]; 9] =
        [&["#"], &["item", "exit item", "exit"], &["mode"], &["done when"], &["gate"], &["park"], &["lane"], &["state"], &["result"]];
    for c in by_columns(&head, &rows, &cols, "the exit table")? {
        let [id, item, mode, done_when, gate, park, lane, state, result]: [String; 9] =
            c.try_into().map_err(|_| "the exit table: a short row")?;
        s.exits.push(Exit { id, item, mode, done_when, gate, park, lane, state, result });
    }
    let (head, rows) = section_table(text, "## Decisions").ok_or("no `## Decisions` table")?;
    let cols: [&[&str]; 6] = [
        &["#"],
        &["decision"],
        &["lead's recommendation", "recommendation", "lead\u{2019}s recommendation"],
        &["default"],
        &["by"],
        &["state"],
    ];
    for c in by_columns(&head, &rows, &cols, "the decisions table")? {
        let [id, decision, recommendation, default, by, state]: [String; 6] =
            c.try_into().map_err(|_| "the decisions table: a short row")?;
        s.decisions.push(Decision { id, decision, recommendation, default, by, state });
    }
    Ok(s)
}

/// Whether a cell counts as empty (blank).
fn blank(c: &str) -> bool {
    c.trim().is_empty()
}

/// The criteria table of a record file: `(criterion, class, verdict)` per row.
pub fn record_criteria(text: &str) -> Result<Vec<(String, String, String)>, String> {
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let Some(head) = cells(l) else { continue };
        let low: Vec<String> = head.iter().map(|h| h.to_lowercase()).collect();
        let col = |n: &str| low.iter().position(|h| h == n);
        let (Some(c), Some(k), Some(v)) = (col("criterion"), col("class"), col("verdict")) else { continue };
        if lines.peek().and_then(|l| cells(l)).is_some_and(|x| is_separator(&x)) {
            lines.next();
        }
        let rows: Vec<(String, String, String)> = lines
            .map_while(cells)
            .map(|r| {
                let g = |i: usize| r.get(i).cloned().unwrap_or_default();
                (g(c), g(k), g(v))
            })
            .collect();
        return if rows.is_empty() { Err("the criteria table has no rows".into()) } else { Ok(rows) };
    }
    Err("no criteria table (columns criterion, class, verdict)".into())
}

/// Where a record named in a state lives: a path with a `/` is relative to the repository, a bare name is in `docs/milestones/<M>/`.
pub fn record_path(root: &Path, milestone: &str, record: &str) -> std::path::PathBuf {
    if record.contains('/') {
        root.join(record)
    } else {
        root.join("docs").join("milestones").join(milestone).join(record)
    }
}

/// The checks of BUILD-074 on a parsed sprint file. `read` gives a record's text (`None` when it does not exist).
pub fn errors(s: &Sprint, name: &str, read: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut e = Vec::new();
    let file = format!("docs/milestones/{name}.md");
    let status = s.status.to_lowercase();
    if !["active", "closed", "draft"].iter().any(|w| status.starts_with(w)) {
        e.push(format!("{file}: the Status line must start `active`, `draft` or `closed`, not `{}`", s.status));
    }
    match (day_number(&s.opened), day_number(&s.closes_by)) {
        (Some(o), Some(c)) if c < o => e.push(format!("{file}: Closes by {} is before Opened {}", s.closes_by, s.opened)),
        (Some(o), Some(c)) if c - o > MAX_DAYS => {
            e.push(format!("{file}: Closes by {} is {} days after Opened {}: at most {MAX_DAYS}", s.closes_by, c - o, s.opened))
        }
        (o, c) => {
            if o.is_none() {
                e.push(format!("{file}: `Opened:` needs a YYYY-MM-DD date, not `{}`", s.opened));
            }
            if c.is_none() {
                e.push(format!("{file}: `Closes by:` needs a YYYY-MM-DD date, not `{}`", s.closes_by));
            }
        }
    }
    let look = s.owner_look.trim();
    if look.to_lowercase() != "none" && day_number(look).is_none() {
        e.push(format!("{file}: `Owner look:` needs a date or `none`, not `{look}`"));
    }
    if blank(&s.rd_lane) {
        e.push(format!("{file}: `R&D lane:` is empty"));
    }
    if blank(&s.objective) {
        e.push(format!("{file}: `Objective:` is empty"));
    }
    if s.out_of_scope.is_empty() {
        e.push(format!("{file}: `Out of scope:` names nothing"));
    }
    if s.exits.is_empty() || s.exits.len() > MAX_EXITS {
        e.push(format!("{file}: {} exit rows: 1 to {MAX_EXITS}", s.exits.len()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for x in &s.exits {
        let at = format!("{file} exit {}", x.id);
        if !seen.insert(x.id.clone()) {
            e.push(format!("{at}: the exit ID appears twice"));
        }
        for (cell, v) in
            [("Item", &x.item), ("Done when", &x.done_when), ("Gate", &x.gate), ("Park", &x.park), ("Lane", &x.lane), ("State", &x.state)]
        {
            if blank(v) {
                e.push(format!("{at}: the {cell} cell is empty"));
            }
        }
        if !MODES.contains(&x.mode.trim()) {
            e.push(format!("{at}: mode `{}` is none of R, P, X, M", x.mode));
        }
        for part in x.gate.split(';').map(str::trim).filter(|p| !p.is_empty() && *p != "-") {
            let class = part.split(':').next().unwrap_or("").trim().to_lowercase();
            if !part.contains(':') || !GATE_CLASSES.contains(&class.as_str()) {
                e.push(format!("{at}: gate `{part}` is not `<class>: ...` with a class of num, fun, dom, look"));
            }
        }
        let word = x.state_word();
        match word.as_str() {
            "open" | "running" | "look-pending" | "waiting-owner" => {}
            "done" => {
                if day_number(x.state.split_whitespace().nth(1).unwrap_or("")).is_none() {
                    e.push(format!("{at}: state `{}` needs `done <YYYY-MM-DD>`", x.state));
                }
            }
            "carried" => {
                if x.state.split_whitespace().nth(1).is_none() {
                    e.push(format!("{at}: state `{}` needs `carried <M>`", x.state));
                }
            }
            "promoted" | "parked" => e.extend(record_errors(x, &at, read)),
            _ => e.push(format!(
                "{at}: state `{}` is none of open, running, look-pending, waiting-owner, done <date>, promoted <record>, parked <record>, carried <M>",
                x.state
            )),
        }
        if word == "promoted" && x.mode.trim() != "P" {
            e.push(format!("{at}: only a P row is promoted (mode {})", x.mode));
        }
    }
    let open: Vec<&Decision> = s.decisions.iter().filter(|d| d.is_open()).collect();
    if open.len() > MAX_OPEN_DECISIONS {
        e.push(format!(
            "{file}: capped: {} open decisions ({}), at most {MAX_OPEN_DECISIONS}",
            open.len(),
            open.iter().map(|d| d.id.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    for d in &s.decisions {
        let at = format!("{file} decision {}", d.id);
        if blank(&d.default) {
            e.push(format!("{at}: the Default cell is empty"));
        }
        if day_number(&d.by).is_none() {
            e.push(format!("{at}: `By` needs a YYYY-MM-DD date, not `{}`", d.by));
        }
        if d.state.to_lowercase().starts_with("default taken") && !d.default_is_safe() {
            e.push(format!(
                "{at}: default `{}` was taken on silence, but only park, keep-off or keep today's fire on silence (A1); the row waits for the owner (`waiting-owner`)",
                d.default
            ));
        }
    }
    e
}

/// The checks of a promoted or parked row's record: it exists, its criteria table has rows, a promotion has no `fail` or empty verdict, and no numerical criterion takes an exception.
fn record_errors(x: &Exit, at: &str, read: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let Some(rec) = x.record() else { return vec![format!("{at}: state `{}` names no record", x.state)] };
    let Some(text) = read(&rec) else { return vec![format!("{at}: the record `{rec}` does not exist")] };
    let rows = match record_criteria(&text) {
        Ok(r) => r,
        Err(err) => return vec![format!("{at}: record `{rec}`: {err}")],
    };
    let promoted = x.state_word() == "promoted";
    let mut e = Vec::new();
    for (crit, class, verdict) in rows {
        let v = verdict.trim().to_lowercase();
        if promoted && (v.is_empty() || v.starts_with("fail")) {
            e.push(format!(
                "{at}: record `{rec}`: criterion `{crit}` has verdict `{verdict}`: a promotion needs pass or an owner exception"
            ));
        }
        if class.trim().to_lowercase().starts_with("num") && v.starts_with("exception") {
            e.push(format!("{at}: record `{rec}`: criterion `{crit}` is numerical and takes no exception (CON-14, CON-28 to CON-31)"));
        }
    }
    e
}

/// What `plan` prints about the sprint's dates and decisions on `today`: `overdue`, a safe default that is due, a decision waiting for the owner (A1).
pub fn notices(s: &Sprint, today: i64) -> Vec<String> {
    let mut n = Vec::new();
    if is_overdue(s, today) {
        n.push(format!("overdue: past Closes by {}; only PRs that close, carry or park an exit may merge", s.closes_by));
    }
    for d in s.decisions.iter().filter(|d| d.is_open()) {
        let Some(by) = day_number(&d.by) else { continue };
        if today > by {
            if d.default_is_safe() {
                n.push(format!(
                    "decision {}: By {} passed: the default `{}` fires (record `default taken (owner silent, <date>)`)",
                    d.id, d.by, d.default
                ));
            } else {
                n.push(format!(
                    "decision {}: waiting-owner: By {} passed, and `{}` is not a safe default (A1), so it does not fire",
                    d.id, d.by, d.default
                ));
            }
        }
    }
    n
}

/// Whether an active sprint is past its `Closes by` date.
pub fn is_overdue(s: &Sprint, today: i64) -> bool {
    s.status.to_lowercase().starts_with("active") && day_number(&s.closes_by).is_some_and(|c| today > c)
}

/// The printed sprint: the header, per exit its mode, state, lane, gate and done-when, and the open decisions.
pub fn render(name: &str, s: &Sprint, today: i64) -> String {
    let open = s.exits.iter().filter(|x| x.is_open()).count();
    let mut out = format!(
        "sprint {name}: opened {}, closes by {}{}; owner look {}; R&D lane {}; {} exits, {open} open\n  objective: {}\n",
        s.opened,
        s.closes_by,
        if is_overdue(s, today) { " (overdue)" } else { "" },
        s.owner_look,
        s.rd_lane,
        s.exits.len(),
        s.objective
    );
    for x in &s.exits {
        out.push_str(&format!(
            "{} [{}] {} [{}] lane {}\n  gate: {}\n  done when: {}\n",
            x.id, x.mode, x.item, x.state, x.lane, x.gate, x.done_when
        ));
    }
    let open_d: Vec<&Decision> = s.decisions.iter().filter(|d| d.is_open()).collect();
    out.push_str(&format!("decisions: {} open of {MAX_OPEN_DECISIONS}\n", open_d.len()));
    for d in open_d {
        out.push_str(&format!("  {} {} (default {}, by {})\n", d.id, d.decision, d.default, d.by));
    }
    for n in notices(s, today) {
        out.push_str(&format!("{n}\n"));
    }
    out
}

/// `plan --pr`: the R/P firewall and, past `Closes by`, the closing-PRs-only rule, for a PR's title, branch and changed paths.
/// - A PR whose title names an R or P exit (not promoted) may touch only `docs/` and `spikes/`.
/// - A `change/*` PR names an X or M exit, or a promoted P exit (its promotion record).
/// - Past `Closes by`, a PR that touches more than `docs/` must also change the sprint file (it closes, carries or parks an exit); a docs-only PR never fails.
pub fn pr_errors(name: &str, s: &Sprint, title: &str, branch: Option<&str>, paths: &[String], today: i64) -> Vec<String> {
    let mut e = Vec::new();
    let named: Vec<&Exit> = s.exits.iter().filter(|x| crate::plancmd::names(title, &x.id)).collect();
    let promoted = |x: &Exit| x.state_word() == "promoted";
    let slash = |p: &String| p.replace('\\', "/");
    for x in named.iter().filter(|x| matches!(x.mode.trim(), "R" | "P") && !promoted(x)) {
        let off: Vec<String> = paths.iter().map(slash).filter(|p| !RP_PATHS.iter().any(|r| p.starts_with(r))).collect();
        if !off.is_empty() {
            e.push(format!("exit {} is mode {}: its PR may touch only docs/ and spikes/, not {}", x.id, x.mode, off.join(", ")));
        }
    }
    if branch.is_some_and(|b| b.starts_with("change/"))
        && !named.iter().any(|x| matches!(x.mode.trim(), "X" | "M") || (x.mode.trim() == "P" && promoted(x)))
    {
        e.push(format!("a change/* PR names an X or M exit, or a promoted P exit, of {name}; the title `{title}` names none"));
    }
    if is_overdue(s, today) {
        let sprint_file = format!("docs/milestones/{name}.md");
        let docs_only = paths.iter().map(slash).all(|p| p.starts_with("docs/"));
        if !docs_only && !paths.iter().map(slash).any(|p| p == sprint_file) {
            e.push(format!(
                "{name} is overdue (Closes by {}): only a PR that closes, carries or parks an exit (it changes {sprint_file}) may merge",
                s.closes_by
            ));
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = include_str!("fixtures/sprint-good.md");
    const RECORD_PASS: &str = include_str!("fixtures/record-pass.md");
    const RECORD_FAIL: &str = include_str!("fixtures/record-fail.md");

    fn day(s: &str) -> i64 {
        day_number(s).unwrap()
    }

    fn reader<'a>(files: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |r: &str| files.iter().find(|(n, _)| *n == r).map(|(_, t)| t.to_string())
    }

    fn errs(text: &str, files: &[(&str, &str)]) -> Vec<String> {
        errors(&parse(text).unwrap(), "S-Test", &reader(files))
    }

    const PASS: [(&str, &str); 1] = [("E2-record.md", RECORD_PASS)];

    // spec: BUILD-074, TEST-161
    #[test]
    fn the_good_fixture_parses_into_its_header_exits_and_decisions_and_passes() {
        let s = parse(GOOD).unwrap();
        assert_eq!(
            (s.opened.as_str(), s.closes_by.as_str(), s.owner_look.as_str(), s.rd_lane.as_str()),
            ("2026-12-01", "2026-12-05", "2026-12-04", "spikes")
        );
        assert!(s.objective.starts_with("Region boxes"), "{}", s.objective);
        assert_eq!(s.out_of_scope.len(), 2, "{:?}", s.out_of_scope);
        assert_eq!(
            s.exits.iter().map(|x| (x.id.as_str(), x.mode.as_str())).collect::<Vec<_>>(),
            [("E1", "X"), ("E2", "P"), ("E3", "R"), ("E4", "M")]
        );
        assert_eq!(s.decisions.len(), 2);
        assert!(errs(GOOD, &PASS).is_empty(), "{:?}", errs(GOOD, &PASS));
        assert!(is_sprint_file(GOOD) && !is_sprint_file("# P3\nStatus: active\n| # | Exit item | Done when | Lane | State |\n"));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn dates_are_calendar_dates_and_a_sprint_lasts_at_most_seven_days() {
        assert_eq!(day("1970-01-02"), 1);
        assert_eq!(day("2026-12-08") - day("2026-12-01"), 7);
        assert!(day_number("2026-02-30").is_none() && day_number("2026-13-01").is_none() && day_number("26-12-01").is_none());
        assert_eq!(day_number("2026-12-01 (owner)"), day_number("2026-12-01"));
        let long = GOOD.replace("Closes by: 2026-12-05", "Closes by: 2026-12-09");
        assert!(errs(&long, &PASS).iter().any(|m| m.contains("8 days") && m.contains("at most 7")));
        assert!(errs(&GOOD.replace("Closes by: 2026-12-05", "Closes by: 2026-12-08"), &PASS).is_empty());
        let back = GOOD.replace("Closes by: 2026-12-05", "Closes by: 2026-11-30");
        assert!(errs(&back, &PASS).iter().any(|m| m.contains("before Opened")));
        let bad = GOOD.replace("Opened: 2026-12-01", "Opened: soon");
        assert!(errs(&bad, &PASS).iter().any(|m| m.contains("`Opened:` needs")));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn the_header_needs_an_objective_out_of_scope_lines_a_look_date_and_a_lane() {
        let no_obj = GOOD.replace("Objective: Region boxes are marked, baked and reopened in the editor.", "Objective:");
        assert!(errs(&no_obj, &PASS).iter().any(|m| m.contains("`Objective:` is empty")));
        let no_scope = GOOD.replace("Out of scope: editing source maps\n- cross-face regions\n", "Out of scope:\n");
        assert!(errs(&no_scope, &PASS).iter().any(|m| m.contains("names nothing")), "{:?}", errs(&no_scope, &PASS));
        let look = GOOD.replace("Owner look: 2026-12-04", "Owner look: later");
        assert!(errs(&look, &PASS).iter().any(|m| m.contains("`Owner look:`")));
        assert!(errs(&GOOD.replace("Owner look: 2026-12-04", "Owner look: none"), &PASS).is_empty());
        let no_lane = GOOD.replace("R&D lane: spikes", "R&D lane:");
        assert!(errs(&no_lane, &PASS).iter().any(|m| m.contains("`R&D lane:` is empty")));
        let status = GOOD.replace("Status: active (owner, 2026-12-01)", "Status: planned");
        assert!(errs(&status, &PASS).iter().any(|m| m.contains("Status line")));
        let draft =
            GOOD.replace("Status: active (owner, 2026-12-01)", "Status: draft (lead, 2026-11-30): waits for the owner's decision: open");
        assert!(errs(&draft, &PASS).is_empty(), "a draft gets the same checks and passes: {:?}", errs(&draft, &PASS));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn at_most_five_exits_each_with_every_cell_a_mode_and_a_gate_class() {
        let row = "| E9 | extra | X | x | fun: BUILD-001 (2026-12-01) | days 3 -> park, keep today's | crusader | open | - |\n";
        let six = GOOD.replacen("| E4 |", &format!("{}{}| E4 |", row, row.replace("E9", "E8")), 1);
        assert!(errs(&six, &PASS).iter().any(|m| m.contains("6 exit rows")), "{:?}", errs(&six, &PASS));
        let mode = GOOD.replace("| E1 | Region file | X |", "| E1 | Region file | Production |");
        assert!(errs(&mode, &PASS).iter().any(|m| m.contains("exit E1") && m.contains("none of R, P, X, M")));
        let gate = GOOD.replace("fun: BUILD-074 (2026-12-01)", "tier A/B");
        assert!(errs(&gate, &PASS).iter().any(|m| m.contains("exit E1") && m.contains("gate `tier A/B`")));
        let park = GOOD.replace("| days 3 -> park, keep today's | explorer |", "|  | explorer |");
        assert!(errs(&park, &PASS).iter().any(|m| m.contains("exit E1") && m.contains("Park cell is empty")), "{:?}", errs(&park, &PASS));
        let state = GOOD.replace("| explorer | running |", "| explorer | busy |");
        assert!(errs(&state, &PASS).iter().any(|m| m.contains("state `busy`")));
        let done = GOOD.replace("| explorer | running |", "| explorer | done |");
        assert!(errs(&done, &PASS).iter().any(|m| m.contains("done <YYYY-MM-DD>")));
        assert!(errs(&GOOD.replace("| explorer | running |", "| explorer | done 2026-12-03 |"), &PASS).is_empty());
        let dup = GOOD.replace("| E4 |", "| E1 |");
        assert!(errs(&dup, &PASS).iter().any(|m| m.contains("appears twice")));
        assert!(parse(&GOOD.replace("| Park |", "| Limit |")).unwrap_err().contains("park"));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn a_promoted_or_parked_row_needs_its_record_and_a_promotion_has_no_fail_or_empty_verdict() {
        assert!(errs(GOOD, &[]).iter().any(|m| m.contains("exit E2") && m.contains("`E2-record.md` does not exist")));
        let e = errs(GOOD, &[("E2-record.md", RECORD_FAIL)]);
        assert!(e.iter().any(|m| m.contains("criterion `relief10`") && m.contains("verdict `fail`")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("criterion `look R1h`") && m.contains("verdict ``")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("criterion `bit-identical off`") && m.contains("takes no exception")), "{e:?}");
        assert!(errs(GOOD, &[("E2-record.md", "# E2 record\nno table\n")]).iter().any(|m| m.contains("no criteria table")));
        let parked = GOOD.replace("promoted E2-record.md", "parked E2-record.md");
        let p = errs(&parked, &[("E2-record.md", RECORD_FAIL)]);
        assert!(!p.iter().any(|m| m.contains("verdict `fail`")), "a park records failed criteria: {p:?}");
        assert!(p.iter().any(|m| m.contains("takes no exception")), "a numerical exception fails a park too: {p:?}");
        let no_name = GOOD.replace("promoted E2-record.md", "promoted");
        assert!(errs(&no_name, &[]).iter().any(|m| m.contains("names no record")));
        let x_promoted = GOOD.replace("| explorer | running |", "| explorer | promoted E2-record.md |");
        assert!(errs(&x_promoted, &PASS).iter().any(|m| m.contains("exit E1") && m.contains("only a P row is promoted")));
        assert_eq!(record_path(Path::new("/r"), "S-Test", "E2-record.md"), Path::new("/r/docs/milestones/S-Test/E2-record.md"));
        assert_eq!(record_path(Path::new("/r"), "S-Test", "spikes/x/results/results.md"), Path::new("/r/spikes/x/results/results.md"));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn at_most_three_open_decisions_each_with_a_default_and_a_by_date() {
        let more = "| D3 | c | x | park | 2026-12-03 | open |\n| D4 | d | x | keep-off | 2026-12-03 | waiting-owner |\n";
        let four = GOOD.replacen("\n## Changes", &format!("{more}\n## Changes"), 1);
        assert!(errs(&four, &PASS).iter().any(|m| m.contains("capped: 4 open decisions") && m.contains("D4")), "{:?}", errs(&four, &PASS));
        let answered = four.replace(
            "| D1 | Cube margin | 4 cells | keep today's | 2026-12-02 | open |",
            "| D1 | Cube margin | 4 cells | keep today's | 2026-12-02 | decided 2026-12-02: 4 cells |",
        );
        assert!(errs(&answered, &PASS).is_empty(), "an answered decision is not open: {:?}", errs(&answered, &PASS));
        let no_default = GOOD.replace("| 4 cells | keep today's | 2026-12-02 |", "| 4 cells |  | 2026-12-02 |");
        assert!(errs(&no_default, &PASS).iter().any(|m| m.contains("decision D1") && m.contains("Default cell is empty")));
        let no_by = GOOD.replace("| keep today's | 2026-12-02 |", "| keep today's | Tuesday |");
        assert!(errs(&no_by, &PASS).iter().any(|m| m.contains("decision D1") && m.contains("`By` needs")));
        assert!(parse(&GOOD.replace("## Decisions (at most 3 open)", "## Choices")).unwrap_err().contains("Decisions"));
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn only_a_safe_default_fires_on_silence_and_any_other_waits_for_the_owner() {
        let s = parse(GOOD).unwrap();
        assert!(s.decisions[0].default_is_safe(), "keep today's");
        assert!(!s.decisions[1].default_is_safe(), "promote is a promotion decision");
        for safe in ["park", "park E2", "keep-off", "Keep off", "keep today's read", "keep today\u{2019}s"] {
            assert!(Decision { default: safe.into(), ..s.decisions[0].clone() }.default_is_safe(), "{safe}");
        }
        assert!(notices(&s, day("2026-12-02")).is_empty(), "nothing is due on the By date");
        let n = notices(&s, day("2026-12-03"));
        assert!(n.iter().any(|m| m.contains("decision D1") && m.contains("fires")), "{n:?}");
        assert!(n.iter().any(|m| m.contains("decision D2") && m.contains("waiting-owner") && m.contains("A1")), "{n:?}");
        let taken =
            GOOD.replace("| promote E2 | 2026-12-02 | open |", "| promote E2 | 2026-12-02 | default taken (owner silent, 2026-12-03) |");
        assert!(errs(&taken, &PASS).iter().any(|m| m.contains("decision D2") && m.contains("A1")));
        let safe_taken = GOOD
            .replace("| keep today's | 2026-12-02 | open |", "| keep today's | 2026-12-02 | default taken (owner silent, 2026-12-03) |");
        assert!(errs(&safe_taken, &PASS).is_empty());
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn past_closes_by_an_active_sprint_is_overdue() {
        let s = parse(GOOD).unwrap();
        assert!(!is_overdue(&s, day("2026-12-05")) && is_overdue(&s, day("2026-12-06")));
        assert!(render("S-Test", &s, day("2026-12-06")).contains("closes by 2026-12-05 (overdue)"));
        assert!(notices(&s, day("2026-12-06")).iter().any(|m| m.starts_with("overdue")));
        let closed = parse(&GOOD.replace("Status: active (owner, 2026-12-01)", "Status: closed (owner, 2026-12-05)")).unwrap();
        assert!(!is_overdue(&closed, day("2026-12-09")), "a closed sprint is never overdue");
        let out = render("S-Test", &s, day("2026-12-02"));
        assert!(out.contains("E2 [P] Erosion C2 read [promoted E2-record.md]") && out.contains("decisions: 2 open of 3"), "{out}");
    }

    // spec: BUILD-074, TEST-161
    #[test]
    fn an_r_or_p_rows_pr_touches_only_docs_and_spikes_and_a_change_pr_names_an_x_or_m_exit() {
        let s = parse(GOOD).unwrap();
        let t = day("2026-12-02");
        let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let e = pr_errors(
            "S-Test",
            &s,
            "E3: hump survey",
            Some("spike/e3"),
            &p(&["crates/cache/src/x.rs", "spikes/hump/results/results.md"]),
            t,
        );
        assert!(
            e.iter().any(|m| m.contains("exit E3 is mode R") && m.contains("crates/cache/src/x.rs") && !m.contains("spikes/hump")),
            "{e:?}"
        );
        assert!(pr_errors("S-Test", &s, "E3: results", None, &p(&["spikes/hump/results/results.md", "docs/spikes.md"]), t).is_empty());
        let on = pr_errors("S-Test", &s, "E2: switch on (record)", Some("change/e2-on"), &p(&["crates/cache/src/x.rs"]), t);
        assert!(on.is_empty(), "a promoted P row may land: {on:?}");
        assert!(pr_errors("S-Test", &s, "E1: region file", Some("change/region-file"), &p(&["xtask/src/x.rs"]), t).is_empty());
        let none = pr_errors("S-Test", &s, "tidy imports", Some("change/tidy"), &p(&["xtask/src/x.rs"]), t);
        assert!(none.iter().any(|m| m.contains("names none")), "{none:?}");
        let r_change = pr_errors("S-Test", &s, "E3: hump", Some("change/hump"), &p(&["docs/x.md"]), t);
        assert!(r_change.iter().any(|m| m.contains("names none")), "an R row does not open a change: {r_change:?}");
        assert!(pr_errors("S-Test", &s, "docs: typo", None, &p(&["docs/x.md"]), t).is_empty());
        let late = day("2026-12-06");
        assert!(pr_errors("S-Test", &s, "docs: typo", None, &p(&["docs/x.md"]), late).is_empty(), "a docs-only PR never fails");
        let code = pr_errors("S-Test", &s, "E1: more", Some("change/region-file"), &p(&["xtask/src/x.rs"]), late);
        assert!(code.iter().any(|m| m.contains("overdue")), "{code:?}");
        let closing =
            pr_errors("S-Test", &s, "E1: close", Some("change/region-file"), &p(&["xtask/src/x.rs", "docs/milestones/S-Test.md"]), late);
        assert!(closing.is_empty(), "{closing:?}");
    }
}
