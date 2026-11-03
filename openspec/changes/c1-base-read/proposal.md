## Why
- Owner, 2026-11-03: the "pyramid" blocks seen from 300 m are the bilinear bake read: its slope jumps across every 611 m lattice edge (E2b probe: up to 16 deg on the steepest flank, median 0.3-3 deg elsewhere). A C1 read is wanted as a proposal in the erosion-bake preview.
- Inbox #50 (planner, 2026-11-03): build it as a new field beside the old one, so the preview runs both readers and no golden moves.

## What Changes
- `global::read_elevation_c1` (GEN-107): the bake elevation by a separable Catmull-Rom blend of the 4 x 4 samples around a direction; nodes beyond a face edge come from the neighbour face.
- `cpu-erosion-v2` (`HeightFieldKind::ErodedC1`, `ErosionGenerator::field_c1`): the eroded field with that base read; selectable by `world.height_field`; `cpu-erosion-v1` is unchanged and stays the default.
- `height_bounds` of v2 hold the overshoot: the scan window grows by 3 cells, the range by 28.125 % of itself on each side, the slope steps by 4.
- Versions: `READER_C1_VERSION` 1, `EROSION_C1_VERSION` 1, a new key family (the inputs hash of v2 differs from v1). The determinism manifest gains `erosionc1` rows and one header line; v1 rows do not move.
- `WorldTerrain::base_elevation_c1` and two examples (`field_probe`, `c1_cost`) for the evidence.

## Impact
- Code: `generators::{global, terrain (scan_base), erosion, height_fields, manifest}`, `app::worldfield` (one accessor), `app/examples`.
- Requirement: GEN-107 added.
- No golden, known answer or pin of an existing field moves (the C1 field is new). The default field is not switched here: that is a follow-up after the owner sees the preview (planner, 2026-11-03).
