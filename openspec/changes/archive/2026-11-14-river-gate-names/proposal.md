## Why
- Window names collided between lanes: SPIKE-114/115 and the river gate called face 1, 8056-8675 x 760-1379, seed 14 "R3a"; the spikes lane calls it R1h and uses R3a for face 4, seed 15 (explorer, #1505).
- Lead, 2026-11-14 (explorer #146): canonical names R1h, R3a, R3b; the river gate gets them as aliases.

## What changes
- BUILD-055: `cargo xtask river-gate` takes `r1h`, `r3a` and `r3b` (any case) as the target or as `--window`; `r3a` is now the face 4 window; R1h is the default window of a store run.

| Name | Window (regional cells) | Seed |
|---|---|---|
| R1h | face 1, 8056-8675 x 760-1379 | 14 |
| R3a | face 4, 368-987 x 4720-5339 | 15 |
| R3b | face 2, 9968-10587 x 8304-8923 | 15 |

## Impact
xtask (rivergate.rs), the river_gate example's default constant renamed. The measures are unchanged.
