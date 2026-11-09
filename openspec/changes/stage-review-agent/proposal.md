# stage-review-agent (BUILD-048; briefing item 18, PR 3), paper only

## What
- `.claude/agents/stage-reviewer.md` (placement is the architect's call; question open): read-only agent, tools Read, Glob, Grep, Bash. Advice only: it never edits, never blocks, its report is not a gate.
- Trigger: only when `docs/stages/manifest.txt` differs from main (`git diff origin/main -- docs/stages/manifest.txt` is non-empty).
- Input: the manifest diff (per stage and field: hash and key numbers, before and after); the sheets of the before run (main's build of the fixture) and the after run, both from `cargo xtask stages` into `target/review/stages/<before|after>/`; the PR's declared intent (the PR body and the change's `proposal.md`).
- Output (one page): per changed stage, whether the numbers move as the declared intent says; stages that changed without being declared; sheets worth the owner's eyes; the verdict words "consistent", "undeclared change" or "cannot judge". A stage change with no declared intent is reported, not failed.
- Sheets are committed only at gates (`docs/stages/sheets/<gate>/`), under a size cap (`SHEETS_MAX_BYTES`, in the check); `cargo xtask spec-lint` (or a `stages` check) fails over the cap. Not goldens (ADR 0017 does not apply).

## Open
- Where it runs: agent file used by a lane step (run by the PR's author before the push, report pasted in the PR body) vs a CI step (needs a model key in CI: a new secret, an outward-facing integration) vs bulldozer's pre-merge step.
- The size cap value.
