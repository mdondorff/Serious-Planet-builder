# 0052 First M5 goldens: an edge tolerance for the cross-adapter debug-view check (amends ADR 0037 point 1(b))
Status: accepted (owner, 2026-10-17, opus-planner session)
Date: 2026-10-17
Source: ADR 0037 point 1(b); ADR 0017 (rounding noise: 32 px, delta 1; cross-adapter agreement); TEST-022; the reviews of the lit and sky goldens (PRs #199, #204, #207)

## Context
ADR 0037 point 1(b) blesses a first M5 golden under policy only when its debug view agrees across WARP, llvmpipe and the RTX under ADR 0017's cross-adapter rule, which `golden-review` reads as rounding noise: at most 32 differing pixels, each within delta 1. The first 14 M5 goldens (8 lit, 6 sky) have green bound tests and are declared, yet escalate on (b) alone: an EV-band view differs by 1 pixel of delta 95 where a pixel at a band edge falls into the neighbouring band, and the continuous false-colour views differ by 5 to 40 pixels, mostly delta 1, with delta 4 to 11 at 5 to 10 pixels on the planet's silhouette. Adapters rasterise edges and coverage differently; the shading itself is checked by the bound tests against the f64 references. The water views will meet the same edges.

## Decision
1. For the **first golden of an M5 class** under ADR 0037, condition (b) holds when the declared debug view's candidates agree pairwise across the participating adapters with **every pixel within delta 1 except at most 64 pixels, which may differ by any amount**.
2. Everything else in ADR 0037 and ADR 0017 is unchanged: at least two participating adapters, an adapter that did not run is never counted as agreeing, the driver gate, bound tests green in this run, the golden declared, a changed existing golden escalates, and non-M5 classes keep ADR 0017 exactly.
3. Every M5 golden blessed under policy appears on the M5 gate's contact sheet for the owner's look review (ADR 0037 point 2).
4. The window ends with ADR 0037's: the owner decides at the M5 gate whether it continues.

## Consequences
- The 14 pending goldens are reviewed again with the tool once it implements point 1 (CI WARP, CI llvmpipe, the local RTX; one golden per line in the proposal).
- A real defect confined to at most 64 edge pixels of a debug view could pass this check; the bound tests and the owner's look review are the guards for that.
- `golden-review` gains the constant and its tier-A tests (TEST-022 modified).

## Licences of adopted code or techniques
None.
