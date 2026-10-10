# generator-pipeline Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: GEN-001 Pure generators
A generator SHALL be a pure function of its recorded definition reads, `generator_version`, `implementation_id` and tile ID, with no global state, wall-clock time or unseeded randomness.

Verify: A
Status: planned
Source: CON-06, ADR 0006

#### Scenario: Repeat
- **WHEN** the same tile is generated twice in one process and in two processes
- **THEN** the outputs are bit-identical

### Requirement: GEN-002 Thread-count independence
Generator output SHALL not depend on the number of threads, and reductions SHALL use a fixed order.

Verify: A
Status: planned
Source: CON-14

#### Scenario: Thread counts
- **WHEN** a region is generated with 1, 2 and 8 threads
- **THEN** all outputs are bit-identical

### Requirement: GEN-003 Cross-OS tile hashes
Tile hashes of a committed set of tiles SHALL be identical on Windows and Linux.

Verify: A
Status: planned
Source: CON-14, ADR 0010

#### Scenario: CI comparison
- **WHEN** the hash job runs on both operating systems
- **THEN** the hash lists are equal, and a difference names the first differing tile

### Requirement: GEN-004 Border agreement
Neighbouring tiles SHALL agree on every shared border sample, including across cube-face edges and at the eight corners.

Verify: A
Status: planned
Source: CON-08, report §16

#### Scenario: Face edge
- **WHEN** two tiles on adjacent faces are generated
- **THEN** their shared border samples are identical

### Requirement: GEN-005 Partition invariance
Generating a region as one large tile or as many small tiles SHALL give the same values at every common sample.

Verify: A
Status: planned
Source: CON-08, report §16

#### Scenario: Split
- **WHEN** a tile and its four children are generated
- **THEN** samples at common positions are equal

### Requirement: GEN-006 Coarse and fine agreement
A coarse tile SHALL agree with its four children within a stated tolerance at the coarse sample positions.

Verify: A
Status: planned
Source: CON-08, report §16

#### Scenario: Tolerance
- **WHEN** a parent and its children are compared
- **THEN** the maximum difference is within the tolerance and is reported when exceeded

### Requirement: GEN-007 Staged generation
Generation SHALL be staged global-coarse, regional (per tile with a halo), local (per point), and a stage SHALL read only outputs of earlier stages.

Verify: A
Status: planned
Source: CON-08

#### Scenario: Illegal read
- **WHEN** a stage requests a later stage's output
- **THEN** the request is rejected with the stage names

### Requirement: GEN-008 One implementation per layer
Within one cache each field layer SHALL be produced by exactly one implementation, the CPU implementation SHALL be the canonical reference, and a GPU implementation SHALL agree with it within 1e-4 relative error.

Verify: B
Status: planned
Source: CON-07, CON-14

#### Scenario: Differential
- **WHEN** a GPU implementation is dispatched on test inputs
- **THEN** its read-back output matches the CPU output within 1e-4 relative error

### Requirement: GEN-009 Field dumps
CPU code SHALL be able to write height maps, field maps and the unfolded six-face cube net to PNG without any GPU.

Verify: A
Status: planned
Source: report §16

#### Scenario: Face net
- **WHEN** the face-net dump is written
- **THEN** the PNG contains six distinct face colours and no background pixels inside the net
