## Why
- Crusader's #124 diagnosis (2026-11-13): in a 25 min free flight over the R3a commit the viewer's resident tiles grew from 10,922 to 66,114 and never fell; the stalls (render submit 4 to 6.5 s, every 25 to 30 s) tracked that count.
- STRM-007 evicts only to make room, and the viewer's streamer is as large as the renderer's pool (205,170 slots on the mid tier), so a flight never has to evict.

## What changes
- VIEW-070: the viewer's streamer capacity is the smaller of the pool and a cap, 24,000 tiles (`--resident-cap N` overrides). The eviction order is STRM-007's; D with its ancestors and the roots stay.
- The cap is above R of the standard views (20,517 slots), so a settled view is the exact plan and the pixels do not change. A golden candidate from this change is a finding.

## Not in this change
- The perf scenes (they keep the pool as capacity), the pool sizes (STRM-006), the eviction order (STRM-007).
- Whether the stalls vanish at the cap: measured on the R3a path replay (G-RT), reported, and the cap value restated from it if needed.

## Impact
app (pipelined.rs, window.rs, lib.rs). Spec: viewer. Tests: frame managed (tier A), app unit.
