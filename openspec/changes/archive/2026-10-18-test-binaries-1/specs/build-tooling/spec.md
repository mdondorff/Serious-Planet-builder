## ADDED Requirements

### Requirement: BUILD-015 One test binary per tier in a crate with many test files
A crate with more than five test files SHALL build its integration tests as one binary per tier (SPIKE-15 step 3): `autotests = false` in its `Cargo.toml`, and a `[[test]]` for each tier it has, named `it` (tier A), `oracle_all` (tier B) and `gpu_all` (tier C) after the tier-prefix rule of BUILD-003, each of `tests/<name>.rs` including the crate's test files of that tier as modules (`#[path = "<file>.rs"] mod <file>;`, in the file's own directory, so `support/` and `data/` paths keep working). A new test file SHALL be added to the aggregator of its tier. The test names gain the module as prefix (`<file>::<test>`); the tier filters of BUILD-003 (`binary(/^gpu_/)`, `binary(/^oracle_/)`) are unchanged. The `real-gpu` profile of `.config/nextest.toml` SHALL name the heavy tests of such a binary by package, binary and module (`package(=planet) and binary(gpu_all) and test(/^(gpu_flight|...)::/)`), and the xtask guard SHALL check that every binary named there is a test file or a `[[test]]` and every module named is a test file. The crates done: `generators`, `cache`, `streaming`, `app`; `frame` and `render` follow.

Verify: A
Status: active
Source: SPIKE-15 (step 3), BUILD-003, BUILD-011, owner (2026-10-18)

#### Scenario: Tiers are still selected by name
- **WHEN** `cargo xtask test fast`, `test` and `test gpu` select tests of a converted crate
- **THEN** tier A is the `it` binary, tier B `oracle_all` and tier C `gpu_all`, no test is lost (each crate's test count is the number of its `#[test]` functions) and the tier-A filter `not binary(/^gpu_/) and not binary(/^oracle_/)` still matches `it`

#### Scenario: The heavy GPU tests keep their group
- **WHEN** `test gpu --real` runs under the `real-gpu` profile
- **THEN** `cargo nextest show-config test-groups --profile real-gpu` lists the `gpu_flight`, `gpu_stream_perf`, `gpu_water_coverage` and `gpu_world_field` tests of `planet::gpu_all` and the `oracle_concurrent_source` tests of `planet-cache::oracle_all` in the group `real-gpu-heavy`, and a misspelt binary or module name fails the xtask guard

#### Scenario: A test that starts itself again
- **WHEN** a test runs its own binary again by test name (`pack::a_writer_process_that_is_killed_releases_its_lock`)
- **THEN** it passes the helper's name with its module prefix (`module_path!()` without the crate), so it works in the per-tier binary
