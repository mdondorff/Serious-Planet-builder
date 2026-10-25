## ADDED Requirements

### Requirement: GEN-061 The fine creases follow the landform
The erosion field (`cpu-erosion-v1`) SHALL split its crease octaves at wavelength 1.6 km (octave 4 of 25 km spacing halving): the coarse octaves SHALL keep the amplitude `A(E) carve(slope)`, the fine octaves the amplitude `A(E) (carve + (1 - carve) L)` with `L` the landform driver in `[0, 1]` read from the landform lattice (GEN-062, the interpolant of `global::landform(seed, direction)`). With `L` 0 the detail SHALL be the detail of version 1; with `L` 1 only the fine creases differ, and on ground where `carve` is 1 nothing differs. The field's version SHALL be 2.

Verify: A
Status: planned
Source: ADR 0080 point 2, owner decision 2026-10-25 (terrain relief into M8), GEN-064

#### Scenario: Plain and range zone
- **WHEN** the detail is evaluated at the same ground elevation and bake slope with `L` 0 and `L` 1
- **THEN** the coarse part is equal and the fine part changes by `A (1 - carve) L` times the fine crease sum

### Requirement: GEN-062 The landform is read from a fixed lattice and bounded over a cap
The erosion field SHALL read the landform from `global::LandformLattice`: the samples of `global::landform` (rounded to f32) at the face coordinates `2 i / 256 - 1` of the tangent warp on each cube face, interpolated bilinearly, a pure function of `(seed, direction)` that gives the same bits to every tile and is continuous across face edges. Its range over a cap SHALL be the least and greatest sample of the cells the cap touches (`LandformLattice::bounds`), `[0, 1]` for a cap over 0.05 rad, over 16 cells an axis or reaching a face edge, and the erosion field's height bounds SHALL take the landform in that range. The cost of the landform term SHALL be at most 10 % of the height's cost (measured 3 to 4 %).

Verify: A
Status: planned
Source: GEN-017, GEN-061, ADR 0074 (bounds must be sound)

#### Scenario: Random caps
- **WHEN** 20,000 random caps of radius 1e-6 to 0.04 rad are taken
- **THEN** the lattice value at the centre and at a point of the rim lie in the range of the cap, the lattice differs from `landform` by less than 0.1, and a pair 1e-6 apart differs by less than 1e-3
