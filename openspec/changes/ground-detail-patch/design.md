## Decisions
- No schema bump: `patch` is an optional key of the existing `[body.<id>.ground]` table, written only when not 0, so a file without it reads and writes as before; a build older than this one refuses a file that has it as an unknown key (it does not say "newer"). Bumping to schema 6 would add a migration and a fixture for one optional number; revisit if the table grows more keys (slice 2 will, and bumps then).
- The shift is in slope-cosine units (the spike tried 0.12 and 0.25); the maximum accepted is 0.5.
- The patch octaves use the same planes, lattice turns and footprint fade as the detail octaves, so tiles and levels read the same noise (REND-034's anchoring); they hash with salts 4 and 5.
- The steepness term is `1 - smoothstep(0.5, 0.86, cs)`; it multiplies the rock's tilt at 2 m by `1 + 2 steep` and at 25 cm by `1 + steep`, only with the patch on.
- Not in this slice: five materials (scree, wet ground), the channels, six octaves, geometry below 100 m.

## Risks
- The patches are 8 and 32 m features: a 1 m view of a few metres sees a part of one patch, so the picture changes in places, by a lot where the boundary moves. The stills at 10 and 50 m show them.
