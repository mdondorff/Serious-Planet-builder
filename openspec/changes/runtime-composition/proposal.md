# runtime-composition (briefing item 15: sampler composition, lakes, rivers; G-RT), design notes only

Briefing item 15 starts when items 7 and 11 are on main; item 7 is, item 11's gate and G-SURF-I report are open (explorer). These notes fix the order and the questions before code; no requirement, no code, no test in this change.

## Why
A3 section 3.1 makes the runtime surface `S = B + L + C + A` and retires `change + fill depth` (GEN-112, ADR 0101 point 3); section 5.5 gives lakes a water level from the basin balance; section 3.3 gives rivers as K6 segments drawn by K11; G-RT is the region's flight. Today's runtime still adds the fill depth, has no basin object and draws no K6 rivers.

## What
- `design.md`: the current state from the code, the target from A3, three steps (sampler composition, lakes, rivers) with their gates, what each needs from item 11 and from the bake, and the questions for the architect.
