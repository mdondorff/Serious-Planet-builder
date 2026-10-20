## Why
The line and region tools need commands over explorer's item model (ADR 0069, PR 416): a line longer than one item reaches is several linked items, and the owner's undo must take it back whole. The command layer had one item per command and no points. opus-planner (2026-10-19): the editor side of E5; stage 2a.

## What Changes
- `NewItem` carries `points`, `group`, `part` and `mask_layer`; `Editor::put` makes them part of the item.
- A command is a list of steps (one for a put, a change, a delete or a drag): `Editor::group(Vec<GroupStep>)` applies puts, changes and deletes as one command; if a step fails the steps before it are taken back; one undo and one redo take all of them.
- On top of it, with `planet_editor::polyline` (VIEW-027): `put_line` (a ridge or channel; split into linked parts when longer than one reach, one group, consecutive parts), `set_line_points` (the parts are made again; existing items are changed in place, more are put, surplus ones deleted), `delete_line`, `line_items`, `line_points`; `put_region` (refused beyond one reach, with a self-crossing or empty outline, naming the reason) and `set_region_points`.
- `EditError::Geometry`; VIEW-028 and tier-A tests (`tests/lines.rs`).

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves. The reach of a made item is its extent, width and 1 m of margin.

## Out of scope
The tools that click and drag points (interaction), the inspector and the relevance of lines and regions, the overlay; the world definition and the generator stages (sonnet-explorer).
