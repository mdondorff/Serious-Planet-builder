# 0130 Mode-based sprints: a mode on every exit row, records at mode changes, rules as checks
Status: proposed (lead, 2026-11-27). Applied while proposed, on the owner's word of 2026-11-27: "Don't block on me on the ADR acceptance ... start executing towards the defined milestone as the process defined". The pilot sprint's close report judges whether it worked.
Date: 2026-11-27
Source: lanes digest `review/sprint-process-plan-combined-2026-11-27.md` (Fable's and Carbon's plans combined, with Carbon's amendments A1-A4); ADR 0120, ADR 0129; CON-31

## Context
- P3's river-graph work slid between research, prototype and production without deciding to (ADR 0129's record): research closed with no answer, a prototype was promoted with its own criteria unmet, and the gate ran all three on one branch.
- ADR 0120 point 10 (every rule is a check) was broken under pressure: routing rules 10-13 were added as text.
- ADR 0120's own review date (2026-11-19) passed without the review.
- Eight owner decisions were open at once, and three planned flights went unflown.

## Decision
1. **The sprint is the milestone** (amends ADR 0120 point 3). One file `docs/milestones/<M>.md` is active at a time. It runs 1-5 days, 7 at most (`Opened:`, `Closes by:`). It holds at most five exit rows, and rows may close daily. It names its objective, its out-of-scope lines, an owner look date and the R&D lane.
2. **Every exit row has a mode** (amends ADR 0120 point 6): R research, P prototype, X production, M maintenance.
   - R and P work lives on `spike/*`, and never reaches `crates/*` on main.
   - X is today's engineering process.
   - M is X on an existing requirement, with no OpenSpec change folder when no spec text changes.
3. **Every exit row also has a gate cell** in one of four classes: numerical (no exception); functional (owner exception, dated); domain (calibrated, TEST-155, with its value set before any row runs); owner look (the owner alone). It also has a park criterion: a counter, a limit and a default.
4. **A mode change needs a one-page record.**
   - A research close uses the research package in the spike's `results.md`.
   - A promotion or a park uses `docs/milestones/<M>/<E>-record.md`, with a criteria table, the look verdict and what stays on the tag.
   - No promotion carries a failed or empty verdict, and a numerical criterion takes no exception.
5. **Decisions** (amends ADR 0120 point 8): at most three open, each with a recommendation, a default and a `By` date.
   - Only safe defaults (park, keep-off, keep today's) fire on silence. A scope, bar, trade-off or promotion decision pauses its row until the owner answers.
   - Only owner lines starting with `decision:` or `look:` change a sprint file or a record. Every other message changes nothing (amends ADR 0120 point 9).
6. **The check** (amends ADR 0120 point 5): `cargo xtask plan` v2, spec-lint and `pr-body-check` enforce points 1-5 (BUILD-074 to BUILD-077).
   - Past `Closes by`, a sprint is `overdue`, and only PRs that close, carry or park an exit may merge.
   - `docs/routing.md`'s rules carry a `Check` cell, and an empty cell fails spec-lint (ADR 0120 point 10 made mechanical).
7. **The review is part of every close.** A sprint closes with its report, the owner's sign-off and a fixed process section: the combined plan's counted measures. There is no separate review date to miss.

## Consequences
- Lanes have freedom inside a row. The boundaries (modes, scope, records, decisions, the close) are checked by tools.
- The owner's part per sprint is about 20-30 minutes to open, one dated look, at most three decisions with defaults, and about 15 minutes to close.
- The ledger park counters, the full rewrite of the routing table and the remaining skills wait until a sprint's review shows the need.
- The pilot is S-Region-Authoring, and its exit E4 confirms the process worked up to its close.
