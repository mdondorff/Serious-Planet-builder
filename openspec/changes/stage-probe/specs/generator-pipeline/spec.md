## ADDED Requirements
### Requirement: GEN-161 The stage hooks only read
The stages of the new chain SHALL call the stage probe's `emit` at their boundaries with a closure that only borrows the stage's output: each round of the round solvers, and thermal, deposition and pit spreading of the sediment pass here; the landmass and H1 boundaries by the build that owns them, K7 after its own change. With no sink installed `emit` SHALL be one atomic load: the closure does not run and nothing is allocated. With a sink installed the hooks SHALL only read: the rounds and the sediment pass SHALL give the same output bits and reports with the probe on and off.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100, owner decision: stage previews, briefing item 18), CON-14

#### Scenario: On and off
- **WHEN** the rounds and the sediment pass run on a compact landmass without a sink and with a sheet probe installed
- **THEN** every height bit, the round report and the sediment report are equal, and with the probe installed each stage has written its fields
