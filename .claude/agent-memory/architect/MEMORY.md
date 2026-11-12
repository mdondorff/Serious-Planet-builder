# Architect memory

Refreshed 2026-11-11 (audit batch A). Confirm against `main`, `docs/routing.md` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log, ledger); this file keeps only what it does not.

## Sessions
- **opus-architect**: the lead (ADR 0120): what, how and when; milestone exit list, queue, lane orders, IDs, perf windows, ledger, verdicts, ADRs, the daily digest.
- **sonnet-bulldozer**: retired 2026-11-12 (ADR 0120); its handover is `C:/DEV/planner-inbox/bulldozer-handover.md`.
- **sonnet-explorer**, **sonnet-crusader**: the coding lanes; **sonnet-spikes**: spikes, independent gate measurement, docs-only reviews. Worktrees: `docs/lanes.txt`.
- **opus-planner**: retired 2026-11-09; its worktree stays.
- Desktop sessions wake on a message; after a restart a session gets a new `[ref]`: re-check `ListAgents` before sending.

## Practice
- Lane orders go to the lane's file `C:/DEV/planner-inbox/<lane>.md`; the decisions file is archived after its last entry (ADR 0120).
- The app pauses a session's sends after 10 without an owner message: orders go in lane files; a message only stops or changes running work; never relay through another session.
- Check a lane's headline number against what the gate measures, and the units of every input (2026-11-11: an area passed as a discharge).
- A merge a lane's permission check refused goes to the owner, never to another session.
- Escalate gate failures of architecture gates, the A/B gate and constitution matters to the owner; decide reversible design questions and say so.
