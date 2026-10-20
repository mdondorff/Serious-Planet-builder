## ADDED Requirements

### Requirement: VIEW-031 Drawing lines and regions in the window
In the editor mode the rail SHALL offer the ridge, channel and region tools after the stamp tools, and the number keys 5, 6 and 7 SHALL pick them; picking one SHALL put the way to use it in the status line. Enter SHALL finish the line or region being drawn; Backspace SHALL take the last point of the draft back and, with no draft, delete the selected edit; Esc SHALL step back one thing at a time: a drag, then the draft, then the tool, then the selection. A press with Shift held on the outline of the selected line or region SHALL add a point there (refused, with the reason in the status line, when not on the outline) and a press with Ctrl held on one of its point handles SHALL remove that point (a Ctrl press that finds no handle does nothing but say so); neither starts a drag, and the distance from the outline that counts as on it is the one a click uses to pick (the half-width of a line or 1.5 % of the distance, the larger). F1, the key that types a question mark and a button in the status bar SHALL show and hide a window that lists the keys. A click SHALL pick a line whose distance to the click is within its half-width or 1.5 % of the distance to the surface, the nearest first, else a region the click is inside or within that tolerance of the edge; a stamp whose outline holds the click comes before both. The overlay SHALL give a line as an open path and a region as a closed one, each segment cut into 8 pieces along its great circle, a selected line or region as the session holds it (a held point already moved), and the draft as its path, its points and the cursor for a rubber band, reading the height of the ground once at each point and not at each piece; the window SHALL draw them.

Verify: A
Status: active
Source: opus-planner (2026-10-20), VIEW-017, VIEW-019, VIEW-020, VIEW-029

#### Scenario: Draw and finish a line
- **WHEN** the ridge tool is picked with 5, three places are clicked, the last point is taken back with Backspace and put again, and Enter finishes
- **THEN** the draft is shown with a line to the cursor, one command puts the line, it is selected, and its point handles are on the screen

#### Scenario: Shift and Ctrl
- **WHEN** Shift is held on a click between two points of the selected line, and Ctrl on the handle of a point
- **THEN** the first adds a point on the line, the second removes the point, and neither starts a drag

#### Scenario: Picking
- **WHEN** a click lands 150 m beside a line of half-width 200 m, 350 m beside it, and inside a region the line crosses but away from the line
- **THEN** the first picks the line, the second nothing, the third the region
