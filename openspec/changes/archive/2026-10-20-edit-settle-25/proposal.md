## Why
Window 18 (`docs/perf/window-18.md`) measured the editor's 40 ms settle window at 4 ms of margin for the 1 m, 50 m case (996 of 1,000 ms). opus-planner decided (2026-10-20): the editor's default is 25 ms, still above one 60 Hz drag interval (about 17 ms), so a drag still coalesces. A constant, no ADR change.

## What Changes
- The editor's settle default is 25 ms (STRM-031, TEST-041 default); `REFRESH_SETTLE_MS`.
- `Streamer::refresh` counts a refresh the window holds in its return value, so the edit-latency table's "tiles refreshed / resident" and half/all-refreshed instants are not 0 with the settle on.
- `cargo xtask perf stream` accepts `--settle-ms N` and passes it to the scene.

## Impact
- Spec: STRM-031 and TEST-041 (modified: the default). Code: `streaming` (`refresh` return), `app` (`editwin.rs`), `xtask` (`testcmd.rs`).
- Goldens: none. Perf: re-measured in the next edit-latency window.
