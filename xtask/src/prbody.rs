//! `cargo xtask pr-body-check` (BUILD-044): the `pr-gate` check that a PR with code in it carries `Verifier: PASS <sha>` in its body, the sha a
//! commit of the PR. It reads `PR_BODY`, `PR_TITLE`, `HEAD_REF`, `BASE_SHA` and `HEAD_SHA` from the environment, so the body never reaches a
//! shell command line. A docs-only PR (the paths the `ci` workflow ignores) is exempt from the verifier line.
//!
//! BUILD-075 adds the review lines: `Numerics: PASS <sha>` when a code PR changes `crates/core/`, `crates/generators/` or a `*.wgsl`;
//! `Spec-Guardian: PASS <sha>` when a code PR changes `openspec/`; `Research-Review: READY <sha>` and `Owner-Look: yes|no` when any PR adds or
//! changes a record; `Sprint: <M>` while a sprint file is active; then the active sprint's R/P firewall (`cargo xtask plan --pr`, BUILD-074).

use crate::util::{capture, Res};
use std::path::Path;

/// The paths `ci.yml` ignores (`paths-ignore`); a PR whose every changed path matches one is docs-only. A test keeps this list equal to the workflow's.
#[cfg(test)]
pub const DOCS_PATTERNS: [&str; 7] =
    ["docs/**", "openspec/**", ".claude/**", ".agents/**", "*.md", "spikes/**/results/**", "spikes/README.md"];

/// True when `path` matches one of [`DOCS_PATTERNS`].
pub fn is_docs_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.starts_with("docs/")
        || p.starts_with("openspec/")
        || p.starts_with(".claude/")
        || p.starts_with(".agents/")
        || (!p.contains('/') && p.ends_with(".md"))
        || (p.starts_with("spikes/") && p.contains("/results/"))
        || p == "spikes/README.md"
}

/// The sha of the first `<prefix><sha>` line of a PR body (at least 7 hex digits), lower case.
pub fn stamp_sha(body: &str, prefix: &str) -> Option<String> {
    body.lines().find_map(|l| {
        let rest = l.trim().strip_prefix(prefix)?;
        let sha: String = rest.trim().chars().take_while(char::is_ascii_hexdigit).collect();
        (7..=64).contains(&sha.len()).then(|| sha.to_lowercase())
    })
}

/// The sha of the first `Verifier: PASS <sha>` line of a PR body (at least 7 hex digits), lower case.
pub fn verifier_sha(body: &str) -> Option<String> {
    stamp_sha(body, "Verifier: PASS ")
}

/// A review stamp by path: the stamp's prefix, which changed path needs it, the reviewer to run.
type PathStamp = (&'static str, fn(&str) -> bool, &'static str);

/// The review stamps by path (BUILD-075).
const PATH_STAMPS: [PathStamp; 2] = [
    (
        "Numerics: PASS ",
        |p| p.starts_with("crates/core/") || p.starts_with("crates/generators/") || p.ends_with(".wgsl"),
        "the numerics-reviewer agent",
    ),
    ("Spec-Guardian: PASS ", |p| p.starts_with("openspec/"), "the spec-guardian agent"),
];

/// Whether a changed path is a record (BUILD-075): a promotion or park record `docs/milestones/<M>/<E>-record.md`, or a research package
/// `spikes/<name>/results/results.md` whose `Decision:` line says `promote`; `text` is the file at the PR's head (`None` when deleted).
pub fn is_record(path: &str, text: Option<&str>) -> bool {
    let p = path.replace('\\', "/");
    let Some(text) = text else { return false };
    if p.starts_with("docs/milestones/") && p.ends_with("-record.md") && p.matches('/').count() == 3 {
        return true;
    }
    p.starts_with("spikes/")
        && p.ends_with("/results/results.md")
        && text.lines().any(|l| l.trim_start().strip_prefix("Decision:").is_some_and(|d| d.trim_start().starts_with("promote")))
}

/// The `Owner-Look:` verdict of a PR body: `yes` or `no` as the first word (the date and the sheet follow).
pub fn owner_look(body: &str) -> Option<&'static str> {
    body.lines().find_map(|l| {
        let v = l.trim().strip_prefix("Owner-Look:")?.trim_start();
        let word: String = v.chars().take_while(char::is_ascii_alphabetic).collect();
        match word.as_str() {
            "yes" => Some("yes"),
            "no" => Some("no"),
            _ => None,
        }
    })
}

/// The milestone a `Sprint: <M>` line of the body names.
pub fn sprint_line(body: &str) -> Option<&str> {
    body.lines().find_map(|l| l.trim().strip_prefix("Sprint:").map(str::trim).and_then(|v| v.split_whitespace().next()))
}

/// The review lines of BUILD-075 beyond the verifier line: the problems, one line each. `sprint` is the active sprint file's name (none in the
/// old milestone form, a draft or between sprints); `read` gives a changed file's text at the PR's head.
pub fn review_errors(
    body: &str,
    commits: &[String],
    changed: &[String],
    sprint: Option<&str>,
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut errs = Vec::new();
    let names_commit = |sha: &str| commits.iter().any(|c| c.starts_with(sha));
    let mut need = |prefix: &str, why: String, how: &str| match stamp_sha(body, prefix) {
        None => {
            errs.push(format!("{why}, and the body has no `{prefix}<sha>` line: run {how} and add the line with the commit it reviewed"))
        }
        Some(sha) if !names_commit(&sha) => errs.push(format!("`{prefix}{sha}` names no commit of this PR: name the commit {how} ran on")),
        Some(_) => {}
    };
    if !changed.iter().all(|p| is_docs_path(p)) {
        for (prefix, needs, how) in PATH_STAMPS {
            let hit: Vec<&str> = changed.iter().map(String::as_str).filter(|p| needs(&p.replace('\\', "/"))).take(3).collect();
            if !hit.is_empty() {
                need(prefix, format!("the PR changes {}", hit.join(", ")), how);
            }
        }
    }
    let records: Vec<&str> = changed.iter().map(String::as_str).filter(|p| is_record(p, read(p).as_deref())).take(3).collect();
    if !records.is_empty() {
        need("Research-Review: READY ", format!("the PR adds or changes the record {}", records.join(", ")), "the research-review agent");
        if owner_look(body).is_none() {
            errs.push(format!(
                "the PR adds or changes the record {}, and the body has no `Owner-Look: yes|no (<date>, <sheet>)` line: add the owner's look verdict",
                records.join(", ")
            ));
        }
    }
    if let Some(m) = sprint {
        match sprint_line(body) {
            None => errs.push(format!("the sprint {m} is active, and the body has no `Sprint: {m}` line")),
            Some(s) if s != m => errs.push(format!("`Sprint: {s}` is not the active sprint {m}")),
            Some(_) => {}
        }
    }
    errs
}

/// The active sprint file's name (`docs/milestones/<M>.md` whose `Status:` starts with `active` and that has the sprint form); none in the old
/// milestone form, a draft or between sprints.
pub fn active_sprint(root: &Path) -> Option<String> {
    let rd = std::fs::read_dir(root.join("docs/milestones")).ok()?;
    let mut active: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            (crate::plancmd::is_active(&text) && crate::sprintfile::is_sprint_file(&text))
                .then(|| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
        })
        .collect();
    active.sort();
    active.into_iter().next()
}

/// The verdict for a PR: `Ok` with a note, or the refusal. `commits` are the full shas of `BASE..HEAD`, newest first; `changed` the changed paths.
pub fn check(body: &str, commits: &[String], changed: &[String]) -> Result<String, String> {
    if changed.iter().all(|p| is_docs_path(p)) {
        return Ok(format!("docs-only PR ({} path(s)): the verifier line is not required", changed.len()));
    }
    let code: Vec<&str> = changed.iter().filter(|p| !is_docs_path(p)).map(String::as_str).take(3).collect();
    let Some(sha) = verifier_sha(body) else {
        return Err(format!(
            "the PR changes code (e.g. {}) and its body has no `Verifier: PASS <sha>` line: run the verifier agent and add the line with the commit it verified",
            code.join(", ")
        ));
    };
    let Some(at) = commits.iter().position(|c| c.starts_with(&sha)) else {
        return Err(format!(
            "`Verifier: PASS {sha}` names no commit of this PR (its {} commit(s) are base..head): name the commit the verifier ran on",
            commits.len()
        ));
    };
    Ok(if at == 0 {
        format!("Verifier: PASS {sha} is the head of the PR")
    } else {
        format!("Verifier: PASS {sha} is a commit of the PR; {at} later commit(s) came after it (re-verify if they change code)")
    })
}

fn env(name: &str) -> Res<String> {
    std::env::var(name).map_err(|_| format!("{name} is not set (pr-body-check reads PR_BODY, PR_TITLE, HEAD_REF, BASE_SHA and HEAD_SHA)"))
}

pub fn run(root: &Path) -> Res {
    let body = std::env::var("PR_BODY").unwrap_or_default();
    let (base, head, title) = (env("BASE_SHA")?, env("HEAD_SHA")?, env("PR_TITLE")?);
    let commits: Vec<String> = capture(root, "git", &["rev-list", &format!("{base}..{head}")])?.lines().map(str::to_string).collect();
    let diff = capture(root, "git", &["diff", "--name-only", "--no-renames", &format!("{base}...{head}")])?;
    let changed: Vec<String> = diff.lines().map(str::to_string).collect();
    if changed.is_empty() {
        println!("pr-body-check: no changed path, nothing to check");
        return Ok(());
    }
    let read = |p: &str| capture(root, "git", &["show", &format!("{head}:{p}")]).ok();
    let branch = std::env::var("HEAD_REF").ok();
    let problems = problems(root, &body, &commits, &changed, &title, branch.as_deref(), &read)?;
    if problems.is_empty() {
        let sprint = active_sprint(root);
        println!("pr-body-check: review lines ok ({})", sprint.map_or("no active sprint".to_string(), |m| format!("sprint {m}")));
        Ok(())
    } else {
        Err(format!("pr-body-check: {} problem(s):\n  {}", problems.len(), problems.join("\n  ")))
    }
}

/// Every problem of a PR, one line each: the verifier line (BUILD-044), the review lines (BUILD-075), then the active sprint's R/P firewall
/// (`plan --pr`, BUILD-074) on the title, the branch, the body and the changed paths.
pub fn problems(
    root: &Path,
    body: &str,
    commits: &[String],
    changed: &[String],
    title: &str,
    branch: Option<&str>,
    read: &dyn Fn(&str) -> Option<String>,
) -> Res<Vec<String>> {
    let mut problems = Vec::new();
    match check(body, commits, changed) {
        Ok(note) => println!("pr-body-check: {note}"),
        Err(e) => problems.push(e),
    }
    problems.extend(review_errors(body, commits, changed, active_sprint(root).as_deref(), read));
    static CALL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let tag = format!("{}-{}", std::process::id(), CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let tmp = std::env::temp_dir();
    let (body_file, files_file) = (tmp.join(format!("pr-body-{tag}.md")), tmp.join(format!("pr-files-{tag}.txt")));
    std::fs::write(&body_file, body).map_err(|e| format!("{}: {e}", body_file.display()))?;
    std::fs::write(&files_file, changed.join("\n")).map_err(|e| format!("{}: {e}", files_file.display()))?;
    let mut args = vec![title.to_string()];
    if let Some(b) = branch {
        args.extend(["--branch".to_string(), b.to_string()]);
    }
    args.extend(["--body-file".to_string(), body_file.display().to_string(), "--files".to_string(), files_file.display().to_string()]);
    let firewall = crate::plancmd::pr_check(root, &args);
    let _ = (std::fs::remove_file(&body_file), std::fs::remove_file(&files_file));
    if let Err(e) = firewall {
        problems.push(e);
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    // spec: BUILD-044
    #[test]
    fn docs_paths_are_those_the_ci_workflow_ignores() {
        for p in [
            "docs/status.md",
            "openspec/specs/x/spec.md",
            ".claude/agents/a.md",
            ".agents/hooks.json",
            "CLAUDE.md",
            "GEMINI.md",
            "spikes/s/results/r.txt",
            "spikes/README.md",
        ] {
            assert!(is_docs_path(p), "{p}");
        }
        for p in ["crates/a/src/lib.rs", "xtask/src/main.rs", "Cargo.toml", ".github/workflows/ci.yml", "spikes/s/run.rs", "sub/README.md"]
        {
            assert!(!is_docs_path(p), "{p}");
        }
        // The list stays the one of the `ci` workflow.
        let ci = std::fs::read_to_string(crate::util::repo_root().join(".github/workflows/ci.yml")).unwrap();
        let line = ci.lines().find(|l| l.contains("paths-ignore")).unwrap();
        for pat in DOCS_PATTERNS {
            assert!(line.contains(&format!("\"{pat}\"")), "{pat} is not in `{line}`");
        }
    }

    // spec: BUILD-044
    #[test]
    fn the_verifier_line_is_read_from_the_body() {
        assert_eq!(verifier_sha("text\n\nVerifier: PASS ABCDEF1234\nmore").as_deref(), Some("abcdef1234"));
        assert_eq!(verifier_sha("Verifier: PASS abc12"), None, "fewer than 7 digits");
        assert_eq!(verifier_sha("Verifier: FAIL abcdef1"), None);
        assert_eq!(verifier_sha("verifier pass abcdef1"), None);
        assert_eq!(verifier_sha("Verifier: PASS e6a45cd0d00e71c4f687555866f294355b2d4bb8 (note)").map(|s| s.len()), Some(40));
    }

    // spec: BUILD-044
    #[test]
    fn a_code_pr_needs_the_line_and_a_docs_pr_does_not() {
        let commits = paths(&[B, A]);
        let code = paths(&["docs/a.md", "crates/x/src/lib.rs"]);
        let e = check("no line here", &commits, &code).unwrap_err();
        assert!(e.contains("crates/x/src/lib.rs") && e.contains("Verifier: PASS <sha>"), "{e}");
        assert!(check("", &commits, &paths(&["docs/a.md", "CLAUDE.md"])).unwrap().contains("docs-only"));
        assert!(check("", &commits, &paths(&["docs/a.md", "xtask/src/x.rs"])).is_err(), "one code path makes it a code PR");
    }

    // spec: BUILD-044
    #[test]
    fn the_sha_must_be_a_commit_of_the_pr() {
        let commits = paths(&[B, A]);
        let code = paths(&["crates/x/src/lib.rs"]);
        assert!(check(&format!("Verifier: PASS {}", &B[..8]), &commits, &code).unwrap().contains("head of the PR"));
        assert!(check(&format!("Verifier: PASS {A}"), &commits, &code).unwrap().contains("1 later commit"));
        let e = check("Verifier: PASS cccccccc", &commits, &code).unwrap_err();
        assert!(e.contains("names no commit of this PR") && e.contains("2 commit(s)"), "{e}");
        assert!(check(&format!("Verifier: PASS {A}"), &[], &code).is_err(), "a PR with no commits names none");
    }

    /// A reader of head files for [`review_errors`]: every path has the text `t`, except the ones in `gone` (deleted).
    fn files<'a>(t: &'a str, gone: &'a [&'a str]) -> impl Fn(&str) -> Option<String> + 'a {
        move |p: &str| (!gone.contains(&p)).then(|| t.to_string())
    }

    // spec: BUILD-075, TEST-162
    #[test]
    fn the_path_stamps_are_needed_on_code_prs_by_path_and_name_a_commit_of_the_pr() {
        let commits = paths(&[B, A]);
        let none = files("", &[]);
        // numerics: crates/core, crates/generators, any .wgsl
        for p in ["crates/core/src/coord.rs", "crates/generators/src/erosion.rs", "crates/render/src/terrain.wgsl"] {
            let e = review_errors("", &commits, &paths(&[p]), None, &none);
            assert!(
                e.len() == 1 && e[0].contains(p) && e[0].contains("Numerics: PASS <sha>") && e[0].contains("numerics-reviewer"),
                "{e:?}"
            );
            assert!(review_errors(&format!("Numerics: PASS {}", &A[..9]), &commits, &paths(&[p]), None, &none).is_empty());
        }
        let e = review_errors("Numerics: PASS cccccccc", &commits, &paths(&["crates/core/src/a.rs"]), None, &none);
        assert!(e.len() == 1 && e[0].contains("names no commit of this PR"), "{e:?}");
        // spec-guardian: openspec/ in a code PR
        let mixed = paths(&["xtask/src/x.rs", "openspec/specs/build-tooling/spec.md"]);
        let e = review_errors("", &commits, &mixed, None, &none);
        assert!(
            e.len() == 1 && e[0].contains("openspec/specs/build-tooling/spec.md") && e[0].contains("Spec-Guardian: PASS <sha>"),
            "{e:?}"
        );
        assert!(review_errors(&format!("Spec-Guardian: PASS {B}"), &commits, &mixed, None, &none).is_empty());
        // both at once; and other code paths need neither
        let both = paths(&["crates/generators/src/a.rs", "openspec/changes/x/proposal.md"]);
        assert_eq!(review_errors("", &commits, &both, None, &none).len(), 2);
        assert!(review_errors("", &commits, &paths(&["crates/render/src/lib.rs", "xtask/src/x.rs"]), None, &none).is_empty());
        // a docs-only PR needs no path stamp, openspec/ included (the light docs check, owner 2026-10-16)
        assert!(review_errors("", &commits, &paths(&["openspec/specs/x/spec.md", "docs/a.md"]), None, &none).is_empty());
    }

    // spec: BUILD-075, TEST-162
    #[test]
    fn a_record_needs_the_research_review_and_the_owner_look_and_the_sprint_line_is_needed_while_a_sprint_is_active() {
        let commits = paths(&[B, A]);
        let promote = "Question: q\nFinding: f\nDecision: promote: it holds\n";
        // a promotion or park record, in a docs-only PR
        let rec = paths(&["docs/milestones/S1/E1-record.md"]);
        let e = review_errors("", &commits, &rec, None, &files("# E1 record", &[]));
        assert!(e.len() == 2 && e[0].contains("Research-Review: READY <sha>") && e[1].contains("Owner-Look"), "{e:?}");
        let ok = format!("Research-Review: READY {}\nOwner-Look: yes (2026-12-01, sheet s1)", &B[..7]);
        assert!(review_errors(&ok, &commits, &rec, None, &files("# E1 record", &[])).is_empty());
        assert!(review_errors("Research-Review: READY cccccccc\nOwner-Look: no (2026-12-01)", &commits, &rec, None, &files("x", &[]))
            .iter()
            .any(|e| e.contains("names no commit")));
        assert!(
            review_errors(&format!("Research-Review: READY {A}\nOwner-Look: maybe"), &commits, &rec, None, &files("x", &[])).len() == 1
        );
        assert_eq!(owner_look("Owner-Look: yesterday"), None, "the verdict is a whole word");
        assert_eq!(owner_look("Owner-Look: no: R1h faceted (2026-12-01)"), Some("no"));
        assert_eq!(owner_look("Owner-Look: yes"), Some("yes"));
        // a body with Windows line ends reads the same
        let crlf = format!("Sprint: S1\r\nResearch-Review: READY {}\r\nOwner-Look: yes (2026-12-01)\r\n", &B[..7]);
        assert!(review_errors(&crlf, &commits, &rec, Some("S1"), &files("x", &[])).is_empty(), "CRLF body");
        // a research package that promotes is a record; one that rejects, a deleted record and an archived record are not
        let res = paths(&["spikes/s/results/results.md"]);
        assert_eq!(review_errors("", &commits, &res, None, &files(promote, &[])).len(), 2);
        assert!(review_errors("", &commits, &res, None, &files("Decision: reject: no", &[])).is_empty());
        assert!(review_errors("", &commits, &rec, None, &files("x", &["docs/milestones/S1/E1-record.md"])).is_empty(), "deleted at head");
        assert!(review_errors("", &commits, &paths(&["docs/archive/S1/E1-record.md"]), None, &files("x", &[])).is_empty());
        assert!(!is_record("docs/milestones/S1-record.md", Some("x")) && is_record(r"docs\milestones\S1\E2-record.md", Some("x")));
        // the sprint line: required while a sprint is active, and it must name that sprint
        let docs = paths(&["docs/a.md"]);
        assert!(review_errors("", &commits, &docs, None, &files("", &[])).is_empty(), "no active sprint: no line needed");
        let e = review_errors("", &commits, &docs, Some("S-Region-Authoring"), &files("", &[]));
        assert!(e.len() == 1 && e[0].contains("`Sprint: S-Region-Authoring`"), "{e:?}");
        let e = review_errors("Sprint: M-E1", &commits, &docs, Some("S-Region-Authoring"), &files("", &[]));
        assert!(e.len() == 1 && e[0].contains("is not the active sprint"), "{e:?}");
        assert!(review_errors("text\nSprint: S-Region-Authoring (E2)\n", &commits, &docs, Some("S-Region-Authoring"), &files("", &[]))
            .is_empty());
    }

    // spec: BUILD-075, TEST-162
    #[test]
    fn the_active_sprint_is_the_active_file_in_the_sprint_form_only() {
        let d = std::env::temp_dir().join(format!("xtask-prbody-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("docs/milestones")).unwrap();
        let w = |n: &str, t: &str| std::fs::write(d.join("docs/milestones").join(n), t).unwrap();
        w("P3.md", "# P3\n\nStatus: active, closing (lead, 2026-11-27)\n");
        w("S1.md", "# S1\nStatus: draft (lead, 2026-11-27)\nOpened: 2026-11-28   Closes by: 2026-12-04\n");
        assert_eq!(active_sprint(&d), None, "the old milestone form and a draft are no active sprint");
        w("S1.md", "# S1\nStatus: active (owner, 2026-11-28)\nOpened: 2026-11-28   Closes by: 2026-12-04\n");
        assert_eq!(active_sprint(&d).as_deref(), Some("S1"));
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-075, TEST-162
    #[test]
    fn all_problems_are_listed_together_and_the_active_sprints_firewall_runs_on_the_pr() {
        let d = std::env::temp_dir().join(format!("xtask-prbody-fw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("docs/milestones")).unwrap();
        std::fs::write(d.join("docs/milestones/S-Test.md"), include_str!("fixtures/sprint-good.md")).unwrap();
        let commits = paths(&[B, A]);
        let none = |_: &str| None;
        // not past `Closes by` problems only (the fixture's date is fixed; `today` is not)
        let live = |v: Vec<String>| v.into_iter().filter(|e| !e.contains("overdue")).collect::<Vec<_>>();
        // an R exit's PR touching a crate: the firewall refuses it, beside the missing verifier, numerics and sprint lines
        let gen = paths(&["crates/generators/src/a.rs"]);
        let p = live(problems(&d, "", &commits, &gen, "E3: hump survey", Some("spike/hump"), &none).unwrap());
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(
            p[0].contains("Verifier: PASS <sha>") && p[1].contains("Numerics: PASS <sha>") && p[2].contains("`Sprint: S-Test`"),
            "{p:?}"
        );
        assert!(p[3].contains("exit E3 is mode R") && p[3].contains("crates/generators/src/a.rs"), "{p:?}");
        // an X exit's change PR with every line passes
        let body = format!("Sprint: S-Test\nVerifier: PASS {}\n", &B[..8]);
        let p = live(problems(&d, &body, &commits, &paths(&["xtask/src/x.rs"]), "E1: region file", Some("change/e1"), &none).unwrap());
        assert!(p.is_empty(), "{p:?}");
        // a change/* PR that names no X or M exit is refused by the firewall
        let p = live(problems(&d, &body, &commits, &paths(&["xtask/src/x.rs"]), "tidy", Some("change/tidy"), &none).unwrap());
        assert!(p.len() == 1 && p[0].contains("names none"), "{p:?}");
        // no milestone file at all: the firewall's own error is listed
        std::fs::remove_file(d.join("docs/milestones/S-Test.md")).unwrap();
        let p = problems(&d, "", &commits, &paths(&["docs/a.md"]), "docs", None, &none).unwrap();
        assert!(p.len() == 1 && p[0].contains("no milestone file"), "{p:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
