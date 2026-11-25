## ADDED Requirements
### Requirement: TEST-155 Geometric metrics on synthetic valleys and on S0
`planet_cache::metric_calibration` SHALL build, deterministically, a reference surface and a valley surface for every case below, and SHALL run a `GeometricMetric` on each. A metric's calibration SHALL pass only when every smooth case reads inside the metric's declared band (angle-invariant) and every defect case reads outside it (detects its defect). The harness SHALL write each metric's table to `target/review/metric-calibration/<metric>.txt`.

The cases:
- angles 0, 22.5, 45, 67.5 and 135 degrees;
- profiles V and round;
- floors: smooth, and the defects stair4, stair8 and sawtooth;
- backgrounds: none, fBm and S0.

Verify: A
Status: active
Source: Lead 2026-11-25 (after the owner's caution and the external process review: M4, in-zone Xdiag and D8 sinuosity would have failed this), spikes #184f, BUILD-069
Detail:
- the grid:
  - 256 x 256 cells of 101.802 m, the R1h L cell;
  - one straight valley whose axis passes 0.37 cells across itself from the grid's centre, so that no floor lies on cell centres;
  - its zone is the cells within 12 cells of the axis;
  - the box is the grid less 16 cells at each edge; the counted cells are the box's cells whose reference is above 0.
- the carve, at along-axis coordinate `s` and floor distance `d` (m):
  - depth `D(s) = 80 + 0.002 s`, so the floor falls downstream at 0.002 (the wall top stays within 11 cells of the floor everywhere on the grid);
  - V: `reach = d`; round: `reach = sqrt(d^2 + L^2) - L`, L one cell;
  - `carve = min(0.12 reach, D(s)) - D(s)`; the surface is the reference plus the carve.
- the floors:
  - smooth: `d` to the true axis;
  - stair4 / stair8: `d` to the nearest of the axis's 4- / 8-connected raster cells (the cell under the axis at quarter-cell steps; for stair4 a diagonal step is split by the axis step whose cell lies nearer the axis);
  - sawtooth: the smooth floor plus `5 m x (x mod 8) / 8` (the cell's x index) on the cells within 1.5 cells of the axis;
  - a defect case is a defect floor at an angle where it does not repeat with the lattice step along the axis: stair4 and stair8 off multiples of 45 degrees (along a lattice direction a raster is a straight floor of another cross-section), sawtooth at every angle.
- the backgrounds:
  - none: a flat 200 m;
  - fBm: 200 m plus integer-hashed value noise with a quintic fade, lattice spacings 37, 19, 11 and 7 cells, amplitudes 20, 10, 5 and 2.5 m, seed 155;
  - S0: `crates/cache/tests/support/s0-r1h-river-256.f32`:
    - 256 x 256 little-endian f32 of r1h-def's S0 (R1h window `1,8056,8675,760,1379,14`, chain e3799ec5);
    - its window cell (3088, 3576), global L (65936, 8056) on face 1; both multiples of 8, so its 8-cell phases are local;
    - SHA-256 `be1687ade107ebea9a52f9b874bf1bdf401470431617c69a14111264802ba09e`; the test pins its CRC32C;
    - derived project data: the generator chain's own start surface, no external dataset.
- the channel path (for path metrics), walked on the carve (the surface minus the reference, a router on a well-formed valley):
  - it starts at the counted zone cell within 0.75 cells of the axis with the least `s`;
  - each step goes to the counted zone 8-neighbour with a larger `s` and the steepest descent (drop over step length, possibly negative; ties by cell index), until it reaches the box's edge downstream of the centre (`s > 0`) or has no such neighbour.
- determinism: integer hashing; `libm` for `sin` and `cos`; the same case gives the same bits; the test pins the CRC32Cs of the fBm background's bits and of in-zone Xdiag's readings on it.
- the table: one row per background, profile and floor; one column per angle; each value marked `in` or `out` of the band, a defect case starred; the verdict names the first smooth case out of the band or the first defect case inside it.
- a NaN reading is outside every band, and it is not a detection: a NaN smooth reading fails "angle-invariant", a NaN defect reading fails "detects its defect".
- a metric without a band is `report only`: its table is written, and it does not pass.
- the harness's own checks:
  - a control metric that reads the floor class passes;
  - a constant metric fails "detects its defect";
  - a metric that reads the angle fails "angle-invariant".

#### Scenario: A metric's calibration
- **WHEN** a metric with a declared band runs on the case grid
- **THEN** it passes only if every smooth case is in the band and every defect case is out of it, and its table names each case's value and the first case that breaks the rule
