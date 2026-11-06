## Why
SPIKE-102 (2026-11-04): there is no shelf. The ocean floor was one smoothstep to -4,500 m, so shallow-water colour and distant coasts had nothing to show, and the coast had no bathymetric counterpart. The wave power of the coastal process (ADR 0107) reads the near-shore depth, so the shelf comes first.

## What Changes
- GEN-118: the ocean floor of the default class source falls from the coast to the shelf break over a width set by the margin type, then down the continental slope to the abyssal depth. Land is untouched (bit-identical, pinned).
- Margin type: a smooth weight from the plate mask (GEN-094): active where a convergent margin lies within a few hundred km, passive elsewhere; the width blends between the two, with no switch at a margin.
- The distance to the coast is `|smooth land depth| / |gradient|` (GEN-104: the continent stage without its detail; the detail would put noise into the slope). The profile fades in from 0 at the field's coast over 0.06 of sea depth, so that it meets the land's coast ramp, and blends into the abyssal depth over smooth land depths 0.5 to 0.9, so no step stands where the gradient stops being read.
- The margin type is `ClassSource::margin_activity`: the largest over the pairs of the three nearest plates of the pair's weight (as the mask's) times a ramp of the distance and the convergence, so it is continuous where the nearest plate changes.
- `CLASS_SOURCE_VERSION` 6 to 7 and `GLOBAL_VERSION` 7 to 8 (older stores refused; owner, 2026-11-02).
- `generate --shelf-preview DIR`: bathymetry map, a passive and an active coast region at 2 km and 10 km, and `shelf.txt` with the numbers against Earth's.
- Decided: the shelf is part of the field, no new channel; the pass-1 sea outlet and the coast ramp on land are unchanged.

## Earth-specific values (ADR 0108)
Every value is a field of `ShelfProfile`, Earth as default (`ShelfProfile::earth()`), held by the class source (`with_shelf`); a tuning changes the preset, not the kernel.

| Name | Default | Unit | Kind | Parameter |
|---|---|---|---|---|
| shelf-break depth | 130 | m | Earth state (global median, Kennett 1982) | `break_depth_m` |
| abyssal depth | 4,500 | m | Earth state (the field's former value) | `abyssal_depth_m` |
| passive shelf width | 150 | km | Earth state (tens to a few hundred km) | `passive_width_km` |
| active shelf width | 30 | km | Earth state (a few tens of km) | `active_width_km` |
| slope width | 90 | km | Earth state (3 to 6 degrees) | `slope_width_km` |
| active within | 150 to 450 | km | calibration (distance to a plate margin) | `active_within_km` |
| active convergence | 0.05 to 0.35 | mask units | calibration (as the mask's convergent gate) | `active_convergence` |
| blend into the abyss | smooth land depth 0.5 to 0.9 | land depth | calibration (bounds the gradient read) | `SHELF_ABYSS_FROM/TO` |
| coast fade | 0.06 | land depth | calibration (meets the land's coast ramp) | `SHELF_COAST_RAMP` |
| minimum water | 5 over 0.01 | m, land depth | calibration (a bay is not dry) | `MIN_SEA_M`, `SHELF_MIN_SEA_RAMP` |

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-118 (new)

Known limits: the detail of the continent stage moves the field's coast up to a few tenths of a land depth from the smooth coast; near such rugged coasts the slope steps by up to a few hundred metres over 0.6 km (the test bounds it at 400 m and checks it shrinks with the step), and the realised width in the preview departs from the parameter there. A distance to the field's own coast (a distance transform) would remove it and is the owner's call.

Risk: low (sea only; versions bump). The walk-based widths in the preview are measured along the axes of the map and at the map's resolution; they are indicative.
