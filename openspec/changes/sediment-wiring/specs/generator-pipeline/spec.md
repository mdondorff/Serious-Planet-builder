## ADDED Requirements
### Requirement: GEN-151 The sediment steps run after the rounds on a compact landmass
The sediment pass SHALL run thermal, the deposition on the surface's own route and the one-cell pit spreading on a compact landmass after the rounds, give the bits of the masked cube grid, satisfy eroded = deposited + exported to 1e-9 relative, and keep the volume over thermal and over the pit spreading to 1e-9 relative.
- The report SHALL give the pits before the steps and after each step.
- The deposition's change of the height volume SHALL equal the volume it reports as deposited to 1e-9 relative.
- `breach_lowered_m3` SHALL give the volume the breach alone lowers on the surface it is given, in cubic metres; the deposition counts it as eroded load when its original surface is the one before the breach (the departure of GEN-148). The share of the eroded column is a quotient of this volume and the sweep's, reported by the example.

- The run of `examples/landmass_scale sed` (the prototype landmass) is a measurement, not a gate; the gate is the tier-A test.
- Gate G-SED (per catchment and per round) is not closed by this requirement: the test and the run take the whole landmass once.

Verify: A
Status: planned
Source: A3 section 6.7 (the order after K7's pass, A3 line 385), gate G-SED, inbox #104, briefing item 10

#### Scenario: a cap of a small body after two rounds
- **WHEN** a landmass over a cube corner (24 cells a face edge, a body of 4,000 m radius) is carved for 2 rounds and given the sediment pass, on the compact grid and the masked cube grid
- **THEN** the heights are the same bits, eroded equals deposited plus exported to 1e-9, the volumes over thermal and the pit spreading change by under 1e-9, and thermal moves volume
