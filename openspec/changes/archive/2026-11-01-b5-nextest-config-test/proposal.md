## Why
#1021 (`b5-test-limits`, BUILD-003) added two 450 s limits, five tests to the `heavy-window` group and the `gpu-tier` cap of 8 to `.config/nextest.toml`, but the xtask config test does not check them (planner inbox #40; tracked in `docs/status.md`). Its assertion "the default profile has no test group" passes only because the default profile's group overrides sit after `[profile.real-gpu]` in the file: it guards nothing.

## What Changes
- `xtask/src/testcmd.rs`, a parser of the config's override tables (profile, filter, text, in file order) and a new test (`// spec: BUILD-003`):
  - the `gpu-tier` group is at most 8 and `heavy-window` at most 2;
  - each of the five declared 450 s limits is its own override, before the `gpu_` override (the first match sets the value), and names an existing test function;
  - the `heavy-window` override names the six heavy modules and the three `gpu_skeleton` loop tests (each an existing test function) and comes before the `gpu-tier` override, which is `binary(gpu_all)`;
  - every group of the default profile holds `gpu_all` tests only (tier A and B are not capped).
- The real-GPU test's vacuous assertion is replaced: the `real-gpu-heavy` group is used only by the `real-gpu` profile.
- No config change, no test skipped or weakened; three planted config errors (cap 9, a renamed heavy test, a group over `all()`) each fail the new test.

## Capabilities
### Modified Capabilities
- None: the tests check BUILD-003 as it reads since #1021; no spec delta.

## Impact
- `xtask/src/testcmd.rs` (tests only). Tier A.
