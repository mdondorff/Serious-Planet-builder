## Why
ADR 0097 (#1123, owner 2026-11-03) retired GEN-014's ten-minute limit on the field bake. The spec now reports the bake's wall time against terrain's world-build share (BUDG-014). The `global-bake` perf scene still failed a bake over 600 s; today's round-2 bake took 1020.2 s. Inbox #78 (opus-planner, 2026-11-03).

## What Changes
- `crates/app/src/globalperf.rs`:
  - `budget_failures` judges only the 8 GB of resident blocks (CON-21); a time that was not measured is still no result.
  - `share_report` prints the bake's wall time as a percentage of terrain's world-build share: `TERRAIN_BUILD_SHARE_S`, 40 % of 24 h, values in BUDG-014.
  - The log's one-off reading names that share as its reference.
- The `cargo xtask perf global-bake` usage text says the same.
- The coupled run's GEN-042 limit is unchanged.

## Impact
- No spec delta: GEN-014 and BUDG-014 already say this (#1123).
- The tier-A test `the_memory_budget_is_eight_gigabytes_and_the_time_is_reported_against_terrains_share` (GEN-014) replaces `the_budgets_are_ten_minutes_and_eight_gigabytes`. The claim it drops is the one ADR 0097 retired.
