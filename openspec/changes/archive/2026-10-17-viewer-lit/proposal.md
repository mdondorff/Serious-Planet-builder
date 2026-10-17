## Why
M5 item 5b: the owner wants to see the work in progress. The viewer window still draws the debug path (item 1); the lit path, the atmosphere and the look file (items 3 to 5) were validated and measured but never shown in the window.

## What Changes
- `planet editor` opens on the lit path with the atmosphere (the look's sun, tone curve, units and atmosphere, adaptive exposure on the frame clock's dt). The key L switches to the debug path and back; V cycles the debug views on the debug path (and does nothing on the lit path). Free camera and the scripted flight work in both.
- The title names the path (`lit + atmosphere`, `lit`, or the debug view) and, on the lit path, the exposure: `EV 12.3 (adapted)` or `EV 12.3 (adapting to 14.0)`. The exposure is read back from the GPU where the title is updated (twice a second).
- `viewer_lit_plan` (GPU-free, `frame::viewer`) builds the lit plan of a frame from the body's look, the world's radius, dt and a reset flag; `ViewerState` carries the lit flag and the one-shot exposure snap (first lit frame, return to the lit path, restart of the flight).
- `TerrainRenderer::warm_up_lit`: enables the lit path and the atmosphere and draws the first lit frame (no node) once per ring set, waited, before frame 0. `FirstUse` counts the lit variants recorded. `enable_lit` is not part of `warm_up`, so the window calls it at start whichever path it opens on.
- `--smoke` runs lit with the atmosphere, presses L and V through the command path, and judges counts (lit frames, frames with the atmosphere, debug frames, frames presented per path, exposure readings, and the renderer's own account of the passes it ran); no wall-clock. The no-display rule (VIEW-005) and the headless `--no-window` start check are unchanged. On the software adapter the smoke uses the vertex tile format (see design).

## Impact
Specs: viewer VIEW-003 and VIEW-005 modified, VIEW-007 added; renderer REND-017 modified (lit warm-up and its counters). No golden, digest or plan text changes: the debug-path defaults of `test-render` and the tests stay; lit-by-default applies to the interactive editor window only. No shader changes.

Expected golden changes: none.
