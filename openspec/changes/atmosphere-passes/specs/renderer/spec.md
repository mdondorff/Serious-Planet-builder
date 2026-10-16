## MODIFIED Requirements

### Requirement: REND-005 No planet-scale shader values
Geometry shaders (terrain, tile quads, windows) SHALL receive only tile-local or camera-relative quantities and SHALL NOT compute values of planet-radius magnitude: no planet-radius uniform or constant. The atmosphere's shaders are presentation shading (CON-11 v1.4, ADR 0043, ADR 0050 point 6): they MAY hold the body's radius as a value in their uniform block, decided in f64 on the CPU, and form ray quantities by the altitude-relative primitives; they SHALL contain no planet-scale literal or radius-like name, and their declared bound (ATMO-001, ATMO-010) is tested.

Verify: A
Status: active
Source: CON-11

#### Scenario: Review rule
- **WHEN** a shader contains a planet-radius constant or a geometry shader a planet-radius uniform
- **THEN** the shader scan test fails and names file and line
