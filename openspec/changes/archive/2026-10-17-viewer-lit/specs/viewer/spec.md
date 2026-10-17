## MODIFIED Requirements

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

## ADDED Requirements

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
