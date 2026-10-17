## ADDED Requirements

### Requirement: VIEW-008 The atmosphere that does not fit the body
A look whose atmosphere does not fit the body of the world (a top above the body's surface) SHALL leave the viewer on the lit path without the atmosphere, and the run SHALL print why (`atmosphere off: <reason>` and `view: the lit path with no atmosphere`); the smoke verdict SHALL then require no frame with the atmosphere and the run SHALL succeed. The decision is `viewer_lit_plan`'s refusal (VIEW-007); this requirement is its tier-C test through the binary.

Verify: C
Status: active
Source: VIEW-007, opus-planner request of 2026-10-17

#### Scenario: A small body
- **WHEN** `planet editor --smoke` runs a world of 60 km radius under the default look (an atmosphere 100 km thick)
- **THEN** the output says the atmosphere is off and why, the lit path ran without it for at least 16 frames, no frame has the atmosphere and the run ends `[smoke ok]`

### Requirement: VIEW-009 Water in the window
The viewer SHALL draw the water of a world that has a bake: the world computes the ocean channel with its in-memory bake (or reads it from the store) and assembles the water field W from the sea of its definition; each lit frame's plan SHALL carry the water plan of ATMO-012 made from the frame's terrain plan, the world's liquid table, the look's water parameters (LOOK-001) and the injected clock's time (CON-04); the window SHALL warm the water pass with the lit path before frame 0 (REND-017) and its plans SHALL bound their tiles up to the sea level for the frustum cut (ATMO-016); the smoke summary SHALL report the lit frames that had water. A world with no bake has no water and draws as before.

Verify: C
Status: active
Source: ADR 0041 points 4 and 8, ADR 0051 point 6, ADR 0042 point 4

#### Scenario: A world with a sea
- **WHEN** `planet editor --smoke` runs a world with the default field over an in-memory bake
- **THEN** the summary counts at least 16 lit frames with water and the run ends `[smoke ok]`
