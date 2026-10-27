## Why
Window 12 (steady 60 s): 100 m and 1000 m reached only 78 m/s while 300 m reached 480 and 30 m 320. The 78s are trials where a chunk wanted at the start of the window took 9 to 10 s to arrive (5 to 7 s is typical): a slow chunk at one start place, found by a search that assumes the share rises with the speed and that flew every trial over a place of its own.

## What Changes
- One start place for all trials of a steady search (`--places K`, default 2 places per altitude); each place is a search of its own in the log, and the lowest speed is the figure.
- The trace records, for every chunk that landed more than a second after it was first wanted, when it was first asked to prefetch, when a job for it was first seen running (`ChunkManager::is_running`) and when it landed; the log gives the three worst per trial with the wait and the solve, so a late chunk is told apart as asked late or solved slowly.

## Impact
- Spec: TEST-059 (new). Code: `app/envelopeperf.rs`, `app/chunkstage.rs`, `app/streamperf.rs`, `app/lib.rs`, `xtask`, `streaming/chunks.rs` (the accessor only).
