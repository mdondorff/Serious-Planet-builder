## Why
- Crusader's #124 diagnosis (2026-11-13): in a 25 min free flight over the R3a commit the viewer's resident tiles grew from 10,922 to 66,114 and never fell; the stalls (render submit 4 to 6.5 s, every 25 to 30 s) tracked that count.
- STRM-007 evicts only to make room, and the viewer's streamer is as large as the renderer's pool (205,170 slots on the mid tier), so a flight never has to evict.

## What changes
- VIEW-070: the viewer's streamer capacity is the smaller of the pool and a cap, 24,000 tiles (`--resident-cap N` overrides). The eviction order is STRM-007's; D with its ancestors and the roots stay.
- The cap is above R of the standard views (20,517 slots), so a settled view is the exact plan and the pixels do not change. A golden candidate from this change is a finding.

## Not in this change
- The perf scenes (they keep the pool as capacity), the pool sizes (STRM-006), the eviction order (STRM-007).
- Whether the stalls vanish at the cap. G-RT (2026-11-13, the RTX, `cargo xtask repro frame-17426-path.txt --window --real` over `r3a-e1`, BUILD-054): the 60 s path alone shows 0 stalls with the pool as capacity (14,290 frames, resident at most 17,125, none evicted) and with the default cap (12,812 frames, resident at most 16,483, none evicted). The recorded minute starts after the residency had grown, so it cannot show the effect; a long free flight (25 min, as in the diagnosis) can. The cap value stays 24,000.
- Mechanism check in the real window (same path, `--resident-cap 10000`): resident at most 10,000, 5,178 tiles evicted, 0 stalls, longest frame 101 ms. It found a crash the cap exposes: the clipmap shadow pass wrote its skipped floor tiles again alone and asserted that they cover the cube ("the floor tiles must cover every face of the cube", area 2,048 of 1,572,864), which any eviction that delays a floor tile triggers. Fixed here (`floor_job`, `rebuild_part`): a floor set that arrived is the whole cube, skipped tiles alone are a part.

## Impact
app (pipelined.rs, window.rs, lib.rs). Spec: viewer. Tests: frame managed (tier A), app unit.
