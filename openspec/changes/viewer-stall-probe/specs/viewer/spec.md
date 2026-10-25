## ADDED Requirements

### Requirement: VIEW-056 A stall names its culprit
The window's frame SHALL time its phases (`stall::StallProbe::mark` after each) and the locks the main thread waits on (`stall::read`, `stall::write` and `stall::lock` record a wait of 2 ms or more in a thread-local of the thread that began the frame). A frame that takes at least one second SHALL be a stall: the report `stall (the window went on)` SHALL be written with the crash report's means (at most one in ten seconds, the others one log line) and SHALL list the phases longest first, the locks waited on longest first, the tile streamer's jobs in flight and queued and the chunk and horizon job counts, and the title SHALL carry a notice with the frame time and the longest phase. The probe SHALL read only the clock. Every thread the program starts for the frame, the streaming pool and the scoped workers of the generators SHALL have a name.

Verify: A
Status: active
Source: opus-planner (2026-10-25), owner: the editor stalls and Windows reports it not responding, one unnamed thread busy; VIEW-037

#### Scenario: A frame over a second
- **WHEN** a frame has phases of 20, 900 and 300 ms and took 1.3 s, and another took 0.4 s
- **THEN** the first is a stall listing the phases longest first and the second is none, and a report is due at most once in ten seconds

#### Scenario: A lock waited on
- **WHEN** another thread holds a mutex for 40 ms while the main thread acquires it through the timed acquire, and an uncontended acquire is made too
- **THEN** only the held lock is recorded, with its name and a wait of at least 20 ms, and the next frame starts with no waits

#### Scenario: Named workers
- **WHEN** a scoped worker is started with `spawn_named`
- **THEN** its thread has the name it was given
