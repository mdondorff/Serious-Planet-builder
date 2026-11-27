# 0129 River graphs parked
Status: proposed (lead, 2026-11-26; the decision to park is the owner's, 2026-11-26; this record waits for the owner's acceptance)
Date: 2026-11-26
Source: P3 E6, `docs/archive/P3/river-graph/` (snapshot, both feasibility reviews, both park plans), ADR 0112, ADR 0113, ADR 0118, ADR 0119, ADR 0120

## Context
- P3 E6 built a multi-scale river-graph chain (RG) on `spike/rg-prototype` from 2026-11-16: 537 commits, `rg_chain.rs` at 6,425 lines with 96 switches.
- G2 (2026-11-22) chose RG over B, main's chain, with five of the plan's own G2 criteria unmet. G3 (2026-11-23) set nine gate rows; by 2026-11-26 none had passed in full.
- RG's look was judged on S0, its start surface, which the plan held fixed. 88-93 % of the residual look pattern traced to S0 (K5 and the base), not to RG.
- Two independent reviews (Fable, Carbon) recommend parking. The owner parked RG on 2026-11-26.

## Decision
1. RG stops. `spike/rg-prototype` is frozen at the tag `archive/rg-parked-2026-11-26`; no RG stage is ported to main.
2. B stays the production chain. Its known defects are B's (crest p95 loss 15-316 m and saddles 40-27,108 on the five windows; R3a's undrained pits under the connected sea) and are not reasons to resume RG.
3. What lands on main from the work:
   - wave 1's memory levers that live in B's own bake, bit-identical (GEN-192); base1's parallel decide and the K6 levers of RG's hydrology stay on the tag (base1 is an RG stage);
   - the stage sheets with diffs (BUILD-071);
   - the gate integrity work, the planet primitives and the calibration harness already merged;
   - the record in `docs/archive/P3/river-graph/`.
   Everything else is reference on the tags.
4. A restart needs all of these:
   - the base is signed off with rivers off, including the world parameters that feed hydrology;
   - an ADR decides the editor's edit model below erosion, within BUDG-003/005;
   - one lane, and about an hour a day of owner time;
   - a pre-set H1-on-sills test passes on the new K5 before any L stage is written;
   - gates are calibrated (TEST-155) and set before any row runs, with the owner's flight at every look gate;
   - rivers and lakes are gated on discharge, not on a dry-ground drainage raster.
5. ADR 0123's draft (#1499) is closed as superseded by this record. ADR 0124 stays proposed, as B's path. The drafts of ADRs 0125-0127 (lanes digest `review-v4/`) are parked with RG.
6. P3's exits E4, E5 and E6 close as parked with record.

## Consequences
- Lane time returns to topics the owner names next. The base work (K5's hillslope and planform, K7's read) waits in `docs/parking.md` with RG.
- A restart starts from the record's restart forms and dead ends, not from the tagged chain.
- Routing rules 10-13 (`docs/routing.md`) carry the process lessons to any later work.
