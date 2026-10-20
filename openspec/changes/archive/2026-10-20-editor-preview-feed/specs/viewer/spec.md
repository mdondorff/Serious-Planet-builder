## ADDED Requirements

### Requirement: VIEW-023 The editor mode previews an edit in flight
After the editor mode hands a changed definition to the generator and names the tiles to refresh (VIEW-022), the window SHALL call `ManagedLoop::set_preview` with `preview_delta(oldest, current, camera)` (REND-033), where `current` is the footprint items of the whole body (`footprint_items_cap` over the full sphere) of the definition just handed over, `camera` is the camera's planet-fixed position, and `oldest` is the footprint items of the oldest definition any resident tile was built from: the definition replaced by the first hand-over since the preview last ended, kept unchanged by further hand-overs, so that a drag in many steps previews as one delta from its first state. The preview SHALL end (`set_preview` with an empty list, `oldest` forgotten) at the first frame whose plan is exact with no substituted node, no unfinished tile job, no busy plan worker and no pending upload. The editor world SHALL be a plain `WorldTerrain` (ADR 0068).

Verify: A, C
Status: active
Source: owner (M6 step U4, 2026-10-19), REND-033, ADR 0066 points 3 and 5, ADR 0068

#### Scenario: A drag is one delta from its first state
- **WHEN** a stamp is placed (hand-over 1), then moved twice (hand-overs 2 and 3) before any frame settles
- **THEN** the preview after hand-over 3 is `preview_delta` from the items before hand-over 1 to the items of hand-over 3, which removes nothing the placement added and adds the stamp once at its final place; it is not the delta from hand-over 2 to 3

#### Scenario: The preview ends when the exact tiles are in
- **WHEN** a hand-over has set a preview and a frame is exact with nothing substituted, no unfinished job, no busy worker and nothing pending, and another hand-over then follows
- **THEN** the first sets the empty list and forgets `oldest`, and the next preview is the delta from the definition the second hand-over replaced

#### Scenario: The real window previews
- **WHEN** the real window on the software adapter places and drags a stamp with the editor mode (`--smoke --edit`)
- **THEN** at least one preview was set, and the run still ends exact
