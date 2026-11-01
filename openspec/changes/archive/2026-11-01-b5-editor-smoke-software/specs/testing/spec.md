## ADDED Requirements
### Requirement: TEST-085 The gate smokes the real store on the software adapter
`cargo xtask gate` on the real store SHALL also run `editor --smoke --edit --adapter software --store DIR` once and print its PASS or FAIL whether or not the desktop is locked. The run passes when it exits successfully and prints its `[smoke ok]` line. The store SHALL stay read only: the gate compares the store's files (names, sizes, modification times) before and after the run, and any addition, removal or change SHALL fail it, naming the file. The gate's exit SHALL be the worst of its parts: a failed software smoke exits 1, a passed one beside a locked desktop leaves the hardware part NOT RUN and exits 3 (TEST-073).

Verify: A
Status: active
Source: opus-planner (2026-11-01, SPIKE-84 fix 6), TEST-073, CON-19

#### Scenario: The verdict
- **WHEN** a smoke run exits successfully with a `[smoke ok]` line, then without the line, then with an error status
- **THEN** the first passes and the others fail, naming the last line of stderr

#### Scenario: The store is opened read only
- **WHEN** a run adds, resizes, touches or removes a file of the store, and when it changes none
- **THEN** the first is named by file and fails the gate, and an untouched store passes

#### Scenario: Locked desktop
- **WHEN** the desktop is locked and the software smoke passes
- **THEN** the hardware part is NOT RUN and the gate exits 3, and a failed software smoke exits 1
