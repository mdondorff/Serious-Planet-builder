## Why
F9 writes a bundle and the camera's last minute (VIEW-024) but nothing reads the path back, and the owner cannot tell from the window that the capture worked. opus-planner (2026-10-19): replay the path through the plan, and make F9 say what it wrote.

## What Changes
- `cargo xtask repro frame-<n>-path.txt [--real] [--every N]` replays the path with its bundle (`frame-<n>.bundle`): `test-render --repro BUNDLE --replay-path PATH --out-dir DIR [--replay-every N]` plans the bundle's world once per logged pose, in the logged order, with the bundle's lens, view and LOD parameters, and writes `replay.txt` (time, altitude, nodes, plan hash, finest level, plan time, logged frame time per pose) and, with `--replay-every N`, `pose_NNNN.png` for every Nth pose. The summary counts the poses where the finest level fell while the camera got lower (the tracked LOD-drop bug).
- `cargo xtask repro <bundle>` prints the path file lying beside it.
- F9 shows `PathLog::saved_notice` in the window title for 8 s and prints it with `PathLog::summary()`; a failed write is shown the same way.
- `PathSample::camera` (a pose with a lens), `saved_notice`; VIEW-026; tier-A tests of the row format, the drop finder, the pose camera, the notice, and the file pairing; the real-window editor test replays the path it wrote.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves.

## Out of scope
Replaying the asynchronous streaming (residency, upload timing): a replay plans each pose from nothing, so it shows what the plan chose, not what had arrived.
