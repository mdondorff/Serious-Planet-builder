## MODIFIED Requirements

### Requirement: VIEW-015 What the camera shows of the edits
The editor SHALL decide which edits it shows in markers, outlines and the list of nearby edits from the camera (`planet_editor::relevance`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14):
- an edit is shown when its footprint meets the visible area and its reach covers at least `MIN_REACH_PX` = 8 pixels on the screen; the footprint of a brush holding strokes is where a stroke reaches (WDEF-007), its size the reach disc's;
- the selected edit is always shown, whatever its size or place;
- an edit the user hid is left out unless it is selected.

The footprint part is the footprint find (WDEF-007, `WorldDefinition::footprint_items`, halo 0) over the tiles of the visible area, one find per tile and layer, never a scan of all items. The size is `reach_m / d * (height_px / 2) / tan(fov_y / 2)` with `d` the distance from the camera to the anchor's point on the sphere; a camera on the point gives infinity. So near the ground small stamps show with the larger edits around them, and from high up the small stamps drop out.

Verify: A
Status: active
Source: owner (M6 step U3, relevance rule 2026-10-19; hiding is the layer list's checkbox in the mock), WDEF-007, VIEW-012

#### Scenario: The size on the screen
- **WHEN** a reach of 100 m is seen from 1,000 m and 2,000 m up with a 60 degree field of view over 1,000 pixels, doubled, and from a camera on the point
- **THEN** the sizes are `0.1 * 500 / tan(30 degrees)` pixels, half of that, twice that, and infinity

#### Scenario: Near the ground and from high up
- **WHEN** a hill (reach 400 m) and a plateau (reach 15 km) at one place are seen from 2 km, from 100 km, and from the heights where the hill covers 7.9 and 8.1 pixels
- **THEN** both show at 2 km, only the plateau at 100 km, and the hill shows at 8.1 pixels and not at 7.9

#### Scenario: The footprint and the selection
- **WHEN** two large plateaus lie on two sides of the planet and one tile is visible, then no tile is visible, then the far one is selected, then a small hill outside the tiles is selected from 2,000 km up, then the near plateau is hidden
- **THEN** only the plateau in the tile shows, nothing shows without a tile, the selected edit is added each time, and a hidden edit shows only while selected

#### Scenario: Not a scan
- **WHEN** forty large plateaus 330 to 700 km away, each covering more than 8 pixels from 30 km up, lie outside the visible tile
- **THEN** none of them shows, so the tiles and not the pixel test kept them out

### Requirement: VIEW-019 What the pointer does on the planet
The editor SHALL turn the pointer into session calls in one place (`planet_editor::interaction`, GPU-free, f64 on the CPU), from plain data the window resolves: the cursor, the surface hit, the shown edits and the handles on the screen.
- A press takes a handle of the selected edit when the cursor is within the hit radius of it (`handle_hit_px` of the layout, VIEW-018; the nearest wins, the radius inclusive).
- Else, with the stamp tool and the cursor on the surface, the press places a stamp (selected, the tool back to select); with the stamp tool and the cursor off the planet it does nothing and the tool stays.
- Else the press selects the shown edit under the cursor (VIEW-017), or clears the selection when there is none.
- While a handle is held a move makes the item the pointer says (position and radius follow the surface hit and stay put without one; the height follows the vertical move of the cursor in pixels times the metres per pixel at the handle, up raises); the release ends the drag as one command; an abandoned drag goes back and records nothing.
- The hover is the shown edit under the cursor; it is none over a handle and does not change while a handle is held.

The tiles an edit sends to refresh are the resident tiles whose area with a halo of 0.25 tile edges a reach of the places the commands changed can touch (`planet_editor::feed`, the footprint touch test with its margins: false positives only cost a refresh); a place that is a brush item holding strokes touches a tile only when one of its strokes does as well (WDEF-007), so a stroke names the tiles along it and not the reach disc round its first point; they come in tile order without repeats, and the places of another body count for none.

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
- **THEN** a handful of tiles are found including those of points 4.9 km away in every direction and the tile of the centre, none far away, both places count, another body counts for none, a larger reach finds more, and empty lists find nothing; a brush stroke 712 m long names the tiles along it and fewer than half the tiles of its reach disc at level 17
