## Why
D0 failed its stop rule and the owner's fallback of 2026-10-28 applies (PR #848): the chunk stage is off for M8. The gate (TEST-061 to TEST-064) shows 0 held-back tiles and no chunk-frame stalls with it off.

## What Changes
1. The viewer and the editor start without the chunk stage; `--chunks` turns it on (`PLANET_SMOKE_CHUNKS` still asks for it in a smoke run). The stage's code and tests stay; nothing is deleted (M9 cleans up).
2. `cargo xtask gate` runs stage-off by default; `gate --chunks` is the stage-on run and keeps its KNOWN ceilings. `gate --small` runs both small-world cases.

## Impact
- Spec: STRM-046 (modified), TEST-064 (modified). Code: `app/window.rs`, `app/lib.rs`, `xtask/gate.rs`. Tests: `tests/gpu_window.rs`.
