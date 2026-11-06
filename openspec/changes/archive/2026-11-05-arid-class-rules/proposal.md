## Why
- Badlands and sand sea are in the class list (GEN-095) but no rule gives them weight: 0 % of land, against about 3 % and 8 % in the SPIKE-77 targets. Owner (2026-11-05, ADR 0104 point 4): default rules from soft rock, aridity, height, relief and sand supply, through the class-weight interface (ADR 0089). The aridity comes from the orographic climate (GEN-117, change `orographic-precip`).
- The independent review (planner inbox #74, constraint 3): smooth weights; the badlands effect through K (soft rock, GEN-116) and the detail band, not a hard relief-amplitude step at a class edge.

## What Changes
- GEN-128: arid class rules, a pure function of the aridity, the lithology hardness, the base elevation, the relief terms and the sand supply, giving the badlands and sand-sea weights; `BakeInputs::weights_at` blends them over the base weights, so the class source, `ClassLattice` and the class source's elevation (`e0`, which the climate reads) are unchanged and the climate never reads a weight.
- `ARID_RULES_VERSION` joins `BakeInputs::id()` together with the climate identity (the base elevation now depends on them): every layer and global key of a class-source bake changes.
- A planet preview of the class map with the shares against Earth's and the two scorecard sites (badlands, Erg Chebbi) before and after: `generate --arid-preview DIR`.
- Not here: runoff in the carve (explorer's), dunes (runtime), glacial classes (ice-history).

## Impact
- Code: `generators` (`arid.rs`, `bake/inputs.rs`), `app` (the option). Requirement: GEN-128.
- Relief changes where the rules give weight: class amplitudes and the scorecard are re-tuned in the owner's round.
- All numbers are start values.
