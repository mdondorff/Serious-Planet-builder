## Why
The editor has stamps, lines and regions; the brush is the last E6 tool: raise, lower, smooth and flatten with a radius and a strength in metres (owner requirement; opus-planner, 2026-10-20: E6 editor side). The brush item, its schema and its stage are in main (WDEF-013, AUTH-005 schema 3, GEN-030); the tool only records strokes and files them into brush items as ADR 0070 point 3 says.

## What Changes
- A brush tool per kind (rail buttons, keys 8, 9, 0 and the minus key). Dragging over the planet records the places of the pointer; the release puts the stroke as one command.
- The places are resampled along the great circles to a quarter of the radius, cut into strokes of at most 64 points that each stay within one item's reach (consecutive strokes share a point), and filed: a stroke joins the first brush item this session painted into that is in the layer `brush` and has room (under 64 strokes and 4,096 points) and reach for it, else it starts an item anchored at its first point with a reach of its extent plus the radius, at least 1 km. The items one command starts share a group (consecutive parts). The bound on the change is the exact sum of the strengths. `Editor::add_stroke` is the command; `NewItem` and the group change step carry `strokes`.
- The radius (1 m to 5 km), the strength (0.01 m to 100 m) and, for a flatten, the height it levels towards (typed, else the terrain height under the start of the stroke) are typed in metres in the inspector panel; a stroke keeps the numbers it started with.
- The stroke is drawn as it is painted and a ring of the radius follows the cursor. A brush item has no handle and is not picked from the planet (its strokes start at its anchor).
- VIEW-035; tier-A tests of the geometry, the filing, the numbers, the pointer flow, the panel and the runtime.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves. The height output of a stroke is the brush stage's (GEN-030); the preview does not show a brush (it appears when its exact tiles arrive).

## Out of scope
The look of the egui widgets (tier D); deleting or editing the strokes of a brush item one by one (a stroke is undone with Ctrl+Z); the brush's cost measurements (a perf window).
