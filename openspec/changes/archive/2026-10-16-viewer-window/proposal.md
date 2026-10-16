## Why
M5 item 1 (owner's first M5 item, ADR 0042 point 4, ADR 0015): the first on-screen window, so the terrain can be looked at live (also over remote desktop) and the cold start measured once. The owner added on 2026-10-16: an adapter gate that picks a discrete GPU and refuses anything else before a device exists, and a free camera on a key.

## What Changes
- `planet editor` opens a `winit` window. `render` owns the surface (`render::window`, `raw-window-handle`, no wgpu type in the call), blits the terrain renderer's colour target into it keeping the aspect, falls back vsync-off from Immediate to Mailbox to Fifo, and reports the present mode.
- The window runs the pipelined loop (`Rig`, `ManagedLoop`) with the scripted orbit-to-1 m flight (the `dive` script) on the injected clock, on today's debug path. Keys: pause, restart, debug view cycle, vsync on/off, flight or free camera.
- The title shows frame rate, CPU frame time, GPU time (one frame a second is timed) and the readiness report (ADR 0045 point 5).
- Adapter gate: `frame::adapter` (GPU-free: descriptions in, a choice or a refusal naming every reason out); `render::gate` enumerates and describes the adapters. `GpuContext::new(HighPerformance)` and the window go through it; the software adapters (WARP, lavapipe) stay for test and CI mode.
- Free camera: `frame::freecam::free_step`, one pure function from a pose, the frame's input and dt to the next pose (parallel-transported heading, pitch from the vertical rebuilt each frame, clamped short of 90 degrees); its input already carries a look delta in radians for a mouse later (item 1b).
- `editor --smoke` opens, drives and closes the window path (tier C, `tests/gpu_window.rs`); `editor --smoke --no-window` remains the display-free start check (BUILD-004). Where CI has no display the smoke test is skipped with a declared reason (`PLANET_NO_DISPLAY`), never silently.
- New dependencies: `winit` 0.30.13 (app), `raw-window-handle` 0.6.2 (render).

## Out of scope
No egui, no mouse look (item 1b after SPIKE-11), no lit path (item 2), no resize of the internal resolution (the frame is scaled into the window).
