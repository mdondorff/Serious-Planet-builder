## Why
Since #1032 an ignored test's citation counts for no tier. Two real-store scenarios are covered only by ignored tests and did not say who runs them (planner inbox #45 item 1).

## What Changes
- TEST-076: a scenario "The pair over a real store (by hand)": the two ignored `gpu_edit_pair` real-store tests, run by hand for the owner's drag report, not by a tier, `gate` or `accept`; the owner checks the pictures.
- TEST-079: the scenario "SPIKE-55 reproduced" says it is run by hand (by the lane that changes the scorecard or the generator rows, or the owner), not by a tier, `gate` or `accept`.

## Not in this change (planner inbox #45 item 2)
spec-lint judges only the first tier of `Verify: A, B` and the like. Checking every declared tier would fail 14 of the 55 active multi-tier requirements today, each needing a test or a tier change, so it is a Tracked item for the planner rather than part of this PR:

| Requirement | Verify | Cited by tiers |
|---|---|---|
| GEN-021, GEN-025 | A, B | A, C |
| GEN-026 | A, B, C | A, C |
| GEN-073, GEN-088 | A, C | A |
| REND-030 | A, B, C | A |
| LOD-011 | A, C | A, B |
| TEST-057, TEST-058 | A, C | A |
| TEST-078, TEST-079 | A, B | A |
| VIEW-023, VIEW-024, VIEW-026 | A, C | A |

(Counted with the rule of #1032: ignored tests count for no tier; `fn perf_*` scenes under `crates/app/src/` count as C.)

## Capabilities
### Modified Capabilities
- `testing`: TEST-076 (a scenario), TEST-079 (a clause of a scenario).

## Impact
- Specs only.
