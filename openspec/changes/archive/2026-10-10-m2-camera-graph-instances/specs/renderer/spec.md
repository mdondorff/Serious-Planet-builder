## ADDED Requirements

### Requirement: REND-010 Adaptive camera
The camera controller SHALL move at a speed proportional to the height above the terrain (within configured limits), SHALL support orbit and fly modes, SHALL never come closer to the terrain than a clearance, and SHALL be a deterministic function of its state, input and step.

Verify: A
Status: active
Source: report §10, CON-04

#### Scenario: Dive
- **WHEN** the camera descends towards flat ground
- **THEN** its speed never increases and it stops at the clearance

#### Scenario: Orbit
- **WHEN** the camera orbits at a fixed distance
- **THEN** it keeps looking at the planet centre and the distance stays constant

### Requirement: REND-011 Instance culling reference
The instance path SHALL have a CPU reference that conservatively culls bounding spheres against the four side planes of the view frustum (keeping every sphere that touches the frustum, possibly also spheres near its corners; no near or far plane), produces indirect draw arguments and camera-relative float32 instance positions, which a GPU culling pass must match (REND-008).

Verify: A
Status: active
Source: CON-03, CON-11, ADR 0013

#### Scenario: Frustum
- **WHEN** 5,000 instances are culled against a frustum
- **THEN** exactly the spheres whose centre is not farther outside a side plane than their radius are kept, in ascending order, and the indirect instance count equals their number

### Requirement: REND-012 Render graph
Rendering SHALL be described by a render graph whose passes declare reads, writes and side effects; compiling it SHALL reject a read without an earlier producer, drop passes whose outputs are unused, and compute transient lifetimes and alias groups of equal memory class.

Verify: A
Status: active
Source: report §8, ADR 0013

#### Scenario: Unused pass
- **WHEN** a pass writes a resource nobody reads and has no side effect
- **THEN** it is culled from the compiled order

#### Scenario: Missing producer
- **WHEN** a pass reads a resource that no earlier pass writes and that is not external
- **THEN** compiling fails naming the pass and the resource
