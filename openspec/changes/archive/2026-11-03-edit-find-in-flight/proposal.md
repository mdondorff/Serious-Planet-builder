## Why
The edit-latency smoke (`gpu_edit_perf`) did not settle with 5 tile workers: "kind Stale, substituted N, unfinished jobs 0, pending 0". With ADR 0092 PR 3 (#1117), the default pool on the reference machine is the terrain share (6, so 5 tile workers beside the plan worker), so it failed there every time. It passed at 6 and 18 workers. Earlier "did not settle under load" failures of the same test were the same race.
- Every edit's footprint find (the window, the edit scene, the coupled-lens switch) ran over the resident tiles only.
- A tile whose job was in flight when the definition changed had already read the old field. It landed resident with the old key, nothing marked it stale, and the plan wanted the new key, so the frame stayed Stale for good.
- A larger pool finishes those jobs before the edit, which hides the race. It is timing, not a thread-count-dependent result (CON-14 holds).
- It is not the stale-find bug of `docs/learnings.md` ("The editor's find named a tile only when the edit's reach touched it", edit-stale-diagnostics); that one missed resident neighbours.

## What Changes
- `ManagedLoop::edit_candidates()`: the resident tiles and those whose job is in flight (`Streamer::in_flight_tiles`), sorted.
- The window's edit and coupled-lens refresh and the edit scene's finds run over it. `Streamer::refresh` already stops a named in-flight job and loads the tile again from the new field.
- A request still queued in the streamer is not named: its job reads the field when it starts (`MeshSource::load`, `load_staged`), and a job handed to the spawner (`queue_ahead`) is already in flight.
- STRM-071 states the rule.

## Impact
- `crates/streaming/src/lib.rs`, `crates/frame/src/managed.rs`, `crates/app/src/window.rs`, `crates/app/src/editperf.rs`; tier-A tests in `crates/frame/tests/managed.rs`.
- No core, generator or shader code; no result changes for a tile that was resident at the edit.
