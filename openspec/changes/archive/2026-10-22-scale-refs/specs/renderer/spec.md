## ADDED Requirements

### Requirement: REND-036 Scale references pass
The lit path SHALL draw the scale references it is given (`TerrainRenderer::set_scale_refs`, VIEW-050) as boxes into the terrain pass, with the terrain's depth test, so the terrain hides what lies under it and the boxes hide what lies behind them. A box is shaded by Lambert diffuse `albedo * (max(n . sun, 0) + 0.12) / pi` at the plan's pre-exposure, in the units of the lit terrain. Positions reach the shader relative to the camera in f32 (CON-11); boxes beyond 200 km of the camera are left out. The light is the plan's sun only, with no planet shadow or atmospheric transmittance (day-side use). A frame without references or on the debug path is what it was.

Verify: C
Status: active
Source: owner (2026-10-22, via opus-planner), VIEW-050, REND-026

#### Scenario: Depth and absence
- **WHEN** a red cube is set half way to the ground on the view axis, then one under the ground, then none, and a debug frame is drawn with and without the cube
- **THEN** the first frame differs from the plain one in red pixels, the second and the third equal the plain frame, and the debug frames are equal
