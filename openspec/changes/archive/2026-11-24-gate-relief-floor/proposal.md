## Why
- Owner decision, 2026-11-23 (lead #191): `bar_relief_lost` is relaxed for the look and capped at 2 m.
- Before, a candidate could lose no more relief at 10 km than B (`max(B's, 0)`). Small losses on flat windows then failed a bar the owner does not want to hold that tightly.

## What changes
- BUILD-060: `bar_relief_lost` is `relief10_lost_m <= max(B's, min(0.03 x relief index, 2 m))`, for B and RG alike.
  - The relief index is the window's shared S0's (`relief_index_m`, #1648), else B's `relief10_start_m`.
- No other bar moves.
- Lead #192 (the gate lives on main only): the spike's notch columns (lead #183) join BUILD-060 as columns, with no bar.
  - `g_kept_in_window`, `g_notch_over_0_01` / `_0_1` / `_0_5`, `g_notch_max_m` and `g_kept_dry`.
  - The inlet-origin counts are on main already (#1639).
  - The spike's uphill dump hook (an environment toggle) is not ported.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Spec delta: BUILD-060.
- Crates: xtask (`rivergate.rs`'s `b_bars`; `RELIEF_LOST_SHARE`, `RELIEF_LOST_CAP_M`), cache (`river_gate.rs`'s `spill_notch`).
- Tests (tier A): `the_relief_lost_bar_allows_three_percent_of_the_relief_index_capped_at_2_m`; the notch in `kept_lakes_get_their_own_bars_beside_the_raw_closed_basin_share_and_a_spill_mismatch_is_a_violation`.
- Five-window rows for B and RG before and after are in the PR body.
