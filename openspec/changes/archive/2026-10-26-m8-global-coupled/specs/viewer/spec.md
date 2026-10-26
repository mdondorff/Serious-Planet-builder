## ADDED Requirements

### Requirement: VIEW-053 K shows the global coupled run
K SHALL cycle what the climate views show on the lit path (`Command::CycleCoupledLens`): the climate (at the start), the heights of the global coupled run (temperature view), its drainage area (precipitation view), and the climate again. The window asks the world for the lens (GEN-043, STRM-044), which loads or makes the run on a thread; the resident tiles are refreshed once the run is ready, or at once when the lens goes off, so no frame waits and a tile is drawn from its old version until the new one is uploaded (STRM-026). The window title names the lens and where the run stands (working, stored run, run just made, or why there is none); the keys help and the editor's help name K.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c)

#### Scenario: The key
- **WHEN** K is pressed four times in a viewer
- **THEN** the lens is the heights, the area, off and the heights again, no other switch changes, and the help text and the editor's help name K

#### Scenario: Ready, then off
- **WHEN** the lens is asked for on a world, and again switched off
- **THEN** the fields the world builds carry the lens (its climate and mesh key, the base heights) only once the run is ready, and are the plain fields again with the lens off
