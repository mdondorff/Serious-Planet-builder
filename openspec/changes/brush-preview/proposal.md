## Why
A brush stroke shows nothing until its exact tiles arrive (the preview skipped brushes); the owner values visible feedback while painting (opus-planner, 2026-10-20: E6, item 2).

## What Changes
- A raise or a lower stroke is previewed as a chain of hill preview stamps along its line, a flatten as plateaus at its target; a smooth is not previewed (a known limit: it moves the ground towards the mean of what is around it, which the stale tile cannot give). `preview::brush_chain`; the hill profile is the brush's own `(1 - x^2)^2`; the stamps reach 1.118 times the radius so that the cross profile of a line of them matches the stage's, a dab of half the strength on each end rounds it; at most 16 stamps per stroke.
- `preview_delta_on` (with the body's radius) includes the brushes: a brush item added or removed is its chains, one that gained or lost strokes at the end is only those strokes' chains. `preview_delta` (no radius) leaves brushes out as before.
- A transient preview channel (`ManagedLoop::set_transient_preview`, `draw_plan_transient`): stamps of something being made flag every node they touch, stale or exact; the editor gives it the chain of the stroke under the pointer every frame, and an empty list once when the stroke ends (the put stroke's delta takes over).
- `MAX_PREVIEW_STAMPS` 8 to 32 (the globals' preview table 16 to 64 vec4s, 1,024 bytes); the vertex stage already loops over the active count, so an empty or short list costs what it did.
- The edit-latency scene prints the preview's cost with 0, 8 and 32 active stamps on every flagged node (flagged nodes, GPU time from the timestamps when the device has them, wall time), for the next perf window.
- VIEW-036, REND-033 amended (cap, transient list, the limit), TEST-043.

## Classes and bounds (ADR 0035)
The globals grow by 768 bytes; a frame with no preview is bit-identical to a frame of a build without it (no node flagged, the loop not entered).

## Out of scope
Previewing a smooth; the exactness of the chain (it is approximate and replaced when the exact tiles arrive; measured on heights in `tests/preview_brush.rs`); a measured cost (the perf window).
