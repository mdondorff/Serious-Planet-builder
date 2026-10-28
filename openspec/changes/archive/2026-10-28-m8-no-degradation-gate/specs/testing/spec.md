## ADDED Requirements

### Requirement: TEST-061 The no-degradation gate counters and limits
The window SHALL, when `PLANET_GATE` is set, count the main-thread time of every frame, the tiles the plan holds back (ADR 0074), the horizon maps baked and the panics caught, and the gate SHALL fail the run on a frame over 100 ms, a held-back tile with no edit in reach, no horizon map over a lit run with a chunk stage, a caught panic, or a step between levels over the bound of M8 exit criterion 3. A failure SHALL name the counter, the frame, the phase or the tile IDs.

Verify: A, C
Status: active
Source: owner (2026-10-28), owner test 2026-10-27 findings 7, 11 and 16, ADR 0074, TEST-058

Detail: A frame's time is its phases without the waits for the GPU, and the phase of the longest one is named. Held-back tiles are listed with their IDs and whether an edit is in reach; those under an edit are reported and not failed. The first 20 frames are not judged. On the software adapter (`PLANET_GATE_SOFTWARE`, the CI run) frame time and horizon maps are reported and not judged, because its rendering shares the CPU with the main thread; held-back tiles and panics are always judged. The step between levels is measured by `cargo xtask gate` (the field-side `anet_steps_report`, levels above the entry level), not by the window.
#### Scenario: A held-back tile away from an edit
- **WHEN** the counters record a frame whose plan holds back 3 tiles and no edit reaches them
- **THEN** the verdict fails naming the 3 tile IDs, and the frame

#### Scenario: A held-back tile under an edit
- **WHEN** the counters record held-back tiles all within the reach of an edit
- **THEN** the verdict passes and the report lists the tiles as flagged

#### Scenario: A slow frame
- **WHEN** a frame takes 140 ms of main-thread time in the phase "chunk frame"
- **THEN** the verdict fails naming the frame, 140 ms and the phase

### Requirement: TEST-062 The gate runs the real window path
`cargo xtask gate` SHALL run the editor window path with the chunk stage on over `C:/DEV/SeriousPlanetData/global16384` (opened read-only, outputs under `target/gate/`), script one large edit (a 400 m hill), and replay the camera paths of the owner test (`frame-*-path.txt`) over the capture sites, reporting the counters of TEST-061 per site. `cargo xtask gate --small` SHALL run the same counters and verdict on a generated 32-cell world with its coupled run and coarse horizon on the software adapter. Neither SHALL run a perf measurement.

Verify: C
Status: active
Source: owner (2026-10-28), TEST-061

#### Scenario: The small world in CI
- **WHEN** `cargo xtask gate --small` runs on the software adapter
- **THEN** it prints the counter table and exits non-zero exactly when the verdict of TEST-061 fails

### Requirement: TEST-063 The gate is part of the pre-push run and known failures are quarantined
`cargo xtask prepush` SHALL run `gate --small` when the diff against `origin/main` touches `generators`, `streaming`, `render`, `editor`, `frame`, `app` or `cache` (the shaders are under `render`). A check that fails on main at the gate's landing SHALL be listed in a known-failures table of the gate with a Tracked entry in `docs/status.md`; the run reports it as known and fails on any other failure; no limit changes.

Verify: A
Status: active
Source: owner (2026-10-28), CLAUDE.md prepush rule

#### Scenario: Paths that trigger the gate
- **WHEN** the changed paths include `crates/streaming/src/chunks.rs`
- **THEN** the pre-push run includes the small gate; for `docs/status.md` alone it does not
