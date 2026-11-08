## MODIFIED Requirements
### Requirement: GEN-126 The channel block of a compact tile
A compact tile SHALL carry a channel block of 81 words after its driver block, one word per texel of the 9 x 9 grid: bits 0-7 the fill depth code of keep-pits (GEN-120), bits 8-15 the erosion depth code, every other bit zero; a code is `round(255 sqrt(depth / 64 m))` and the neutral word 0. A field that reads baked channels SHALL give both depths per texel and the builder write them; a texel without a depth or with a failed read SHALL be 0, and a slot read back with a reserved bit set SHALL be refused. `CHANNEL_BLOCK_VERSION` and the depth scale SHALL enter the channel identity of a field's climate version word (GEN-119).

Verify: A, C
Status: active
Source: planner inbox #67 (owner, 2026-11-05, ADR 0101, STRM-076), GEN-119, GEN-120

#### Scenario: The channel block
- **WHEN** a compact tile is built over a field that gives fill and erosion depths, over a field without channels, over fields wrapping one, with a failed read at a texel, and its slot is rebuilt from its bytes
- **THEN** each texel holds the square-root codes of its depths with every other bit zero, the field without channels has the zero block, every wrapper forwards the depths, a texel without a depth is 0, the slot round-trips and a set reserved bit is refused, and the keys with channels differ from those without

#### Scenario: The shader reads the block
- **WHEN** a tile holds a uniform channel block and the fill and erosion views are drawn, and a tile holds the neutral block
- **THEN** each view is the CPU mirror's false colour of the decoded depth within 0.003, and the neutral block shows 0 m

### Requirement: GEN-073 The default look ignores the drivers
The default look SHALL give no curve to the soil-depth, moisture, insolation, fill-depth and erosion-depth inputs, so its material weights and albedo SHALL be bit-identical for every driver value, and the images of every committed golden SHALL be unchanged with no candidate; the neutral values matter only once a look gives those inputs curves.

Verify: A, C
Status: active
Source: planner decision 2026-10-26, ADR 0017

#### Scenario: Today's goldens
- **WHEN** the default look's weights are evaluated for several driver vectors and the committed goldens are rendered
- **THEN** the weights are equal bit for bit and every image equals its golden

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the tile's driver block in its reserved inputs (and the fill depth and the erosion depth from the channel block in its two, GEN-126), by the same function and the same input vector as for the other inputs.

Verify: A
Status: active
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank
