# 0037 M5: new goldens under policy, look reviewed at the gate
Status: accepted (owner, 2026-10-15, opus-planner session); point 1 superseded by 0061. Amends ADR 0017 in part, for milestone M5 only.
Date: 2026-10-15
Source: owner's decisions of 2026-10-15 (`docs/status.md`, M5 section); ADR 0017; ADR 0035 (presentation data and declared bounds); constitution CON-18 (v1.2), CON-14 (v1.3)

## Context
M5 (atmosphere, HDR and exposure, ocean, shoreline) creates many golden images that have no predecessor: ground, limb and sunset views, ocean and shoreline views, per adapter. Under ADR 0017 a brand-new golden escalates to the owner's `/bless`, which would stop the coder at every visual feature. The owner wants M5 to run without his attention between gates.

## Decision
1. **New M5 goldens are blessed under policy** by `golden-review` (ADR 0017) when all of these hold: (a) the attribute's declared class and precision bound (ADR 0035) are in its spec and its bound test passes (for the atmosphere: the LUTs against the f64 CPU reference); (b) the feature's debug view exists (CON-18) and agrees across WARP, llvmpipe and the RTX within ADR 0017's cross-adapter rule; (c) the PR declares the new golden and what it shows. Anything else escalates as before.
2. **The look is reviewed once, at the M5 gate**, from a contact sheet and a flight video produced for the gate report. Look parameters are data (sky, sun, haze, ocean colour, exposure range) with physically based Earth defaults from the literature; no mid-milestone look stops. Changes after the review are parameter changes, re-blessed under ADR 0017's declared-change rule.
3. Owner-only stays: changes to existing non-M5 goldens outside ADR 0017's rules, and anything `golden-review` escalates.

## Consequences
- The coder is not blocked by first-time goldens in M5; a wrong look is caught at the gate, and reverting it is a parameter change.
- Reviewers check that declared bounds follow from purpose, not from the measured result (ADR 0035 point 4).
- After the M5 gate the owner decides whether this rule continues.

## Licences of adopted code or techniques
None.
