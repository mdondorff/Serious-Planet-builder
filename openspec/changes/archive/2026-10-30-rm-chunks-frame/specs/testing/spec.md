## MODIFIED Requirements

### Requirement: TEST-047 The speed-envelope scene
`planet test-render --scene envelope` (`cargo xtask perf envelope`) SHALL, for each altitude of the table below, fly level over ground that is not in the cache at a constant speed along a meridian, 8 degrees below the horizon, and find the highest speed whose worst window of 1 s of frames has a coarse share (TEST-046) of at most the bound. It SHALL search the speed as the table states, write the log named below, and judge the coarse share. The scene runs the readiness gate and the reference-adapter check (ADR 0020), and is never run by an agent without the architect's window.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-V), M8 exit criterion 7 (owner, 2026-10-23, D4)

Detail:

| Item | Value |
|---|---|
| Altitudes | 30 m, 100 m, 300 m, 1 km |
| Start place | one of its own per trial, an empty tile cache |
| Bound | default 1 % (`--share-bound`) |
| Search | start at 10 m/s; double while a trial passes, up to 3,000 m/s; then halve the gap between the highest passing and the lowest failing speed four times; a trial that fails at the first speed reports no speed |
| Log | `perf/logs/envelope.json` (or `--out`): per altitude the highest speed, the worst window's share at it, the trials (speed and share), whether the cap was reached, the bound, the window, the frame step, the source and the adapter; a one-line summary per altitude on the output |
| Log fields | `coarse_share` (pixel-weighted) and its weighting in words |
| `--smoke` | one short trial per altitude on the software adapter; measures nothing |

#### Scenario: The search
- **WHEN** the search runs against a fake trial that passes below a limit speed and fails above it, against one that never fails, and against one that fails at the first speed
- **THEN** it returns a speed within one bisection step below the limit, the cap with the capped flag, and no speed, with the trials listed in the order they ran

#### Scenario: The worst window
- **WHEN** the shares of a series of frames are judged over windows of 1 s, with a spike shorter than the window, a spike of the window's length, and a series shorter than the window
- **THEN** the worst window's mean is the spike spread over the window, the spike's own value, and the mean of the whole series

#### Scenario: The flight and the log
- **WHEN** the cameras of a flight are built at an altitude and a speed from two start places
- **THEN** consecutive cameras are the speed times the frame step apart along the surface, each stands the altitude above the field, the two flights share no place, and the log text holds every altitude with its speed, share, trials and the bound

#### Scenario: The judged share
- **WHEN** the measure of a flight is made from per-frame coarse shares, with a missing value among them, and the log is written
- **THEN** the judged share is the worst window of the coarse share (NaN when a value is missing), and the log names the pixel weighting and holds no chunk share

#### Scenario: The scene on the software adapter
- **WHEN** the scene runs with `--smoke` over an in-memory bake
- **THEN** it ends without error, writes the log with four altitudes, and says it is not a measurement

### Requirement: TEST-054 The envelope scene runs the horizon jobs
The envelope scene (TEST-047) SHALL run the horizon jobs from the base terrain when its world has a store, and no chunk stage (ADR 0086: the stage is removed). The jobs are driven every frame from the plan's tiles and every flight starts with no horizon map resident. A world without a store SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 plan, TEST-047, STRM-064, ADR 0086

#### Scenario: The stage of a flight
- **WHEN** the envelope scene is set up with a store and with an in-memory bake
- **THEN** the first runs the horizon jobs and no chunk stage, and the second has none

#### Scenario: A frame waits for the horizon jobs
- **WHEN** a frame has a horizon job in flight, and when it has none
- **THEN** the frame is busy in the first case and not in the second, and the settle before a flight waits for a frame that is not busy

### Requirement: TEST-055 The envelope log gives what the horizon jobs did
The log of the envelope scene (TEST-047) SHALL say `"chunk_stage": false` (the stage is removed, ADR 0086; `accept M8` reads the field) and, for each trial, give the horizon maps landed, the horizon jobs in flight at the end and the counters of the horizon step (TEST-058).

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-054, planner decision 2026-10-26, ADR 0086

#### Scenario: The log
- **WHEN** the scene's log is written for a flight with the horizon jobs and for one without them
- **THEN** the first holds the per-trial facts and `"chunk_stage": false`, and the second says `stage: null`

### Requirement: TEST-058 Every frame loop runs the same horizon step and an enabled feature must act
The viewer and every perf scene that drives the horizon jobs SHALL run the per-frame step through one function (`horizonstep::frame_step`), and each scene SHALL report the counters of its enabled features (horizon maps baked and landed, plan-cache announcements). A scene SHALL fail, naming the feature and the counter table, when a feature it has enabled counted 0 where the scene expects it to act.

Verify: A, C
Status: active
Source: owner (2026-10-27: one frame loop for the viewer and the scenes), TEST-047, ADR 0086

Detail: A scene MAY turn the `horizon` feature off and says so in its log; an off feature is not checked. Which counters a scene expects is a table in this requirement's design: flights expect horizon bakes; the edit scenes expect plan-cache announcements; a settling loop that is not a frame runs the step with the horizon on or off by call.

Per-scene table (the counters each scene checks; a scene with the feature off checks nothing for it and logs `off`):

| Scene | Horizon step ran | Plan-cache announcements | Where logged |
|---|---|---|---|
| envelope flights (not smoke) | > 0 | not checked | `perf/logs/envelope.json`, per trial `stage.counters` |
| edit latency | > 0 | > 0 | report line per altitude |
| stills settling loop | on | not checked | not logged |
| viewer window (`editor --smoke`) | reported, not judged | not checked | the smoke summary, `horizon step:` |

#### Scenario: A feature that never acted
- **WHEN** a scene enabled the horizon step and its counters show 0 horizon frames, and again with the feature turned off
- **THEN** the first fails the check naming the horizon and the counter table, and the second is not checked and logs `off`

### Requirement: TEST-062 The gate runs the real window path
`cargo xtask gate` SHALL run the editor window path over `C:/DEV/SeriousPlanetData/global16384` (opened read-only, outputs under `target/gate/`), script one large edit (a 400 m hill), and replay the camera paths of the owner test (`frame-*-path.txt`) over the capture sites, reporting the counters of TEST-061 per site. `cargo xtask gate --small` SHALL run the same counters and verdict on a generated 32-cell world with its coupled run and coarse horizon on the software adapter. Neither SHALL run a perf measurement.

Verify: C
Status: active
Source: owner (2026-10-28), TEST-061

#### Scenario: The small world in CI
- **WHEN** `cargo xtask gate --small` runs on the software adapter
- **THEN** it prints the counter table and exits non-zero exactly when the verdict of TEST-061 fails
