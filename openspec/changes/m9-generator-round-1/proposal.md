## Why
The planner's critique of the preview's iteration 0 (inbox #39, 2026-11-01): the coastal arcs ring whole continents, the belts are tens of km wide and unbroken, the gates switch in straight edges at plate margins, one polar belt is about 15,000 km long, and the largest step of `k` is 6.5 against GEN-097's 2.0.

## What Changes
- **Class source (planet-generators `classes`):** coastal belts only at convergent margins with an oceanic side; belts of 220 to 500 km (tent profile) cut into segments by low-frequency noise, with the height varying along the strike; every gate smooth over 100 to 250 km, the mask continuous where the nearest plate changes (pairs of the three nearest plates, symmetric terms); the land gate over 250 km inland; the mountains membership is the core itself, and the high-mountain split reads an elevation without the ridge noise's fine structure.
- **Preview report:** SPIKE-91's S1 to S7 beside Earth, the share of lattice steps of `k` over 2.0 (`shape.rs`).
- **Planned spec text (m9-bake, GEN-094, GEN-095, GEN-096; and GEN-103, TEST-086 of the preview change):** rewritten to match, with the planner's rulings on the iteration-0 departures (the plateau as a difference, the estimate of the coastal distance, the land share, the body's radius from the definition for the bake).
- Not in this change: the continents (round 2), wiring into the bake, R7's tuning.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-094 to GEN-096 (planned, in `m9-bake`), GEN-103 (planned, in `m9-world-preview`)
- `testing`: TEST-086 (planned, in `m9-world-preview`)
