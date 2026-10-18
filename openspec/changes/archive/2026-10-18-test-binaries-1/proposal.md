## Why
SPIKE-15 step 3: every integration-test file is its own binary, compiled and linked on its own, so an edit relinks about 100 binaries (frame edit 89 s, spike results). One binary per tier shortens the build of an edit; the cost is a module prefix on test names and a rework of the real-GPU group.

## What Changes
- `generators`, `cache`, `streaming` and `app` build their tests as one binary per tier: `it` (A), `oracle_all` (B), `gpu_all` (C, app), each including the test files as `#[path]` modules; `autotests = false`.
- `.config/nextest.toml`: the `real-gpu-heavy` group names the heavy tests of the converted crates by package, binary and module (`planet::gpu_all`: `gpu_flight`, `gpu_stream_perf`, `gpu_water_coverage`, `gpu_world_field`; `planet-cache::oracle_all`: `oracle_concurrent_source`).
- cache's `pack` test that re-runs its own binary by name builds the helper's name with `module_path!()`.
- xtask tests: the real-GPU guard reads `[[test]]` names and checks the modules; a guard parses every aggregator and requires that it declares exactly the test files of its tier, each once (a commented-out or misnamed module does not count); a tier-B test (`testkit::oracle_test_layout`) compares what `cargo nextest list` reports for the per-tier binaries with the `#[test]` functions of the test files.
- BUILD-015.

## Measured (build half of `test fast`: `cargo nextest run --no-run` of the edited crate and every crate above it; median of runs 1 to 3 after a warm-up edit; system CPU load sampled every 4 s, 30 to 45 % mean from other sessions; Rasierklinge)
| Edit | Before | After | Change |
|---|---|---|---|
| app | 11.3 s (load 38 %) | 6.5 s (40 %) | -43 % |
| generators, all four converted | 58.6 s (46 %) | 35.4 s (33 %) | -40 % |
| generators, `generators` and `app` converted vs. all four (two rounds a, b, a, b, 6 builds each) | 38.8 s (35 %) | 33.7 s (36 %) | -13 % |
| generators, `cache` and `streaming` not converted vs. `generators` too (one round) | 38.8 s (36 %) | 34.5 s (35 %) | -11 % |
| cache, cache converted vs. not (one round) | 34.6 s (33 %) | 32.0 s (32 %) | -7 % |

The build of an edit relinks the test binaries of every crate above it, so the chain is the measure, not one crate (opus-planner, 2026-10-18). A generators edit passes through cache, streaming, frame, render, editor and app: with all four converted it beats `generators` and `app` alone by 13 %, over the 10 % rule, so cache and streaming stay converted. `frame` and `render` follow after #304.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling. No golden moves.

## Out of scope
frame, render; the CI time per job (measured after merge with `cargo xtask ci-stats`).
