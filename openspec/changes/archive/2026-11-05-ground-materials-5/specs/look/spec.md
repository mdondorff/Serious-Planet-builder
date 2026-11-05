## ADDED Requirements
### Requirement: LOOK-013 The octave coefficients of the ground look
The `[body.<id>.ground]` table SHALL take `crack` and the tables `octave.m16`, `octave.m2`, `octave.cm25`, `octave.cm3` (schema version 7) and `octave.m64` and `octave.m8` (REND-057, schema version 9), each with `albedo` (rock, sand, soil), `grass` (three numbers) and `tilt` (rock, sand, grass, soil): the per-material coefficients of the detail octaves (REND-034). The Earth default SHALL be the values the shader held as constants; a number outside 0 to 4, a wrong count or an unknown octave SHALL be refused naming the key. The lit plan SHALL carry the table, in its text and key only when it is not the Earth default, and the shader SHALL read it as f32 (LOOK-001), and LOOK-011's `ground` keys include them.

Verify: A, C
Status: planned
Source: SPIKE-100 D2, planner 2026-11-04 (I8a slice 2a, step a), REND-034, LOOK-011

#### Scenario: The table
- **WHEN** a look file gives a `crack`, a changed `tilt` for one octave, a bad number, a wrong count and an unknown octave, a build that knows version 6 reads the version 7 fixture, and the plans of a file and of the default choice are drawn
- **THEN** the table reads and writes back equal, the bad ones are refused naming the key, the older build refuses the file as a newer schema, the file's plan has a text and key that differ from the plain plan's, the lit ground goldens do not move (the Earth values are the old constants) and zero coefficients draw a picture that differs on over a fifth of the ground
