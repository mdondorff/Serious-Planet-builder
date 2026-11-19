## Why
- Lead #172 (step C, PR 1): G2 measures of plan section 4.1 the gate lacked: ground lowered over 50 m per stage, relief lost against B, the spectrum's bar, KC's patches, cost per window against B.

## What changes
- BUILD-060: `b_<stage>_over50`; `b_kc_patches`, `b_kc_patch_km3` from `kc-patches.csv`; `relief2_lost_m`, `relief10_lost_m`, `beta_change`; `bar_spectrum_steep`, `bar_spectrum_rough` (benchmark range -4.13 to -2.05).
- BUILD-060 `--table`: `#timing=` (`cost_s`, `cost_peak_mib`); bars against the window's store row (B): `bar_relief_lost`, `bar_beta_change`, `bar_cost`.

## Impact
cache (`river_gate_graph`, `river_gate`, the `river_gate` example), xtask (`rivergate.rs`, help); tests. The chain runner writes `kc-patches.csv` (spike branch).
