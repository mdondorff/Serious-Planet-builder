# Architect memory

Refreshed 2026-11-11 (audit batch A). Confirm against `main`, `docs/routing.md` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log, ledger); this file keeps only what it does not.

## Sessions
- **opus-architect**: the owner's main contact; contract, ADRs, gate verdicts, briefing content and order, reviews, decision briefs.
- **sonnet-bulldozer**: execution: lane orders and sequencing, merges, CI, `docs/status.md`, ID reservations, ledger upkeep, perf windows, summaries.
- **sonnet-explorer**, **sonnet-crusader**: the coding lanes; **sonnet-spikes**: spikes, independent gate measurement, docs-only reviews. Worktrees: `docs/lanes.txt`.
- **opus-planner**: retired 2026-11-09; its worktree stays.
- Desktop sessions wake on a message; after a restart a session gets a new `[ref]`: re-check `ListAgents` before sending.

## Practice
- Decisions on defined work go to `C:/DEV/planner-inbox/architect-decisions.md` (one entry each, never edited after writing); new work and briefing order go to bulldozer by message.
- The app pauses a session's sends after 10 without an owner message: batch decisions into one message to bulldozer; never relay through another session.
- Check a lane's headline number against what the gate measures, and the units of every input (2026-11-11: an area passed as a discharge).
- A merge a lane's permission check refused goes to the owner, never to another session.
- Escalate gate failures of architecture gates, the A/B gate and constitution matters to the owner; decide reversible design questions and say so.
