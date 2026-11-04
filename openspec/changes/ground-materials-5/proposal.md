## Why
- I8a slice 2a (planner, 2026-11-04), three PRs in sequence, each golden-neutral: (a) the detail coefficients as look data, (b) six material slots, (c) six octaves. This is step (a).

## What Changes
- The per-material coefficients of the four detail octaves (albedo, grass colour, tilt, the rock's crack depth) move from constants of the terrain shader into the look file: `ground.crack` and `ground.octave.{m16,m2,cm25,cm3}` (LOOK-013, schema version 7). The Earth default is the former constants, so no picture or golden moves.
- The lit plan carries the table (text and key only when not the Earth default); the globals carry 13 more vec4s.

## Impact
- Code: `frame` (look file, plan, materials), `render` (globals, terrain shader). Requirements: LOOK-013 added, LOOK-002 wording (schema 7, the version 7 fixture) edited in the unarchived delta of `ground-detail-patch`, which owns that text; whichever archives later keeps the schema 7 wording.
- Steps (b) REND-056 and (c) REND-057 follow in their own changes.
