## ADDED Requirements

### Requirement: VIEW-019 What the pointer does on the planet
The editor SHALL turn the pointer into session calls in one place (`planet_editor::interaction`, GPU-free, f64 on the CPU), from plain data the window resolves: the cursor, the surface hit, the shown edits and the handles on the screen.
- A press takes a handle of the selected edit when the cursor is within the hit radius of it (`handle_hit_px` of the layout, VIEW-018; the nearest wins, the radius inclusive).
- Else, with the stamp tool and the cursor on the surface, the press places a stamp (selected, the tool back to select); with the stamp tool and the cursor off the planet it does nothing and the tool stays.
- Else the press selects the shown edit under the cursor (VIEW-017), or clears the selection when there is none.
- While a handle is held a move makes the item the pointer says (position and radius follow the surface hit and stay put without one; the height follows the vertical move of the cursor in pixels times the metres per pixel at the handle, up raises); the release ends the drag as one command; an abandoned drag goes back and records nothing.
- The hover is the shown edit under the cursor; it is none over a handle and does not change while a handle is held.

The tiles an edit sends to refresh are the resident tiles whose area with a halo of 0.25 tile edges a reach of the places the commands changed can touch (`planet_editor::feed`, the footprint touch test with its margins: false positives only cost a refresh); they come in tile order without repeats, and the places of another body count for none.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, 2026-10-19), VIEW-012, VIEW-017, WDEF-007

#### Scenario: Handles under the cursor
- **WHEN** three handles lie at known pixels and the cursor is near each, on the edge of the radius, just beyond it, between two within reach, and there are none
- **THEN** the nearest within the radius is taken, the radius is inclusive, a cursor beyond it takes none

#### Scenario: Hover and press
- **WHEN** the cursor is over an edit, off every outline, off the planet, over an edit that is not shown, over a handle, and a handle is held; then presses land on an edit, on nothing and in the sky, and the stamp tool presses on the planet and off it
- **THEN** the hover follows (none over a handle, unchanged while a handle is held), a press selects without a command, clears the selection, or places one selected stamp, and the stamp tool off the planet changes nothing

#### Scenario: Drags
- **WHEN** a press on the move handle is followed by fifty moves and a release, then drags of the height handle (20 pixels up at half a metre a pixel) and the radius handle, a drag with no surface hit, and an abandoned drag
- **THEN** each drag is one command and three undos reach the exact starting hash, the height rose by 10 m, a move without a hit changes nothing, and the abandoned drag changes nothing and records nothing

#### Scenario: The tiles to refresh
- **WHEN** a 5 km reach at one place is sent over all level-8 tiles, the place before and after of a move, another body, a larger reach, and empty lists
- **THEN** a handful of tiles are found including those of points 4.9 km away in every direction and the tile of the centre, none far away, both places count, another body counts for none, a larger reach finds more, and empty lists find nothing

### Requirement: VIEW-020 The edits on the screen
The editor SHALL turn the shown edits into plain data for the egui layer (`planet_editor::overlay`, GPU-free, the camera's f64 projection): for each edit the label, kind, selected and hovered flags, the anchor in pixels, and an outline of `OUTLINE_SEGMENTS` = 64 points on the ground at the outline radius, closed; the selected edit carries its handles in pixels and is last in the list (drawn on top).
- A point is on the screen only when it is on the side of the planet the camera sees (`faces_camera`) and inside the viewport; any other point is `None`.
- The ground height at the edit lifts the outline and the handles.
- An id with no edit is skipped.

Verify: A
Status: active
Source: owner (M6 step U3: outlines and names on hover, handles on the planet, 2026-10-18), VIEW-012, VIEW-013

#### Scenario: The ring
- **WHEN** a hill is seen from 2 km straight down, with another edit hovered and the hill selected
- **THEN** the selected edit is last, only it has handles, its 64 points lie at `400 / 2000 * 500 / tan(30 degrees)` pixels from the centre within 1 %, the move handle is on the centre, the radius handle on the outline within 1 %, the height handle behind the move handle straight down and above the centre from an oblique view, and a ground 500 m up grows the ring

#### Scenario: Off the screen
- **WHEN** the ring leaves the screen, the edit is on the other side of the planet, an edit is 1 degree and another 3 degrees along the horizon from a camera 2 km up, and an id has no edit
- **THEN** those points are `None`, the far side shows nothing, the 3 degrees edit is behind the horizon though the projection alone would draw it, the 1 degree edit is drawn, and the id is skipped

### Requirement: VIEW-021 The editor panels
The editor window SHALL draw the editor panels with egui from the layout value (VIEW-018) (`planet::panels`, no editing state of its own): the tool rail (select, the stamp kinds with the numbers 1 to 4, brush, line and region disabled as later), the left panel with its tabs, the inspector on the layout's side, the status bar, and the outlines, names and handles over the planet. The panels read plain data and return events; the window applies them to the session.
- The tabs are `near` (a search box, the kind filter, the rows: the shown edits nearest first, or the planet-wide matches when the query or the kind filter is set), `layers` (by layer with a checkbox that hides the edit in the views, in the order they apply) and `graph` (the chain from the procedural base to the tiles).
- A click on a row asks to select the edit and a double click to fly to it.
- The inspector shows the selected edit: name, kind, layer, id, anchor, distance, place in its layer, reach and the parameters with their ranges.
- The name field asks for a rename when it is left changed.
- A parameter field shows the value in its unit and follows the value while it is not being typed in.
- A parameter field asks for the typed text when it is left and differs from the value.
- The inspector has Fly to, Deselect and Delete buttons.
- Undo and Redo buttons are disabled when there is nothing to do.
- The status bar shows the status line, the count of edits, the undo depth, where the edits are saved and the window's own status.
- A rail of width 0 or no status bar is not drawn; the first tab of the layout is the first shown.

Verify: A
Status: active
Source: owner (M6 step U3: map first, list, search, inspector bound to the commands, layer list and graph as secondary views, 2026-10-18), VIEW-011, VIEW-018

#### Scenario: Where the panels sit
- **WHEN** the default layout and a layout with the inspector on the left, no rail, no status bar, a wider left panel and one tab are drawn at 1920x1080
- **THEN** the panels have the widths and sides of the layout, the middle of the window is not covered, and the tabs, rail and status bar are as the layout says

#### Scenario: Tools, tabs and rows
- **WHEN** the rail's buttons, the tabs, a row and a row twice, a layer checkbox, the search box, and the kind filter are clicked
- **THEN** the tool, select, fly and hide events come out, the tabs switch without an event, the results replace the nearby list while a query or filter is set

#### Scenario: Typed values and buttons
- **WHEN** a parameter field and the name field are focused and left unchanged, then given new text and Enter; the buttons are pressed with and without something to undo; the counts line is built
- **THEN** an unchanged field asks for nothing, the typed text is asked for once, a disabled button asks for nothing, the other buttons ask for their events, and the counts line has the edit count, the undo depth and where the edits are saved

#### Scenario: The overlay
- **WHEN** the overlay of a ring, a ring with gaps, a selected edit with handles and an edit with nothing on the screen is drawn at one and at one and a half pixels per point
- **THEN** outlines, names and handles are drawn, gaps are not joined, and an edit with nothing on the screen draws nothing

### Requirement: VIEW-022 The editor mode of the window
`planet editor` SHALL have an editor mode (`planet::editwin`, with `window`) that edits the world's definition through the session (VIEW-017) and the panels (VIEW-021).
- Tab turns the mode on and off; `--edit` starts the window with it on.
- `--items DIR` is the directory the edits are written to and loaded from: one TOML file per edit (AUTH-001), every command writes its file; without it the edits live in memory and the status line says they are not saved.
- `--editor-layout F` is the layout file (VIEW-018).
- `--edit` with `--no-ui` is an error that says the mode needs the UI.
- Turning the mode on switches to the free camera and ends the globe overview; while the mode is on C and G do nothing and the left button does not capture the pointer.
- The left button edits: a press in the world presses, any release releases, and a press while the pointer is captured does nothing.
- The right button held looks around (VIEW-010); it waits while the left button holds a handle.
- Esc abandons a drag, else leaves the stamp tool, else deselects, and does not quit while the mode is on (VIEW-003).
- Keys go to a text field with the focus.
- Turning the mode off abandons a drag; its places are still named for the refresh.
- The mode finds the shown edits every frame by the footprint find over the tiles the plan wants (VIEW-015), draws their outlines, names and handles (VIEW-020), lists them (VIEW-016), and names the resident tiles to refresh after each command (VIEW-019).
- `--smoke --edit` runs a script of the mode after the overview phases (VIEW-005): the mode on, a hill placed where the view meets the surface, its move handle dragged, a radius typed, undo, redo, delete, undo; the run fails unless all of it happened and nothing was refused, and the summary reports the counts.

The keys of the mode, taken by the letter on the key where a letter is meant (so on a QWERTZ keyboard the key labelled Z undoes):

| Key | Action |
|---|---|
| `1` | select tool |
| `2`, `3`, `4` | crater, hill, plateau tool |
| Ctrl+Z | undo |
| Ctrl+Y, Ctrl+Shift+Z | redo |
| Delete, Backspace | delete the selected edit |
| Esc | step back (above) |

Fly to puts the free camera south of the edit looking at it, pitched down:

| | Distance |
|---|---|
| Back, in outline radii (at least 150 m) | 2.5 |
| Up, in outline radii (at least 80 m) | 1.2 |

Verify: A, C
Status: active
Source: owner (M6 step U3, 2026-10-19), VIEW-003, VIEW-010, VIEW-011, VIEW-017, VIEW-019, AUTH-001

#### Scenario: The mode on and off
- **WHEN** keys and presses come while the mode is off, the mode is turned on, a stamp is placed, its move handle is held and dragged, and the mode is turned off
- **THEN** nothing is placed, counted or drawn while it is off, and turning it off abandons the drag and goes back, recording only the placement

#### Scenario: A whole session of edits
- **WHEN** a hill is placed with the keys and a click, its move handle dragged 60 pixels, a radius typed, a radius out of range typed, and undo, redo, delete and many undos pressed
- **THEN** each step is one command with the counts and places to refresh, the drag moves the item the metres 60 pixels span, the refused value says why, and the undos reach an empty world

#### Scenario: Selecting, Esc and fly
- **WHEN** a press lands on an edit, the cursor is over a panel, and Esc is pressed with a drag held, with the stamp tool on, and with a selection; and Fly to is asked
- **THEN** the edit is selected and hovered, there is no hover over a panel, Esc steps back one thing each time, and the camera pose looks at the edit from the distances above

#### Scenario: The keys
- **WHEN** the keys of the table are pressed on a QWERTY and on a QWERTZ keyboard, with and without Ctrl, and the fly keys W A S D Q E, Space, G and C
- **THEN** each key does its action by the letter on it, no letter is a command without Ctrl, and the fly keys are not the editor's

#### Scenario: Files and search
- **WHEN** a stamp is placed and a value typed with `--items`, and the directory is opened again; and a search for an edit out of view
- **THEN** one file named for the edit's id holds it, the reopened session has the same definition hash, and the search finds the far edit while the nearby list holds only the shown ones

#### Scenario: The options
- **WHEN** `--edit` is given with `--no-ui`
- **THEN** the run fails before it opens anything and says the mode needs the UI

#### Scenario: The real window
- **WHEN** `planet editor --smoke --edit --items DIR` runs on the selected adapter
- **THEN** the run succeeds with one edit placed, one drag, one typed value, one delete, two undos and one redo and nothing refused, tiles sent to refresh, and one item file in DIR
