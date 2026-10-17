## ADDED Requirements

### Requirement: TEST-028 CPU settle between the children of perf m5
`cargo xtask perf m5` SHALL, after every child except the last, poll the system CPU load (the mean over one readiness-gate window, ADR 0020) until a reading is below the gate's limit, at most 30 polls. It SHALL print one line per wait: the child, the last reading, the limit, the polls, `settled` or `NOT settled`. An unreadable load ends the wait. The next gate decides.

Verify: A
Status: active
Source: window 6 (three gate refusals after heavy children), ADR 0020, TEST-019

#### Scenario: Settling between children
- **WHEN** a session of three children runs with a settle between children
- **THEN** the settle is called after the first and the second child and not after the third, polls until a reading is below 15 %, stops at the cap or at an unreadable load, and its line says which

### Requirement: TEST-029 Held clock before the lit views
The warm-load phase of the lit views and flights (TEST-025; it replaces the stop condition of the one-reading warm-up of TEST-012 for them, the terrain scene and the streaming scenes keep it) SHALL render chunks of at least 1 s and 60 frames and end after 3 consecutive readings of the graphics clock at the floor of ADR 0036, a reading below it restarting the count, capped at 30 s. The outcome SHALL name the readings held.

Verify: A
Status: active
Source: window 6 (coast-2km, coast-3m and the flights GPU-invalid), ADR 0034, ADR 0036, TEST-012, TEST-025

#### Scenario: Held clock
- **WHEN** the readings are 1702, 1327, 1450, 1420, 1410 MHz
- **THEN** the warm-up with 3 readings ends after the fifth, and with readings that alternate above and below the floor it ends at the cap, not reached

### Requirement: TEST-030 Log path of a single scene
`--out` of a single scene (`perf stream <scene>`, `perf lit`, `perf lit-formats`) SHALL take a directory (existing, or a path without an extension) and write `<scene>[-lit].json` inside it, or a path ending in `.json` as the file. Any other value SHALL be refused by the option check before the case runs, naming the file name the directory would get.

Verify: A
Status: active
Source: window 6 (single-scene `--out DIR`), TEST-019, TEST-025

#### Scenario: Out path
- **WHEN** `--out` is an existing directory, a new path without extension, a `.json` path, or an existing `.txt` file, for a streaming scene and for a lit scene
- **THEN** the log path is `<dir>/<scene>-lit.json`, the same under the new directory, the file itself, and a refusal by the option check before the case
