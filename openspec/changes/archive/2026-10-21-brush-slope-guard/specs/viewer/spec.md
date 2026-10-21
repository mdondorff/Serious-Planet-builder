## ADDED Requirements

### Requirement: VIEW-043 The brush keeps the ground it makes drawable
`LodParams::drawable_slope` SHALL be the steepest slope (rise over run) that a tile's neighbourhood can have and still leave the leaf a morph interval (LOD-008): with `rho = hypot(r, H)` for the node radius bound `r` and the half range `H` of a neighbour's height bounds, the interval's start `split(L) + 2 rho` stays within 95 % of the end `split(L - 1)`, which gives `H / r` (about 2.5, 68 degrees, at the defaults; the same at every level; 0 where the parameters leave no room even on flat ground).

The editor's brush SHALL limit the stacked gradient of its raise and lower strokes:

| Quantity | Value |
|---|---|
| Limit | `gradient_limit = max(0, drawable_slope - BASE_SLOPE_SHARE)`, `BASE_SLOPE_SHARE` 1.0 (the share left to the base and the other edits), about 1.5 at the defaults |
| Gradient of one stroke | at most `STROKE_GRADIENT s / r` for strength `s` and radius `r`, `STROKE_GRADIENT` 1.5396 (the largest of `4 x (1 - x^2)`); a raise and a lower by magnitude; a flatten and a smooth not counted |
| Stack at a point | the sum over the strokes whose disc reaches the stroke being painted (conservative: the maxima rarely coincide) |
| Where it is read | on the pieces the stroke is filed as (resampled to a quarter of the radius, cut at 64 points); a stroke of several pieces counts its own gradient twice (the pieces share their cut point at full weight) |
| A raise or lower over the limit | filed with the strength that fits (`slope::allowed_strength`), status "<kind> stroke clamped to <s> m: the ground would be too steep to draw (<asked> m was asked)" |
| No room (under the smallest strength of the brush) | refused, status "<kind> stroke not painted: the ground there is already too steep to draw", nothing put |
| Item file, schema, items already filed | unchanged |

- The session SHALL take the drawable slope from the window's LOD parameters when the editor is created (`Session::set_drawable_slope`).
- A brush item (or a linked group) whose stacked gradient over the points of its own strokes is over the limit SHALL carry the warning "too steep to draw fully" (`slope::WARNING`) in the inspector and in its row of every list (nearby, search, apply order, layers: `Inspector::warning`, `Row::steep`), found once per revision of the editor.

The guard does not cover, and the warning does not find:
- the base terrain, stamps, lines and regions (the share left to them is a constant, not derived), and what a flatten onto a far target or a smooth does at its edge;
- strokes filed before the guard or from older files (flagged, not clamped), and a redo after other strokes were added (not checked again);
- a lower on a stack over the limit (it counts by magnitude: the way back is undo or deleting the item);
- a change of the window's LOD parameters after the editor is created; the preview of a stroke in progress, which shows the typed strength where the release may clamp it.

Verify: A
Status: active
Source: opus-planner (2026-10-21), SPIKE-26, ADR 0074, ADR 0070, VIEW-035, LOD-008

#### Scenario: The limit follows the parameters
- **WHEN** the drawable slope is read at the production parameters, at a smaller tolerance and where there is no room
- **THEN** it is about 2.5, larger for the smaller tolerance and 0 where there is none, a neighbour of half range `slope * r` leaves the interval its 5 % and one a thousandth more does not, at every level, and the guard's limit is that slope less the share for the base

#### Scenario: A stack of raise strokes
- **WHEN** raise strokes of 5 m and 100 m radius are painted on one spot until the limit, then once more, then again
- **THEN** the strokes up to the limit are filed as painted, the next is filed with a smaller strength and the notice, the stack stays under the limit at the spot, the one after is refused with the reason, and undo takes the clamped stroke back

#### Scenario: What counts
- **WHEN** a lower is painted on a full stack, a lower 10 km away, and a flatten and a smooth on the stack
- **THEN** the lower on the stack is held back as a raise is, the one far away is filed as painted, and the flatten and the smooth are filed as painted without the notice

#### Scenario: A long drag
- **WHEN** a drag given by two raw points passes 120 m from a stack of three strokes, and a drag of 600 m and a click are painted with the same strength on empty ground
- **THEN** the drag is clamped along its resampled path, and the long drag gets about half the strength of the click (its pieces count twice at their joints)

#### Scenario: An item already too steep
- **WHEN** 40 strokes of 5 m and 100 m radius are filed on one spot without the guard, and a single stroke elsewhere
- **THEN** the stack's item carries the warning in the inspector and its row of the nearby, search, apply-order and layer lists, and the other does not

## MODIFIED Requirements

(VIEW-035: "a stroke keeps the numbers it started with, except that a raise or lower is filed with the strength that fits the slope guard (VIEW-043)".)
