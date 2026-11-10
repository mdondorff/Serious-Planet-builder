## Why
Inbox #122 (2026-11-10): BUILD-042 compared the re-estimate by date, so a second re-estimate on one day (items 11, 13 and 18 done after the base's 2026-11-10 line) could not pass.

## What Changes
- `xtask::process::reestimate_stamp` (new) and `reestimate_errors`: the `Re-estimate:` line may carry a time `HH:MM`; stamps compare as a whole (a date alone is 00:00); the same stamp and an earlier stamp are refused.
- BUILD-042 changed in `openspec/specs/build-tooling` and in its identical copy in `openspec/changes/process-checks` (a second delta of the same ID would be a duplicate, and an ADDED copy that differs from the main spec is refused by spec-lint); no new ID.
- Tier-A tests for the date-only pair, a later time on the same day, the same time, an earlier time, and a date-only base or line.

## Impact
- Code: `xtask/src/process.rs` only. A date-only line keeps passing when its date is later than the base's. An earlier date, which the old check let through because it only asked for a different date, is now refused (the check moves forward only; not weakened).
