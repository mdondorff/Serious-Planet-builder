## ADDED Requirements

### Requirement: GEN-040 The global coupled run
`generators::coupled` SHALL run stream-power landscape evolution on the cube-sphere bake lattice (ADR 0077 point 1): per step a priority flood fills the depressions, each land cell takes the steepest receiver down the filled surface with a jitter `1 + j (2 h - 1)` (`h` a hash of seed, cell and direction), the drainage area is accumulated downstream, and the heights take the implicit update `z = (z + dt U + f z_r) / (1 + f)`. The run SHALL be single-threaded in a fixed order with `libm`, so its results are bit-identical across runs and platforms (CON-14); every receiver is lower on the filled surface; the outlets drain the land area.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1), ADR 0077, SPIKE-33, SPIKE-35

#### Scenario: Network, area and determinism
- **WHEN** a 12-cell lattice with a 60 % land world runs 30 steps twice, with jitter 0 and 0.8
- **THEN** no receiver drains uphill and no chain cycles, the outlets' area equals the land area, both runs agree bit for bit, the mean change per step falls, and the jitter changes the receivers
