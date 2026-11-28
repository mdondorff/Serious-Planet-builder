## ADDED Requirements

### Requirement: VIEW-077 The editor places, resizes and deletes region boxes and reopens a baked one
`planet editor` SHALL edit the region boxes of a regions file (WDEF-019): `--regions FILE`, else `<world-dir>/regions.toml` of `--world F`; with neither, the Sites tab SHALL say how to name one and no box is kept.
The Sites tab's Regions section SHALL list each box with its name, its size in L tiles, its face and its derived state (`unbaked`, `ready`, `stale`; WDEF-019), from the editor's current definition with its items.
Place SHALL arm a box of the chosen side (2 to 4 L tiles each axis) that follows the ground under the cursor, snapped to whole L tiles of the production layout and clamped into its allowed tiles; a click on the ground SHALL put it under the typed name (or `region-<n>`). Move SHALL re-place the selected box the same way; the width and height buttons SHALL resize it within 2 to 4 tiles; Delete SHALL remove it.
Every box the editor puts SHALL pass `check_box`; a box it refuses SHALL leave the file unchanged and say why in the status.
The file SHALL be written whole after every change, outside `WorldDefinition` (its hash unchanged).
The window SHALL draw each box's outline on the ground coloured by its state, the placement box, and the face-edge margin (the boundary of the allowed tiles) of the face being edited.
Open on a `ready` box SHALL print the command that opens its commit (`--commit`, VIEW-069) in the status and on the console; the world is not swapped in place.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2), WDEF-019, BUILD-078, VIEW-069, VIEW-075

#### Scenario: Placing at a face edge
- **WHEN** a 2x2 box is placed with the cursor on the first tile of a face
- **THEN** the box sits at the first allowed tile, passes `check_box`, is in the reread file, and the world's hash is unchanged

#### Scenario: Resize, move, delete and refusals
- **WHEN** the box is widened to 4 and once more, moved, placed again under a taken name, and deleted
- **THEN** the side stops at 4 with the reason in the status, the move snaps to the new tile, the taken name is refused, and the file ends empty

#### Scenario: State and reopen
- **WHEN** a box's commit is baked, the definition then changes, and Open is pressed on the ready box
- **THEN** the row reads `unbaked`, `ready`, `stale` in turn, and Open prints the `--commit` command of its folder
