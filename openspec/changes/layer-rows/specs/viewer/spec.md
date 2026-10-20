## MODIFIED Requirements

### Requirement: VIEW-016 Lists and search of the edits
The editor SHALL list the edits as plain data (`planet_editor::lists`, GPU-free); a row holds the id, a label, kind, layer, distance from the camera, reach, and for lines, regions and brushes the numbers of its detail: the parts of its group (1 when it is not split), which part it is, its points and its strokes.
- The label of an edit is its name, trimmed, else its kind and the last six characters of its id.
- The distance is that from the camera to the anchor's point on the sphere.
- The nearby list holds the edits the camera shows (VIEW-015), nearest first, ties by id.
- The search runs over every edit of the body, not only the shown ones: every word of the query is part of the label, kind, layer or id text, in any case; an empty query matches all; a kind filter restricts to one kind; the rows are nearest first, ties by id.
- The order the edits apply in is the layer's place in the layer stack, then priority, then id (ADR 0007, ADR 0066 point 4); the stack bottom to top is `base`, `imported`, `intent`, `stamps`, `splines`, `placed`, `frozen`, and a layer outside it applies after these, by name.
- The layers list groups by layer in stack order, each group in the order the edits apply; the chain is all edits in that order.
- A line or a brush drag that was split into linked items (a group, VIEW-028, VIEW-035) is one row of the layers list, the row of its first part, with the number of its parts and, for a brush, the strokes of all of them; the nearby list, the search and the chain keep every item, and a part says which one it is. The detail of a row (`Row::detail`) is, for a brush, its strokes (`1 stroke`, `3 strokes`, with `N parts, ` before when it has several items), for a split line its parts (`N parts`), for a part `part K`, for a line or region its points (`17 points`), and nothing for a stamp. The panels show it after the kind in the layers list and the other lists, and a brush has a colour of its own.
- Selecting a row selects its item (a double click flies to it, as the Fly to button does); hiding a row hides, and deleting the selected item deletes, the whole group it belongs to, as one command, so that a brush, which has no handle, is found and deleted by its row and no part of a line or a brush drag is left behind.

Verify: A
Status: active
Source: owner (M6 step U3, search planet-wide sorted by distance from the camera 2026-10-19), VIEW-015, ADR 0007, ADR 0066

#### Scenario: Nearby
- **WHEN** three named edits are listed from a camera near the second, with an id outside the set and two edits at the same distance
- **THEN** the rows are nearest first, an id with no edit is left out, and equal distances order by id

#### Scenario: Search
- **WHEN** the query is `hill`, `HILL far`, empty, a kind filter, a text that matches nothing, and the last six characters of an unnamed edit's id, and a name with spaces around it
- **THEN** the far hill is found after the near one, every word must match in any case, all edits are listed for the empty query, the filter keeps one kind, nothing matches the unknown text, the unnamed edit is found with the label `kind` plus those characters, and the padded name is shown trimmed

#### Scenario: Lines, regions and brushes
- **WHEN** a ridge of 150 km (split into parts), a short channel, a region of three points and a brush with two strokes are in the layers list, and a brush drag of 12 km made several items
- **THEN** the ridge is one row saying `N parts`, the channel `17 points`, the region `3 points`, the brush `2 strokes`, the long brush drag one row saying `N parts, S strokes`, the search lists every part of the ridge, and hiding or deleting the ridge's row takes all its parts (the delete is one command that one undo takes back)

#### Scenario: Layers and order
- **WHEN** two edits of priorities 5 and -1 in `stamps`, one in `splines` (also at priority -100) and one in a layer outside the stack are listed
- **THEN** the groups come in the order `stamps`, `splines`, the outside layer, the `stamps` rows by priority, and the chain is the same order whatever the priorities of the higher layers
