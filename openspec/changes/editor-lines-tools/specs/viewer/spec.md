## ADDED Requirements

### Requirement: VIEW-029 The line and region tools
With the line tool (`ridge` or `channel`) or the region tool on, a press on the planet SHALL add a point to a draft; a point within 0.5 m of the last one SHALL be refused as the same click again, a line draft SHALL hold at most 640 points and a region draft 64, a press off the planet SHALL do nothing, and changing the tool SHALL drop the draft. Finishing SHALL put the line (parameters `amount_m` 30 m and `width_m` 200 m, bound 60 m, layer `stamps`, name `Ridge 1`, `Channel 1`) or the region (parameters `surface_type` 0 and `mask_mode` 0, layer `regions`) as one command, select it and return to the select tool; fewer points than the kind needs (2 and 3), or a region the command layer refuses (VIEW-028), SHALL be refused with the reason and leave the draft. A line or region SHALL have one point handle per point, a line counting the points of all its parts joined, positioned on its points; while one is held the points SHALL be kept in the session (drawn from there, the editor told nothing) and the release SHALL make the one command that sets the points; Esc SHALL drop them with nothing recorded; a point list the item refuses SHALL leave the item as it was and the drag closed. A press on the outline of the selected line or region within a distance SHALL add a point at the foot, one command, the closing segment included for a region; a point SHALL be removed in one command and refused when fewer than the kind's minimum would be left.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, VIEW-017, VIEW-019, VIEW-027, VIEW-028

#### Scenario: Draw a line
- **WHEN** the line tool is on and three places are clicked, the last again at once, and Enter finishes
- **THEN** the draft holds three points, the same place twice is refused, and finishing makes one command whose undo removes the line

#### Scenario: A point drag
- **WHEN** point 1 of a line is dragged over several moves and released
- **THEN** the editor holds the old points until the release, the handle follows the pointer meanwhile, the release is one command and one undo restores the old points

#### Scenario: A long line
- **WHEN** a point of the later part of a 150 km line is dragged
- **THEN** every point of every part has a handle, and the release is one command for the whole line
