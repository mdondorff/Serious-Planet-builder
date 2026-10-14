# 0032 Streaming test definitions: dive, teleport, mouse look
Status: proposed (sketch by opus-planner, 2026-10-13; sonnet-explorer completes it, M4 item 6. Adding the teleport and mouse-look cases to M4's exit criteria is the owner's decision)
Date: 2026-10-13
Source: architecture report M4 row ("dive test: no blank tiles, p99 frame ≤ 20 ms, VRAM within tier budget, RAM ≤ 8 GB") and risk table ("orbit to ground in < 30 s", "p99 tile latency ≤ 200 ms"); owner's remarks 2026-10-13; STRM-002, TEST-013; constitution CON-17, CON-20, CON-21, ADR 0020

## Context
M4's exit test names a dive but not its profile. A constant speed is meaningless across seven decades of altitude: fast at orbit is trivial, the same speed at 10 m is impossible. The owner also named two cases that velocity prefetch cannot help: a teleport ("portal") and erratic mouse look. "No blank tiles" alone does not catch a view that stays blurry for seconds.

## Decision (proposed)
1. **Dive profile:** altitude falls by a constant factor per unit time (equal time per decade), from 20,000 km to 1 m above the terrain in 30 s (about 4.1 s per decade), then 10 s of ground travel at 1 m altitude at walking-to-vehicle speed (to be fixed: 1.5 m/s and 30 m/s variants), deterministic and scripted like TEST-013.
2. **Teleport:** at a resident position at 2 km altitude, jump to an antipodal position at 1 m and 2 km altitude, then hold still for 10 s.
3. **Mouse look:** at a fixed position (1 m and 2 km), a scripted deterministic yaw/pitch sequence with reversals and a 180° turn within 0.2 s, 10 s long.
4. **Measures, per case:**
   - **Blank tiles: none, ever** (hard): every visible node has a resident ancestor in every frame (STRM-002), checked at tier A with fake I/O and on the real machine.
   - **Frame time:** p99 ≤ 20 ms, median ≤ 16.6 ms (CON-21), on Rasierklinge in a perf window (ADR 0020).
   - **Detail lag:** per frame, the screen-weighted level deficit (target level minus drawn level, weighted by screen area); budgets: dive mean ≤ 1 level and no frame over 3; teleport back under 1 level within 2 s (to be fixed from SPIKE-05 and measurement); mouse look 0 after the residency set is complete (ADR 0031), else the outside-in degradation of ADR 0031 point 4 only.
   - **Memory:** VRAM within terrain's share (ADR 0033) and the tier cap; RAM ≤ 8 GB (CON-21); upload ≤ 2 ms per frame.
5. Count-based measures (blank tiles, detail lag with a fake clock, resident counts, upload bytes) are tier A on any adapter (CON-20); time-based measures only in perf windows.

## Consequences
- New perf scenes `dive`, `teleport`, `look` in `cargo xtask perf`; tier-A twins with fake I/O at 200 ms latency (STRM-002's scenario).
- If the owner adds teleport and mouse look to the exit criteria, the M4 report must show them; otherwise they are tracked measures.
- Open for the completion: the detail-lag budgets' numbers (after SPIKE-05 and a first measurement), ground-travel speeds, whether the 1440p internal resolution (CON-21) or the 1080p LOD parameters apply to the counts.

## Licences of adopted code or techniques
None.
