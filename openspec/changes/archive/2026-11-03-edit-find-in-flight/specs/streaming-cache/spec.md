## ADDED Requirements
### Requirement: STRM-071 An edit's find covers resident and in-flight tiles
The footprint find of an item edit (WDEF-007) and every refresh of all tiles (the coupled lens switched) SHALL run over the resident tiles and the tiles whose job is in flight (`ManagedLoop::edit_candidates`): a job reads the field when it starts, so a tile still loading when the definition changes would otherwise land with the old field's key and never be refreshed (STRM-026). A named in-flight tile's job SHALL be told to stop and the tile SHALL load again from the new field. A request still queued in the streamer is not named: its job starts on the new field.

Verify: A
Status: active
Source: opus-planner (2026-11-03), the edit-latency smoke at 5 tile workers (#1117), STRM-026, STRM-005

#### Scenario: A tile loading at the edit
- **WHEN** a drawn tile the edit touches has read the old field and its job waits, the edit lands with the find over the candidates, and the job then finishes
- **THEN** the view becomes exact and the tile holds the new field's key

#### Scenario: The find over the resident tiles alone
- **WHEN** the same edit lands with the find over the resident tiles only
- **THEN** the tile stays resident, not stale, on the old key, and the view never becomes exact

#### Scenario: Tiles queued at the edit
- **WHEN** two jobs run at a time, so touched tiles of the view still wait in the streamer's queue when the edit lands
- **THEN** they are not named, and each loads the new field's key
