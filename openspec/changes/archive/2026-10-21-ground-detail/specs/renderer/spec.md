## ADDED Requirements

### Requirement: REND-034 Procedural ground detail
The lit terrain shader SHALL add a procedural detail term to the albedo and the shading normal, driven by the plan's strength (LOOK-009) and the material weights (LOOK-008): six noise octaves with cells of 32 m, 8 m, 2 m, 0.5 m, 12.5 cm and 3 cm, an octave fading out while a pixel covers more than 0.1 to 0.4 of its cell. The octaves SHALL be anchored to the world: a node record holds the integer cell of the tile origin at 1/32 m and the fraction, both decided in f64 on the CPU, in the unused fourth lanes of its grid phases (the record does not grow), and an octave's cell is that integer shifted right by two bits per octave, so two tiles (or two levels of detail) read the same noise at the same ground point; the cell is an exact integer used only for shifts and hashing and no planet-radius float is formed in the shader (CON-11), and the cell range is 6.7e7 m from the centre. With the strength 0 the picture SHALL be what it was before the term, bit for bit; the climate and lighting-only views SHALL not show the albedo term. Per material the procedural term is one source of the albedo multiplier and the normal tilt: a later texture source (a close-range photo layer) SHALL be able to replace or blend into it without a change of the weights function or the node record. Presentation only: heights, collision and picking are untouched (ADR 0028).

Verify: A, C
Status: active
Source: M7 G2a (opus-planner, 2026-10-21), SPIKE-27 option A, owner decisions of 2026-10-21 (the photo layer later, with a seam), ADR 0035

#### Scenario: The anchor
- **WHEN** two node records have origins 64 m apart, one at a negative planet-radius coordinate
- **THEN** their cells differ by 2048 at 1/32 m and by the matching shift at every octave, their fractions are equal, and each record's cell plus fraction recovers its origin to 1e-6 m

#### Scenario: Off, on, and the table
- **WHEN** a ground-level scene is drawn with the strength 0 (with any material table), with 1, with a table of no material, with a table of rock alone and of soil alone, in the temperature and precipitation views, and with the tiles in other slots
- **THEN** the strength 0 pictures are equal whatever the table; the strength 1 picture changes more than a fifth of the ground by more than 2 %, is equal on a second render and keeps its mean brightness within a factor of two; the no-material table equals the off picture; rock alone and soil alone differ on more than a fifth of the ground; the climate views equal their off pictures; other slots give the same picture
