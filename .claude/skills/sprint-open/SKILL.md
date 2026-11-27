---
name: sprint-open
description: Draft and open a sprint, the milestone file docs/milestones/<M>.md of ADR 0130 (modes, gates, park criteria, at most five exits and three decisions). Use when the lead prepares the next sprint for the owner, and when the owner writes `decision: open <M>`.
---
The sprint is the milestone (ADR 0130, proposed, applied on the owner's word of 2026-11-27). One file is active at a time. The lead drafts it; the owner opens it.

**1. Draft** `docs/milestones/<M>.md` (at most 4 KB) in exactly this shape:

```
# <M>: <topic in one line>
Status: draft (lead, <date>)
Opened: <date>   Closes by: <date, at most Opened + 7>   Owner look: <date> | none   R&D lane: <lane>
Objective: <one falsifiable sentence>
Out of scope: <one line each; everything else goes to docs/parking.md>
## Exit items (at most 5)
| # | Item | Mode | Done when | Gate | Park | Lane | State | Result |
## Decisions (at most 3 open)
| # | Decision | Lead's recommendation | Default | By | State |
## Changes
## Close
```

- **Mode:** R (research, `spike/*`, ends in a research package: `/spike`), P (prototype, `spike/*`, ends in a promotion or park record), X (production, `change/<id>` with OpenSpec), M (X on an existing requirement; no change folder when no spec text changes).
- **Gate:** `<class>: <spec ID or value, date set>`, several separated by `;`. Classes: `num` (no exception), `func` (owner exception, dated), `dom` (names its calibration TEST ID; value set before any row runs), `look: <sheet or flight>` (the owner alone).
- **Park:** a counter, a limit and a default, e.g. `rows 2 -> park, keep today's`.
- **State:** `open`, `running`, `look-pending`, `waiting-owner`, `done <date>`, `promoted <record>`, `parked <record>`, `carried <M>`.
- **Decisions:** each has a recommendation, a default and a `By` date. Only park, keep-off or keep-today's defaults fire on silence; a scope, bar, trade-off or promotion decision pauses its row (`waiting-owner`) until the owner answers. No irreversible action is a default.
- Rows come from the owner's topic and the top of `docs/parking.md`; a row a lane can't finish in the sprint is split or left out.
- Carried rows of the previous sprint come first.

**2. Show the owner one page:** the draft plus the top of the parking list. The owner sets the topic, each gate value (or `look`), the out-of-scope lines, the look date and the R&D lane, and may answer decision rows now.

**3. Open** on the owner's `decision: open <M>` line (only `decision:` and `look:` lines change a sprint file):
- set `Status: active (owner, <date>)` and `Opened:` to that date, and fold the owner's answers into the cells;
- run `cargo xtask plan` and `cargo xtask spec-lint`; both green means the sprint is open, and no row starts before that;
- one docs-only PR, merged by the lead;
- tell each lane its rows in its order file.

**During the sprint:** a change to an exit, gate or park cell needs a line under `## Changes` (`- <date> | <cell> | <old -> new> | <who decided>`); without the owner's `decision:` line there is no change.

Sprint: $ARGUMENTS
