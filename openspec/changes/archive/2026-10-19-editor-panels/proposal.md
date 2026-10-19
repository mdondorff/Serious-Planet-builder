## Why
M6 step U3, second half: the egui editor panels and the window wiring, so the owner can try the editor hands-on (docs/status.md "Editor MVP requirements"; docs/briefings/m6-editor-plan.md U3): the camera decides which edits show, handles on the planet are the primary way to edit a stamp, the inspector takes typed metric values, undo and redo work, and the layout is a file.

## What Changes
- `planet_editor::interaction`, `feed`, `overlay` (VIEW-019, VIEW-020): the pointer as session calls, the tiles an edit sends to refresh, the edits on the screen.
- `planet::panels` (VIEW-021): the tool rail, tabs (near with search, layers, graph), inspector, status bar and the overlay in egui, driven by the layout value.
- `planet::editwin` and `window` (VIEW-022): the editor mode (Tab, `--edit`, `--items DIR`, `--editor-layout F`), its keys, the right button to look around, fly to an edit, the smoke script and verdict.

## Notes
- VIEW-003 and VIEW-010 gain one clause each: in the editor mode Esc does not quit and the right button captures the pointer while the left button edits.
- Undo and redo are the keys labelled Z and Y (logical keys), so they are right on a QWERTZ keyboard (docs/learnings.md).
- The terrain does not change yet: the definition the generator reads is swapped by `WorldTerrain::set_definition` (explorer's `live-definition`) and `ManagedLoop::edit` takes the tiles `feed` finds; the mode counts the tiles it would send. The next step wires both when `set_definition` is in main (tracked in docs/status.md).
- The parameter names are the provisional table of `planet_editor::params`; the stamp stage's spelling (kind, `reach_m` as the radius, `height_m`, `depth_m`, `target_m`) is adapted in a small change once the stamp stage is in main.
- Lines, regions and the brush are the later steps E5 and E6; their rail buttons are disabled.

## No golden moves
Editor-only UI, drawn only in the editor window.
