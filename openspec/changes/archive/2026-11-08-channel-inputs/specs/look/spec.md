## MODIFIED Requirements
### Requirement: LOOK-008 Ground material weights
The weights of the ground materials (the detail at the lowest levels, M7 G2) SHALL be one function, `MaterialParams::weights`, of a fixed vector of ten inputs in a fixed order (the slope cosine, the height above the water, the temperature, the precipitation, the region's surface type, three driver slots: soil depth, moisture, insolation, and the fill depth and erosion depth of the tile's channel block, GEN-126, in metres, 0 without a bake), with the response curves per material as look data and no material as a branch of code. A material's weight SHALL be the product over the inputs of the smoothstep response of a `(lo, hi)` pair (0 at `lo`, 1 at `hi`, falling when `lo > hi`; a pair with `lo == hi` leaves the input out; a material with all pairs equal is off), the weights SHALL be normalised to sum to 1 when any is positive and all be 0 otherwise, and they SHALL be monotone in each input a curve uses. The default curves SHALL use the thresholds of the base albedo (`AlbedoParams::EARTH`), so that the detail and the colour agree on what is rock, desert and vegetation. A driver map SHALL be a new value in a reserved slot and new curves in the look, with no change of the shader's interface (the input vector and the curve table). The curves SHALL be part of the lit plan, written in its text only when they are not the default (so a plan made with the defaults keeps its text and hash), and the table SHALL reach the shader as `MATERIAL_VEC4S` words of `f32`, each value rounded once. Presentation data (ADR 0035).

Verify: A, C
Status: active
Source: M7 G2 (opus-planner, 2026-10-21), SPIKE-27, ADR 0039 point 3, ADR 0035, GEN-126 (planner inbox #67)

#### Scenario: The default
- **WHEN** the weights are evaluated on steep ground, on flat dry ground and on flat wet ground with the default curves
- **THEN** rock, sand and grass are the material of each, the weights sum to 1 everywhere, and soil is off

#### Scenario: A driver slot
- **WHEN** a look gives soil a curve on the reserved soil-depth input
- **THEN** soil takes weight where the depth is, with the same function and the same input vector

#### Scenario: A channel slot
- **WHEN** a look gives wet ground a curve on the fill depth and rock one on the erosion depth, and the lit frame is drawn over tiles with a deep-fill channel block and over neutral ones
- **THEN** the picture differs where the fill is deep, the default look draws the same pixels over any channel block, and the channel debug views show the block's depths

### Requirement: LOOK-011 The ground look in the look file
The look file SHALL hold the ground look of a body in an optional table `[body.<id>.ground]` (schema version 5): `detail`, the strength of the ground detail (LOOK-009), and per material (`rock`, `sand`, `grass`, `soil`) a sub-table with the `(lo, hi)` pair of each input of LOOK-008 as two numbers (`slope_cos = [0.86, 0.62]`). A missing key SHALL be the default: detail `NATURAL_STRENGTH` and the Earth curves. A value SHALL be refused naming its key and value as LOOK-008 and LOOK-009 refuse it; an unknown material or input SHALL be an unknown key. The canonical text SHALL write every key. Schema version 10 adds the inputs `fill_depth` and `erosion_depth`. Presentation data (ADR 0035).

Verify: A
Status: active
Source: opus-planner (2026-10-28; owner: compare the looks live), LOOK-008, LOOK-009, CON-10

#### Scenario: A ground table
- **WHEN** a look file gives soil a moisture curve and a detail of 0.6, and another gives a detail of 1.5 or a curve with one value
- **THEN** the first reads with the Earth curves for everything else and writes back equal; the others are refused naming `ground.detail` and `ground.<material>.<input>` with the value

#### Scenario: No table
- **WHEN** a look file has no `ground` table
- **THEN** its ground look is the natural detail and the Earth curves, which is what the window shows without the file
