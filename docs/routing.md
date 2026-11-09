# Sessions and routing

Owner, 2026-11-09. Read at session start.

| Session | Owns |
|---|---|
| architect (Opus; the owner's main contact) | contract, ADRs, gate verdicts, briefing content and order, external reviews, decision briefs |
| sonnet-bulldozer | lane orders, merging lane PRs, CI, `docs/status.md`, ID reservations, perf windows, summaries |
| explorer, crusader | implementation |
| spikes | one spike at a time; clears itself after its exit list |

| A lane's message | Goes to |
|---|---|
| PR ready, CI, merge slot, ID request, idle, perf window, next step the briefing names | bulldozer |
| Failed gate, departure from an ADR or the contract, a measurement contradicting a contract number, "what next" the briefing doesn't answer | the architect directly, with a one-line note to bulldozer |

- A message that fits both rows, or a doubt, goes to the architect.
- Only bulldozer writes lane orders; lane orders are in `C:/DEV/planner-inbox/<lane>.md`. An architect's answer that changes the order of work reaches a lane through bulldozer.
- Bulldozer merges lane PRs under the rules below; the architect first for a PR that departs from an accepted ADR, changes a contract number or claims to close a gate.
