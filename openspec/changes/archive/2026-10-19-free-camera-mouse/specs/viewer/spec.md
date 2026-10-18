## MODIFIED Requirements

### Requirement: VIEW-003 Esc and the captured mouse
Esc SHALL quit as before, except that while the mouse is captured (VIEW-010) it releases the mouse and does not quit; the key table and the help text name this.

## ADDED Requirements

### Requirement: VIEW-010 Mouse look in the free camera
In the free camera (VIEW-004) the viewer SHALL turn the view with the mouse. The source is the raw relative motion of the pointer (winit `DeviceEvent::MouseMotion`, device counts), turned into a look delta in radians as counts times a sensitivity of 0.0015 rad per count (the SPIKE-11 value), moving the mouse right looking right and moving it up looking up, added to the frame's `look_delta_rad` once per frame and never multiplied by the frame time, so the turn per count does not depend on the frame rate. The cursor SHALL NOT be recentred. A left click on the window in the free camera SHALL capture the pointer (grab `Locked` first and `Confined` when the platform refuses it, the cursor hidden); motion is ignored while the pointer is not captured and non-finite counts count for nothing. Esc while captured, a loss of the window's focus, leaving the free camera (C) and a restart SHALL release the pointer (cursor shown), forget the pending motion and not capture again until the next click; a loss of focus SHALL also release every held key. The state is GPU-free and window-free (`planet_frame::mouselook::MouseLook`, held by the viewer state), driven by synthetic events in tests; the window only turns its events into calls and applies the grab change it is asked for. The arrow keys keep turning the camera with or without the mouse. The pose function, the keyboard feel and the orbit behaviour are those of VIEW-004.

Verify: A
Status: active
Source: SPIKE-11 (owner's local check, 2026-10-18), owner (M6 item 4c), VIEW-004

#### Scenario: Counts times sensitivity
- **WHEN** 100 counts right and 50 counts up are captured and the camera steps by 16 ms, and again by 1 s
- **THEN** the heading turns right by 100 x 0.0015 rad and the pitch rises by 50 x 0.0015 rad in both, and 2,000 counts cut into 1, 7 or 2,000 frames turn by the same total

#### Scenario: Capture and release
- **WHEN** the free camera is on and the window is clicked, and later Esc is pressed, the focus is lost, C is pressed or the flight restarts
- **THEN** the pointer is captured by the click only in the free camera, motion before the click and after a release is ignored, the window is asked to grab once per change, and Esc with the pointer captured does not quit

#### Scenario: The SPIKE-11 traces replay
- **WHEN** the committed raw-source and cursor-source traces (`spikes/mouse-look/results/traces/`) are replayed through the mouse look and the free camera (the raw events as counts, the frames as steps, a click or, for the raw trace, a capture at the start, the resets as resets)
- **THEN** every stretch of a trace ends at the pose an independent reference gives (the yaw the sum of the counts times the sensitivity, the pitch clamped frame by frame) and at least one stretch turned by more than a radian
