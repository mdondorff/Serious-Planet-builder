## Why
Owner bugs (a sea edge that ends at a tile, an atmosphere colour jump) are hard to replay: a repro bundle holds one pose, and the window cannot write a bundle at all. opus-planner (2026-10-19): a rolling log of the camera's last minute, written with a bundle by F9, in the viewer and in the editor mode.

## What Changes
- `planet_frame::pathlog::PathLog` (GPU-free): a ring of the last 60 s of camera poses, one about every 100 ms of the window's clock (position and orientation in f64, altitude, frame time); a text format (`planet-path 1`) that reads back bit for bit; a summary for people.
- F9 in the window (`Command::SaveRepro`, viewer and editor mode alike): writes `frame-<n>-path.txt` and `frame-<n>.bundle` (a terrain repro bundle of this frame's request over the world as the window holds it, edits included; `cargo xtask repro` replays it) to `target/review/`, or to `PLANET_REPRO_DIR`.
- The smoke script presses F9 once where `PLANET_REPRO_DIR` names a directory; the summary reports the writes.
- VIEW-024 and tier-A tests of the ring, the format, the summary and the key; the real-window editor test checks the two files.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves.

## Out of scope
The replay of the path through the streaming plan (`cargo xtask repro <path file>`), the next change.
