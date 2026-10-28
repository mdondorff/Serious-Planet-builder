## ADDED Requirements

### Requirement: TEST-065 The gate judges horizon bakes at every site and the re-bake after its edit
The gate SHALL fail a lit run with the horizon jobs on when no horizon map was baked, and when no map landed within 10 s of its scripted edit (the shadow of a plateau on its neighbour, M8 criterion 6). On the software adapter both are reported and not judged.

Verify: A
Status: active
Source: opus-planner M8-F step 2, TEST-061

#### Scenario: A slow re-bake
- **WHEN** the first map landing after the edit comes 11 s after it, or none comes
- **THEN** the verdict fails naming the time or that none landed
