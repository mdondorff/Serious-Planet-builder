# viewer Specification

## Purpose
The viewer window of `planet editor` (ADR 0042 point 4, ADR 0015): the adapter gate that picks a discrete GPU before any device exists (ADR 0047), the flight clock and the scripted orbit-to-1 m flight, the key commands, title and readiness report (ADR 0045 point 5), the keyboard free camera as one pure pose function, the window path smoke test and the presentation blit. Decisions are GPU-free (CON-03); `render` owns the surface.

## Requirements

### Requirement: VIEW-001 Adapter gate
The application SHALL choose its GPU before any device is created, on plain descriptions of the adapters (name, backend, device class, supported features, limits by name, and for a window whether the adapter can present to it), in a GPU-free function (CON-03). The interactive application SHALL accept a discrete GPU only and SHALL refuse an integrated or virtual GPU, a software renderer, an adapter missing a required feature or below a required limit (the limits are listed by name: among them `max_storage_buffer_binding_size` and `max_buffer_size` at the 128 MiB of a compact tile block, ADR 0054, and `max_storage_buffers_per_shader_stage` for the stage that reads the block), and an adapter that cannot present to the window, with an error that names the reason for every adapter turned down (the feature or limit by name and, for a limit, its value and the minimum) and, for an integrated GPU, the way out; it SHALL never crash on a refusal. The software adapters (WARP, lavapipe) SHALL run only in the test and CI mode (`PLANET_ADAPTER=software`, `editor --smoke`); the interactive editor SHALL refuse `--adapter software`. Among several acceptable adapters the choice SHALL be deterministic whatever order the platform lists them in (Vulkan before Dx12 before other backends, then by name). `render` SHALL enumerate and describe the adapters and request the device with the limits the gate checked; `GpuContext::new(HighPerformance)` SHALL use the same gate.

Verify: A
Status: active
Source: owner 2026-10-16, ADR 0047, ADR 0054, ADR 0015, ADR 0004, CON-03

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
The viewer SHALL map the keys P and Space (pause), R (restart), L (the lit path with the atmosphere, or the debug path), V (next debug view on the debug path, cycling through all views and back; no effect on the lit path), T (vsync on or off), C (flight or free camera) and Esc (quit), and the held keys of VIEW-004, in a table that is tested without a window, and the help text SHALL name each. It SHALL show in the window title the frame rate and the mean CPU frame time over a window of injected time, the GPU time of the terrain pass (one frame a second is timed; `gpu n/a` without timestamps), the vsync state with the present mode in use, what is drawn (`lit + atmosphere`, `lit`, or the debug view), on the lit path the exposure (VIEW-007), the flight time or the free camera, the altitude and the readiness report (ADR 0045 point 5): bake blocks done of total, pre-fill tiles done of total, and the share of the newest plan's wanted tiles drawn at their own level with the mean lag in levels. The readiness report SHALL be GPU-free data.

Verify: A
Status: active
Source: ADR 0042 point 4, ADR 0045 point 5, ADR 0032 point 4, owner 2026-10-17 (item 5b)

#### Scenario: Views cycle
- **WHEN** V is pressed once per debug view on the debug path
- **THEN** every view is shown once and the first is shown again; on the lit path V changes nothing

#### Scenario: Coverage
- **WHEN** only the root is drawn for four wanted children
- **THEN** the view coverage is 0 of 4 with a mean lag of 1 level; with all four drawn it is 4 of 4

#### Scenario: Title
- **WHEN** the title is built from a frame rate, times, state and readiness
- **THEN** it contains each of them in a fixed format, `gpu n/a` when no GPU time exists, and `loading` or `ready` for the readiness; on the lit path it names `lit + atmosphere` (or `lit`) and the exposure after the vsync state, on the debug path the debug view and no exposure

### Requirement: VIEW-004 Free camera
The viewer SHALL offer a free camera on the key C, moved by W, A, S, D (along the view and to the side), Q and E (down and up), the arrow keys (look) and Shift (boost), with a speed proportional to the height above the terrain within limits. The pose update SHALL be one pure function in a GPU-free crate, `free_step(pose, input, dt, height, params) -> pose`, with the input carrying key axes, key look rates and a look delta in radians (so that a mouse delta can feed it later). Yaw and pitch SHALL be relative to the local vertical and the camera rebuilt from them every frame, never accumulated into a rotation; the heading SHALL be carried by parallel transport along a great circle, so the poles are not singular; pitch SHALL be clamped short of +-90 degrees (89 degrees). The keyboard feel is kept: W and S move along the view's horizontal heading and the pitch, and the orbit behaviour is unchanged. The speed limits (least and greatest, metres per second) SHALL scale with the body's radius over the Earth's mean radius (6,371,000 m), so that an Earth-sized body moves bit for bit as before and a body of half the radius has half the limits; between the limits the speed is proportional to the height above the terrain. The eye SHALL never end a frame below the terrain at its direction plus the clearance (1 m): the viewer lifts the pose after each move to the CPU f64 height field of the opened world plus the body's radius (the field the tiles are generated from, exact at every position and at every level of detail; the resident tile heights are f32 and coarser at a distance). Tests, goldens and perf scenes SHALL keep the scripted flight.

Verify: A
Status: active
Source: owner 2026-10-16 (status item 1 addition), owner 2026-10-17 (item 1b, keyboard), ADR 0042 point 4, CON-04, CON-14

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

#### Scenario: Earth speeds unchanged
- **WHEN** an Earth-sized body is flown at heights from below zero to infinity with boost 0, 1, 2.5 and 8
- **THEN** every speed is bit-identical to the speed before the limits scaled, and a second of forward flight gives the same pose

#### Scenario: Half the radius
- **WHEN** a body of half the Earth's radius is flown
- **THEN** its limits are 0.25 and 1.5e6 m/s, its speed between them equals the Earth's at the same height, and the boost multiplies the scaled limit

#### Scenario: Ground clamp
- **WHEN** the camera flies down and forward into a 300 m hill for 20 s
- **THEN** at every step its distance from the centre is at least the terrain there plus 1 m, and it ends within a metre of that

#### Scenario: Poles, 1 m and orbit
- **WHEN** the camera flies over the pole at 1 m, and moves and looks exactly at the pole
- **THEN** the heading changes smoothly with no jump and there is no roll; at 10 km and 2000 km one second forward covers the height in metres

### Requirement: VIEW-005 Viewer window path
`planet editor` SHALL open a `winit` window on the lit path with the atmosphere (VIEW-007) and run the pipelined loop with the scripted flight, present each frame to the window through `render` (the surface is owned by `render`, no graphics-API type above it), and close on request. `planet editor --smoke` SHALL open the window path, press the keys through the command path (V on the lit path, L to the debug path, V, L back, vsync, pause, the free camera, restart) for a few dozen frames, judge its own counts without a clock (at least 16 lit frames, every lit frame with the atmosphere, a lit and a debug frame presented, an exposure reading, and the renderer's own account that the exposure and atmosphere passes ran) and close it; it runs on the software adapter in `cargo xtask test gpu` and on the discrete GPU in `cargo xtask test gpu --real`. Where there is no display the smoke SHALL stop with an error that starts with `no display: `, and the test SHALL be skipped with that reason only when the runner declares it (`PLANET_NO_DISPLAY`), and fail otherwise. The headless start check `editor --no-window` SHALL stay. Remote desktop SHALL work: nothing assumes a present mode or adapter beyond what the gate allows (vsync off falls back from Immediate to Mailbox to Fifo, and the title names the mode in use). The software adapter's smoke SHALL use the vertex tile format unless `--tile-format` says otherwise.

Verify: A, C
Status: active
Source: ADR 0042 point 4, ADR 0015, owner 2026-10-16, owner 2026-10-17 (item 5b)

#### Scenario: Smoke
- **WHEN** `planet editor --smoke` runs with a display
- **THEN** it prints the adapter line and `[smoke ok]` with the frames drawn (lit, with the atmosphere, debug) and presented (lit, debug), the exposure readings, and exits with status 0

#### Scenario: Verdict on counts
- **WHEN** the counts of a run fall short of any condition (fewer than 16 lit frames, a lit frame without the atmosphere, no lit or no debug frame presented, no exposure reading, a renderer that did not run the passes)
- **THEN** the smoke fails naming the count that fell short

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

### Requirement: VIEW-007 Lit path in the window
The viewer window SHALL start on the lit path with the atmosphere and draw the body's look (`--look`, else the file beside `--world`, else the Earth defaults): the lit plan of each frame SHALL be a pure function of the look, the world's radius, the frame clock's dt (capped at 0.25 s) and a reset flag (`viewer_lit_plan`), carrying the look's sun direction, tone curve, scene units, histogram and albedo, the adaptive exposure (range and speeds of the look, the pre-exposure of the body: 12 stops moved by the stops its sun differs from the Earth's) and the atmosphere of a body of the world's radius; a look whose atmosphere does not fit the body SHALL leave the lit path without the atmosphere and the run SHALL say why. The exposure SHALL snap to its target on the first lit frame, on the return to the lit path and on a restart of the flight, and adapt otherwise. L SHALL switch to the debug path, which draws the plan as the pipelined loop decided it, and back; the free camera and the scripted flight SHALL work on both. `enable_lit` is not part of `warm_up`: the window SHALL enable the lit path and the atmosphere and draw the first lit frame once per ring set before frame 0, whichever path it opens on, so that no lit frame creates a long-lived GPU object or uses a part of the renderer for the first time. The title SHALL show the exposure of the last sampled lit frame as `EV <ev100> (adapted)` within 0.05 stop of the target and `EV <ev100> (adapting to <target>)` otherwise. The debug-path defaults of `test-render` and the tests SHALL NOT change.

Verify: A, C
Status: active
Source: owner 2026-10-17 (item 5b), ADR 0042 points 1, 3 and 4, ADR 0048 points 2 and 3, ADR 0050, CON-03, CON-04

#### Scenario: Plan from the look
- **WHEN** the lit plan is built twice from the same look, radius, dt and reset, and from a look with another sun, another scene unit and a body whose radius the atmosphere cannot sit on
- **THEN** equal inputs give equal plans and equal text; the sun direction is the look's; a sun a quarter as bright lowers the pre-exposure by two stops; the unfit radius is an error

#### Scenario: Exposure reset and the debug views
- **WHEN** the viewer starts lit, L is pressed twice, the flight is paused, the free camera taken and the flight restarted
- **THEN** the exposure snaps on the first lit frame, on the return to the lit path and on the restart, and on no other frame; V changes the debug view only on the debug path

#### Scenario: Look drawn
- **WHEN** `planet editor --smoke --look F` runs with a look file that changes the sun, the scene unit and the exposure range
- **THEN** it prints the look line and passes the smoke with the atmosphere on every lit frame

### Requirement: VIEW-008 The atmosphere that does not fit the body
A look whose atmosphere does not fit the body of the world (a top above the body's surface) SHALL leave the viewer on the lit path without the atmosphere, and the run SHALL print why (`atmosphere off: <reason>` and `view: the lit path with no atmosphere`); the smoke verdict SHALL then require no frame with the atmosphere and the run SHALL succeed. The decision is `viewer_lit_plan`'s refusal (VIEW-007); this requirement is its tier-C test through the binary.

Verify: C
Status: active
Source: VIEW-007, opus-planner request of 2026-10-17

#### Scenario: A small body
- **WHEN** `planet editor --smoke` runs a world of 60 km radius under the default look (an atmosphere 100 km thick)
- **THEN** the output says the atmosphere is off and why, the lit path ran without it for at least 16 frames, no frame has the atmosphere and the run ends `[smoke ok]`

### Requirement: VIEW-009 Water in the window
The viewer SHALL draw the water of a world that has a bake: the world computes the ocean channel with its in-memory bake (or reads it from the store) and assembles the water field W from the sea of its definition; each lit frame's plan SHALL carry the water plan of ATMO-012 made from the frame's terrain plan, the world's liquid table, the look's water parameters (LOOK-001) and the injected clock's time (CON-04); the window SHALL warm the water pass with the lit path before frame 0 (REND-017) and its plans SHALL bound their tiles up to the sea level for the frustum cut (ATMO-016); the smoke summary SHALL report the lit frames that had water. A world with no bake has no water and draws as before.

Verify: C
Status: active
Source: ADR 0041 points 4 and 8, ADR 0051 point 6, ADR 0042 point 4

#### Scenario: A world with a sea
- **WHEN** `planet editor --smoke` runs a world with the default field over an in-memory bake
- **THEN** the summary counts lit frames with water (the flight starts in orbit over the sea) and the run ends `[smoke ok]`
