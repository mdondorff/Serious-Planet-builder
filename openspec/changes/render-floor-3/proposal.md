## Why
ADR 0062 point 3: levels 0 to 2 are not drawn. The 3x3 base surface is wrong there (0.46 px at orbit; SPIKE-12 Q1) and the alternative, a 5x5 base, costs +10 to +48 % vertex-stage time on every tile. A render floor at level 3 changes nothing in nine of the ten standard views and adds 129 nodes at orbit (52 level-2 nodes become 181 level-3 nodes, +0.147 ms indicative). SPIKE-12 Q5 step 3.

## What Changes
- frame: `LodParams::min_level` (default 3, `--floor-level N`, repro bundle `min_level`): selection splits every node below the floor; floor leaves are roots: no drawn parent, morph 0, interval `[1e30, 1e30]`, exempt from the plan's empty-interval check (LOD-011, LOD-004).
- streaming: the residency set, eviction rule, start-up load and cover treat floor tiles as the roots (384 at floor 3); R stops at the floor (STRM-001, STRM-007, STRM-015, STRM-016); the pre-fill covers levels 3..=4 (1,920 tiles, was 2,046 of levels 0..=4) and the pack drops the vertex-form records (STRM-021); a cold orbit start has no coarse stand-in (the pre-fill covers it).
- app: `--floor-level`, the meaning of `--prefill-levels`; the managed-loop tests that assume level-0 roots take the floor as a parameter.
- tests: orbit-20000km plans (181 level-3 nodes), floor 0 reproduces the old plans, floor leaves do not morph, cold start; only the orbit goldens change, by declaration, and go to the owner's `/bless` (ADR 0062).

## Impact
Spec: LOD-011 added; LOD-004, STRM-001, STRM-007, STRM-015, STRM-016, STRM-021 modified. STRM-021's delta is written on top of `compact-slot-v2`'s (payload version 3): archive that change first, or merge the two deltas. Not in this change: slot layout, pool arithmetic, removing the vertex pool (its 256 manager slots for levels 0-2 become unused and go with `remove-vertex-render-path`), the testing spec's pre-fill wording (audit task 4.2). SPIKE-12 wrote 2,040 tiles for levels 3..=4; 6 x (64 + 256) = 1,920 is used here.
