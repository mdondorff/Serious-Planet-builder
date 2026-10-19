## MODIFIED Requirements

### Requirement: VIEW-022 The editor mode of the window
`planet editor` SHALL have an editor mode (`planet::editwin`, with `window`) that edits the world's definition through the session (VIEW-017) and the panels (VIEW-021).
- Tab turns the mode on and off; `--edit` starts the window with it on.
- `--items DIR` is the directory the edits are written to and loaded from: one TOML file per edit (AUTH-001), every command writes its file; without it the edits live in memory and the status line says they are not saved.
- `--editor-layout F` is the layout file (VIEW-018).
- `--edit` with `--no-ui` is an error that says the mode needs the UI.
- Turning the mode on switches to the free camera and ends the globe overview (once: G shows it again); while the mode is on C does nothing and the left button does not capture the pointer.
- The globe overview (VIEW-013) shows a marker for each edit the camera shows (VIEW-015 over the tiles the last plan wanted, at the overview's camera, so from high up only the large edits and the selected one), numbered from 1,000,000, sized by the outline radius and labelled by the edit's name; this holds with the mode off as well.
- A click on an edit's marker selects the edit and starts the flight to it as for any marker.
- The left button edits: a press in the world presses, any release releases, and a press while the pointer is captured does nothing.
- The right button held looks around (VIEW-010); it waits while the left button holds a handle.
- Esc abandons a drag, else leaves the stamp tool, else deselects, and does not quit while the mode is on (VIEW-003).
- Keys go to a text field with the focus.
- Turning the mode off abandons a drag; its places are still named for the refresh.
- The mode finds the shown edits every frame by the footprint find over the tiles the plan wants (VIEW-015), draws their outlines, names and handles (VIEW-020), lists them (VIEW-016), and names the resident tiles to refresh after each command (VIEW-019).
- Before the frame plans, a changed definition is handed to the generator (`WorldTerrain::set_definition`, STRM-028) and then the named tiles are refreshed (`ManagedLoop::edit`), so the terrain follows the edits; a definition the generator refuses is a refused count and a status line. Items loaded at the start refresh every resident tile.
- After a hand-over the plan's height bound is the new field's maximum (which counts the stamps' deltas) when that is above the configured bound, so a stamp on the highest peak does not fail the plan; a run with no edits plans as it did.
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

#### Scenario: The overview
- **WHEN** a plateau of reach 15 km and a hill of 400 m are seen from 200 km up, the hill is selected, nothing is in view, the plateau is hidden, a marker is clicked and a number that is no edit's is clicked, with the mode off
- **THEN** only the plateau is a marker, then the selected hill too (the markers follow the order of the ids and carry the names), a hidden edit is none unless selected, the click selects the edit, the other number changes nothing, and the markers come with the mode off

#### Scenario: The options
- **WHEN** `--edit` is given with `--no-ui`
- **THEN** the run fails before it opens anything and says the mode needs the UI

#### Scenario: The real window
- **WHEN** `planet editor --smoke --edit --items DIR` runs on the selected adapter
- **THEN** the run succeeds with one edit placed, one drag, one typed value, one delete, two undos and one redo and nothing refused, definitions handed to the generator, tiles sent to refresh, and one item file in DIR
