# Learnings archived from docs/learnings.md (2026-11-26, the cap of ADR 0075)

Entries of the P3 E1 river stage on K7, superseded by the river-graph work or fixed since; still true unless a later entry says otherwise.

## 2026-11-14 E1's drawn "lakes" were mostly sea on inland ground below 0 m
Scope: project (K7 sea floor, window outlets, coastal band)
- 80 % were coastal-band water on 4,764 cells K7 put 3.5-42 m below 0 m, 1-23 km inland; the 4,317 km2 margin lake is a window-edge outlet never lowered (SPIKE-112; lanes digest 2026-11-13).

## 2026-11-14 The drawn ground differed from B + L by the crease detail only
Scope: project (runtime detail, GEN-187, VIEW-072)
- 1-7 m (p50 -2.1 m), seen as water flowing uphill; the base was bit-equal. Pinning the detail cost 31 % of frames until a memo (ADR 0122; #1490).

## 2026-11-14 The flight stalls were viewer residency, not machine load
Scope: project (viewer residency VIEW-070, G-RT)
- Resident tiles grew from 10,922 to 66,114 with no eviction (2-10 s stalls above about 20k); with the cap a 26-min flight had 0 stalls (#1473, #1485).

## 2026-11-14 The width law never reaches a visible size on the prototype
Scope: project (K7 width GEN-165, river drawing)
- Widest trench 1.9 cells; the drawn width's one-cell floor makes tributaries as wide as the main stem (gsurfi-reread-2026-11-11.md).

Full text of the 2026-11-26 entry, shortened in docs/learnings.md for the cap:

## 2026-11-26 River-graph fixes ran only downstream; S0, the base and the climate were treated as fixed
Scope: project (any pipeline gated on a look or on bars measured against a stage's own input)
What happened: from G2 (2026-11-22) to 2026-11-26, at least 10 river-graph fix rows moved a defect to another stage or bar. The attribution (2026-11-25) then traced 88-93 % of the residual look pattern, plus the blocky beds and the straight valleys, to S0, the start surface (K5 and the base), which no row changed. Causes:
- The plan defined S0 as the reference of every delta measure (budget, relief lost, saddles, crests), so changing S0 looked like moving the yardstick.
- The plan listed the base and the climate (K0-K4) as unchanged inputs, with a one-direction rule.
- G3 let main take only changes that move no stored output, which ruled out base fixes.
- The lead generalised the owner's remark "M1's look stays after G3; base changes need no river-graph change" (2026-11-23) into a test for every parked item, kept the items parked although the owner asked to reconsider them, and deferred the owner's 2026-11-25 pointer to fix the straight rivers in S0.
- Kept lakes came from a geometric rule (an early keep reading plus a 10 m hump cap), not from water balance (ADR 0111 point 5), so too many lakes could never be traced to precipitation or evaporation.
Assumption it changes: ADR 0120's lead role (attribution and sequencing); ADR 0111 point 5 (lakes by water balance) was not applied in the prototype.
Proposed follow-up: routing rule 10 (attribute before fixing; upstream stages and world parameters included); gate measures against an outside reference (GLO-30 statistics) as well as against the stage's input; kept lakes by water balance when the work resumes.
Obsolete when: never.
