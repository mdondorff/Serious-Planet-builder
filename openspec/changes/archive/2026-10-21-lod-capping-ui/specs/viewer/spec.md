## MODIFIED Requirements

### Requirement: VIEW-044 Stale-tile overlay
The capped colour (ADR 0074): a plan leaf held back from splitting because the tiles under it are too steep to morph is in the state `TileState::Capped` (grey in the stale-tile overlay and its legend) when it is otherwise exact, and the tiles and mesh overlays draw it grey whatever its level. The rest of the requirement is as before (full text in `openspec/specs/viewer/spec.md`).

Verify: A, C
Status: active
Source: ADR 0074, VIEW-014

#### Scenario: A capped tile
- **WHEN** a plan over a spike is made and drawn with the overlays on
- **THEN** the capped leaves are in the capped state and no other leaf is, and a capped tile is grey in tiles and mesh

### Requirement: VIEW-040 A flight over ground the plan refuses prints a few lines, not one per frame
The window also prints one line in `REFUSAL_REPORT_PERIOD_MS` with the count of tiles held back (`CappedReporter`), none while there are none, and no crash report. The rest as before.

Verify: A
Status: active
Source: ADR 0074, opus-planner (2026-10-20)
