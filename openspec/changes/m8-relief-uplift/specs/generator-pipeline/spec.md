## ADDED Requirements

### Requirement: GEN-059 The stationary uplift of the coupled run
`coupled::stationary_uplift` SHALL give each land cell the uplift `rho K A^m S` in m/year, `A` the drainage area and `S = (z - max(z_r, 0)) / distance` the slope to the receiver, both taken from the input heights `z` by the run's priority flood, jittered receivers and accumulation, so that the implicit update `z = (z + dt U + f z_r) / (1 + f)` leaves the input where it is at `rho` 1 (to a few f32 ulps; not the cells in a depression, which the flood raises, nor land cells under `min_height_m`, which the step lifts to it, nor the cells that drain to either, which read the moved height). Sea cells, outlets and cells whose drop to the receiver is not positive SHALL get 0, the result SHALL be linear in `rho` (a factor of zero or more) and a pure function of its arguments in the fixed order of the run (CON-14). The coupled run of a world SHALL take this field with `rho` `UPLIFT_RHO` (1) in place of a constant uplift; `rho` and `UPLIFT_FIELD_VERSION` SHALL be part of the declared identity of the run's inputs (STRM-043).

Verify: A
Status: active
Source: owner (2026-10-25, via opus-planner): plausible relief in M8; ADR 0080 point 4, `openspec/changes/m8-relief/proposal.md` items 5 and 6, GEN-040

#### Scenario: The input is kept
- **WHEN** a 24-cell world runs 20 steps on its stationary uplift at `rho` 1 and, from the same heights, on a constant uplift of 1e-3 m/year
- **THEN** the median height change over land is under a tenth of the constant run's and under 20 m, one step moves every land cell that is not in a depression, under the height floor or draining to such a cell by at most 4 f32 ulps (coastal cells included), the uplift is never negative, `rho` 0 gives zero everywhere and `rho` 2 gives exactly twice the field, and in a world without sea the outlets and the cells in pits get 0

#### Scenario: Sea and determinism
- **WHEN** the field is made twice on a 12-cell world
- **THEN** both agree bit for bit and every sea cell is 0
