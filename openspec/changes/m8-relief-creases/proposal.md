## Why
`m8-relief` part 3, the field side (ADR 0080 point 2): the bake carries the hill band for wavelengths of 2 to 8 km (GEN-064, `global::landform`); the erosion field's creases (`cpu-erosion-v1`, spacing 25 km down to 12 m) keep their amplitude keyed to the bake's slope alone, which is small on every plain and every hillside the bake resolves (p99 under 2.4 degrees at 611 m), so the wavelengths under 1.6 km carry little relief in the hills and ranges.

## What Changes
1. The crease octaves split at octave 4 (wavelengths under 1.6 km): the fine octaves take the bake slope's factor lifted towards 1 by the landform driver, `carve + (1 - carve) L`; the coarse octaves are unchanged (`L` 0 is today's detail exactly).
2. `global::landform_lipschitz()` (a constant derived from the noise's gradient bound and the structure of the terrain terms, 1.05 margin) bounds the landform over a tile's cap: `landform(centre) +- K r`, clipped to `[0, 1]`; the field's height bounds use it, so they stay sound (the random-cap tests of GEN-017 cover it).
3. `EROSION_VERSION` 2: the field's key changes and every height over land with a landform above 0 and a bake slope below the steepest moves.

## Tests
- Tier A: the detail with `L` 0 is the old detail and with `L` 1 only the fine part is lifted (nothing changes on the steepest ground); the landform constant holds against 20,000 random pairs; the bounds of GEN-017 hold over random caps; the known answers re-pinned.

Expected golden changes: none (the render goldens draw the synthetic `Reference` field).
