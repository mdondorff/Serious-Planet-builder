## ADDED Requirements

### Requirement: VIEW-048 The edit-health lists follow the items
Brushes too steep to draw fully (VIEW-043), brushes stacked and whether a stack can be merged (VIEW-047) SHALL be kept up to date as items change, not read from all items at each change (SPIKE-29). The editor SHALL log the ids of changed items (`Editor::change_seq`, `Editor::changes_since`). The session SHALL file brush items in a grid of 0.1 degree cells by the circle bounding their strokes (`health::Health`) and re-check only the changed items and those whose circle meets an old or new circle of a changed item. The first call, a new drawable slope and a log that no longer reaches back start again from all items. The result SHALL equal the full scans `slope::steep_items` and `problems::stacked_items`. `Session::health_checked` reports the work.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 2: incremental and lazy lists, the work per change bounded), VIEW-043, VIEW-047

#### Scenario: Equal to the full scans
- **WHEN** 30 brushes in four small areas are put, then 60 random edits (put, change, delete, undo) are made, and the drawable slope is changed
- **THEN** after the puts, after every edit and after the slope change the steep set and the stacked map equal those of the full scans over the same items

#### Scenario: The work follows the neighbours, not the items
- **WHEN** 5,041 isolated brushes a kilometre apart are put and the lists asked for, then one is changed, one is put on top of another, that one is deleted, and 40 strokes are piled on one place and one of them changed
- **THEN** the first call checks 5,041 items, the change checks 1, asking again checks nothing new, the brush on top checks 2, the delete checks 1, the change in the pile checks at most 42, and the lists equal the full scans

### Requirement: VIEW-049 The lists read what is near
The group warning of a row (any item of its group over the limit) SHALL be read from an index of the groups, not by scanning the items. The problems list SHALL look up its rows, not scan the items for them. Whether a stack can be merged SHALL be remembered per item until that item changes. The slope guard of a stroke (VIEW-043) SHALL read only the strokes of the brush items whose footprint meets the circle round the stroke: the same strokes in the same order, so the same strength. The lists are computed when asked, once per change; a list that is not shown does no work.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 2), VIEW-043, VIEW-047, VIEW-048

#### Scenario: Same answers
- **WHEN** the problems list, the row warnings and the slope guard are asked for after the edits of the VIEW-048 scenarios
- **THEN** they equal what the scans over all items give
