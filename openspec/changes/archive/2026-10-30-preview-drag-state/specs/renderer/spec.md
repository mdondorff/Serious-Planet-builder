## ADDED Requirements

### Requirement: REND-045 The preview delta is applied only to a tile at the oldest state
A stale node SHALL be flagged by the delta of REND-033 only while its resident key is the key its tile had when the preview began and no stamp the delta left out at the cap (`PreviewList::left_out`) touches it. Any other stale node (a tile refreshed at an intermediate state of a drag, loaded since, refreshed ahead of the plan, or under a left-out stamp) SHALL be drawn as it is, stale and counted as substituted. The keys SHALL be recorded by the first non-empty `ManagedLoop::set_preview` or `set_preview_list` since the preview ended, kept by an empty `set_preview_list`, and forgotten by `set_preview` with an empty list.

Verify: A
Status: active
Source: owner (2026-10-29, drag vs rest), opus-planner (2026-10-29), REND-033

#### Scenario: A tile refreshed at an intermediate state of a drag
- **WHEN** a hill is dragged in two steps, some tiles are refreshed after the first step, and the frame after the second is drawn with the delta from the oldest state
- **THEN** only tiles still at the oldest state are flagged, a tile at the middle state is drawn as it is and stale, and the heights of a flagged tile with the delta equal the current state while an unflagged tile shows its own

#### Scenario: Left out at the cap
- **WHEN** a delta leaves stamps out at the cap and one of them touches a stale tile
- **THEN** that tile is not flagged by the delta

#### Scenario: The edit named a tile that was refreshed ahead of the plan
- **WHEN** a tile the edit named holds the new key before the plan made after the edit is delivered
- **THEN** it is not flagged: it holds the edit already
