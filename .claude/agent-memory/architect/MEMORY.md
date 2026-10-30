# Architect memory

Refreshed 2026-10-30 at M8's housekeeping (session `opus-planner`, worktree `.claude/worktrees/opus-planner`). Confirm against `main` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log); this file keeps only what it does not.

## Sessions
- **sonnet-explorer** (worktree `start-879e4b`): main lane; B4 owner checks, then B2/B3.
- **sonnet-crusader** (worktree `sonnet-crusader-c1a4f2`): support lane; B6 chunk-stage removal (ADR 0086), then B5.
- **sonnet-spikes** (worktree `sonnet-spikes-36fc88`): spikes and docs-only passes; keeps uncommitted prototypes (SPIKE-56 until ADR 0085's build).
- Desktop sessions wake on a message; after a clear or restart a lane gets a new `[ref]`: re-check `ListAgents`.

## Practice
- Owner decisions are committed in the planner session; coders merge them.
- Merge slots: one PR merges at a time while the up-to-date ruleset is on (2026-10-30); the planner hands the slot on.
- New requirement IDs, ADR and SPIKE numbers are reserved through the planner.
- A merge a lane's permission check refused goes to the owner, never to another session.
- Real-GPU window checks need an unlocked desktop; NOT RUN when locked (TEST-073).
- Check a coder's headline number against what the criterion measures.
- Repro hunts are time-boxed to about 1 h.
- Escalate exit criteria, constitution and open trade-offs (push notification); decide reversible design questions and say so.
