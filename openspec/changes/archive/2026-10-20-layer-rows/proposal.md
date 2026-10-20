## Why
The layer list had a row per item and said nothing of lines, regions and brushes: a ridge of 150 km was five rows of "ridge xxxxxx", a brush was a row like a stamp, and deleting a part of a line left a gap. A brush has no handle, so its row is how it is found and deleted (opus-planner, 2026-10-20, item 3).

## What Changes
- A `Row` carries the parts of its group, which part it is, its points and its strokes; `Row::detail()` is the text: `3 strokes`, `N parts, S strokes`, `N parts`, `part K`, `17 points`, nothing for a stamp. The panels show it after the kind (`row_text`), and a brush has a colour.
- The layers list shows a split line or a brush drag as one row (its first part) with the parts count and the strokes of all of them; the nearby list, the search and the chain keep every item.
- `Session::toggle_hidden` and `Session::delete_selected` take the whole group (the line's parts, a brush drag's items) as one command; before, deleting a part of a line left the others.
- VIEW-016 modified; tier-A tests of the rows, the selection, the hiding and the delete, and of the panel.

## Classes and bounds (ADR 0035)
No field and no shader value; no golden moves.

## Out of scope
The look of the rows (tier D); per-stroke deleting of a brush; the nearby list collapsing parts.
