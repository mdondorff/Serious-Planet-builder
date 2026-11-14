## ADDED Requirements

### Requirement: TEST-081 Criterion 1 on the baked field
The scorecard run (TEST-079) over a store with the erosion layer (STRM-065) and the relief classes (GEN-093) SHALL check M9 criterion 1. At Valley and Ridge, the Black Forest, the Scottish Highlands and the Alps, the generator's slope p50 over cells at or below the heightfield's slope ceiling (45°, on both sides) and its 1 km relief p50 SHALL be at least 0.6 of the real tile's, at both matched sites. The sites SHALL be matched to the baked field by class and mean height (SPIKE-80's method), and the sites file SHALL record them. The report SHALL name each landform and site with its two ratios and PASS or FAIL. Without the layer the check SHALL be NOT RUN, never a pass. The bound 0.6 is a start value; R7 tunes towards 0.8.

Verify: A, B
Status: planned
Source: M9 draft (`docs/archive/M9/briefings/m9-draft.md`) decision 6, criterion 1's bound; ADR 0088, ADR 0089, ADR 0088/0089 accepted 2026-11-01; SPIKE-80, SPIKE-81 (the stand-in: the hill landforms pass with the class hill band, the Alps at 0.71-0.72); the 45° ceiling: ADR 0091, SPIKE-85

#### Scenario: Pass and fail named (A)
- **WHEN** the check reads a site whose slope ratio is 0.65 and 1 km relief ratio 0.55
- **THEN** that site is reported FAIL with both ratios, and the landform fails criterion 1

#### Scenario: No layer (B)
- **WHEN** the run is pointed at `global16384` without an erosion layer
- **THEN** criterion 1 is reported NOT RUN, naming the missing layer

### Requirement: TEST-082 The default classes and the base elevation match Earth's shares
A tier-B check SHALL evaluate the default source (GEN-094 to GEN-097) of seeds 1 to 5 and report each measure per seed, with the median, against its band. Unless a row says otherwise, it samples at 0.25 degree over land under 60 degrees latitude, area weighted. Earth's values are from ETOPO 2022 (SPIKE-93). The class bands (plains / hills / mountains 45-62 / 15-30 / 20-32 %) are approved (owner, 2026-11-02); the other bands are the planner's, from SPIKE-93 (2026-11-02). They replace the single-seed bands of SPIKE-80.
| Measure | Earth | Band |
|---|---|---|
| Land under 500 m | 58.4 % | 50-65 % |
| Median land elevation | about 410 m | 300-550 m |
| Median elevation of land within about 55 km of the sea | 82 m | under 200 m |
| Land above 1 km / 2 km / 3 km / 4 km | 21.6 / 6.5 / 3.5 / 2.2 % | 17-27 / 5-10 / 2-5 / 1-3 % |
| Highest land | 8.16 km | 7-9 km |
| Relief classes, plains / hills / mountains | 45.7 / 24.6 / 29.7 % (R7 at 1') | 45-62 / 15-30 / 20-32 % |
| Belt land (range of +-50 km at least 1,000 m), 0.25 degree | 17.6 % | 13-23 %; reported, gated with the convergent-only arc round (seed 4 holds 5.4 %, the median 9.7 %) |
| p99 of the step of `k` between 40 km cells | 3.3 | reported, at most 3.5 (realism; SPIKE-93) |
| p99 of the lattice steps of the untapered `k` (the largest step is reported) | p99 3.3 (largest 5.4) | at most 2.0 (GEN-097; gated on every seed) |
- **Gate form:** the median of the five seeds SHALL lie within the band widened by 10 % of its width, and no seed SHALL lie outside the band by more than one band width. Earth is one sample and some worlds are more mountainous than others: the seed spread is a feature and the median carries the realism.
- A point's relief class comes from the class weights; its elevation is the base elevation of GEN-096 before any bake or erosion.
- Until the bake is wired the check reads the base elevation of the default source (the preview, before any bake or erosion) and its result is not the baked field's; with the bake it SHALL read the seed's global bake, and without that bake it SHALL be NOT RUN, never a pass.
- The preview report prints each measure beside its band (TEST-086).

Verify: B
Status: active
Source: ADR 0089 decisions 3, 5 and 7, ADR 0088/0089 accepted 2026-11-01; SPIKE-69, SPIKE-78, SPIKE-79, SPIKE-80; SPIKE-93 (Earth's bands from ETOPO), planner (2026-11-02, inbox #69 and later); the class bands (plains / hills / mountains 45-62 / 15-30 / 20-32 %) approved by the owner, 2026-11-02

#### Scenario: Seeds 1 to 5
- **WHEN** the check runs on seeds 1 to 5
- **THEN** every gated row passes the gate form, and each seed's value is printed with the median and the band

### Requirement: TEST-083 Steep classes plan without a capped tile
A tier-B check SHALL plan at production parameters (`LodParams::earth_1080p`) over a store with the erosion layer and its pyramid (STRM-066). It SHALL plan at one high-mountain, one mountain and one hill pose of the scorecard sites, each at 200 m and 2 km above the ground. At every pose:
- no tile SHALL be capped (ADR 0074);
- the deepest level SHALL equal today's field's (the layer switch off) at the same pose and altitude;
- the node count SHALL be at most 1.1 times today's.
The report SHALL give, per pose, the nodes, the capped tiles and the deepest level against today's. The poses' altitudes and the 1.1 bound are start values. Without the layer and its pyramid the check SHALL be NOT RUN, never a pass.

Verify: B
Status: planned
Source: ADR 0088 point 8, ADR 0089, ADR 0088/0089 accepted 2026-11-01; SPIKE-82 (level 13-15 and 28-85 capped tiles without the pyramid), SPIKE-83 (no capped tile, 5-8 % more nodes with it); M9 criterion 6 (no degradation)

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
Source: ADR 0088 point 4, ADR 0088/0089 accepted 2026-11-01; SPIKE-68 (a per-tile carve leaves p99 134-172 m), SPIKE-70, SPIKE-72 (the two passes are seam-free at four of four sites once the local carve only incises)

#### Scenario: Four sites
- **WHEN** the check runs at the four bake sites
- **THEN** at each the core-edge p99 is at most the off-edge p99, and the face carve's cube-edge step is reported
