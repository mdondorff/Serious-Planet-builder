## MODIFIED Requirements

### Requirement: REND-062 The viewer plans the rivers of every lit frame from its river source
When the stored world holds a river source (STRM-086), the viewer SHALL set the lit plan's `rivers` each frame from `plan_rivers` over the terrain plan's nodes, with the river law of the bake (`WidthLaw::STANDARD`), the minimum and full samples `MIN_RIVER_SAMPLES` and `FULL_RIVER_SAMPLES`, the layer count `RIVER_LAYERS` and the node's distance from the camera; a world without a river source SHALL leave `rivers` empty.
- A plan error ends the frame with the error's text (never a silent drop).
- A river is drawn only over the commit's L tiles: a node sample whose nearest cell lies in a tile the commit does not hold is far from every river, and such a tile's channels are not read (the graph's margin blocks lie over ground the commit did not carve).

Verify: A
Status: active
Source: Architect 2026-11-12 (item 15 PR 8), REND-059, STRM-086

#### Scenario: A world with a graph
- **WHEN** the viewer plans a lit frame over a world whose commit holds a channel graph
- **THEN** the plan's rivers hold the nodes with a river in range

### Requirement: REND-065 The water pass draws a lake node only where the commit's lake mask is wet
The water plan SHALL give each lake node (REND-058) a mask of its samples, each read at its nearest cell of the commit's lake grid; a node none of whose samples is the lake's SHALL carry no water. The water pass SHALL discard a lake node's fragment where the mask interpolated over the node is under one half; the sea has no mask.
- The masks share the river coverage atlas (REND-061 layout), in the slots after the rivers'; a mask with no slot left is drawn without it and counted in the frame stats.
- The masks are part of the plan's text form.
- A river ends at a lake's water: a river sample whose nearest cell is wet in a lake's mask is far from every river (the same commit's masks, read through the river source).

Verify: A
Status: active
Source: Architect 2026-11-11 (item 15 PR 4 decisions), lead 2026-11-13 (E1), REND-058, REND-061

#### Scenario: A mask
- **WHEN** a node of 8 cells over the first half of a 64-cell face is masked for a lake that holds that half
- **THEN** every sample is wet but the last column, whose nearest cell lies in the other half

#### Scenario: The plan
- **WHEN** a lake everywhere is planned with a mask wet everywhere, with one dry everywhere, and the sea with either
- **THEN** every lake node has a mask, the dry mask leaves no node, and the sea's nodes have none
