//! Tests over repository files themselves (no code under test beyond the files).

#[cfg(test)]
mod tests {
    use crate::util::{files_with_ext, repo_root};

    // spec: BUILD-002
    #[test]
    fn repository_has_no_shell_or_powershell_scripts() {
        let root = repo_root();
        let mut found = Vec::new();
        for ext in ["sh", "ps1", "bat", "cmd"] {
            files_with_ext(&root, ext, &mut found);
        }
        let found: Vec<_> = found.iter().map(|p| p.strip_prefix(&root).unwrap().display().to_string()).collect();
        assert!(found.is_empty(), "CON-25: no scripts in the repo, add an xtask command instead: {found:?}");
    }

    // spec: TEST-002
    #[test]
    fn every_golden_directory_records_its_driver() {
        let dir = repo_root().join("tests/goldens");
        for e in std::fs::read_dir(&dir).expect("tests/goldens").flatten() {
            if e.path().is_dir() {
                let sidecar = e.path().join("driver.txt");
                let text = std::fs::read_to_string(&sidecar).unwrap_or_default();
                assert!(
                    !text.trim().is_empty(),
                    "{} must record the driver string the goldens were blessed on (ADR 0009)",
                    sidecar.display()
                );
            }
        }
    }

    fn workflow() -> String {
        std::fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("CI workflow .github/workflows/ci.yml")
    }

    // spec: BUILD-005
    #[test]
    fn ci_runs_both_operating_systems_and_the_xtask_commands() {
        let w = workflow();
        for needle in [
            "windows-latest",
            "ubuntu-24.04",
            "PLANET_REQUIRE_ADAPTER_GOLDENS",
            "mesa-vulkan-drivers",
            "cargo xtask check",
            "cargo xtask spec-lint",
            "cargo xtask test\n",
            "cargo xtask test gpu",
            "--smoke",
            "pull_request",
        ] {
            assert!(w.contains(needle), "ci.yml is missing `{needle}`");
        }
        // The runner image is pinned: a moving `-latest` Linux image changes Mesa and LLVM under the goldens.
        assert!(!w.contains("ubuntu-latest"), "ci.yml must pin the Linux runner image (ubuntu-24.04), not ubuntu-latest");
    }

    // spec: BUILD-018
    #[test]
    fn tier_c_runs_on_windows_in_two_halves_with_their_own_artifact_names() {
        let w = workflow();
        let gpu = w.split("\n  gpu:\n").nth(1).expect("ci.yml has a `gpu` job").split("\n  determinism:").next().unwrap();
        for needle in [
            "part: 1",
            "part: 2",
            "--partition count:{0}/2",
            "test-logs-${{ runner.os }}-gpu${{ matrix.part",
            "review-${{ runner.os }}${{ matrix.part",
        ] {
            assert!(gpu.contains(needle), "the gpu job is missing `{needle}`");
        }
        assert_eq!(gpu.matches("os: windows-latest").count(), 2, "two Windows entries, one per half");
        assert_eq!(gpu.matches("os: ubuntu-24.04").count(), 1, "the Linux leg runs whole");
    }

    // spec: BUILD-017
    #[test]
    fn every_tool_install_in_ci_gets_one_automatic_second_attempt() {
        let w = workflow();
        let lines: Vec<&str> = w.lines().collect();
        let installs: Vec<usize> =
            lines.iter().enumerate().filter(|(_, l)| l.trim() == "- uses: taiki-e/install-action@v2").map(|(i, _)| i).collect();
        assert!(!installs.is_empty(), "ci.yml installs its tools with taiki-e/install-action");
        // Each job's installs come in pairs: the first one (id `install`, allowed to fail) and the retry (only after that failure),
        // with the same `tool:` list.
        assert_eq!(installs.len() % 2, 0, "install steps come in pairs (first attempt, retry)");
        let block = |start: usize| -> String { lines[start..(start + 6).min(lines.len())].join("\n") };
        for pair in installs.chunks(2) {
            let (first, retry) = (block(pair[0]), block(pair[1]));
            assert!(first.contains("id: install") && first.contains("continue-on-error: true"), "first attempt may fail:\n{first}");
            assert!(retry.contains("if: steps.install.outcome == 'failure'"), "the retry runs only after a failure:\n{retry}");
            let tool = |b: &str| b.lines().find(|l| l.trim_start().starts_with("tool:")).map(str::trim).map(String::from);
            assert_eq!(tool(&first), tool(&retry), "the retry installs the same tools");
            assert!(tool(&retry).is_some());
        }
    }

    // spec: BUILD-006
    #[test]
    fn licence_policy_is_present_and_enforced_in_ci() {
        let deny = std::fs::read_to_string(repo_root().join("deny.toml")).expect("deny.toml");
        assert!(deny.contains("[licenses]") && deny.contains("allow = ["), "deny.toml needs a licence allow-list");
        assert!(workflow().contains("cargo deny check"), "CI must run cargo deny");
    }

    /// The quoted entries of the `key: [..]` line that follows the given trigger (`push:` or `pull_request:`) in a workflow.
    fn trigger_paths(workflow: &str, trigger: &str, key: &str) -> Vec<String> {
        let mut lines = workflow.lines().skip_while(|l| l.trim() != trigger);
        let line = lines.find(|l| l.trim_start().starts_with(key)).unwrap_or_else(|| panic!("no `{key}` after `{trigger}`"));
        let list = line.split_once('[').and_then(|(_, r)| r.split_once(']')).map(|(l, _)| l).unwrap_or("");
        list.split(',').map(|p| p.trim().trim_matches('"').to_string()).filter(|p| !p.is_empty()).collect()
    }

    // spec: BUILD-005
    #[test]
    fn both_workflows_share_the_docs_only_paths() {
        let ci = workflow();
        let docs =
            std::fs::read_to_string(repo_root().join(".github/workflows/docs.yml")).expect("docs workflow .github/workflows/docs.yml");
        let mut expected: Vec<String> =
            ["docs/**", "openspec/**", ".claude/**", "*.md", "spikes/**/results/**"].iter().map(|s| s.to_string()).collect();
        expected.sort();
        for (name, text, key) in [("ci.yml", &ci, "paths-ignore"), ("docs.yml", &docs, "paths")] {
            for trigger in ["push:", "pull_request:"] {
                let mut got = trigger_paths(text, trigger, key);
                got.sort();
                assert_eq!(
                    got, expected,
                    "{name} `{trigger}` {key}: the docs-only paths must match across both workflows (owner, 2026-10-16)"
                );
            }
        }
    }

    // spec: BUILD-011
    #[test]
    fn dev_builds_link_with_rust_lld_and_carry_line_tables_only() {
        let root = repo_root();
        let cargo = std::fs::read_to_string(root.join(".cargo/config.toml")).expect(".cargo/config.toml");
        assert!(
            cargo.contains("[target.x86_64-pc-windows-msvc]") && cargo.contains("linker = \"rust-lld\""),
            ".cargo/config.toml must link x86_64-pc-windows-msvc with rust-lld (BUILD-011)"
        );
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
        // The lines of one table: from its header to the next header.
        let table = |header: &str| -> Vec<String> {
            let lines = manifest.lines().skip_while(|l| l.trim() != header).skip(1);
            lines.take_while(|l| !l.trim_start().starts_with('[')).map(|l| l.trim().to_string()).collect()
        };
        assert!(
            table("[profile.dev]").iter().any(|l| l.starts_with("debug = \"line-tables-only\"")),
            "[profile.dev] needs debug = \"line-tables-only\""
        );
        assert!(
            table("[profile.dev.package.\"*\"]").iter().any(|l| l.starts_with("debug = false")),
            "dependencies need debug = false in the dev profile"
        );
    }

    /// Names of the changes in `changes` (name and `tasks.md` text, `None` when there is none) whose tasks are all
    /// done: at least one `- [x]` task and no `- [ ]` task.
    fn finished_changes(changes: &[(String, Option<String>)]) -> Vec<String> {
        changes
            .iter()
            .filter(|(_, tasks)| {
                tasks.as_deref().is_some_and(|t| {
                    let done = t.lines().filter(|l| l.trim_start().starts_with("- [x]")).count();
                    let open = t.lines().filter(|l| l.trim_start().starts_with("- [ ]")).count();
                    done > 0 && open == 0
                })
            })
            .map(|(n, _)| n.clone())
            .collect()
    }

    // spec: BUILD-008
    #[test]
    fn finished_changes_are_recognised() {
        let c = |n: &str, t: Option<&str>| (n.to_string(), t.map(str::to_string));
        let changes = [
            c(
                "done",
                Some(
                    "## 1.
- [x] 1.1 a
- [x] 1.2 b
",
                ),
            ),
            c(
                "open",
                Some(
                    "- [x] 1.1 a
- [ ] 1.2 b
",
                ),
            ),
            c(
                "fresh",
                Some(
                    "- [ ] 1.1 a
",
                ),
            ),
            c("no-tasks", None),
            c(
                "empty",
                Some(
                    "no checklist here
",
                ),
            ),
        ];
        assert_eq!(finished_changes(&changes), ["done"]);
    }

    // spec: BUILD-008
    #[test]
    fn no_finished_change_stays_unarchived() {
        let dir = repo_root().join("openspec/changes");
        let mut changes = Vec::new();
        for e in std::fs::read_dir(&dir).expect("openspec/changes").flatten() {
            if e.path().is_dir() && e.file_name() != "archive" {
                let tasks = std::fs::read_to_string(e.path().join("tasks.md")).ok();
                changes.push((e.file_name().to_string_lossy().into_owned(), tasks));
            }
        }
        let finished = finished_changes(&changes);
        assert!(finished.is_empty(), "finished but not archived (run the archive step, then add the log line): {finished:?}");
    }
}
