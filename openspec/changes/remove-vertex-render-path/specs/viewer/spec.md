## MODIFIED Requirements

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

