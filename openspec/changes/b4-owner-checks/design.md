## Context
- Owner test 2026-10-27 findings (`docs/milestones/M8/owner-test-2026-10-27/README.md`) and the pass conditions of its "Decisions" table: level boundaries (step 15/14 and entry step under 0.25 m p99, no visible pop in the descent), rivers (no bed rise over 0.5 m downstream), chunk change (held-back tiles 0 at the capture sites without edits).
- Detectors that exist (from the specs; the code behind each not re-read for this note):

| Owner failure | Existing detector | Gap |
|---|---|---|
| Frames over budget, stalls | gate (TEST-061, TEST-062): frames over 100 ms, main-thread decodes, panics | none |
| Held-back tiles | gate counters at the capture sites (TEST-061) | none |
| Level step at a boundary | gate `levels` check at the entry step and 15/14 (TEST-061) | measures heights at sites; a moving descent is not scripted |
| Tile coarsening while the camera nears | TEST-067 through the managed loop with manual spawners | on a generated world, not the real store |
| Still-camera flicker | TEST-068, TEST-069, TEST-070 (level, relief, normals, lit, streamed lit) | not covered: the editor hover, wall-clock water (status Tracked) |
| Uphill rivers | GEN-048 (breach rule, chunk level) | no check of the bed rise along the routed path on the real store |
| Edit shows differently dragged than at rest | REND-033 preview bound (5 % of the pixels the edit changes); a measurement was queued to crusader (2026-10-29) | no automated pair test on the real store |
| Pop when a chunk or edit lands | none | needs a frame-pair detector at the swap |

## Decisions (proposed)
1. One scenario file format: the poses file of TEST-050 extended with a `[flight]` table (waypoints, speed, altitude) and `[[edit]]` entries (kind, centre, radius, at second); detectors read the frame sequence the scene already produces.
2. A detector is a pure function over a frame sequence or a numeric trace, with its own unit test on a synthetic bad input (it must fail on a planted pop, rise or flicker) and a threshold taken from an owner decision or an ADR.
3. Verdicts are numbers and image paths; the package lists a detector as PASS, FAIL or NOT RUN, never silently skipped.
4. Videos: short (5 to 10 s) mp4 through the existing `capture flights` ffmpeg path; if ffmpeg is absent the package carries PNG strips and says so.
5. Nothing in B4 runs inside a perf window; scenes are tier C and run on a quiet machine.

## Risks
- A pop detector on pixels needs a threshold that separates a level pop from honest motion; it is derived from two frames at the same eye position one tile-level apart, not from adjacent frames of the flight.
- The real store is read-only and large; scenes use `--store` and write only under `target/`.

## Open questions for the planner
- Delivery channel (step B4-6): file through Remote Control, a scheduled cloud task cannot reach the local store.
- Whether the owner's own detectors (his eye) have a list beyond owner tests 1 and 2.
