## ADDED Requirements

### Requirement: TEST-058 Every frame loop runs the same chunk step and an enabled feature must act
The viewer and every perf scene that drives chunks SHALL run the per-frame chunk step through one function (`chunkstage::frame_step`), and each scene SHALL report the counters of its enabled features (prefetch asked, prefetch jobs started, horizon maps baked and landed, stale-hold frames, chunks landed and evicted, plan-cache announcements). A scene SHALL fail, naming the feature and the counter table, when a feature it has enabled counted 0 where the scene expects it to act.

Verify: A, C
Status: active
Source: owner (2026-10-27: one frame loop for the viewer and the scenes), STRM-042, STRM-056, TEST-047, TEST-056

Detail: A scene MAY turn a feature off (`prefetch`, `horizon`, `stale_hold`) and says so in its log; an off feature is not checked. Which counters a scene expects is a table in this requirement's design: flights expect prefetch and, with horizon on, horizon bakes; the edit scenes expect plan-cache announcements and stale-hold frames; a settling loop that is not a frame runs the step with the prefetch off.

Per-scene table (the counters each scene checks; a scene with the feature off checks nothing for it and logs `off`):

| Scene | Prefetch asked | Horizon step ran | Stale-hold frames | Plan-cache announcements | Where logged |
|---|---|---|---|---|---|
| envelope flights (not smoke) | > 0 | > 0 | not checked | not checked | `perf/logs/envelope.json`, per trial `stage.counters` |
| edit latency, with chunks | not checked (the camera does not travel) | > 0 | > 0 | > 0 | report line per altitude |
| stills and edit-resolve settling loops | off | off or on by call | off | not checked | not logged |
| viewer window | not checked | not checked | not checked | not checked | counted only |

#### Scenario: A step left out
- **WHEN** a frame loop over a travelling camera on the stand-in chunk stage runs the step, and again with the prefetch step left out
- **THEN** the first run counts prefetch, horizon bakes and announcements, and the second fails the check naming the prefetch
