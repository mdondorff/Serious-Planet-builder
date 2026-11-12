# Sessions and routing

Owner, 2026-11-09. Read at session start.

## Roles and session names
"Architect" and "bulldozer" are always nouns naming a role, never verbs. A lane is a session that implements work; there are three, listed below. To message a role, send to its session name in this table; when a session is recreated under a new name, this table changes.

| Role | Session name | Model | Owns |
|---|---|---|---|
| architect (the owner's main contact) | `opus-architect` | Opus | contract, ADRs, gate verdicts, briefing content and order, external reviews, decision briefs for the owner |
| bulldozer | `sonnet-bulldozer` | Sonnet | lane orders, merging lane PRs, CI, `docs/status.md`, ID reservations, perf windows, summaries |
| lane | `sonnet-explorer`, `sonnet-crusader` | Sonnet | implementation |
| lane | `sonnet-spikes` | Sonnet unless the brief says otherwise | one spike at a time; clears itself after the spike's exit list holds |

## Where a lane sends its message
| The message is about | Send it to |
|---|---|
| PR ready, CI state, merge slot, ID request, being idle, a perf window, or the next step when the current briefing in `docs/briefings/` lists a next item | `sonnet-bulldozer` |
| A failed gate, a departure from an ADR or the contract, a measurement that contradicts a contract number, or the next step when it does not, or when the listed item is blocked | `opus-architect`, and a one-line note to `sonnet-bulldozer` |

A message that fits both rows goes by the second row. In doubt, use the second row.

## Rules
1. Only the bulldozer writes lane orders, in `C:/DEV/planner-inbox/<lane>.md`, where `<lane>` is `explorer`, `crusader` or `spikes`.
2. When the architect's answer to a lane changes the order of work, the change reaches the lane through the bulldozer.
3. The bulldozer merges lane PRs when the conditions in the "Git" section of `CLAUDE.md` hold. The architect and the bulldozer merge their own PRs under the same conditions. A lane does not merge.
4. Before merging, the bulldozer sends these PRs to the architect for review: a PR that departs from an accepted ADR, changes a contract number, or claims to close a gate.
5. Decisions for the owner come from the architect. Execution blockers for the owner come from the bulldozer.
6. The architect's decisions are in `C:/DEV/planner-inbox/architect-decisions.md` (ADR 0118). The bulldozer and every lane read it at the start of each turn and record the entries they act on, by number, in their state or inbox file. A message from the architect means an entry stops or changes running work.
7. A new generator stage, or a rule that changes results, lands switched off. A separate PR switches it on after its gate measure passes on the production path (ADR 0118).
8. A lane may restate a test's guard, the assertion that the test exercises its case, when a fix removes what it counted, if every property assertion is unchanged; the PR body says so. A raised tolerance or a changed property assertion goes to the architect (ADR 0118, CON-31).
9. Every approved work list (an audit's batches, a review's conditions, an owner decision with follow-ups) gets a queue row in `docs/status.md` the same day, with a lane and a date. The bulldozer writes it and drives it to done, also when the content is the architect's; an item waiting on the architect is listed as waiting. The bulldozer's daily plan check reports every row past its target date, with the reason (ADR 0118).
