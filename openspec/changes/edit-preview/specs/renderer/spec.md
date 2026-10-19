## ADDED Requirements

### Requirement: REND-033 Edit preview
A draw plan SHALL carry (in the `DrawPlan` of the frame) a list of at most 8 preview stamps, each a signed stamp (GEN-026: kind, anchor, reach, parameter, bound) with a sign of +1 or -1, and a flag per node: a node is previewed when its resident tile is stale (REND-032) and the reach of a preview stamp touches the tile's cap. The list SHALL be computed from the footprint items before and after an edit: an item only in the after set is +1, an item only in the before set is -1, a changed item is both, an unchanged one is absent. A list longer than 8 SHALL keep the 8 nearest to the camera and say so. The compact vertex stage SHALL add, for a previewed node, the delta of each preview stamp, times its sign, to the height of the vertex along the tile's up in the order of the list, with the profiles of GEN-026 on the distance from the anchor measured in the tile's tangent plane in tile-local f32 arithmetic (no planet-radius value, CON-11), and SHALL correct the normal by the radial slope of the summed delta; a plateau uses the vertex's own height as the running height. With no preview stamp the stage SHALL not be entered: every frame is bit-identical to a frame of a build without the preview. A node whose resident key equals the plan's SHALL not be previewed, so the exact tile replaces the preview without a step in the list.

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
