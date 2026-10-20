## Why
Bumping the stamp stage's version (line-stage: 1 to 2) made the editor refuse every saved item file authored against version 1 (AUTH-002 refused any stage-version difference), although the points and hills of those files make the same stamps under the new stage. A stage bump must not make authored work unloadable (opus-planner, 2026-10-20; ADR 0007's re-bake intent).

## What Changes
- **AUTH-002 modified**: an item of an older stage version loads; the editor's load re-keys it under the active stage version, writes it back and reports it (`Editor::load_report`, `LoadReport`); only a newer stage version is refused (with the other differences, as before).
- **Code**: `Item::check_versions` (older stage no longer a difference), `Item::is_older_stage`, `Item::restaged`; `Editor::load_report` (`load` is its count); the window's status line says how many items were re-keyed.

## Impact
- Spec: AUTH-002 (modified, authored-layers). Tests: `crates/world-def/tests/item_file.rs` (the version test now expects an older stage to pass and a newer one to fail), `crates/editor/tests/stage_versions.rs` (a version-1 file loads under version 2, written back, a newer is refused).
- No golden, budget or format change; the item file keeps its schema.
