## ADDED Requirements

### Requirement: VIEW-015 What the camera shows of the edits
The editor SHALL decide which edits it shows in markers, outlines and the list of nearby edits from the camera (`planet_editor::relevance`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14):
- an edit is shown when its footprint meets the visible area and its reach covers at least `MIN_REACH_PX` = 8 pixels on the screen;
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
- **WHEN** a hill (reach 500 m) and a plateau (reach 18.75 km) at one place are seen from 2 km, from 100 km, and from the heights where the hill covers 7.9 and 8.1 pixels
- **THEN** both show at 2 km, only the plateau at 100 km, and the hill shows at 8.1 pixels and not at 7.9

#### Scenario: The footprint and the selection
- **WHEN** two large plateaus lie on two sides of the planet and one tile is visible, then no tile is visible, then the far one is selected, then a small hill outside the tiles is selected from 2,000 km up, then the near plateau is hidden
- **THEN** only the plateau in the tile shows, nothing shows without a tile, the selected edit is added each time, and a hidden edit shows only while selected

#### Scenario: Not a scan
- **WHEN** forty large plateaus 330 to 700 km away, each covering more than 8 pixels from 30 km up, lie outside the visible tile
- **THEN** none of them shows, so the tiles and not the pixel test kept them out

### Requirement: VIEW-016 Lists and search of the edits
The editor SHALL list the edits as plain data (`planet_editor::lists`, GPU-free); a row holds the id, a label, kind, layer, distance from the camera and reach.
- The label of an edit is its name, trimmed, else its kind and the last six characters of its id.
- The distance is that from the camera to the anchor's point on the sphere.
- The nearby list holds the edits the camera shows (VIEW-015), nearest first, ties by id.
- The search runs over every edit of the body, not only the shown ones: every word of the query is part of the label, kind, layer or id text, in any case; an empty query matches all; a kind filter restricts to one kind; the rows are nearest first, ties by id.
- The order the edits apply in is the layer's place in the layer stack, then priority, then id (ADR 0007, ADR 0066 point 4); the stack bottom to top is `base`, `imported`, `intent`, `stamps`, `splines`, `placed`, `frozen`, and a layer outside it applies after these, by name.
- The layers list groups by layer in stack order, each group in the order the edits apply; the chain is all edits in that order.

Verify: A
Status: active
Source: owner (M6 step U3, search planet-wide sorted by distance from the camera 2026-10-19), VIEW-015, ADR 0007, ADR 0066

#### Scenario: Nearby
- **WHEN** three named edits are listed from a camera near the second, with an id outside the set and two edits at the same distance
- **THEN** the rows are nearest first, an id with no edit is left out, and equal distances order by id

#### Scenario: Search
- **WHEN** the query is `hill`, `HILL far`, empty, a kind filter, a text that matches nothing, and the last six characters of an unnamed edit's id, and a name with spaces around it
- **THEN** the far hill is found after the near one, every word must match in any case, all edits are listed for the empty query, the filter keeps one kind, nothing matches the unknown text, the unnamed edit is found with the label `kind` plus those characters, and the padded name is shown trimmed

#### Scenario: Layers and order
- **WHEN** two edits of priorities 5 and -1 in `stamps`, one in `splines` (also at priority -100) and one in a layer outside the stack are listed
- **THEN** the groups come in the order `stamps`, `splines`, the outside layer, the `stamps` rows by priority, and the chain is the same order whatever the priorities of the higher layers

### Requirement: VIEW-017 Selecting, handles and typed values
The editor SHALL select, place, delete, drag and type values through one session over the E4 commands (`planet_editor::session`, with `shape` and `params`; GPU-free, f64 on the CPU, `libm` only), so the flows are tested without a window.

Shape and handles:
- The outline of an edit is its `radius_m` parameter when positive, else its reach.
- The handles are the anchor (always), the radius (when it has an outline) and the height (when it has a finite `height_m`).
- A handle sits on the anchor, on the outline along the item's heading (0 north, 90 east), or above the anchor by the height.
- At a pole, north is the limit along longitude 0: -x at the north pole, +x at the south pole.
- A direction becomes latitude and longitude in the spelling the item file takes: longitude in (-180, 180], 0 at a pole; latitude by `atan2` of the height over the distance from the axis.

Drags (each result is a valid item and the bounds hold it):
- move puts the anchor under the pointer;
- resize sets `radius_m` to the arc from the anchor to the pointer and grows the reach to 1.25 times the radius (at most one cell edge; the reach never shrinks);
- raise adds to `height_m` and grows the bound on the change to hold it (never shrinks, at most `MAX_DELTA_LIMIT_M`);
- the radius and the height stay in the kind's range from the parameter table, the range a typed value is held to; a kind the table does not know keeps the radius within 1 m and one cell edge and the height within the limit on the change;
- a pixel is `2 d tan(fov_y / 2) / height_px` metres at a point whose depth along the view axis is `d` (at least 1 m), and a drag up raises.

Commands, each one E4 command:
- a whole drag, `begin_drag` to `end_drag`, whatever the number of pointer moves; `cancel_drag` records nothing; while a handle is held no other command, undo, redo or change of selection runs;
- placing a stamp (the kind's defaults from the table, in the layer `stamps`, reach 1.25 times the radius, the bound the largest metric parameter other than the radius), deleting, renaming and a typed value;
- a place that is not a finite non-zero direction is refused.

Typed values:
- the text is read in the parameter's unit (`450`, `2.4 km`, `40 %`);
- a value out of the kind's range, not a number, in another unit, or of an unknown parameter changes nothing and says why;
- where the table has no range the item file's limits hold: a radius within 0 and one cell edge, any other metric parameter within the limit on the change;
- the reach and the bound only grow to hold the value.

Picking and inspection:
- the item under the pointer is the nearest in units of its own outline radius among the shown ones (`pick_footprint`); an item that is not shown cannot be picked;
- the inspector is the selected edit's values as plain data: kind, layer, place in the order of its layer, distance, reach and the parameters in the table's order with their ranges, then the others by name without a range;
- lengths read `450 m` below 1 km (after rounding to a tenth of a metre) and `2.4 km` above.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, typed metric input the fallback, 2026-10-19), AUTH-004, VIEW-012

#### Scenario: Outline and handles
- **WHEN** an item with a radius and a height, one without a height, one with neither and no reach, and one with a radius that is not positive are examined, and the handles are placed at headings 0, 90 and 217
- **THEN** the handle lists are move, radius, height; move, radius; move; the outline falls back to the reach, the radius handle is 400 m from the anchor within 1 mm along the heading, the height handle is the height above the anchor, and heading 90 is east

#### Scenario: One spelling and the poles
- **WHEN** directions at ordinary places, 89.9999 degrees north, the poles, the antimeridian and negative zero are turned into latitude and longitude, put through `Item::validate`, and the north of a pole is compared with the north a micro-degree away
- **THEN** they equal the input within 1e-9 degrees, a pole has longitude 0, the antimeridian is +180, every result is a valid item, and the north does not jump at either pole

#### Scenario: Drag arithmetic
- **WHEN** a hill is moved, resized to 1,500 m, 100 m, 0 m and 90 km, raised by 40 m, -200 m and a billion metres, and a kind the table does not know is resized and raised the same way
- **THEN** the anchor is under the pointer, the radius is the arc within 1 mm and the reach is 1.25 times it without shrinking, the hill stays within 50 m to 2 km and 5 m to 400 m with its bound growing, the unknown kind stays within 1 m to one cell edge and within the limit with its bound holding the height, a pixel at 1 km is `2 d tan(fov_y / 2) / height_px` metres, and a drag up raises

#### Scenario: Picking
- **WHEN** a small hill lies inside a large plateau, the pointer is on the hill's centre, 5 km away, far away, and the hill or both are left out of the shown set or an id has no item
- **THEN** the hill is under the pointer at its centre, the plateau 5 km away, nothing far away, and an edit that is not shown or not an item is not picked

#### Scenario: Placing and deleting
- **WHEN** craters are placed, an edit is deleted and undone and redone, and everything is undone
- **THEN** each placement is one selected command with the kind's defaults and a written file, a delete deselects, and the undos reach the exact starting hash and clear a selection whose edit is gone

#### Scenario: Typed values
- **WHEN** `3.5 km`, `300`, `20 km`, `10`, `-5`, `abc`, an empty text, `5 kg`, an unknown parameter and `NaN` are typed, and over-limit values for a kind the table does not know
- **THEN** the valid values are one command each in their unit with the reach and bound following, every refused value changes nothing, and the stored item keeps its outline within its reach and its height within its bound

#### Scenario: Drags are one command
- **WHEN** a hundred pointer moves of a position handle end, then radius and height drags end, a drag is cancelled, and a place that is zero, not a number or infinite is given to a drag and to a placement, and another edit is selected during a drag
- **THEN** each drag is one undo step and three undos reach the exact starting hash, the cancelled drag changes nothing and records nothing, the bad places are refused, and the selection does not change while the handle is held

#### Scenario: Inspector and units
- **WHEN** an edit with table parameters and two other parameters is inspected, and lengths are typed and formatted
- **THEN** the kind, layer, `47.20000° N`, `8.55000° E`, `2.4 km`, `1 of 2 in its layer` and `500 m` show, the parameters come in table order with their ranges and the others by name without a range, `450`, `450 m`, `2.4 km` and `40 %` are read in their units, empty text, words, a wrong unit, exponent notation and non-finite numbers are refused with the reason, and 999.96 m reads `1 km`

### Requirement: VIEW-018 The layout of the panels is data
The layout of the editor panels SHALL be a value read from a file of keys, not code (`planet_editor::layout`, GPU-free); a key the file does not name keeps its default. A key, its unit and range:

| Key | Default | Range |
|---|---|---|
| `rail_width` (points; 0 hides the rail) | 56 | 0 to 200 |
| `left_width` (points) | 300 | 120 to 800 |
| `inspector_width` (points) | 320 | 120 to 800 |
| `inspector_side` | `right` | `left` or `right` |
| `tabs` (the left panel's tabs in order) | `near`, `layers`, `graph` | at least one, none twice |
| `status_bar` | true | true or false |
| `handle_hit_px` (pixels around a handle in which a press takes it) | 10 | 3 to 60 |

An unknown key, a value out of its range, a wrong type, a bad tab list or a file that does not parse is refused naming the key (`(file)` for a file that does not parse).

Verify: A
Status: active
Source: owner (M6 step U3: layout data-driven and cheap to change, 2026-10-19), VIEW-011

#### Scenario: Defaults and overrides
- **WHEN** an empty file and a file naming some keys are read
- **THEN** the first is the default and the second changes only the named keys

#### Scenario: Refusals
- **WHEN** a file has an unknown key, a width out of range, a string for a number, `nan`, a bad side, an empty, repeated, unknown or mistyped tab list, or a syntax error
- **THEN** each is refused with the key named
