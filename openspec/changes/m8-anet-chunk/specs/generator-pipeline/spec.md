## ADDED Requirements

### Requirement: GEN-047 The global network in the chunk raster
A chunk stage of the A-net kind SHALL take the river network of a chunk from the stored global coupled run: the coarse path from every coarse cell whose drainage area exceeds 100 km2 to its receiver is the chunk's corridor, displaced by a noise meander that is a function of position only and zero at the coarse cell centres, so that two neighbouring chunks find the same path through their shared border; the drainage area crossing the halo ring is injected as inflow, and the initial fine heights are the coarse heights upsampled plus the base field's detail. The corridor SHALL be held at the run's height (interpolated along the segment, the lower of it and the base height) at every quarter of a coarse segment, so a channel bed does not depend on the heights at the region's ring. A chunk with no such cell SHALL have no corridor.

Verify: A
Status: planned
Source: ADR 0077 points 1 and 3, SPIKE-33 (two-scale scheme), SPIKE-35 item 1, GEN-040, STRM-043

#### Scenario: One path for two chunks
- **WHEN** two neighbouring chunks are given the same stored run
- **THEN** their corridor polylines agree at the shared border to the bit, each starts and ends at a coarse cell centre, and the inflow at each ring equals the coarse area that crosses it

### Requirement: GEN-048 The breach rule leaves no uphill channel
After the incision no channel cell (drainage area over 50,000 m2) SHALL have a receiver higher than itself by more than 1 micrometre. The rule is: channel cells take no diffusion and no slumping, a cell whose receiver is a higher ring cell is an outlet, and inside the corridor a cell drains to the lowest forward neighbour.

Verify: A
Status: planned
Source: SPIKE-35 item 1 (variant c4), ADR 0077 consequences

#### Scenario: Zero uphill
- **WHEN** a chunk of a terrain whose corridor runs uphill in places runs N = 30 steps
- **THEN** 0 channel cells have a higher receiver, and two runs give the same receivers and heights, bit for bit

### Requirement: GEN-049 The halo of an A-net chunk
An A-net chunk SHALL state its halo as 3 km (at least its step cap times its reach per step) and SHALL, with that halo, equal the same run over the whole region in every cell of its core, bit for bit, at the step cap, at a site where a river crosses the chunk border.

Verify: B
Status: planned
Source: SPIKE-35 item 2, ADR 0077 point 4, GEN-038

#### Scenario: Halo
- **WHEN** two neighbouring chunks run with halos of 1.5, 3 and 5 km against the whole-region run
- **THEN** the cores differ in 0 cells at 3 and 5 km, and the test prints the number of differing cells and the largest difference in metres at 1.5 km

### Requirement: GEN-050 Jittered receivers
The receiver of a cell SHALL be the steepest of its eight neighbours after scaling the slope of direction `k` by `1 + j (2 h - 1)`, `h` in `[0, 1)` a hash of the cell and `k`, with `j` 0.4: a function of position only, the same as the global run's jitter, so every chunk makes the same choice. The share of axis directions among the receivers of cells over 2e4 m2 SHALL be at most 0.56.

Verify: A
Status: planned
Source: SPIKE-35 item 3, GEN-040

#### Scenario: Axis share
- **WHEN** the receivers of a 4 m lattice of 49 km2 are chosen with j 0.4 and with j 0
- **THEN** the axis share with jitter is at most 0.56 and below the share without, and two runs give the same receivers

### Requirement: GEN-051 Diffusion and slumping
After the incision a chunk SHALL apply hillslope diffusion and thermal slumping to its non-channel cells in a fixed order with `libm` only: diffusion within its stability bound, slumping moving the excess over the angle of repose to the lowest neighbour for a fixed number of passes. Channel cells SHALL be unchanged by both, and the result SHALL be bit-identical for 1, 2 and 8 threads.

Verify: A
Status: planned
Source: SPIKE-33 stage model, ADR 0077 point 3, CON-14

#### Scenario: Hillslopes
- **WHEN** a chunk with steep slopes and a channel runs the processes
- **THEN** the channel cells are unchanged, no non-channel cell exceeds the angle of repose by more than one pass can move, the volume moved equals the volume received, and the hash is the same on 1, 2 and 8 threads

### Requirement: GEN-052 A-net chunk time
An A-net chunk SHALL take at most 0.1 s per km2 of core at 4 m cells on the reference machine, one thread, measured by the performance harness through the architect and never in a test (CON-20); the table of time per km2 at 4, 8 and 10 m is recorded with the run.

Verify: C
Status: planned
Source: SPIKE-33 (0.06 s per km2 at 4 m), M8 plan risks, CON-22

#### Scenario: Chunk time
- **WHEN** a chunk of 25 km2 runs on the reference machine in a perf window
- **THEN** it finishes within 2.5 s at 4 m cells and the log holds the time per km2 at 4, 8 and 10 m
