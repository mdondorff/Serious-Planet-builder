## MODIFIED Requirements

### Requirement: VIEW-003 Commands, statistics, title and readiness
The viewer SHALL map the keys P and Space (pause), R (restart), L (the lit path with the atmosphere, or the debug path), V (next debug view on the debug path, cycling through all views and back; no effect on the lit path), T (vsync on or off), C (flight or free camera), F9 (write a repro bundle of the frame that was just drawn, TEST-008) and Esc (quit), and the held keys of VIEW-004, in a table that is tested without a window, and the help text SHALL name each. It SHALL show in the window title the frame rate and the mean CPU frame time over a window of injected time, the GPU time of the terrain pass (one frame a second is timed; `gpu n/a` without timestamps), the vsync state with the present mode in use, what is drawn (`lit + atmosphere`, `lit`, or the debug view), on the lit path the exposure (VIEW-007), the flight time or the free camera, the altitude and the readiness report (ADR 0045 point 5): bake blocks done of total, pre-fill tiles done of total, and the share of the newest plan's wanted tiles drawn at their own level with the mean lag in levels. The readiness report SHALL be GPU-free data.

Verify: A
Status: active
Source: ADR 0042 point 4, ADR 0045 point 5, ADR 0032 point 4, owner 2026-10-17 (item 5b)

#### Scenario: Views cycle
- **WHEN** V is pressed once per debug view on the debug path
- **THEN** every view is shown once and the first is shown again; on the lit path V changes nothing

#### Scenario: Capture
- **WHEN** F9 is pressed in the viewer
- **THEN** the frame drawn next is written as a version 3 repro bundle to `target/review/frame-<n>.txt` (n counts from 1 in the run), the title names the path for a few seconds, a failure to write is named there instead, and a run without F9 writes nothing

#### Scenario: Coverage
- **WHEN** only the root is drawn for four wanted children
- **THEN** the view coverage is 0 of 4 with a mean lag of 1 level; with all four drawn it is 4 of 4

#### Scenario: Title
- **WHEN** the title is built from a frame rate, times, state and readiness
- **THEN** it contains each of them in a fixed format, `gpu n/a` when no GPU time exists, and `loading` or `ready` for the readiness; on the lit path it names `lit + atmosphere` (or `lit`) and the exposure after the vsync state, on the debug path the debug view and no exposure

