## ADDED Requirements

### Requirement: REND-014 Vertex-shader morph
The terrain vertex shader SHALL morph each vertex from its own position towards its parent-grid position by the factor `clamp((d - start) / (end - start), 0, 1)`, where `d` is the camera distance to the vertex's height-free reference position and `start`, `end` are the node's morph interval from the plan; the shader SHALL read no other morph input.

Verify: B
Status: active
Source: CON-11, ADR 0014

#### Scenario: GPU against CPU twin
- **WHEN** the morph debug view of a scripted view is rendered on the software adapter
- **THEN** at every visible interior vertex the red channel equals the f64 CPU value of the formula within the steepest neighbour step plus 2/255, and the worst error stays below 0.05
