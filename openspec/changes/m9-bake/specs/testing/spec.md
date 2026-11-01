## ADDED Requirements

### Requirement: TEST-081 Criterion 1 on the baked field
The scorecard run (TEST-079) over a store with the erosion layer (STRM-065) and the relief classes (GEN-093) SHALL check M9 criterion 1. At Valley and Ridge, the Black Forest, the Scottish Highlands and the Alps, the generator's slope p50 and 1 km relief p50 SHALL be at least 0.6 of the real tile's, at both matched sites. The sites SHALL be matched to the baked field by class and mean height (SPIKE-80's method), and the sites file SHALL record them. The report SHALL name each landform and site with its two ratios and PASS or FAIL. Without the layer the check SHALL be NOT RUN, never a pass. The bound 0.6 is a start value; R7 tunes towards 0.8.

Verify: A, B
Status: planned
Source: M9 draft (`docs/briefings/m9-draft.md`) decision 6, criterion 1's bound; ADR 0088, ADR 0089, pending the owner's acceptance of ADR 0088/0089; SPIKE-80, SPIKE-81 (the stand-in: the hill landforms pass with the class hill band, the Alps at 0.71-0.72)

#### Scenario: Pass and fail named (A)
- **WHEN** the check reads a site whose slope ratio is 0.65 and 1 km relief ratio 0.55
- **THEN** that site is reported FAIL with both ratios, and the landform fails criterion 1

#### Scenario: No layer (B)
- **WHEN** the run is pointed at `global16384` without an erosion layer
- **THEN** criterion 1 is reported NOT RUN, naming the missing layer

### Requirement: TEST-082 The default classes and the base elevation match Earth's shares
A tier-B check SHALL evaluate seed 1's default source (GEN-094 to GEN-097) and report each measure against its band. Seed 2 is reported beside it and not judged. Unless a row says otherwise, it samples at 0.25 degree over land under 60 degrees latitude.
| Measure | Band | Measured |
|---|---|---|
| Land with a belt core over 0.5 that lies in regions with length squared over area of 4 or more | at least 90 % | not measured on the core; SPIKE-79 measured 96 % of land above 2 km in such regions |
| Relief classes, plains / hills / mountains | 55-67 / 10-25 / 14-30 % | 61 / 20 / 19 % (SPIKE-80; Earth 61 / 14 / 25 %, SPIKE-69) |
| Land above 2 km / 4 km | 5-10 % / 1-3 % | 7.6 / 1.8 % (SPIKE-80; Earth without Antarctica 6.7 / 1.9 %, SPIKE-78) |
| Highest land | 7-9 km | 8.0 km (SPIKE-80) |
| Largest step of the untapered `k` between neighbouring lattice points, read at every lattice point of the planet | at most 2.0 | not measured |
- A point's relief class comes from `k R14`, where `R14` is the elevation range of today's field (the layer switch off) within 7 km of the point at `k` 1. The class is plain under 100 m, hills from 100 to 300 m, and mountains over 300 m or where `e0` (GEN-095) exceeds 2.5 km.
- The bands are start values around one measured seed.
- Without the seed's global bake the check SHALL be NOT RUN, never a pass.

Verify: B
Status: planned
Source: ADR 0089 decisions 3, 5 and 7, pending the owner's acceptance of ADR 0088/0089; SPIKE-69, SPIKE-78, SPIKE-79, SPIKE-80

#### Scenario: Seed 1 within its bands
- **WHEN** the check runs on seed 1
- **THEN** every measure lies in its band, and each is printed with its band and the measured value; seed 2's values are printed without a verdict

### Requirement: TEST-083 Steep classes plan without a capped tile
A tier-B check SHALL plan at production parameters (`LodParams::earth_1080p`) over a store with the erosion layer and its pyramid (STRM-066). It SHALL plan at one high-mountain, one mountain and one hill pose of the scorecard sites, each at 200 m and 2 km above the ground. At every pose:
- no tile SHALL be capped (ADR 0074);
- the deepest level SHALL equal today's field's (the layer switch off) at the same pose and altitude;
- the node count SHALL be at most 1.1 times today's.
The report SHALL give, per pose, the nodes, the capped tiles and the deepest level against today's. The poses' altitudes and the 1.1 bound are start values. Without the layer and its pyramid the check SHALL be NOT RUN, never a pass.

Verify: B
Status: planned
Source: ADR 0088 point 8, ADR 0089, pending the owner's acceptance of ADR 0088/0089; SPIKE-82 (level 13-15 and 28-85 capped tiles without the pyramid), SPIKE-83 (no capped tile, 5-8 % more nodes with it); M9 criterion 6 (no degradation)

#### Scenario: High-mountain pose
- **WHEN** the plan is made at the high-mountain pose at 200 m
- **THEN** it has no capped tile, its deepest level is today's, and its node count is at most 1.1 times today's

### Requirement: TEST-084 The erosion layer has no seam at tile or cube edges
A tier-B check SHALL bake, at each of the four bake sites of SPIKE-70, the face carve (GEN-100) of the face that holds the site and two neighbouring tiles of pass 2 (GEN-101).
- **Judged:** the p99 of the height step across the tiles' shared core edge SHALL be at most the p99 of the step between neighbouring cells one cell off the edge.
- **Reported, not judged:** the step across a cube edge of the face carve.
The report SHALL give both p99 and the maximum per site. The p99 comparison is a start value. Without the store the check SHALL be NOT RUN, never a pass.

Verify: B
Status: planned
Source: ADR 0088 point 4, pending the owner's acceptance of ADR 0088/0089; SPIKE-68 (a per-tile carve leaves p99 134-172 m), SPIKE-70, SPIKE-72 (the two passes are seam-free at four of four sites once the local carve only incises)

#### Scenario: Four sites
- **WHEN** the check runs at the four bake sites
- **THEN** at each the core-edge p99 is at most the off-edge p99, and the face carve's cube-edge step is reported
