## Why
- Owner decisions 1 and 3 (2026-11-17), lead #168: G2's blockers in the gate. The bars are pass or fail per row, and the budget is read against `S_KH` (reading 21) with KH against `S0` in its own field.

## What changes
- BUILD-060: the G2 bars (`bar_corridor` >= 97.5 %, 95 % on M1; `bar_uphill` <= 1.0 %; `bar_agree_<A>km2` >= 90 %; `bar_violations` = 0; `bar_spill_missing` = 0) as columns with classes and a line.
- BUILD-060: `--reference` / `#reference=` (`S_KH`) for the budget field; stage `kh` read; budget use against `S_KH` and, kh added, against `S0`; `--corridor-bar`.

## Impact
cache (`river_gate_graph`, `river_gate`, the `river_gate` example), xtask (`rivergate.rs`, help); tests.
