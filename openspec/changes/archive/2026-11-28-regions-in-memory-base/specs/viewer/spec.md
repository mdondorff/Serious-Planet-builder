## MODIFIED Requirements

### Requirement: VIEW-022 The editor mode of the window
`planet editor` SHALL have an editor mode (`planet::editwin`, with `window`) that edits the world's definition through the session (VIEW-017) and the panels (VIEW-021).
- Tab toggles the mode; `--edit`, `--regions` or a regions file beside `--world` start it (`--site`: after descent).
- `--items DIR` is the directory the edits are written to and loaded from: one TOML file per edit (AUTH-001), every command writes its file; without it the edits live in memory and the status line says they are not saved.
- `--editor-layout F` is the layout file (VIEW-018).
- `--edit` with `--no-ui` is refused: the mode needs the UI.
- Turning the mode on switches to the free camera and ends the globe overview (once: G shows it again); while the mode is on C does nothing and the left button does not capture the pointer.
- The globe overview (VIEW-013) shows a marker for each edit the camera shows (VIEW-015 over the tiles the last plan wanted, at the overview's camera, so from high up only the large edits and the selected one), numbered from 1,000,000, sized by the outline radius and labelled by the edit's name; this holds with the mode off as well.
- A click on an edit's marker selects the edit and starts the flight to it as for any marker.
- The left button edits: a press in the world presses, any release releases, and a press while the pointer is captured does nothing.
- The right button held looks around (VIEW-010); it waits while the left button holds a handle.
- Esc abandons a drag, else leaves the stamp tool, else deselects, and does not quit while the mode is on (VIEW-003).
- Keys go to a text field with the focus.
- While the pointer is captured egui is given no pointer event, so when a capture starts and when it ends every button is let go for egui (a move far from the cursor, then the buttons up: no click, no button left down).
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
| `5`, `6`, `7` | ridge, channel, region tool (VIEW-029, VIEW-031) |
| `8`, `9`, `0`, `-` | brush: raise, lower, smooth, flatten (VIEW-035) |
| Enter | finish the line or region being drawn |
| Backspace | take the last point of the line or region being drawn back; it never deletes an edit |
| Ctrl+Z | undo |
| Ctrl+Y, Ctrl+Shift+Z | redo |
| Delete | delete the selected edit |
| F1, `?` | the list of keys |
| every key above, the viewer's keys (VIEW-003) and the free camera's | named in `EDIT_KEYS_HELP`, the usage text of the mode; a test over the key tables fails when a bound key is not named |
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

#### Scenario: A look-around leaves no button down
- **WHEN** egui sees the right button go down, no event follows for a while, and then the release events of a capture are given, twice
- **THEN** the button stays down for egui until the release events, which let it go without a click, and given again they do nothing

#### Scenario: The overview and the mode
- **WHEN** the mode is turned on with the overview shown and with none, and G shows the overview in the mode
- **THEN** the overview ends once on the turn-on and the free camera is taken, and the overview shown with G stays

#### Scenario: The overview
- **WHEN** a plateau of reach 15 km and a hill of 400 m are seen from 200 km up, the hill is selected, nothing is in view, the plateau is hidden, a marker is clicked and a number that is no edit's is clicked, with the mode off
- **THEN** only the plateau is a marker, then the selected hill too (the markers follow the order of the ids and carry the names), a hidden edit is none unless selected, the click selects the edit, the other number changes nothing, and the markers come with the mode off

#### Scenario: The options
- **WHEN** `--edit` is given with `--no-ui`
- **THEN** the run fails before it opens anything and says the mode needs the UI

#### Scenario: The real window
- **WHEN** `planet editor --smoke --edit --items DIR` runs on the selected adapter
- **THEN** the run succeeds with one edit placed, one drag, one typed value, one delete, two undos and one redo and nothing refused, definitions handed to the generator, tiles sent to refresh, and one item file in DIR

#### Scenario: Site descent
- **WHEN** the window starts with `--edit --site LAT,LON`
- **THEN** the scripted flight starts high above the site and descends over it with the mode off, the mode turns on where the last camera is reached (or at a Tab before then), and the free camera takes over there

### Requirement: VIEW-052 The editor opens the production store
Without `--store`, `--commit`, `--regions`, `--world` or `--global-res`, outside a smoke run, the editor SHALL open the first existing directory of: `PLANET_STORE`, the pointer file `target/stores/default`'s, `target/stores/global16384`, `target/accept/global16384` (under the repo root); with none, the in-memory preview bake.
Outside a smoke run, `--regions` or `--world` SHALL compute the class-source base in memory (1,024 cells, else `--global-res N`).
An unreadable default store SHALL be refused naming `--commit DIR`, `--global-res N` and `cargo xtask store`.
The title SHALL name the source: `store <dir>` or `preview bake <N> cells`.
`cargo xtask store [--res N] [--relief-scale S] [--link DIR]` SHALL build a store of N cells a face edge (default 16384) into `target/stores/global<N>` and write the pointer file; `--link` only points to an existing store.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner): the editor showed a flat world because it ran on the 256-cell preview bake

#### Scenario: The order
- **WHEN** a root holds none, then an accept store, a stores store, a pointer file and an environment variable, in that order
- **THEN** the choice is none, the accept store, the stores store, the pointer's, the variable's, and a variable naming no directory falls through

### Requirement: VIEW-069 The viewer and capture-stills open a commit over the in-memory production base
With `--commit DIR` the viewer and `capture-stills` SHALL open the commit over the production base computed in memory for the definition: exact base values over its tiles and one tile of margin, the in-memory bake elsewhere. The commit SHALL pass the normal key check. `--store`, `--erosion`, `--erosion-store`, `--global-res`, `--allow-memory-bake` and the source flags SHALL be refused beside it. The log, the title and every still SHALL say "base: computed in memory, not read from a stored bake". The viewer SHALL start held still 25 km over the ground, 20 km south of `--site` or the centre of the commit's tiles, looking north 50 degrees down; the log SHALL say "inside the region" or "outside the region: coarse base".

Verify: A
Status: active
Source: Lead 2026-11-13 (E1, option b; E4 is the owner flying it), TEST-128, REND-058

#### Scenario: Refusals
- **WHEN** `--commit` is given with a stored world, the erosion layer, a bake size, or to another scene
- **THEN** the run is refused naming the clashing options

#### Scenario: Another seed
- **WHEN** a commit baked for seed 3 is opened for seed 4
- **THEN** it is refused before any base is computed

#### Scenario: Start over the region
- **WHEN** the viewer opens a commit, without `--site` and with it
- **THEN** the camera starts over the centre of the commit's tiles or the site, the log says "inside the region", and over the default site it would say "outside the region: coarse base"

### Requirement: VIEW-077 The editor places, resizes and deletes region boxes and reopens a baked one
`planet editor` SHALL edit a regions file's boxes (WDEF-019): `--regions FILE`, else the `regions.toml` beside `--world F`.
The Sites tab SHALL list each box with its derived state.
Place and Move SHALL put a box of 2 to 4 L tiles a side where a click hits the ground, snapped to whole tiles and clamped into the allowed tiles; Resize and Delete SHALL change it.
Every box put SHALL pass `check_box`; a refused one leaves the file unchanged.
The file SHALL be rewritten after each change, outside `WorldDefinition`.
The window SHALL draw the boxes by state, the placement box and the face-edge margin on the ground.
Open on a `ready` box SHALL copy its `--commit` command to the clipboard.

Verify: A
Status: active
Source: owner 2026-11-28 (S-Region-Authoring E2), WDEF-019, BUILD-078, VIEW-052, VIEW-069, VIEW-075

#### Scenario: Placing at a face edge
- **WHEN** a 2x2 box is placed with the cursor on the first tile of a face
- **THEN** the box sits at the first allowed tile, passes `check_box`, is in the reread file, and the world's hash is unchanged

#### Scenario: Resize, move, delete and refusals
- **WHEN** the box is widened to 4 and once more, moved, placed again under a taken name, and deleted
- **THEN** the side stops at 4 with the reason in the status, the move snaps to the new tile, the taken name is refused, and the file ends empty

#### Scenario: State and reopen
- **WHEN** a box's commit is baked, the definition then changes, and Open is pressed on the ready box
- **THEN** the row reads `unbaked`, `ready`, `stale` in turn, and Open copies and prints the `--commit` command of its folder

#### Scenario: No default store for region boxes
- **WHEN** `editor --seed 14 --regions F` runs with a default store on disk
- **THEN** no store is opened and the base is computed in memory (VIEW-052)

#### Scenario: The default store's refusal
- **WHEN** a default store cannot be read
- **THEN** the run is refused naming `--commit DIR`, `--global-res N` and `cargo xtask store` (VIEW-052)
