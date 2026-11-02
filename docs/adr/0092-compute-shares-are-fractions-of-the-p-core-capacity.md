# 0092 Compute shares are fractions of the P-core capacity; background work is budgeted, not pooled on E-cores
Status: proposed (drafted by opus-planner, 2026-11-02; for the owner)
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

## Open point for the owner
| Terrain fraction | Rasierklinge (P 12, B 10) | 8-core desktop (P 16, B 14) | Measured on Rasierklinge |
|---|---|---|---|
| 0.6 | 6 | 8 | below the 8-10 knee; teleport-1m coverage not measured at 6 |
| 0.8 | 8 | 11 | teleport-1m 83 % of tiles |
| 0.8 with max 10 | 8 | 10 | as above |
A window measures the chosen value on Rasierklinge (tile-ready, coverage, frame p99) before BUDG-003 is set active.

## Consequences
- BUDG-003, BUDG-011, BUDG-012, STRM-020, STRM-064, TEST-039/040 change (SPIKE-89 section 4); about 4 PRs.
- Rasierklinge's terrain ceiling falls from 12 to 6-8; streaming may land tiles later. The window above decides whether that is acceptable.
- ADR 0083 point 3's "stated fraction of the cores" becomes a fraction of the P-core capacity.

## Licences of adopted code or techniques
None.
