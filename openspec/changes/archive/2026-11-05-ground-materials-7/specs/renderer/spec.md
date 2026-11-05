## ADDED Requirements
### Requirement: REND-057 The 64 m and 8 m detail octaves
The ground detail (REND-034) SHALL have six octaves: the four of 16 m, 2 m, 25 cm and 3 cm cells and two more of 64 m and 8 m, each with the per-material coefficients of LOOK-013 and the scree and wet entries of REND-056, taken from the look's `ground.octave.m64` and `ground.octave.m8` (look schema version 9). The Earth default SHALL have both new octaves at 0, a look of an older version SHALL read with them at 0, and an octave whose coefficients are all 0 SHALL add nothing to the picture. This supersedes the four-octave wording of REND-034 and LOOK-013.
With both at 0 the picture SHALL be what it was (the exact goldens of the lit ground need no candidate; the claim is at the 8-bit output).

Verify: A, C
Status: planned
Source: SPIKE-100 D2, planner 2026-11-04 (I8a slice 2a, step c), REND-034, LOOK-013, REND-056

#### Scenario: Six octaves
- **WHEN** a look file gives the `m64` and `m8` tables, a bad number and an unknown octave, a build that knows version 8 reads the version 9 fixture, and the version 8 fixture is read; and the ground is drawn with the Earth default, with each new octave given albedo coefficients and given tilts, and with the strength 0 and an octave given; and the goldens of the lit ground are drawn
- **THEN** the tables read and write back equal, the bad ones are refused naming the key, the older build refuses the file as a newer schema, the version 8 file reads with both octaves at 0, each given octave changes the picture, the strength 0 picture is the off picture, and the goldens need no candidate
