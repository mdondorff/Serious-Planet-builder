# stage-review-agent (BUILD-048; briefing item 18, PR 3), paper only

Placement decided by the architect, 2026-11-10: `.claude/agents/stage-reviewer.md`, run as a lane step before the push; no CI step, no model key in CI, not a bulldozer step.

## What
- `.claude/agents/stage-reviewer.md`: read-only (Read, Glob, Grep, Bash). Advice only; its report never blocks a merge.
- Trigger: `docs/stages/manifest.txt` differs from main. The PR's author spawns the agent as a subagent and pastes its report, unedited, into the PR body. A manifest change with no report: bulldozer asks for it.
- Input: the manifest diff (per stage and field: hash and key numbers, before and after); the before and after sheets from `cargo xtask stages` (before = main's build of the fixture); the PR's declared intent (PR body and the change's `proposal.md`). The author's assessment of the result is not an input.
- Output, one page: a header stating that "consistent" means the change matches the declared intent, not that the terrain is right; per changed stage a verdict (`consistent`, `undeclared change`, `cannot judge`); the sheets worth the owner's eyes.
- The agent opens every sheet it judges. A sheet missing or unreadable gives `cannot judge`, never `consistent`.
- The before run: no worktree outside `.claude/worktrees/`; temp output only under `target/`.
- Sheets are committed only at a gate, under `docs/stages/sheets/<gate>/`. Check: 1 MB per sheet (CLAUDE.md's limit), 5 MB per gate in total; it counts every sheet added for the gate and fails with file, size and limit. Not goldens (ADR 0017 does not apply).

## Not in this change
- Any CI step or secret. Any gate on the report.
- The production phase plan's emits (explorer's #109).

## Next
- Done in this change: BUILD-048 in the build-tooling spec, the agent file, and the size check in `xtask/src/stages.rs`, run by `cargo xtask spec-lint` (so a docs-only gate PR is checked too).
