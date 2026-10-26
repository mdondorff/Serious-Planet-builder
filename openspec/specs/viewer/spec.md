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
The viewer SHALL map the keys P and Space (pause), R (restart), L (the lit path with the atmosphere, or the debug path), V (next debug view on the debug path, cycling through all views and back; no effect on the lit path), T (vsync on or off), C (flight or free camera) and Esc (quit; with the mouse captured it releases the mouse first, VIEW-010; in the editor mode it quits nothing, VIEW-022), and the held keys of VIEW-004, in a table that is tested without a window, and the help text SHALL name each. It SHALL show in the window title the frame rate and the mean CPU frame time over a window of injected time, the GPU time of the terrain pass (one frame a second is timed; `gpu n/a` without timestamps), the vsync state with the present mode in use, what is drawn (`lit + atmosphere`, `lit`, or the debug view), on the lit path the exposure (VIEW-007), the flight time or the free camera, the altitude and the readiness report (ADR 0045 point 5): bake blocks done of total, pre-fill tiles done of total, and the share of the newest plan's wanted tiles drawn at their own level with the mean lag in levels. The readiness report SHALL be GPU-free data.

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
The viewer SHALL offer a free camera on the key C, moved by W, A, S, D (along the view and to the side), Q and E (down and up), the arrow keys (look) and Shift (boost), with a speed proportional to the height above the terrain within limits. The pose update SHALL be one pure function in a GPU-free crate, `free_step(pose, input, dt, height, params) -> pose`, with the input carrying key axes, key look rates and a look delta in radians (so that a mouse delta can feed it later). Yaw and pitch SHALL be relative to the local vertical and the camera rebuilt from them every frame, never accumulated into a rotation; the heading SHALL be carried by parallel transport along a great circle, so the poles are not singular; pitch SHALL be clamped short of +-90 degrees (89 degrees). The keyboard feel is kept: W and S move along the view's horizontal heading and the pitch, and the orbit behaviour is unchanged. The speed limits (least and greatest, metres per second) SHALL scale with the body's radius over the Earth's mean radius (6,371,000 m), so that an Earth-sized body moves bit for bit as before and a body of half the radius has half the limits; between the limits the speed is proportional to the height above the terrain. The eye SHALL never end a frame below the terrain at its direction plus the clearance (`FREE_EYE_CLEARANCE_M`, 0.5 m: a crouch), and the free camera's near plane SHALL be `FREE_NEAR_M` (0.05 m; the scripted flight keeps 0.1 m), with a depth resolution (reversed-Z `Depth32Float`) finer than 1 mm out to 1 km (measured 0.1 mm at 1 km): the viewer lifts the pose after each move to the CPU f64 height field of the opened world plus the body's radius (the field the tiles are generated from, exact at every position and at every level of detail; the resident tile heights are f32 and coarser at a distance). Tests, goldens and perf scenes SHALL keep the scripted flight.

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
`planet editor` SHALL open a `winit` window on the lit path with the atmosphere (VIEW-007) and run the pipelined loop with the scripted flight, present each frame to the window through `render` (the surface is owned by `render`, no graphics-API type above it), and close on request. `planet editor --smoke` SHALL open the window path, press the keys through the command path (V on the lit path, L to the debug path, V, L back, vsync, pause, the free camera, restart) for a few dozen frames, judge its own counts without a clock (at least 16 lit frames, every lit frame with the atmosphere, a lit and a debug frame presented, an exposure reading, and the renderer's own account that the exposure and atmosphere passes ran) and close it; it runs on the software adapter in `cargo xtask test gpu` and on the discrete GPU in `cargo xtask test gpu --real`. Where there is no display the smoke SHALL stop with an error that starts with `no display: `, and the test SHALL be skipped with that reason only when the runner declares it (`PLANET_NO_DISPLAY`), and fail otherwise. The headless start check `editor --no-window` SHALL stay. Remote desktop SHALL work: nothing assumes a present mode or adapter beyond what the gate allows (vsync off falls back from Immediate to Mailbox to Fifo, and the title names the mode in use). The software adapter's smoke SHALL use the compact tile format with the atmosphere, like every other adapter (there is no other tile format, ADR 0062; WARP, which lost its device with compact tiles and the atmosphere, passes this smoke on the current build).

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

### Requirement: VIEW-010 Mouse look in the free camera
In the free camera (VIEW-004) the viewer SHALL turn the view with the mouse. The source is the raw relative motion of the pointer (winit `DeviceEvent::MouseMotion`, device counts), turned into a look delta in radians as counts times a sensitivity of 0.0015 rad per count (the SPIKE-11 value), moving the mouse right looking right and moving it up looking up, added to the frame's `look_delta_rad` once per frame and never multiplied by the frame time, so the turn per count does not depend on the frame rate. The cursor SHALL NOT be recentred. A left click on the window in the free camera SHALL capture the pointer (in the editor mode, VIEW-022, the right button captures it while held and the left button edits) (grab `Locked` first and `Confined` when the platform refuses it, the cursor hidden); motion is ignored while the pointer is not captured and non-finite counts count for nothing. Esc while captured, a loss of the window's focus, leaving the free camera (C) and a restart SHALL release the pointer (cursor shown), forget the pending motion and not capture again until the next click; a loss of focus SHALL also release every held key. The state is GPU-free and window-free (`planet_frame::mouselook::MouseLook`, held by the viewer state), driven by synthetic events in tests; the window only turns its events into calls and applies the grab change it is asked for. The arrow keys keep turning the camera with or without the mouse. The pose function, the keyboard feel and the orbit behaviour are those of VIEW-004.

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

### Requirement: VIEW-011 Editor UI shell
`planet editor` SHALL draw an immediate-mode UI (egui; `egui`, `egui-wgpu` and `egui-winit` pinned `=0.36.2` with default features off, SPIKE-14, ADR 0004, ADR 0015) over the scene in its window, and nothing else SHALL draw it (the headless modes, `test-render` and the golden paths never do, so no golden moves); `--no-ui` turns it off. The UI is one frame-graph node `ui` after `tonemap` that reads and writes the presentable target only, so no HDR, depth or histogram resource, lifetime or byte count of the lit frame changes (ADR 0042 point 2, node 8). The editor produces GPU-free egui output (`planet_render::UiFrame`: tessellated shapes, the texture changes since the last painted frame, pixels per point) and `render` paints it (`UiPainter`, loading the scene and blending over it); the texture changes of a pass that is not painted (the swap chain was out of date) are kept for the next one, and the painter clears the changes it applied. Pixels the UI does not cover SHALL equal the blit alone. Input: egui sees every window event first; a pointer press goes to the UI when egui wants the pointer (over a panel, a window or a popup) and to the world otherwise, the choice latched until the release, so a drag that starts in the world keeps turning the camera across a panel and a drag that starts on a slider never moves it; hover without a button is decided per motion; keys go to the UI while a text field has the focus (the camera and the commands get no press, and a release still lets go of a held key); while the mouse is captured (VIEW-010) egui gets no pointer event, and a click captures the mouse only when the world got the press. The one font is Roboto Regular v2.138 under Apache-2.0 (CON-27: egui's bundled fonts are OFL and Ubuntu-font licensed and stay off), with its provenance in `crates/app/assets/fonts/PROVENANCE.md`.

Verify: A, C
Status: active
Source: SPIKE-14, ADR 0015, ADR 0042 point 2, owner (M6 step U1; gate decision 14: fonts under an allowed licence), VIEW-010

#### Scenario: The node follows the tone map
- **WHEN** the lit frame graph gets the `ui` node at 2560x1440 without and with the atmosphere and the water
- **THEN** the order ends `tonemap`, `ui`, every other pass keeps its place, and the frame target bytes and lifetimes are those of the graph without it

#### Scenario: Only the UI's pixels change
- **WHEN** a rectangle is painted over a blitted frame at 1 and 2 pixels per point
- **THEN** every pixel clear of the rectangle's edge equals the blit alone and the inside is the rectangle's colour

#### Scenario: Routing
- **WHEN** a press lands over the shell and another outside it (at 1 and 1.5 pixels per point), a drag starts in the world and crosses the shell, the pointer only hovers, and a text field has the focus
- **THEN** the first goes to the UI and the second to the world, every motion of the drag goes to the world, hovering is decided by where the pointer is, and a key typed with the focus lands in the field and is not a camera key

#### Scenario: The window draws it
- **WHEN** `planet editor --smoke` runs on the software adapter, with and without `--no-ui`
- **THEN** the summary counts a UI pass for the frames it presented, and with `--no-ui` there is none

### Requirement: VIEW-012 Surface picking
The editor SHALL turn a cursor position into the point of the planet surface under it and the item under that point (`planet_frame::pick`, GPU-free, f64 on the CPU, `libm` only; ADR 0001, CON-11, CON-14). The ray starts at the camera in `PlanetFixed` and runs through the pixel (`cursor_ray`: pixel centres at `+ 0.5`, the camera's own `ray_frame`). It meets the height field `r(d) = R + h(d)`, not the drawn tiles, so the answer does not depend on the level of detail or on which tiles have arrived. The march runs inside the shell `[R - max_height, R + max_height]` by sphere tracing with a declared slope bound (`PickLimits::max_slope`, default 2) and bisects the bracketing step to `tolerance_m` (default 1e-4 m). A ray that never enters the shell, leaves it without touching the surface, starts below the surface (or inside the inner sphere), meets a height that is not finite, or has a zero or non-finite direction has no hit; a ray that enters the shell where the surface is already at or above it hits at the entry; a camera inside the shell picks from its own position. The march never steps less than `min_step_m` (default 0.05 m) or a millionth of its distance from the camera and runs at most 100,000 steps, so a ray within about 1e-4 rad of the horizon can run out and miss; the bisection ends at the tolerance (a tolerance that is not positive means 1e-9 m) or after 200 halvings. The hit carries the point, the unit direction from the centre, the height at that direction and the distance along the ray. An item is a footprint (id, unit direction, radius along the surface): the item under a hit is the footprint that contains it (great-circle distance by `atan2` of the cross and dot products, metres on `R`; the rim belongs to the footprint) with the smallest distance in units of its own radius, then the smaller radius, then the lower id. The result is a pure function of its inputs, bit for bit.

Verify: A
Status: active
Source: owner (M6 step U2), ADR 0001, SPIKE-14, VIEW-011

#### Scenario: Rays meet the sphere where the analytic answer says
- **WHEN** pixels across the viewport are picked over a bare sphere of 6,371 km radius from orbit, from 50 km and from 300 m up
- **THEN** every hit is within 0.2 mm of the quadratic's solution and the centre pixel from orbit hits at the distance `R`

#### Scenario: Rays meet a hill where a dense march does
- **WHEN** a ray descends onto a 3 km hill with a 0.25 m reference march
- **THEN** the picked distance is within 0.3 m of the reference, the reported height is the field's height at the hit direction, and the point is on the surface to 1 mm

#### Scenario: Misses and starts
- **WHEN** the ray looks past the limb, away from the planet, or from a camera below the surface, and when a camera 100 m above the surface inside the shell looks down at an angle
- **THEN** the first three give no hit and the last picks a hit between 100 m and 200 m away

#### Scenario: Bounds, grazing rays and bad inputs
- **WHEN** rays enter the shell onto a bare sphere with a bound of 0 m and onto a plateau at the bound, rays pass 1 km inside and 1 m outside the limb, and the camera is at the centre, the direction is zero, not a number or not a unit vector, the tolerance is 0, the field returns NaN, the viewport has no area, or a footprint has no direction or radius
- **THEN** the bound cases hit within 0.2 mm of the analytic sphere, the 1 km ray hits and the 1 m ray misses, every degenerate input ends at once with a miss (or a hit for the non-unit direction and the zero tolerance) and a broken footprint contains nothing in either list order

#### Scenario: The hit projects back to the cursor
- **WHEN** a hit is projected with the camera's f64 projection
- **THEN** it lands on the cursor pixel within 1e-6 pixels, and two calls give the same bits

#### Scenario: Footprints
- **WHEN** footprints nest, overlap with equal keys, or lie off the hit
- **THEN** the nearest in its own radius wins (a small item near its centre inside a large one), equal keys go to the smaller radius then the lower id, a hit outside every footprint gives none, and small arcs of 1 mm to 2,000 km measure within 1 nm

### Requirement: VIEW-014 Debug overlay and coarse LOD view
The editor window SHALL be able to draw a debug overlay over the lit terrain so that the real mesh density and the levels of detail are visible, and to show the mesh at a coarser level of detail for inspection. F3 cycles the overlay (off, tiles, mesh, regions, stale tiles, off; the regions overlay is VIEW-030, the stale-tile overlay VIEW-044) and F4 toggles the coarse LOD; both are off at the start; both act only in `planet editor` and on the lit path (the headless modes, `test-render` and the golden paths never set them, so no golden moves). The overlay is plan data (CON-03): `planet_frame::look::Overlay` (`None`, `Tiles`, `Mesh`, `Regions`, `Stale`; further debug overlays plug into the same enum; `ALL` and `next` give the cycle, `shader_code` 0, 1, 2, 3) is a field of `LitPlan` (`overlay`), `LitPlan::with_overlay` sets it, and the plan text and hash carry it only when it is not `None` (` overlay tiles`, ` overlay mesh` or ` overlay regions` after the other tokens, parsed back and validated). The renderer does not branch on a mode of its own: the plan's value reaches the lit fragment stages through the globals (`extra.y`) and the node record (`morph.w` carries the tile's level). The fragment form needs no wgpu feature, so it runs on WARP, lavapipe and the discrete GPU: from the compact tile's grid coordinates it draws, in `Tiles`, a faint tint of the level's colour and the tile's outline, and in `Mesh` also a line where the fragment is within about a pixel of a grid edge or of the diagonal of the grid's triangulation (the cell's `a` to `c`, as `grid_indices`), a fixed line that does not fade out however fine the mesh is. The colour is one of eight per level, wrapping; the brightness falls with the vertex morph factor (full detail bright, fully morphed on the parent grid 0.3). The lines are radiance in scene units (the pre-exposure factor of the frame times 0.8), so they show at any exposure. With the overlay `None` the frame is the lit frame bit for bit. The coarse LOD multiplies the LOD error bound `tau_px` by 4 (`COARSE_LOD_FACTOR`), or by the largest factor below 4 that keeps `LodParams::validate` satisfied (`LodParams::coarse`, `coarse_factor`; the production 1080p parameters, 6 px and 32 cells, allow about 1.7, VIEW-032), in the request the frame is planned with (`effective_lod`; terrain and water alike), so the mesh edges are about that many times longer on the screen: it changes the view, not the data, and is decided in the GPU-free plan, never in a shader. The coarser bound can fail on steep terrain (the plan worker reports a tile whose morph interval has no room): the frame then falls back to the base parameters, the switch turns itself off (F4 asks again) and the smoke summary counts the refusal; the window never stops for it. The editor's corner label shows the state (`overlay_status`: `Overlay: Mesh, LOD x4`, `Overlay: Tiles, LOD x1.7`, `Overlay: Regions, LOD x1`, `Overlay: off, LOD x1`; the factor is the one the plan uses).

Verify: A, C
Status: active
Source: owner (2026-10-19, via opus-planner), CON-03, CON-18, VIEW-011, VIEW-030

#### Scenario: The plan carries the overlay
- **WHEN** a lit plan is made, given tiles, mesh or regions, written as text, parsed back, and given an unknown overlay name
- **THEN** it has no overlay by default, the text of a plan without one has no overlay token (hashes are as before), the overlays add ` overlay tiles`, ` overlay mesh` and ` overlay regions`, the plan round-trips with its other tokens, and the unknown name is an error naming it

#### Scenario: F3 and F4
- **WHEN** F3 is pressed four times and F4 twice in the editor, in the lit and debug paths
- **THEN** the overlay goes tiles, mesh, regions, stale tiles, off (off at the start, kept when the debug path is shown), the coarse LOD turns on and off independently, the keys help names both, the coarse parameters are the base ones with only `tau_px` changed, times 4 where that is valid and else the largest valid factor (always valid, the production 1080p ones about 1.7), the corner label reads `Overlay: Mesh, LOD x4`, `Overlay: Regions, LOD x1` or `Overlay: off, LOD x1`, and the window smoke counts lit frames drawn with an overlay, with the mesh, and frames planned with the coarse LOD

#### Scenario: A refused coarse plan
- **WHEN** the plan with the coarse LOD fails and the base one works, when the coarse view is off and the plan fails, when both fail, and when the coarse plan works
- **THEN** the first uses the base plan, turns the switch off and counts one refusal, the second returns its error at once with no second attempt, the third returns the second error and counts one refusal, and the fourth changes nothing

#### Scenario: The overlays change the terrain's pixels only
- **WHEN** lit frames of a close view, a mid view and the orbit view are drawn on the software adapter with the overlay none, tiles and mesh
- **THEN** the none frame equals the plain lit frame, the mesh frame is deterministic, no pixel outside the planet changes, a close view has at least 50 changed terrain pixels in each overlay and the mesh differs from the tiles, and inspection images (production LOD and a coarse one, three overlays each) are written to `target/review/overlay-inspection/`

### Requirement: VIEW-013 Globe overview
The editor's globe overview SHALL be an orbit camera, item markers on the screen and a flight to a clicked place (`planet_frame::globe`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14). A `GlobeView` is a unit direction (the point under the camera) and an altitude above the sphere of radius `R`; its camera sits over that point at `R + altitude`, looks at the planet's centre and has north up. The centre stays at least 1 degree from either pole (a view over a pole is moved to the edge of its cap; a zero or non-finite centre is +x; an altitude below 1,000 m or not finite is 1,000 m), so north stays up and the east axis is defined. A drag by `(dx, dy)` pixels turns the centre along the great circle in the drag's direction by exactly the angle its pixels span at the centre (`2 tan(fov/2) altitude / (height R)` per pixel; the surface follows the cursor, within 1 px of a flat surface at 100 km altitude) and keeps the altitude; a drag whose path would enter a cap stops at the cap's edge on the side it came from, so a view never flips; a viewport without height, a value that is not finite and a zero drag change nothing. Zoom multiplies the altitude and clamps it to `[1,000 m, 8 R]` (at least 1,000 m on a small planet); a factor that is not positive and finite changes nothing. A marker (id, unit direction, footprint radius) is shown when it faces the camera (the camera is above the tangent plane at the marker, so the horizon at `acos(R / (R + h))` from the centre point is the limit) and projects inside the viewport (`project`, pixel origin top-left); `visible_markers` returns them in the order given. The marker under the cursor is the nearest visible one within `hit_px` pixels, the radius inclusive, ties going to the lower id. The closing view of a marker is over it at four footprint radii, never below 1,000 m. A `GlobeFlight` between two views moves the centre along the great circle (spherical interpolation, the arc halved for an exactly opposite pair through the start's east axis) with a smootherstep time curve, interpolates the altitude geometrically and adds a rise that peaks mid-trip at `1.5 R` times the arc over `pi`; it starts and ends at the two views exactly (the time clamped to the flight, a time that is not a number the start; between, the centre stays outside the caps, so a flight over a pole bends round it) and lasts `0.8 s + 1.2 s` per radian of arc. In the editor window (G, `GlobeSession`, `ViewerState.globe`) the overview is the camera: G shows it over the point under the camera now (the free camera's or the flight's) at 1.5 radii up, and G again leaves it with the camera mode untouched. The left button drags it (`ClickTracker`: a press and release within 4 px are a click, not a drag; a press the overview cannot vouch for, because the overview was toggled, the focus was lost, a release was not seen or the press was over a panel, is cancelled, so the globe never drags with no button down), the wheel zooms by 0.9 per line (`wheel_zoom_factor`), and the overlay draws each marker the camera sees (egui, a dot with its id, the one within 12 px of the cursor larger). A click names the marker within 12 px, else the surface under the cursor (`pick_surface` against the height field, VIEW-012), and starts the flight to the marker's closing view or to 20 km over the spot; while it flies the overview takes no drag, zoom or click; the frame it ends the free camera takes over over the place, looking north and 0.5 rad down, and the flight clock waits. A click on the sky does nothing. `--marker LAT,LON[,RADIUS_M]` (repeatable, degrees, the radius 1,000 m by default) gives the markers, numbered from 1; until the item commands exist (E4) they are the only source.

Verify: A
Status: active
Source: owner (M6 step U4; MVP scope 2026-10-18: 3D and globe views), VIEW-011, VIEW-012

#### Scenario: The view and the markers
- **WHEN** a view over any point (poles included) is built, and markers lie at the centre, on the far side, past the horizon (65 degrees from a camera at one radius up whose horizon is at 60) and 20 degrees north-east
- **THEN** the centre point projects to the middle of the viewport with north up and east right, the markers on the far side and past the horizon are left out and the other two are returned in order

#### Scenario: The click and the drag
- **WHEN** the cursor is within 12 px of one or of two markers, and a view is dragged by (50, -30) px at 100 km altitude or at the pole, and zoomed beyond both limits
- **THEN** the nearest marker is found (ties to the lower id, the radius inclusive), the old centre point appears at (850, 420) within 1 px, a drag at 85 degrees latitude north stops at the cap's edge without flipping the view, a drag of 300 px at 8 R turns the centre by exactly its angle, values that are not numbers change nothing and never panic, and the altitude stops at 1,000 m and at 8 R

#### Scenario: The flight
- **WHEN** a flight runs between two views 120 degrees apart at 2 km, between two nearby views, between the same place twice and between opposite places
- **THEN** every step is a unit centre with the angle from the start growing and the two arcs adding to the whole, the ends are exact and clamped, the long flight rises above 100 times its end altitudes mid-trip, the short one lasts under 1 s, and the opposite pair stays finite

#### Scenario: Click to fly
- **WHEN** a marker on the overview is clicked
- **THEN** the flight ends over it at four footprint radii (1,000 m at least)

#### Scenario: The window runs the overview
- **WHEN** G is pressed in the flight and in the free camera, a click lands on a marker, on bare surface and on the sky, the drag and the wheel are used while a flight runs, and the flight ends
- **THEN** the press tracker calls a press released within 4 px a click and any other a drag, ignores motion without a press and forgets a cancelled one, each wheel line is a factor of 0.9, G during a flight drops it and leaves the camera mode as it was, the overview is shown over the camera's point at 1.5 radii and G leaves it, the marker click flies to its closing view and the surface click to 20 km over the spot, the sky click does nothing, the drag, wheel and clicks are ignored while flying, the flight ends in the free camera at the landing altitude with the flight clock paused, `--marker` values parse (errors name the value), and the window smoke shows the overview with a marker, clicks it and counts the flight

### Requirement: VIEW-015 What the camera shows of the edits
The editor SHALL decide which edits it shows in markers, outlines and the list of nearby edits from the camera (`planet_editor::relevance`, GPU-free, f64 on the CPU, `libm` only; CON-03, CON-11, CON-14):
- an edit is shown when its footprint meets the visible area and its reach covers at least `MIN_REACH_PX` = 8 pixels on the screen; the footprint of a brush holding strokes is where a stroke reaches (WDEF-007), its size the reach disc's;
- the selected edit is always shown, whatever its size or place;
- an edit the user hid is left out unless it is selected.

The footprint part is the footprint find (WDEF-007, `WorldDefinition::footprint_items`, halo 0) over the tiles of the visible area, one find per tile and layer, never a scan of all items. The size is `reach_m / d * (height_px / 2) / tan(fov_y / 2)` with `d` the distance from the camera to the anchor's point on the sphere; a camera on the point gives infinity. So near the ground small stamps show with the larger edits around them, and from high up the small stamps drop out.

Verify: A
Status: active
Source: owner (M6 step U3, relevance rule 2026-10-19; hiding is the layer list's checkbox in the mock), WDEF-007, VIEW-012

#### Scenario: The size on the screen
- **WHEN** a reach of 100 m is seen from 1,000 m and 2,000 m up with a 60 degree field of view over 1,000 pixels, doubled, and from a camera on the point
- **THEN** the sizes are `0.1 * 500 / tan(30 degrees)` pixels, half of that, twice that, and infinity

#### Scenario: Near the ground and from high up
- **WHEN** a hill (reach 400 m) and a plateau (reach 15 km) at one place are seen from 2 km, from 100 km, and from the heights where the hill covers 7.9 and 8.1 pixels
- **THEN** both show at 2 km, only the plateau at 100 km, and the hill shows at 8.1 pixels and not at 7.9

#### Scenario: The footprint and the selection
- **WHEN** two large plateaus lie on two sides of the planet and one tile is visible, then no tile is visible, then the far one is selected, then a small hill outside the tiles is selected from 2,000 km up, then the near plateau is hidden
- **THEN** only the plateau in the tile shows, nothing shows without a tile, the selected edit is added each time, and a hidden edit shows only while selected

#### Scenario: Not a scan
- **WHEN** forty large plateaus 330 to 700 km away, each covering more than 8 pixels from 30 km up, lie outside the visible tile
- **THEN** none of them shows, so the tiles and not the pixel test kept them out

### Requirement: VIEW-016 Lists and search of the edits
The editor SHALL list the edits as plain data (`planet_editor::lists`, GPU-free); a row holds the id, a label, kind, layer, distance from the camera, reach, and for lines, regions and brushes the numbers of its detail: the parts of its group (1 when it is not split), which part it is, its points and its strokes.
- The label of an edit is its name, trimmed, else its kind and the last six characters of its id.
- The distance is that from the camera to the anchor's point on the sphere.
- The nearby list holds the edits the camera shows (VIEW-015), nearest first, ties by id.
- The search runs over every edit of the body, not only the shown ones: every word of the query is part of the label, kind, layer or id text, in any case; an empty query matches all; a kind filter restricts to one kind; the rows are nearest first, ties by id.
- The order the edits apply in is the layer's place in the layer stack, then priority, then id (ADR 0007, ADR 0066 point 4); the stack bottom to top is `base`, `imported`, `intent`, `stamps`, `splines`, `placed`, `frozen`, and a layer outside it applies after these, by name.
- The layers list groups by layer in stack order, each group in the order the edits apply; the chain is all edits in that order.
- A line or a brush drag that was split into linked items (a group, VIEW-028, VIEW-035) is one row of the layers list, the row of its first part, with the number of its parts and, for a brush, the strokes of all of them; the nearby list, the search and the chain keep every item, and a part says which one it is. The detail of a row (`Row::detail`) is, for a brush, its strokes (`1 stroke`, `3 strokes`, with `N parts, ` before when it has several items), for a split line its parts (`N parts`), for a part `part K`, for a line or region its points (`17 points`), and nothing for a stamp. The panels show it after the kind in the layers list and the other lists, and a brush has a colour of its own.
- Selecting a row selects its item (a double click flies to it, as the Fly to button does); hiding a row hides, and deleting the selected item deletes, the whole group it belongs to, as one command, so that a brush, which has no handle, is found and deleted by its row and no part of a line or a brush drag is left behind.

Verify: A
Status: active
Source: owner (M6 step U3, search planet-wide sorted by distance from the camera 2026-10-19), VIEW-015, ADR 0007, ADR 0066

#### Scenario: Nearby
- **WHEN** three named edits are listed from a camera near the second, with an id outside the set and two edits at the same distance
- **THEN** the rows are nearest first, an id with no edit is left out, and equal distances order by id

#### Scenario: Search
- **WHEN** the query is `hill`, `HILL far`, empty, a kind filter, a text that matches nothing, and the last six characters of an unnamed edit's id, and a name with spaces around it
- **THEN** the far hill is found after the near one, every word must match in any case, all edits are listed for the empty query, the filter keeps one kind, nothing matches the unknown text, the unnamed edit is found with the label `kind` plus those characters, and the padded name is shown trimmed

#### Scenario: Lines, regions and brushes
- **WHEN** a ridge of 150 km (split into parts), a short channel, a region of three points and a brush with two strokes are in the layers list, and a brush drag of 12 km made several items
- **THEN** the ridge is one row saying `N parts`, the channel `17 points`, the region `3 points`, the brush `2 strokes`, the long brush drag one row saying `N parts, S strokes`, the search lists every part of the ridge, and hiding or deleting the ridge's row takes all its parts (the delete is one command that one undo takes back)

#### Scenario: Layers and order
- **WHEN** two edits of priorities 5 and -1 in `stamps`, one in `splines` (also at priority -100) and one in a layer outside the stack are listed
- **THEN** the groups come in the order `stamps`, `splines`, the outside layer, the `stamps` rows by priority, and the chain is the same order whatever the priorities of the higher layers

### Requirement: VIEW-017 Selecting, handles and typed values
The editor SHALL select, place, delete, drag and type values through one session over the E4 commands (`planet_editor::session`, with `shape` and `params`; GPU-free, f64 on the CPU, `libm` only), so the flows are tested without a window.

Shape and handles:
- The radius of a stamp (hill, crater, plateau) is the file's `reach_m` (WDEF-010), and so is its outline; the outline of another kind is its `radius_m` parameter when positive, else its reach.
- The amount of a stamp is its one parameter: `height_m` of a hill (signed), `depth_m` of a crater, `target_m` of a plateau (an absolute height); the height handle edits it. Another kind has a height handle when it has `height_m`.
- The handles are the anchor (always), the radius (when it has an outline) and the height (when it has its amount).
- A handle sits on the anchor, on the outline along the item's heading (0 north, 90 east), or at the height: above the anchor by a hill's height, below it by a crater's depth, at a plateau's absolute target whatever the ground.
- At a pole, north is the limit along longitude 0: -x at the north pole, +x at the south pole.
- A direction becomes latitude and longitude in the spelling the item file takes: longitude in (-180, 180], 0 at a pole; latitude by `atan2` of the height over the distance from the axis.

Drags (each result is a valid item and the bounds hold it):
- move puts the anchor under the pointer;
- resize sets a stamp's `reach_m` to the arc from the anchor to the pointer, so it follows the pointer in and out; another kind's `radius_m` becomes the arc and its reach grows to 1.25 times it (at most one cell edge, never shrinking);
- raise changes the kind's amount: a hill's `height_m` and a plateau's `target_m` go up with the pointer, a crater's `depth_m` goes down (the handle is the bottom of the bowl);
- the bound of a hill or a crater is exactly its amount, and a plateau's bound is its own parameter and does not move;
- another kind's raise adds to its `height_m` and its bound grows to hold it (never shrinking, at most `MAX_DELTA_LIMIT_M`);
- the radius and the height stay in the kind's range from the parameter table, the range a typed value is held to; a kind the table does not know keeps the radius within 1 m and one cell edge and the height within the limit on the change;
- a pixel is `2 d tan(fov_y / 2) / height_px` metres at a point whose depth along the view axis is `d` (at least 1 m), and a drag up raises.

Commands, each one E4 command:
- a whole drag, `begin_drag` to `end_drag`, whatever the number of pointer moves; `cancel_drag` records nothing; while a handle is held no other command, undo, redo or change of selection runs;
- placing a stamp (the kind's defaults from the table, in the layer `stamps`: the reach is the radius, the bound is the amount of a hill or a crater and the table's default for a plateau), deleting, renaming and a typed value;
- a place that is not a finite non-zero direction is refused.

Typed values:
- the text is read in the parameter's unit (`450`, `2.4 km`, `40 %`);
- a value out of the kind's range, not a number, in another unit, or of an unknown parameter changes nothing and says why;
- where the table has no range the item file's limits hold: a radius within 0 and one cell edge, any other metric parameter within the limit on the change;
- the radius of a stamp typed is its reach;
- the amount of a hill or a crater typed is its bound;
- a plateau's flattening limit is its `max_delta_m`, a field of the file and no parameter;
- for another kind the reach and the bound only grow to hold the value.

Picking and inspection:
- the item under the pointer is the nearest in units of its own outline radius among the shown ones (`pick_footprint`; a select press and the hover take the smallest footprint first and a press again goes on to the next, VIEW-041); an item that is not shown cannot be picked;
- the inspector is the selected edit's values as plain data: kind, layer, place in the order of its layer, distance, reach and the parameters in the table's order with their ranges, then the others by name without a range;
- lengths read `450 m` below 1 km (after rounding to a tenth of a metre) and `2.4 km` above.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, typed metric input the fallback, 2026-10-19), AUTH-004, VIEW-012

#### Scenario: Outline and handles
- **WHEN** an item with a radius and a height, one without a height, one with neither and no reach, and one with a radius that is not positive are examined, and the handles are placed at headings 0, 90 and 217
- **THEN** the handle lists are move, radius, height; move, radius; move; the outline falls back to the reach, the radius handle is 400 m from the anchor within 1 mm along the heading, the height handle is above the anchor by a hill's height, below it by a crater's depth and at a plateau's absolute target, and heading 90 is east

#### Scenario: One spelling and the poles
- **WHEN** directions at ordinary places, 89.9999 degrees north, the poles, the antimeridian and negative zero are turned into latitude and longitude, put through `Item::validate`, and the north of a pole is compared with the north a micro-degree away
- **THEN** they equal the input within 1e-9 degrees, a pole has longitude 0, the antimeridian is +180, every result is a valid item, and the north does not jump at either pole

#### Scenario: Drag arithmetic
- **WHEN** a hill is moved, resized to 1,500 m, 100 m, 0 m and 90 km, raised by 40 m, -200 m and a billion metres, and a kind the table does not know is resized and raised the same way
- **THEN** the anchor is under the pointer, the reach is the arc within 1 mm and follows it down as well as up, the hill stays within 50 m to 2 km and -400 m to 400 m with its bound equal to its height, a crater's depth stays within 10 m to 800 m (raising makes it shallower) with its bound equal to it, a plateau's target stays within -500 m to 9,000 m and its bound does not move, the unknown kind stays within 1 m to one cell edge and within the limit with its bound holding the height, a pixel at 1 km is `2 d tan(fov_y / 2) / height_px` metres, and a drag up raises

#### Scenario: Picking
- **WHEN** a small hill lies inside a large plateau, the pointer is on the hill's centre, 5 km away, far away, and the hill or both are left out of the shown set or an id has no item
- **THEN** the hill is under the pointer at its centre, the plateau 5 km away, nothing far away, and an edit that is not shown or not an item is not picked

#### Scenario: Placing and deleting
- **WHEN** craters are placed, an edit is deleted and undone and redone, and everything is undone
- **THEN** each placement is one selected command with the kind's defaults and a written file, a delete deselects, and the undos reach the exact starting hash and clear a selection whose edit is gone

#### Scenario: Typed values
- **WHEN** `3.5 km`, `300`, `20 km`, `10`, `-5`, `abc`, an empty text, `5 kg`, an unknown parameter and `NaN` are typed, a plateau's limit and target, a hill's negative height and a crater's depth are typed, and over-limit values for a kind the table does not know
- **THEN** the valid values are one command each in their unit with the reach and bound following (a plateau's limit goes to `max_delta_m`, a negative hill height sets the bound to its size, a crater's depth to its bound), every refused value changes nothing, and the stored item keeps its outline within its reach and its height within its bound

#### Scenario: Drags are one command
- **WHEN** a hundred pointer moves of a position handle end, then radius and height drags end, a drag is cancelled, and a place that is zero, not a number or infinite is given to a drag and to a placement, and another edit is selected during a drag
- **THEN** each drag is one undo step and three undos reach the exact starting hash, the cancelled drag changes nothing and records nothing, the bad places are refused, and the selection does not change while the handle is held

#### Scenario: Inspector and units
- **WHEN** an edit with table parameters and two other parameters is inspected, and lengths are typed and formatted
- **THEN** the kind, layer, `47.20000° N`, `8.55000° E`, `2.4 km`, `1 of 2 in its layer` and `500 m` show, the parameters come in table order with their ranges and the others by name without a range, `450`, `450 m`, `2.4 km` and `40 %` are read in their units, empty text, words, a wrong unit, exponent notation and non-finite numbers are refused with the reason, and 999.96 m reads `1 km`

### Requirement: VIEW-018 The layout of the panels is data
The layout of the editor panels SHALL be a value read from a file of keys, not code (`planet_editor::layout`, GPU-free); a key the file does not name keeps its default. A key, its unit and range:

| Key | Default | Range |
|---|---|---|
| `rail_width` (points; 0 hides the rail) | 56 | 0 to 200 |
| `left_width` (points) | 300 | 120 to 800 |
| `inspector_width` (points) | 320 | 120 to 800 |
| `inspector_side` | `right` | `left` or `right` |
| `tabs` (the left panel's tabs in order) | `near`, `layers`, `graph`, `problems` | at least one, none twice |
| `status_bar` | true | true or false |
| `handle_hit_px` (pixels around a handle in which a press takes it) | 10 | 3 to 60 |

An unknown key, a value out of its range, a wrong type, a bad tab list or a file that does not parse is refused naming the key (`(file)` for a file that does not parse).

Verify: A
Status: active
Source: owner (M6 step U3: layout data-driven and cheap to change, 2026-10-19), VIEW-011

#### Scenario: Defaults and overrides
- **WHEN** an empty file and a file naming some keys are read
- **THEN** the first is the default and the second changes only the named keys

#### Scenario: Refusals
- **WHEN** a file has an unknown key, a width out of range, a string for a number, `nan`, a bad side, an empty, repeated, unknown or mistyped tab list, or a syntax error
- **THEN** each is refused with the key named

### Requirement: VIEW-019 What the pointer does on the planet
The editor SHALL turn the pointer into session calls in one place (`planet_editor::interaction`, GPU-free, f64 on the CPU), from plain data the window resolves: the cursor, the surface hit, the shown edits and the handles on the screen.
- A press takes a handle of the selected edit when the cursor is within the hit radius of it (`handle_hit_px` of the layout, VIEW-018; the nearest wins, the radius inclusive).
- Else, with the stamp tool and the cursor on the surface, the press places a stamp (selected, the tool back to select); with the stamp tool and the cursor off the planet it does nothing and the tool stays.
- Else the press selects the shown edit under the cursor (VIEW-017), or clears the selection when there is none.
- While a handle is held a move makes the item the pointer says (position and radius follow the surface hit and stay put without one; the height follows the vertical move of the cursor in pixels times the metres per pixel at the handle, up raises); the release ends the drag as one command; an abandoned drag goes back and records nothing.
- The hover is the shown edit under the cursor; it is none over a handle and does not change while a handle is held.

The tiles an edit sends to refresh are the resident tiles whose area with a halo of 0.25 tile edges a reach of the places the commands changed can touch (`planet_editor::feed`, the footprint touch test with its margins: false positives only cost a refresh); a place that is a brush item holding strokes touches a tile only when one of its strokes does as well (WDEF-007), so a stroke names the tiles along it and not the reach disc round its first point; they come in tile order without repeats, and the places of another body count for none.

Verify: A
Status: active
Source: owner (M6 step U3: handles on the planet are the primary way to edit a stamp, 2026-10-19), VIEW-012, VIEW-017, WDEF-007

#### Scenario: Handles under the cursor
- **WHEN** three handles lie at known pixels and the cursor is near each, on the edge of the radius, just beyond it, between two within reach, and there are none
- **THEN** the nearest within the radius is taken, the radius is inclusive, a cursor beyond it takes none

#### Scenario: Hover and press
- **WHEN** the cursor is over an edit, off every outline, off the planet, over an edit that is not shown, over a handle, and a handle is held; then presses land on an edit, on nothing and in the sky, and the stamp tool presses on the planet and off it
- **THEN** the hover follows (none over a handle, unchanged while a handle is held), a press selects without a command, clears the selection, or places one selected stamp, and the stamp tool off the planet changes nothing

#### Scenario: Drags
- **WHEN** a press on the move handle is followed by fifty moves and a release, then drags of the height handle (20 pixels up at half a metre a pixel) and the radius handle, a drag with no surface hit, and an abandoned drag
- **THEN** each drag is one command and three undos reach the exact starting hash, the height rose by 10 m, a move without a hit changes nothing, and the abandoned drag changes nothing and records nothing

#### Scenario: The tiles to refresh
- **WHEN** a 5 km reach at one place is sent over all level-8 tiles, the place before and after of a move, another body, a larger reach, and empty lists
- **THEN** a handful of tiles are found including those of points 4.9 km away in every direction and the tile of the centre, none far away, both places count, another body counts for none, a larger reach finds more, and empty lists find nothing; a brush stroke 712 m long names the tiles along it and fewer than half the tiles of its reach disc at level 17

### Requirement: VIEW-020 The edits on the screen
The editor SHALL turn the shown edits into plain data for the egui layer (`planet_editor::overlay`, GPU-free, the camera's f64 projection): for each edit the label, kind, selected and hovered flags, the anchor in pixels, and an outline of `OUTLINE_SEGMENTS` = 64 points on the ground at the outline radius, closed; the selected edit carries its handles in pixels and is last in the list (drawn on top).
- A point is on the screen only when it is on the side of the planet the camera sees (`faces_camera`) and inside the viewport; any other point is `None`.
- The ground height at the edit lifts the outline and the handles.
- An id with no edit is skipped.

Verify: A
Status: active
Source: owner (M6 step U3: outlines and names on hover, handles on the planet, 2026-10-18), VIEW-012, VIEW-013

#### Scenario: The ring
- **WHEN** a hill is seen from 2 km straight down, with another edit hovered and the hill selected
- **THEN** the selected edit is last, only it has handles, its 64 points lie at `400 / 2000 * 500 / tan(30 degrees)` pixels from the centre within 1 %, the move handle is on the centre, the radius handle on the outline within 1 %, the height handle behind the move handle straight down and above the centre from an oblique view, and a ground 500 m up grows the ring

#### Scenario: Off the screen
- **WHEN** the ring leaves the screen, the edit is on the other side of the planet, an edit is 1 degree and another 3 degrees along the horizon from a camera 2 km up, and an id has no edit
- **THEN** those points are `None`, the far side shows nothing, the 3 degrees edit is behind the horizon though the projection alone would draw it, the 1 degree edit is drawn, and the id is skipped

### Requirement: VIEW-021 The editor panels
The editor window SHALL draw the editor panels with egui from the layout value (VIEW-018) (`planet::panels`, no editing state of its own): the tool rail (select, the stamp kinds with the numbers 1 to 4, brush, line and region disabled as later), the left panel with its tabs, the inspector on the layout's side, the status bar, and the outlines, names and handles over the planet. The panels read plain data and return events; the window applies them to the session.
- The tabs are `near` (a search box, the kind filter, the rows: the shown edits nearest first, or the planet-wide matches when the query or the kind filter is set), `layers` (by layer with a checkbox that hides the edit in the views, in the order they apply) `graph` (the chain from the procedural base to the tiles) and `problems` (the edits the engine cannot draw fully, VIEW-047).
- A click on a row asks to select the edit and a double click to fly to it.
- The inspector shows the selected edit: name, kind, layer, id, anchor, distance, place in its layer, reach and the parameters with their ranges.
- The name field asks for a rename when it is left changed.
- A parameter field shows the value in its unit and follows the value while it is not being typed in.
- A parameter field asks for the typed text when it is left and differs from the value.
- The inspector has Fly to, Deselect and Delete buttons.
- Undo and Redo buttons are disabled when there is nothing to do.
- The status bar shows the status line, the count of edits, the undo depth, where the edits are saved and the window's own status.
- A rail of width 0 or no status bar is not drawn; the first tab of the layout is the first shown.

Verify: A
Status: active
Source: owner (M6 step U3: map first, list, search, inspector bound to the commands, layer list and graph as secondary views, 2026-10-18), VIEW-011, VIEW-018

#### Scenario: Where the panels sit
- **WHEN** the default layout and a layout with the inspector on the left, no rail, no status bar, a wider left panel and one tab are drawn at 1920x1080
- **THEN** the panels have the widths and sides of the layout, the middle of the window is not covered, and the tabs, rail and status bar are as the layout says

#### Scenario: Tools, tabs and rows
- **WHEN** the rail's buttons, the tabs, a row and a row twice, a layer checkbox, the search box, and the kind filter are clicked
- **THEN** the tool, select, fly and hide events come out, the tabs switch without an event, the results replace the nearby list while a query or filter is set

#### Scenario: Typed values and buttons
- **WHEN** a parameter field and the name field are focused and left unchanged, then given new text and Enter; the buttons are pressed with and without something to undo; the counts line is built
- **THEN** an unchanged field asks for nothing, the typed text is asked for once, a disabled button asks for nothing, the other buttons ask for their events, and the counts line has the edit count, the undo depth and where the edits are saved

#### Scenario: The overlay
- **WHEN** the overlay of a ring, a ring with gaps, a selected edit with handles and an edit with nothing on the screen is drawn at one and at one and a half pixels per point
- **THEN** outlines, names and handles are drawn, gaps are not joined, and an edit with nothing on the screen draws nothing

### Requirement: VIEW-022 The editor mode of the window
`planet editor` SHALL have an editor mode (`planet::editwin`, with `window`) that edits the world's definition through the session (VIEW-017) and the panels (VIEW-021).
- Tab turns the mode on and off; `--edit` starts the window with it on.
- `--items DIR` is the directory the edits are written to and loaded from: one TOML file per edit (AUTH-001), every command writes its file; without it the edits live in memory and the status line says they are not saved.
- `--editor-layout F` is the layout file (VIEW-018).
- `--edit` with `--no-ui` is an error that says the mode needs the UI.
- Turning the mode on switches to the free camera and ends the globe overview (once: G shows it again); while the mode is on C does nothing and the left button does not capture the pointer.
- The globe overview (VIEW-013) shows a marker for each edit the camera shows (VIEW-015 over the tiles the last plan wanted, at the overview's camera, so from high up only the large edits and the selected one), numbered from 1,000,000, sized by the outline radius and labelled by the edit's name; this holds with the mode off as well.
- A click on an edit's marker selects the edit and starts the flight to it as for any marker.
- The left button edits: a press in the world presses, any release releases, and a press while the pointer is captured does nothing.
- The right button held looks around (VIEW-010); it waits while the left button holds a handle.
- Esc abandons a drag, else leaves the stamp tool, else deselects, and does not quit while the mode is on (VIEW-003).
- Keys go to a text field with the focus.
- While the pointer is captured egui is given no pointer event, so when a capture starts and when it ends every button is let go for egui (a move far from the cursor, then the buttons up: no click, no button left down).
- Turning the mode off abandons a drag; its places are still named for the refresh.
- The mode finds the shown edits every frame by the footprint find over the tiles the plan wants (VIEW-015), draws their outlines, names and handles (VIEW-020), lists them (VIEW-016), and names the resident tiles to refresh after each command (VIEW-019).
- Before the frame plans, a changed definition is handed to the generator (`WorldTerrain::set_definition`, STRM-028) and then the named tiles are refreshed (`ManagedLoop::edit`), so the terrain follows the edits; a definition the generator refuses is a refused count and a status line. Items loaded at the start refresh every resident tile.
- After a hand-over the plan's height bound is the new field's maximum (which counts the stamps' deltas) when that is above the configured bound, so a stamp on the highest peak does not fail the plan; a run with no edits plans as it did.
- `--smoke --edit` runs a script of the mode after the overview phases (VIEW-005): the mode on, a hill placed where the view meets the surface, its move handle dragged, a radius typed, undo, redo, delete, undo; the run fails unless all of it happened and nothing was refused, and the summary reports the counts.

The keys of the mode, taken by the letter on the key where a letter is meant (so on a QWERTZ keyboard the key labelled Z undoes):

| Key | Action |
|---|---|
| `1` | select tool |
| `2`, `3`, `4` | crater, hill, plateau tool |
| `5`, `6`, `7` | ridge, channel, region tool (VIEW-029, VIEW-031) |
| `8`, `9`, `0`, `-` | brush: raise, lower, smooth, flatten (VIEW-035) |
| Enter | finish the line or region being drawn |
| Backspace | take the last point of the line or region being drawn back; it never deletes an edit |
| Ctrl+Z | undo |
| Ctrl+Y, Ctrl+Shift+Z | redo |
| Delete | delete the selected edit |
| F1, `?` | the list of keys |
| every key above, the viewer's keys (VIEW-003) and the free camera's | named in `EDIT_KEYS_HELP`, the usage text of the mode; a test over the key tables fails when a bound key is not named |
| Esc | step back (above) |

Fly to puts the free camera south of the edit looking at it, pitched down:

| | Distance |
|---|---|
| Back, in outline radii (at least 150 m) | 2.5 |
| Up, in outline radii (at least 80 m) | 1.2 |

Verify: A, C
Status: active
Source: owner (M6 step U3, 2026-10-19), VIEW-003, VIEW-010, VIEW-011, VIEW-017, VIEW-019, AUTH-001

#### Scenario: The mode on and off
- **WHEN** keys and presses come while the mode is off, the mode is turned on, a stamp is placed, its move handle is held and dragged, and the mode is turned off
- **THEN** nothing is placed, counted or drawn while it is off, and turning it off abandons the drag and goes back, recording only the placement

#### Scenario: A whole session of edits
- **WHEN** a hill is placed with the keys and a click, its move handle dragged 60 pixels, a radius typed, a radius out of range typed, and undo, redo, delete and many undos pressed
- **THEN** each step is one command with the counts and places to refresh, the drag moves the item the metres 60 pixels span, the refused value says why, and the undos reach an empty world

#### Scenario: Selecting, Esc and fly
- **WHEN** a press lands on an edit, the cursor is over a panel, and Esc is pressed with a drag held, with the stamp tool on, and with a selection; and Fly to is asked
- **THEN** the edit is selected and hovered, there is no hover over a panel, Esc steps back one thing each time, and the camera pose looks at the edit from the distances above

#### Scenario: The keys
- **WHEN** the keys of the table are pressed on a QWERTY and on a QWERTZ keyboard, with and without Ctrl, and the fly keys W A S D Q E, Space, G and C
- **THEN** each key does its action by the letter on it, no letter is a command without Ctrl, and the fly keys are not the editor's

#### Scenario: Files and search
- **WHEN** a stamp is placed and a value typed with `--items`, and the directory is opened again; and a search for an edit out of view
- **THEN** one file named for the edit's id holds it, the reopened session has the same definition hash, and the search finds the far edit while the nearby list holds only the shown ones

#### Scenario: A look-around leaves no button down
- **WHEN** egui sees the right button go down, no event follows for a while, and then the release events of a capture are given, twice
- **THEN** the button stays down for egui until the release events, which let it go without a click, and given again they do nothing

#### Scenario: The overview and the mode
- **WHEN** the mode is turned on with the overview shown and with none, and G shows the overview in the mode
- **THEN** the overview ends once on the turn-on and the free camera is taken, and the overview shown with G stays

#### Scenario: The overview
- **WHEN** a plateau of reach 15 km and a hill of 400 m are seen from 200 km up, the hill is selected, nothing is in view, the plateau is hidden, a marker is clicked and a number that is no edit's is clicked, with the mode off
- **THEN** only the plateau is a marker, then the selected hill too (the markers follow the order of the ids and carry the names), a hidden edit is none unless selected, the click selects the edit, the other number changes nothing, and the markers come with the mode off

#### Scenario: The options
- **WHEN** `--edit` is given with `--no-ui`
- **THEN** the run fails before it opens anything and says the mode needs the UI

#### Scenario: The real window
- **WHEN** `planet editor --smoke --edit --items DIR` runs on the selected adapter
- **THEN** the run succeeds with one edit placed, one drag, one typed value, one delete, two undos and one redo and nothing refused, definitions handed to the generator, tiles sent to refresh, and one item file in DIR

### Requirement: VIEW-030 Region overlay
The debug overlay enum `planet_frame::look::Overlay` (VIEW-014) SHALL have a fourth value `Regions` (shader code 3, plan token `overlay regions`) that tints the lit terrain by the surface type of the region over each fragment. For each frame that shows it the app SHALL decide a `planet_frame::regions::RegionList` and hand it to the renderer (the renderer decides nothing, CON-03): the at most 8 region items of the definition nearest the camera (ties by item id), each as a polygon of its 3 to 64 points in the orthographic frame of its anchor (the anchor relative to the camera, the tangent plane's basis, and the points in metres in that plane; f64 on the CPU, rounded once to f32, CON-11), with the surface type's tint as linear light, and the count of the regions it left out (the farthest). The list SHALL have a stable text form (a header line and one line per polygon, floats as bit patterns) that round-trips, and an empty list has no text. The lit fragment stage SHALL take the fragment's position relative to the camera, subtract the anchor, project onto the basis, and test the point against the polygons nearest first by the even-odd rule, skipping a polygon whose plane is farther from the fragment than its band or whose reach the fragment is beyond; where it is inside a polygon it SHALL mix the tint at the weight 0.45 into the lit colour (`mix(lit, tint * 0.8 * pre-exposure, 0.45)`), with no texture and no planet-radius value in the shader. F3 cycles the overlay through off, tiles, mesh, regions, off, and the corner label names it. With the overlay not `Regions`, or with no polygon in the list, the frame is the lit frame bit for bit, and the headless modes and goldens never set the overlay (VIEW-014).

Verify: A, C
Status: active
Source: ADR 0069, owner (2026-10-19), VIEW-014

#### Scenario: The list
- **WHEN** ten regions lie along the equator and the list is made for a camera over the first
- **THEN** it holds the eight nearest, nearest first, and counts the two left out; a definition without regions gives an empty list with no text

#### Scenario: The text
- **WHEN** a list of three polygons is written as text, parsed back, and given a header announcing nine polygons, one line fewer than announced, or a misspelt key
- **THEN** it round-trips exactly and the three malformed texts are errors naming the problem

#### Scenario: The plane
- **WHEN** a grid of points over and around a concave polygon is tested at the datum in the polygon's plane and by the gnomonic test that decides the stamp mask (WDEF-012), and the far side of a point inside the polygon is tested
- **THEN** the two agree on every grid point and the far side is outside

#### Scenario: F3
- **WHEN** F3 is pressed four times in the editor
- **THEN** the overlay goes tiles, mesh, regions, stale tiles, off, and the corner label names it

#### Scenario: The tint
- **WHEN** a lit frame of a close view with one polygon under the camera is drawn on the software adapter with the regions overlay and with none
- **THEN** every pixel the polygon covers is the plain lit colour mixed with the tint at the stated weight (within the half-float rounding of the scene target), every other pixel and every pixel outside the planet is identical, a polygon on the other side of the planet and an empty list change nothing, and the frame is deterministic

#### Scenario: The tint lands where the polygon is
- **WHEN** an L-shaped polygon is put under the ground the view shows and the pixels of a grid of ground points far from its edges are looked up through the camera
- **THEN** a pixel is tinted exactly when its ground point is inside the L, with points on both sides in view (a mirrored or swapped plane, or a missing edge, fails)

### Requirement: VIEW-023 The editor mode previews an edit in flight
After the editor mode hands a changed definition to the generator and names the tiles to refresh (VIEW-022), the window SHALL call `ManagedLoop::set_preview` with `preview_delta(oldest, current, camera)` (REND-033), where `current` is the footprint items of the whole body (`footprint_items_cap` over the full sphere) of the definition just handed over, `camera` is the camera's planet-fixed position, and `oldest` is the footprint items of the oldest definition any resident tile was built from: the definition replaced by the first hand-over since the preview last ended, kept unchanged by further hand-overs, so that a drag in many steps previews as one delta from its first state. The preview SHALL end (`set_preview` with an empty list, `oldest` forgotten) at the first frame whose plan is exact with no substituted node, no unfinished tile job, no busy plan worker and no pending upload. The editor world SHALL be a plain `WorldTerrain` (ADR 0068).

Verify: A, C
Status: active
Source: owner (M6 step U4, 2026-10-19), REND-033, ADR 0066 points 3 and 5, ADR 0068

#### Scenario: A drag is one delta from its first state
- **WHEN** a stamp is placed (hand-over 1), then moved twice (hand-overs 2 and 3) before any frame settles
- **THEN** the preview after hand-over 3 is `preview_delta` from the items before hand-over 1 to the items of hand-over 3, which removes nothing the placement added and adds the stamp once at its final place; it is not the delta from hand-over 2 to 3

#### Scenario: The preview ends when the exact tiles are in
- **WHEN** a hand-over has set a preview and a frame is exact with nothing substituted, no unfinished job, no busy worker and nothing pending, and another hand-over then follows
- **THEN** the first sets the empty list and forgets `oldest`, and the next preview is the delta from the definition the second hand-over replaced

#### Scenario: The real window previews
- **WHEN** the real window on the software adapter places and drags a stamp with the editor mode (`--smoke --edit`)
- **THEN** at least one preview was set, and the run still ends exact

### Requirement: VIEW-024 The window keeps the camera's last minute and F9 writes it
The window SHALL keep a rolling log of the camera path (`planet_frame::pathlog::PathLog`): a pose recorded at most every 100 ms of the window's clock (the first always), each with the time, the position, the view direction and the up vector in f64, the altitude above the terrain and the frame time, and nothing older than 60 s before the newest; in the viewer and in the editor mode alike. F9 (`Command::SaveRepro`, which changes nothing in the viewer and is never blocked by the editor mode) SHALL write `frame-<n>-path.txt` (the log) and `frame-<n>.bundle` (a terrain repro bundle, TEST-008, of that frame's request over the world as the window holds it, edits included) with `<n>` the frame count, to `target/review` or to the directory `PLANET_REPRO_DIR` names, and print their names. The path file SHALL be written first, so a bundle that fails never loses it. With authored items the bundle's world section SHALL carry the definition text without items and each item as its item-file TOML (`world_item` lines), so that the replay builds a definition equal to the window's, items included; a record that does not rebuild to the definition the frame was planned from SHALL be refused with both hashes. A path file is `planet-path 1`, the window and period, the sample count and one line per sample, with floats in the shortest round-trip form so that reading restores them bit for bit; an unknown header, a wrong count, a malformed line or a time that goes back SHALL be refused with the line. The `--smoke` script SHALL press F9 once where `PLANET_REPRO_DIR` is set and the summary SHALL report the writes.

Verify: A, C
Status: active
Source: opus-planner (2026-10-19), TEST-008, VIEW-022; opus-planner (2026-10-21, H0: F9 with authored items), AUTH-001

#### Scenario: Ten poses a second, one minute
- **WHEN** frames come every 7 ms for 3 minutes, and again 50 ms, 99 ms and 100 ms after a recorded pose
- **THEN** the log holds about 10 poses a second over the last 60 s and no more than 601, and the 50 ms and 99 ms poses are not recorded while the 100 ms one is

#### Scenario: A path file round trip
- **WHEN** a log is written to text and read back, and damaged texts are read
- **THEN** every float restores bit for bit, and each damaged text is refused naming the problem (header, count, line, time going back)

#### Scenario: F9 in the real window
- **WHEN** the editor smoke runs with `PLANET_REPRO_DIR` set
- **THEN** one F9 write is reported, the directory holds `frame-<n>-path.txt` with at least two poses and `frame-<n>.bundle` that starts `planet-terrain-repro` and records the world

#### Scenario: A world with a stamp, a line and a brush stroke
- **WHEN** the definition of an editor holding a stamp, a line and a brush stroke is recorded and rebuilt from the recorded texts
- **THEN** the rebuilt definition equals the editor's (and hashes the same), and an item list that is not the definition's is refused naming both hashes

### Requirement: VIEW-026 A camera path replays through the plan and F9 says it wrote
`cargo xtask repro <frame-n-path.txt>` SHALL replay the path with its bundle: `test-render --repro BUNDLE --replay-path PATH --out-dir DIR [--replay-every N]` SHALL plan the terrain of a terrain repro bundle (a world, or the seed-only field) once per pose of the path file, in the logged order, with the bundle's lens, view and LOD parameters, and SHALL write `DIR/replay.txt` (`t_ms altitude_m nodes plan_hash finest_level plan_ms logged_frame_ms`, one line per pose) and, with `--replay-every N`, `DIR/pose_NNNN.png` for poses 0, N, 2N and so on. A pose the plan refuses (a terrain too steep for the bundle's LOD parameters at that camera) SHALL be written as a `REFUSED` line with the reason and the replay SHALL go on. It SHALL print the number of poses (refused ones included, with their reasons), the least and most nodes, the finest level, the slowest plan, and the poses where the finest level fell while the altitude fell, each with its time and altitude. `--replay-path` without `--repro` or `--out-dir`, a path file that does not parse (VIEW-024) or holds no pose, and a bundle that is not a terrain bundle SHALL be refused naming the problem. `cargo xtask repro <bundle>` SHALL name the `frame-<n>-path.txt` beside it when there is one. F9 SHALL show what it wrote (the two file names, the pose count and the span, the directory) in the window title for 8 s and print it with `PathLog::summary()`, and SHALL show a failed write the same way.

Verify: A, C
Status: active
Source: opus-planner (2026-10-19), VIEW-024, TEST-008

#### Scenario: A level that drops on approach
- **WHEN** the finest level falls from 19 to 17 while the altitude falls from 800 m to 700 m, and falls from 17 to 16 while the altitude rises
- **THEN** the first pose is listed as a drop with its time and altitude, and the second is not

#### Scenario: Files name each other
- **WHEN** `repro` is given `target/review/frame-120-path.txt` or `target/review/frame-120.bundle`
- **THEN** the bundle is `frame-120.bundle` and the path file `frame-120-path.txt` in the same directory

#### Scenario: Replay of what the real window wrote
- **WHEN** the editor smoke wrote `frame-<n>.bundle` and `frame-<n>-path.txt` and the path is replayed against the bundle
- **THEN** `replay.txt` holds one line per pose and the summary names that many poses

### Requirement: VIEW-027 Point lists of lines and regions
`planet_editor::polyline` SHALL hold the geometry of the line and region tools on points given as latitude and longitude in degrees, in f64 with `libm`, without a window, a GPU or an item. A line SHALL have 2 to 64 points, each finite and within 90 degrees of latitude and 180 of longitude, no two consecutive points at the same place (the same direction, not the same spelling: longitude 180 and -180 are one place). A region SHALL have 3 to 64 such points, every point within one cell edge (WDEF-007, 40 km) of the first, and an outline that does not cross itself (a touch counts), has an area, does not run back over itself along one line, and does not repeat its first point as its last; a region beyond the reach SHALL be refused with the limit in the message, and one that crosses itself or folds back with the two edges. A width that is not finite or is negative, and a radius that is not finite and positive, SHALL be refused. A line whose extent from its first point plus its width fits one cell edge, with at most 64 points, SHALL stay one part; a longer one SHALL be split into parts that each reach, width included, at most 80 % of the cell edge from their first point, hold at most 64 points, and share their boundary point exactly (the last point of a part is the first of the next), with a segment longer than half a part cut at great-circle points; a width that leaves no room SHALL be refused. A click, given as a unit direction, SHALL pick the nearest point within a distance, and the nearest segment (open, or closed for a region) with the foot of the click on it, an end point when the click is beyond the segment. Inserting, moving and removing a point SHALL return a new list and leave the input unchanged; removing SHALL be refused when fewer than the kind's minimum would remain.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, docs/status.md "Editor MVP requirements"

#### Scenario: A long line
- **WHEN** a line of 150 km with points every 10 km and a width of 200 m is split
- **THEN** it becomes at least four parts, each of 2 to 64 points reaching at most 32 km, each sharing its first point with the last of the one before, and every original point is a point of some part

#### Scenario: A region too large, and a bow tie
- **WHEN** a square region of 50 km sides and a square with two corners swapped are checked
- **THEN** the first is refused naming the 40 km limit and the second naming the two edges that cross

#### Scenario: A click
- **WHEN** a click lands 300 m beside the middle of the second segment of a line, and another lands beyond its last point
- **THEN** the first picks that segment with a foot on the line 300 m away, the second picks the last segment with its end point as the foot

### Requirement: VIEW-028 Lines and regions in the command layer
The command layer SHALL apply a group of steps (put a new item, replace the content of an item with its id, body and versions kept, delete an item) as one command: all steps happen or none does (a step that fails takes back those before it, the files included; a step that cannot be taken back SHALL be reported, never swallowed: the error names the items left as the command made them, and the editor then refuses every command, naming them, until `Editor::reload` reads the items of the body from the store again, which drops the undo and redo stacks), one undo takes all of them back to the exact previous definition hash, one redo does them again, and two steps on one item are refused. A group whose steps change nothing SHALL NOT be a command. A new item SHALL carry its points, group, part and mask layer (ADR 0069). A line (`ridge` or `channel` with `width_m` in its parameters) SHALL be put through a point list as one item when it fits the reach of one (VIEW-027), else as the linked parts of the split (one group, consecutive parts, each anchored at its first point with a reach that covers its points and its width), as one command; its points SHALL be changed by making the parts again, changing the items of the line in place, putting the extra and deleting the surplus ones, as one command, a line that becomes one part losing its group; and it SHALL be deleted whole as one command. The line commands SHALL refuse an item that is not a ridge or a channel, naming the kind, and a region put SHALL refuse a template that is not a region; points SHALL be written in the spelling of an item file (longitude 180 for -180, longitude 0 at a pole), so that what the point list accepts the item accepts; and a group id SHALL be made only for a line that splits. A region SHALL be put or re-pointed as one item, refused with the reason (the 40 km limit, the crossing edges) when its outline is not valid (VIEW-027); a refusal changes nothing.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, AUTH-004, VIEW-027

#### Scenario: A split line
- **WHEN** a line of 150 km is put and then undone and redone
- **THEN** it is four or more parts in one group, one undo removes all and restores the starting hash, one redo restores the end hash

#### Scenario: Re-splitting
- **WHEN** a point of the line moves, the line is shortened to two points, and then lengthened
- **THEN** each is one command; the parts keep their ids where they can, the surplus parts are deleted or the extra ones put, and one undo restores the exact previous hash

#### Scenario: A failure halfway
- **WHEN** the store fails on the third part of a split line, or a step is invalid, or two steps name one item
- **THEN** the definition, the files and the undo stack are as they were

### Requirement: VIEW-029 The line and region tools
With the line tool (`ridge` or `channel`) or the region tool on, a press on the planet SHALL add a point to a draft; a point within 0.5 m of the last one SHALL be refused as the same click again, a line draft SHALL hold at most 640 points and a region draft 64, a press off the planet SHALL do nothing, and changing the tool SHALL drop the draft. Finishing SHALL put the line (parameters `amount_m` 30 m and `width_m` 200 m, bound 60 m, layer `stamps`, name `Ridge 1`, `Channel 1`) or the region (parameters `surface_type` 0 and `mask_mode` 0, layer `regions`) as one command, select it and return to the select tool; fewer points than the kind needs (2 and 3), or a region the command layer refuses (VIEW-028), SHALL be refused with the reason and leave the draft. A line or region SHALL have one point handle per point, a line counting the points of all its parts joined, positioned on its points; while one is held the points SHALL be kept in the session (drawn from there, the editor told nothing) and the release SHALL make the one command that sets the points; Esc SHALL drop them with nothing recorded; a point list the item refuses SHALL leave the item as it was and the drag closed; undo, redo, delete, deselecting and changing the tool SHALL close a held point drag without telling the editor; a line SHALL count once in the numbering of its name however many items it is split into. A press on the outline of the selected line or region within a distance SHALL add a point at the foot, one command, the closing segment included for a region; a click on an existing point SHALL add nothing; a point SHALL be removed in one command and refused when fewer than the kind's minimum would be left.

Verify: A
Status: active
Source: opus-planner (2026-10-19), ADR 0069, VIEW-017, VIEW-019, VIEW-027, VIEW-028

#### Scenario: Draw a line
- **WHEN** the line tool is on and three places are clicked, the last again at once, and Enter finishes
- **THEN** the draft holds three points, the same place twice is refused, and finishing makes one command whose undo removes the line

#### Scenario: A point drag
- **WHEN** point 1 of a line is dragged over several moves and released
- **THEN** the editor holds the old points until the release, the handle follows the pointer meanwhile, the release is one command and one undo restores the old points

#### Scenario: A long line
- **WHEN** a point of the later part of a 150 km line is dragged
- **THEN** every point of every part has a handle, and the release is one command for the whole line

### Requirement: VIEW-031 Drawing lines and regions in the window
In the editor mode the rail SHALL offer the ridge, channel and region tools after the stamp tools, and the number keys 5, 6 and 7 SHALL pick them; picking one SHALL put the way to use it in the status line. Enter SHALL finish the line or region being drawn; Backspace SHALL take the last point of the draft back and never delete an edit (Delete does); Esc SHALL step back one thing at a time: a drag, then the draft, then the tool, then the selection. A press with Shift held on the outline of the selected line or region SHALL add a point there (refused, with the reason in the status line, when not on the outline) and a press with Ctrl held on one of its point handles SHALL remove that point (a Ctrl press that finds no handle does nothing but say so); neither starts a drag, and the distance from the outline that counts as on it is the one a click uses to pick (the half-width of a line or 1.5 % of the distance, the larger). F1, the key that types a question mark and a button in the status bar SHALL show and hide a window that lists the keys. A click SHALL pick a line whose distance to the click is within its half-width or 1.5 % of the distance to the surface, the nearest first, else a region the click is inside or within that tolerance of the edge; a stamp whose outline holds the click comes before both. The overlay SHALL give a line as an open path and a region as a closed one, each segment cut into 8 pieces along its great circle, a selected line or region as the session holds it (a held point already moved), and the draft as its path, its points and the cursor for a rubber band, reading the height of the ground once at each point and not at each piece; the window SHALL draw them.

Verify: A
Status: active
Source: opus-planner (2026-10-20), VIEW-017, VIEW-019, VIEW-020, VIEW-029

#### Scenario: Draw and finish a line
- **WHEN** the ridge tool is picked with 5, three places are clicked, the last point is taken back with Backspace and put again, and Enter finishes
- **THEN** the draft is shown with a line to the cursor, one command puts the line, it is selected, and its point handles are on the screen

#### Scenario: Shift and Ctrl
- **WHEN** Shift is held on a click between two points of the selected line, and Ctrl on the handle of a point
- **THEN** the first adds a point on the line, the second removes the point, and neither starts a drag

#### Scenario: Picking
- **WHEN** a click lands 150 m beside a line of half-width 200 m, 350 m beside it, and inside a region the line crosses but away from the line
- **THEN** the first picks the line, the second nothing, the third the region

### Requirement: VIEW-032 The coarse view keeps clear of the plan's refusal at the large levels, and a late refusal does not end the window
`LodParams::coarse` SHALL return the base parameters with a larger `tau_px` such that the morph interval of every level keeps its 5 % of the end distance plus a margin of 1 % (the node-radius rule, as `LodParams::validate` has it), and such that at every level whose node radius is at least half of `max_height_m` (`COARSE_WORST_CASE_RADIUS_FACTOR`) where the base parameters meet the plan's own start (`split(l) + 2 x extent(r (1 + H / R), H)`, `LodParams::morph_interval_worst`) the coarse ones meet it too with the same margin. Over a field that has only a global height bound this leaves the coarse view refused, where the base view is accepted, at no more than one pose in twenty-five of a sweep from 20 km altitude up (measured 0 % for the window's 320x200 and the 16-cell parameters, 1.4 to 2.7 % for the production 1080p parameters, all at level 11 between 31 and 39 km); a refusal that remains falls back to the base parameters as VIEW-014 says. `LodParams::validate` SHALL stay the node-radius rule. The production 1080p coarse factor is about 1.7 times the error bound (the window's 320x200 parameters about 2.2). A plan failure whose request carried other LOD parameters than the frame that receives it SHALL be dropped and counted (`PlanPipeline::dropped_plan_errors`), not returned. A failure of the frame's own request SHALL NOT end the window either: the frame keeps the newest plan in hand (drawn as stale, or the cover when there is none), the refusal is counted (`PlanPipeline::refused_plans`) and its newest message handed out by `PlanPipeline::take_refusal`; what the window prints of them is VIEW-040.

Verify: A
Status: active
Source: opus-planner (2026-10-20), SPIKE-19, VIEW-014, LOD-003, LOD-011

#### Scenario: The window's coarse view over the eroded world's bound
- **WHEN** the window's LOD at 320x200 over a field whose global height bound is 9,735 m is made coarse and planned from five orbital poses, and again from 40 altitudes between 20 km and 3,400 km at three poses (a face centre, a cube corner and a generic direction) at 320x200 and at 1920x1080
- **THEN** the plan accepts the five orbital poses (it refused them all with the former rule), and refuses the coarse LOD at no more than one pose in twenty-five among those where it accepts the base LOD

#### Scenario: A late refusal
- **WHEN** the plan of parameters too coarse for 16 cells is asked for and the frame then asks again with the base parameters, and later asks for the coarse parameters themselves
- **THEN** the first refusal is dropped and counted while the frame draws, and the refusal of the frame's own parameters is counted and reported once, and the frame still draws

### Requirement: VIEW-033 The inspector of a line and of a region
The inspector SHALL show a ridge's and a channel's `amount_m` (labelled Height and Depth; 0 to 1,000 m and 0 to 500 m) and `width_m` (Half-width; 1 m to 5 km) as typed rows with their ranges. A typed `amount_m` or `width_m` SHALL change every part of the line the selected item belongs to as one command (`Editor::set_line_params`): the parts are made again from the joined points of the line, so their reach covers the new width (a line that must have more parts is split again), the bound on the change is raised to at least the amount, and a value out of its range, a number that cannot be read, or a width that leaves no room (`WidthTooLarge`) is refused with the reason and changes nothing. For a region the inspector SHALL show a surface type (a choice among the names of `SURFACE_TYPES`, stored as the index) and whether it cuts the edits of another layer inside its outline (the mask mode 2, GEN-029; labelled `MASK_CUT_LABEL` "Removes the edits of a layer inside the outline", with the tooltip `MASK_CUT_HINT` saying that the edits are cut away inside with a hard edge and stay outside) with the layer it cuts (any other layer name; its own is refused): picking a surface type SHALL be one command, and setting the mask SHALL change the mask mode (2 or 0) and the mask layer (set or none) together in one command, so that no state in between reaches the world definition; cutting without a layer SHALL be refused with "choose the layer the region clips". The mask mode 1 (the edits kept only inside the outline, "clip outside" in ADR 0069) SHALL NOT be offered; a test holds the label and the tooltip to the stage's behaviour. The line kinds SHALL NOT be placed as stamps.

Verify: A
Status: active
Source: opus-planner (2026-10-20), ADR 0069, VIEW-017, WDEF-011, WDEF-012, VIEW-028

#### Scenario: A typed width on a split line
- **WHEN** a 150 km ridge of four or more parts is selected at its third part and a half-width of 800 m is typed, and then undone
- **THEN** every part has the half-width 800 m and a reach that covers it, it is one command, and one undo restores the former line exactly

#### Scenario: The mask of a region
- **WHEN** a region is set to clip the layer "stamps" and then to label only
- **THEN** the first change sets the mode 2 and the layer together, the second sets the mode 0 and removes the layer, and clipping with no layer or with the region's own layer is refused and changes nothing

### Requirement: VIEW-035 The brush tool
The editor mode SHALL have four brush tools (raise, lower, smooth, flatten; keys 8, 9, 0 and the minus key; rail buttons). With one on, a press over the planet starts a stroke, moving the pointer with the button down records its places, and the release puts the stroke as one command (one undo); Esc during the drag, a tool change and a lost focus put nothing. The recorded places SHALL be resampled along the great circles to a spacing of a quarter of the radius, cut into strokes of at most 64 points that stay within one item's reach (a cell edge less the radius), consecutive strokes sharing the point where they are cut, and filed by `Editor::add_stroke`: a stroke joins the first of the brush items this session painted into that is in the layer `brush` and has room (under 64 strokes and 4,096 points, the sum of the strengths within the limit) and reach (its extent from the item's anchor plus the largest radius), else it starts a brush item (kind `brush`, no parameters, no points) anchored at its first point with a reach of its extent plus the radius, at least 1 km and at most a cell edge, and a bound on the change of the exact sum of the strengths; the bound of an item a stroke joins is raised to hold the sum. The items one command starts SHALL share one group (consecutive parts) when there are several. The brush radius SHALL be 1 m to 5 km and the strength 0.01 m to 100 m, typed in metres in the inspector panel, a value outside is refused with the range and changes nothing, and a stroke keeps the numbers it started with, except that a raise or lower is filed with the strength that fits the slope guard (VIEW-043). A flatten SHALL level towards the typed height, else the terrain height under the start of the stroke; the other kinds have no target. A brush item SHALL have no handle and SHALL NOT be picked from the planet. The stroke being painted and a ring of the brush radius under the cursor SHALL be drawn.

Verify: A
Status: active
Source: owner (E6: raise, lower, smooth, flatten with metric radius and strength), opus-planner (2026-10-20), ADR 0070, WDEF-013, AUTH-005, GEN-030, VIEW-022

#### Scenario: A drag, then another stroke of the same area
- **WHEN** a raise stroke is dragged over 300 m and a lower stroke is dragged 100 m away, then the second is undone
- **THEN** the first is one brush item with one stroke, the second joined it (two strokes, bound 10 m), and the undo leaves the item with its first stroke exactly

#### Scenario: A long drag
- **WHEN** a 12 km stroke is dragged with a 200 m brush
- **THEN** it becomes linked brush items of one group in one command, no stroke over 64 points, each stroke starting at the point where the one before ended, and one undo removes them all

### Requirement: VIEW-036 The brush preview
A brush stroke SHALL be previewed (REND-033) as a chain of preview stamps along its line (`preview::brush_chain`). A raise SHALL be hill stamps and a lower the same with the sign turned, of the profile of the brush (`(1 - x^2)^2`), with a reach of 1.118 times the radius, both ends of the line included, a spacing of half the radius (wider when the stroke would need more than 14 stamps), and the amplitude that makes their sum the strength on the line, plus a dab of the radius at each end of the amount the chain falls short of the strength there (the stage's distance to the polyline is to the end point). A flatten SHALL be plateau stamps at the stroke's target with a reach of 0.8 of the radius and, as their bound, the strength shared over the plateaus that cover a point of the line, so that a row of them moves the ground by the strength and not by the strength times their number. A stroke of one point SHALL be one stamp of the whole strength. A stroke SHALL have at most 16 stamps. A smooth stroke SHALL NOT be previewed (a known limit: it moves the ground towards the mean of what is around it, which the stale tile cannot give; it shows when the exact tile arrives), nor a removed flatten (as a removed plateau is not). The chain SHALL stay within a third of the strength of the stage's delta, and within a tenth on average, along and across a bent stroke and along the longest stroke the tool records (64 points a quarter of the radius apart, 16 radii); a flatten SHALL stay within the strength.

While the pointer paints a stroke, the editor SHALL give the loop the chain of the stroke so far every frame as a transient preview, and when the stroke ends an empty list once; the put stroke is then previewed by the delta of the footprint items (`preview_delta_on`): a brush item added is the chains of its strokes (sign +1), one removed the same with the signs turned, one that gained strokes at the end only the chains of the new strokes, and one that lost strokes from the end only those strokes' chains with the signs turned. The stamps of a brush item SHALL keep the order of its strokes (a plateau reads the running height). The delta SHALL have at most 16 stamps (`MAX_DELTA_STAMPS`), kept in whole units, nearest the camera first, a unit being a plain stamp's entries or one stroke of a brush item (so that one large brush item is kept stroke by stroke, not dropped whole), and the transient preview at most 16 (`BRUSH_CHAIN_STAMPS`), so that both fit the 32 of REND-033 whole.

Verify: A, C
Status: active
Source: opus-planner (2026-10-20), VIEW-035, REND-033, GEN-030

#### Scenario: The stroke under the pointer
- **WHEN** a raise stroke is dragged and then released
- **THEN** each frame of the drag gives the loop the chain of the stroke so far as a transient preview, the release gives an empty list once, and the delta of the put stroke is a chain of hills of the same kind

#### Scenario: A stroke added to an item, and taken off
- **WHEN** a brush item gains a stroke at the end, and then loses it again
- **THEN** the first delta holds only the new stroke's stamps and the second only the same stamps with the signs turned

#### Scenario: A flatten and a lower in one item
- **WHEN** a brush item holds a flatten and then a lower
- **THEN** the delta lists the flatten's plateaus before the lower's hills

### Requirement: VIEW-037 A failure that ends the window leaves a crash report, and F9 saves the frame
A panic, an error that ends the editor run, and a lost GPU device SHALL write `target/review/crashes/<UTC time>.txt` (`PLANET_CRASH_DIR` overrides the directory; `crash::report`, once per run) holding the message, the location, the backtrace, the build commit, the command line, the camera path of the last minute (the F9 path file, refreshed about once a second) and the last 200 log lines, and print its path. A plan refusal the window degrades from (VIEW-032) SHALL write the same report (`crash::snapshot`, every time, without using up the once-per-run report) and name it in the title notice. F9 SHALL also write `frame-<n>.png` beside the bundle and the path file: the renderer's last submitted colour target, the final tone-mapped scene without the egui panels; the F9 notice names it.

Verify: A
Status: active
Source: opus-planner (2026-10-20), VIEW-024, VIEW-026, VIEW-032

#### Scenario: A report
- **WHEN** a report is written after a log line was noted
- **THEN** the file holds the message, location, backtrace, commit, command line, path text and the log line, and a second request in the same run writes nothing

### Requirement: VIEW-038 A refused plan makes a measured run invalid
A run that measures or checks (the perf scenes of editperf, streamperf and litperf, the pipelined loop of test-render, the scheduler ablation) SHALL begin its frames with `ManagedLoop::begin_frame_strict`: from the first frame after the plan worker refused a plan for the frame's own request it returns the error `run invalid: the plan was refused N time(s); first refusal: <message>`, which ends the scene with a non-zero exit. The window's loop keeps `begin_frame` and degrades (VIEW-032).

Verify: A
Status: active
Source: opus-planner (2026-10-20), VIEW-032, VIEW-037

#### Scenario: A refusal in a measured run
- **WHEN** the plan of a frame's own request is refused by the worker and the next frame is begun strictly
- **THEN** the call returns the error naming the count and the refusal, while `begin_frame` of the same loop returns the frame

### Requirement: VIEW-040 A flight over ground the plan refuses prints a few lines, not one per frame
The window SHALL print, write a refusal report for (`crash::snapshot`) and name in the title notice a plan refusal at most once per cause in `REFUSAL_REPORT_PERIOD_MS` (5 s), a cause being the kind of the refusal and the level of the tile it names (`refusal_cause`; not the tile, the distances or the frame); the line says how many refusals of the cause came in between (`RefusalReporter`). The title SHALL show the count of all refused plans (`plans refused xN`, `PlanPipeline::refused_plans`). The window SHALL also print one line, at most once in `REFUSAL_REPORT_PERIOD_MS` (5 s), saying how many tiles of the drawn plan are held back from refinement because the ground under them is too steep to morph (ADR 0074, `CappedReporter`), and nothing while none is; it is not a refusal and writes no crash report. A refused frame SHALL cost no more than a frame whose plan was made: the request is repeated only when the camera or the definition changed, and the commit of the report is asked of git once per run.

Verify: A
Status: active
Source: opus-planner (2026-10-20), owner finding on PR 483, VIEW-032, VIEW-037

#### Scenario: A hundred refused frames
- **WHEN** a camera moving 16 ms a frame over ground the worker refuses gets 100 refusals, each naming another tile and distance
- **THEN** at most three lines are printed, the same cause is due again after 5 s with the number in between, and another level is another cause

### Requirement: VIEW-039 The keys of the editor and the camera are never taken by a panel's focus, and a focus gain presses nothing
Only a text field being edited (`Context::text_edit_focused`) SHALL take the keys from the camera, the commands and the editor mode; any other widget that holds the keyboard focus lets it go at the next key (`Router::key` is the UI's only while a text field is edited). Tab outside a text field SHALL reach the editor toggle and never egui (no focus traversal over the panels). A key press that winit makes up when the window gains the focus (`is_synthetic`, Alt+Tab back in holds Tab) SHALL do nothing; a release always counts. A capture SHALL forget the pointer target latched at the last press (egui gets no pointer events while the pointer is captured).

Verify: A
Status: active
Source: opus-planner (2026-10-20), owner test P1-3, VIEW-022, VIEW-034

#### Scenario: A button holds the focus
- **WHEN** a panel button holds the keyboard focus, as after a Tab through the panels, and a key arrives
- **THEN** the key is the world's, the focus is let go, and Tab is the editor toggle's

### Requirement: VIEW-041 The inspector steps values, the wheel sets the brush, and a click again goes through the edits under it
A numeric field of the inspector and of the brush section SHALL commit on Enter and keep the focus; Up and Down SHALL step its value and commit it at once (`params::stepped`: the larger of the field's own step and 1 % of the value, ten times with Shift, kept inside the field's range; `panels::value_field`). With a brush tool on in the editor mode the mouse wheel SHALL set the brush radius in geometric steps of 10 % a line and Shift+wheel the strength, inside the brush's ranges (`EditRuntime::wheel`); without a brush tool, and with the mode off, the wheel is left to the overview's zoom. A press with the select tool SHALL select the edit with the smallest footprint among those under the cursor (`Session::pick_all`: the stamps whose outline holds the hit, smallest first, then the line or region `pick` takes); a press at the same spot (`SAME_SPOT_PX`, 4 px) while one of them is selected SHALL select the next in the same list, wrapping round. The hover label SHALL name the edit a press would select and its place, `(1 of 3)`, when more than one edit is under the cursor.

Verify: A
Status: active
Source: opus-planner (2026-10-21), owner test P2-2, P2-3, P2-4, VIEW-022, VIEW-033, VIEW-035

#### Scenario: A hill inside a plateau
- **WHEN** a plateau, a hill and a crater are placed over one spot and the select tool presses there four times
- **THEN** the edits are selected smallest footprint first and then round again, the hover label names "(1 of 3)" before the first press, and a press 60 px away starts at the smallest again

### Requirement: VIEW-045 The overview says where a click acts
In the globe overview `GlobeSession::click_target` SHALL name what a click at the cursor would do without doing it: a marker within the hit radius, else the surface under the cursor at any altitude (an orbital one too: the flight to the place starts there), `Flying` while a flight runs, `Sky` over the sky or past the horizon. The window SHALL show the pointer cursor only where a click acts (`ClickTarget::acts`) and the arrow elsewhere, SHALL put the hover hint in the title only there (`click to fly here`, `click to fly to this item`), and SHALL say in the title's notice why a click was ignored: `click ignored: a flight is in progress`, `click ignored: the cursor is not over the planet`, `click ignored: the cursor is over a panel`.

Verify: A
Status: active
Source: opus-planner (2026-10-21), owner test P2-9, VIEW-013

#### Scenario: A click at orbital altitude
- **WHEN** the overview is zoomed out to orbital altitudes and the cursor is over the planet, over a marker, over the sky, or a flight is running
- **THEN** the targets are the surface, the marker, the sky (with its reason) and the flight (with its reason), and a click on the first two starts a flight

### Requirement: VIEW-044 The stale-tile overlay: which tiles are current
`Overlay::Stale` (shader code 4, plan token `overlay stale`; the fifth F3 stop) SHALL tint each drawn tile by its state against the current definition, with the tile's outline in the state's colour: exact green (resident with the plan's key), stale amber (a resident tile of another version, or one an edit named, drawn while its refresh is pending), stand-in red (a coarser ancestor drawn over the quadrants of nodes that are not resident, or the cover when no plan is in hand), previewed violet (a preview stamp or a brush stroke under the pointer displaces it), capped grey (a plan leaf held back from splitting because the tiles under it are too steep to morph, ADR 0074: the planner sets `TileState::Capped` on it when it is otherwise exact; the tiles and mesh overlays draw such a tile grey too, whatever its level). The state is `TerrainDraw::state` (`TileState`), set by the draw plan and carried in the node record (the level plus 32 times the state code); it is not in the plan text, so no recorded plan changes. A legend of the five colours SHALL be drawn in the top right corner while the overlay is on, and the title SHALL show the counts (`tiles 120 exact, 4 stale, 2 stand-in, 1 previewed`). The overlay is off at the start and changes no frame when off. The smoke run shows it for some lit frames and its verdict wants them (`SmokeCounts::stale_overlay_frames`).

Verify: A
Status: active
Source: opus-planner (2026-10-21), owner ("it should be visible which tiles are updated and which are still stale"), VIEW-014, REND-032, REND-033, M7 step A2

#### Scenario: An edit in flight
- **WHEN** an edit names three resident tiles, one tile of the plan is not resident, and a stamp displaces tiles under the pointer
- **THEN** the three tiles are stale, the missing tile's ancestor is a stand-in, the displaced tiles are previewed and the rest are exact, and the legend and the counts say so

### Requirement: VIEW-042 The editing light: a sun fixed in camera space
H SHALL switch the editing light on and off (`Command::ToggleEditingLight`; off at the start, so the look's sun and every golden stay; not taken by the editor mode). While it is on, every lit frame SHALL take `editing_sun_dir(camera)` for `LitPlan::sun_dir`, one sun for the terrain shading, the atmosphere and the sky (the plan has a single sun direction): the unit vector `EDITING_SUN_ELEVATION_DEG` (35) above the horizon of the viewer's place and `EDITING_SUN_AZIMUTH_DEG` (40) to the right of the direction behind the viewer, so the view is lit from behind and over the right shoulder and no slope it shows faces away; for a view straight up or down the camera's up stands for the heading. It is a pure function of the camera, converted every frame. The title SHALL name it (`editing light`). The smoke run SHALL switch it on for a few lit frames and its verdict SHALL want them (`SmokeCounts::editing_light_frames`).

Verify: A
Status: active
Source: opus-planner (2026-10-21), owner test P2-5, VIEW-007

#### Scenario: The sun follows the view
- **WHEN** the camera looks along a heading and then a quarter circle to the right, from the same place
- **THEN** the sun is 35 degrees above the horizon both times, behind the viewer and to the right of the heading, and at the same angles to the new heading

### Requirement: VIEW-043 The brush keeps the ground it makes drawable
`LodParams::drawable_slope` SHALL be the steepest slope (rise over run) that a tile's neighbourhood can have and still leave the leaf a morph interval (LOD-008): with `rho = hypot(r, H)` for the node radius bound `r` and the half range `H` of a neighbour's height bounds, the interval's start `split(L) + 2 rho` stays within 95 % of the end `split(L - 1)`, which gives `H / r` (about 2.5, 68 degrees, at the defaults; the same at every level; 0 where the parameters leave no room even on flat ground).

The editor's brush SHALL limit the stacked gradient of its raise and lower strokes:

| Quantity | Value |
|---|---|
| Limit | `gradient_limit = max(0, drawable_slope - BASE_SLOPE_SHARE)`, `BASE_SLOPE_SHARE` 1.0 (the share left to the base and the other edits), about 1.5 at the defaults |
| Gradient of one stroke | at most `STROKE_GRADIENT s / r` for strength `s` and radius `r`, `STROKE_GRADIENT` 1.5396 (the largest of `4 x (1 - x^2)`); a raise and a lower by magnitude; a flatten and a smooth not counted |
| Stack at a point | the sum over the strokes whose disc reaches the stroke being painted (conservative: the maxima rarely coincide) |
| Where it is read | on the pieces the stroke is filed as (resampled to a quarter of the radius, cut at 64 points); a stroke of several pieces counts its own gradient twice (the pieces share their cut point at full weight) |
| A raise or lower over the limit | filed with the strength that fits (`slope::allowed_strength`), status "<kind> stroke clamped to <s> m: the ground would be too steep to draw (<asked> m was asked)" |
| No room (under the smallest strength of the brush) | refused, status "<kind> stroke not painted: the ground there is already too steep to draw", nothing put |
| Item file, schema, items already filed | unchanged |

- The session SHALL take the drawable slope from the window's LOD parameters when the editor is created (`Session::set_drawable_slope`).
- A brush item (or a linked group) whose stacked gradient over the points of its own strokes is over the limit SHALL carry the warning "too steep to draw fully" (`slope::WARNING`) in the inspector and in its row of every list (nearby, search, apply order, layers: `Inspector::warning`, `Row::steep`), found once per revision of the editor.

The guard does not cover, and the warning does not find:
- the base terrain, stamps, lines and regions (the share left to them is a constant, not derived), and what a flatten onto a far target or a smooth does at its edge;
- strokes filed before the guard or from older files (flagged, not clamped), and a redo after other strokes were added (not checked again);
- a lower on a stack over the limit (it counts by magnitude: the way back is undo or deleting the item);
- a change of the window's LOD parameters after the editor is created; the preview of a stroke in progress, which shows the typed strength where the release may clamp it.

Verify: A
Status: active
Source: opus-planner (2026-10-21), SPIKE-26, ADR 0074, ADR 0070, VIEW-035, LOD-008

#### Scenario: The limit follows the parameters
- **WHEN** the drawable slope is read at the production parameters, at a smaller tolerance and where there is no room
- **THEN** it is about 2.5, larger for the smaller tolerance and 0 where there is none, a neighbour of half range `slope * r` leaves the interval its 5 % and one a thousandth more does not, at every level, and the guard's limit is that slope less the share for the base

#### Scenario: A stack of raise strokes
- **WHEN** raise strokes of 5 m and 100 m radius are painted on one spot until the limit, then once more, then again
- **THEN** the strokes up to the limit are filed as painted, the next is filed with a smaller strength and the notice, the stack stays under the limit at the spot, the one after is refused with the reason, and undo takes the clamped stroke back

#### Scenario: What counts
- **WHEN** a lower is painted on a full stack, a lower 10 km away, and a flatten and a smooth on the stack
- **THEN** the lower on the stack is held back as a raise is, the one far away is filed as painted, and the flatten and the smooth are filed as painted without the notice

#### Scenario: A long drag
- **WHEN** a drag given by two raw points passes 120 m from a stack of three strokes, and a drag of 600 m and a click are painted with the same strength on empty ground
- **THEN** the drag is clamped along its resampled path, and the long drag gets about half the strength of the click (its pieces count twice at their joints)

#### Scenario: An item already too steep
- **WHEN** 40 strokes of 5 m and 100 m radius are filed on one spot without the guard, and a single stroke elsewhere
- **THEN** the stack's item carries the warning in the inspector and its row of the nearby, search, apply-order and layer lists, and the other does not

### Requirement: VIEW-034 The movement keys are the camera's while the pointer looks around
While the pointer is captured for looking around, the window SHALL give every key press to the camera and the commands, whether or not a text field of the editor UI has the keyboard focus (`window::key_goes_to_world`), and starting a capture SHALL take the keyboard focus from the widget that has it (`Ui::release_keyboard_focus`), so that a field the user clicked into earlier does not swallow W, A, S, D, Q and E. With the pointer free a key is the camera's unless a text field has the focus, as before. The window title SHALL end with the input state (`viewer::input_text`: the keys held, whether the pointer is captured, whether the overview is shown), so that keys that do not arrive can be told from a camera that does not move. The free camera SHALL move with the movement keys after the overview was shown and left (G and G), with a key held across the overview, and after a click flight landed.

Verify: A
Status: active
Source: owner (2026-10-20, via opus-planner), VIEW-010, VIEW-011, VIEW-013

#### Scenario: A field keeps the focus, then the pointer looks around
- **WHEN** the user clicked into a text field and then starts looking around with the pointer
- **THEN** the focus is taken from the field, W moves the camera, and a key press is the camera's for as long as the pointer is captured

#### Scenario: G and G, and a landing
- **WHEN** the free camera is flown, the overview is shown and left, or a click flight on a marker lands
- **THEN** W moves the free camera afterwards

### Requirement: VIEW-046 F6 cycles the ground detail
The editor window SHALL show the procedural ground detail and gloss (REND-034, REND-035) at the strength `GroundDetailMode` names: F6 cycles natural, full and off (`planet_frame::look::materials::GroundDetailMode`; strengths `NATURAL_STRENGTH` 0.6, 1 and 0); the window starts at natural. The window's title SHALL name the mode (`detail natural`, `detail full`, `detail off`), and the keys help (`KEYS_HELP`, the editor's help text) SHALL name F6. The plan's strength is 0 for every plan the window does not make (`test-render` and the goldens), so no golden moves.

Verify: A
Status: active
Source: owner (2026-10-21, via opus-planner): the detail must be visible in the editor and viewer without code changes, VIEW-022, VIEW-014

#### Scenario: F6
- **WHEN** F6 is pressed four times in a viewer at its start
- **THEN** the mode is natural at the start, then full, off, natural, full; the strengths are 0.6, 1 and 0 for natural, full and off; the other switches are unchanged; the help names F6 and the ground detail, F6 is bound to the command and no other key is

### Requirement: VIEW-047 The problems list
The editor's left panel SHALL have a tab `problems` (`Tab::Problems`, last of the default tabs; its title shows the count, `Problems (3)`) that lists every edit the engine cannot draw fully or that costs more than it should, so the owner can find and fix it. An edit is listed when it is (1) a brush too steep to draw fully (VIEW-043), (2) a brush with very many strokes stacked: the discs of at least `STACK_WARN` = 8 strokes (of any kind and item) cover one point of its strokes, or the item holds at least `STROKES_WARN` = 48 of the 64 strokes a file allows, or (3) over a tile the last plan held back (ADR 0074, `TileState::Capped`, VIEW-044): the footprint find over the capped tiles of the plan, without the selected edit. Each row names the edit, its distance and every issue in words (`too steep to draw fully`, `9 strokes stacked at one place (12 of 64 strokes)`, `tiles near it are held back (too steep to refine)`), and has the buttons Fly to (the camera goes there, as a double click in a list does), Reduce, Merge strokes and Delete; clicking the name selects. The rows are sorted steep first, then stacked, then capped, then by distance. Reduce multiplies the strengths of the raises and lowers of the brush by the largest factor, to 1 %, under which it is no longer too steep with the other edits as they are (`problems::fit_factor`), by one half for a stacked brush that is not too steep, and is refused naming the other edits when even a hundredth of its strengths is too steep. Merge strokes replaces the stack of overlapping strokes of one kind (a raise or a lower; the deepest stack, the first stroke keeps its place and its path) by one stroke of the largest radius and the summed strength, and is refused when no two strokes of one kind overlap; the merged ground is not the stack's ground (the others' paths are dropped), so it is offered as a simplification. A fix that does not apply is disabled (Reduce and Merge on a stamp or a line). Every action is one command: one undo takes it back to the exact definition, and the places it changed are refreshed like any edit's. Presentation and authoring only (no generator output changes except through the edit the owner chose).

Verify: A
Status: active
Source: M7 A5 (owner, 2026-10-21: the owner may attempt what the engine does not support; it must not crash, must not cost much performance, and must be findable and fixable), VIEW-043, VIEW-044, VIEW-018, ADR 0074

#### Scenario: What is listed
- **WHEN** a world has a brush of 40 raises of 5 m on 100 m, one of 9 light raises 20 km away, one of 3 raises, a hill that is over a capped tile, and a brush of 50 strokes a hundred metres apart
- **THEN** the first two, the hill and the last are listed with the issues above (the 40 raises steep and stacked, the nine stacked only, the hill capped only, the fifty stacked by count), the three raises are not, the steep one is first and the hill last, and a capped tile on the far side of the planet lists nothing

#### Scenario: Reduce
- **WHEN** the steep brush is reduced, then undone, and the stacked one is reduced, and a stamp and a brush made too steep by another item are given to Reduce
- **THEN** the first fits afterwards (every strength scaled by one factor between 0 and 1, the item valid), undo restores the exact definition, the stacked one has half its strengths, the stamp is refused (no strokes) and the other is refused (the other edits), changing nothing

#### Scenario: Merge
- **WHEN** a brush of 12 raises on one place is merged, and a list of strokes with three raises on a place, a flatten and a lower there and a raise far away is merged
- **THEN** the first is one stroke of the summed strength (valid, one undo restores it, a second merge is refused), the list has the three raises as one (the first one's path, the largest radius, the sum), the flatten, the lower and the far raise untouched, and strokes of different kinds, flattens and smooths, and strokes that do not overlap are never merged

#### Scenario: The panels
- **WHEN** the tab is shown with no problem, then with a steep and stacked brush and a capped edit, and its buttons are clicked
- **THEN** it says there is no problem, then each row has its name, its issues and the four buttons, which ask for Fly to, Reduce, Merge, Delete and Select, and Reduce and Merge on the capped edit are disabled and ask for nothing

### Requirement: VIEW-048 The edit-health lists follow the items
Brushes too steep to draw fully (VIEW-043), brushes stacked and whether a stack can be merged (VIEW-047) SHALL be kept up to date as items change, not read from all items at each change (SPIKE-29). The editor SHALL log the ids of changed items (`Editor::change_seq`, `Editor::changes_since`). The session SHALL file brush items in a grid of 0.02 degree cells by the circle bounding their strokes (`health::Health`) and re-check only the changed items and those whose circle meets an old or new circle of a changed item. The first call, a new drawable slope and a log that no longer reaches back start again from all items. The result SHALL equal the full scans `slope::steep_items` and `problems::stacked_items`. `Session::health_checked` reports the work.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 2: incremental and lazy lists, the work per change bounded), VIEW-043, VIEW-047

#### Scenario: Equal to the full scans
- **WHEN** 30 brushes in four small areas are put, then 60 random edits (put, change, delete, undo) are made, and the drawable slope is changed
- **THEN** after the puts, after every edit and after the slope change the steep set and the stacked map equal those of the full scans over the same items

#### Scenario: The work follows the neighbours, not the items
- **WHEN** a grid of 625 and then of 2,500 isolated brushes a kilometre apart is loaded and the lists asked for, then one is changed, one is put on top of another, that one is deleted, and 40 strokes are piled on one place and one of them changed
- **THEN** at both sizes the first call checks every item, the change checks 1, asking again checks nothing new, the brush on top checks 2, the delete checks 1, the change in the pile checks at most 42, and only the pile is listed

### Requirement: VIEW-049 The lists read what is near
The group warning of a row (any item of its group over the limit) SHALL be read from an index of the groups, not by scanning the items. The problems list SHALL look up its rows, not scan the items for them. Whether a stack can be merged SHALL be remembered per item until that item changes. The slope guard of a stroke (VIEW-043) SHALL read only the strokes of the brush items whose footprint meets the circle round the stroke: the same strokes in the same order, so the same strength. The lists are computed when asked, once per change; a list that is not shown does no work.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 2), VIEW-043, VIEW-047, VIEW-048

#### Scenario: Same answers
- **WHEN** the problems list, the row warnings and the slope guard are asked for after the edits of the VIEW-048 scenarios
- **THEN** they equal what the scans over all items give

### Requirement: VIEW-050 F7 drops scale references
F7 SHALL drop plain boxes of known size on the ground in front of the camera, and take them away when pressed again (`Command::ToggleScaleRefs`, off at the start). `planet_frame::scalerefs::drop_references` decides them from the camera, its heading and the ground height: a chair 0.9 m high, a house 8 m wide, a boat 10 m long, a tower 291 m high and a block of 10 x 10 boxes 100 m apart (1 km across). Each stands ahead of the camera on the lowest ground under its footprint, none overlaps another, and the same camera gives the same drop. The window title names the references while they are dropped; the keys help names F7.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner): scale references to judge the ground detail by, VIEW-046, REND-036

#### Scenario: The sizes and the places
- **WHEN** a drop is made at a camera with a flat ground, and at one with a ground that rises ahead
- **THEN** the boxes have the named sizes, the block has 100 boxes spanning 950 m, each box stands on the lowest ground under its footprint ahead of the camera and none overlaps another, and a camera looking straight down still gets the drop

#### Scenario: The key
- **WHEN** F7 is pressed twice in a viewer
- **THEN** the references are asked for, then not, and the help text and the editor's help name F7

### Requirement: VIEW-051 The shown edits are bounded
The editor SHALL show at most `MAX_SHOWN` = 400 edits at once, (SPIKE-29). Of the edits VIEW-015 shows it keeps those whose reach disc can be seen (`relevance::in_view`: the cone of the disc, widened by `HEIGHT_MARGIN_M` = 4 km for the ground's height, meets the cone of the camera's view) and, when more than 400 remain, the 400 with the largest reach on the screen (ties by the lower id), and the selected edit besides. The problems list (VIEW-047) is not limited. The footprint find is kept while the definition revision and the tiles stay, and the ground heights of the outlines while the field's item revision stays.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 3: separate the shown set from the item count, then bound it), VIEW-015

#### Scenario: The cap
- **WHEN** 800 hills lie under a camera 400 m up looking down, one of those left out is selected, and a world of 30 hills is asked the same
- **THEN** 400 are shown, none left out is larger on the screen than one shown, the same camera shows the same edits, the selected one is added, and the 30 are those the plain find gives

#### Scenario: The view
- **WHEN** a camera 12 km from the hills looks at them, and one looks away
- **THEN** the first shows some, the second only the selected edit, and a disc beside the view is not in view

#### Scenario: Reuse
- **WHEN** the camera moves with the world unchanged, and then an edit is made
- **THEN** the session's answer equals the plain function's each time, and the new edit is shown at the next call

### Requirement: VIEW-052 The editor opens the production store
The editor SHALL, when `--store`, `--world` and `--global-res` are not given and the run is not a smoke run, open the first existing directory of: the one in the `PLANET_STORE` environment variable, the one named in the pointer file `target/stores/default`, `target/stores/global16384`, `target/accept/global16384` (relative paths under the repo root). With none it SHALL open the in-memory preview bake. The window title SHALL name the source: `store <dir>`, or `preview bake <N> cells`. `cargo xtask store [--res N] [--relief-scale S] [--link DIR]` builds a store of N cells per face edge (default 16384) into `target/stores/global<N>` and writes the pointer file, or with `--link` only writes the pointer file for an existing store.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner): the editor showed a flat world because it ran on the 256-cell preview bake

#### Scenario: The order
- **WHEN** a root holds none, then an accept store, a stores store, a pointer file and an environment variable, in that order
- **THEN** the choice is none, the accept store, the stores store, the pointer's, the variable's, and a variable naming no directory falls through

### Requirement: VIEW-058 The flight site
`planet editor --site LAT,LON` SHALL start the scripted flight (VIEW-005) over that point, in degrees; without it the flight descends over latitude 20, longitude 30. A latitude beyond 90 degrees, a longitude beyond 360 degrees or a value that is not a number SHALL be refused with a message naming the option.

Verify: A
Status: active
Source: m8-refresh-priority, VIEW-005

#### Scenario: A flight over another site
- **WHEN** `--site 45,-100` is given
- **THEN** the options hold the site (45, -100), the script of the flight starts over it and differs from the default site's, and `--site 95,0` is refused
