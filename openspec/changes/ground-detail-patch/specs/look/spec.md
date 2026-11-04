## ADDED Requirements
### Requirement: LOOK-012 The patch key of the ground look
The `[body.<id>.ground]` table of the look file SHALL take an optional `patch`, the patch break-up of the material boundaries (REND-055) as a share of the slope cosine, from 0 (the default, off) to 0.5; a value outside it SHALL be refused naming `ground.patch`. The canonical text SHALL write the key only when it is not 0, so a file made before it reads and writes as it was (no schema version change). The lit plan SHALL carry the value, in its text and key only when it is not 0, take it from the file's table with the file's curves (VIEW-060), and the poses file's `[look] ground_patch` SHALL set it for the stills.

Verify: A
Status: planned
Source: REND-055, LOOK-011; planner 2026-11-04

#### Scenario: The key
- **WHEN** a look file gives `patch = 0.12`, none, 0.7 and -0.01, and the plans of a file, of the default choice and of a poses file are built
- **THEN** 0.12 reads and writes back equal, none is 0 and writes no key, the others are refused naming `ground.patch` with the value, the file's plan carries 0.12 with a text and key that differ from the plain plan's, and the default choice keeps 0
