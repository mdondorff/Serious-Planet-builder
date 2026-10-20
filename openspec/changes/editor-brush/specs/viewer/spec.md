## ADDED Requirements

### Requirement: VIEW-035 The brush tool
The editor mode SHALL have four brush tools (raise, lower, smooth, flatten; keys 8, 9, 0 and the minus key; rail buttons). With one on, a press over the planet starts a stroke, moving the pointer with the button down records its places, and the release puts the stroke as one command (one undo); Esc during the drag, a tool change and a lost focus put nothing. The recorded places SHALL be resampled along the great circles to a spacing of a quarter of the radius, cut into strokes of at most 64 points that stay within one item's reach (a cell edge less the radius), consecutive strokes sharing the point where they are cut, and filed by `Editor::add_stroke`: a stroke joins the first of the brush items this session painted into that is in the layer `brush` and has room (under 64 strokes and 4,096 points, the sum of the strengths within the limit) and reach (its extent from the item's anchor plus the largest radius), else it starts a brush item (kind `brush`, no parameters, no points) anchored at its first point with a reach of its extent plus the radius, at least 1 km and at most a cell edge, and a bound on the change of the exact sum of the strengths; the bound of an item a stroke joins is raised to hold the sum. The items one command starts SHALL share one group (consecutive parts) when there are several. The brush radius SHALL be 1 m to 5 km and the strength 0.01 m to 100 m, typed in metres in the inspector panel, a value outside is refused with the range and changes nothing, and a stroke keeps the numbers it started with. A flatten SHALL level towards the typed height, else the terrain height under the start of the stroke; the other kinds have no target. A brush item SHALL have no handle and SHALL NOT be picked from the planet. The stroke being painted and a ring of the brush radius under the cursor SHALL be drawn.

Verify: A
Status: active
Source: owner (E6: raise, lower, smooth, flatten with metric radius and strength), opus-planner (2026-10-20), ADR 0070, WDEF-013, AUTH-005, GEN-030, VIEW-022

#### Scenario: A drag, then another stroke of the same area
- **WHEN** a raise stroke is dragged over 300 m and a lower stroke is dragged 100 m away, then the second is undone
- **THEN** the first is one brush item with one stroke, the second joined it (two strokes, bound 10 m), and the undo leaves the item with its first stroke exactly

#### Scenario: A long drag
- **WHEN** a 12 km stroke is dragged with a 200 m brush
- **THEN** it becomes linked brush items of one group in one command, no stroke over 64 points, each stroke starting at the point where the one before ended, and one undo removes them all
