## ADDED Requirements
### Requirement: VIEW-066 --channels fills the driver block from the layer's channels
The viewer and the stills SHALL take `--channels on|off` (off by default) which, with `--erosion on`, gives the layer's field the baked channels of the layer's store, the drainage area under pass 1's keys and the deposited depth under pass 2's, so the tiles' driver block holds them (GEN-119) and the `soil` and `moisture` views show them. It SHALL be refused without `--erosion on`, and refused naming the folder when the layer holds no drainage area; the title and the log SHALL say it is on. The default look's picture SHALL not change (GEN-073).

Verify: A
Status: active
Source: planner inbox #65 (2026-11-05), ground-channels-2b, VIEW-062, GEN-119

#### Scenario: The option
- **WHEN** `--channels on` is given without `--erosion on`, with a bad value, with a layer whose folder holds no drainage area, and with the areas copied beside the tiles
- **THEN** the first two are refused naming the option, the third naming the folder, and the last opens with the note in the log and the title, the field's version word changed, its heights those of the layer, and the tile builder handed the channels
