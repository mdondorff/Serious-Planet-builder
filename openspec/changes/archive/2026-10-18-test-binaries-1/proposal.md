## Why
SPIKE-15 step 3: every integration-test file is its own binary, compiled and linked on its own, so an edit relinks about 100 binaries (frame edit 89 s, spike results). One binary per tier shortens the build of an edit; the cost is a module prefix on test names and a rework of the real-GPU group.

## What Changes
- `generators` and `app` build their tests as one binary per tier: `it` (A), `oracle_all` (B), `gpu_all` (C, app), each including the test files as `#[path]` modules; `autotests = false`.
- `.config/nextest.toml`: the `real-gpu-heavy` group names the app's heavy GPU tests by `package(=planet) and binary(gpu_all) and test(/^(gpu_flight|...)::/)`.
- xtask tests: the real-GPU guard reads `[[test]]` names and checks the modules; a new guard checks that every test file is a module of the aggregator of its tier and of no other.
- BUILD-015.

## Measured (build half of `test fast`, `cargo nextest run --no-run` of the edited crate and every crate above it, median of runs 1 to 3 after a warm-up edit, system CPU load sampled every 4 s, 30 to 40 % mean from other sessions; Rasierklinge)
| Edit | Before | After | Change |
|---|---|---|---|
| app | 11.3 s (load mean 38 %) | 6.5 s (40 %) | -43 % |
| generators, all four converted | 58.6 s (46 %) | 35.4 s (33 %) | -40 % |
| generators, only the other crates converted vs. generators too | 38.8 s (36 %) | 34.5 s (35 %) | -11 % |
| cache, cache converted vs. not | 34.6 s (33 %) | 32.0 s (32 %) | -7 % |
| streaming (chain includes app) | 40.9 s (42 %) | 35.0 s (37 %) | -15 %, of which the app conversion is about 5 of 6 s |

By the rule (convert only where the gain is more than about 10 %, opus-planner 2026-10-18), `generators` and `app` stay converted; `cache` and `streaming` were converted, measured and returned to their layout. `frame` and `render` follow after #304.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling. No golden moves.

## Out of scope
frame, render, cache, streaming; the CI time per job (measured after merge with `cargo xtask ci-stats`).
