## MODIFIED Requirements

### Requirement: TEST-048 Pose stills
`test-render --scene capture-stills --poses FILE --out-dir DIR` (a world source as in TEST-027; `--size WxH` default 2560x1440; `--views A,B` selects poses by name) SHALL draw each pose on the lit path over the base terrain of the world, with the horizon maps of the chunks near the eye baked first (REND-038), and write `DIR/<pose>.png` and `DIR/stills.txt` (pose, size, eye latitude, longitude and height, heading, pitch, sun hours, horizon maps waited for, seconds waited). A bad file or call SHALL be refused before it draws. The tool is not a measurement: no gate, no timestamp query, no perf log. The eye SHALL be `altitude_m` over the ground of the world's field.

Verify: A, C
Status: active
Source: opus-planner (2026-10-24), M8-4 and M8-8 look reviews, TEST-027, ADR 0086

#### Scenario: Two poses
- **WHEN** the scene runs on the software adapter over a small store with two poses on the ground
- **THEN** it writes one PNG of the requested size per pose and `stills.txt` with at least one horizon map per pose, and the two images differ in their sun

#### Scenario: A bad call
- **WHEN** `--poses` is missing, `--views` names no pose, or `--poses` is given to another scene
- **THEN** the run is refused before it draws

#### Scenario: Maps that do not bake
- **WHEN** the horizon maps of a pose are not baked within `chunk_timeout_s`
- **THEN** the run fails naming the pose, the number of chunks without a map and the first of them

### Requirement: TEST-050 Poses file
The poses file (TOML) SHALL hold the keys of the table; a bad value SHALL be refused naming the key and the value.

| Key | Meaning | Default |
|---|---|---|
| `[place]` `anchor = "largest-river"` | largest river, at least `min_height_m` high (0) | or `lat`, `lon` |
| `chunk_timeout_s` | wait per pose, seconds | 900 |
| `horizon_cell_m` | horizon chunk cell, metres | 8 |
| `[[pose]]` `name` | names the PNG | required |
| `altitude_m`, `back_m` | eye over the ground; behind the anchor | required |
| `heading_deg`, `pitch_deg` | clockwise from north; down | required |
| `sun_local_hours` | local solar time | required |
| `sun_declination_deg` | degrees | 0 |

Verify: A
Status: active
Source: TEST-048

#### Scenario: A bad value
- **WHEN** `heading_deg` is a string, or `pitch_deg` is missing, or a name is given twice
- **THEN** the file is refused naming the key and the value

### Requirement: TEST-052 Anchor on the steepest river
`[place] anchor = "steepest-river"` (`slope_radius_m`; optional `ground_min_m`, `ground_max_m`, `min_area_m2`) SHALL anchor on the coupled run's river cell of at least that drainage area, with ground on the base field in the range, whose surroundings within the radius have the steepest slope; it prints the slope. `slope_table_stride` SHALL write `slopes.txt`: per ground band, the land share and the slope percentiles of land and river cells.

Verify: A
Status: active
Source: opus-planner (2026-10-25), look review 1 second site, TEST-050, ADR 0086

#### Scenario: The slope table
- **WHEN** the northern half of the ground is a plateau of 600 m and the southern half is sea
- **THEN** all the land is in the band of 600 m with no slope, and the same table comes out every time

### Requirement: TEST-054 The envelope scene runs the horizon jobs
The envelope scene (TEST-047) SHALL run the horizon jobs from the base terrain when its world has a store, and no chunk stage (ADR 0086: the stage is removed). The jobs are driven every frame from the plan's tiles and every flight starts with no horizon map resident. A world without a store SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 plan, TEST-047, STRM-064, ADR 0086

#### Scenario: The stage of a flight
- **WHEN** the envelope scene is set up with a store and with an in-memory bake
- **THEN** the first runs the horizon jobs and no chunk stage, and the second has none

#### Scenario: A frame waits for the horizon jobs
- **WHEN** a frame has a plan node that stands in for a chunk or a horizon job in flight, and when it has neither
- **THEN** the frame is busy in the first two cases and not in the last, and the settle before a flight waits for a frame that is not busy

### Requirement: TEST-055 The envelope log gives what the horizon jobs did
The log of the envelope scene (TEST-047) SHALL say `"chunk_stage": false` (the stage is removed, ADR 0086; `accept M8` reads the field) and, for each trial, give the horizon maps landed, the horizon jobs in flight at the end and the counters of the horizon step (TEST-058). A log of a world without the horizon jobs SHALL say why the stand-in share is missing.

Verify: A
Status: active
Source: M8 exit criterion 7 (owner, 2026-10-23, D4), TEST-054, planner decision 2026-10-26, ADR 0086

#### Scenario: The log
- **WHEN** the scene's log is written for a flight with the horizon jobs and for one without them
- **THEN** the first holds the per-trial facts and `"chunk_stage": false`, and the second says `stage: null` and the reason

### Requirement: TEST-057 The envelope scene flies a steady curve beside the cold one
With the horizon jobs the envelope scene (TEST-047) SHALL search each altitude twice: the cold trial of TEST-054 (the first visit after a stop) and a steady-flight trial that flies a run-in at the speed under test before its measured stretch, with the horizon jobs running. The stretch SHALL be `--steady-window-s` long (60 s by default) over ground the run-in did not cover, and the bound SHALL be judged over every 1 s window of the whole stretch. The log SHALL give both curves, the run-in and the stretch.

Verify: A, C
Status: active
Source: opus-planner (2026-10-27, m8-chunk-lead: the cold trial measures the start-up), owner (2026-10-27, #790: the floor applies to steady flight), TEST-047, TEST-054, ADR 0086

Detail: The run-in is 12 s and is not judged. A stretch of 12 s is the quick trial. The steady curve is not part of the smoke run beyond one short trial per altitude. The owner decides the floor applies to the steady curve.

#### Scenario: Steady and cold
- **WHEN** the scene runs with the horizon jobs
- **THEN** the log holds the cold curve and the steady curve with its run-in and stretch

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
| viewer window (`editor --smoke`) | > 0 on the lit frames (the smoke verdict) | not checked | the smoke summary, `horizon step:` |

#### Scenario: A feature that never acted
- **WHEN** a scene enabled the horizon step and its counters show 0 horizon frames, and again with the feature turned off
- **THEN** the first fails the check naming the horizon and the counter table, and the second is not checked and logs `off`

## REMOVED Requirements

### Requirement: TEST-049 Stills wait for their chunks
