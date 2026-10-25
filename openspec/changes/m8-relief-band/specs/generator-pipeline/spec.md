## ADDED Requirements

### Requirement: GEN-064 The hill band and the landform driver of the bake
`global::sample` SHALL add to the elevation of land a hill band: a ridged multifractal of three octaves at the frequencies 796, 1592 and 3184 per unit direction (wavelengths of 8, 4 and 2 km at Earth's radius; they scale with the body's radius) on the warped direction, weights 1, 1/2 and 1/4 of `(1 - |n|)^2`, normalised, times an amplitude and times `smoothstep(0, 0.1, land depth)`. The band SHALL be 0 over the sea and at the coast and never negative, and at most `HILL_BAND_MAX_M` (1000 m). The amplitude SHALL be `30 + 145 smoothstep(0, 0.25, m) + 825 smoothstep(0.25, 0.7, m)` m for the mountain strength `m = landform x (0.25 + 0.75 ridge)`. `global::landform(seed, d)` SHALL be the range-zone strength times the distance from the coast, in `[0, 1]`, a pure function of the seed and the direction that does not read the band. The bake's elevation clamp and the field's `max_abs_height_m` SHALL stay as they are while the highest base and range (6200 m) plus `HILL_BAND_MAX_M` stay below the clamp. `GLOBAL_VERSION` SHALL be 5.

Verify: A
Status: active
Source: owner (2026-10-25, via opus-planner): plausible relief in M8; ADR 0080 points 1, 3 and 5, `openspec/changes/m8-relief/proposal.md`

#### Scenario: Where the band is and how large
- **WHEN** 40,000 directions spread over the sphere are sampled
- **THEN** the band is between 0 and 1000 m, 0 over the sea, positive over a quarter of the land at least, the elevation never exceeds the clamp, and the highest base plus range plus the band is below it

#### Scenario: By landform
- **WHEN** land with landform under 0.02 and land with landform over 0.5 are compared
- **THEN** the plains carry at most 40 m and the mean band of the ranges is at least eight times the plains'

#### Scenario: No short wavelengths
- **WHEN** the band is stepped 100 m along 300 great circles
- **THEN** no step moves it by more than the gradient bound of its three octaves, 0.23 of the largest amplitude, which a wavelength under 2 km would exceed

#### Scenario: Known answers
- **WHEN** the landform driver is read at four fixed directions
- **THEN** its bits are the pinned ones (GLOBAL_VERSION 5)
