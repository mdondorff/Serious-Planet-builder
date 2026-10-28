## Why
The stage-off gate at frame-102263 (planner, 2026-10-28) fails on frames of 100 to 360 ms. Measured (instrumented builds; decode counts from 3 gate runs): in 22 of 29 slow phases that decoded a bake block on the main thread, the phase was the editor update's hover pick (`pick_surface` along a ray 230 to 2100 km long, one cold height read of 30 to 125 ms; the same pick rerun takes 1 ms), 4 were "plan and residency (begin_frame)" and 3 "camera, ground heights and the globe overview". Rule (planner): no main-thread call decodes a bake block.

## What Changes
1. `HoverPicker` (`app/hoverpick.rs`): the hover pick on a thread of its own; the frame polls the last finished hit and starts the next pick, a clear drops a pick in flight. `EditRuntime::set_hover_source`; the window gives it a source that makes `world.field()` and picks. Clicks keep the synchronous exact pick.
2. The stall probe counts the blocks the main thread decoded per phase (`thread_block_decodes`), and a report names the phases with a count.
3. The remaining main-thread decodes (begin_frame, camera ground) are reported with their counts; see the design notes for what was done about them.

## Impact
- Spec: VIEW-061 (new). Code: `app/hoverpick.rs`, `app/editwin.rs`, `app/window.rs`, `app/stall.rs`. Tests: tier A (`hoverpick`, `editwin`, `stall`).
- Measured: the stage-off gate at frame-102263, runs before and after.
