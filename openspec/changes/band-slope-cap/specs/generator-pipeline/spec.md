## ADDED Requirements
### Requirement: GEN-179 The band octaves are capped by what the base slope can drain
When the band rule is on (`BandRules::slope_cap_c` above 0), each octave of the class band SHALL have its height held to `c x slope(e0) x wavelength / 2`, with the slope of the base elevation `e0` taken from two forward differences of `e0` at the step `SLOPE_STEP_M`; an octave below its cap SHALL be unchanged. The mountain band's centre SHALL fall with the capped octaves, so the band stays zero-mean. A second rule (`BandRules::hill8_weight` below 1) SHALL weight the 8 km hill octave. With both rules off every elevation and every key SHALL be the bits of the build before this requirement; with a rule on, the inputs identity SHALL change with each rule and its value.

Verify: A
Status: planned
Source: Architect, 2026-11-12 (SPIKE-109 verdict: the start-surface fix at the source, default off)

#### Scenario: A plain with a small slope
- **WHEN** the cap is on and the base slope under a hill band is small
- **THEN** the band's octaves are held to what that slope drains across half a wavelength, and a larger `c` never lowers the band

#### Scenario: Details of the rules
- **WHEN** the rules are read
- **THEN** the octave wavelengths are 16, 8, 4, 2 and 1 km (ridged) and 8, 4 and 2 km (hill) on a body of Earth's radius and scale with the radius
- **AND** the signals of the octaves are those of the band without a rule; a cap that never binds gives the same bits as no cap
- **AND** the rules are process-wide switches like the turned lattices (GEN-140): a run sets them once at its start, before any field is read
