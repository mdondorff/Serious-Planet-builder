## MODIFIED Requirements
### Requirement: BUDG-004 One job system with priority classes
All background work SHALL run on one job system with a core budget per subsystem (BUDG-003) and the priority classes frame-critical, visible streaming and edits, prefetch, and idle (bakes), in that order. No subsystem SHALL create threads outside it, including the scoped workers of a job.

Verify: A
Status: planned
Source: ADR 0083 point 2; opus-planner (2026-10-31, the order of the moves: tile pool, chunk pool, plan workers, horizon bakes)

#### Scenario: A thread outside
- **WHEN** the source of the crates is scanned for thread creation (BUDG-010)
- **THEN** every start is on the list with its count and reason, and each pool that moves onto the job system leaves the list in the change that moves it
- **AND** the requirement turns `active` when only the drivers and tools that stay are left on the list

## ADDED Requirements
### Requirement: BUDG-008 A share's ceiling and the class order
The job system SHALL run at most a share's ceiling of that share's jobs at once, however many workers are free, and SHALL start the jobs of the best class first (frame-critical, visible, prefetch, idle), in submission order within a class and share; a running job SHALL never be preempted. A share at its ceiling SHALL not hold back another share's jobs, and a closing system SHALL keep the ceilings while it drains. A share MAY cap its idle-class jobs at once (the tile pool of the horizon: STRM-060), so that a bake never holds a worker a visible job of the share needs. A job system MAY have workers of its own for frame-critical jobs (at normal priority, beside tile workers below normal, ADR 0046): the other workers SHALL then never take that class.

Verify: A
Status: active
Source: ADR 0083 point 2, BUDG-003, BUDG-004

#### Scenario: A ceiling with free workers
- **WHEN** five jobs of a share with ceiling 2 are queued on six workers and none ends
- **THEN** two run, the other three wait, and the share's peak is 2 for the whole run

#### Scenario: An idle cap
- **WHEN** a share with ceiling 4 and an idle cap of 1 has three idle jobs and then three visible jobs queued
- **THEN** one idle job runs, the other two wait, and the three visible jobs run beside it

#### Scenario: Frame-critical workers
- **WHEN** both ordinary workers of a system with one frame-critical worker are held by visible jobs, and a frame-critical job and a visible job are queued
- **THEN** the frame-critical job runs at once on the frame worker, and the visible job waits for an ordinary worker

#### Scenario: A better class passes a worse one
- **WHEN** jobs of the classes idle, prefetch, visible, idle, frame-critical and visible are queued behind a running job of one worker
- **THEN** they start in the order frame-critical, visible, visible, prefetch, idle, idle

### Requirement: BUDG-009 Pool API and fan-out on the caller's share
A subsystem SHALL spawn background work on its `Share` (a job with a class, or a `Spawner`) and SHALL fan a computation out over it with `fan_out`, which returns the results in index order whatever the number of workers and the ceiling (CON-14). The calling thread SHALL work on the indices too, so a fan-out makes progress when every worker is busy; helpers SHALL stay inside the share's ceiling; a panic of the work SHALL be raised in the caller after every index has finished and SHALL not end a worker.

Verify: A
Status: active
Source: ADR 0083 point 2, CON-14, opus-planner (2026-10-31: the plan job's fan-out counts against terrain's share at the highest priority within it)

#### Scenario: Order and progress
- **WHEN** 200 indices are fanned out over shares of 1 to 8 workers and ceilings of 1 to 8, and again with the only worker held by another job
- **THEN** the results are identical and in index order each time, and the held case completes on the calling thread

### Requirement: BUDG-010 The thread-creation scan
A tier-A test SHALL scan the production code of `crates/*/src` and `xtask/src` for calls that start a thread (`thread::spawn`, `thread::Builder::new`, `thread::scope`, `spawn_named`, `spawn_scoped`) and SHALL fail on a call that is not on the allow-list, on a count that differs from the list, and on a listed file that starts no thread any more. Each entry SHALL carry a reason: the job system itself, a driver or tool that stays, or the PR of the change `job-system` that moves it.

Verify: A
Status: active
Source: ADR 0083 point 2, BUDG-004

#### Scenario: A new thread
- **WHEN** a file starts a thread that the list does not name, or a moved pool leaves its entry in place
- **THEN** the test fails and names the file, the count and what to do
