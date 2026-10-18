# SPIKE-11 results: mouse look plumbing

Owner's local check, 2026-10-18, on Rasierklinge (Windows 11, RTX 3080 Ti Laptop, Vulkan, scale 1.0, 960x540 window), following `CHECKLIST.md` of the spike branch `spike/spike-11-mouse-look`. Polling rate and DPI scaling were not tested.

## Owner's answers
- Smoothness: works. Frame-rate independence (normal, 50 ms frames, no vsync): no dependence noticed. Fast spins: no issue.
- Sources: **raw works and behaves as expected; cursor does not work at all.**
- Grab: `none` lets the cursor leave the window; confined and locked>confined feel the same.
- Focus (Alt-Tab, click away and back) and poles: no issue.
- Overall: raw mode is a suitable first camera interface for the editor and the viewer (it combines first-person and orbital navigation).

## What the traces show (four sessions, 31 minutes)
- Raw `DeviceEvent::MouseMotion` arrived at every capture: 16,414 and 7,721 events in the two long sessions.
- **Cursor mode fails because of recentring.** Recentre attempts that did not take effect (`recenter_failures`): 22 of 38 in session 1 and 138 of 161 in session 4 (86 %). In the short cursor-source trace (`traces/cursor-source.jsonl`, 33 s) 938 raw events arrived against 14 cursor events while captured. Recentring with `set_cursor_position` on a hidden, confined cursor is not reliable on this machine.
- `Locked` was accepted (`grab Locked: ok`), so on this machine, winit/Windows did not refuse it; the fallback to `Confined` was never needed. Whether other machines refuse it was not tested.
- Focus loss: the pose was identical before and after every focus change (`focus true/false: pose before/after identical: true`, 30 changes); no key stayed down.
- Frame times: median 16.7 ms (vsync), p99 19-21 ms. Slow frames and no-vsync runs did not change the turn per count (mouse deltas are never multiplied by dt, so the turn is counts x sensitivity).
- The raw trace replays to the recorded final pose bit for bit (`mouse-look replay`).
- No lost or stuck direction; no jump at the screen edge.

## Recommendation (input to M5 item 1b / the editor and viewer camera)
1. **Event source: raw `DeviceEvent::MouseMotion`**, applied as counts x sensitivity, never scaled by dt. Drop the cursor-delta path; do not build recentring into the product.
2. **Grab: request `Locked` first and fall back to `Confined`** with the cursor hidden, applied at click; release on Esc and on focus loss, and do not recapture until the next click. Both worked here; the fallback costs one branch and covers drivers that refuse `Locked`.
3. **Pose function:** adopt the spike's (yaw and pitch relative to the local vertical, heading by parallel transport, pitch clamped at 89 degrees; no roll after 20,000 random inputs). Add the three items of `remote-session-analysis.md` (altitude clamp, key speed scaled with body radius, whether W follows the view direction).
4. **Keep a keyboard look (arrow keys)** for machines without a usable mouse, as item 1b's fallback; mouse look over remote desktop stays out of scope (owner, 2026-10-17).
5. **Not covered:** 1000 Hz+ mice and display scales other than 100 %. Raw deltas are in device counts, so a high rate does not change the turn per cm; a check on a scaled display is cheap and can ride on the first viewer test.

## Traces
`traces/raw-source.jsonl` and `traces/cursor-source.jsonl` (format `spike11-trace-v1`, documented in `src/trace.rs` on the spike branch). The two long sessions (9.5 MB and 1.5 MB) are not committed.

## Visuals
Nothing to look at: the question is about input events, not rendering.
