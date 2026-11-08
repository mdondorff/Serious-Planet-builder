## Why
- 2b PR 4c (planner inbox #67, #68): the channel block of the compact slot (GEN-126, 4b) holds the fill depth and the erosion depth of the bake, but no shader reads it. SPIKE-100 D1 (material thresholds) reads the erosion depth ("rock where it exceeds 2 m") and the owner's direction (inbox #65) reads the fill depth (shallow fill wet or bog, deep fill gravel or sediment flats).

## What Changes
- LOOK-008: `INPUTS` 8 to 10 (`fill_depth`, `erosion_depth`, metres, 0 without a bake), the curve table 24 to 30 `vec4` words (five a material), `MATERIAL_VEC4S` 30; the Earth curves leave both inputs out, so the weights and every picture are what they were.
- The terrain shaders (`fs_lit_view`, `fs_lit_atmo_view`) read the channel block bilinearly (`tile_channels`, decoded square-root codes over 0 to 64 m) and pass it to `ground_detail` and the material weights; the reference renderer holds the neutral value 0.
- Debug views `fill` and `erosion` (`ShadeView` indices 9 and 10, false colour 0 to 64 m on a square-root scale).
- Look file schema 10 (identity migration; two optional keys per material table); the repro bundle's `materials` token grows from 96 to 120 numbers, so older bundles with a materials token are refused (owner, 2026-11-05: older data need not stay compatible); the poses file's `[[look.curve]]` input names follow `INPUT_NAMES`.
- 4d (the curves on these inputs, the owner's look) is not part of this change: the default look stays neutral.

## Impact
- Code: `frame` (`look/materials.rs`, `look/shading.rs`, `look/file.rs`), `render` (`terrain.rs`, `atmosphere.rs`, `compact.rs`, `reference.rs`), `app` (`stills.rs`).
- No golden moves: tier C `the_default_look_draws_the_same_pixels_whatever_the_channel_block_holds_and_a_curve_on_the_fill_depth_moves_the_picture` shows the default look bit-identical over a uniform and a ramp channel block at orbit and at 1 m.
- Cost: one more word read per fragment (a 9 x 9 bilinear of the channel block) and one more vec4 compare per material in the weights; the same-run ratio is the planner's perf window (REND-034 budget stays the owner's).
