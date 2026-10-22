# Change: edit-health

## Why
- M7 A5 (owner, 2026-10-21): the owner may attempt what the engine does not support; it must not crash, must not cost much performance, and must be findable and fixable. Today a brush too steep to draw wears a warning in the inspector and the layer row (VIEW-043); nothing lists them, and the only fixes are to select, type a smaller strength and delete.

## What changes
- `planet_editor::problems` (GPU-free): `Issue` (too steep, stacked, tiles capped), `Problem`, `stack_depths` and `stacked_items` (the depth of the stack at the points of a brush, and the item's stroke count against the file's cap of 64), `merge_stack`, `scaled` and `fit_factor` (the largest factor by which the strengths of a brush can be cut so that it is no longer too steep).
- `Session::problems`, `reduce_item`, `merge_item`, `items_over` (the footprint find over tiles, without the selection), `stacked_items` (cached per revision like `steep_items`).
- The `problems` tab (`Tab::Problems`, the last default tab; layout key `problems`) with rows, the issues in words and the buttons Fly to, Reduce, Merge strokes, Delete; events `Reduce`, `Merge`, `DeleteItem`; the window gives the runtime the tiles of the plan that are `Capped` each frame (`set_capped_tiles`).
- VIEW-047.

## Design notes
- Capped: the problem is the edit's, found by the footprint find over the capped tiles of the last plan (the same find that lists the edits in view); an edit too small on the screen to be shown is not listed until the camera is nearer. "Refused tiles" of the owner's note are gone since ADR 0074 (a plan is no longer refused for steep ground), so the list has capped tiles only.
- Reduce is the guard's inverse: the guard clamps a new stroke so that the stack stays under the limit; Reduce finds the factor for strokes already there (bisection on `steep_items`, 14 steps, only on a click).
- Merge is lossy by design (the others' paths are dropped, the sum along one path is steeper): it is the offered simplification for a pile of strokes painted over one place, and one undo takes it back. A smarter merge (union of the paths) is not attempted.
- The stack depth counts the discs of all strokes (any kind and item) covering a point of the brush's own strokes, with the bounding test of `steep_items`; the thresholds are constants (`STACK_WARN`, `STROKES_WARN`) to tune once the owner has used the list.

## Tests
- Tier A (editor): the list, the fixes and their undo, the refusals, the pure `merge_stack` and `fit_factor` (`crates/editor/tests/problems.rs`); the default tab (`tests/model.rs`).
- Tier A (app): the panels' rows and events (`panels.rs`), and the runtime over capped tiles, with the events as commands (`editwin.rs`).

Expected golden changes: none (the problems list is editor UI; no render change).
