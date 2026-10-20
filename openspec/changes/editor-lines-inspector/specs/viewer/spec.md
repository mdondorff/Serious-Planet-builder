## ADDED Requirements

### Requirement: VIEW-033 The inspector of a line and of a region
The inspector SHALL show a ridge's and a channel's `amount_m` (labelled Height and Depth; 0 to 1,000 m and 0 to 500 m) and `width_m` (Half-width; 1 m to 5 km) as typed rows with their ranges. A typed `amount_m` or `width_m` SHALL change every part of the line the selected item belongs to as one command (`Editor::set_line_params`): the parts are made again from the joined points of the line, so their reach covers the new width (a line that must have more parts is split again), the bound on the change is raised to at least the amount, and a value out of its range, a number that cannot be read, or a width that leaves no room (`WidthTooLarge`) is refused with the reason and changes nothing. For a region the inspector SHALL show a surface type (a choice among the names of `SURFACE_TYPES`, stored as the index) and whether it clips another layer inside its outline with the layer it clips (any other layer name; its own is refused): picking a surface type SHALL be one command, and setting the mask SHALL change the mask mode (2 or 0) and the mask layer (set or none) together in one command, so that no state in between reaches the world definition; clipping without a layer SHALL be refused with "choose the layer the region clips". The mask mode outside (1) SHALL NOT be offered. The line kinds SHALL NOT be placed as stamps.

Verify: A
Status: active
Source: opus-planner (2026-10-20), ADR 0069, VIEW-017, WDEF-011, WDEF-012, VIEW-028

#### Scenario: A typed width on a split line
- **WHEN** a 150 km ridge of four or more parts is selected at its third part and a half-width of 800 m is typed, and then undone
- **THEN** every part has the half-width 800 m and a reach that covers it, it is one command, and one undo restores the former line exactly

#### Scenario: The mask of a region
- **WHEN** a region is set to clip the layer "stamps" and then to label only
- **THEN** the first change sets the mode 2 and the layer together, the second sets the mode 0 and removes the layer, and clipping with no layer or with the region's own layer is refused and changes nothing
