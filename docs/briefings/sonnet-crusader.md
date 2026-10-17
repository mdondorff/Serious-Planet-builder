# Briefing for sonnet-crusader: second coder session (opus-planner, 2026-10-17)

You are a second coder beside sonnet-explorer, which owns the M5 queue (water fixes, perf window 7). sonnet-spikes runs M6 spikes. opus-planner is the architect: message it (`opus-planner`) with results, questions and stop reports.

## Read first
1. `docs/constitution.md` and `CLAUDE.md`, especially "How to work", "Writing" (ADR 0055), "Git" and "When to stop and report".
2. `openspec/specs/build-tooling/spec.md` (BUILD-001 to BUILD-009).
3. `docs/status.md`: Tracked, and the M5 queue (so you know what not to touch).

## Rules for this session
- Work only in your worktree, `.claude/worktrees/sonnet-crusader`. One branch per change (`change/<id>`), cut from `origin/main`.
- Don't touch what sonnet-explorer is changing: the water pass, lit planning (`crates/app/src/litperf.rs`), `litscenes`, the perf harness, `golden-review`. If your change needs one of these, ask opus-planner first.
- Cap builds at half the cores: `CARGO_BUILD_JOBS=10`.
- No `cargo xtask perf`. If opus-planner announces a perf window, stop all builds, tests and runs until it says resume.
- You need no GPU for this task. If a later task does, ask sonnet-explorer whether the GPU is free first.
- Merge yourself (squash) when CI is green on Windows and Linux and the verifier passed. Before merging main into your branch, search for conflict markers (spec-lint also checks, BUILD-009).

## Task 1: log fragments (time box half a day)
**Problem:** every PR prepends a line to `docs/log.md`, so three parallel writers conflict on it daily (the `housekeeping` skill names this as a root cause; PR #222 merged unresolved markers this way).

**Build:**
1. **Fragments:** a new log line is a file `docs/log.d/<YYYY-MM-DD>-<id>.md` holding one line in today's format (`- YYYY-MM-DD **title** ...`). `<id>` is the change id or a short slug; two writers never write the same file.
2. **`cargo xtask log [--since YYYY-MM-DD] [--limit N]`:** prints the fragments and the entries of `docs/log.md`, merged newest first. Order: date descending, then by file name descending, with fragments before `log.md` lines of the same date. The output is deterministic.
3. **spec-lint:** fails on a malformed fragment: not exactly one entry, an entry date that differs from the file-name date, or a bad name. It names the file and the reason.
4. **Existing `docs/log.md`:** unchanged history. Its header says new entries go to `docs/log.d/` and points to `cargo xtask log`. The `housekeeping` skill folds fragments into `docs/archive/<M>/log.md` at milestone ends; update its "Root causes" line, since this is the fix it names.
5. **Docs in the same PR:** CLAUDE.md "How to work" step 5 and the `adr`, `spike` and `learning` skills say "add a fragment in `docs/log.d/`" instead of "add one line at the top of `docs/log.md`".

**Spec:** a new requirement BUILD-010 in `openspec/specs/build-tooling/spec.md` with scenarios (fragment accepted; a malformed fragment rejected with its name; the merged output ordered and deterministic; two fragments of one day in a stable order). Tier-A tests on fixtures, citing BUILD-010.

**Done when:**
- `cargo xtask test fast`, `cargo xtask spec-lint` and `cargo xtask test` pass;
- the verifier has passed;
- CI is green;
- the PR is merged;
- this change's own log entry is a fragment.

Then message opus-planner: the PR number, what changed for other sessions (one line they must follow), and the time it took. opus-planner tells the other sessions.
