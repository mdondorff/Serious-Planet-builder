# Change: stamp-bounds

## Why
- Owner test of the editor (M6, 2026-10-20): four plan refusals ("morph interval empty because the height extent of its neighbourhood leaves no room"; levels 12 to 19; plateaus and craters; CRSH-0001 to 0004). Cause: a plateau's bound over a cap was its weight times its declared `max_delta_m` (5 km for the owner's plateaus), symmetric and independent of the ground, so a fine tile inside its flat part had a 10 km extent; flatten and smooth strokes had the same shape (the sum of the strengths).
- The smooth brush sharpens after several strokes: the part a smooth has moved was counted per item, one drag is one item, so each overlapping item pulled the point to the mean of the procedural base again (two items gave `2 mean - base`).

## What changes
- GEN-026: the bounds of a stamp stage are the range of the running height carried through the items in application order. A plateau maps the range (monotone in the height, linear in the weight), so over a flat part it adds the extent of the gap to its target, not its declared bound.
- GEN-030: flatten maps the range as a plateau does; a run of smooth strokes adds at most the width of the distance to the mean of the base; the smoothed part is counted over the items of the layer.
- `STAMP_STAGE_VERSION` 3 to 4 (the output of overlapping smooth items changes); items of version 3 are re-keyed on load (AUTH-002).
- The sentence of GEN-030 "the stage version is unchanged" (a layer without a brush item) is dropped: the version is 4.
- ADR 0067 point 4 (bounds by delta range) is kept; the LOD rule is not touched. `max_delta_m` stays the declared bound; the contract test still checks it.
- Tier A is the evidence for the plateau (the frame-level plan test took 35 minutes and is not kept); the plan of the owner's items is checked by hand in the editor.

## Tests
- `stamp_stage::a_plateau_of_a_large_declared_bound_adds_little_to_the_extent_of_a_fine_tile_whatever_its_target` (the owner's Plateau 7: 15 km reach, target 300 m, bound 5 km; failed with a growth of 10,000 m).
- `stamp_stage::the_bounds_of_plateaus_over_other_stamps_and_a_steep_base_hold_for_random_caps`, `brush_stage::the_bounds_of_smooth_and_flatten_strokes_hold_for_random_caps_over_other_strokes_and_a_steep_base_and_stay_tight_on_a_fine_tile`.
- `brush_stage::overlapping_smooth_items_converge_on_the_mean_together_and_never_pass_it`, `brush_stage::the_bounds_allow_the_rounding_of_every_smooth_stroke_of_a_run_over_a_base_with_exact_bounds` (the numerics-reviewer's finding).

Expected golden changes: none (no golden PNG moves: `cargo xtask test gpu` and `--real` are green). One recorded known answer moves: the hash of `gpu_stamps::a_stamped_scene_in_the_relief_view_is_the_recorded_frame_of_the_adapter_family` (WARP and RTX), because the tighter bounds change which tiles the planner selects for the stamped field (no height changes); re-recorded after looking at the frame (the hill, the crater and the plateau show, no hole or seam).
