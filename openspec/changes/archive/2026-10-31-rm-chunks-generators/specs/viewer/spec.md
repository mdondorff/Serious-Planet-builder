## MODIFIED Requirements

### Requirement: VIEW-059 Bake blocks are read ahead off the main thread
The window SHALL read the bake blocks under the camera and where it will be in 1, 2 and 4 s on a thread of its own (`WorldTerrain::warm_heights`, at most one batch queued, a full queue drops the batch), so the main thread reads a warmed block without decoding it (CON-22).

Verify: B
Status: active
Source: opus-planner (2026-10-27), owner test 2026-10-27 group A item 2, VIEW-056, CON-22

#### Scenario: A warmed block
- **WHEN** a direction on a block nobody has read is warmed and the warm-up thread has read it
- **THEN** the height read at that direction on the main thread decodes no block, where the same read on a cold block decodes one on the thread that reads it
