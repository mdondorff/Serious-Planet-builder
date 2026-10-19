## Why
Window 14 (2026-10-19) attributed the preview time (74 to 247 ms against 16 ms) to the plan worker: the preview appears in the frame that delivers the first plan made after the edit (within 1.5 ms of it in all nine cases), because the plan in hand carries the mesh keys of the unedited world, so the resident tiles still match it and `draw_plan_preview` flags nothing until the new keys arrive (opus-planner go, 2026-10-19: flag the nodes from the edit's found set on the main thread).

## What Changes
- **REND-033 modified**: a tile named by the edit in flight is drawn as stale and previewed from the frame after the edit until a plan made after the edit is delivered; from then on the keys decide as before.
- **Code**: `PlanPipeline::mark_edited` (called by `ManagedLoop::edit`), `draw_plan_edited` (the draw with the set of edited tiles; `draw_plan_preview` is it with an empty set).

## Impact
- Spec: REND-033 (modified, renderer). Code: `crates/frame/src/pipeline.rs`, `crates/frame/src/managed.rs`, one tier-A test in `crates/frame/tests/managed.rs`.
- Goldens: none moves (no edit in flight in any golden scene; an empty edited set is the old draw).
- Frames after an edit are `Stale` (not `Exact`) one plan longer than before for the named tiles only; the editor's idle test is unchanged.
- numerics-reviewer: not needed (no numeric or shader change); spec-guardian before archive.
