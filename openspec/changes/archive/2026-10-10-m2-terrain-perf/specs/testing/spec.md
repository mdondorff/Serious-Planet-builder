## ADDED Requirements

### Requirement: TEST-012 Terrain performance workload
The performance harness SHALL provide a terrain frame workload at 1440p over scripted views from orbit to 1 m that generates and uploads meshes once, then reports per run the GPU time (timestamp queries), CPU planning time, CPU record time, wall time and the deterministic counts (nodes, draw calls, triangles, resident vertex bytes), checks the terrain budget (median GPU time at most 8 ms, p99 frame time at most 20 ms), refuses unready machines like every performance run, and offers a labelled smoke mode that exercises the path on any adapter without being a measurement.

Verify: C
Status: active
Source: CON-20, CON-21

#### Scenario: Refusal
- **WHEN** the workload is started on a software adapter with the machine-ready flag
- **THEN** it refuses with the adapter and the corrective action

#### Scenario: Smoke
- **WHEN** the smoke mode runs on the software adapter
- **THEN** it prints that it is not a measurement, completes every view and writes no log unless asked
