# probe-hillshade (briefing item 13, step 6)

Architect 2026-11-10 (sheet review): the z sheets use a fixed colour scale on which 0 to 500 m land is one green, so they show no relief.

## What
- `SheetProbe` writes `<stage>-<field>-shade.png` for every `FieldKind::Height` field: a hillshade of the sheet's bin means (light from the north-east), the relief read against the sheet's own rms slope over land bins, so no scale or exaggeration is chosen. Sea bins one dark blue, bins without a cell grey.
- The bound is unchanged: it reads the sheet's `sum` and `count` buffers and allocates only the RGBA bytes of the image, after the colour sheet's bytes are released (a stage's peak does not rise).
- BUILD-046 gains one sentence; no new requirement. One test cites BUILD-046 (bins, light direction, scale-free).

## Not in this change
- Hillshade of other kinds; the manifest (shade sheets have no line).
