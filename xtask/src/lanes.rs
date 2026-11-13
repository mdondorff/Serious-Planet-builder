//! `cargo xtask lanes` (BUILD-043): per lane the last commit of its worktree and its age, the open PR of its branch, the latest number in its
//! inbox file and the cargo or example processes that run for it. The lane list is `docs/lanes.txt` (names and worktree directory prefixes, no
//! machine paths). A missing piece prints `-`; the command never fails for one.

use crate::util::{capture, Res};
use crate::worktrees::{list, norm, Entry};
use std::path::{Path, PathBuf};

pub const LANES_FILE: &str = "docs/lanes.txt";

/// A lane of the config: its name and the prefix of its worktree directory.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub name: String,
    pub prefix: String,
}

/// The lanes of `docs/lanes.txt`: `name | prefix` lines; `#` lines and blank lines are skipped.
pub fn parse_lanes(text: &str) -> Vec<Lane> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (n, p) = l.split_once('|')?;
            (!n.trim().is_empty() && !p.trim().is_empty()).then(|| Lane { name: n.trim().into(), prefix: p.trim().into() })
        })
        .collect()
}

/// The last component of a path (either slash).
fn dir_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or("")
}

/// The worktree of a lane: the entry under `.claude/worktrees/` whose directory name starts with the lane's prefix (the first, in list order).
pub fn lane_worktree<'a>(lane: &Lane, entries: &'a [Entry]) -> Option<&'a Entry> {
    entries.iter().find(|e| norm(&e.path).contains("/.claude/worktrees/") && dir_name(&e.path).starts_with(&lane.prefix))
}

/// The highest `N` of the `## #N` headings of an inbox file.
pub fn latest_inbox_number(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(|l| l.strip_prefix("## #"))
        .filter_map(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok())
        .max()
}

/// `95 s`, `12 min`, `3 h`, `2 d`: an age in seconds as short text.
pub fn age_text(secs: u64) -> String {
    match secs {
        0..=119 => format!("{secs} s"),
        120..=7199 => format!("{} min", secs / 60),
        7200..=172_799 => format!("{} h", secs / 3600),
        _ => format!("{} d", secs / 86_400),
    }
}

/// A running process.
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub name: String,
    pub pid: u32,
    pub path: String,
    pub mem_mb: u64,
}

#[cfg_attr(not(windows), allow(dead_code))]
/// A line of CSV with quoted fields, split into its fields.
pub fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut cur, mut quoted) = (String::new(), false);
    for c in line.chars() {
        match (c, quoted) {
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

#[cfg_attr(not(windows), allow(dead_code))]
/// The processes of `Get-CimInstance Win32_Process | Select Name,ProcessId,ExecutablePath,WorkingSetSize | ConvertTo-Csv`.
pub fn parse_cim_csv(text: &str) -> Vec<Proc> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let f = csv_fields(l);
            if f.len() < 4 {
                return None;
            }
            Some(Proc {
                name: f[0].clone(),
                pid: f[1].parse().ok()?,
                path: f[2].clone(),
                mem_mb: f[3].parse::<u64>().unwrap_or(0) / (1 << 20),
            })
        })
        .collect()
}

#[cfg_attr(windows, allow(dead_code))]
/// The processes of `ps -eo pid=,rss=,args=` (the executable is the first word of the arguments).
pub fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let rss_kb: u64 = it.next()?.parse().ok()?;
            let exe = it.next()?.to_string();
            let name = dir_name(&exe).to_string();
            Some(Proc { name, pid, path: exe, mem_mb: rss_kb / 1024 })
        })
        .collect()
}

/// A cargo run or an example: `cargo`, `rustc`, `nextest` by name, or an executable under a `target` folder.
pub fn is_build_process(p: &Proc) -> bool {
    let n = p.name.to_lowercase();
    let n = n.trim_end_matches(".exe");
    let path = norm(&p.path);
    n == "cargo" || n == "rustc" || n.contains("nextest") || path.contains("/target/")
}

/// The lane a process belongs to: its executable path holds the lane's worktree directory.
pub fn process_lane<'a>(p: &Proc, lanes: &'a [(Lane, Option<Entry>)]) -> Option<&'a str> {
    let path = norm(&p.path);
    lanes.iter().find_map(|(l, e)| {
        let e = e.as_ref()?;
        path.starts_with(&format!("{}/", norm(&e.path))).then_some(l.name.as_str())
    })
}

#[cfg(windows)]
fn processes() -> Vec<Proc> {
    let script =
        "Get-CimInstance Win32_Process | Select-Object Name,ProcessId,ExecutablePath,WorkingSetSize | ConvertTo-Csv -NoTypeInformation";
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map(|o| parse_cim_csv(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn processes() -> Vec<Proc> {
    std::process::Command::new("ps")
        .args(["-eo", "pid=,rss=,args="])
        .output()
        .map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// The inbox directory: `PLANET_INBOX_DIR`, else `SeriousPlanetData/lanes` beside the main checkout (ADR 0121).
fn inbox_dir(main_root: &str) -> PathBuf {
    std::env::var_os("PLANET_INBOX_DIR").map(PathBuf::from).unwrap_or_else(|| default_inbox_dir(main_root))
}

fn default_inbox_dir(main_root: &str) -> PathBuf {
    Path::new(main_root).parent().map(|p| p.join("SeriousPlanetData").join("lanes")).unwrap_or_default()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The text of one lane's block.
pub fn render_lane(
    lane: &Lane,
    entry: Option<&Entry>,
    last_commit: Option<(u64, String)>,
    pr: Option<String>,
    inbox: Option<u32>,
    procs: &[&Proc],
    now: u64,
) -> String {
    let mut s = format!("{}\n", lane.name);
    let Some(e) = entry else {
        s.push_str(&format!("  worktree: - (no directory under .claude/worktrees/ starts with `{}`)\n", lane.prefix));
        s.push_str(&format!("  inbox: {}\n", inbox.map(|n| format!("#{n}")).unwrap_or_else(|| "-".into())));
        return s;
    };
    s.push_str(&format!("  worktree: {} ({})\n", dir_name(&e.path), e.branch.as_deref().unwrap_or("detached")));
    match last_commit {
        Some((t, subject)) => s.push_str(&format!(
            "  last commit: {} ago, {}\n",
            age_text(now.saturating_sub(t)),
            subject.chars().take(90).collect::<String>()
        )),
        None => s.push_str("  last commit: -\n"),
    }
    s.push_str(&format!("  open PR: {}\n", pr.unwrap_or_else(|| "-".into())));
    s.push_str(&format!("  inbox: {}\n", inbox.map(|n| format!("#{n}")).unwrap_or_else(|| "-".into())));
    if procs.is_empty() {
        s.push_str("  running: -\n");
    } else {
        for p in procs {
            s.push_str(&format!("  running: {} (pid {}, {} MB)\n", p.name, p.pid, p.mem_mb));
        }
    }
    s
}

pub fn run(root: &Path) -> Res {
    let text = std::fs::read_to_string(root.join(LANES_FILE)).map_err(|e| format!("cannot read {LANES_FILE}: {e}"))?;
    let entries = list(root).unwrap_or_default();
    let main_root = entries.first().map(|e| e.path.clone()).unwrap_or_else(|| root.to_string_lossy().into_owned());
    let inbox = inbox_dir(&main_root);
    let lanes: Vec<(Lane, Option<Entry>)> = parse_lanes(&text)
        .into_iter()
        .map(|l| {
            let e = lane_worktree(&l, &entries).cloned();
            (l, e)
        })
        .collect();
    let procs: Vec<Proc> = processes().into_iter().filter(is_build_process).collect();
    let now = now_secs();
    for (lane, entry) in &lanes {
        let (commit, pr) = match entry {
            Some(e) => {
                let p = PathBuf::from(&e.path);
                let commit = capture(&p, "git", &["log", "-1", "--format=%ct|%s"]).ok().and_then(|o| {
                    let (t, s) = o.trim().split_once('|')?;
                    Some((t.parse().ok()?, s.to_string()))
                });
                let pr = e.branch.as_deref().and_then(|b| {
                    let o = capture(
                        root,
                        "gh",
                        &[
                            "pr",
                            "list",
                            "--head",
                            b,
                            "--state",
                            "open",
                            "--json",
                            "number,title",
                            "--jq",
                            ".[] | \"#\\(.number) \\(.title)\"",
                        ],
                    )
                    .ok()?;
                    let first = o.lines().next()?.trim().to_string();
                    (!first.is_empty()).then_some(first)
                });
                (commit, pr)
            }
            None => (None, None),
        };
        let number = std::fs::read_to_string(inbox.join(format!("{}.md", lane.name))).ok().and_then(|t| latest_inbox_number(&t));
        let mine: Vec<&Proc> = procs.iter().filter(|p| process_lane(p, &lanes) == Some(lane.name.as_str())).collect();
        print!("{}", render_lane(lane, entry.as_ref(), commit, pr, number, &mine, now));
    }
    let loose: Vec<&Proc> = procs.iter().filter(|p| process_lane(p, &lanes).is_none()).collect();
    println!("not attributed to a lane");
    if loose.is_empty() {
        println!("  running: -");
    }
    for p in loose {
        println!("  running: {} (pid {}, {} MB) {}", p.name, p.pid, p.mem_mb, p.path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, branch: Option<&str>) -> Entry {
        Entry { path: path.into(), head: "abc".into(), branch: branch.map(String::from), locked: false, prunable: false }
    }

    // spec: BUILD-043
    #[test]
    fn the_default_inbox_is_the_lanes_folder_of_the_data_root_beside_the_checkout() {
        let d = default_inbox_dir("C:/DEV/Serious-Planet-Builder");
        assert_eq!(d, Path::new("C:/DEV").join("SeriousPlanetData").join("lanes"));
    }

    // spec: BUILD-043
    #[test]
    fn the_lane_list_skips_comments_and_reads_name_and_prefix() {
        let l = parse_lanes("# c\n\nexplorer | sonnet-explorer\ncrusader|sonnet-crusader\nbad line\n | x\n");
        assert_eq!(
            l,
            [
                Lane { name: "explorer".into(), prefix: "sonnet-explorer".into() },
                Lane { name: "crusader".into(), prefix: "sonnet-crusader".into() }
            ]
        );
        assert!(
            !std::fs::read_to_string(crate::util::repo_root().join(LANES_FILE)).unwrap().contains(":\\"),
            "no machine paths in the config"
        );
    }

    // spec: BUILD-043
    #[test]
    fn a_lane_finds_its_worktree_by_directory_prefix_under_claude_worktrees_only() {
        let es = [
            entry("C:/DEV/R", Some("main")),
            entry("C:/DEV/R/.claude/worktrees/sonnet-crusader-c1a4f2", Some("change/x")),
            entry("C:/Users/u/scratch/sonnet-crusader-old", None),
        ];
        let lane = Lane { name: "crusader".into(), prefix: "sonnet-crusader".into() };
        assert_eq!(lane_worktree(&lane, &es).map(|e| e.path.as_str()), Some("C:/DEV/R/.claude/worktrees/sonnet-crusader-c1a4f2"));
        assert!(lane_worktree(&Lane { name: "x".into(), prefix: "nope".into() }, &es).is_none());
    }

    // spec: BUILD-043
    #[test]
    fn the_latest_inbox_number_is_the_highest_heading_and_an_age_is_short_text() {
        assert_eq!(latest_inbox_number("## #9 a\n text\n## #94 2026\n## #93 b\n### #200 not\n"), Some(94));
        assert_eq!(latest_inbox_number("nothing"), None);
        assert_eq!([age_text(45), age_text(600), age_text(10_000), age_text(300_000)], ["45 s", "10 min", "2 h", "3 d"]);
    }

    // spec: BUILD-043
    #[test]
    fn build_processes_are_cargo_rustc_nextest_and_anything_under_a_target_folder() {
        let csv = "\"Name\",\"ProcessId\",\"ExecutablePath\",\"WorkingSetSize\"\n\
                   \"cargo.exe\",\"10\",\"C:\\Users\\u\\.cargo\\bin\\cargo.exe\",\"52428800\"\n\
                   \"landmass_scale.exe\",\"20\",\"C:\\DEV\\R\\.claude\\worktrees\\sonnet-explorer-1\\target\\release\\examples\\landmass_scale.exe\",\"8589934592\"\n\
                   \"chrome.exe\",\"30\",\"C:\\Program Files\\chrome.exe\",\"1048576\"\n\
                   \"System\",\"4\",\"\",\"0\"\n";
        let all = parse_cim_csv(csv);
        assert_eq!(all.len(), 4);
        let build: Vec<&str> = all.iter().filter(|p| is_build_process(p)).map(|p| p.name.as_str()).collect();
        assert_eq!(build, ["cargo.exe", "landmass_scale.exe"]);
        assert_eq!(all[1].mem_mb, 8192);
        let lanes = vec![(
            Lane { name: "explorer".into(), prefix: "sonnet-explorer".into() },
            Some(entry("C:/DEV/R/.claude/worktrees/sonnet-explorer-1", None)),
        )];
        assert_eq!(process_lane(&all[1], &lanes), Some("explorer"));
        assert_eq!(process_lane(&all[0], &lanes), None);
        let ps = parse_ps("  12 2048 /home/u/R/.claude/worktrees/a/target/release/x --flag\n  13 100 /usr/bin/cargo build\n");
        assert!(ps.iter().all(is_build_process) && ps[0].mem_mb == 2);
    }

    // spec: BUILD-043
    #[test]
    fn a_lane_block_prints_dashes_for_what_is_missing() {
        let lane = Lane { name: "spikes".into(), prefix: "sonnet-spikes".into() };
        let none = render_lane(&lane, None, None, None, Some(7), &[], 0);
        assert!(none.contains("worktree: - (no directory") && none.contains("inbox: #7"), "{none}");
        let e = entry("C:/DEV/R/.claude/worktrees/sonnet-spikes-1", Some("docs/x"));
        let p = Proc { name: "cargo.exe".into(), pid: 5, path: String::new(), mem_mb: 50 };
        let full = render_lane(&lane, Some(&e), Some((1000, "a subject".into())), Some("#12 title".into()), None, &[&p], 4600);
        assert!(
            full.contains("sonnet-spikes-1 (docs/x)") && full.contains("60 min ago, a subject") && full.contains("open PR: #12 title"),
            "{full}"
        );
        assert!(full.contains("inbox: -") && full.contains("running: cargo.exe (pid 5, 50 MB)"), "{full}");
        let bare = render_lane(&lane, Some(&e), None, None, None, &[], 0);
        assert!(bare.contains("last commit: -") && bare.contains("open PR: -") && bare.contains("running: -"), "{bare}");
    }
}
