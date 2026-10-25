## ADDED Requirements

### Requirement: GEN-047 The global network in the chunk raster
A chunk stage of the A-net kind SHALL take the river network of a chunk from the stored global coupled run: the path from every coarse cell whose drainage area exceeds `river_area_m2` (GEN-068) to its receiver, from the node of the one to the node of the other (GEN-066; the cell centres when routing is off, with a noise meander that is a function of position only and zero at the centres), is the chunk's corridor, so that two neighbouring chunks find the same path through their shared border; the drainage area crossing the halo ring is injected as inflow, and the initial fine heights are the coarse heights upsampled plus the base field's detail. The corridor's bed is set by GEN-059, so a channel bed does not depend on the heights at the region's ring. A chunk with no such cell SHALL have no corridor.

Verify: A
Status: active
Source: ADR 0077 points 1 and 3, SPIKE-33 (two-scale scheme), SPIKE-35 item 1, GEN-040, STRM-043

#### Scenario: One path for two chunks
- **WHEN** two neighbouring chunks are given the same stored run
- **THEN** their corridor polylines agree at the shared border to the bit, each starts and ends at a coarse cell centre (with no slide and no routing, else at the node of GEN-066), and the inflow at each ring equals the coarse area that crosses it

### Requirement: GEN-048 The breach rule leaves no uphill channel
After the incision no channel cell (drainage area over 50,000 m2) SHALL have a receiver higher than itself by more than 1 micrometre, except a link between two fixed corridor cells, which may climb by at most 5 cm (GEN-059). The rule is: channel cells take no diffusion and no slumping, a cell whose receiver is a higher ring cell is an outlet, and along the corridor's links a cell drains to the next cell of the path.

Verify: A
Status: active
Source: SPIKE-35 item 1 (variant c4), ADR 0077 consequences

#### Scenario: Zero uphill
- **WHEN** a chunk of a terrain whose corridor runs uphill in places runs N = 30 steps
- **THEN** 0 channel cells have a higher receiver, and two runs give the same receivers and heights, bit for bit

### Requirement: GEN-049 The halo of an A-net chunk
An A-net chunk SHALL state its halo (3 km by default) and SHALL, with it, equal the same run over a wider halo in every cell of its core: bit for bit where every channel chain meets a fixed cell (an anchor) inside the halo and every catchment lies within the area window, and to within 2 cm on a gentle valley where a stream runs a long way to the river. The reaches that bound this (breach window, area window, tributary step cap) are parameters of the stage and part of its definition.

Verify: A, B
Status: active
Source: SPIKE-35 item 2, ADR 0077 point 4, GEN-038

#### Scenario: Halo
- **WHEN** two neighbouring chunks run with halos of 1.5, 3 and 5 km against the whole-region run
- **THEN** the cores differ in 0 cells at 3 and 5 km, and the test prints the number of differing cells and the largest difference in metres at 1.5 km

### Requirement: GEN-050 Jittered receivers
The receiver of a cell SHALL be the steepest of its eight neighbours after scaling the slope of direction `k` by `1 + j (2 h - 1)`, `h` in `[0, 1)` a hash of the cell and `k`, with `j` 0.4: a function of position only, the same as the global run's jitter, so every chunk makes the same choice. The share of axis directions among the receivers of cells over 2e4 m2 SHALL be at most 0.56.

Verify: A
Status: active
Source: SPIKE-35 item 3, GEN-040

#### Scenario: Axis share
- **WHEN** the receivers of a 4 m lattice of 49 km2 are chosen with j 0.4 and with j 0
- **THEN** the axis share with jitter is at most 0.56 and below the share without, and two runs give the same receivers

### Requirement: GEN-051 Diffusion and slumping
After the incision a chunk SHALL apply hillslope diffusion and thermal slumping to its non-channel cells in a fixed order with `libm` only: diffusion within its stability bound, slumping moving the excess over the angle of repose to the lower neighbours for a fixed number of passes. Channel cells SHALL be unchanged by both, and the result SHALL be bit-identical for 1, 2 and 8 threads.

Verify: A
Status: active
Source: SPIKE-33 stage model, ADR 0077 point 3, CON-14

#### Scenario: Hillslopes
- **WHEN** a chunk with steep slopes and a channel runs the processes
- **THEN** the channel cells are unchanged, no non-channel cell exceeds the angle of repose by more than one pass can move, the volume moved equals the volume received, and the hash is the same on 1, 2 and 8 threads

### Requirement: GEN-052 A-net chunk time
An A-net chunk of 24 km2 of core SHALL take at most 4 s at 8 m cells with the 3 km halo (1.8 million cells) on the reference machine, one thread, measured by the performance harness through the architect and never in a test (CON-20); the time at 4 and 10 m is recorded with the run (4 m was about 14 s per chunk on a synthetic scene: not met, ADR 0077 allows 4 to 10 m).

Verify: C
Status: planned
Source: SPIKE-33, M8 plan risks, CON-22, planner decision 2026-10-24 (8 m default)

#### Scenario: Chunk time
- **WHEN** a chunk of 24 km2 at 8 m cells runs on the reference machine in a perf window
- **THEN** it finishes within 4 s at 8 m cells and the log holds the times at 4 and 10 m

### Requirement: GEN-057 Corridor anchors follow the drawn ground
The corridor of an A-net chunk SHALL be anchored at every polyline point at the base field's height there minus the valley depth `depth_m (A / area_ref_m2)^exponent` for the river's area `A` at that point, at most `lerp(cap_low_m, cap_m, rf)` with `rf` the relief factor (GEN-060; defaults: 3 m at 100 km2, exponent 0.5, `cap_m` 100 m, `cap_low_m` 30 m, so the cap is at most a third of `bound_m`, 300 m), and GEN-059 sets the bed from these raw anchors. The coupled run's absolute height SHALL not set an anchor. The law's constants are part of the stage's definition hash.

Verify: A
Status: active
Source: owner decision 2026-10-25 (terrain relief into M8, part 1), GEN-047, learnings 2026-10-25

#### Scenario: High ground over a low run
- **WHEN** a chunk of a base 2,600 m above the coupled run runs its incision
- **THEN** every anchor lies at most `cap_m` under the base, no core cell is cut by half the bound or more, and the depth of the law rises with the drainage area up to the cap

### Requirement: GEN-058 The banks settle to the angle of repose
After the incision steps an A-net chunk SHALL lower every free cell within `bank_rounds` (`ceil(bound_m / (talus cell_m)) + 6`, 48 at 8 m) rings of the core to at most `z(s) + talus x distance` of every protected cell `s` that is not in the outer ring (a channel, the corridor, an anchor), but not under a protected neighbour by more than `talus x distance`, and SHALL not move any protected cell. A cell SHALL only be lowered, and the core SHALL be the same for every halo of at least the stated one.

Verify: A
Status: active
Source: planner decision 2026-10-25, GEN-051, learnings 2026-10-25 (walls), ADR 0074

#### Scenario: A breach cut through a ridge
- **WHEN** a chunk of a river that crosses a 600 m ridge runs with and without the settle
- **THEN** without it more than ten bank pairs stand over the repose slope, with it at most one in twenty, no protected cell differs, and no cell is higher with it than without

### Requirement: GEN-059 The bed of a river follows the ground and falls the whole way
Every corridor cell of an A-net chunk SHALL be held at a bed interpolated along its segment from samples at every polyline point: the base height minus the depth law (GEN-057), the running minimum from upstream along the river's main stem (the donor of the largest area at every cell) over the previous `window` (16) segments, each sample at least `min(slope_ks A^-slope_theta, slope_cap)` per metre under the sample above it and at most `cut_cap_m` (200 m) under the ground; a tributary's bed SHALL end no lower than the main stem's bed that arrives at the confluence. No corridor bed SHALL lie over the ground, no link SHALL climb by more than 5 cm and no linked corridor cell SHALL be left without a receiver, wherever the ground does not rise along the river by more than the cut cap. The law's constants are part of the stage's definition hash.

Verify: A
Status: active
Source: SPIKE-40, SPIKE-42, ADR 0081, ADR 0082 (proposed), GEN-057

#### Scenario: A ridge across the river
- **WHEN** a chunk of a river over a base with a ridge across its path runs
- **THEN** every corridor bed is at most the ground, no link climbs by more than 5 cm, and the cut is at most the cut cap plus the valley depth

### Requirement: GEN-060 The channel from discharge, the valley from relief
Round every corridor cell an A-net chunk SHALL hold the cells within `band_cap_m` of the river at a channel and valley profile: a channel of width `w_coef Q^w_exp` and depth `d_coef Q^d_exp` under the valley floor, discharge `Q = q_per_km2 A` (km2), the valley floor at the depth law of GEN-057 (capped at `lerp(cap_low_m, cap_m, rf)`); a valley floor `lerp(floor_low, floor_high, rf)` channel widths wide at the bed of the corridor, no wider than `band_cap_m` allows (the cap follows the halo: `(halo_rings - area_window - 2)` cells at most, and 0 paints no band); the corridor cells at the channel floor, the lowest point of the cross-section; sides rising at `lerp(side_low, side_high, rf)` to the ground; `rf` the base's relief over the river's coarse cell and the eight round it over `relief_full_m`. The constants are part of the stage's definition hash.

Verify: A
Status: active
Source: SPIKE-40, SPIKE-41, planner decision 2026-10-25 (H2), GEN-057, GEN-058

#### Scenario: A great lowland river
- **WHEN** a chunk of a river of 9.5e6 km2 over smooth ground runs its incision
- **THEN** the channel is within a factor 2 of the law's width at half depth and its depth within 1.5 of the law's, in a valley floor wider than half the channel
