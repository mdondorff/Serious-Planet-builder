# S-Test: a fixture sprint for plan v2 (TEST-161)
Status: active (owner, 2026-12-01)
Opened: 2026-12-01   Closes by: 2026-12-05   Owner look: 2026-12-04   R&D lane: spikes
Objective: Region boxes are marked, baked and reopened in the editor.
Out of scope: editing source maps
- cross-face regions

## Exit items (at most 5)
| # | Item | Mode | Done when | Gate | Park | Lane | State | Result |
|---|---|---|---|---|---|---|---|---|
| E1 | Region file | X | bake-region bakes a valid commit | fun: BUILD-074 (2026-12-01) | days 3 -> park, keep today's | explorer | running | - |
| E2 | Erosion C2 read | P | the C2 read passes its criteria | num: bit-identical off (2026-12-01); look: R1h sheet | rows 2 -> park, keep today's | crusader | promoted E2-record.md | pass |
| E3 | Hump survey | R | the research package answers its question | dom: TEST-155 (2026-12-01) | days 2 -> park | spikes | open | - |
| E4 | Tidy the queue | M | the queue names every exit | fun: BUILD-053 (2026-12-01) | - | lead | open | - |

## Decisions (at most 3 open)
| # | Decision | Lead's recommendation | Default | By | State |
|---|---|---|---|---|---|
| D1 | Cube margin | 4 cells | keep today's | 2026-12-02 | open |
| D2 | Promote E2 | promote | promote E2 | 2026-12-02 | open |

## Changes
- 2026-12-01 | E1 Gate | - -> fun: BUILD-074 | owner

## Close
- report: docs/milestones/S-Test-report.md; signed off: -
