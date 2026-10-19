## Why
The owner's relevance rule (docs/status.md, "Relevance"): the camera decides which edits show, in markers, outlines and the list alike. Outlines and lists followed it; the markers of the globe overview came from `--marker` only.

## What Changes
- The globe overview shows a marker for each edit the camera shows (VIEW-015 over the tiles the last plan wanted, at the overview's camera), labelled by name, sized by the outline radius; a click on one selects the edit and flies there. It works with the editor mode off.
- `ui::MarkerLayer` carries the labels; `EditRuntime::{globe_markers, marker_labels, select_marker}`.
- Turning the mode on ends the overview once instead of every frame, so G shows the overview in the mode; C stays inert there. VIEW-022 modified.

## No golden moves
Editor-only UI.
