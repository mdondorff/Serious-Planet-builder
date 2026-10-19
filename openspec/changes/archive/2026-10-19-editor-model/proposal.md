## Why
M6 step U3 (editor panels), first half: everything the panels show and every flow the pointer and keyboard start, as GPU-free tested logic, so the egui panels (U3.2) are thin glue and the owner's rule "the camera decides which edits show" and "handles are the primary way to edit a stamp" are tier-A tests (docs/status.md "Editor MVP requirements", docs/briefings/m6-editor-plan.md U3).

## What Changes
- `planet_editor::relevance` (VIEW-015): the footprint find over the visible tiles, then the screen size of the reach (`MIN_REACH_PX` 8), the selected edit always.
- `planet_editor::lists` (VIEW-016): the nearby list, the planet-wide search sorted by distance, the layers and the chain.
- `planet_editor::shape`, `params`, `session` (VIEW-017): outline and handles, drag arithmetic, picking among the shown edits, placing, deleting, typed metric values with their units and ranges, a drag as one command, the inspector as data.
- `planet_editor::layout` (VIEW-018): the panel layout as a value read from a TOML file.
- `Editor::items()`.

## Notes
- The parameter table (`crater`, `hill`, `plateau`, with `radius_m`, `height_m`, `depth_m`, `rim_m`, `softness_pct`) is provisional until the stamp stage fixes its parameter names (E2); parameters the table does not know are still shown and edited without a range.
- The panels in `app` (U3.2) and the call of `ManagedLoop::edit` with the tiles of the footprint find (U3 step b, after `live-definition`) follow.

## No golden moves
Editor-crate logic and tests, and spec text only.
