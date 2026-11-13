# Sessions and routing

Owner, 2026-11-12 (ADR 0120). Read at session start.

## Roles and session names
To message a role, send to its session name in this table. When a session is recreated under a new name, this table changes.

| Role | Session name | Model | Owns |
|---|---|---|---|
| owner | - | - | direction, look approvals, architecture, formats, constitution, scope, milestone exit lists, moves from the parking list |
| lead (the owner's main contact) | `opus-architect` | Opus | what, how and when: the milestone exit list, the queue in `docs/status.md`, lane orders, IDs, perf windows, the gate ledger, gate verdicts, ADRs, the contract, reviews, the daily digest |
| production lane | `sonnet-explorer` | Sonnet | engine and production PRs |
| production lane | `sonnet-crusader` | Sonnet | production PRs when the milestone has independent work; idle otherwise |
| exploration lane | `sonnet-spikes` | Sonnet unless the brief says otherwise | spikes and look experiments as spike code with picture sheets; one at a time |

## Messages
- Every lane message goes to `opus-architect`: PR ready or merged, a failed gate, a departure from an ADR or the contract, a measurement against a contract number, being idle, a perf window, an ID request.
- Non-urgent reports may instead be appended to `C:/DEV/SeriousPlanetData/lanes/<lane>-outbox.md` (one entry, starting `## <time> <subject>`, ending `--- end`).

## Rules
1. Lane orders are entries in `C:/DEV/SeriousPlanetData/lanes/<lane>.md` (`<lane>` is `explorer`, `crusader` or `spikes`), written only by the lead. A lane reads its file at the start of each turn. The lead sends a message only to stop or change running work.
2. Every work item names the exit item of the current milestone (`docs/milestones/`) that it serves. Anything else is on `docs/parking.md`, and nobody works on it (ADR 0120).
3. A lane merges its own PR when the conditions in the "Git" section of `CLAUDE.md` hold:
   - read `gh pr checks` all green in a separate command before merging;
   - re-read main's checks after merging;
   - one merge at a time.
4. A PR that departs from an accepted ADR, changes a contract number or claims to close a gate waits for the lead's review before its merge.
5. No production PR for a look direction the owner has not seen. Look work stays in spike code until the owner picks a variant (ADR 0120).
6. A new generator stage, or a rule that changes results, lands switched off. A separate PR switches it on after its gate measure passes on the production path (ADR 0118).
7. A lane may restate a test's guard, the assertion that the test exercises its case, when a fix removes what it counted, if every property assertion is unchanged; the PR body says so. A raised tolerance or a changed property assertion goes to the lead (ADR 0118, CON-31).
8. One heavy machine job at a time (a k=4 run, a region bake, a prepush with GPU tests). Perf runs only in a window the lead authorises.
