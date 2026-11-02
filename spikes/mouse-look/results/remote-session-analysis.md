# SPIKE-11: the owner's remote session (2026-10-17) and the keyboard-movement check

Session: the owner ran `mouse-look.exe` on Rasierklinge from an Android phone with a touch screen over Chrome Remote Desktop (no mouse, on-screen keyboard). Trace `target/traces/trace-1792220511-1.jsonl` (1.36 MB, 32,491 lines, 535 s; not committed: over the size habit of the repo and it holds no mouse look). The remote part is out of scope by the owner's decision of 2026-10-17; this is recorded as an observation, not as a failure of the plan.

## What the trace says about input over the remote session
- Counters at the end: raw 402, cursor 4, recenters 0, recenter_failures 0, dropped_while_free 406, captured false throughout. The window never captured the mouse: **there is not one `button` event** in the trace. A tap on a touch screen probably reaches the window as a touch / pointer message and not as `WindowEvent::MouseInput`; the harness handles only the left mouse button and logs no `Touch` events, so this cannot be confirmed from the trace. Nothing was lost by the adapter's logic: every raw and cursor event arrived while uncaptured and was counted as dropped.
- 402 `DeviceEvent::MouseMotion` events did arrive over the remote connection (so raw motion was not absent there); the 4 `CursorMoved` events are window sizing. Whether those raw events carry a usable relative signal was not testable without a click.
- Focus changes (6 events, three pairs): the pose before and after was identical each time.
- Keyboard: 52 key presses (104 events): arrows 27 presses, W 6, A 4, S 1, D 1, Q 4, E 6, Escape 2, SuperRight 1. The 14 key-up events without a recorded key-down are F1 and F2 (handled by the window, not fed to the simulation; harmless).

## Keyboard movement against heading and pitch
Method: an independent replay of the trace's frames and keys through a re-implementation of the pose step (`analysis/trace_check.py`); it reproduces the recorded final pose **exactly** (position difference 0.0 m, same pitch), so it is the same arithmetic as the window. For each of the 4,542 frames with a movement key held, the displacement was decomposed onto the heading, right and vertical vectors before the step and compared in sign with the commanded motion (W/S forward, D/A right, E/Q up). Result: **0 frames disagree** (no sign flip, near a pole or after looking up or down; the largest absolute latitude reached was 70.4 degrees, so the pole region was approached but never crossed).

What the replay does show, and what can look wrong without being a sign error:
1. **W/S move along the horizontal heading, not along the view direction.** Looking down 30 degrees and holding W for 4.6 s moved 23 m and kept the altitude at 1.7 m. A user who expects "fly toward where I look" sees W neither descend nor climb. By design of the pose function (a ground-walking camera); a free-flight variant would add the view-direction component.
2. **Q goes below the surface.** Q from 1.7 m for 1.0 s reached -3.3 m (minimum over the session -3.4 m; -0.2 m again later). The shader intersects the sphere from outside only, so with the eye inside the planet there is no hit and the whole view is sky; keys keep working but the ground has vanished. The draft pose function has no ground clamp.
3. **Speed grows with altitude** (5 m/s times altitude / 2 above 2 m): E held for 3.4 s reached 4,595 m on a planet of radius 1,000 m (circumference 6.3 km); then W at 947 m moved 22 km in 9 s and A moved 17.9 km in 7.5 s, several times round the planet; with pitch -89 degrees the ground then streams past. The direction is right; the magnitude looks like "wrong direction" on a body this small.
4. **One W press of 30.6 s** (437.5 s to 468.1 s), with arrow-key presses in between: either a held key or a lost key-up from the on-screen keyboard. The movement during it is consistent with W held (185 m at 2.4 m altitude).
5. The pitch clamp worked (-89 degrees reached by holding ArrowDown) and no key stayed down across a focus loss (the simulation clears keys on focus loss).

Recommendation for the pose function (input to item 1b): clamp the altitude at a small positive minimum above the surface (or at the field height), scale the key speed with the body radius as well as the altitude, and decide whether W follows the view direction in the free camera. None of this touches mouse look.

## Files
- `analysis/trace_check.py TRACE.jsonl`: replay, final-pose check, sign check, key-hold table.
- `analysis/trace_keys.py TRACE.jsonl`: key press / release pairs and presses over 10 s.
