## Why
SPIKE-102 (2026-11-04): there is no shelf. The ocean floor is one smoothstep to -4,500 m, so shallow-water colour and distant coasts have nothing to show, and the coast type has no bathymetric counterpart.

## What Changes
- New requirement GEN-118: a shelf profile in the field bake: the ocean floor falls to about -130 m over a shelf width set by the coast type (passive wide, active narrow, from the plate mask), then the slope to -4,500 m.
- GLOBAL_VERSION bump (the elevation channel changes in the sea only; land is unchanged).
- Preview: coast region at 2 km and 10 km, planet bathymetry map.
- No values in this proposal.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-118 (new), GEN-096 (ocean part)

Depends on: SPIKE-102 plan row 8. Risk: low.
