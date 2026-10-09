## ADDED Requirements

### Requirement: REND-001 FramePlan executor
The renderer SHALL execute a FramePlan and SHALL make no decisions about which nodes, levels or batches to draw; GPU-side culling SHALL be limited to instances and SHALL be covered by a differential test against a CPU reference.

Verify: A
Status: planned
Source: CON-03, ADR 0009

#### Scenario: Plan in, pixels out
- **WHEN** a FramePlan listing one node is executed
- **THEN** exactly that node is drawn and the draw-call count equals the plan's count

### Requirement: REND-002 Shader validation
Every shipped shader module SHALL pass backend validation on the software adapter in CI.

Verify: B
Status: active
Source: report §16

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
Debug views (tile ID, face, level, morph, normals, depth, streaming state) SHALL render flat, lighting-independent colours that compare exactly against goldens of the same adapter.

Verify: C
Status: planned
Source: CON-18, report §16

#### Scenario: Re-render
- **WHEN** a debug view is rendered twice on one adapter
- **THEN** the images are identical

### Requirement: REND-007 Adapter selection and logging
The renderer SHALL request the high-performance adapter for interactive and performance runs and the software adapter for test runs, and SHALL log the chosen adapter's backend, name and type at start-up.

Verify: C
Status: active
Source: report §8

#### Scenario: Test render log
- **WHEN** `planet test-render` runs on the software adapter
- **THEN** its output begins with an `adapter:` line naming backend, name and device type
