# P3 report: rivers across scales, measured and decided

Status: closure report (lead, 2026-11-27); signed off (owner, 2026-11-27). P3 ran from 2026-11-14 to 2026-11-28, its last closure row being ADR 0130's core build. Its river-graph exit was parked with record on 2026-11-26 (owner; ADR 0129, accepted 2026-11-27).
Not run: `cargo xtask accept P3`. Its perf wall-clock step needs a perf window (ADR 0020), and none is authorised. P3's exits are not perf exits, so this report closes them on their own evidence.

## Summary
- P3 measured how drainage becomes valid at L (E1-E3), then built a multi-scale river-graph chain (RG) as the candidate over B, main's chain (E6, G0-G3).
- G2 (2026-11-22) chose RG over B. G3 (2026-11-23) set nine pre-production rows; none passed in full by 2026-11-26, and the owner parked RG on two independent reviews.
- What main kept is candidate-neutral and serves B:
  - the gate's own connected sea and integrity bars;
  - the planet primitives (GlobalCell, CubePatch, the seam spill engine, cell area);
  - REND-068's bilinear normals;
  - the look sheets with switch and stage diffs (BUILD-067, BUILD-071);
  - the metric calibration harness (TEST-155);
  - wave 1's lean memory on B's bake (GEN-192).
- The record is in `docs/archive/P3/river-graph/`. The code is on the tags `archive/rg-parked-2026-11-26`, `archive/rg-g2-record-2026-11-22` and `archive/rg-k5p-sills-2026-11-17`.

## Exit items
| # | Exit item | Result | Evidence |
|---|---|---|---|
| E1 | The measurement suite frozen | done | `cargo xtask river-gate` (BUILD-055 and later) |
| E2 | The verification items of the critical reads | done | P3.md E2 row |
| E3 | D-1: how drainage becomes valid at L | done 2026-11-16, closed as the benchmark | the D-1 table (#1557, #1558) |
| E4 | D-2 and D-3: the river hierarchy, its data contract and region boundaries | done: parked with record | ADR 0129 point 5 (ADRs 0125, 0126 parked) |
| E5 | D-4: lakes and basins | done: parked with record | ADR 0129 point 5 (ADR 0127 parked); the basin objects are in the record |
| E6 | River graphs across scales: G0 to G3 | done: parked with record | ADR 0129; `docs/archive/P3/river-graph/g3-snapshot-2026-11-26.md` (the rows, the effort left) |

## Queue as executed (from 2026-11-22; the earlier queue is in the log)
- G2 closed (2026-11-22); G3 adopted (2026-11-23).
- G3-A: the gate's own sea (#1644), the G3-1 bars and windows (#1648), the capped relief bar (#1651), look sheets (#1650), width columns (#1659).
- G3-B: O8's step 1 rows on the spike (look not passed, 2026-11-25); the attribution (SPIKE-143 follow-ups); the calibration harness (#1665).
- G3-C: the planet primitives (#1654, #1655, #1658, #1661); the cross-seam breach proofs and the three-wave runner on the spike; G3-6's wave 1 at the full count, 9.26 GB against 10.8 GB (2026-11-26).
- Park (2026-11-26/27): #1672, #1673 (the record), #1674 (stage sheets), #1675 (GEN-192), and this closure.

## Open risks carried
- B keeps its own defects: crest and saddle loss against S0, R3a's undrained pits under the connected sea, and the hump trenches. They are recorded as B's in ADR 0129.
- The base's look defects (K5's washboard and straight trunks, K7's read) are in `docs/parking.md`. They affect every chain.
- The editor's edit model below erosion is undecided (BUDG-003/005; ADRs 0112, 0113).

## Learnings
- Seven entries of 2026-11-26 in `docs/learnings.md`; routing rules 10-13 in `docs/routing.md`.

## Proposed ADRs awaiting the owner
| ADR | Subject |
|---|---|
| 0129 | River graphs parked (the record of the owner's park decision) |
| 0124 | Drainage at L in one scaled round: B's path; stays proposed |
| 0045 | Bodies, bake resolution and the storage budget (point 2 superseded by 0115) |
| 0117 | Driver regions as a world-level table |
| 0128 | The multi-harness companion lane |

## Process review (recorded data; compared with P1-P2 where the data exists)
| Measure | P3 | Source |
|---|---|---|
| PRs merged to main | 187 (2026-11-14 to 11-27); about 35 of them docs and status | `git log origin/main` |
| Reverted changes | 0 | git log |
| Spike branch | 537 commits, +38,149 lines, 96 chain switches; none ported | `archive/rg-parked-2026-11-26` |
| Reworked fix rows | at least 10 that moved a defect to another stage or bar (2026-11-22 to 11-26) | the record, both feasibility reviews |
| Bars withdrawn after registration | 5 of 7 look bars, which failed calibration two days later | TEST-155, O13, O14 |
| Coordination stalls | lane messaging paused twice at the app's 10-send limit (11-25, 11-26); a queuing script outlived its stop (11-26); the heavy lock serialised three lanes for hours (11-26/27) | lanes digest, outboxes |
| Owner load | eight owner decisions open at once (11-25); three planned flights unflown | the decisions page of 2026-11-24 |
| Perf windows | one (G3-6 k=4, 4.4 h, a verdict) | the gate file, 2026-11-26 |
- Improvements, by gain against cost (in routing rules 10-13 and the parking list):
  1. attribute before fixing, upstream stages and world parameters included (rule 10);
  2. calibrate a bar and set its value before any row runs (rule 12);
  3. cap the queue to the owner's review rate (rules 11, 13);
  4. an `archive-milestone` xtask, so housekeeping can't skip a milestone (proposal S1 in the closure plan).

## Recommendation for the next milestone
- The P4 draft (`docs/archive/P3/P4.md`) is built on RG's D-1 to D-4 and is superseded by the park. The owner picks the next topic. Candidates from the parking list are the base work (rivers off), the editor's edit model, and non-river topics.
- Keep one R&D lane, and the owner's flight at every look gate.
