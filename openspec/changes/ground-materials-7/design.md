## Decisions
- The two octaves are appended to the table (names `m64`, `m8`); the first four keep their places, so every older file and plan reads as before. Cells 2^11 and 2^8 of 1/32 m (64 m, 8 m), hash salts 6 and 7 (4 and 5 are the patch octaves).
- Coefficient words: octaves 4 and 5 are vec4s 16 to 23, the crack word moves from index 16 to 24 (25 vec4s in all). The sums add the new octaves after the old ones, so an off octave adds nothing and is skipped by a uniform branch when its four words are all 0.
- The cells are not those of SPIKE-100 D2 (50, 12, 3, 0.75, 0.2 m, 3 cm); they extend today's grid with the two scales the spike's roles lack (broad albedo patches, rock faces) at powers of two of the shader's lattice. The look sheet decides whether to move them.
- Cost: reported as a ratio of the terrain pass with the two octaves on against off, same pose and run (indicative until the machine restart, 2026-11-07); the GPU budget decision waits for the owner (REND-034).
