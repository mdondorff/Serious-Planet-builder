//! `cargo xtask pr-body-check` (BUILD-044): the `pr-gate` check that a PR with code in it carries `Verifier: PASS <sha>` in its body, the sha a
//! commit of the PR. It reads `PR_BODY`, `BASE_SHA` and `HEAD_SHA` from the environment, so the body never reaches a shell command line.
//! A docs-only PR (the paths the `ci` workflow ignores) is exempt.

use crate::util::{capture, Res};
use std::path::Path;

/// The paths `ci.yml` ignores (`paths-ignore`); a PR whose every changed path matches one is docs-only. A test keeps this list equal to the workflow's.
#[cfg(test)]
pub const DOCS_PATTERNS: [&str; 6] = ["docs/**", "openspec/**", ".claude/**", "*.md", "spikes/**/results/**", "spikes/README.md"];

/// True when `path` matches one of [`DOCS_PATTERNS`].
pub fn is_docs_path(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.starts_with("docs/")
        || p.starts_with("openspec/")
        || p.starts_with(".claude/")
        || (!p.contains('/') && p.ends_with(".md"))
        || (p.starts_with("spikes/") && p.contains("/results/"))
        || p == "spikes/README.md"
}

/// The sha of the first `Verifier: PASS <sha>` line of a PR body (at least 7 hex digits), lower case.
pub fn verifier_sha(body: &str) -> Option<String> {
    body.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("Verifier: PASS ")?;
        let sha: String = rest.trim().chars().take_while(char::is_ascii_hexdigit).collect();
        (7..=64).contains(&sha.len()).then(|| sha.to_lowercase())
    })
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
    std::env::var(name).map_err(|_| format!("{name} is not set (pr-body-check reads PR_BODY, BASE_SHA and HEAD_SHA)"))
}

pub fn run(root: &Path) -> Res {
    let body = std::env::var("PR_BODY").unwrap_or_default();
    let (base, head) = (env("BASE_SHA")?, env("HEAD_SHA")?);
    let commits: Vec<String> = capture(root, "git", &["rev-list", &format!("{base}..{head}")])?.lines().map(str::to_string).collect();
    let diff = capture(root, "git", &["diff", "--name-only", "--no-renames", &format!("{base}...{head}")])?;
    let changed: Vec<String> = diff.lines().map(str::to_string).collect();
    if changed.is_empty() {
        println!("pr-body-check: no changed path, nothing to check");
        return Ok(());
    }
    let note = check(&body, &commits, &changed)?;
    println!("pr-body-check: {note}");
    Ok(())
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
        for p in
            ["docs/status.md", "openspec/specs/x/spec.md", ".claude/agents/a.md", "CLAUDE.md", "spikes/s/results/r.txt", "spikes/README.md"]
        {
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
}
