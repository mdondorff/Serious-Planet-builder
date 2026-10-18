## ADDED Requirements

### Requirement: BUILD-015 One test binary per tier where it shortens the edit-to-test build
A crate whose conversion cuts the build of an edit by more than about 10 % (median of three edits on a machine with the same load, SPIKE-15 step 3) SHALL build its integration tests as one binary per tier: `autotests = false` in its `Cargo.toml`, and a `[[test]]` for each tier it has, named `it` (tier A), `oracle_all` (tier B) and `gpu_all` (tier C) after the tier-prefix rule of BUILD-003, each of `tests/<name>.rs` including the crate's test files of that tier as modules (`#[path = "<file>.rs"] mod <file>;`, in the file's own directory, so `support/` and `data/` paths keep working). A new test file SHALL be added to the aggregator of its tier. The test names gain the module as prefix (`<file>::<test>`); the tier filters of BUILD-003 (`binary(/^gpu_/)`, `binary(/^oracle_/)`) are unchanged. The `real-gpu` profile of `.config/nextest.toml` SHALL name the heavy tests of such a binary by package, binary and module (`package(=planet) and binary(gpu_all) and test(/^(gpu_flight|...)::/)`), and the xtask guard SHALL check that every binary named there is a test file or a `[[test]]` and every module named is a test file. The crates done: `generators` (edit of generators: 38.8 to 34.5 s, 11 %) and `app` (edit of app: 11.3 to 6.5 s, 43 %); measured and left as they are: `cache` (edit of cache: 34.6 s without, 32.0 s with, 7 %) and `streaming` (its edit chain gained 15 %, of which the app conversion is about 5 of 6 s); `frame` and `render` follow after the removal of the vertex render path.

Verify: A
Status: active
Source: SPIKE-15 (step 3), BUILD-003, BUILD-011, owner and opus-planner (2026-10-18)

#### Scenario: Tiers are still selected by name
- **WHEN** `cargo xtask test fast`, `test` and `test gpu` select tests of a converted crate
- **THEN** tier A is the `it` binary, tier B `oracle_all` and tier C `gpu_all`, no test is lost (each crate's test count is the number of its `#[test]` functions) and the tier-A filter `not binary(/^gpu_/) and not binary(/^oracle_/)` still matches `it`

#### Scenario: Every test file is in its aggregator
- **WHEN** a crate with `autotests = false` has a test file that no aggregator declares, declares twice, declares in the aggregator of another tier or only in a comment, or an aggregator declares a file that does not exist
- **THEN** the xtask guard fails naming the crate and the aggregator

#### Scenario: No test is lost
- **WHEN** `cargo xtask test` (tier B, `oracle_test_layout` of `testkit`) compares the tests `cargo nextest list` reports for the `it`, `oracle_all` and `gpu_all` binaries of a converted crate with the `#[test]` functions of its test files (an `#[ignore]`d one is not listed)
- **THEN** the two sets are equal as `<file>::<function>`, and a test file left out of an aggregator fails the comparison naming the missing tests

#### Scenario: The heavy GPU tests keep their group
- **WHEN** `test gpu --real` runs under the `real-gpu` profile
- **THEN** `cargo nextest show-config test-groups --profile real-gpu` lists the `gpu_flight`, `gpu_stream_perf`, `gpu_water_coverage` and `gpu_world_field` tests of `planet::gpu_all` in the group `real-gpu-heavy`, and a misspelt binary or module name fails the xtask guard
