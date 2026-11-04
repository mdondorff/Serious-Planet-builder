## Why
SPIKE-102 (2026-11-04): climate ignores relief. There is no wind, no rain shadow and no aridity index, so arid classes have no rule and the carve's K·t ignores runoff. Owner D3 (2026-11-04): wind and orographic precipitation with arid default rules in M9, with a preview; option C stays.

## What Changes
- New requirement GEN-117: a wind field (prevailing direction by latitude band, noise warp) and an orographic precipitation sweep: moisture advected upwind, rain-out on windward slopes, shadow on the lee, over the pre-carve base on a 2048-per-face lattice, fixed order, f64.
- Aridity index AI = P / PET (PET = 300 + 45 T, SPIKE-81) as a derived value.
- The precipitation channel changes values: GLOBAL_VERSION bump.
- Arid default class rules (badlands, sand sea) through the class-weights interface (GEN-095).
- Runoff in the carve (K·t scaled by P) follows as its own PR.
- Whole-planet preview maps: P against today's, aridity, wind arrows.
- No values in this proposal.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-117 (new), GEN-095 (arid defaults), the climate rules of the field bake

Depends on: SPIKE-102 plan rows 5 to 7. Material colours change globally (the owner's look).
