## MODIFIED Requirements

### Requirement: LOOK-007 The albedo table, the sun and the water level reach the shader
The renderer SHALL take the albedo table from the plan (`AlbedoParams::to_words`, ten `vec4<f32>` appended to the globals), the sun direction from the plan's sun and the water level W from the plan (`water_level_m`: the viewer sets it from the sea of the definition, the sea of the definition, which is M5's only producer of W, GEN-023, through `LitPlan::with_water_level`; a later change reads `WaterField::plan_water` per view; 0 for the Earth default, which leaves the plan text and hash as they were), and SHALL hold no albedo colour, threshold or sea level as a constant in a shader. Class: presentation data. Bound: with a table whose every colour is the same grey the albedo view is that grey wherever there is terrain, whatever the climate, the slope and the height (to within f16 rounding, 2e-3 relative); with W far above the terrain every pixel is the seabed colour and with W far below it none is; and the lit view equals the albedo view times the lighting view (2e-3 relative).

Verify: C
Status: active
Source: ADR 0039 points 1 and 3, ADR 0041, ADR 0042 point 6, ADR 0035

#### Scenario: Flat table
- **WHEN** the albedo view of a terrain frame is drawn with every colour of the table equal to 0.5, in both tile formats
- **THEN** every covered pixel is 0.5 / pi of a unit in the scene target

#### Scenario: Water level
- **WHEN** the water level of the plan is 100 km above and 100 km below the terrain
- **THEN** every pixel is the seabed colour in the first frame and none is in the second

#### Scenario: Lit is albedo times lighting
- **WHEN** the lit, albedo and lighting views of one plan are drawn
- **THEN** every lit pixel equals the albedo pixel times the lighting pixel over a lit unit
