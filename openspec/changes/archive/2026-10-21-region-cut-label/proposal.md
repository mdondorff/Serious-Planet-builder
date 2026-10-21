# Change: region-cut-label

## Why
- Owner test of the editor (M6, 2026-10-20, block 3): a region set to "clip" a layer removed that layer's edits inside the region; the owner expected the edits to apply only inside it.
- The code is what ADR 0069 point 6 says: a mode-2 mask multiplies the layer's deltas by 0 where the point is inside the polygon (GEN-029), with a hard edge (a feathered edge is later). The mode the owner expected, the edits kept only inside (cut outside), is the ADR's mode 1, "clip outside", which the world definition refuses (`openspec/changes/region-mode1`).
- The word "clip" read both ways: a requirement-wording mismatch, not a stage bug.

## What changes
- The editor says what mode 2 does: the checkbox reads "Removes the edits of a layer inside the outline", its tooltip says the edits are cut away inside with a hard edge and stay outside, and that keeping them only inside is not offered yet (`planet_editor::params::MASK_CUT_LABEL`, `MASK_CUT_HINT`; inspector rows "Mask" and "Layer cut").
- VIEW-033, WDEF-012 and GEN-029 name mode 2 "cut inside" and say which side keeps the edits.
- No change to the stage, the item file or any key.
- Keep-inside (mode 1) stays in `openspec/changes/region-mode1` for the owner's decision (build now or in M7; about a day of work, a full refresh of the layer per edit of the region). Until then the editor's workaround is cutting inside regions that tile the complement.

## Tests
- `panels::tests::the_mask_label_says_what_the_stage_does_the_edits_are_removed_inside_the_outline_and_stay_outside_it` (VIEW-033): the stage removes a hill's edit inside a mode-2 region and keeps it outside; the label and the tooltip say so and read as nothing else.

Expected golden changes: none.
