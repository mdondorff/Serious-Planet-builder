# 0092 Compute shares are fractions of the P-core capacity; background work is budgeted, not pooled on E-cores
Status: accepted (owner, 2026-11-02: a conservative terrain share; flight, not teleport, is the performance goal; no machine counts as rules; drafted by opus-planner)
Date: 2026-11-02
Source: owner, 2026-11-01 (shares divide P-core capacity only; E-cores only as a background pool that never slows critical work; BUDG-003 generalised beyond the reference machine); SPIKE-89 (`spikes/relative-shares/results/results.md`); SPIKE-90 (`spikes/dual-pool/results/results.md`); amends ADR 0083 point 3

## Context
- BUDG-003 and the code fix the reference machine's counts: terrain 12 of 20 threads, `TERRAIN_CORES = 12`, horizon bakes at most 8; the job system detects only the logical CPU count (SPIKE-89).
- SPIKE-90, two windows: a saturated background pool slows the critical path (dive frame p99 +43 to +58 %) with every tested knob (2 threads, EcoQoS, 50 % duty); the cause is shared power, cache and memory, not core placement.
- Terrain throughput saturates at 8 to 10 workers on Rasierklinge; 8 workers leave teleport-1m at 83 % of its tiles (2026-11-01).
- E-core terrain throughput is 0.63 to 0.71 of a P-core thread; the owner does not count it.

## Decision
1. **Capacity is the P-core threads.** `C = P` (logical threads on performance cores, SMT siblings counted); `B = C - 2` for the shares (the main and render threads stay outside). Machines without core classes count every thread as P.
2. **A share is a fraction of `B`** with a minimum and a maximum, summed shares fitted to `B` by BUDG-004's priority order (the lowest class gives up first). The fractions, minimums and maximums live in BUDG-003.
3. **No background pool on E-cores.** Idle-class work (bakes) runs inside the shares at idle priority and only while no higher class is queued; E-cores are left to the OS scheduler, and no thread is pinned.
4. **Detection without new crates:** `GetLogicalProcessorInformationEx` on Windows (a feature of the existing `windows` dependency), sysfs on Linux, `available_parallelism` as the fallback; `PLANET_CPU_TOPOLOGY=P,E` overrides for tests.
5. **Absolute caps derive from the shares** (horizon bakes `terrain / 2`, the coupled run `B`); reference-machine calibrations (TEST-039/040) are named as such and give no verdict elsewhere.

## Terrain share (owner, 2026-11-02)
- The terrain share is conservative: the terrain must later run inside a game engine beside physics, game logic, sky and networking, each keeping at least its minimum.
- The performance goal is flight (dive, glide, look scenes): tile-ready and frame budgets hold at the terrain share. Teleport speed is a nice-to-have, recorded, not a verdict.
- The fractions, minimums and maximums live in BUDG-003 only; thread counts per machine appear only as worked examples, never as rules (owner, 2026-11-02).
- Worked example (not a rule): a terrain fraction of 0.6 gives 6 threads on Rasierklinge (12 P threads) and 8 on an 8-core desktop.
- The next perf window confirms the flight scenes at the spec's fraction; a miss comes back to the owner.

## Consequences
- BUDG-003, BUDG-011, BUDG-012, STRM-020, STRM-064, TEST-039/040 change (SPIKE-89 section 4); about 4 PRs.
- The reference machine's terrain ceiling falls below today's fixed 12 (6 in the worked example); teleports land tiles later (accepted); flights are checked in the next window.
- Spec work: BUDG-003 states the formula and the fraction table; scenarios use synthetic topologies and name results as examples.
- ADR 0083 point 3's "stated fraction of the cores" becomes a fraction of the P-core capacity.

## Licences of adopted code or techniques
None.
