# Change: ground-materials

## Why
- M7 G2, ground detail build on SPIKE-27's approach (opus-planner, 2026-10-21): the owner moves towards driver maps (soil depth, moisture, insolation) as the source of materials and instances. So the material weights come from one function, inputs to weights, with the response curves as look data. This is its first of three changes: the function and its data. The shader term (change 2) and the tuning, goldens and the look file's table (change 3) follow.

## What changes
- `look/materials.rs`: `MaterialParams` (curves per material and input), `MaterialInputs`, `weights`, `to_words` / `as_f32` / `bits` / `from_bits` / `validate`; four materials, eight inputs (three reserved for driver maps).
- `LitPlan::materials`, written in the plan text only when not the default (token `materials`, last in the order); validated.
- LOOK-008.

## Design note: the seam for the driver maps
- The shader's interface is the input vector (two `vec4<f32>`) and the curve table (`MATERIAL_VEC4S` `vec4<f32>`); a driver adds a value to a reserved slot and curves to the look, never a branch. Materials are slots of the table, not code; a fifth material is a larger table (a constant and the layout of the uniform), not a new function.
- Not in this change: the look file's `[materials]` table (the default is in code until change 3 reads it), the shader, the octaves.

## Tests
- `materials.rs` (tier A): the default classifies steep / dry / wet ground and sums to 1; monotone, an input left out changes nothing, a reserved slot makes soil a material without a new interface; round trips (bits, f32 words); the plan text omits the default and round-trips otherwise; validation names the key.

Expected golden changes: none (the lit plan's text and hash are unchanged with the default curves).
