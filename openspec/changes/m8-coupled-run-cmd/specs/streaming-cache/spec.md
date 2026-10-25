## ADDED Requirements

### Requirement: STRM-051 The coupled run is made by an explicit command with progress
`planet generate --coupled-run <store dir> --global-res N [--world F | --seed S]` SHALL make the coupled run of the bake stored in the directory (the viewer's `load_or_run`: the stored channel is read when it is there, else the run is made and stored) and print the stages and the wall time. The reduction of the bake onto the run's lattice SHALL report the bands it has done (`reduce_bake_with`, called once per band with the count done and the total, from any thread); the run SHALL print a line at most every 10 s with the share done, the seconds so far and the projected total. The reduced heights SHALL NOT depend on the reporting or the thread count (CON-14).

Verify: A
Status: active
Source: opus-planner (2026-10-25, M8-R rebuild)

#### Scenario: Progress
- **WHEN** a bake is reduced with a progress callback on 1 and on 4 threads
- **THEN** the callback is called once per band with counts that reach the total, and the heights equal those of `reduce_bake`
