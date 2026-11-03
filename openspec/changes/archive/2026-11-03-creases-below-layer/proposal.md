## Why
- Owner decision 2026-11-03 (ADR 0098): the erosion layer owns erosion at its scales; the analytic creases act only below the layer's grid, and those fine creases are the fine-detail term of ADR 0088 point 5 (one term, not two).
- With the layer read, the crease octaves of 25 km down to 400 m would put a second, unrelated valley pattern over the layer's drainage, amplified by `k`.

## What Changes
- `CreaseBand` and `ErosionField::with_crease_band` (GEN-109): per-octave weights, 1 up to 2 layer cells (about 200 m), 0 from 4 cells (about 400 m), a smoothstep in log wavelength between; defaults 101.8 m (pass 2), 2 and 4 cells; the band is a parameter of the field.
- Where it applies: with the relief channels on and the call made, on ground at or above the band's sea level (the definition's, not 0), joined over `blend_m` (200 m) above the level so the field has no cliff along a shore (a hard step was 67 m in the synthetic test); the sea floor keeps today's creases. ADR 0098 point 3 is silent on how the two regimes join: this is that rule, to be written into the ADR.
- Off by default: without the call the field is today's, bit for bit (no golden, pin or manifest row moves). `k` stays on the fine creases (the open point of ADR 0098, to be decided on the preview stills).
- `height_bounds` take the weighted octaves only (zero-weight octaves are not evaluated), joined with today's where a cap reaches the blend zone (the blend is a convex combination, so the union holds it); nested caps nest (LOD-012). `with_relief` after the band is refused (the keys must not depend on the order of the calls).
- Keys: the band's cell, cells, sea level and blend and `CREASE_BAND_VERSION` enter the inputs hash; the implementation ids are `cpu-erosion-v1+relief+band` and `cpu-erosion-v2+relief+band`; the field version does not change. ADR 0098's consequence says "the field's version and implementation id change with the relief switch on": here they change only when the band is called (planner, 2026-11-03: off unless called, the runtime layer read calls it), so the ADR's line should read "when the band is on".

## Hand-off to the runtime layer read (m9-bake PR 6)
- The runtime field does not read the erosion layer yet (only the pass-1 store exists). `with_crease_band` is what the layer read calls when it attaches the layer, with `CreaseBand::of_the_bake(sea_level_m)` and the definition's sea level; until then nothing calls it and the field is today's field, so ADR 0098's "where a layer is read" holds once PR 6 lands.

## Conventions
- An octave's cell is its noise lattice cell, `radius / (256 2^o)`: 24.9 km for octave 0 on the Earth, 195 m for octave 7; the layer cell is that of the bake's pass 2, 101.8 m (ADR 0088), so the defaults keep octaves 7 to 11 and weigh octave 6 at about 0.012.

## Impact
- Code: `generators::erosion` only. Requirement GEN-109 added. No golden moves. Tests: the unit tests of `erosion.rs` and `tests/crease_band.rs`.
