# generator-pipeline Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: GEN-001 Pure generators
A generator SHALL be a pure function of its recorded definition reads, `generator_version`, `implementation_id` and tile ID, with no global state, wall-clock time or unseeded randomness.

Verify: A
Status: active
Source: CON-06, ADR 0006

#### Scenario: Repeat
- **WHEN** the same tile is generated twice in one process and in two processes
- **THEN** the outputs are bit-identical

### Requirement: GEN-002 Thread-count independence
Generator output SHALL not depend on the number of threads, and reductions SHALL use a fixed order.

Verify: A
Status: active
Source: CON-14

#### Scenario: Thread counts
- **WHEN** a region is generated with 1, 2 and 8 threads
- **THEN** all outputs are bit-identical

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Different builds
- **WHEN** the manifests carry different version lines
- **THEN** the report lists the version lines first and says the manifests come from different builds

### Requirement: GEN-004 Border agreement
Neighbouring tiles SHALL agree on every shared border sample, including across cube-face edges and at the eight corners.

Verify: A
Status: active
Source: CON-08, report §16

#### Scenario: Face edge
- **WHEN** two tiles on adjacent faces are generated
- **THEN** their shared border samples are identical

### Requirement: GEN-005 Partition invariance
Generating a region as one large tile or as many small tiles SHALL give the same values at every common sample.

Verify: A
Status: active
Source: CON-08, report §16

#### Scenario: Split
- **WHEN** a tile and its four children are generated
- **THEN** samples at common positions are equal

### Requirement: GEN-006 Coarse and fine agreement
A coarse tile SHALL agree with its four children within a stated tolerance at the coarse sample positions.

Verify: A
Status: active
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
Within one cache each field layer SHALL be produced by exactly one implementation, and neighbouring tiles SHALL share one `implementation_id`. The CPU implementation SHALL be the canonical reference, and a GPU implementation SHALL agree with it per field within `|Δ| ≤ max(rel·|v|, abs)`: heights rel 1e-6, abs 1 mm; normals 0.01°; masks abs 1e-4.

Verify: B
Status: planned
Source: CON-07, CON-14, ADR 0010

#### Scenario: Differential
- **WHEN** a GPU implementation is dispatched on test inputs
- **THEN** its read-back output matches the CPU output within the field's bound, and a failure reports the field, the tile, the sample and both values

#### Scenario: No mixed seams
- **WHEN** a cache holds tiles of one layer
- **THEN** every tile and its neighbours carry the same `implementation_id`

### Requirement: GEN-009 Field dumps
CPU code SHALL be able to write tile height maps (further field maps as fields appear) and the unfolded six-face cube net, whose cells are laid out so that shared edges are continuous, to PNG without any GPU.

Verify: A
Status: active
Source: report §16

#### Scenario: Face net
- **WHEN** the face-net dump is written
- **THEN** the PNG contains six distinct face colours and no background pixels inside the net

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs. Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to

### Requirement: GEN-012 Body identity in keys
Every height key and mesh key SHALL cover the identity and parameters of the body it belongs to (ADR 0001, 0007): the body id, the id of its reference surface and, for a mesh, the body parameter hash (id, seed, radius and reference surface). Two bodies SHALL never share a key for the same tile, even with equal seeds, inputs and radii, a field built directly from a seed SHALL stand for the single planet (`BodyId(0)`), and a key composition that gains an input SHALL bump `KEY_FORMAT_VERSION` and re-record the known answers, which the determinism manifest compares between operating systems.

Verify: A
Status: active
Source: ADR 0001, ADR 0006, ADR 0007, M3 queue item 4

#### Scenario: Body alone changes the key
- **WHEN** two fields differ only in the body they describe
- **THEN** their height keys and mesh keys differ, and the planet through the default equals the field built directly from the seed

#### Scenario: Body parameters
- **WHEN** the body id, seed, radius or reference surface changes
- **THEN** the body parameter hash, and with it the mesh key, changes

### Requirement: GEN-011 Height bounds
A height field SHALL report sound bounds `(min, max)` of its height over every spherical cap (centre direction, angular radius), never excluding a height the field takes inside the cap, and the bounds of small caps SHALL follow the local relief closely enough for node volumes to hug the terrain; the default is the field's global range.

Verify: A
Status: active
Source: ADR 0016, LOD-008

#### Scenario: Soundness
- **WHEN** random points of random caps from sub-metre to several degrees are evaluated on the reference and the steep field
- **THEN** every height lies inside its cap's bounds

#### Scenario: Small caps
- **WHEN** the bounds of a cap of about 130 m are taken
- **THEN** they are tens of metres wide on the reference field, not the global range
