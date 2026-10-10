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

    fn workflow() -> String {
        std::fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).expect("CI workflow .github/workflows/ci.yml")
    }

    // spec: BUILD-005
    #[test]
    fn ci_runs_both_operating_systems_and_the_xtask_commands() {
        let w = workflow();
        for needle in [
            "windows-latest",
            "ubuntu-latest",
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
    }

    // spec: BUILD-006
    #[test]
    fn licence_policy_is_present_and_enforced_in_ci() {
        let deny = std::fs::read_to_string(repo_root().join("deny.toml")).expect("deny.toml");
        assert!(deny.contains("[licenses]") && deny.contains("allow = ["), "deny.toml needs a licence allow-list");
        assert!(workflow().contains("cargo deny check"), "CI must run cargo deny");
    }
}
