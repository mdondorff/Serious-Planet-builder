//! `cargo xtask ci-stats [--runs N]`: the CI time of the last N completed runs on `main` (BUILD-014), read with `gh`. Median and p90
//! of each job's duration; a job whose median exceeds [`CI_MEDIAN_BUDGET_SECS`] is flagged. A warning, never a failure: an
//! exceeded budget becomes a Tracked item in `docs/status.md`.

use crate::util::{capture, Res};
use std::collections::BTreeMap;
use std::path::Path;

/// The owner's budget (2026-10-18): the median CI job takes at most 16 minutes.
pub const CI_MEDIAN_BUDGET_SECS: f64 = 16.0 * 60.0;
/// Completed main runs read when `--runs` is not given.
pub const DEFAULT_RUNS: usize = 20;

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the Unix epoch of a GitHub timestamp `YYYY-MM-DDTHH:MM:SSZ`.
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim().strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (y, mo, da) = (d.next()??, d.next()??, d.next()??);
    let (h, mi, se) = (t.next()??, t.next()??, t.next()??);
    if d.next().is_some() || t.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    Some(days_from_civil(y, mo, da) * 86_400 + h * 3600 + mi * 60 + se)
}

/// The duration in seconds of one job line `name<TAB>startedAt<TAB>completedAt<TAB>conclusion`, for the jobs that ran to a verdict
/// (`success` or `failure`; skipped and cancelled jobs say nothing about the time).
pub fn job_seconds(line: &str) -> Option<(String, f64)> {
    let mut f = line.split('\t');
    let (name, start, end, conclusion) = (f.next()?, f.next()?, f.next()?, f.next()?.trim());
    if !matches!(conclusion, "success" | "failure") {
        return None;
    }
    let secs = parse_iso(end)? - parse_iso(start)?;
    (secs >= 0).then(|| (name.to_string(), secs as f64))
}

/// Nearest-rank percentile (`p` in 0..=100) of `values`, which need not be sorted.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    v[rank.min(v.len()) - 1]
}

/// The report: one row per job name with the number of runs, median and p90 in minutes, and a flag line for each median over the
/// budget. The second value is true when any median is over.
pub fn report(jobs: &BTreeMap<String, Vec<f64>>, runs: usize) -> (String, bool) {
    let mut out =
        format!("CI time of the last {runs} completed runs on main (budget: median at most {:.0} min)\n", CI_MEDIAN_BUDGET_SECS / 60.0);
    let mut over = false;
    for (name, secs) in jobs.iter().filter(|(_, s)| !s.is_empty()) {
        let (median, p90) = (percentile(secs, 50.0), percentile(secs, 90.0));
        out.push_str(&format!("  {name}: median {:.1} min, p90 {:.1} min ({} runs)\n", median / 60.0, p90 / 60.0, secs.len()));
        if median > CI_MEDIAN_BUDGET_SECS {
            over = true;
            out.push_str(&format!(
                "BUDGET EXCEEDED: CI median of '{name}' {:.1} min > {:.0} min; make it a Tracked item in docs/status.md (BUILD-014)\n",
                median / 60.0,
                CI_MEDIAN_BUDGET_SECS / 60.0
            ));
        }
    }
    (out, over)
}

/// Groups job lines (see [`job_seconds`]) by job name.
pub fn group(lines: &[String]) -> BTreeMap<String, Vec<f64>> {
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for (name, secs) in lines.iter().filter_map(|l| job_seconds(l)) {
        out.entry(name).or_default().push(secs);
    }
    out
}

/// Where `--record` keeps the per-job CI evidence (BUILD-020), relative to the repository root.
pub const HISTORY: &str = "docs/perf/ci-history.tsv";
/// The most runs one `gh run list` call returns for `--record`; a range that reaches it is cut off and the tool says so.
const RECORD_LIMIT: usize = 1000;
const HISTORY_HEADER: &str = "run\tcreated\tevent\tbranch\tjob\tconclusion\tseconds\tfailed_steps";

/// The history rows of one run: one per job line `name<TAB>start<TAB>end<TAB>conclusion<TAB>failed steps` that ran to a verdict.
pub fn history_rows(run: &str, created: &str, event: &str, branch: &str, job_lines: &[String]) -> Vec<String> {
    job_lines
        .iter()
        .filter_map(|l| {
            let (name, secs) = job_seconds(l)?;
            let mut f = l.split('\t').skip(3);
            let conclusion = f.next()?.trim();
            let failed = f.next().unwrap_or("").trim();
            Some(format!("{run}\t{created}\t{event}\t{branch}\t{name}\t{conclusion}\t{secs:.0}\t{failed}"))
        })
        .collect()
}

/// The run ids already in a history file (its first column, without the header).
pub fn recorded_runs(text: &str) -> std::collections::HashSet<String> {
    text.lines().skip(1).filter_map(|l| l.split('\t').next()).filter(|id| !id.is_empty()).map(String::from).collect()
}

/// The ids of `ids` that are not recorded yet, in order.
pub fn new_runs(known: &std::collections::HashSet<String>, ids: &[String]) -> Vec<String> {
    ids.iter().filter(|id| !known.contains(*id)).cloned().collect()
}

/// `cargo xtask ci-stats --record --since D [--until D]`: appends the jobs of every completed run created in the range to [`HISTORY`].
fn record(root: &Path, since: &str, until: Option<&str>) -> Res {
    if parse_iso(&format!("{since}T00:00:00Z")).is_none() || until.is_some_and(|u| parse_iso(&format!("{u}T00:00:00Z")).is_none()) {
        return Err("--since and --until take a date YYYY-MM-DD".into());
    }
    let created = format!("{since}..{}", until.unwrap_or("*"));
    let list = capture(
        root,
        "gh",
        &[
            "run",
            "list",
            "--status",
            "completed",
            "--created",
            &created,
            "--limit",
            &RECORD_LIMIT.to_string(),
            "--json",
            "databaseId,createdAt,event,headBranch",
            "--jq",
            ".[] | [.databaseId, .createdAt, .event, .headBranch] | @tsv",
        ],
    )?;
    let path = root.join(HISTORY);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let known = recorded_runs(&existing);
    let runs: Vec<Vec<&str>> = list.lines().map(|l| l.split('\t').collect()).filter(|f: &Vec<&str>| f.len() == 4).collect();
    let ids: Vec<String> = runs.iter().map(|f| f[0].to_string()).collect();
    let fresh = new_runs(&known, &ids);
    let mut out = if existing.is_empty() { format!("{HISTORY_HEADER}\n") } else { existing };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let mut rows = 0;
    for f in runs.iter().filter(|f| fresh.iter().any(|i| i == f[0])) {
        let tsv = capture(
            root,
            "gh",
            &[
                "run",
                "view",
                f[0],
                "--json",
                "jobs",
                "--jq",
                ".jobs[] | [.name, .startedAt, .completedAt, .conclusion, ([.steps[] | select(.conclusion == \"failure\") | .name] | join(\"+\"))] | @tsv",
            ],
        )?;
        let jobs: Vec<String> = tsv.lines().map(String::from).collect();
        for row in history_rows(f[0], f[1], f[2], f[3], &jobs) {
            out.push_str(&row);
            out.push('\n');
            rows += 1;
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, out).map_err(|e| format!("{}: {e}", path.display()))?;
    if ids.len() >= RECORD_LIMIT {
        eprintln!("warning: gh returned {RECORD_LIMIT} runs, its limit: the range is cut off; record it again in shorter ranges (--since D --until D)");
    }
    println!("ci-stats --record: {} runs in range, {} new, {rows} job rows added to {HISTORY}", ids.len(), fresh.len());
    Ok(())
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let mut runs = DEFAULT_RUNS;
    let (mut recording, mut since, mut until): (bool, Option<String>, Option<String>) = (false, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs" => {
                runs = it.next().and_then(|v| v.parse().ok()).filter(|n| *n > 0).ok_or("--runs needs a positive number")?;
            }
            "--record" => recording = true,
            "--since" => since = Some(it.next().ok_or("--since needs a date YYYY-MM-DD")?.clone()),
            "--until" => until = Some(it.next().ok_or("--until needs a date YYYY-MM-DD")?.clone()),
            other => {
                return Err(format!(
                    "unknown ci-stats argument '{other}'\n(usage: cargo xtask ci-stats [--runs N] | ci-stats --record --since YYYY-MM-DD [--until YYYY-MM-DD])"
                ))
            }
        }
    }
    if recording {
        let since = since.ok_or("ci-stats --record needs --since YYYY-MM-DD")?;
        return record(root, &since, until.as_deref());
    }
    let limit = runs.to_string();
    let ids = capture(
        root,
        "gh",
        &["run", "list", "--branch", "main", "--status", "completed", "--limit", &limit, "--json", "databaseId", "--jq", ".[].databaseId"],
    )?;
    let ids: Vec<&str> = ids.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if ids.is_empty() {
        return Err("no completed runs on main found by `gh run list`".into());
    }
    let mut lines = Vec::new();
    for id in &ids {
        let tsv = capture(
            root,
            "gh",
            &["run", "view", id, "--json", "jobs", "--jq", ".jobs[] | [.name, .startedAt, .completedAt, .conclusion] | @tsv"],
        )?;
        lines.extend(tsv.lines().map(String::from));
    }
    let (text, _over) = report(&group(&lines), ids.len());
    print!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-014
    #[test]
    fn github_timestamps_parse_to_epoch_seconds_and_jobs_give_their_duration() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2026-10-18T20:30:15Z"), Some(1_792_355_415));
        assert_eq!(parse_iso("2000-02-29T23:59:59Z"), Some(951_782_400 + 86_399));
        assert_eq!(parse_iso("not a time"), None);
        assert_eq!(parse_iso("2026-13-18T20:30:15Z"), None);
        let ok = "windows (WARP)\t2026-10-18T17:00:00Z\t2026-10-18T17:21:09Z\tsuccess";
        assert_eq!(job_seconds(ok), Some(("windows (WARP)".to_string(), 1269.0)));
        assert!(job_seconds("x\t2026-10-18T17:00:00Z\t2026-10-18T17:00:05Z\tfailure").is_some(), "a failed job took real time");
        assert_eq!(job_seconds("x\t2026-10-18T17:00:00Z\t2026-10-18T17:00:05Z\tcancelled"), None);
        assert_eq!(job_seconds("x\t2026-10-18T17:00:00Z\t2026-10-18T17:00:05Z\tskipped"), None);
        assert_eq!(job_seconds("x\t\t\tsuccess"), None, "a job with no times");
    }

    // spec: BUILD-020
    #[test]
    fn a_run_gives_one_row_per_job_with_a_verdict_and_a_recorded_run_is_skipped() {
        let jobs: Vec<String> = [
            "linux (lavapipe) A+B\t2026-10-19T10:00:00Z\t2026-10-19T10:05:00Z\tsuccess\t",
            "windows (WARP) C 1/2\t2026-10-19T10:00:00Z\t2026-10-19T10:01:00Z\tfailure\ttest gpu (tier C, software adapter)",
            "docs (spec-lint)\t2026-10-19T10:00:00Z\t2026-10-19T10:00:00Z\tskipped\t",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let rows = history_rows("101", "2026-10-19T09:59:00Z", "pull_request", "change/x", &jobs);
        assert_eq!(
            rows,
            [
                "101\t2026-10-19T09:59:00Z\tpull_request\tchange/x\tlinux (lavapipe) A+B\tsuccess\t300\t",
                "101\t2026-10-19T09:59:00Z\tpull_request\tchange/x\twindows (WARP) C 1/2\tfailure\t60\ttest gpu (tier C, software adapter)",
            ]
        );
        let file = format!("{HISTORY_HEADER}\n99\t2026-10-18T09:00:00Z\tpush\tmain\tdocs\tsuccess\t30\t\n");
        let known = recorded_runs(&file);
        assert!(known.contains("99") && !known.contains("run") && !known.contains("101"), "{known:?}");
        assert_eq!(new_runs(&known, &["99".to_string(), "101".to_string()]), ["101"], "run 99 is skipped");
    }

    // spec: BUILD-014
    #[test]
    fn the_median_and_p90_are_nearest_rank_and_a_median_over_16_minutes_is_flagged() {
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        assert_eq!((percentile(&v, 50.0), percentile(&v, 90.0), percentile(&[7.0], 90.0)), (5.0, 9.0, 7.0));
        let mut jobs = BTreeMap::new();
        // 5 runs of 15, 15, 15, 20, 30 min: median 15 min (within), p90 30 min.
        jobs.insert("linux (lavapipe)".to_string(), [15.0, 15.0, 15.0, 20.0, 30.0].iter().map(|m| m * 60.0).collect());
        let (text, over) = report(&jobs, 5);
        assert!(!over && text.contains("linux (lavapipe): median 15.0 min, p90 30.0 min (5 runs)"), "{text}");
        assert!(!text.contains("BUDGET EXCEEDED"), "{text}");
        // 5 runs of 14, 17, 18, 19, 25 min: median 18 min (over).
        jobs.insert("windows (WARP)".to_string(), [14.0, 17.0, 18.0, 19.0, 25.0].iter().map(|m| m * 60.0).collect());
        let (text, over) = report(&jobs, 5);
        assert!(over, "{text}");
        assert!(text.contains("BUDGET EXCEEDED: CI median of 'windows (WARP)' 18.0 min > 16 min"), "{text}");
        assert_eq!(text.matches("BUDGET EXCEEDED").count(), 1, "only the job over budget is flagged: {text}");
    }

    // spec: BUILD-014
    #[test]
    fn job_lines_are_grouped_by_name_over_runs() {
        let lines: Vec<String> = [
            "a\t2026-10-18T17:00:00Z\t2026-10-18T17:10:00Z\tsuccess",
            "b\t2026-10-18T17:00:00Z\t2026-10-18T17:20:00Z\tsuccess",
            "a\t2026-10-18T18:00:00Z\t2026-10-18T18:12:00Z\tsuccess",
            "a\t2026-10-18T19:00:00Z\t2026-10-18T19:01:00Z\tskipped",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let g = group(&lines);
        assert_eq!(g["a"], [600.0, 720.0]);
        assert_eq!(g["b"], [1200.0]);
    }
}
