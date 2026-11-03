## ADDED Requirements

### Requirement: GEN-109 The crease band
A relief field MAY drop the crease octaves above the erosion layer's grid (ADR 0098): `with_crease_band(CreaseBand)`, valid only with the relief channels on; without the call the field is today's, bit for bit. At or above the band's sea level an octave whose lattice cell (`radius / (256 2^o)`) is at most `near` layer cells keeps its weight, one of `far` cells or more is dropped, and between the weight falls by a smoothstep in the log of the cell (defaults 101.8 m, 2 and 4); below it the creases are today's. The bounds SHALL take the weighted octaves only, joined with today's where a cap straddles the level; the band SHALL enter the keys and the id.

Verify: A
Status: active
Source: ADR 0098 (owner, 2026-11-03), ADR 0088 points 2 and 5, GEN-099, inbox #54

#### Scenario: Off
- **WHEN** the field has the relief channels on and the call is not made, or the band keeps every octave, or its sea level is above every ground
- **THEN** every height equals today's field's to the bit, with and without the C1 base

#### Scenario: The weights
- **WHEN** the weight is read at 2 cells, 4 cells, their geometric mean and everything between and beyond
- **THEN** it is 1, 0 and 0.5, non-increasing, and the octaves of the Earth keep octaves 7 to 11, drop octaves 0 to 5 and keep a hundredth of octave 6

#### Scenario: The sea floor and the land
- **WHEN** heights are read where the base is below and at or above the band's sea level
- **THEN** the first equal today's field's to the bit, and nine in ten of the second differ

#### Scenario: Bounds
- **WHEN** random caps, caps on cube edges and caps that straddle the band's sea level are bounded, and nested caps are compared
- **THEN** every height, also rounded to f32, lies within its bounds, the bounds of a cap on land are no wider than today's, and an inner cap's bounds lie within the outer cap's

#### Scenario: Keys
- **WHEN** the band's cell, cells or sea level change, or it is switched on
- **THEN** the inputs hash and, on and off, the implementation id differ; a batch and every thread count give the single reads' bits
