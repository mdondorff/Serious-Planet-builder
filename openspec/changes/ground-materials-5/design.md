## Decisions
- The shader keeps its arithmetic and its fixed tints (rock, sand, soil each times a constant tint); only the coefficients are data, so the Earth picture stays bit for bit (checked on the goldens of the lit ground).
- Per octave the table holds the albedo of rock, sand and soil, the grass colour (a vector: the 25 cm octave's is not grey) and four tilts; the 16 m octave gets a tilt row too (0 by default) so a look can tilt it, at the cost of one dot and one multiply-add.
- The rock's tilt growth with the steepness (the patch break-up, REND-055) stays in the shader as a factor on the table's value.
- Range 0 to 4 for every coefficient; the Earth values are at most 0.34.
- Next steps: the six slots (REND-056) widen the weights and the tables; the six octaves (REND-057) add rows.
