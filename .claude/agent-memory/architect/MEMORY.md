# Architect memory

Refreshed 2026-11-22 (G2 closed). Confirm against `main`, `docs/routing.md` and `docs/status.md` before acting. The repo records decisions (ADRs, specs, log, ledger); this file keeps only what it does not.

## Sessions
- **opus-architect**: the lead (ADR 0120): what, how and when; milestone exit list, queue, lane orders, IDs, perf windows, ledger, verdicts, ADRs, the daily digest. Only one lead session at a time: an older one is archived by the owner, or lanes see two leads.
- **sonnet-explorer**, **sonnet-crusader**, **sonnet-spikes**: the lanes; **ag2-antigravity**: companion lane (ADR 0128). Worktrees: `docs/lanes.txt`.
- **sonnet-bulldozer** (2026-11-12) and **opus-planner** (2026-11-09): retired.
- Desktop sessions wake on a message; after a restart a session gets a new `[ref]`: re-check `ListAgents` before sending.

## Where the work lives (outside the repo)
- `C:/DEV/SeriousPlanetData/lanes/`: order files `<lane>.md`, outboxes `<lane>-outbox.md`, `id-reservations.md`.
- `lanes/digest/`: daily `<date>-morning.md`, `lead-handoff-<date>.md` (read it first in a fresh session), and `river-graph/` (contract `g0-contracts.md` §12 readings, reviews, the G2 report, the G3 execution briefs).
- `tmp/g1fNN/`: chain runs and `gate-table-main.*` per record run; flight stores in explorer's worktree `target/stores/`.

## Practice
- A loop tick checks each lane's last session activity (`get_session`), not only its outbox: a lane silent over 60 min has lost its loop (2026-11-22: explorer stopped for 2.5 h unnoticed).
- Lane orders go to the order files; a message only answers a decision request or stops running work. The send budget resets with each owner message.
- Check a lane's headline number against what the gate measures and against the same gate and chain build: the violations bar grew a validator component when the chain started writing `budget-violations.csv`, so g1f14 and g1f15 counts differ in kind.
- Ask a lane for the one check that would expose a bug in its claim (2026-11-22: "excused rises = stopped carries at a down node" found two A3 bugs).
- A rule the owner adopted is applied literally; an edge case it did not foresee (a failing bar worsening by 0.004) goes to the owner, never reinterpreted.
- Delete a finished queue row from `docs/status.md`; do not mark it "Done" (forward-only).
- Reviews by subagents or other models: spot-check their code citations at the right revision before relaying; line numbers drift between main and the spike branch.
- Check the units of every input (2026-11-11: an area passed as a discharge).
- A merge a lane's permission check refused goes to the owner, never to another session.
- Escalate gate failures of architecture gates and constitution matters to the owner; decide reversible design questions and say so.
