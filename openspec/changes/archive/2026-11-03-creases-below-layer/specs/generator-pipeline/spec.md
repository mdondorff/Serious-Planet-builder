## ADDED Requirements

### Requirement: GEN-109 The crease band
`with_crease_band(CreaseBand)` MAY drop a relief field's crease octaves above the erosion layer's grid (ADR 0098):
- it needs the relief channels on; without the call the field is today's, bit for bit;
- an octave of at most `near` layer cells (2) keeps its weight, one of `far` (4) or more is dropped, between them a smoothstep in the log of its cell;
- above the sea level it blends in over `blend_m` (200 m); below it the creases are today's;
- the bounds take the weighted octaves, and today's where a cap reaches the blend;
- the band enters the keys and the implementation id, not the field version.

Verify: A
Status: active
Source: ADR 0098 (owner, 2026-11-03), ADR 0088 points 2 and 5, GEN-099

#### Scenario: Off
- **WHEN** the field has the relief channels on and the call is not made, or the band keeps every octave, or its sea level is above every ground
- **THEN** every height equals today's field's to the bit, with and without the C1 base

#### Scenario: The weights
- **WHEN** the weight is read at 2 cells, 4 cells, their geometric mean and everything between and beyond
- **THEN** it is 1, 0 and 0.5, non-increasing, and the octaves of the Earth keep octaves 7 to 11, drop octaves 0 to 5 and weigh octave 6 below 0.02

#### Scenario: The sea floor, the land and the shore
- **WHEN** heights are read where the base is below the band's sea level, and along a line across a shore with a blend of 200 m and of 0
- **THEN** the first equal today's field's to the bit, most of the land's differ, and the largest step per half metre across the shore is a third of the hard step's or less, and under 3 m

#### Scenario: Bounds
- **WHEN** random caps, caps on cube edges and caps that reach the blend are bounded, and nested caps are compared
- **THEN** every height, also rounded to f32, lies within its bounds, the width of a cap on land is no more than today's, and an inner cap's bounds lie within the outer cap's, on both bases

#### Scenario: Keys
- **WHEN** the band's cell, cells, sea level or blend change, or it is switched on, and the relief channels are asked for after the band
- **THEN** the inputs hash and the implementation id differ, the late call is refused, and a batch and every thread count give the single reads' bits
