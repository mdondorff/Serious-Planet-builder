## Why
- I8a slice 2a (planner, 2026-11-04), step (c) of three, golden-neutral: the ground detail gets two more octaves (SPIKE-100 D2), 0 in the Earth default. Steps (a) (#1176) and (b) (#1181) made the coefficients look data and added the scree and wet slots.

## What Changes
- Six octaves: the four of REND-034 (16 m, 2 m, 25 cm, 3 cm) and two new ones, 64 m (patches of the ground) and 8 m (mounds, the rock's faces), each with the per-material coefficients of LOOK-013 and the scree and wet entries of REND-056.
- The look file takes `ground.octave.m64` and `ground.octave.m8` (look schema version 9, identity step). The Earth default has both at 0; the shader skips an octave whose coefficients are all 0.
- A look sheet with the new octaves on and their cost as a same-run ratio (octaves on versus off, same pose), labelled indicative.

## Impact
- Code: `frame` (materials, look file), `render` (terrain shader and globals). Requirements: REND-057 added; LOOK-002 wording (schema 9, the version 9 fixture) edited in the unarchived delta of `ground-detail-patch`, which owns that text.
