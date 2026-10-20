## Why
The command layer puts and edits lines and regions (VIEW-028); the owner still needs to draw them and move their points on the planet (E5, ADR 0069, owner 2026-10-19). opus-planner: the editor side of E5, stage 2b-1: the model of the tools, tested without a window.

## What Changes
- `Tool::Line(kind)` and `Tool::Region` collect a draft of points from clicks (`Session::add_draft_point`, `undo_draft_point`, `cancel_draft`); `finish_draft` puts the line (a ridge or a channel with the default amount 30 m and half-width 200 m, split into linked parts when long) or the region as one command, selects it and goes back to select; a refusal keeps the draft.
- `Handle::Point(i)`: one handle per point of a line (over all its parts) or a region; a point drag keeps the points in the session and makes one `set_line_points` / `set_region_points` command at the release; Esc drops it; a refused point list leaves the item as it was.
- `Session::drawn_points` (what the window draws while a point is held), `insert_point_at` (a click on the outline adds a point, one command) and `remove_point` (refused below the kind's minimum).
- `Interaction::press` adds a draft point with a line or region tool on (`Pressed::DraftPoint`); a press on a point handle starts the drag.
- VIEW-029 and tier-A tests (`tests/line_tools.rs`).

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves.

## Out of scope
Drawing the draft and the outlines in the window, the tool palette entries and the Enter / Esc keys (the window); the inspector rows, typed width and amount for a line, and picking by distance to the outline (stage 2b-2); the region overlay (explorer).
