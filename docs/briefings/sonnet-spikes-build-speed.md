# Briefing for sonnet-spikes: SPIKE-15 edit-to-test build time (opus-planner, 2026-10-18)

The owner finds 50-100 s of compile and link after a one-line edit too slow for effective development (`test fast` measured 48-100 s of build before 17-21 s of tests). Measure where the time goes and prototype the fixes. Message `opus-planner` with results and questions.

## Read first
1. `docs/constitution.md`, `CLAUDE.md` (the "Writing" section, ADR 0055).
2. The `spike` skill: the worktree check, one branch, the output rule (`spikes/<name>/results/results.md` with a Visuals section; here the visuals are the `--timings` charts as PNG or HTML screenshots, each under 1 MB).
3. `Cargo.toml` (profiles), `.cargo/config.toml` if present, `.config/nextest.toml`, `xtask/src/testcmd.rs`, the CI workflow.

## Constraints
- Your worktree only; branch `spike/spike-15-build-speed`, never merged; no production code merged.
- Timings are indicative: two coder sessions build at the same time. Record the system CPU load next to every number, repeat each measurement 3 times, report the median. Ask opus-planner for a quiet slot if contention makes the numbers useless.
- `CARGO_BUILD_JOBS=10` for your builds unless a quiet slot is granted.
- No `cargo xtask perf`; stop everything when opus-planner announces a perf window.

## Steps (in order; stop when the target is met)
1. **Measure.** Three typical edits (a comment-free one-line change in a function body of `crates/frame`, `crates/generators`, `crates/app`), each followed by `cargo xtask test fast`. Record per edit: total wall, compile of each crate, link time per binary, number of test binaries linked, `cargo build --timings` charts. Identify the dominant cost (codegen, linking, the dependent chain, contention).
2. **Faster linker:** `rust-lld` (or `lld-link`) for the dev and test profiles. Same three edits.
3. **Fewer test binaries:** prototype one integration-test binary per crate (`tests/main.rs` with modules) for the two crates with the most test binaries. Same edits; check nextest still runs and reports every test by its name.
4. **Test profile tuning:** workspace crates at a lower `opt-level` and reduced debug info, dependencies at `opt-level = 3`; measure the build gain and the test run cost (the numeric tests may slow down). Report both.
5. **Only if steps 2-4 leave linking above about 10 s per edit** (owner, 2026-10-18: keep as the alternative): a development-only dynamic library of the heavy dependencies (wgpu, winit), the Bevy `dynamic_linking` pattern, behind a feature for dev and test builds; release stays static. Check the Windows export-symbol limit and how test binaries find the library.
6. **CI note:** what of 2-4 applies to CI (Windows and Linux runners), and whether a CI cache change would help the median of about 16 min. Do not change CI.

## Answer
A table per step: wall time per edit (median of 3, load), the gain against step 1, the cost (hours to apply properly), the risks. A recommendation: which steps to apply, in which order, and the expected edit-to-test time. Target: a one-line edit to the first test result in 20-30 s on a quiet machine.

## When done
One docs-only PR (entry in `docs/spikes.md`, results with Visuals, learnings, a log fragment in `docs/log.d/`), then a message to opus-planner with the PR number, the table and the recommendation. sonnet-crusader applies the chosen steps afterwards.
