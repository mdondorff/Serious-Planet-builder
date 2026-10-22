## ADDED Requirements

### Requirement: VIEW-048 The edit-health lists follow the items
The lists of VIEW-043 and VIEW-047 (the brushes too steep to draw fully, the brushes stacked, whether a stack can be merged) SHALL be kept up to date as the items change instead of read from all of them at each change (SPIKE-29: the full scan, quadratic in the strokes, was the first thing to break, at about 100 items). The editor SHALL keep a log of the ids of the items whose copy changed (`Editor::change_seq`, `Editor::changes_since`; a put, a change, a delete, an undo, a redo, a drag step, a load). The session SHALL file the brush items in a grid of cells by latitude and longitude (0.1 degree) by the circle that bounds their strokes (`health::Health`), and when the log has grown re-check the changed items and the items whose circle meets the old or the new circle of a changed item, and no others; the first call, a change of the drawable slope, and a log that no longer reaches back start again from all the items. The lists are computed when asked (a list, a row's warning, the problems tab, the inspector), once per change; a list that is not shown does no work. The result SHALL equal that of the full scans `slope::steep_items` and `problems::stacked_items` for the same items (the same terms summed in the same order). The group warning of a row (any item of its group over the limit) SHALL be read from an index of the groups, not by scanning the items; the problems list SHALL look up its rows and not scan the items for them; the slope guard of a stroke (VIEW-043) SHALL read only the strokes of the brush items whose footprint meets the circle round the stroke (the same strokes in the same order, so the same strength); whether a stack can be merged SHALL be remembered per item until that item changes. The count of items the last refresh checked is reported (`Session::health_checked`).

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 2: incremental and lazy lists, the work per change bounded), VIEW-043, VIEW-047

#### Scenario: Equal to the full scans
- **WHEN** 30 brushes in four small areas are put, then 60 random edits (put, change, delete, undo) are made, and the drawable slope is changed
- **THEN** after the puts, after every edit and after the slope change the steep set and the stacked map equal those of the full scans over the same items

#### Scenario: The work follows the neighbours, not the items
- **WHEN** 5,041 isolated brushes a kilometre apart are put and the lists asked for, then one is changed, one is put on top of another, that one is deleted, and 40 strokes are piled on one place and one of them changed
- **THEN** the first call checks 5,041 items, the change checks 1, asking again checks nothing new, the brush on top checks 2, the delete checks 1, the change in the pile checks at most 42, and the lists equal the full scans
