## MODIFIED Requirements
### Requirement: GEN-074 The tile driver block and the driver views
A compact tile SHALL carry a driver block of 9 x 9 words (GEN-025): at each climate texel the neutral soil depth, moisture and insolation codes, and the neutral insolation of the texel's latitude in the top byte (neutral unless the field reads baked channels, GEN-119). The fragment stage SHALL read the block bilinearly, blend it as `neutral + w (value - neutral)` with `w` 1 (GEN-071), and fill the reserved material inputs (GEN-072). The debug views `soil`, `moisture` and `insolation` (`ShadeView` 7 to 9, reached by J) SHALL show the blended drivers in false colour.

Verify: A, C
Status: active
Source: ADR 0076 point 5, GEN-071, GEN-072, GEN-025, ADR 0086, GEN-119

#### Scenario: The neutral block
- **WHEN** compact tiles are built over a field
- **THEN** every texel of the block holds the neutral codes of its latitude

#### Scenario: The views
- **WHEN** a compact tile whose driver block is uniform, and one whose block is a ramp, are drawn in each driver view
- **THEN** every covered pixel has the colour the CPU mirror gives for those codes (within 0.003 for the uniform block), and a node with no fade plane reads the block's values at full weight

#### Scenario: Constants
- **WHEN** the block is decoded
- **THEN** the soil code uses the default `soil_max_m` of 8 m in the builder and the shader alike

## ADDED Requirements
### Requirement: GEN-119 The driver block from the baked channels
A field MAY give the tile builder the baked channels at the 81 texels of a tile's driver block (`HeightField::driver_channels`). The builder SHALL then write, at each texel that has them, the deposited depth as the soil code and the drainage area as the moisture code (a linear ramp of its log2 in cells, 0 at 0 and 1 at 10, `DRIVER_MAP_VERSION`), keeping the insolation codes; a texel without a channel stays neutral. A field without channels SHALL give the neutral block, and every wrapper of a field SHALL forward them. A field that reads channels SHALL fold `DRIVER_MAP_VERSION` and the channels' identity into its climate version word, so its mesh keys differ from the same field without them and no other field's key changes (no mesh version bump, GEN-018).

Verify: A, C
Status: active
Source: planner inbox #65 (2026-11-05), ground-channels-2b decision 1, STRM-075, GEN-074, ADR 0076, ADR 0086

#### Scenario: Channels in the block
- **WHEN** compact tiles are built over a field that gives channels, over the same field without them, and over fields wrapping one, and the driver views of the tiles are drawn with the default look
- **THEN** each texel holds the soil and moisture codes of its channels with the insolation of the neutral block, the field without them has the neutral block and its old keys, the keys with channels differ from those without and between channel identities, every wrapper forwards both, the soil and moisture views show the channels, and the lit picture is the one it was
