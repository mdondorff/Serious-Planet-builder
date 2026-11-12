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
            // Tiers A+B (in parts, BUILD-036).
            "cargo xtask test -- --partition hash:",
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
    fn tier_c_runs_on_windows_in_four_parts_from_one_archive_with_their_own_artifact_names() {
        let w = workflow();
        let gpu = w.split("\n  gpu:\n").nth(1).expect("ci.yml has a `gpu` job").split("\n  determinism:").next().unwrap();
        // One build of the tier-C binaries into an archive; the Windows parts run it and build nothing.
        let build = w.split("\n  gpu-build:\n").nth(1).expect("ci.yml has a `gpu-build` job").split("\n  gpu:\n").next().unwrap();
        assert!(
            build.contains("cargo xtask test gpu --archive-out target/gpu-archive.tar.zst") && build.contains("name: gpu-archive"),
            "{build}"
        );
        for needle in [
            "needs: gpu-build",
            "cargo xtask test gpu --archive-file target/gpu-archive.tar.zst -- --partition count:${{ matrix.part }}/4",
            "part: 1",
            "part: 2",
            "part: 3",
            "part: 4",
            "test-logs-${{ runner.os }}-gpu${{ matrix.part",
            "review-${{ runner.os }}${{ matrix.part",
        ] {
            assert!(gpu.contains(needle), "the gpu job is missing `{needle}`");
        }
        assert_eq!(gpu.matches("os: windows-latest").count(), 4, "four Windows entries, one per part");
        assert_eq!(gpu.matches("os: ubuntu-24.04").count(), 1, "the Linux leg runs whole");
    }

    // spec: BUILD-036
    #[test]
    fn tiers_a_and_b_run_in_three_hash_parts_per_os_and_part_two_alone_runs_the_checks_and_the_manifest() {
        let w = workflow();
        let job = w
            .split(
                "
  test:
",
            )
            .nth(1)
            .expect("ci.yml has a `test` job")
            .split(
                "
  gpu",
            )
            .next()
            .unwrap();
        assert!(job.contains("name: ${{ matrix.name }} A+B ${{ matrix.part }}/3"), "{job}");
        // A test's part depends on its name alone (hash), so a new test does not move the others between parts.
        assert!(job.contains("cargo xtask test -- --partition hash:${{ matrix.part }}/3"), "{job}");
        assert_eq!(job.matches("os: windows-latest").count(), 3, "three Windows parts");
        assert_eq!(job.matches("os: ubuntu-24.04").count(), 3, "three Linux parts");
        assert_eq!((job.matches("part: 1").count(), job.matches("part: 2").count(), job.matches("part: 3").count()), (2, 2, 2));
        // The determinism manifest (its upload too), check, spec-lint, licences and the run-mode starts run once per OS, in part 2.
        assert_eq!(job.matches("if: matrix.part == 2").count(), 6, "{job}");
        for step in [
            "determinism manifest (GEN-003)",
            "check (fmt, clippy, layering)",
            "spec-lint",
            "licences (CON-27)",
            "run modes start (CON-01)",
        ] {
            let after = job.split(&format!("name: {step}")).nth(1).unwrap_or_else(|| panic!("no step `{step}`"));
            assert!(after.trim_start().starts_with("if: matrix.part == 2"), "`{step}` runs in part 2 only");
        }
        assert!(
            job.contains("name: test-logs-${{ runner.os }}-ab-${{ matrix.part }}")
                && job.contains("name: review-${{ runner.os }}-ab-${{ matrix.part }}")
        );
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
        let mut expected: Vec<String> = ["docs/**", "openspec/**", ".claude/**", "*.md", "spikes/**/results/**", "spikes/README.md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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

    // spec: BUILD-045
    #[test]
    fn the_class_source_store_tests_take_four_slots_and_keep_the_default_limit() {
        let toml = std::fs::read_to_string(repo_root().join(".config/nextest.toml")).expect(".config/nextest.toml");
        let filter = "package(=planet-cache) and binary(it) and test(/^class_source_store::/)";
        let at = toml.find(filter).expect("an override for the class_source_store tests");
        let block = &toml[at..];
        let block = &block[..block
            .find(
                "

",
            )
            .unwrap_or(block.len())];
        assert!(block.contains("threads-required = 4"), "the override takes 4 slots: {block}");
        assert!(!block.contains("slow-timeout"), "the limit stays the default's: {block}");
    }

    // spec: BUILD-049
    #[test]
    fn the_tests_over_sixty_seconds_on_the_windows_runner_share_the_one_at_a_time_group_and_keep_their_limits() {
        let toml = std::fs::read_to_string(repo_root().join(".config/nextest.toml")).expect(".config/nextest.toml");
        let at = toml.find("platform = 'cfg(windows)'").expect("an override for the Windows runner");
        let block = &toml[at..];
        let block = &block[..block.find("\n\n").unwrap_or(block.len())];
        for name in [
            "an_in_memory_run_is_labelled_not_the_production_source_and_real_runs_refuse_without_a_store_and_on_software",
            "the_lit_scene_runs_every_view_and_both_flights_with_a_pass_row_each_and_logs_the_metrics_in_smoke_mode",
            "the_stills_scene_draws_one_pose_by_name_and_refuses_a_bad_call",
            "a_still_camera_after_streaming_settles_draws_the_same_pixels_frame_after_frame",
            "the_hills_cast_the_march_shadow_onto_the_ground_at_eye_level_and_from_above_at_ten_and_two_degrees_of_sun_and_match_the_adapters_goldens",
            "the_indirect_path_draws_the_image_of_the_per_draw_loop",
            "the_overlay_inspection_images_are_written_for_a_look",
            "ten_scripted_views_on_steep_terrain_have_no_holes",
        ] {
            assert!(block.contains(&format!("test({name})")), "the override names {name}: {block}");
        }
        assert!(block.contains("test-group = \"warp-heavy\""), "one at a time: {block}");
        assert!(!block.contains("slow-timeout") && !block.contains("retries"), "no limit and no retry changes: {block}");
        assert!(toml.contains("warp-heavy = { max-threads = 1 }"), "the group runs one test at a time");
    }

    fn pinned_channel() -> String {
        let toml = std::fs::read_to_string(repo_root().join("rust-toolchain.toml")).expect("rust-toolchain.toml");
        let line = toml.lines().find(|l| l.trim_start().starts_with("channel")).expect("a `channel` line in rust-toolchain.toml");
        line.split('"').nth(1).expect("a quoted channel").to_string()
    }

    // spec: BUILD-050
    #[test]
    fn the_rust_toolchain_is_one_exact_release_and_every_ci_job_installs_that_release() {
        let channel = pinned_channel();
        let parts: Vec<&str> = channel.split('.').collect();
        assert!(
            parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
            "rust-toolchain.toml pins an exact release (X.Y.Z), not `{channel}` (ADR 0114 point 6)"
        );
        let w = workflow();
        assert!(!w.contains("rust-toolchain@stable"), "ci.yml installs a moving `stable`: pin it to {channel}");
        let jobs = w.matches("dtolnay/rust-toolchain@").count();
        let pinned = w.matches(&format!("toolchain: {channel}")).count();
        assert!(jobs > 0 && jobs == pinned, "every toolchain step of ci.yml names `toolchain: {channel}` ({pinned} of {jobs} do)");
        assert!(!w.contains("toolchain: stable") && !w.contains("toolchain: nightly"), "ci.yml names a moving toolchain");
    }

    // spec: BUILD-050
    #[test]
    fn cargo_lock_is_committed_and_no_compiler_flag_or_cpu_feature_changes_the_float_results() {
        let root = repo_root();
        assert!(root.join("Cargo.lock").is_file(), "Cargo.lock is committed (ADR 0114 point 6)");
        let ignore = std::fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
        assert!(!ignore.lines().any(|l| l.trim() == "Cargo.lock"), "Cargo.lock must not be ignored");
        // No target CPU, no fast-math style flag, no RUSTFLAGS in any file that sets the build.
        let mut files = vec![root.join(".cargo/config.toml"), root.join("Cargo.toml"), root.join(".github/workflows/ci.yml")];
        if let Ok(rd) = std::fs::read_dir(root.join("crates")) {
            for e in rd.flatten() {
                files.push(e.path().join("Cargo.toml"));
            }
        }
        files.push(root.join("xtask/Cargo.toml"));
        for f in files {
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            for needle in ["target-cpu", "target-feature", "fast-math", "ffast-math", "RUSTFLAGS", "opt-level = \"z\""] {
                let hits: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with('#') && l.contains(needle)).collect();
                assert!(hits.is_empty(), "{} sets `{needle}`: the build settings stay the profile's defaults (ADR 0114 point 6): {hits:?}", f.display());
            }
        }
    }
}
