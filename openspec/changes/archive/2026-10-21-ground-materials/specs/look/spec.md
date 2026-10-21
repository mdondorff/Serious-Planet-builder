## ADDED Requirements

### Requirement: LOOK-008 Ground material weights
The weights of the ground materials (rock, sand, grass, soil; the detail of the ground at the lowest levels, M7 G2) SHALL be one function, `MaterialParams::weights`, of a fixed vector of eight inputs in a fixed order (the slope cosine, the height above the water, the temperature, the precipitation, the region's surface type, and three slots reserved for driver maps: soil depth, moisture, insolation), with the response curves per material as look data and no material as a branch of code. A material's weight SHALL be the product over the inputs of the smoothstep response of a `(lo, hi)` pair (0 at `lo`, 1 at `hi`, falling when `lo > hi`; a pair with `lo == hi` leaves the input out; a material whose pairs are all equal is off), the weights SHALL be normalised to sum to 1 when any is positive and all be 0 otherwise, and they SHALL be monotone in each input a curve uses. The default curves SHALL use the thresholds of the base albedo (`AlbedoParams::EARTH`), so that the detail and the colour agree on what is rock, desert and vegetation. A driver map SHALL be a new value in a reserved slot and new curves in the look, with no change of the shader's interface (the input vector and the curve table). The curves SHALL be part of the lit plan, written in its text only when they are not the default (so a plan made with the defaults keeps its text and hash), and the table SHALL reach the shader as `MATERIAL_VEC4S` words of `f32`, each value rounded once. Presentation data (ADR 0035): nothing is stored, keyed or read back.

Verify: A
Status: active
Source: M7 G2 (opus-planner, 2026-10-21), SPIKE-27, ADR 0039 point 3, ADR 0035

#### Scenario: The default
- **WHEN** the weights are evaluated on steep ground, on flat dry ground and on flat wet ground with the default curves
- **THEN** rock, sand and grass are the material of each, the weights sum to 1 everywhere, and soil is off

#### Scenario: A driver slot
- **WHEN** a look gives soil a curve on the reserved soil-depth input
- **THEN** soil takes weight where the depth is, with the same function and the same input vector
