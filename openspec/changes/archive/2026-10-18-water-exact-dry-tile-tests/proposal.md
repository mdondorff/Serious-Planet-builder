## Why
Follow-up to water-exact-dry-tile (#286): the pre-filled-tile branch of the tile job had no test of its own, and the reduced tier B frame sampling was only in the PR body.

## What Changes
- ATMO-020: scenario "Pre-filled tiles" (tier B, `oracle_water_prefilled.rs`) and a known-limit note on the reduced tier B sampling (3 of 5 flight frames).
- No code change.

## Impact
Spec and tests only. No ADR.
