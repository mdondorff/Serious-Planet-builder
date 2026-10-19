## ADDED Requirements

### Requirement: BUILD-016 The pre-push gate is one command
`cargo xtask prepush` SHALL run, in this order and stopping at the first failure, `cargo xtask check` (fmt, clippy, layering), `cargo xtask test fast`, and `cargo xtask test gpu` (tier C on the software adapter) when the branch's changed files (the diff against `origin/main` and the working tree, as `test fast` reads them) include a golden-producing path. A path is golden-producing when it is in `crates/render`, `crates/frame`, `crates/generators`, `crates/core`, `crates/streaming`, `crates/world-def`, `crates/cache`, `crates/app` or `crates/testkit`, ends in `.wgsl`, is under `tests/goldens/`, or is `Cargo.toml`, `Cargo.lock` or under `.cargo/`. `crates/editor`, `crates/perf`, `xtask/`, `docs/`, `openspec/`, `.github/` and root files are not. The command prints which of the two cases applies before it starts, the name of each step as it begins, and at the end one line with the time of every step and the total (`prepush OK in <s> s: check ok <s> s, test fast ok <s> s[, test gpu ok <s> s]`, or `prepush FAILED at <step> after <s> s: ...`); a failure ends the command with a non-zero status. Golden differences that appear on the lavapipe adapter only stay CI's job (ADR 0061).

Verify: A
Status: active
Source: owner and opus-planner (2026-10-19)

#### Scenario: Which paths need the GPU tier
- **WHEN** the changed paths are `crates/render/src/terrain.rs`, `crates/frame/src/look/water.rs`, a `.wgsl` file, `tests/goldens/software/x.png` or `Cargo.lock`, and when they are only `docs/status.md`, `crates/editor/src/lib.rs`, `crates/perf/src/calibration.rs`, `xtask/src/main.rs`, `CLAUDE.md` or `.github/workflows/ci.yml`
- **THEN** the first set includes `test gpu` and the second does not, backslash separators included

#### Scenario: One line verdict
- **WHEN** `check` took 20 s and `test fast` 31 s without a golden path, or `test fast` failed after 9 s
- **THEN** the lines are `prepush OK in 51 s: check ok 20 s, test fast ok 31 s; test gpu skipped (no golden-producing path changed)` and `prepush FAILED at test fast after 29 s: check ok 20 s, test fast FAILED 9 s`

### Requirement: BUILD-017 A runner fault in the tool install gets one second attempt
Each `taiki-e/install-action` step of `.github/workflows/ci.yml` SHALL be followed by a second identical step (same `tool:` list) that runs only when the first one failed (`if: steps.install.outcome == 'failure'`), the first having `id: install` and `continue-on-error: true`. The failure it covers is the runner fault "bash startup failure" (actions/partner-runner-images#169), not a code failure; any other install failure fails the second attempt too and stays visible.

Verify: A
Status: active
Source: opus-planner (2026-10-19), actions/partner-runner-images#169

#### Scenario: The install is paired
- **WHEN** `ci.yml` is read
- **THEN** every install step has its retry with the same tools right after it, the first with `continue-on-error`, the retry with the failure condition
