# renderer Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: REND-001 FramePlan executor
The renderer SHALL execute a FramePlan and SHALL make no decisions about which nodes, levels, batches, tiles, rectangles or colours to draw; the number of draw calls SHALL equal the number of draws in the plan, and a plan that cannot be executed (a resource the selected view needs is missing, rectangle outside the frame, unusable height range or resolution) SHALL be reported, not guessed.

Verify: C
Status: active
Source: CON-03, ADR 0009

#### Scenario: Plan in, pixels out
- **WHEN** a FramePlan listing two tiles is executed
- **THEN** exactly those two rectangles are drawn, the draw-call count is 2, and the GPU frame equals the CPU twin of the executor (exactly for flat views, within 1 grey level for the height view)

#### Scenario: Unexecutable plan
- **WHEN** the plan names a tile whose resource is missing
- **THEN** execution fails with the tile id

### Requirement: REND-002 Shader validation
Every shipped shader module SHALL pass backend validation on the software adapter in CI.

Verify: B
Status: active
Source: ADR 0004, CON-17

#### Scenario: Invalid shader
- **WHEN** a shader module fails validation
- **THEN** the test fails and prints the shader name and the validation message

### Requirement: REND-003 Reversed-Z depth
Rendering SHALL use reversed-Z with a 32-bit float depth buffer, compare function greater, clear value 0 and an infinite far plane.

Verify: C
Status: planned
Source: CON-12, ADR 0002

#### Scenario: Depth view
- **WHEN** the depth debug view of a near and a far object is rendered
- **THEN** the near object has the larger depth value and neither is clipped

### Requirement: REND-004 Precision at the surface
At 1 m camera distance anywhere on the planet, including 6,371 km from the origin, rendering jitter SHALL be below 0.25 px.

Verify: C
Status: planned
Source: CON-13, ADR 0001

#### Scenario: Sub-pixel motion
- **WHEN** the camera is moved in 1 mm steps at 1 m distance on the far side of the planet
- **THEN** projected positions of a marker move monotonically within 0.25 px of the exact value

### Requirement: REND-005 No planet-scale shader values
Shaders SHALL receive only tile-local or camera-relative quantities and SHALL NOT compute values of planet-radius magnitude.

Verify: A
Status: planned
Source: CON-11

#### Scenario: Review rule
- **WHEN** a shader contains a planet-radius uniform or constant
- **THEN** the shader scan test fails and names file and line

### Requirement: REND-006 Deterministic debug views
Debug views SHALL render flat, lighting-independent colours (or a grey ramp of data) that compare exactly against goldens of the same adapter. M1 provides face, tile-id and height; level, morph, normals, depth and streaming state are added with the changes that introduce those quantities.

Verify: C
Status: active
Source: CON-18, report §16

#### Scenario: Re-render
- **WHEN** a debug view is rendered twice on one adapter
- **THEN** the images are identical

### Requirement: REND-007 Adapter selection and logging
The headless harness SHALL select the software adapter for test runs (and the high-performance adapter when asked for performance runs), and SHALL log the chosen adapter's backend, name and type as its first output line; interactive adapter selection becomes a requirement when the editor gets a window (M2).

Verify: C
Status: active
Source: CON-21, report §8

#### Scenario: Test render log
- **WHEN** `planet test-render` runs on the software adapter
- **THEN** its output begins with an `adapter:` line naming backend, name and device type

### Requirement: REND-008 GPU culling differential
GPU-side culling, when added for instances, SHALL be covered by a differential test against a CPU reference on the software adapter.

Verify: B
Status: planned
Source: CON-03, ADR 0009

#### Scenario: Differential
- **WHEN** a GPU culling pass and the CPU reference run on the same instances and frustum
- **THEN** the visible sets are equal
