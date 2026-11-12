## ADDED Requirements

### Requirement: REND-065 The water pass draws a lake node only where the commit's lake mask is wet
The water plan SHALL give each node whose water is a lake (REND-058) a mask of the node's samples, each read at its nearest cell of the commit's lake grid (clamped to the face); a node none of whose samples is the lake's SHALL carry no water. The water pass SHALL discard a lake node's fragment where the mask interpolated over the node is under one half; a node without a mask (the sea) is drawn wherever `W - h_morph > 0`.
- The masks share the river coverage atlas (REND-061 layout), in the slots after the rivers'; a mask with no slot left is drawn without it and counted in the frame stats.
- The masks are part of the plan's text form.

Verify: A
Status: active
Source: Architect 2026-11-11 (item 15 PR 4 decisions), lead 2026-11-13 (E1), REND-058, REND-061

#### Scenario: A mask
- **WHEN** a node of 8 cells over the first half of a 64-cell face is masked for a lake that holds that half
- **THEN** every sample is wet but the last column, whose nearest cell lies in the other half

#### Scenario: The plan
- **WHEN** a lake everywhere is planned with a mask wet everywhere, with one dry everywhere, and the sea with either
- **THEN** every lake node has a mask, the dry mask leaves no node, and the sea's nodes have none
