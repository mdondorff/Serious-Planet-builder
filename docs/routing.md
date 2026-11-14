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

## Communication (owner, 2026-11-14)
Three levels; level 1 is the default, level 3 the exception.
1. **State, no message.** Order files, `<lane>-state.md`, the outbox (`C:/DEV/SeriousPlanetData/lanes/<lane>-outbox.md`, entries `## <time> <subject>` … `--- end`), the 2-hour heartbeat line, git.
2. **Notification, one message, no answer expected.** Only for:
   - a merged PR or a finished item that unblocks another lane (to that lane, and to `opus-architect`);
   - being blocked, with the blocker named;
   - a failed gate, or a correctness or integration risk.
3. **Decision, one request, one answer.** Only for what needs the lead: a departure from an ADR or the contract, a cross-lane change, a measurement against a contract number, an ID, a perf window. Use the template below.

Rules:
- No progress messages, no acknowledgements, no "received". Never a second message to clarify the first; make the first self-contained.
- **Peers first.** An interface owned by another lane: message that lane with a concrete proposal. The lead hears of it only if the two disagree.
- **The machine through the tooling.** Every heavy job (a bake, a probe, a replay, a test run, a spike example) runs as `cargo xtask heavy <command>` or as an xtask command that takes the lock. No messages about machine order. The owner clears the machine with `cargo xtask quiet on`.
- **Never idle while waiting.** Every decision request names a default. While waiting, work on the default or the next item in the order file.
- **Never end a turn without a pending wakeup.** A lane loop ends every turn with a wakeup at most 60 min away, also while waiting for an answer. A file watch alone is not enough: it expires.
- The lead's answers and rulings are also written into the lane's order file, so that a cleared or restarted session finds them.

Decision request (level 3):
```
DEC <lane>-<n>: <title>
To: opus-architect   Blocking: yes|no
Problem: <one or two lines, with numbers>
Options: A <...> | B <...> | C <...>
Recommendation: <letter>, <one line why>
Default: <what I do now, and if no answer by <time>; never "wait">
Evidence: <file paths>
```
Answer: `DEC <lane>-<n>: <letter>. <condition>`. `DEC` numbers live only in the lanes folder; the repository never cites them.

## Rules
1. Lane orders are entries in `C:/DEV/SeriousPlanetData/lanes/<lane>.md` (`<lane>` is `explorer`, `crusader` or `spikes`), written only by the lead. A lane reads its file at the start of each turn. Every lane file holds the lane's next two items at all times. The lead sends a message only to stop or change running work, or to answer a decision.
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
