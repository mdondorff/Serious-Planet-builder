## Why
A drag of a stamp in the editor window shows the exact terrain only when the refreshed tiles arrive (up to 300 ms to 1 s, SPIKE-13). `ManagedLoop::set_preview` (REND-033, #386) draws the stale nodes under the edit displaced by the change at once (preview <= 16 ms); the window does not call it yet. M6 plan row U4 / E3.

## What Changes
- `EditRuntime` keeps the definition last handed to the generator and the footprint items of the oldest definition any resident tile was built from (`oldest`). The first hand-over after a settled state records the items of the definition it replaces; further hand-overs (the steps of a drag) keep `oldest`, so a drag A to B to C previews as the delta A to C.
- After `WorldTerrain::set_definition` and `ManagedLoop::edit(&tiles)`, the window calls `ManagedLoop::set_preview(preview_delta(oldest, current, camera).stamps)` (`planet::window`, the publish block at the start of `frame()`).
- At the first frame whose plan is Exact with nothing substituted, no unfinished job, no busy plan worker and no pending upload (the rule of `editperf`), the window calls `set_preview(vec![])` and forgets `oldest`. `editperf` uses the same predicate.
- `EditStats` counts the previews set and the most nodes previewed in a frame; the smoke summary reports them.
- VIEW-023, tier-A tests of the feed, and the real-window editor test asserts a preview was set.

## Classes and bounds (ADR 0035)
No field and no shader value is added; the displacement is REND-033 / GEN-026 and is unchanged. The editor world is a plain `WorldTerrain` (ADR 0068); `with_edit_layers` is not used.

## Out of scope
Lines, regions and the brush (E5, E6); the edit-latency perf figures (window 13); a preview for the exact-tile arrival order.
