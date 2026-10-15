# 0044 M4 implementation readings of ADR 0031 (cancellation) and ADR 0032 (dive pitch)
Status: accepted (opus-planner, 2026-10-15, under the owner's delegation of 2026-10-12; amends ADR 0031 point 3 and ADR 0032 point 1 in part; neither touches M4's exit criteria)
Date: 2026-10-15
Source: sonnet-explorer's stop report and questions of 2026-10-15 (#149, #150); spec-guardian findings on #150; ADR 0031 point 3, ADR 0032 point 1, TEST-013

## Context
Two places where the M4 implementation reads an accepted ADR differently from its literal text; spec-guardian flagged both.
1. ADR 0032 point 1 says the dive camera "looks along the direction of travel with a fixed pitch (no look input)". A constant pitch cannot frame the planet at 20,000 km and the ground at 1 m; the dive shares `dive_camera(t)` with TEST-013, whose flight law tilts the pitch from 85 to 8 degrees with altitude.
2. ADR 0031 point 3 says "an in-flight job is cancelled at its next stage boundary; a tile that completes anyway goes to the persistent cache (nothing is wasted)". #150 polls the cancel token on every height sample of `MeshSource::load_staged`, so a cancelled tile is discarded unbuilt instead of finishing its stage; before it, teardown waited 9-14 s for uninterruptible stages over the 16384 store, 0.07 s after.

## Options (with trade-offs)
1. Keep the literal texts: a fixed-pitch dive that misses the planet or the ground; a teardown and teleport cancellation that waits seconds for a stage to finish.
2. **Record the implementation readings** (chosen): the intent of each point holds (no look input; no wasted completed work), the letter changes.

## Decision
1. **ADR 0032 point 1:** the dive's pitch follows TEST-013's flight law (85 degrees at 20,000 km to 8 degrees at the ground) through the shared `dive_camera(t)`; there is still no look input, the profile is a pure function of time, and the descent's detail-lag gate is unchanged.
2. **ADR 0031 point 3:** an in-flight job polls cancellation at fine-grained points (at least per height sample), and a tile cancelled before it is built is discarded; a tile that completes before the cancel is seen still goes to the persistent cache and is not uploaded. "Nothing is wasted" applies to completed tiles only.

## Consequences
- The dive perf scene, TEST-018 and TEST-019 stay as implemented in #148; teardown and teleport cancellation as in #150.
- Partially generated tiles are recomputed if requested again; at the measured fill rates this costs less than waiting for a whole stage.

## Licences of adopted code or techniques
None.
