## MODIFIED Requirements

### Requirement: REND-033 Edit preview
A draw plan SHALL carry (in the `DrawPlan` of the frame) a list of at most 32 preview stamps, each a signed stamp (GEN-026: kind, anchor, reach, parameter, bound) with a sign of +1 or -1, and a flag per node: a node is previewed when its resident tile is stale (REND-032), or when the edit in flight named its tile (the footprint find, `ManagedLoop::edit`) and no plan made after that edit has been delivered yet, and the reach of a preview stamp touches the tile's cap. A tile the edit named is drawn as it is and counts as substituted until such a plan arrives, whatever the mesh key of the older plan says, so the preview shows in the frame after the edit and does not wait for the plan worker; from the delivery of that plan on, the keys decide (REND-032). The list SHALL be computed from the footprint items the stale tiles were built from (the oldest state any resident tile shows) and the current items: an item only in the current set is +1, an item only in the old set is -1, a changed item is both, an unchanged one is absent, so a drag of many steps is one delta from the first state; a removed plateau is not listed (the flattened ground is not in the stale tile) and a changed plateau is listed as its new state. The delta SHALL have at most 16 stamps (`MAX_DELTA_STAMPS`); a longer one SHALL keep the units nearest to the camera, a unit's entries together or not at all (a unit is an item's stamp, or one stroke of a brush item, VIEW-036), and report how many entries it left out. The compact vertex stage SHALL add, for a previewed node, the delta of each preview stamp, times its sign, to the height of the vertex along the tile's up in the order of the list, with the profiles of GEN-026 on the distance from the anchor measured in the tangent plane of the vertex's own radial up (so two tiles sharing a border vertex agree) in camera-relative f32 arithmetic (the anchor is rounded once from f64 on the CPU; no planet-radius value in the shader, CON-11), and SHALL displace the neighbours that make the parent-grid target the same way, so the morph target is the displaced parent grid; it SHALL correct the normal by the slope of the summed delta and lean it to the up by how flat a plateau makes the ground; a plateau uses the vertex's own height as the running height. The stage SHALL loop over the active count of the list (the count the globals carry), not over the table, so that a frame with no stamp or a short list costs what it holds. A transient preview (VIEW-036), the stamps of something being made such as a brush stroke under the pointer (`ManagedLoop::set_transient_preview`, `draw_plan_transient`), SHALL flag every node drawn from a resident tile whose cap a transient stamp touches, whether the tile is stale or exact, and the draw plan SHALL carry the delta's stamps and then the transient ones, at most 32 (the delta's at most 16 and the transient ones at most 16, so that none is cut); it does not change the kind of the frame. A node SHALL apply only the transient stamps (`transient_only`, node record `morph.z` 2) when its tile is exact (it already holds the edit the delta stands for) or when no stamp of the delta touches it; the globals' `extra.w` SHALL carry the active count in its low 6 bits and 64 times the number of the delta's stamps above them, and the vertex stage SHALL start such a node's loop after the delta's stamps. A smooth brush stroke is not previewed (a known limit, VIEW-036). With no preview stamp the stage SHALL not be entered: every frame is bit-identical to a frame of a build without the preview. A node whose resident key equals the plan's SHALL not be previewed, unless the plan predates an edit that named its tile, so the exact tile replaces the preview without a step in the list. Except as follows, the delta runs from the oldest state, so a stale node SHALL be flagged by it only while its resident key is the key its tile had when the preview began and no stamp the delta left out at the cap (`PreviewList::left_out`) touches it: a node drawn from a tile refreshed at an intermediate state of a drag, from a tile loaded since the preview began, from a tile an edit named that was refreshed ahead of the plan, or under a left-out stamp is drawn as it is (stale, counted as substituted), never with a delta that is not its own (the delta lists stamps only: a removed plateau or a line is not in it, as before). The states are recorded by the first non-empty `ManagedLoop::set_preview` or `set_preview_list` since the preview ended (after `ManagedLoop::edit` and before any frame uploads a refreshed tile), kept by an empty `set_preview_list` (a drag that returns to its start) and forgotten by `set_preview` with an empty list.

Verify: A, C
Status: active
Source: SPIKE-13, ADR 0066 points 3 and 5, M6 plan row E3, window 14 (2026-10-19)

#### Scenario: Delta
- **WHEN** a stamp is added, removed, moved, and changed in size between two item sets
- **THEN** the list holds the +1 and -1 entries named above, none for an unchanged item, and at most 16 of them, nearest first

#### Scenario: Previewed nodes
- **WHEN** a draw plan has resident tiles with the plan's key, with an older key under a preview stamp, and with an older key away from it
- **THEN** only the second kind is flagged, and a node flagged earlier whose exact tile arrived is not

#### Scenario: The frame after the edit
- **WHEN** an edit names resident tiles under a preview stamp and the next frame is begun before any plan made after the edit is delivered, then the jobs finish
- **THEN** that frame flags the named tiles under the stamp (and only tiles the edit named), is not exact and counts them as substituted, and once the exact tiles are resident the frame is exact and nothing is flagged

#### Scenario: Displacement
- **WHEN** a scene whose nodes are all flagged is drawn on the compact path with no preview stamp, with a stamp on unflagged nodes, with a stamp, with a stamp and its removal, and with a stamp of a smaller reach
- **THEN** the first two frames equal the frame of the build without the preview, the third differs, the fourth equals the first, and the fifth changes no more pixels than the third; the shader's profiles are those of GEN-026 (a scan of the shader text) and the stage is entered only for a flagged node with a stamp

#### Scenario: Close to the exact tiles
- **WHEN** a hill, a crater and a plateau are added to a world, and the Height view is drawn from the stale tiles of the unedited world with the stamp as preview, and from the exact tiles of the edited world planned for it
- **THEN** the preview frame differs from the exact frame by more than three grey levels in at most 5 % of the pixels the edit changes, and the same holds for the Normals view with steeper stamps (more than six grey levels)

#### Scenario: A tile refreshed at an intermediate state of a drag
- **WHEN** a hill is dragged in two steps, some tiles are refreshed after the first step, and the frame after the second is drawn with the delta from the oldest state
- **THEN** only tiles still at the oldest state are flagged, a tile at the middle state is drawn as it is and stale, and the heights of a flagged tile with the delta equal the current state while an unflagged tile shows its own

#### Scenario: Left out at the cap
- **WHEN** a delta leaves stamps out at the cap and one of them touches a stale tile
- **THEN** that tile is not flagged by the delta

#### Scenario: The edit named a tile that was refreshed ahead of the plan
- **WHEN** a tile the edit named holds the new key before the plan made after the edit is delivered
- **THEN** it is not flagged: it holds the edit already
