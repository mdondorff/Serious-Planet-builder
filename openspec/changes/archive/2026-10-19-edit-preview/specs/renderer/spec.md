## ADDED Requirements

### Requirement: REND-033 Edit preview
A draw plan SHALL carry (in the `DrawPlan` of the frame) a list of at most 8 preview stamps, each a signed stamp (GEN-026: kind, anchor, reach, parameter, bound) with a sign of +1 or -1, and a flag per node: a node is previewed when its resident tile is stale (REND-032) and the reach of a preview stamp touches the tile's cap. The list SHALL be computed from the footprint items the stale tiles were built from (the oldest state any resident tile shows) and the current items: an item only in the current set is +1, an item only in the old set is -1, a changed item is both, an unchanged one is absent, so a drag of many steps is one delta from the first state; a removed plateau is not listed (the flattened ground is not in the stale tile) and a changed plateau is listed as its new state. A list longer than 8 SHALL keep the items nearest to the camera, an item's entries together or not at all, and report how many entries it left out. The compact vertex stage SHALL add, for a previewed node, the delta of each preview stamp, times its sign, to the height of the vertex along the tile's up in the order of the list, with the profiles of GEN-026 on the distance from the anchor measured in the tangent plane of the vertex's own radial up (so two tiles sharing a border vertex agree) in camera-relative f32 arithmetic (the anchor is rounded once from f64 on the CPU; no planet-radius value in the shader, CON-11), and SHALL displace the neighbours that make the parent-grid target the same way, so the morph target is the displaced parent grid; it SHALL correct the normal by the slope of the summed delta and lean it to the up by how flat a plateau makes the ground; a plateau uses the vertex's own height as the running height. With no preview stamp the stage SHALL not be entered: every frame is bit-identical to a frame of a build without the preview. A node whose resident key equals the plan's SHALL not be previewed, so the exact tile replaces the preview without a step in the list.

Verify: A, C
Status: active
Source: SPIKE-13, ADR 0066 points 3 and 5, M6 plan row E3

#### Scenario: Delta
- **WHEN** a stamp is added, removed, moved, and changed in size between two item sets
- **THEN** the list holds the +1 and -1 entries named above, none for an unchanged item, and at most 8 of them, nearest first

#### Scenario: Previewed nodes
- **WHEN** a draw plan has resident tiles with the plan's key, with an older key under a preview stamp, and with an older key away from it
- **THEN** only the second kind is flagged, and a node flagged earlier whose exact tile arrived is not

#### Scenario: Displacement
- **WHEN** a scene whose nodes are all flagged is drawn on the compact path with no preview stamp, with a stamp on unflagged nodes, with a stamp, with a stamp and its removal, and with a stamp of a smaller reach
- **THEN** the first two frames equal the frame of the build without the preview, the third differs, the fourth equals the first, and the fifth changes no more pixels than the third; the shader's profiles are those of GEN-026 (a scan of the shader text) and the stage is entered only for a flagged node with a stamp

#### Scenario: Close to the exact tiles
- **WHEN** a hill, a crater and a plateau are added to a world, and the Height view is drawn from the stale tiles of the unedited world with the stamp as preview, and from the exact tiles of the edited world planned for it
- **THEN** the preview frame differs from the exact frame by more than three grey levels in at most 5 % of the pixels the edit changes
