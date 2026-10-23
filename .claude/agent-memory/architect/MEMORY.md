# Architect memory

Refreshed 2026-10-21 at the M6 close (session `opus-planner`, worktree `.claude/worktrees/opus-planner`). Confirm against `main` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log); this file keeps only what it does not.

## Sessions
- **sonnet-explorer** (worktree `sonnet-m4`): main lane; runs perf windows on the planner's go (native UI; the planner's VS Code session measures about 1.4x slower).
- **sonnet-crusader:** support lane, parallel side work; moves no goldens without golden-review.
- **sonnet-spikes:** spikes and diagnosis only; idle unless a spike blocks.
- Cleared sessions wake on an incoming message; no loop line or paste needed. Check `ListAgents` for exact names.

## State (2026-10-21)
- M6 closed by the owner (GO, `docs/milestones/M6-report.md`); M7 "ground detail and world authoring 2" queued in `docs/status.md`, plan `docs/archive/M7/m7-planning.md`.
- Weekly quota is tight (owner reports the share; reset on Sunday): one coder lane, gate-critical work only, until the reset.
- Hot files are capped by ADR 0075; housekeeping at every milestone close keeps them there.

## Practice
- Owner decisions are committed in the planner session; coders merge them.
- Mass deletions and archive steps run from the planner's session after the owner's go (classifier refusals in coder sessions, M6).
- Repro hunts are time-boxed to about 1 h, then park on a crash report or the owner's F9.
- Notify the owner (push) only to unblock: decisions, /bless, clearing a session.
- M7 closed 2026-10-23 (owner GO). M8 "Natural landscape": the Outerra image is the minimum bar; stage model in ADR 0077 (proposed; blocks of a fixed grid, a coupled process is one field), decided from SPIKE-33 and SPIKE-34.
- Coder sessions run the loop line that names only docs/status.md; the queue header links the current week's brief.
