## Why
SPIKE-11 (owner's local check, 2026-10-18): raw `DeviceEvent::MouseMotion` is a suitable first camera interface for the editor and the viewer; the cursor path does not work (86 % of the recentres failed). M6 item 4c.

## What Changes
- `planet_frame::mouselook` (new, GPU-free): `MouseLook` (grab state, raw counts, a grab change for the window), `SENSITIVITY_RAD_PER_COUNT` = 0.0015, `GRAB_ORDER` (Locked, then Confined). Counts x sensitivity once per frame, never times dt; no recentring.
- `ViewerState` holds a `MouseLook`: `camera()` feeds its delta into `FreeInput::look_delta_rad`; leaving the free camera and a restart release it; `focus_lost()` releases it and clears every held key.
- `window.rs` (thin glue): `device_event` feeds the raw motion; a left click in the free camera captures; Esc with the mouse captured releases (it quits otherwise); `Focused(false)` calls `focus_lost`; `apply_grab` sets the grab (Locked, else Confined) and the cursor visibility.
- Help text names the mouse. The xtask aggregator guard also refuses a `[[test]]` that is not `it`, `oracle_all` or `gpu_all` (the verifier's gap of #331).
- VIEW-010 added, VIEW-003 (Esc) modified.

## Tests
Tier A: `mouselook` unit tests (capture, counts x sensitivity, ignore rules, Esc, focus loss, frame-independence), `viewer` tests (the mouse turns only the free camera, leaving it releases, focus loss clears keys), and `crates/frame/tests/mouselook_replay.rs`: the two committed SPIKE-11 traces replayed through the product against an independent reference (the traces' own `final` poses belong to the spike's pose function and keys and are not the reference).

## Not tested here
1,000 Hz mice and display scales other than 100 % (SPIKE-11 did not cover them): a manual check at the laptop. The window glue (`apply_grab`, the event arms) has no automated test beyond compiling and the existing window smoke; the owner's local check judges the feel.

## Classes and bounds (ADR 0035)
No field and no shader value is added; the pose function is VIEW-004's. No golden moves.

## Out of scope
The keyboard and orbit feel (the owner judges them), recentring, remote desktop mouse look (owner, 2026-10-17).
