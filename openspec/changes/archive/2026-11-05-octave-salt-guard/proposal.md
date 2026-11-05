## Why
- The numerics review of the 8 m lattice (#1186) found one pairing that cannot be avoided (the 8 m and 16 m detail octaves are both turned lattices) and nothing that stops a later salt edit from recreating a worse one: two octaves of neighbouring scales with the same orientation have nested lattices, whose lines are where both noise gradients vanish. Planner, 2026-11-05.

## What Changes
- A tier-A test of the terrain shader's source (`crates/render/src/terrain.rs`, cites REND-057) parses every `gd_octave(..., shift, salt, ...)` call and asserts that no hash salt is used twice, that no two octaves share both their cell size and their orientation (an odd salt is the axis-aligned lattice, an even one the lattice turned by 45 degrees), and that the detail octaves within a factor of 4 in cell size differ in orientation except the pinned pair (shifts 9 and 8, the 16 m and 8 m octaves). A mutation (the 8 m octave back to salt 7) fails it.

## Impact
- No shader, plan or picture change; one test. No requirement is added (REND-057 carries it).
