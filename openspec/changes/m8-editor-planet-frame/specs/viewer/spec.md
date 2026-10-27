## ADDED Requirements

### Requirement: VIEW-059 The editor's hover and the camera's ground read what a pixel can show, and bake blocks are read ahead off the main thread
The editor's hover pick, its draft outline and its brush ring SHALL read the height of the tile level whose vertex spacing is about one pixel of the view at the camera's height over the surface (`EditRuntime::pick_level`, level 3 at the coarsest), not the finest level; a click that places or drags an edit SHALL still read the exact surface. The ground height under the camera SHALL read the same level. The window SHALL have the bake blocks under the camera and where it will be in 1, 2 and 4 s read on a thread of its own (`WorldTerrain::warm_heights`, at most one batch queued, a full queue drops the batch), so that a block that was warmed is read on the main thread without decoding it (CON-22).

Verify: A
Status: active
Source: opus-planner (2026-10-27), owner test 2026-10-27 group A item 2, VIEW-056, CON-22

#### Scenario: A pick at the planet view
- **WHEN** the camera is 5000 km over the surface of a 1440 px high view with a 90 degree field of view
- **THEN** the pick level is 6 or coarser, and at 10 m it is the finest level of the pyramid

#### Scenario: A warmed block
- **WHEN** a direction on a block nobody has read is warmed and the warm-up thread has read it
- **THEN** the height read at that direction on the main thread decodes no block, where the same read on a cold block decodes one on the thread that reads it
