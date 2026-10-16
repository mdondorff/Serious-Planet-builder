# viewer Specification

## Purpose
The viewer window of `planet editor` (ADR 0042 point 4, ADR 0015): the adapter gate that picks a discrete GPU before any device exists (ADR 0047), the flight clock and the scripted orbit-to-1 m flight, the key commands, title and readiness report (ADR 0045 point 5), the keyboard free camera as one pure pose function, the window path smoke test and the presentation blit. Decisions are GPU-free (CON-03); `render` owns the surface.

## Requirements

### Requirement: VIEW-001 Adapter gate
The application SHALL choose its GPU before any device is created, on plain descriptions of the adapters (name, backend, device class, supported features, limits by name, and for a window whether the adapter can present to it), in a GPU-free function (CON-03). The interactive application SHALL accept a discrete GPU only and SHALL refuse an integrated or virtual GPU, a software renderer, an adapter missing a required feature or below a required limit, and an adapter that cannot present to the window, with an error that names the reason for every adapter turned down (the feature or limit by name and, for a limit, its value and the minimum) and, for an integrated GPU, the way out; it SHALL never crash on a refusal. The software adapters (WARP, lavapipe) SHALL run only in the test and CI mode (`PLANET_ADAPTER=software`, `editor --smoke`); the interactive editor SHALL refuse `--adapter software`. Among several acceptable adapters the choice SHALL be deterministic whatever order the platform lists them in (Vulkan before Dx12 before other backends, then by name). `render` SHALL enumerate and describe the adapters and request the device with the limits the gate checked; `GpuContext::new(HighPerformance)` SHALL use the same gate.

Verify: A
Status: active
Source: owner 2026-10-16, ADR 0047, ADR 0015, ADR 0004, CON-03

#### Scenario: Discrete accepted
- **WHEN** the adapters are one discrete GPU with every required feature and limit
- **THEN** it is chosen

#### Scenario: Integrated refused
- **WHEN** the only adapter is an integrated GPU
- **THEN** selection fails with a message that names the adapter, says it is an integrated GPU and not a discrete one, and points to the High performance setting

#### Scenario: Missing feature or limit
- **WHEN** a discrete GPU lacks a required feature or reports a limit below the minimum
- **THEN** it is refused, naming the feature, or the limit with its value and the minimum

#### Scenario: Software only in test mode
- **WHEN** the adapters are a software renderer and a discrete GPU
- **THEN** interactive selection takes the discrete GPU and refuses the software renderer by name; test-mode selection takes the software renderer

#### Scenario: Several adapters
- **WHEN** an integrated GPU and the same discrete GPU on Dx12 and Vulkan are listed in any order
- **THEN** the same discrete adapter is chosen every time, the Vulkan one

### Requirement: VIEW-002 Flight clock
The viewer SHALL run the scripted orbit-to-1 m flight on a flight clock driven by an injected millisecond clock (CON-04), with pause (which holds the flight time while the injected time passes), resume (which continues from the held time), toggle and restart (from zero, running) as pure operations, and SHALL derive the camera as a pure function of the script and the flight time: the script's own camera on a frame step, the interpolation between two cameras in between, the first camera again after the last (a loop, a cut).

Verify: A
Status: active
Source: ADR 0042 point 4, ADR 0032 point 8, CON-04

#### Scenario: Pause and resume
- **WHEN** the clock runs for 2.5 s, is paused for a minute and resumed
- **THEN** the flight time is 2.5 s at the resume and grows from there

#### Scenario: Restart
- **WHEN** the flight is restarted
- **THEN** the flight time is 0 and running, whatever it was

#### Scenario: Replay
- **WHEN** the camera is taken twice at the same flight time
- **THEN** it is the same, and on a frame step it is the script's camera exactly

### Requirement: VIEW-003 Commands, statistics, title and readiness
The viewer SHALL map the keys P and Space (pause), R (restart), V (next debug view, cycling through all views and back), T (vsync on or off), C (flight or free camera) and Esc (quit), and the held keys of VIEW-004, in a table that is tested without a window. It SHALL show in the window title the frame rate and the mean CPU frame time over a window of injected time, the GPU time of the terrain pass (one frame a second is timed; `gpu n/a` without timestamps), the vsync state with the present mode in use, the debug view, the flight time or the free camera, the altitude and the readiness report (ADR 0045 point 5): bake blocks done of total, pre-fill tiles done of total, and the share of the newest plan's wanted tiles drawn at their own level with the mean lag in levels. The readiness report SHALL be GPU-free data.

Verify: A
Status: active
Source: ADR 0042 point 4, ADR 0045 point 5, ADR 0032 point 4

#### Scenario: Views cycle
- **WHEN** V is pressed once per debug view
- **THEN** every view is shown once and the first is shown again

#### Scenario: Coverage
- **WHEN** only the root is drawn for four wanted children
- **THEN** the view coverage is 0 of 4 with a mean lag of 1 level; with all four drawn it is 4 of 4

#### Scenario: Title
- **WHEN** the title is built from a frame rate, times, state and readiness
- **THEN** it contains each of them in a fixed format, `gpu n/a` when no GPU time exists, and `loading` or `ready` for the readiness

### Requirement: VIEW-004 Free camera
The viewer SHALL offer a free camera on the key C, moved by W, A, S, D (along the view and to the side), Q and E (down and up), the arrow keys (look) and Shift (boost), with a speed proportional to the height above the terrain within limits. The pose update SHALL be one pure function in a GPU-free crate, `free_step(pose, input, dt, height, params) -> pose`, with the input carrying key axes, key look rates and a look delta in radians (so that a mouse delta can feed it later). Yaw and pitch SHALL be relative to the local vertical and the camera rebuilt from them every frame, never accumulated into a rotation; the heading SHALL be carried by parallel transport along a great circle, so the poles are not singular; pitch SHALL be clamped short of +-90 degrees (89 degrees). Tests, goldens and perf scenes SHALL keep the scripted flight.

Verify: A
Status: active
Source: owner 2026-10-16 (status item 1 addition), ADR 0042 point 4, CON-04, CON-14

#### Scenario: No roll
- **WHEN** a long pseudo-random sequence of key and look input is applied at 1 m, 3 km and in orbit, from the equator, a pole and a diagonal
- **THEN** the camera's right axis has no vertical component and the heading stays a unit tangent at every frame

#### Scenario: Pitch clamp
- **WHEN** the pitch is driven up or down by a long key hold or a huge look delta
- **THEN** it stops at 89 degrees

#### Scenario: Reversibility
- **WHEN** a move is applied and then its opposite keys for the same time
- **THEN** the pose returns to the start to within a micrometre-scale tolerance

#### Scenario: Frame splitting
- **WHEN** one second of the same input is applied as 1, 3 or 60 frames
- **THEN** the pose is the same to rounding for motion, and the heading and pitch are the same for look

#### Scenario: Poles, 1 m and orbit
- **WHEN** the camera flies over the pole at 1 m, and moves and looks exactly at the pole
- **THEN** the heading changes smoothly with no jump and there is no roll; at 10 km and 2000 km one second forward covers the height in metres

### Requirement: VIEW-005 Viewer window path
`planet editor` SHALL open a `winit` window and run the pipelined loop with the scripted flight on today's debug path, present each frame to the window through `render` (the surface is owned by `render`, no graphics-API type above it), and close on request. `planet editor --smoke` SHALL open the window path, press the keys through the command path for a few dozen frames, require at least one presented frame, and close it; it runs on the software adapter in `cargo xtask test gpu` and on the discrete GPU in `cargo xtask test gpu --real`. Where there is no display the smoke SHALL stop with an error that starts with `no display: `, and the test SHALL be skipped with that reason only when the runner declares it (`PLANET_NO_DISPLAY`), and fail otherwise. Remote desktop SHALL work: nothing assumes a present mode or adapter beyond what the gate allows (vsync off falls back from Immediate to Mailbox to Fifo, and the title names the mode in use).

Verify: C
Status: active
Source: ADR 0042 point 4, ADR 0015, owner 2026-10-16

#### Scenario: Smoke
- **WHEN** `planet editor --smoke` runs with a display
- **THEN** it prints the adapter line and `[smoke ok]` with the frames drawn and presented, and exits with status 0

#### Scenario: No display
- **WHEN** it runs without a display
- **THEN** it exits non-zero with `no display: ...`; the test reports a skip with that reason when `PLANET_NO_DISPLAY` is set and fails when it is not

### Requirement: VIEW-006 Presentation blit
The window surface SHALL show the terrain renderer's colour target scaled to fit the window keeping its aspect, centred, with black bars, without rebuilding the renderer on a resize; an outdated, lost, timed-out or occluded surface SHALL be a skipped frame (reconfigured where that recovers it), not an error.

Verify: C
Status: active
Source: ADR 0015, ADR 0042 point 4

#### Scenario: Fit
- **WHEN** a 4x2 frame is blitted into a square target
- **THEN** the middle rows hold the frame, scaled, each source texel's colour at its centre, and the rows above and below are black
