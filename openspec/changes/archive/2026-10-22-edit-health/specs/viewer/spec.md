## ADDED Requirements

### Requirement: VIEW-047 The problems list
The editor's left panel SHALL have a tab `problems` (`Tab::Problems`, last of the default tabs; its title shows the count, `Problems (3)`) that lists every edit the engine cannot draw fully or that costs more than it should, so the owner can find and fix it. An edit is listed when it is (1) a brush too steep to draw fully (VIEW-043), (2) a brush with very many strokes stacked: the discs of at least `STACK_WARN` = 8 strokes (of any kind and item) cover one point of its strokes, or the item holds at least `STROKES_WARN` = 48 of the 64 strokes a file allows, or (3) over a tile the last plan held back (ADR 0074, `TileState::Capped`, VIEW-044): the footprint find over the capped tiles of the plan, without the selected edit. Each row names the edit, its distance and every issue in words (`too steep to draw fully`, `9 strokes stacked at one place (12 of 64 strokes)`, `tiles near it are held back (too steep to refine)`), and has the buttons Fly to (the camera goes there, as a double click in a list does), Reduce, Merge strokes and Delete; clicking the name selects. The rows are sorted steep first, then stacked, then capped, then by distance. Reduce multiplies the strengths of the raises and lowers of the brush by the largest factor, to 1 %, under which it is no longer too steep with the other edits as they are (`problems::fit_factor`), by one half for a stacked brush that is not too steep, and is refused naming the other edits when even a hundredth of its strengths is too steep. Merge strokes replaces the stack of overlapping strokes of one kind (a raise or a lower; the deepest stack, the first stroke keeps its place and its path) by one stroke of the largest radius and the summed strength, and is refused when no two strokes of one kind overlap; the merged ground is not the stack's ground (the others' paths are dropped), so it is offered as a simplification. A fix that does not apply is disabled (Reduce and Merge on a stamp or a line). Every action is one command: one undo takes it back to the exact definition, and the places it changed are refreshed like any edit's. Presentation and authoring only (no generator output changes except through the edit the owner chose).

Verify: A
Status: active
Source: M7 A5 (owner, 2026-10-21: the owner may attempt what the engine does not support; it must not crash, must not cost much performance, and must be findable and fixable), VIEW-043, VIEW-044, VIEW-018, ADR 0074

#### Scenario: What is listed
- **WHEN** a world has a brush of 40 raises of 5 m on 100 m, one of 9 light raises 20 km away, one of 3 raises, a hill that is over a capped tile, and a brush of 50 strokes a hundred metres apart
- **THEN** the first two, the hill and the last are listed with the issues above (the 40 raises steep and stacked, the nine stacked only, the hill capped only, the fifty stacked by count), the three raises are not, the steep one is first and the hill last, and a capped tile on the far side of the planet lists nothing

#### Scenario: Reduce
- **WHEN** the steep brush is reduced, then undone, and the stacked one is reduced, and a stamp and a brush made too steep by another item are given to Reduce
- **THEN** the first fits afterwards (every strength scaled by one factor between 0 and 1, the item valid), undo restores the exact definition, the stacked one has half its strengths, the stamp is refused (no strokes) and the other is refused (the other edits), changing nothing

#### Scenario: Merge
- **WHEN** a brush of 12 raises on one place is merged, and a list of strokes with three raises on a place, a flatten and a lower there and a raise far away is merged
- **THEN** the first is one stroke of the summed strength (valid, one undo restores it, a second merge is refused), the list has the three raises as one (the first one's path, the largest radius, the sum), the flatten, the lower and the far raise untouched, and strokes of different kinds, flattens and smooths, and strokes that do not overlap are never merged

#### Scenario: The panels
- **WHEN** the tab is shown with no problem, then with a steep and stacked brush and a capped edit, and its buttons are clicked
- **THEN** it says there is no problem, then each row has its name, its issues and the four buttons, which ask for Fly to, Reduce, Merge, Delete and Select, and Reduce and Merge on the capped edit are disabled and ask for nothing
