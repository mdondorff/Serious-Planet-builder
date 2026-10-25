## ADDED Requirements

### Requirement: GEN-061 The fine creases follow the landform
The erosion field (`cpu-erosion-v1`) SHALL split its crease octaves at wavelength 1.6 km (octave 4 of 25 km spacing halving): the coarse octaves SHALL keep the amplitude `A(E) carve(slope)`, the fine octaves the amplitude `A(E) (carve + (1 - carve) L)` with `L` the landform driver `global::landform(seed, direction)` in `[0, 1]`. With `L` 0 the detail SHALL be the detail of version 1; with `L` 1 only the fine creases differ, and on ground where `carve` is 1 nothing differs. The field's version SHALL be 2.

Verify: A
Status: planned
Source: ADR 0080 point 2, owner decision 2026-10-25 (terrain relief into M8), GEN-064

#### Scenario: Plain and range zone
- **WHEN** the detail is evaluated at the same ground elevation and bake slope with `L` 0 and `L` 1
- **THEN** the coarse part is equal and the fine part changes by `A (1 - carve) L` times the fine crease sum

### Requirement: GEN-062 The landform has a Lipschitz constant and bounds over a cap
`global::landform_lipschitz()` SHALL return a constant `K` with `|landform(a) - landform(b)| <= K |a - b| + LANDFORM_F32_STEP` for every pair of unit directions (`LANDFORM_F32_STEP` 2e-7: the continent is rounded to f32 before use), derived from the noise gradient bound and the terms of the terrain, and a test SHALL check it against random pairs. The erosion field's height bounds over a cap of angular radius `r` SHALL take the landform in `[L(centre) - K r - LANDFORM_F32_STEP, L(centre) + K r + LANDFORM_F32_STEP]` clipped to `[0, 1]`.

Verify: A
Status: planned
Source: GEN-017, GEN-061, ADR 0074 (bounds must be sound)

#### Scenario: Random pairs
- **WHEN** 20,000 pairs of directions at distances from 1e-5 to 1e-1 are compared
- **THEN** no pair changes by more than `K` per unit of chord
