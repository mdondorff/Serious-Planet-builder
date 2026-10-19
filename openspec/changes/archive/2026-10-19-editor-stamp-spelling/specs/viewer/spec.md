## MODIFIED Requirements

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
- **WHEN** a hill (reach 400 m) and a plateau (reach 15 km) at one place are seen from 2 km, from 100 km, and from the heights where the hill covers 7.9 and 8.1 pixels
- **THEN** both show at 2 km, only the plateau at 100 km, and the hill shows at 8.1 pixels and not at 7.9

#### Scenario: The footprint and the selection
- **WHEN** two large plateaus lie on two sides of the planet and one tile is visible, then no tile is visible, then the far one is selected, then a small hill outside the tiles is selected from 2,000 km up, then the near plateau is hidden
- **THEN** only the plateau in the tile shows, nothing shows without a tile, the selected edit is added each time, and a hidden edit shows only while selected

#### Scenario: Not a scan
- **WHEN** forty large plateaus 330 to 700 km away, each covering more than 8 pixels from 30 km up, lie outside the visible tile
- **THEN** none of them shows, so the tiles and not the pixel test kept them out

### Requirement: VIEW-017 Selecting, handles and typed values
The editor SHALL select, place, delete, drag and type values through one session over the E4 commands (`planet_editor::session`, with `shape` and `params`; GPU-free, f64 on the CPU, `libm` only), so the flows are tested without a window.

Shape and handles:
- The radius of a stamp (hill, crater, plateau) is the file's `reach_m` (WDEF-010), and so is its outline; the outline of another kind is its `radius_m` parameter when positive, else its reach.
- The amount of a stamp is its one parameter: `height_m` of a hill (signed), `depth_m` of a crater, `target_m` of a plateau (an absolute height); the height handle edits it. Another kind has a height handle when it has `height_m`.
- The handles are the anchor (always), the radius (when it has an outline) and the height (when it has its amount).
- A handle sits on the anchor, on the outline along the item's heading (0 north, 90 east), or at the height: above the anchor by a hill's height, below it by a crater's depth, at a plateau's absolute target whatever the ground.
- At a pole, north is the limit along longitude 0: -x at the north pole, +x at the south pole.
- A direction becomes latitude and longitude in the spelling the item file takes: longitude in (-180, 180], 0 at a pole; latitude by `atan2` of the height over the distance from the axis.

Drags (each result is a valid item and the bounds hold it):
- move puts the anchor under the pointer;
- resize sets a stamp's `reach_m` to the arc from the anchor to the pointer, so it follows the pointer in and out; another kind's `radius_m` becomes the arc and its reach grows to 1.25 times it (at most one cell edge, never shrinking);
- raise changes the kind's amount: a hill's `height_m` and a plateau's `target_m` go up with the pointer, a crater's `depth_m` goes down (the handle is the bottom of the bowl);
- the bound of a hill or a crater is exactly its amount, and a plateau's bound is its own parameter and does not move;
- another kind's raise adds to its `height_m` and its bound grows to hold it (never shrinking, at most `MAX_DELTA_LIMIT_M`);
- the radius and the height stay in the kind's range from the parameter table, the range a typed value is held to; a kind the table does not know keeps the radius within 1 m and one cell edge and the height within the limit on the change;
- a pixel is `2 d tan(fov_y / 2) / height_px` metres at a point whose depth along the view axis is `d` (at least 1 m), and a drag up raises.

Commands, each one E4 command:
- a whole drag, `begin_drag` to `end_drag`, whatever the number of pointer moves; `cancel_drag` records nothing; while a handle is held no other command, undo, redo or change of selection runs;
- placing a stamp (the kind's defaults from the table, in the layer `stamps`: the reach is the radius, the bound is the amount of a hill or a crater and the table's default for a plateau), deleting, renaming and a typed value;
- a place that is not a finite non-zero direction is refused.

Typed values:
- the text is read in the parameter's unit (`450`, `2.4 km`, `40 %`);
- a value out of the kind's range, not a number, in another unit, or of an unknown parameter changes nothing and says why;
- where the table has no range the item file's limits hold: a radius within 0 and one cell edge, any other metric parameter within the limit on the change;
- the radius of a stamp typed is its reach;
- the amount of a hill or a crater typed is its bound;
- a plateau's flattening limit is its `max_delta_m`, a field of the file and no parameter;
- for another kind the reach and the bound only grow to hold the value.

Picking and inspection:
- the item under the pointer is the nearest in units of its own outline radius among the shown ones (`pick_footprint`); an item that is not shown cannot be picked;
- the inspector is the selected edit's values as plain data: kind, layer, place in the order of its layer, distance, reach and the parameters in the table's order with their ranges, then the others by name without a range;
- lengths read `450 m` below 1 km (after rounding to a tenth of a metre) and `2.4 km` above.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, typed metric input the fallback, 2026-10-19), AUTH-004, VIEW-012

#### Scenario: Outline and handles
- **WHEN** an item with a radius and a height, one without a height, one with neither and no reach, and one with a radius that is not positive are examined, and the handles are placed at headings 0, 90 and 217
- **THEN** the handle lists are move, radius, height; move, radius; move; the outline falls back to the reach, the radius handle is 400 m from the anchor within 1 mm along the heading, the height handle is above the anchor by a hill's height, below it by a crater's depth and at a plateau's absolute target, and heading 90 is east

#### Scenario: One spelling and the poles
- **WHEN** directions at ordinary places, 89.9999 degrees north, the poles, the antimeridian and negative zero are turned into latitude and longitude, put through `Item::validate`, and the north of a pole is compared with the north a micro-degree away
- **THEN** they equal the input within 1e-9 degrees, a pole has longitude 0, the antimeridian is +180, every result is a valid item, and the north does not jump at either pole

#### Scenario: Drag arithmetic
- **WHEN** a hill is moved, resized to 1,500 m, 100 m, 0 m and 90 km, raised by 40 m, -200 m and a billion metres, and a kind the table does not know is resized and raised the same way
- **THEN** the anchor is under the pointer, the reach is the arc within 1 mm and follows it down as well as up, the hill stays within 50 m to 2 km and -400 m to 400 m with its bound equal to its height, a crater's depth stays within 10 m to 800 m (raising makes it shallower) with its bound equal to it, a plateau's target stays within -500 m to 9,000 m and its bound does not move, the unknown kind stays within 1 m to one cell edge and within the limit with its bound holding the height, a pixel at 1 km is `2 d tan(fov_y / 2) / height_px` metres, and a drag up raises

#### Scenario: Picking
- **WHEN** a small hill lies inside a large plateau, the pointer is on the hill's centre, 5 km away, far away, and the hill or both are left out of the shown set or an id has no item
- **THEN** the hill is under the pointer at its centre, the plateau 5 km away, nothing far away, and an edit that is not shown or not an item is not picked

#### Scenario: Placing and deleting
- **WHEN** craters are placed, an edit is deleted and undone and redone, and everything is undone
- **THEN** each placement is one selected command with the kind's defaults and a written file, a delete deselects, and the undos reach the exact starting hash and clear a selection whose edit is gone

#### Scenario: Typed values
- **WHEN** `3.5 km`, `300`, `20 km`, `10`, `-5`, `abc`, an empty text, `5 kg`, an unknown parameter and `NaN` are typed, a plateau's limit and target, a hill's negative height and a crater's depth are typed, and over-limit values for a kind the table does not know
- **THEN** the valid values are one command each in their unit with the reach and bound following (a plateau's limit goes to `max_delta_m`, a negative hill height sets the bound to its size, a crater's depth to its bound), every refused value changes nothing, and the stored item keeps its outline within its reach and its height within its bound

#### Scenario: Drags are one command
- **WHEN** a hundred pointer moves of a position handle end, then radius and height drags end, a drag is cancelled, and a place that is zero, not a number or infinite is given to a drag and to a placement, and another edit is selected during a drag
- **THEN** each drag is one undo step and three undos reach the exact starting hash, the cancelled drag changes nothing and records nothing, the bad places are refused, and the selection does not change while the handle is held

#### Scenario: Inspector and units
- **WHEN** an edit with table parameters and two other parameters is inspected, and lengths are typed and formatted
- **THEN** the kind, layer, `47.20000° N`, `8.55000° E`, `2.4 km`, `1 of 2 in its layer` and `500 m` show, the parameters come in table order with their ranges and the others by name without a range, `450`, `450 m`, `2.4 km` and `40 %` are read in their units, empty text, words, a wrong unit, exponent notation and non-finite numbers are refused with the reason, and 999.96 m reads `1 km`
