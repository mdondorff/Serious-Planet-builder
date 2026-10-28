## Why
D0 failed its stop rule (opus-planner, 2026-10-28); the owner decides whether the chunk stage goes off for M8. The gate's baseline (TEST-061 to TEST-063) has the stage on; the comparison needs the same sites, edit and counters with it off, and the between-levels step of the base field alone.

## What Changes
1. `editor --no-chunks`: the window does not request the chunk stage (the flag of the edit-latency scene, now honoured by the window).
2. `cargo xtask gate --no-chunks`: the same run per site with the stage off, and the between-levels step of the base field alone (`base_steps_report`, an ignored measurement) instead of the chunk change's.

## Impact
- Spec: TEST-064 (new). Code: `app/window.rs`, `app/globalperf.rs`, `xtask/gate.rs`.
