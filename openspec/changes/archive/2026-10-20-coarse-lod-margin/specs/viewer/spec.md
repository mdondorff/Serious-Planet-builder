## MODIFIED Requirements

### Requirement: VIEW-014 Debug overlay and coarse LOD view
The editor window SHALL be able to draw a debug overlay over the lit terrain so that the real mesh density and the levels of detail are visible, and to show the mesh at a coarser level of detail for inspection. F3 cycles the overlay (off, tiles, mesh, regions, off; the regions overlay is VIEW-030) and F4 toggles the coarse LOD; both are off at the start; both act only in `planet editor` and on the lit path (the headless modes, `test-render` and the golden paths never set them, so no golden moves). The overlay is plan data (CON-03): `planet_frame::look::Overlay` (`None`, `Tiles`, `Mesh`, `Regions`; further debug overlays plug into the same enum; `ALL` and `next` give the cycle, `shader_code` 0, 1, 2, 3) is a field of `LitPlan` (`overlay`), `LitPlan::with_overlay` sets it, and the plan text and hash carry it only when it is not `None` (` overlay tiles`, ` overlay mesh` or ` overlay regions` after the other tokens, parsed back and validated). The renderer does not branch on a mode of its own: the plan's value reaches the lit fragment stages through the globals (`extra.y`) and the node record (`morph.w` carries the tile's level). The fragment form needs no wgpu feature, so it runs on WARP, lavapipe and the discrete GPU: from the compact tile's grid coordinates it draws, in `Tiles`, a faint tint of the level's colour and the tile's outline, and in `Mesh` also a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (the cell's `a` to `c`, as `grid_indices`), a fixed line that does not fade out however fine the mesh is. The colour is one of eight per level, wrapping; the brightness falls with the vertex morph factor (full detail bright, fully morphed on the parent grid 0.3). The lines are radiance in scene units (the pre-exposure factor of the frame times 0.8), so they show at any exposure. With the overlay `None` the frame is the lit frame bit for bit. The coarse LOD multiplies the LOD error bound `tau_px` by 4 (`COARSE_LOD_FACTOR`), or by the largest factor below 4 that keeps `LodParams::validate` satisfied (`LodParams::coarse`, `coarse_factor`; the production 1080p parameters, 6 px and 32 cells, allow about 1.7, VIEW-032), in the request the frame is planned with (`effective_lod`; terrain and water alike), so the mesh edges are about that many times longer on the screen: it changes the view, not the data, and is decided in the GPU-free plan, never in a shader. The coarser bound can fail on steep terrain (the plan worker reports a tile whose morph interval has no room): the frame then falls back to the base parameters, the switch turns itself off (F4 asks again) and the smoke summary counts the refusal; the window never stops for it. The editor's corner label shows the state (`overlay_status`: `Overlay: Mesh, LOD x4`, `Overlay: Tiles, LOD x2.6`, `Overlay: Regions, LOD x1`, `Overlay: off, LOD x1`; the factor is the one the plan uses).

Verify: A, C
Status: active
Source: owner (2026-10-19, via opus-planner), CON-03, CON-18, VIEW-011, VIEW-030

#### Scenario: The plan carries the overlay
- **WHEN** a lit plan is made, given tiles, mesh or regions, written as text, parsed back, and given an unknown overlay name
- **THEN** it has no overlay by default, the text of a plan without one has no overlay token (hashes are as before), the overlays add ` overlay tiles`, ` overlay mesh` and ` overlay regions`, the plan round-trips with its other tokens, and the unknown name is an error naming it

#### Scenario: F3 and F4
- **WHEN** F3 is pressed four times and F4 twice in the editor, in the lit and debug paths
- **THEN** the overlay goes tiles, mesh, regions, off (off at the start, kept when the debug path is shown), the coarse LOD turns on and off independently, the keys help names both, the coarse parameters are the base ones with only `tau_px` changed, times 4 where that is valid and else the largest valid factor (always valid, the production 1080p ones about 1.7), the corner label reads `Overlay: Mesh, LOD x4`, `Overlay: Regions, LOD x1` or `Overlay: off, LOD x1`, and the window smoke counts lit frames drawn with an overlay, with the mesh, and frames planned with the coarse LOD

#### Scenario: A refused coarse plan
- **WHEN** the plan with the coarse LOD fails and the base one works, when the coarse view is off and the plan fails, when both fail, and when the coarse plan works
- **THEN** the first uses the base plan, turns the switch off and counts one refusal, the second returns its error at once with no second attempt, the third returns the second error and counts one refusal, and the fourth changes nothing

#### Scenario: The overlays change the terrain's pixels only
- **WHEN** lit frames of a close view, a mid view and the orbit view are drawn on the software adapter with the overlay none, tiles and mesh
- **THEN** the none frame equals the plain lit frame, the mesh frame is deterministic, no pixel outside the planet changes, a close view has at least 50 changed terrain pixels in each overlay and the mesh differs from the tiles, and inspection images (production LOD and a coarse one, three overlays each) are written to `target/review/overlay-inspection/`

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
| Enter | finish the line or region being drawn |
| Backspace | take the last point of the line or region being drawn back; it never deletes an edit |
| Ctrl+Z | undo |
| Ctrl+Y, Ctrl+Shift+Z | redo |
| Delete | delete the selected edit |
| F1, `?` | the list of keys |
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

## ADDED Requirements

### Requirement: VIEW-032 The coarse view keeps clear of the plan's refusal at the large levels, and a late refusal does not end the window
`LodParams::coarse` SHALL return the base parameters with a larger `tau_px` such that the morph interval of every level keeps its 5 % of the end distance plus a margin of 1 % (the node-radius rule, as `LodParams::validate` has it), and such that at every level whose node radius is at least half of `max_height_m` (`COARSE_WORST_CASE_RADIUS_FACTOR`) where the base parameters meet the plan's own start (`split(l) + 2 x extent(r (1 + H / R), H)`, `LodParams::morph_interval_worst`) the coarse ones meet it too with the same margin. Over a field that has only a global height bound this leaves the coarse view refused, where the base view is accepted, at no more than one pose in twenty-five of a sweep from 20 km altitude up (measured 0 % for the window's 320x200 and the 16-cell parameters, 1.4 to 2.7 % for the production 1080p parameters, all at level 11 between 31 and 39 km); a refusal that remains falls back to the base parameters as VIEW-014 says. `LodParams::validate` SHALL stay the node-radius rule. The production 1080p coarse factor is about 1.7 times the error bound (the window's 320x200 parameters about 2.2). A plan failure whose request carried other LOD parameters than the frame that receives it SHALL be dropped and counted (`PlanPipeline::dropped_plan_errors`), not returned; a failure of the frame's own request SHALL be returned as before.

Verify: A
Status: active
Source: opus-planner (2026-10-20), SPIKE-19, VIEW-014, LOD-003, LOD-011

#### Scenario: The window's coarse view over the eroded world's bound
- **WHEN** the window's LOD at 320x200 over a field whose global height bound is 9,735 m is made coarse and planned from five orbital poses, and again from 40 altitudes between 20 km and 3,400 km at three poses (a face centre, a cube corner and a generic direction) at 320x200 and at 1920x1080
- **THEN** the plan accepts the five orbital poses (it refused them all with the former rule), and refuses the coarse LOD at no more than one pose in twenty-five among those where it accepts the base LOD

#### Scenario: A late refusal
- **WHEN** the plan of parameters too coarse for 16 cells is asked for and the frame then asks again with the base parameters, and later asks for the coarse parameters themselves
- **THEN** the first refusal is dropped and counted while the frame draws, and the refusal of the frame's own parameters is returned
