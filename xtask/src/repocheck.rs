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

    // spec: BUILD-006
    #[test]
    fn licence_policy_is_present_and_enforced_in_ci() {
        let deny = std::fs::read_to_string(repo_root().join("deny.toml")).expect("deny.toml");
        assert!(deny.contains("[licenses]") && deny.contains("allow = ["), "deny.toml needs a licence allow-list");
        assert!(workflow().contains("cargo deny check"), "CI must run cargo deny");
    }
}
