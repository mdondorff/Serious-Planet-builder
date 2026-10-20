## Why
The line and region tools work in the session (VIEW-029) but the window cannot draw a line yet. The owner wants to try it hands-on in the window (opus-planner, 2026-10-20): the tools in the rail and on the number keys, the points on the planet, the draft on the screen, and a list of the keys.

## What Changes
- The rail has the ridge, channel and region tools (keys 5, 6, 7; the "later" buttons for line and region go); picking one puts the way to use it in the status line.
- Enter finishes the line or region being drawn; Backspace takes the last point back (with no draft it deletes the selected edit as before); Esc drops the draft before it leaves the tool; Shift + click on the outline of the selected line or region adds a point; Ctrl + click on a point's handle removes it; F1, `?` and a `?` button in the status bar open a window that lists the keys.
- The overlay draws a line as an open path and a region as a closed one along the great circles (the selected one as the session holds it while a point is held), the point handles as amber dots, and the draft with its points and a thin line to the cursor.
- A click picks a line within its half-width or about 15 pixels of it, and a region inside it or at its edge; a stamp over them still comes first.
- VIEW-031; tier-A tests of the picking, the overlay data, the tool hint, the key map, the window flow (keys, Shift and Ctrl presses, Esc order) and the rail.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves (the overlay is egui over the frame).

## Out of scope
The inspector rows and typed values for lines and regions (the next change); the surface-type tint of a region (explorer, VIEW-030); how the egui drawing looks (tier D: the owner tries it).
