## Why
The line and region tools draw (VIEW-029, VIEW-031) but the inspector shows nothing of them: a ridge's height and half-width, a region's surface type and mask have no rows (opus-planner, 2026-10-20: E5 stage 2b-2). The owner's mask rule: a region clips a chosen layer's edits, inside, like a layer mask; the outside mode is refused by the world definition for now.

## What Changes
- Ridge and channel have parameter rows in the inspector: their height or depth (`amount_m`, 0 to 1,000 m and 0 to 500 m) and their half-width (`width_m`, 1 m to 5 km), with ranges, typed like any value.
- A typed amount or width changes every part of a split line as one command (`Editor::set_line_params`): the parts' reach is made again (a wider line is split again when it must be) and the bound on the change is raised to hold the amount; a width that leaves no room is refused and nothing changes.
- A region has two more rows (`RegionRows`): a combo box over the nine surface types of the table (`planet_world_def::region::SURFACE_TYPES`, not copied) and a check box "clips another layer, inside the outline" with a combo box of the other layers. `Session::set_surface_type` and `Session::set_mask` change the item in one command each; the mask mode and the mask layer are never set apart (the world definition refuses a clipping region without a layer and a labelling one with a layer); a region cannot clip its own layer.
- `Session::place` refuses the line kinds (they are not stamps).
- VIEW-033; tier-A tests of the rows, the typed values, the surface type and the mask, the panel events and the runtime.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves.

## Out of scope
The outside mask mode (refused by the world definition, openspec/changes/region-mode1); the layer list's own rows for lines and regions; the look of the egui widgets (tier D).
