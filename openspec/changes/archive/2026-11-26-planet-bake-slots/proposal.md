## Why
- Four prepushes of #1662 (2026-11-25) failed `test fast`: two to four of the planet library's bake tests ran past the 180 s limit.
  - Alone they take 31-40 s; in the 3,233-test set of every crate, 59-114 s.
  - A render, editor and app change selects only 816 tests. The bake tests then start together at the end of the run and starve each other.
- The lead ordered a slot rule for them, with timings before and after and no limit change (2026-11-26; tracked in #1666).

## What changes
- BUILD-070: the planet library's 23 bake tests of 10 s or more take `threads-required = 4` each, with the default limit and no test group. Tier A takes no test group (BUILD-003), so this is BUILD-045's form, not a group.
- TEST-156: a tier-A test checks the override, the limit, the absence of a group, and that each named test is a `#[test]` function of `crates/app/src`.
- No test body, limit, skip or retry changes.

## Expected golden changes:
- none: nextest configuration and an xtask test only

## Impact
- `.config/nextest.toml`; `xtask/src/repocheck.rs`.
- Specs: BUILD-070 (build-tooling), TEST-156 (testing).
