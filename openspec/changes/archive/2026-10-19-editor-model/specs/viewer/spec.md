## ADDED Requirements

### Requirement: VIEW-015 What the camera shows of the edits
The editor SHALL show an edit in its markers, outlines and list of nearby edits only when the edit's footprint meets the visible area and its reach covers at least `MIN_REACH_PX` = 8 pixels on the screen, and SHALL always show the selected edit (`planet_editor::relevance`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14). The footprint part is the footprint find (WDEF-007, `WorldDefinition::footprint_items`) over the tiles of the visible area, one find per tile and layer, never a scan of all items. The size is the reach over the distance from the camera to the anchor's point on the sphere, in the units of the vertical field of view: `reach_m / d * (height_px / 2) / tan(fov_y / 2)`; a camera on the point gives infinity. An edit the user hid is left out unless it is selected. So near the ground small stamps show with the larger edits around them, and from high up the small stamps drop out.

Verify: A
Status: active
Source: owner (M6 step U3, relevance rule 2026-10-19), WDEF-007, VIEW-012

#### Scenario: The size on the screen
- **WHEN** a reach of 100 m is seen from 1,000 m and 2,000 m up with a 60 degree field of view over 1,000 pixels, doubled, and from a camera on the point
- **THEN** the sizes are `0.1 * 500 / tan(30 degrees)` pixels, half of that, twice that, and infinity

#### Scenario: Near the ground and from high up
- **WHEN** a hill (reach 500 m) and a plateau (reach 18.75 km) at one place are seen from 2 km, from 100 km, and from the heights where the hill covers 7.9 and 8.1 pixels
- **THEN** both show at 2 km, only the plateau at 100 km, and the hill shows at 8.1 pixels and not at 7.9

#### Scenario: The footprint and the selection
- **WHEN** two large plateaus lie on two sides of the planet and one tile is visible, no tile is visible, the far one is selected, a small selected hill lies outside the tiles at 2,000 km, 400 hills lie far away, and the near plateau and the selected hill are hidden
- **THEN** only the plateau in the tile shows, nothing shows without a tile, the selected edit is added in each case, the 400 hills change nothing, and a hidden edit shows only while selected

### Requirement: VIEW-016 Lists and search of the edits
The editor SHALL list the edits as plain data (`planet_editor::lists`, GPU-free): the nearby list, the planet-wide search, the layers and the chain (a row holds the id, a label, kind, layer, distance from the camera and reach). The nearby list holds the edits the camera shows (VIEW-015), nearest to the camera first, ties by id. The search runs over every edit of the body, not only the shown ones: every word of the query is part of the label, kind, layer or id text, in any case; an empty query matches all; a kind filter restricts to one kind; the rows are nearest first, ties by id. The label of an edit is its name, else its kind and the last six characters of its id. The distance is that from the camera to the anchor's point on the sphere. The layers list groups by layer name, the edits of a layer in the order they apply (priority, then id); the chain is all edits in that order.

Verify: A
Status: active
Source: owner (M6 step U3, search planet-wide sorted by distance from the camera 2026-10-19), VIEW-015

#### Scenario: Nearby
- **WHEN** three named edits are listed from a camera near the second, with an id outside the set and two edits at the same distance
- **THEN** the rows are nearest first, an id with no edit is left out, and equal distances order by id

#### Scenario: Search
- **WHEN** the query is `hill`, `HILL far`, empty, a kind filter, a text that matches nothing, and the last six characters of an unnamed edit's id
- **THEN** the far hill is found after the near one, every word must match in any case, all edits are listed for the empty query, the filter keeps one kind, nothing matches the unknown text, and the unnamed edit is found and labelled `kind` plus those characters

#### Scenario: Layers and order
- **WHEN** edits with priorities 5, -1, 0 and 0 in two layers are listed
- **THEN** the layers come by name, each layer's rows by priority then id, and the chain is all four in that order

### Requirement: VIEW-017 Selecting, handles and typed values
The editor SHALL select, place, delete, drag and type values through one session over the E4 commands (`planet_editor::session`, with `shape` and `params`; GPU-free, f64 on the CPU, `libm` only), so the flows are tested without a window. The outline of an edit is its `radius_m` parameter when positive, else its reach. The handles are the anchor (always), the radius (when it has an outline) and the height (when it has a finite `height_m`). A handle sits on the anchor, on the outline along the item's heading (0 north, 90 east), or above the anchor by the height. A drag makes the item the pointer says and keeps its bounds true: move puts the anchor under the pointer (latitude and longitude in the spelling the item file takes: longitude in (-180, 180], 0 at a pole, latitude by `atan2` for precision); resize sets `radius_m` to the arc from the anchor to the pointer (at least 1 m, at most one cell edge) and grows the reach to hold it (never shrinks it); raise adds to `height_m` and grows the bound on the change to hold it (up to `MAX_DELTA_LIMIT_M`). A whole drag, `begin_drag` to `end_drag`, is one E4 command whatever the number of pointer moves; `cancel_drag` records nothing; no other command, undo or redo runs while a handle is held. Placing a stamp (kind from the table, its defaults, reach 1.25 times the radius, the bound the largest metric parameter), deleting, renaming and a typed value are one command each; a typed value is read in the parameter's unit (`450`, `2.4 km`, `40 %`) and a value out of the kind's range, not a number, in another unit, or of an unknown parameter changes nothing and says why. The item under the pointer is the nearest in units of its own outline radius among the shown ones (`pick_footprint`); an item that is not shown cannot be picked. The inspector is the selected edit's values as plain data: kind, layer, place, distance, reach and the parameters in the kind's table order with their ranges, then the others by name.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, typed metric input the fallback, 2026-10-19), AUTH-004, VIEW-012

#### Scenario: Outline and handles
- **WHEN** an item with a radius and a height, one without a height, one with neither and no reach, and one with a radius that is not positive are examined, and the handles are placed at headings 0, 90 and 217
- **THEN** the handle lists are move, radius, height; move, radius; move; and the outline falls back to the reach, the radius handle is 400 m from the anchor within 1 mm along the heading, the height handle is the height above the anchor, and heading 90 is east

#### Scenario: One spelling
- **WHEN** directions at ordinary places, 89.9999 degrees north, the poles, the antimeridian and negative zero are turned into latitude and longitude, and the results are put through `Item::validate`
- **THEN** they equal the input within 1e-9 degrees, a pole has longitude 0, the antimeridian is +180, and every result is a valid item

#### Scenario: Drag arithmetic
- **WHEN** an item is moved, resized to 2,000 m, 100 m, 0 m and 90 km, and raised by 40 m, -200 m and a billion metres, and a pixel is converted to metres at 1 km
- **THEN** the anchor is under the pointer, the radius is the arc within 1 mm with the reach holding it (and not shrinking), the radius is at least 1 m and at most one cell edge, the height and its bound follow and stop at the limit, a pixel is `2 d tan(fov / 2) / height_px` metres, and dragging up raises

#### Scenario: Picking
- **WHEN** a small hill lies inside a large plateau, the pointer is on the hill's centre, 5 km away, far away, and the hill or both are left out of the shown set or an id has no item
- **THEN** the hill is under the pointer at its centre, the plateau 5 km away, nothing far away, and an edit that is not shown or not an item is not picked

#### Scenario: Commands
- **WHEN** craters are placed, a value is typed (`3.5 km`, `300`, `20 km`, `10`, `-5`, `abc`, an empty text, `5 kg`, an unknown parameter, `NaN`), a drag of a hundred moves ends, resize and raise drags end, a drag is cancelled, an edit is deleted, and everything is undone
- **THEN** each placement is one selected command with the kind's defaults and a written file, the valid values are one command each in their unit with the reach and bound following, every refused value changes nothing, each drag is one undo step, the cancelled drag changes nothing and records nothing, a delete deselects, and the undos reach the exact starting hash

#### Scenario: Inspector and units
- **WHEN** an edit with table parameters and two other parameters is inspected, and lengths are typed and formatted
- **THEN** the kind, layer, `47.20000° N`, `8.55000° E`, `2.4 km`, `1 of 2 in its layer` and `500 m` show, the parameters come in table order with their ranges and the others by name without a range, `450`, `450 m`, `2.4 km` and `40 %` are read in their units, and empty text, words, a wrong unit, exponent notation and non-finite numbers are refused with the reason

### Requirement: VIEW-018 The layout of the panels is data
The layout of the editor panels SHALL be a value read from a file of keys, not code (`planet_editor::layout`, GPU-free): the width of the tool rail, of the left panel and of the inspector, the side of the inspector, the tabs of the left panel in order (`near`, `layers`, `graph`; at least one, none twice), the status bar and the hit radius of a handle in pixels. A key the file does not name keeps its default; an unknown key, a value out of its range, a wrong type, a bad tab list or a file that does not parse is refused naming the key. The panels (`app`) draw from this value, so a change of layout is an edit of the file.

Verify: A
Status: active
Source: owner (M6 step U3: layout data-driven and cheap to change, 2026-10-19), VIEW-011

#### Scenario: Defaults and overrides
- **WHEN** an empty file and a file naming some keys are read
- **THEN** the first is the default (rail 56, left 300, inspector 320 on the right, all three tabs, status bar on, hit radius 10) and the second changes only the named keys

#### Scenario: Refusals
- **WHEN** a file has an unknown key, a width out of range, a string for a number, `nan`, a bad side, an empty, repeated, unknown or mistyped tab list, or a syntax error
- **THEN** each is refused with the key named (`(file)` for the syntax error)
