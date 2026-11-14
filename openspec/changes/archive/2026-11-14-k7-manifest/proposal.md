# k7-manifest (TEST-105; briefing item 18, follow-up), tests and manifest only

Architect 2026-11-10 (inbox #102): the stage manifest fixture also runs K7 on one tile (face 1, tile 3/4, 384 cells a face edge, 48-cell tiles), with the probe on. Its line `k7-f1-t3-4` joins `docs/stages/manifest.txt` and the up-to-date check covers it.

## Measured
- The fixture tests (4) run in 0.2 s after the build; K7 adds well under a second, within tier A.
