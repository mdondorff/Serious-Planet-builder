# 0063 Slot v2 without the 5x5 reserve: 9,168 B
Status: accepted (opus-planner, 2026-10-18; corrects a figure in owner-accepted ADR 0062 to a smaller cost; the owner confirms at the M5 gate). Amends ADR 0062 point 4; with ADR 0062 amends ADR 0033 points 3 to 5 and ADR 0045 point 3.
Date: 2026-10-18
Source: SPIKE-12 results Q3 (slot 8,840 B; +3.7 % with the wide texel and the climate block; +5.9 % only with the rejected 5x5 header); sonnet-m4's draft of `compact-slot-v2` (#302)

## Context
- ADR 0062 point 4 states a slot of 9,360 B (+5.9 %). That figure includes 192 B reserved for the 5x5 base header, which ADR 0062 point 5 does not adopt.
- Without the reserve the slot is 9,168 B (+3.7 %).
- The slot count drop is measured against 231,090 slots (not 230,640), and pre-fill levels 3 to 4 are 1,920 tiles (not 2,040).
- The changes touch the pool share points of ADR 0033 (points 3 to 5) and the pre-fill levels of ADR 0045 point 3.

## Decision
1. The v2 slot holds no reserve for a 5x5 header: 9,168 B at cells 32. A later per-tile block grows the slot by its own decision.
2. The slot, block and tier counts and the pre-fill tile count are the specs' numbers (GEN-025, STRM-006, STRM-021).

## Consequences
- More slots per block than ADR 0062 assumed; nothing else in ADR 0062 changes.
- `compact-slot-v2` is built on 9,168 B.

## Licences of adopted code or techniques
None.
