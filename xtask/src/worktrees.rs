//! `cargo xtask worktrees [--prune] [--no-size]` (BUILD-039): every git worktree of the repository with its size (time-limited), branch, uncommitted
//! count, last commit and merged PR. `--prune` removes, through `git worktree remove` without `--force`, only the worktrees that are clean and whose
//! PR is merged (its head commit is the worktree's) or that are detached at an ancestor of `origin/main`; never the current worktree, never the
//! root, never one with uncommitted or unmerged work, never a locked one.

use crate::disk::{dir_size, GB};
use crate::util::{capture, Res};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `prepush` warns above this many worktrees.
pub const CROWDED_ABOVE: usize = 12;

/// One entry of `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub locked: bool,
    pub prunable: bool,
}

/// The entries of `git worktree list --porcelain`; the first is the root (the main worktree).
pub fn parse_porcelain(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for line in text.lines() {
        let (key, val) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "worktree" => out.push(Entry { path: val.to_string(), head: String::new(), branch: None, locked: false, prunable: false }),
            "HEAD" => {
                if let Some(e) = out.last_mut() {
                    e.head = val.to_string();
                }
            }
            "branch" => {
                if let Some(e) = out.last_mut() {
                    e.branch = Some(val.strip_prefix("refs/heads/").unwrap_or(val).to_string());
                }
            }
            "locked" => {
                if let Some(e) = out.last_mut() {
                    e.locked = true;
                }
            }
            "prunable" => {
                if let Some(e) = out.last_mut() {
                    e.prunable = true;
                }
            }
            _ => {}
        }
    }
    out
}

/// A path as one comparable string: forward slashes, no trailing slash, no `\\?\` prefix, lower case on Windows.
pub fn norm(p: &str) -> String {
    let s = p.replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).trim_end_matches('/').to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// What is known about a worktree when deciding.
#[derive(Debug, Clone)]
pub struct Info {
    pub entry: Entry,
    pub is_root: bool,
    pub is_current: bool,
    pub uncommitted: usize,
    pub last_commit: String,
    pub merged_pr: Option<u64>,
    pub head_in_main: bool,
    pub size: Option<(u64, bool)>,
    pub misplaced: bool,
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Remove(String),
    Keep(String),
}

/// The prune decision (BUILD-039): removable only when it is not the current or the root worktree, not locked, clean, and either its branch's merged
/// PR ended at its head commit or it is detached at an ancestor of `origin/main`.
pub fn prune_verdict(i: &Info) -> Verdict {
    let keep = |why: &str| Verdict::Keep(why.to_string());
    if i.is_root {
        return keep("the root");
    }
    if i.is_current {
        return keep("the current worktree");
    }
    if i.entry.locked {
        return keep("locked");
    }
    if i.uncommitted > 0 {
        return keep("uncommitted work");
    }
    match (&i.entry.branch, i.merged_pr, i.head_in_main) {
        (Some(_), Some(n), _) => Verdict::Remove(format!("PR {n} merged")),
        (Some(_), None, _) => keep("branch not merged"),
        (None, _, true) => Verdict::Remove("detached at an ancestor of origin/main".into()),
        (None, _, false) => keep("detached and not in origin/main"),
    }
}

/// The warning `prepush` prints for `count` worktrees: nothing up to [`CROWDED_ABOVE`], else the count and the prune command as a tracked line.
pub fn crowd_line(count: usize) -> Option<String> {
    (count > CROWDED_ABOVE).then(|| {
        format!(
            "prepush: WARNING {count} worktrees (above {CROWDED_ABOVE}); each holds a target/ folder. Tracked: run `{}` to remove the finished ones",
            crate::disk::PRUNE_COMMAND
        )
    })
}

pub fn list(root: &Path) -> Res<Vec<Entry>> {
    Ok(parse_porcelain(&capture(root, "git", &["worktree", "list", "--porcelain"])?))
}

/// Prints the crowd warning of [`crowd_line`] for this repository; silent when git cannot list.
pub fn warn_if_crowded(root: &Path) {
    if let Ok(entries) = list(root) {
        if let Some(l) = crowd_line(entries.len()) {
            eprintln!("{l}");
        }
    }
}

/// The merged PR of `branch` whose head is `head`, read with `gh`; `None` when there is none or `gh` is not usable.
fn merged_pr(root: &Path, branch: &str, head: &str) -> Option<u64> {
    let out =
        capture(root, "gh", &["pr", "list", "--head", branch, "--state", "merged", "--json", "number,headRefOid", "--limit", "20"]).ok()?;
    merged_pr_from_json(&out, head)
}

/// The number of the entry of a `gh pr list --json number,headRefOid` answer whose head commit is `head`.
pub fn merged_pr_from_json(json: &str, head: &str) -> Option<u64> {
    json.split('{').filter(|obj| obj.contains(&format!("\"headRefOid\":\"{head}\""))).find_map(|obj| {
        let rest = obj.split("\"number\":").nth(1)?;
        rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
    })
}

/// Everything the listing and the decision need for each worktree. `use_gh` reads the merged PRs, `size_budget` bounds each size walk.
pub fn collect(root: &Path, use_gh: bool, size_budget: Option<Duration>) -> Res<Vec<Info>> {
    let entries = list(root)?;
    let here = norm(&root.to_string_lossy());
    let main_ref_ok = capture(root, "git", &["rev-parse", "--verify", "--quiet", "origin/main"]).is_ok();
    let mut out = Vec::new();
    for (idx, e) in entries.iter().enumerate() {
        let p = PathBuf::from(&e.path);
        let exists = p.is_dir();
        let uncommitted = if exists { capture(&p, "git", &["status", "--porcelain"]).map(|s| s.lines().count()).unwrap_or(0) } else { 0 };
        let last_commit = if exists {
            capture(&p, "git", &["log", "-1", "--format=%cs %s"]).unwrap_or_default().trim().to_string()
        } else {
            String::new()
        };
        let merged = match (&e.branch, use_gh) {
            (Some(b), true) if idx > 0 => merged_pr(root, b, &e.head),
            _ => None,
        };
        let head_in_main =
            main_ref_ok && !e.head.is_empty() && capture(root, "git", &["merge-base", "--is-ancestor", &e.head, "origin/main"]).is_ok();
        let size = size_budget.filter(|_| exists).map(|b| dir_size(&p, Instant::now() + b));
        let n = norm(&e.path);
        out.push(Info {
            entry: e.clone(),
            is_root: idx == 0,
            is_current: n == here,
            uncommitted,
            last_commit,
            merged_pr: merged,
            head_in_main,
            size,
            misplaced: idx > 0 && !n.contains("/.claude/worktrees/"),
        });
    }
    Ok(out)
}

fn size_text(s: Option<(u64, bool)>) -> String {
    match s {
        None => "-".into(),
        Some((b, done)) => format!("{}{:.1} GB", if done { "" } else { ">" }, b as f64 / GB as f64),
    }
}

/// The listing, one line a worktree.
pub fn render(infos: &[Info]) -> String {
    let mut s = String::new();
    for i in infos {
        let who = i.entry.branch.clone().unwrap_or_else(|| format!("detached {}", i.entry.head.chars().take(8).collect::<String>()));
        let mut tags: Vec<String> = Vec::new();
        if i.is_root {
            tags.push("root".into());
        }
        if i.is_current {
            tags.push("current".into());
        }
        if i.entry.prunable {
            tags.push("directory missing".into());
        }
        if i.entry.locked {
            tags.push("locked".into());
        }
        if i.misplaced {
            tags.push("outside .claude/worktrees".into());
        }
        let pr = i.merged_pr.map(|n| format!("PR {n} merged")).unwrap_or_else(|| {
            if i.head_in_main {
                "in origin/main".into()
            } else {
                "no merged PR".into()
            }
        });
        s.push_str(&format!(
            "{} | {} | {} | {} uncommitted | {} | {}{}\n",
            i.entry.path,
            size_text(i.size),
            who,
            i.uncommitted,
            if i.last_commit.is_empty() { "-" } else { &i.last_commit },
            pr,
            if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) }
        ));
    }
    s
}

/// Removes what [`prune_verdict`] allows through `git worktree remove` (no `--force`); returns one line per worktree saying what was done.
pub fn prune(root: &Path, infos: &[Info]) -> Vec<String> {
    let mut lines = Vec::new();
    for i in infos {
        match prune_verdict(i) {
            Verdict::Keep(why) => lines.push(format!("kept {} ({why})", i.entry.path)),
            Verdict::Remove(why) => {
                let r = capture(root, "git", &["worktree", "remove", &i.entry.path]);
                lines.push(match r {
                    Ok(_) => format!("removed {} ({why})", i.entry.path),
                    Err(e) => format!("kept {} (git refused: {})", i.entry.path, e.lines().next().unwrap_or("").trim()),
                });
            }
        }
    }
    lines
}

pub fn run(root: &Path, args: &[String]) -> Res {
    let mut do_prune = false;
    let mut sizes = true;
    for a in args {
        match a.as_str() {
            "--prune" => do_prune = true,
            "--no-size" => sizes = false,
            other => return Err(format!("unknown option '{other}' (usage: cargo xtask worktrees [--prune] [--no-size])")),
        }
    }
    if do_prune {
        let _ = capture(root, "git", &["worktree", "prune"]);
    }
    let infos = collect(root, true, sizes.then(|| Duration::from_secs(5)))?;
    print!("{}", render(&infos));
    println!("{} worktrees", infos.len());
    if do_prune {
        for l in prune(root, &infos) {
            println!("{l}");
        }
    } else if infos.iter().any(|i| matches!(prune_verdict(i), Verdict::Remove(_))) {
        println!(
            "Tracked: `{}` would remove {} worktree(s)",
            crate::disk::PRUNE_COMMAND,
            infos.iter().filter(|i| matches!(prune_verdict(i), Verdict::Remove(_))).count()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(branch: Option<&str>) -> Entry {
        Entry { path: "C:/w/a".into(), head: "abc".into(), branch: branch.map(String::from), locked: false, prunable: false }
    }

    fn info(branch: Option<&str>) -> Info {
        Info {
            entry: entry(branch),
            is_root: false,
            is_current: false,
            uncommitted: 0,
            last_commit: String::new(),
            merged_pr: None,
            head_in_main: false,
            size: None,
            misplaced: false,
        }
    }

    // spec: BUILD-039
    #[test]
    fn the_porcelain_list_is_parsed_with_the_root_first() {
        let text = "worktree C:/DEV/root\nHEAD aaa\nbranch refs/heads/main\n\nworktree C:/DEV/root/.claude/worktrees/x\nHEAD bbb\nbranch refs/heads/change/x\nlocked reason\n\nworktree C:/tmp/y\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n";
        let v = parse_porcelain(text);
        assert_eq!(v.len(), 3);
        assert_eq!((v[0].path.as_str(), v[0].branch.as_deref()), ("C:/DEV/root", Some("main")));
        assert!(v[1].locked && v[1].branch.as_deref() == Some("change/x") && v[1].head == "bbb");
        assert!(v[2].branch.is_none() && v[2].prunable && !v[2].locked);
    }

    // spec: BUILD-039
    #[test]
    fn only_clean_merged_or_detached_ancestor_worktrees_are_removable() {
        let mut merged = info(Some("change/x"));
        merged.merged_pr = Some(77);
        assert_eq!(prune_verdict(&merged), Verdict::Remove("PR 77 merged".into()));

        let mut dirty = merged.clone();
        dirty.uncommitted = 1;
        assert_eq!(prune_verdict(&dirty), Verdict::Keep("uncommitted work".into()));

        assert_eq!(prune_verdict(&info(Some("change/y"))), Verdict::Keep("branch not merged".into()));

        let mut ancestor = info(None);
        ancestor.head_in_main = true;
        assert!(matches!(prune_verdict(&ancestor), Verdict::Remove(_)));
        assert_eq!(prune_verdict(&info(None)), Verdict::Keep("detached and not in origin/main".into()));

        let mut cur = merged.clone();
        cur.is_current = true;
        assert_eq!(prune_verdict(&cur), Verdict::Keep("the current worktree".into()));
        let mut root = merged.clone();
        root.is_root = true;
        assert_eq!(prune_verdict(&root), Verdict::Keep("the root".into()));
        let mut locked = merged.clone();
        locked.entry.locked = true;
        assert_eq!(prune_verdict(&locked), Verdict::Keep("locked".into()));
        // A branch with a merged PR that kept growing after the merge: the PR's head is not the worktree's head, so no PR is found for it.
        assert_eq!(merged_pr_from_json(r#"[{"headRefOid":"old","number":5}]"#, "abc"), None);
    }

    // spec: BUILD-039
    #[test]
    fn the_merged_pr_is_the_one_that_ended_at_the_worktrees_head() {
        let json = r#"[{"headRefOid":"zzz","number":41},{"headRefOid":"abc","number":1242}]"#;
        assert_eq!(merged_pr_from_json(json, "abc"), Some(1242));
        assert_eq!(merged_pr_from_json("[]", "abc"), None);
    }

    // spec: BUILD-039
    #[test]
    fn prepush_warns_above_twelve_worktrees_and_names_the_prune_command() {
        assert_eq!(crowd_line(12), None);
        let l = crowd_line(13).unwrap();
        assert!(l.contains("13 worktrees") && l.contains("Tracked") && l.contains("cargo xtask worktrees --prune"), "{l}");
    }

    // spec: BUILD-039
    #[test]
    fn paths_compare_across_slashes_and_the_extended_prefix() {
        assert_eq!(norm("C:\\DEV\\a\\"), norm("C:/DEV/a"));
        assert_eq!(norm("\\\\?\\C:\\DEV\\a"), norm("C:/DEV/a"));
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        capture(dir, "git", args).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }

    // spec: BUILD-039
    #[test]
    fn prune_in_a_real_repository_removes_the_finished_worktree_and_keeps_the_rest() {
        let base = std::env::temp_dir().join(format!("worktrees-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "t@example.com"]);
        git(&root, &["config", "user.name", "t"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        std::fs::write(root.join("a.txt"), "a").unwrap();
        git(&root, &["add", "a.txt"]);
        git(&root, &["commit", "-q", "-m", "one"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        let done = base.join("done");
        let dirty = base.join("dirty");
        let ahead = base.join("ahead");
        git(&root, &["worktree", "add", "-q", "--detach", done.to_str().unwrap(), "HEAD"]);
        git(&root, &["worktree", "add", "-q", "--detach", dirty.to_str().unwrap(), "HEAD"]);
        git(&root, &["worktree", "add", "-q", "--detach", ahead.to_str().unwrap(), "HEAD"]);
        std::fs::write(dirty.join("new.txt"), "wip").unwrap();
        std::fs::write(ahead.join("b.txt"), "b").unwrap();
        git(&ahead, &["add", "b.txt"]);
        git(&ahead, &["-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-q", "-m", "unmerged"]);

        let infos = collect(&root, false, None).unwrap();
        assert_eq!(infos.len(), 4);
        let lines = prune(&root, &infos);
        assert!(lines.iter().any(|l| l.starts_with("removed") && l.contains("done")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("kept") && l.contains("dirty") && l.contains("uncommitted")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("kept") && l.contains("ahead") && l.contains("not in origin/main")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("kept") && l.contains("the root")), "{lines:?}");
        assert!(!done.exists() && dirty.exists() && ahead.exists() && root.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
