## Why
`m8-relief` part 3, the field side (ADR 0080 point 2): the bake carries the hill band for wavelengths of 2 to 8 km (GEN-064, `global::landform`); the erosion field's creases (`cpu-erosion-v1`, spacing 25 km down to 12 m) keep their amplitude keyed to the bake's slope alone, which is small on every plain and every hillside the bake resolves (p99 under 2.4 degrees at 611 m), so the wavelengths under 1.6 km carry little relief in the hills and ranges.

## What Changes
1. The crease octaves split at octave 4 (wavelengths under 1.6 km): the fine octaves take the bake slope's factor lifted towards 1 by the landform driver, `carve + (1 - carve) L`; the coarse octaves are unchanged (`L` 0 is today's detail exactly).
2. The field reads the landform from `global::LandformLattice` (256 cells a face edge, bilinear, a pure function of the seed and the direction; planner's condition after the first version cost +58 % per height). Its range over a tile's cap is the range of the samples of the cells the cap touches, so the field's height bounds stay sound (the random-cap tests of GEN-017 cover it).
3. `EROSION_VERSION` 2: the field's key changes and every height over land with a landform above 0 and a bake slope below the steepest moves.

## Tests
- Tier A: the detail with `L` 0 is the old detail and with `L` 1 only the fine part is lifted (nothing changes on the steepest ground); the lattice bounds hold for 20,000 random caps and the lattice is continuous and close to the driver; the bounds of GEN-017 hold over random caps; the known answers re-pinned.

Expected golden changes: none (the render goldens draw the synthetic `Reference` field).

## Limits (numerics review)
- **Cost.** Measured with `examples/erosion_cost.rs` (200,000 random directions over a 512 bake, release, one thread): with `landform` evaluated at every height (17 noise evaluations) +58.0 % per height (2.27 us against 1.43 us); with the lattice +3.3 to +4.3 % (1.47 to 1.52 us against 1.43 to 1.46 us). The lattice takes about 0.5 s to build once per seed (396,000 samples, six threads) and 1.6 MB.
- **Looser bounds on wide caps.** A cap over 0.05 rad (about 320 km), reaching a face edge or touching over 16 cells an axis gets the landform range `[0, 1]`; narrower caps get the range of their cells, so the bound is tight at the tile sizes that matter and loose only at the top LOD levels.
- **The lattice is not the driver.** The interpolant differs from `landform` by less than 0.1 (a smoothing at about 39 km); the creases follow it, the hill band keeps the driver itself.
- **Known answers.** The re-pinned heights hold one landform-active point of three; the others sit on sea or plain.
