---
name: sprint-close
description: Close the active sprint (ADR 0130): final row states and records, the report with its fixed process section, the owner's sign-off, `cargo xtask archive-milestone`, and the session reset. Use at a sprint's `Closes by` date, or earlier when every exit row is closed.
---
A sprint closes with its report, the owner's sign-off and the process section below; there is no separate review date (ADR 0130 point 7, proposed, applied on the owner's word of 2026-11-27).

**1. Final states.** Every exit row of `docs/milestones/<M>.md` ends as `done <date>`, `promoted <record>`, `parked <record>` or `carried <next M>`.
- A promoted or parked P row has its record `docs/milestones/<M>/<E>-record.md` (at most 4 KB): the criteria table (`criterion | class | value and date set | result | verdict`), the owner's look verdict with date and sheet, and what stays on the tag. A promotion also has the `research-review` agent's "ready" and the owner's `decision: promote <E>`.
- A closed R row has its research package with a `Decision:` line (`/spike close`).
- A carried row is copied, unchanged, into the next sprint's draft.
- Run `cargo xtask plan`; it must be green.

**2. The report** `docs/milestones/<M>-report.md` (at most 6 KB), in this shape:

```
# <M> report
Sprint: <M> (<Opened> to <closed date>)   Signed off: <owner, date | pending>
## Exits
| # | Item | Mode | Final state | Gate result | Record |
## Decisions
| # | Decision | Answered or default taken (date) | Outcome |
## Process
| Measure | Target | This sprint | Previous |
|---|---|---|---|
| Rules without a check | 0 | | |
| Most open decisions at once | at most 3 | | |
| Decisions defaulted against answered | - | | |
| Defaults fired against rows paused on silence | - | | |
| Unprefixed owner messages that changed the file | 0 | | |
| Owner minutes per week | under 120 | | |
| Looks judged by their date | all | | |
| Rows run per exit against its park counter | within | | |
| Records for every closed P row | all | | |
| Planned against used lane-days | - | | |
| Merged or run work `plan` did not see (from the third sprint on) | 0 | | |
## What the lead would change
## Next sprint
<the draft's path, or "none">
```

- Fill every Process cell with a count and its source (`plan` output, the file's `Changes`, the lanes' outboxes). An unknown value is written `unknown: <why>`, never left empty.

**3. Sign-off.** The owner reads the report (about 15 minutes) and writes `decision: close <M>; open <next>` (or `close <M>` alone). Set the sprint file's `Status: closed (owner, <date>)`, fill its `## Close` line, and the report's `Signed off:`.

**4. Archive.** Run `cargo xtask archive-milestone <M> --dry-run`, read the plan, then `cargo xtask archive-milestone <M>`. It merges the log fragments, moves the closed spike rows, and moves the closed sprint file, its report and its records folder into `docs/archive/<M>/` (BUILD-077). Update every live file it prints as still naming a moved path. Commit the result with steps 1-3 in one docs-only PR, merged by the lead.

**5. Session reset.**
- Every lane writes `lanes/<lane>-state.md` (its role, what is done, what is open, its worktree) and a line in its outbox.
- The lead writes its handoff note (`lanes/digest/lead-handoff-<date>.md`) and each lane's `/loop` line.
- The owner clears the sessions; each lane starts from its state file and the new sprint file.

**6. Open the next sprint** with `/sprint-open`.

Sprint: $ARGUMENTS
