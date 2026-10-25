## Why
The owner's editor (release, quiet machine) stalls: Windows reports "not responding"; the main thread alternates between Wait and Running (131 s of CPU) while one unnamed thread runs a full core (186 s) and the pool workers wait. There is no debugger on the machine, so the next stall must name its culprit itself (opus-planner, 2026-10-25).

## What Changes
- A stall probe (`app/stall.rs`): `App::frame` marks the end of every phase (the coupled lens, the edit republish with `set_definition` and `field`, the field rebuild, input and commands, camera and ground heights, `begin_frame`, the chunk frame and editor update, the wait for the GPU and the lit plan with the water, the horizon frame and upload, the render submit, the title and the exposure readback, the editor panels and present). A frame of a second or more writes the report `stall (the window went on)` to `target/review/crashes` (at most one in ten seconds, the others one log line) with the phases longest first, the locks the main thread waited on (2 ms or more), the tile streamer's jobs in flight and the chunk and horizon job counts, and the title shows the stall and the longest phase.
- The frame path's locks are timed acquires (`stall::read`, `stall::write`, `stall::lock`): the definition lock, the chunk stage, the horizon jobs and the coarse and coupled run slots.
- Every thread the program starts for the frame, the streaming pool and the generators' scoped workers has a name (`planet_core::thread::spawn_named`; `pool-worker-N`, `heights-worker`, `bake-block`, `horizon-bake`, ...): an unnamed busy thread is then the driver's or a library's.

## Impact
- Spec: VIEW-056.
- Code: `app/stall.rs` (new), `app/window.rs`, `app/worldfield.rs`, `app/horizonworld.rs`, `app/coupledworld.rs`, `core/thread.rs` (new), the spawn sites of `generators`, `cache`, `frame`, `streaming`.
- The probe reads only the clock and a thread-local; it changes no decision and no result.
